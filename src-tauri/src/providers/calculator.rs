use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use fend_core::{Context, Interrupt, SpanKind};

use crate::{
    currency::Rates,
    launcher::result::{Action, ResultKind, SearchResult},
    ranking,
};

const TIME_LIMIT: Duration = Duration::from_millis(50);
const COPY_CACHE_LIMIT: usize = 32;

fn implicit_currency_pair(query: &str) -> Option<String> {
    let [amount, source, target] = query
        .split_whitespace()
        .collect::<Vec<_>>()
        .try_into()
        .ok()?;
    if !simple_number(amount) || !currency_code(source) || !currency_code(target) {
        return None;
    }
    Some(format!("{amount} {source} to {target}"))
}

fn simple_number(value: &str) -> bool {
    let value = value.strip_prefix(['+', '-']).unwrap_or(value);
    let mut dots = 0;
    let mut digits = 0;
    for character in value.chars() {
        if character == '.' {
            dots += 1;
        } else if character.is_ascii_digit() {
            digits += 1;
        } else {
            return false;
        }
    }
    digits > 0 && dots <= 1
}

fn currency_code(value: &str) -> bool {
    value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_uppercase())
}

#[derive(Debug, thiserror::Error)]
pub enum CalculationError {
    #[error("This calculation took too long. Try a smaller expression.")]
    Timeout,
    #[error("Enter an arithmetic expression or a unit conversion.")]
    NotNumeric,
    #[error("The result is too long to display.")]
    TooLong,
    #[error("{0}")]
    Evaluation(String),
    #[error("{0}")]
    Currency(String),
}

struct Deadline(Instant);

impl Interrupt for Deadline {
    fn should_interrupt(&self) -> bool {
        Instant::now() >= self.0
    }
}

#[derive(Default)]
pub struct CalculatorProvider {
    // Keep a few issued values so an overlapping search cannot change the value
    // of a displayed result. Copy never re-evaluates or accepts frontend text.
    copies: VecDeque<(String, String)>,
    next_id: u64,
    pub rates: Option<Arc<Rates>>,
}

impl CalculatorProvider {
    pub fn is_candidate(query: &str) -> bool {
        query
            .chars()
            .any(|c| c.is_ascii_digit() || "()+-*/^%".contains(c))
            || matches!(query, "pi" | "π")
    }

    pub fn search(&mut self, query: &str) -> Result<SearchResult, CalculationError> {
        let calculation = calculate(
            query,
            &Deadline(Instant::now() + TIME_LIMIT),
            self.rates.clone(),
        )?;
        let value = calculation.value;
        self.next_id = self.next_id.wrapping_add(1);
        let id = format!("calculation:{}", self.next_id);
        self.copies.push_back((id.clone(), value.clone()));
        if self.copies.len() > COPY_CACHE_LIMIT {
            self.copies.pop_front();
        }
        Ok(SearchResult {
            id,
            kind: ResultKind::Calculation,
            path: None,
            title: value,
            subtitle: calculation
                .rate_label
                .map_or_else(|| query.to_owned(), |label| format!("{query} · {label}")),
            score: ranking::CALCULATION_SCORE,
            icon: None,
            primary_action: Action::Copy,
            secondary_actions: vec![],
            pin: None,
            confirmation: None,
            detail: None,
        })
    }

    pub fn copy_value(&self, id: &str) -> Option<&str> {
        self.copies
            .iter()
            .find(|(key, _)| key == id)
            .map(|(_, value)| value.as_str())
    }
}

struct Calculation {
    value: String,
    rate_label: Option<String>,
}

struct RateLookup {
    rates: Option<Arc<Rates>>,
    used: Arc<AtomicBool>,
}

impl fend_core::ExchangeRateFnV2 for RateLookup {
    fn relative_to_base_currency(
        &self,
        currency: &str,
        _: &fend_core::ExchangeRateFnV2Options,
    ) -> Result<f64, Box<dyn std::error::Error + Send + Sync>> {
        self.used.store(true, Ordering::Relaxed);
        let rates = self
            .rates
            .as_ref()
            .ok_or(crate::currency::Error::Unavailable)?;
        Ok(rates.rate(currency)?)
    }
}

fn calculate(
    query: &str,
    interrupt: &impl Interrupt,
    rates: Option<Arc<Rates>>,
) -> Result<Calculation, CalculationError> {
    if interrupt.should_interrupt() {
        return Err(CalculationError::Timeout);
    }
    // No persistent variables, statements, strings, or user-defined functions.
    if query.is_empty() || query.contains([';', '=', '\\', '"', '\'']) {
        return Err(CalculationError::NotNumeric);
    }
    let mut context = Context::new();
    context.disable_rng();
    let used_rates = Arc::new(AtomicBool::new(false));
    context.set_exchange_rate_handler_v2(RateLookup {
        rates: rates.clone(),
        used: Arc::clone(&used_rates),
    });
    let evaluation = implicit_currency_pair(query).unwrap_or_else(|| query.to_owned());
    let result = fend_core::evaluate_with_interrupt(&evaluation, &mut context, interrupt).map_err(
        |error| {
            if interrupt.should_interrupt() {
                CalculationError::Timeout
            } else if used_rates.load(Ordering::Relaxed) {
                CalculationError::Currency(if rates.is_none() {
                    crate::currency::Error::Unavailable.to_string()
                } else {
                    error
                })
            } else {
                CalculationError::Evaluation(error)
            }
        },
    )?;
    if !result
        .get_main_result_spans()
        .any(|span| span.kind() == SpanKind::Number)
    {
        return Err(CalculationError::NotNumeric);
    }
    let value = result.get_main_result();
    if value.len() > 512 || value.chars().any(char::is_control) {
        return Err(CalculationError::TooLong);
    }
    Ok(Calculation {
        value: value.to_owned(),
        rate_label: rates
            .filter(|_| used_rates.load(Ordering::Relaxed))
            .map(|rates| rates.label(ranking::now())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct NoInterrupt;
    impl Interrupt for NoInterrupt {
        fn should_interrupt(&self) -> bool {
            false
        }
    }

    fn calculate(query: &str, interrupt: &impl Interrupt) -> Result<String, CalculationError> {
        super::calculate(query, interrupt, None).map(|calculation| calculation.value)
    }

    #[test]
    fn currency_uses_only_cached_rates_and_preserves_issued_copy_values() {
        let mut provider = CalculatorProvider {
            rates: Some(Arc::new(crate::currency::fixture())),
            ..CalculatorProvider::default()
        };
        let result = provider.search("100 USD to MYR").expect("conversion");
        assert_eq!(result.title, "400 MYR");
        assert!(result.subtitle.contains("ECB 2026-09-16"));
        let mut changed = crate::currency::fixture();
        changed.values.insert("MYR".into(), 6.0);
        provider.rates = Some(Arc::new(changed));
        let updated = provider.search("100 USD to MYR").expect("new conversion");
        assert_eq!(updated.title, "480 MYR");
        assert_eq!(provider.copy_value(&result.id), Some(result.title.as_str()));
        let plain = provider.search("12 * 8").expect("arithmetic");
        assert_eq!(plain.subtitle, "12 * 8");
    }

    #[test]
    fn accepts_one_anchored_implicit_iso_currency_pair() {
        let rates = Some(Arc::new(crate::currency::fixture()));
        assert_eq!(
            super::calculate("10 USD EUR", &NoInterrupt, rates)
                .expect("implicit currency pair")
                .value,
            "8 EUR"
        );
        assert!(super::calculate("10 usd eur", &NoInterrupt, None).is_err());
        assert!(super::calculate("10 USD CAD extra", &NoInterrupt, None).is_err());
    }

    #[test]
    fn arithmetic_and_units_work_offline_and_preserve_case() {
        for (query, expected) in [
            ("12 * 8", "96"),
            ("sqrt(144)", "12"),
            ("5 ft to cm", "152.4 cm"),
            ("32 C to F", "89.6 °F"),
            ("(128 * 1.2) + 64", "217.6"),
        ] {
            assert_eq!(
                calculate(query, &NoInterrupt).expect(query),
                expected,
                "{query}"
            );
        }
        let speed = calculate("20 km/h to mph", &NoInterrupt).expect("speed");
        assert!(
            speed.contains("12.427") && speed.ends_with("mph"),
            "{speed}"
        );
    }

    #[test]
    fn rejects_invalid_inputs_and_never_fabricates_exchange_rates() {
        for query in [
            "1 / 0",
            "12 +",
            "x = 2; x",
            "\"hello\"",
            "sqrt",
            "100 USD to MYR",
        ] {
            assert!(calculate(query, &NoInterrupt).is_err(), "{query}");
        }
        let error = calculate("100 USD to MYR", &NoInterrupt).expect_err("missing rates");
        assert_eq!(
            error.to_string(),
            crate::currency::Error::Unavailable.to_string()
        );
    }

    #[test]
    fn interruption_is_cooperative_and_copy_values_are_bounded() {
        struct InterruptAfter(AtomicUsize);
        impl Interrupt for InterruptAfter {
            fn should_interrupt(&self) -> bool {
                self.0.fetch_add(1, Ordering::Relaxed) >= 10
            }
        }
        let interrupt = InterruptAfter(AtomicUsize::new(0));
        assert!(matches!(
            calculate("100000!", &interrupt),
            Err(CalculationError::Timeout)
        ));
        assert!(interrupt.0.load(Ordering::Relaxed) > 10);
        let mut provider = CalculatorProvider::default();
        let first = provider.search("12 * 8").expect("calculation");
        provider.search("1 + 1").expect("calculation");
        assert_eq!(provider.copy_value(&first.id), Some("96"));
        assert_eq!(provider.copy_value("calculation:forged"), None);
        for _ in 0..COPY_CACHE_LIMIT {
            provider.search("1 + 1").expect("calculation");
        }
        assert_eq!(provider.copy_value(&first.id), None);
        assert_eq!(provider.copies.len(), COPY_CACHE_LIMIT);
    }
}
