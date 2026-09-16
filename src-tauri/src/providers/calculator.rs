use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use fend_core::{Context, Interrupt, SpanKind};

use crate::{
    launcher::result::{Action, ResultKind, SearchResult},
    ranking,
};

const TIME_LIMIT: Duration = Duration::from_millis(50);
const COPY_CACHE_LIMIT: usize = 32;

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
}

impl CalculatorProvider {
    pub fn is_candidate(query: &str) -> bool {
        query
            .chars()
            .any(|c| c.is_ascii_digit() || "()+-*/^%".contains(c))
            || matches!(query, "pi" | "π")
    }

    pub fn search(&mut self, query: &str) -> Result<SearchResult, CalculationError> {
        let value = calculate(query, &Deadline(Instant::now() + TIME_LIMIT))?;
        self.next_id = self.next_id.wrapping_add(1);
        let id = format!("calculation:{}", self.next_id);
        self.copies.push_back((id.clone(), value.clone()));
        if self.copies.len() > COPY_CACHE_LIMIT {
            self.copies.pop_front();
        }
        Ok(SearchResult {
            id,
            kind: ResultKind::Calculation,
            title: value,
            subtitle: query.to_owned(),
            score: ranking::CALCULATION_SCORE,
            icon: None,
            primary_action: Action::Copy,
            secondary_actions: vec![],
            confirmation: None,
        })
    }

    pub fn copy_value(&self, id: &str) -> Option<&str> {
        self.copies
            .iter()
            .find(|(key, _)| key == id)
            .map(|(_, value)| value.as_str())
    }
}

fn calculate(query: &str, interrupt: &impl Interrupt) -> Result<String, CalculationError> {
    if interrupt.should_interrupt() {
        return Err(CalculationError::Timeout);
    }
    // No persistent variables, statements, strings, or user-defined functions.
    if query.is_empty() || query.contains([';', '=', '\\', '"', '\'']) {
        return Err(CalculationError::NotNumeric);
    }
    let mut context = Context::new();
    context.disable_rng();
    let result =
        fend_core::evaluate_with_interrupt(query, &mut context, interrupt).map_err(|error| {
            if interrupt.should_interrupt() {
                CalculationError::Timeout
            } else {
                CalculationError::Evaluation(error)
            }
        })?;
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
    Ok(value.to_owned())
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
