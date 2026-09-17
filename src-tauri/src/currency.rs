use std::{collections::BTreeMap, time::Duration};

use serde::{Deserialize, Serialize};

pub const FRESH_SECONDS: i64 = 24 * 60 * 60;
const MAX_RESPONSE: usize = 64 * 1024;
const ENDPOINT: &str = "https://api.frankfurter.dev/v2/rates?base=EUR&providers=ECB";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "Currency rates are unavailable. Connect to the internet, then open Actions and choose Refresh currency rates."
    )]
    Unavailable,
    #[error("No cached ECB rate is available for {0}.")]
    Unsupported(String),
    #[error("Invalid currency rates: {0}")]
    Invalid(&'static str),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Rates {
    pub date: String,
    pub fetched_at: i64,
    // Units of each currency per EUR. fend-core accepts any consistent base.
    pub values: BTreeMap<String, f64>,
}

impl Rates {
    pub fn validate(&self, now: i64) -> Result<(), Error> {
        if !valid_date(&self.date)
            || self.fetched_at < 0
            || self.fetched_at > now.saturating_add(300)
        {
            return Err(Error::Invalid("date or timestamp"));
        }
        if !(2..=200).contains(&self.values.len())
            || self.values.get("EUR") != Some(&1.0)
            || self.values.iter().any(|(code, rate)| {
                code.len() != 3
                    || !code.bytes().all(|byte| byte.is_ascii_uppercase())
                    || !rate.is_finite()
                    || *rate <= 0.0
                    || *rate > 1_000_000_000.0
            })
        {
            return Err(Error::Invalid("currency code or value"));
        }
        Ok(())
    }

    pub fn fresh(&self, now: i64) -> bool {
        now >= self.fetched_at && now - self.fetched_at < FRESH_SECONDS
    }

    pub fn rate(&self, code: &str) -> Result<f64, Error> {
        self.values
            .get(code)
            .copied()
            .ok_or_else(|| Error::Unsupported(code.into()))
    }

    pub fn label(&self, now: i64) -> String {
        format!(
            "ECB {}{}",
            self.date,
            if self.fresh(now) {
                ""
            } else {
                " · cached rates"
            }
        )
    }
}

fn valid_date(date: &str) -> bool {
    let parts: Vec<_> = date.split('-').collect();
    if parts.len() != 3
        || parts[0].len() != 4
        || parts[1].len() != 2
        || parts[2].len() != 2
        || !parts
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        parts[0].parse::<u32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<u32>(),
    ) else {
        return false;
    };
    let days = match month {
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        1..=12 => 31,
        _ => return false,
    };
    (1999..=9999).contains(&year) && (1..=days).contains(&day)
}

#[derive(Deserialize)]
struct Row {
    date: String,
    base: String,
    quote: String,
    rate: f64,
}

fn parse(bytes: &[u8], now: i64) -> anyhow::Result<Rates> {
    if bytes.len() > MAX_RESPONSE {
        return Err(Error::Invalid("response is too large").into());
    }
    let rows: Vec<Row> = serde_json::from_slice(bytes)?;
    let date = rows
        .first()
        .ok_or(Error::Invalid("empty response"))?
        .date
        .clone();
    let mut values = BTreeMap::new();
    for row in rows {
        if row.base != "EUR" || row.date != date || values.insert(row.quote, row.rate).is_some() {
            return Err(Error::Invalid("mixed dates, bases, or duplicate currencies").into());
        }
    }
    // Some sources include EUR/EUR; others omit the base currency.
    // Keep an explicit source value so validation can reject a value other than one.
    values.entry("EUR".to_owned()).or_insert(1.0);
    let rates = Rates {
        date,
        fetched_at: now,
        values,
    };
    rates.validate(now)?;
    Ok(rates)
}

// This is the exchange-rate source boundary. Changing the service only changes
// this request and parser. Queries, amounts, and clipboard text are never sent.
pub async fn fetch() -> anyhow::Result<Rates> {
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(0)
        .user_agent("TinyDash/0.1")
        .build()?;
    let mut response = client.get(ENDPOINT).send().await?.error_for_status()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE {
            return Err(Error::Invalid("response is too large").into());
        }
        bytes.extend_from_slice(&chunk);
    }
    parse(&bytes, crate::ranking::now())
}

#[cfg(test)]
pub fn fixture() -> Rates {
    Rates {
        date: "2026-09-16".into(),
        fetched_at: 1_789_516_800,
        values: BTreeMap::from([
            ("EUR".into(), 1.0),
            ("USD".into(), 1.25),
            ("MYR".into(), 5.0),
        ]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_sources_explicit_base_currency_row() {
        let bytes = br#"[{"date":"2026-09-16","base":"EUR","quote":"EUR","rate":1},{"date":"2026-09-16","base":"EUR","quote":"USD","rate":1.25}]"#;
        let rates = parse(bytes, 100).expect("source response with EUR");
        assert_eq!(rates.values.len(), 2);
        assert_eq!(rates.rate("EUR").expect("EUR"), 1.0);
    }

    #[test]
    fn accepts_daily_rates_and_rejects_invalid_or_inconsistent_responses() {
        let good = br#"[{"date":"2026-09-16","base":"EUR","quote":"USD","rate":1.25}]"#;
        let rates = parse(good, 100).expect("rates");
        assert_eq!(rates.rate("USD").expect("USD"), 1.25);
        assert_eq!(rates.rate("EUR").expect("EUR"), 1.0);
        assert!(rates.rate("XYZ").is_err());
        for bad in [
            "[]",
            "{}",
            "null",
            r#"[{"date":"2026-02-30","base":"EUR","quote":"USD","rate":1}]"#,
            r#"[{"date":"2026-09-16","base":"USD","quote":"MYR","rate":4}]"#,
            r#"[{"date":"2026-09-16","base":"EUR","quote":"USD","rate":0}]"#,
            r#"[{"date":"2026-09-16","base":"EUR","quote":"USD","rate":1},{"date":"2026-09-16","base":"EUR","quote":"USD","rate":2}]"#,
        ] {
            assert!(parse(bad.as_bytes(), 100).is_err(), "{bad}");
        }
        assert!(parse(&vec![b' '; MAX_RESPONSE + 1], 100).is_err());
    }

    #[test]
    fn stale_rates_remain_usable_and_clock_changes_do_not_mark_them_fresh() {
        let rates = fixture();
        assert!(rates.fresh(rates.fetched_at));
        assert!(!rates.fresh(rates.fetched_at - 1));
        assert!(!rates.fresh(rates.fetched_at + FRESH_SECONDS));
        assert!(
            rates
                .label(rates.fetched_at + FRESH_SECONDS)
                .contains("cached")
        );
        assert_eq!(rates.rate("MYR").expect("cached rate"), 5.0);
        assert!(rates.validate(rates.fetched_at - 301).is_err());
    }
}
