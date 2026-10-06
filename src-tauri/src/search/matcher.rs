use nucleo_matcher::{
    Config, Matcher as Nucleo, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};
use unicode_normalization::UnicodeNormalization;

/// Bonus for a name that equals the query, ignoring case.
pub const EXACT: u32 = 10_000;
/// Bonus for a name that starts with the query.
pub const PREFIX: u32 = 2_000;
/// Bonus for a name with a word that starts with the query.
pub const WORD_PREFIX: u32 = 500;
/// Scores at or above this come from a name that starts with the query.
pub const STRONG: u32 = PREFIX;
/// The highest fuzzy score. Fuzzy scores grow with the query's length, so
/// without a cap a long word-prefix match with usage (up to 1,000) would
/// reach `STRONG`; capped, it stops at 1,999.
const MAX_FUZZY: u32 = WORD_PREFIX - 1;

/// Fuzzy matching with name bonuses. One matcher serves one query; it keeps
/// its buffers between calls, so scoring many items does not allocate.
pub struct Matcher {
    nucleo: Nucleo,
    pattern: Pattern,
    words: Pattern,
    query: String,
    buffer: Vec<char>,
}

impl Matcher {
    pub fn new(query: &str) -> Self {
        let query = normalize(query);
        let pattern = |kind| Pattern::new(&query, CaseMatching::Ignore, Normalization::Smart, kind);
        Self {
            nucleo: Nucleo::new(Config::DEFAULT),
            pattern: pattern(AtomKind::Fuzzy),
            words: pattern(AtomKind::Substring),
            query,
            buffer: Vec::new(),
        }
    }

    /// The query in NFC lowercase, with single spaces.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Score a name, or `None` when it does not match.
    pub fn name(&mut self, name: &str) -> Option<u32> {
        let fuzzy = self.fuzzy(name)?;
        let bonus = if eq_ignore_case(name, &self.query) {
            EXACT
        } else if starts_with_ignore_case(name, &self.query) {
            PREFIX
        } else if name
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| starts_with_ignore_case(word, &self.query))
        {
            WORD_PREFIX
        } else {
            0
        };
        Some(fuzzy + bonus)
    }

    /// Fuzzy score without name bonuses.
    fn fuzzy(&mut self, text: &str) -> Option<u32> {
        self.pattern
            .score(Utf32Str::new(text, &mut self.buffer), &mut self.nucleo)
            .map(|score| score.min(MAX_FUZZY))
    }

    /// Score when every query word appears in `text` as written, for long
    /// text such as paths, where fuzzy matching finds too much.
    pub fn words(&mut self, text: &str) -> Option<u32> {
        self.words
            .score(Utf32Str::new(text, &mut self.buffer), &mut self.nucleo)
            .map(|score| score.min(MAX_FUZZY))
    }

    /// Best name score among a name and its aliases. Aliases rank a little
    /// below the same match on the name.
    pub fn best<'a>(
        &mut self,
        name: &str,
        aliases: impl IntoIterator<Item = &'a str>,
    ) -> Option<u32> {
        let mut best = self.name(name);
        for alias in aliases {
            let score = self.name(alias).map(|score| score.saturating_sub(100));
            best = best.max(score);
        }
        best
    }
}

/// NFC, lowercase, and single spaces: the form all matching compares.
pub fn normalize(text: &str) -> String {
    text.nfc()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn lowercase_chars(text: &str) -> impl Iterator<Item = char> + '_ {
    text.chars().flat_map(char::to_lowercase)
}

fn starts_with_ignore_case(text: &str, prefix: &str) -> bool {
    let mut text = lowercase_chars(text);
    prefix.chars().all(|expected| text.next() == Some(expected))
}

fn eq_ignore_case(text: &str, other: &str) -> bool {
    lowercase_chars(text).eq(other.chars())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_beats_prefix_beats_word_prefix_beats_fuzzy() {
        let mut matcher = Matcher::new("code");
        let exact = matcher.name("Code").unwrap();
        let prefix = matcher.name("Code Editor").unwrap();
        let word = matcher.name("Visual Studio Code").unwrap();
        let fuzzy = matcher.name("Color Developer").unwrap();
        assert!(exact > prefix && prefix > word && word > fuzzy);
        assert!(prefix >= STRONG && word < STRONG);
        assert_eq!(matcher.name("Safari"), None);

        // A long query scores high on fuzzy alone; with the most usage, a
        // word-prefix match must still stay below a prefix match.
        let mut matcher = Matcher::new("microsoft visual studio code insiders edition");
        let word = matcher
            .name("The Microsoft Visual Studio Code Insiders Edition")
            .unwrap();
        assert!(word + 1_000 < STRONG, "{word}");
        let path = "/Users/me/microsoft visual studio code insiders edition/notes.txt";
        assert!(matcher.words(path).unwrap() < WORD_PREFIX);
    }

    #[test]
    fn matching_ignores_case_accents_and_spacing() {
        let mut matcher = Matcher::new("  CAFE   menu ");
        assert_eq!(matcher.query(), "cafe menu");
        assert!(matcher.name("Café Menu").is_some());
        // An accented query matches only the accented name. Indexes store
        // names in NFC, so decomposed file names compare equal.
        assert_eq!(normalize("Cafe\u{301}"), "café");
        assert!(Matcher::new("Cafe\u{301}").name("café").is_some());
        assert!(Matcher::new("café").name("cafe").is_none());
    }

    #[test]
    fn aliases_rank_just_below_names() {
        let mut matcher = Matcher::new("vsc");
        let alias = matcher.best("Visual Studio Code", ["vsc"]).unwrap();
        let name = matcher.name("vsc").unwrap();
        assert_eq!(alias, name - 100);
    }

    #[test]
    fn words_must_appear_as_written() {
        let mut matcher = Matcher::new("project readme");
        assert!(matcher.words("/Users/me/Project/README.md").is_some());
        assert!(matcher.words("/p/r/o/j/e/c/t/readme").is_none());
    }
}
