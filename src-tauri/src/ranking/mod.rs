use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::launcher::result::{ResultKind, SearchResult};

pub const CALCULATION_SCORE: u32 = 100_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub count: u32,
    pub last_used_at: i64,
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

fn usage_bonus(usage: Usage, now: i64) -> u32 {
    if usage.count == 0 {
        return 0;
    }
    let frequency = usage.count.min(20) * 25;
    let days = now.saturating_sub(usage.last_used_at).max(0) as u64 / 86_400;
    let recency = 500 / (days + 1);
    frequency + recency as u32
}

pub fn apply_usage(results: &mut [SearchResult], usage: &HashMap<String, Usage>, now: i64) {
    for result in results {
        if result.kind != ResultKind::Calculation {
            result.score = score_with_usage(result.score, &result.id, usage, now);
        }
    }
}

pub fn score_with_usage(score: u32, id: &str, usage: &HashMap<String, Usage>, now: i64) -> u32 {
    score.saturating_add(usage.get(id).map_or(0, |stats| usage_bonus(*stats, now)))
}

const EXACT_BONUS: u32 = 10_000;
const PREFIX_BONUS: u32 = 2_000;
/// An alias match scores this much lower than the same name match.
pub const ALIAS_PENALTY: u32 = 100;
/// The lowest score of a name or alias that equals the query.
pub const EXACT_MATCH: u32 = EXACT_BONUS - ALIAS_PENALTY;
/// The lowest score of a name or alias that starts with the query. Fuzzy
/// points stay below it; heavy usage can lift a close fuzzy match to it.
pub const STRONG_MATCH: u32 = PREFIX_BONUS - ALIAS_PENALTY;

/// Tools and calculations come first, then names that equal or start with
/// the query, then fuzzy matches. Category order applies inside each tier.
pub fn tier(result: &SearchResult) -> u8 {
    match result.kind {
        ResultKind::Calculation
        | ResultKind::Password
        | ResultKind::Timezone
        | ResultKind::CleanedUrl
        | ResultKind::WebSearch => 0,
        _ if result.score >= STRONG_MATCH => 1,
        _ => 2,
    }
}

/// Rank by tier, then category, then score, before limiting the response.
pub fn top_results(mut results: Vec<SearchResult>, limit: usize) -> Vec<SearchResult> {
    // An exact emoji shortcode must not displace an app prefix such as "sa".
    // An exact filename must not wait behind fuzzy app matches.
    // Stable sorting keeps provider order for equal scores within a category.
    results.sort_by_key(|result| {
        let category = match result.kind {
            ResultKind::App => 1,
            ResultKind::File | ResultKind::Folder => 2,
            ResultKind::SystemCommand => 3,
            ResultKind::Clipboard => 4,
            ResultKind::Emoji => 5,
            _ => 0,
        };
        (tier(result), category, std::cmp::Reverse(result.score))
    });
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
pub fn name_score(fuzzy: u32, normalized_name: &str, normalized_query: &str) -> u32 {
    let bonus = if normalized_name == normalized_query {
        EXACT_BONUS
    } else if normalized_name.starts_with(normalized_query) {
        PREFIX_BONUS
    } else {
        0
    };
    fuzzy.saturating_add(bonus)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::result::Action;

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

    #[test]
    fn frequency_is_bounded_and_recency_decays_without_clock_overflow() {
        let now = 86_400 * 100;
        let once = Usage {
            count: 1,
            last_used_at: now,
        };
        let often = Usage { count: 20, ..once };
        assert!(usage_bonus(often, now) > usage_bonus(once, now));
        assert!(usage_bonus(once, now) > usage_bonus(once, now + 86_400));
        assert!(usage_bonus(once, now + 86_400) > usage_bonus(once, now + 86_400 * 30));
        assert_eq!(
            usage_bonus(
                Usage {
                    count: u32::MAX,
                    last_used_at: i64::MAX
                },
                0
            ),
            1000
        );
        assert_eq!(usage_bonus(Usage::default(), now), 0);
        assert!(usage_bonus(often, i64::MAX) <= 1000);
    }

    #[test]
    fn usage_ranks_before_truncation_without_overriding_strong_matches() {
        let result = |id: &str, score: u32| SearchResult {
            path: None,
            id: id.into(),
            kind: ResultKind::App,
            title: id.into(),
            subtitle: String::new(),
            score,
            icon: None,
            primary_action: Action::Launch,
            secondary_actions: vec![],
            pin: None,
            confirmation: None,
            detail: None,
        };
        let usage = HashMap::from([(
            "frequent".into(),
            Usage {
                count: u32::MAX,
                last_used_at: 100,
            },
        )]);
        let mut results = vec![result("unused", 0), result("frequent", 0)];
        apply_usage(&mut results, &usage, 100);
        assert_eq!(top_results(results, 1)[0].id, "frequent");
        let mut results = vec![
            result("exact", name_score(100, "code", "code")),
            result("prefix", name_score(100, "code editor", "code")),
            result("frequent", 500),
        ];
        apply_usage(&mut results, &usage, 100);
        let ids: Vec<_> = top_results(results, 3)
            .into_iter()
            .map(|result| result.id)
            .collect();
        assert_eq!(ids, ["exact", "prefix", "frequent"]);
    }
}
