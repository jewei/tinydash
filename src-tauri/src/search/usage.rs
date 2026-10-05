use std::collections::HashMap;

/// How often and how recently the user ran one result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Use {
    pub count: u32,
    pub last_used: i64,
}

/// Usage per result ID, mirrored from the database for ranking.
#[derive(Clone, Debug, Default)]
pub struct Usage(HashMap<String, Use>);

const DAY: i64 = 86_400;
/// Most result IDs whose usage is remembered, in memory and in the database.
pub const MAX_USAGE: usize = 1000;

impl Usage {
    pub fn new(entries: impl IntoIterator<Item = (String, Use)>) -> Self {
        Self(entries.into_iter().collect())
    }

    /// Count one use. Past [`MAX_USAGE`] IDs, the least recently used goes;
    /// of equally old ones, the largest ID, as in `Store::record_use`.
    pub fn record(&mut self, id: &str, now: i64) -> Use {
        let entry = self.0.entry(id.to_owned()).or_default();
        entry.count = entry.count.saturating_add(1);
        entry.last_used = now;
        let used = *entry;
        if self.0.len() > MAX_USAGE
            && let Some(oldest) = self
                .0
                .iter()
                .min_by_key(|(id, entry)| (entry.last_used, std::cmp::Reverse(*id)))
                .map(|(id, _)| id.clone())
        {
            self.0.remove(&oldest);
        }
        used
    }

    pub fn remove(&mut self, id: &str) {
        self.0.remove(id);
    }

    /// Ranking bonus, at most 1,000: up to 500 for frequency and up to 500
    /// for recency. A strong name match (2,000+) always outranks usage alone.
    pub fn bonus(&self, id: &str, now: i64) -> u32 {
        let Some(entry) = self.0.get(id) else {
            return 0;
        };
        let frequency = entry.count.min(20) * 25;
        let days = (now - entry.last_used).max(0) / DAY;
        let recency = 500 / (days + 1);
        frequency + recency as u32
    }

    /// IDs from most to least used, with ties broken by recency, then ID.
    pub fn ranked(&self, now: i64) -> Vec<&str> {
        let mut ids: Vec<_> = self.0.iter().collect();
        ids.sort_by(|(a_id, a), (b_id, b)| {
            let key = |id: &str, entry: &Use| (self.bonus(id, now), entry.last_used);
            key(b_id, b).cmp(&key(a_id, a)).then_with(|| a_id.cmp(b_id))
        });
        ids.into_iter().map(|(id, _)| id.as_str()).collect()
    }
}

/// Pinned result IDs, oldest pin first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pins(Vec<String>);

impl Pins {
    pub fn new(ids: Vec<String>) -> Self {
        Self(ids)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.0.iter().any(|pin| pin == id)
    }

    pub fn ids(&self) -> &[String] {
        &self.0
    }

    pub fn add(&mut self, id: &str) {
        if !self.contains(id) {
            self.0.push(id.to_owned());
        }
    }

    pub fn remove(&mut self, id: &str) {
        self.0.retain(|pin| pin != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequency_and_recency_are_bounded() {
        let now = DAY * 100;
        let mut usage = Usage::default();
        for _ in 0..100 {
            usage.record("often", now);
        }
        usage.record("once", now);
        assert_eq!(usage.bonus("often", now), 1_000);
        assert_eq!(usage.bonus("once", now), 525);
        assert_eq!(usage.bonus("once", now + DAY), 275);
        assert_eq!(usage.bonus("never", now), 0);
    }

    #[test]
    fn ranking_breaks_ties_by_recency_then_id() {
        let usage = Usage::new([
            (
                "b".into(),
                Use {
                    count: 1,
                    last_used: 10,
                },
            ),
            (
                "a".into(),
                Use {
                    count: 1,
                    last_used: 10,
                },
            ),
            (
                "c".into(),
                Use {
                    count: 1,
                    last_used: 20,
                },
            ),
            (
                "d".into(),
                Use {
                    count: 5,
                    last_used: 0,
                },
            ),
        ]);
        assert_eq!(usage.ranked(20), ["d", "c", "a", "b"]);
    }

    #[test]
    fn pins_keep_order_and_ignore_duplicates() {
        let mut pins = Pins::default();
        pins.add("a");
        pins.add("b");
        pins.add("a");
        pins.remove("c");
        assert_eq!(pins.ids(), ["a", "b"]);
        pins.remove("a");
        assert!(!pins.contains("a"));
    }
}
