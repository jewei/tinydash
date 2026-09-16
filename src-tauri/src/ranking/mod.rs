use crate::launcher::result::SearchResult;

pub const CALCULATION_SCORE: u32 = 100_000;

/// Keep ranking and result limits after provider collection. Usage and recency
/// can adjust these scores here before truncation without changing providers.
pub fn top_results(mut results: Vec<SearchResult>, limit: usize) -> Vec<SearchResult> {
    // Stable sorting preserves the alphabetical provider order for ties.
    results.sort_by_key(|result| std::cmp::Reverse(result.score));
    results.truncate(limit);
    results
}

/// Normalize spaces and case once before applying name bonuses.
pub fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Providers supply fuzzy scores; ranking policy lives here.
/// Durable usage and recency belong to Phase 4.
pub fn name_score(fuzzy: u32, normalized_name: &str, normalized_query: &str) -> u32 {
    let bonus = if normalized_name == normalized_query {
        10_000
    } else if normalized_name.starts_with(normalized_query) {
        2_000
    } else {
        0
    };
    fuzzy.saturating_add(bonus)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_and_whitespace_without_removing_accents() {
        assert_eq!(normalize("  CAFÉ\t  Editor  "), "café editor");
    }

    #[test]
    fn exact_then_prefix_then_fuzzy() {
        assert!(name_score(100, "code", "code") > name_score(300, "code editor", "code"));
        assert!(name_score(100, "code editor", "code") > name_score(300, "xcode", "code"));
    }

    #[test]
    fn score_cannot_overflow() {
        assert_eq!(name_score(u32::MAX, "a", "a"), u32::MAX);
    }
}
