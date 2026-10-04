//! Daily ECB exchange rates from Frankfurter. Only the rate table is
//! downloaded; queries and amounts never leave the machine.

use std::{collections::BTreeMap, io::Read, time::Duration};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const ENDPOINT: &str = "https://api.frankfurter.dev/v1/latest";
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;
/// Rates older than this are refreshed when the launcher opens.
pub const MAX_AGE_SECONDS: i64 = 12 * 60 * 60;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rates {
    /// Publication date, `YYYY-MM-DD`.
    pub date: String,
    /// Unix seconds of the download.
    pub fetched_at: i64,
    /// Units of each currency per euro.
    rates: BTreeMap<String, f64>,
}

impl Rates {
    pub fn per_euro(&self, code: &str) -> Option<f64> {
        if code == "EUR" {
            return Some(1.0);
        }
        self.rates.get(code).copied()
    }

    pub fn is_stale(&self, now: i64) -> bool {
        now - self.fetched_at >= MAX_AGE_SECONDS
    }

    /// Parse and check a Frankfurter `latest` response.
    pub fn parse(json: &[u8], now: i64) -> Result<Self> {
        #[derive(Deserialize)]
        struct Response {
            base: String,
            date: String,
            rates: BTreeMap<String, f64>,
        }
        let response: Response = serde_json::from_slice(json)?;
        let valid_code =
            |code: &str| code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase());
        let valid = response.base == "EUR"
            && chrono::NaiveDate::parse_from_str(&response.date, "%Y-%m-%d").is_ok()
            && !response.rates.is_empty()
            && response
                .rates
                .iter()
                .all(|(code, rate)| valid_code(code) && rate.is_finite() && *rate > 0.0);
        if !valid {
            return Err(Error::msg(
                "The exchange rate service sent unexpected data.",
            ));
        }
        Ok(Self {
            date: response.date,
            fetched_at: now,
            rates: response.rates,
        })
    }

    #[cfg(test)]
    pub fn fixture() -> Self {
        Self::parse(
            br#"{"amount":1.0,"base":"EUR","date":"2026-10-02","rates":{"USD":1.25,"MYR":5.0}}"#,
            0,
        )
        .unwrap()
    }
}

/// Download the latest rates. Blocks for up to ten seconds.
pub fn fetch(now: i64) -> Result<Rates> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into();
    let response = agent
        .get(ENDPOINT)
        .call()
        .map_err(|error| Error::msg(format!("Could not download exchange rates: {error}")))?;
    let mut body = Vec::new();
    response
        .into_body()
        .into_reader()
        .take(MAX_RESPONSE_BYTES)
        .read_to_end(&mut body)?;
    Rates::parse(&body, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_checks_rates() {
        let rates = Rates::fixture();
        assert_eq!(rates.per_euro("EUR"), Some(1.0));
        assert_eq!(rates.per_euro("MYR"), Some(5.0));
        assert_eq!(rates.per_euro("XYZ"), None);
        assert!(!rates.is_stale(MAX_AGE_SECONDS - 1));
        assert!(rates.is_stale(MAX_AGE_SECONDS));
        for bad in [
            r#"{"base":"USD","date":"2026-10-02","rates":{"EUR":1.0}}"#,
            r#"{"base":"EUR","date":"yesterday","rates":{"USD":1.0}}"#,
            r#"{"base":"EUR","date":"2026-10-02","rates":{"usd":1.0}}"#,
            r#"{"base":"EUR","date":"2026-10-02","rates":{"USD":-1.0}}"#,
        ] {
            assert!(Rates::parse(bad.as_bytes(), 0).is_err(), "{bad}");
        }
    }
}
