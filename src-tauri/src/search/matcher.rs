use nucleo_matcher::{
    Config, Matcher as Nucleo, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};
use unicode_normalization::UnicodeNormalization;

/// Bonus for a name that equals the query, ignoring case.
pub const EXACT: u32 = 10_000;
/// Scores at or above this come only from an alias that equals the query.
pub const EXACT_ALIAS: u32 = 2 * EXACT;
/// Bonus for a name that starts with the query.
pub const PREFIX: u32 = 3_000;
/// Bonus for a name with a word that starts with the query.
pub const WORD_PREFIX: u32 = 500;
/// An alias ranks a little below the same match on the name.
const ALIAS_PENALTY: u32 = 100;
/// A localized keyword ranks below an equally good name match.
pub const KEYWORD_PENALTY: u32 = 300;
/// Scores at or above this come from a name, alias, or keyword that starts
/// with the query (2,700 or more). Any other match, with the most usage
/// (1,000), stays below: 499 + 500 + 1,000 = 1,999.
pub const STRONG: u32 = PREFIX - KEYWORD_PENALTY;
/// Fuzzy scores grow with the query's length; [`squeeze`] keeps them below
/// this, so they never reach the word-prefix bonus.
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
            .map(squeeze)
    }

    /// Score when every query word appears in `text` as written, for long
    /// text such as paths, where fuzzy matching finds too much.
    pub fn words(&mut self, text: &str) -> Option<u32> {
        self.words
            .score(Utf32Str::new(text, &mut self.buffer), &mut self.nucleo)
            .map(squeeze)
    }

    /// Score an alias the user chose. Only an alias that starts with the
    /// query matches, and one that equals it ranks above any name match.
    pub fn alias(&mut self, alias: &str) -> Option<u32> {
        let score = self.name(alias)?;
        if eq_ignore_case(alias, &self.query) {
            Some(score + EXACT)
        } else {
            starts_with_ignore_case(alias, &self.query).then_some(score)
        }
    }

    /// Best name score among a name and its aliases.
    pub fn best<'a>(
        &mut self,
        name: &str,
        aliases: impl IntoIterator<Item = &'a str>,
    ) -> Option<u32> {
        let mut best = self.name(name);
        for alias in aliases {
            let score = self
                .name(alias)
                .map(|score| score.saturating_sub(ALIAS_PENALTY));
            best = best.max(score);
        }
        best
    }
}

/// Map a fuzzy score below `MAX_FUZZY`. A higher score never maps lower,
/// so long matches still rank by quality, though close high scores may tie.
fn squeeze(score: u32) -> u32 {
    (u64::from(MAX_FUZZY) * u64::from(score) / (u64::from(score) + 500)) as u32
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
    fn a_short_alias_or_keyword_prefix_is_strong() {
        // Short queries score little on fuzzy, so a penalty must not push a
        // prefix match out of the strong tier, where usage could lift it back.
        let mut matcher = Matcher::new("r");
        assert!(matcher.best("Restart", []).unwrap() >= STRONG);
        assert!(matcher.best("Shut Down", ["reboot"]).unwrap() >= STRONG);
        let keyword = matcher.name("reboot").unwrap() - KEYWORD_PENALTY;
        assert!(keyword >= STRONG);
    }

    #[test]
    fn long_matches_keep_their_order() {
        let scores: Vec<u32> = [36, 62, 514, 537, 5_000].map(squeeze).into();
        assert!(
            scores.windows(2).all(|pair| pair[0] < pair[1]),
            "{scores:?}"
        );
        assert!(squeeze(u32::MAX) < WORD_PREFIX);
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
    fn an_exact_user_alias_beats_an_exact_name() {
        let mut matcher = Matcher::new("co");
        let alias = matcher.alias("co").unwrap();
        assert!(alias >= EXACT_ALIAS);
        assert!(matcher.name("Co").unwrap() + 1_000 < EXACT_ALIAS);
        let prefix = matcher.alias("code").unwrap();
        assert!(prefix >= STRONG && prefix + 1_000 < EXACT_ALIAS);
        assert_eq!(matcher.alias("deco"), None);
        assert_eq!(matcher.alias("c"), None);
    }

    #[test]
    fn words_must_appear_as_written() {
        let mut matcher = Matcher::new("project readme");
        assert!(matcher.words("/Users/me/Project/README.md").is_some());
        assert!(matcher.words("/p/r/o/j/e/c/t/readme").is_none());
    }
}
