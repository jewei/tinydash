//! Small HTTPS downloads for the features that need them (exchange rates,
//! weather). Only `refresh.rs` runs them.

use std::{io::Read, time::Duration};

use crate::error::{Error, Result};

/// Download `url` and return at most `max_bytes` of its body. Blocks for up
/// to ten seconds. `what` names the download in the error message.
pub fn get(url: &str, max_bytes: u64, what: &str) -> Result<Vec<u8>> {
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
        .get(url)
        .call()
        .map_err(|error| Error::msg(format!("Could not download {what}: {error}")))?;
    let mut body = Vec::new();
    response
        .into_body()
        .into_reader()
        .take(max_bytes)
        .read_to_end(&mut body)?;
    Ok(body)
}
