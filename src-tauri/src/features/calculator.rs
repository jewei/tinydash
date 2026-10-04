//! Arithmetic, unit conversion, and currency conversion through fend.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crate::{
    actions::Action,
    features::currency::Rates,
    search::result::{Icon, ResultAction, ResultKind, SearchResult, Symbol},
};

/// fend checks this deadline while it works, so huge inputs cannot hang search.
const TIME_LIMIT: Duration = Duration::from_millis(100);

pub fn answer(query: &str, rates: Option<&Rates>) -> Option<SearchResult> {
    let (value, used_rates) = evaluate(query, rates)?;
    let subtitle = match rates {
        Some(rates) if used_rates => format!("{query} · ECB rates from {}", rates.date),
        _ => format!("= {query}"),
    };
    Some(SearchResult {
        id: format!("calc:{query}"),
        kind: ResultKind::Calculation,
        title: value.clone(),
        subtitle,
        icon: Icon::Symbol {
            name: Symbol::Calculator,
        },
        actions: vec![ResultAction::new(
            "Copy Result",
            Action::Copy { text: value },
        )],
        pinned: false,
    })
}

/// The result text and whether it needed exchange rates.
fn evaluate(query: &str, rates: Option<&Rates>) -> Option<(String, bool)> {
    if !query.chars().any(|c| c.is_ascii_digit()) && !matches!(query, "pi" | "π") {
        return None;
    }
    let mut context = fend_core::Context::new();
    context.disable_rng();
    // fend matches units case-insensitively and reads `Bps` as bits. Define
    // bytes per second so `8 Mbps to MBps` gives 1 MBps.
    context.define_custom_unit_v1(
        "Bps",
        "",
        "byte / second",
        &fend_core::CustomUnitAttribute::AllowShortPrefix,
    );
    let used_rates = Arc::new(AtomicBool::new(false));
    if let Some(rates) = rates {
        context.set_exchange_rate_handler_v2(RateLookup {
            rates: rates.clone(),
            used: used_rates.clone(),
        });
    }
    let deadline = Deadline(Instant::now() + TIME_LIMIT);
    let input = currency_shorthand(query).unwrap_or_else(|| query.to_owned());
    let result = fend_core::evaluate_preview_with_interrupt(&input, &context, &deadline);
    let is_number = result
        .get_main_result_spans()
        .any(|span| span.kind() == fend_core::SpanKind::Number);
    let value = result.get_main_result().trim().to_owned();
    if !is_number || value.is_empty() {
        return None;
    }
    // A bare number echoes itself; that is not an answer.
    if value == query.trim() {
        return None;
    }
    if !used_rates.load(Ordering::Relaxed) {
        return Some((value, false));
    }
    Some((money(&value).unwrap_or(value), true))
}

/// `approx. 401.7094 MYR` as `401.71 MYR`. The subtitle already says the
/// value comes from daily rates. (fend's own `to 2 dp` rounds incorrectly.)
fn money(value: &str) -> Option<String> {
    let (amount, currency) = value.trim_start_matches("approx. ").split_once(' ')?;
    let amount: f64 = amount.parse().ok()?;
    Some(format!("{amount:.2} {currency}"))
}

/// `100 USD MYR` means `100 USD to MYR`.
fn currency_shorthand(query: &str) -> Option<String> {
    let [amount, from, to] =
        <[&str; 3]>::try_from(query.split_whitespace().collect::<Vec<_>>()).ok()?;
    let is_code = |code: &str| code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase());
    (amount.parse::<f64>().is_ok() && is_code(from) && is_code(to))
        .then(|| format!("{amount} {from} to {to}"))
}

struct Deadline(Instant);

impl fend_core::Interrupt for Deadline {
    fn should_interrupt(&self) -> bool {
        Instant::now() >= self.0
    }
}

struct RateLookup {
    rates: Rates,
    used: Arc<AtomicBool>,
}

impl fend_core::ExchangeRateFnV2 for RateLookup {
    fn relative_to_base_currency(
        &self,
        currency: &str,
        _: &fend_core::ExchangeRateFnV2Options,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        self.used.store(true, Ordering::Relaxed);
        self.rates
            .per_euro(currency)
            .ok_or_else(|| format!("No exchange rate for {currency}").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(query: &str) -> Option<String> {
        evaluate(query, None).map(|(value, _)| value)
    }

    #[test]
    fn evaluates_arithmetic_and_units() {
        assert_eq!(value("12 * 8").as_deref(), Some("96"));
        assert_eq!(value("sqrt(144)").as_deref(), Some("12"));
        assert_eq!(value("8 Mbps to MBps").as_deref(), Some("1 MBps"));
        assert!(value("5 ft to cm").is_some_and(|v| v.starts_with("152.4")));
    }

    #[test]
    fn ignores_text_bare_numbers_and_errors() {
        assert_eq!(value("safari"), None);
        assert_eq!(value("42"), None);
        assert_eq!(value("1password"), None);
        assert_eq!(value("12 +"), None);
    }

    #[test]
    fn converts_currency_with_cached_rates() {
        let rates = Rates::fixture();
        let (value, used) = evaluate("100 USD MYR", Some(&rates)).unwrap();
        assert!(used);
        assert_eq!(value, "401.71 MYR");
        let answer = answer("100 USD to MYR", Some(&rates)).unwrap();
        assert!(answer.subtitle.contains("2026-10-02"));
        assert_eq!(evaluate("100 USD to MYR", None), None);
    }

    #[test]
    fn rounds_money_to_cents() {
        assert_eq!(
            money("approx. 401.7094017094 MYR").as_deref(),
            Some("401.71 MYR")
        );
        assert_eq!(money("400 MYR").as_deref(), Some("400.00 MYR"));
        assert_eq!(money("MYR"), None);
    }
}
