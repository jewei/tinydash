//! Snippet expansion as you type: when the keys typed in another app end
//! with `;` and a snippet's keyword, such as `;sig`, the snippet replaces
//! them.
//!
//! The platform layer reads each key press as [`Key`]s, and [`Typed`] keeps
//! the last [`MAX_TYPED`] characters, in memory only, to find a trigger.
//! Nothing here reads keys or types; `expander.rs` connects this to the OS.

use chrono::{DateTime, Local};

use super::library::{self, LibraryItem, LibraryKind};

/// Characters kept from what the user types: enough for `;`, the longest
/// keyword (32 characters), the character before them, and a space.
pub const MAX_TYPED: usize = 64;
/// Starts a trigger: `;sig` expands the snippet whose keyword is `sig`.
pub const TRIGGER: char = ';';
/// macOS reports arrows, Home, End, and function keys as these characters.
const FUNCTION_KEYS: std::ops::RangeInclusive<char> = '\u{F700}'..='\u{F8FF}';

/// One key press in another app, as the platform layer reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A character that the key typed.
    Char(char),
    /// Backspace, which deletes the last character.
    Backspace,
    /// Anything after which the text before the cursor is unknown: Return,
    /// Tab, Escape, an arrow, Home, End, Page Up or Down, a key pressed with
    /// Command, Control, Alt, or Windows, a dead key, a click, or another
    /// app or window in front.
    Reset,
}

/// The keys for what one key press typed, as UTF-16: each character, or one
/// `Reset` when it typed nothing (a dead key) or a character that is not
/// text, such as Return, Tab, Escape, or, on macOS, an arrow.
pub fn keys_for(typed: &[u16]) -> Vec<Key> {
    let chars: Result<Vec<char>, _> = char::decode_utf16(typed.iter().copied()).collect();
    match chars {
        Ok(chars) if !chars.is_empty() && chars.iter().all(|&c| is_text(c)) => {
            chars.into_iter().map(Key::Char).collect()
        }
        _ => vec![Key::Reset],
    }
}

fn is_text(c: char) -> bool {
    !c.is_control() && !FUNCTION_KEYS.contains(&c)
}

/// A snippet keyword that can expand.
#[derive(Debug)]
struct Keyword {
    keyword: String,
    /// `;` and the keyword.
    trigger: Vec<char>,
    /// Another keyword starts with this one, as `sig2` does with `sig`, so
    /// only a space after it expands it.
    waits_for_space: bool,
}

/// The keywords of the snippets that have one. Quicklinks never expand.
#[derive(Debug, Default)]
pub struct Keywords(Vec<Keyword>);

impl Keywords {
    pub fn new(items: &[LibraryItem]) -> Self {
        let mut words: Vec<&str> = items
            .iter()
            .filter(|item| expands(item))
            .map(|item| item.keyword.as_str())
            .collect();
        words.sort_unstable();
        words.dedup();
        let mut keywords: Vec<Keyword> = words
            .iter()
            .enumerate()
            .map(|(index, word)| Keyword {
                keyword: (*word).to_owned(),
                trigger: std::iter::once(TRIGGER).chain(word.chars()).collect(),
                // In sorted order, the words that start with this one follow it.
                waits_for_space: words
                    .get(index + 1)
                    .is_some_and(|next| next.starts_with(word)),
            })
            .collect();
        // When two triggers end the typed text, as `;;x` and `;x` do, the
        // longer one is what the user typed.
        keywords.sort_by_key(|keyword| std::cmp::Reverse(keyword.trigger.len()));
        Self(keywords)
    }

    /// The expansion that the last typed character completes. `typed` ends
    /// with that character; `start_known` says whether its first character
    /// is the first since a reset.
    fn completed_by(&self, typed: &[char], start_known: bool) -> Option<Expansion> {
        // A trigger starts the typed text or follows a character that is
        // not a letter or digit, so `a;sig` in code stays as it is.
        let ends_with = |typed: &[char], trigger: &[char]| {
            typed.ends_with(trigger)
                && match typed.len() - trigger.len() {
                    0 => start_known,
                    before => !typed[before - 1].is_alphanumeric(),
                }
        };
        if let Some((&' ', before)) = typed.split_last()
            && let Some(found) = self
                .0
                .iter()
                .find(|keyword| keyword.waits_for_space && ends_with(before, &keyword.trigger))
        {
            return Some(Expansion {
                keyword: found.keyword.clone(),
                erase: found.trigger.len() + 1,
                then_space: true,
            });
        }
        let found = self
            .0
            .iter()
            .find(|keyword| !keyword.waits_for_space && ends_with(typed, &keyword.trigger))?;
        Some(Expansion {
            keyword: found.keyword.clone(),
            erase: found.trigger.len(),
            then_space: false,
        })
    }
}

fn expands(item: &LibraryItem) -> bool {
    item.kind == LibraryKind::Snippet && !item.keyword.is_empty()
}

/// The snippet that a keyword expands.
fn snippet<'a>(items: &'a [LibraryItem], keyword: &str) -> Option<&'a LibraryItem> {
    items
        .iter()
        .find(|item| expands(item) && item.keyword == keyword)
}

/// A trigger the user typed, to replace with its snippet.
#[derive(Debug, PartialEq, Eq)]
pub struct Expansion {
    pub keyword: String,
    /// Backspaces that delete the trigger: one for each of its characters,
    /// and one for the space that confirmed it.
    pub erase: usize,
    /// A space confirmed the trigger; the snippet keeps it after its text.
    pub then_space: bool,
}

impl Expansion {
    /// What to type in place of the trigger: its snippet, filled as when it
    /// is copied from the launcher, with no text after the keyword, so
    /// `{query}` is empty. `None` when the snippet was deleted or lost its
    /// keyword since.
    pub fn text(
        &self,
        items: &[LibraryItem],
        now: DateTime<Local>,
        clipboard: impl FnOnce() -> Option<String>,
    ) -> Option<String> {
        let snippet = snippet(items, &self.keyword)?;
        let text = library::render_snippet(&snippet.text, "", now, || {
            clipboard().map(|copied| one_line(&copied))
        });
        Some(self.replacement(text))
    }

    fn replacement(&self, snippet: String) -> String {
        if self.then_space {
            snippet + " "
        } else {
            snippet
        }
    }
}

/// Copied text as it fills `{clipboard}` in a snippet that types: each run
/// of line breaks and tabs becomes one space, and those at either end go.
/// Text from another app then never presses Return or Tab, which could run
/// a command in a terminal or send a message. The snippet's own line breaks
/// still type as Return.
fn one_line(text: &str) -> String {
    text.split(['\r', '\n', '\t'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The characters typed since the last reset, at most [`MAX_TYPED`]. They
/// are overwritten when they are forgotten, and have no `Debug`, so no log
/// can show them.
pub struct Typed {
    chars: Vec<char>,
    /// Older characters were dropped, so the first one kept may follow a
    /// letter.
    cut: bool,
}

impl Default for Typed {
    fn default() -> Self {
        Self {
            // One allocation for good, so no copy of typed text is left behind.
            chars: Vec::with_capacity(MAX_TYPED),
            cut: false,
        }
    }
}

impl Typed {
    /// Take one key. A key that completes a trigger returns its expansion
    /// and clears the typed text, so the snippet's text, or a trigger in
    /// it, never starts another one.
    pub fn press(&mut self, key: Key, keywords: &Keywords) -> Option<Expansion> {
        match key {
            Key::Reset => self.clear(),
            Key::Backspace => {
                self.chars.pop();
            }
            Key::Char(c) => {
                if self.chars.len() == MAX_TYPED {
                    self.chars.remove(0);
                    self.cut = true;
                }
                self.chars.push(c);
                let expansion = keywords.completed_by(&self.chars, !self.cut)?;
                self.clear();
                return Some(expansion);
            }
        }
        None
    }

    /// Forget the typed text, and overwrite all of it, including characters
    /// that Backspace removed.
    pub fn clear(&mut self) {
        let capacity = self.chars.capacity();
        self.chars.clear();
        self.chars.resize(capacity, '\0');
        std::hint::black_box(&mut self.chars);
        self.chars.clear();
        self.cut = false;
    }
}

impl Drop for Typed {
    fn drop(&mut self) {
        self.clear();
    }
}

/// A part of the text to type.
#[derive(Debug, PartialEq, Eq)]
pub enum Piece {
    /// Text as UTF-16, never with a surrogate pair split between pieces.
    Text(Vec<u16>),
    /// The Return key, for a line break.
    Return,
}

/// Split text to type into pieces of at most `max_units` UTF-16 units (at
/// least 2). A line break (`\n`, `\r\n`, or `\r`) becomes Return, as when
/// the user types it. Other control characters, except Tab, are left out:
/// no key types them.
pub fn pieces(text: &str, max_units: usize) -> Vec<Piece> {
    let max_units = max_units.max(2);
    let mut pieces = Vec::new();
    let mut units: Vec<u16> = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' || c == '\n' {
            if c == '\r' {
                chars.next_if_eq(&'\n');
            }
            if !units.is_empty() {
                pieces.push(Piece::Text(std::mem::take(&mut units)));
            }
            pieces.push(Piece::Return);
            continue;
        }
        if c.is_control() && c != '\t' {
            continue;
        }
        let mut buffer = [0; 2];
        let encoded = c.encode_utf16(&mut buffer);
        if units.len() + encoded.len() > max_units {
            pieces.push(Piece::Text(std::mem::take(&mut units)));
        }
        units.extend_from_slice(encoded);
    }
    if !units.is_empty() {
        pieces.push(Piece::Text(units));
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: LibraryKind, keyword: &str) -> LibraryItem {
        LibraryItem {
            id: Some(1),
            kind,
            name: keyword.into(),
            keyword: keyword.into(),
            text: "text".into(),
        }
    }

    fn keywords(words: &[&str]) -> Keywords {
        let items: Vec<LibraryItem> = words
            .iter()
            .map(|word| item(LibraryKind::Snippet, word))
            .collect();
        Keywords::new(&items)
    }

    /// Type `text`, where `⌫` is Backspace and `↺` a reset, and return
    /// every expansion it made.
    fn type_text(typed: &mut Typed, keywords: &Keywords, text: &str) -> Vec<Expansion> {
        text.chars()
            .filter_map(|c| {
                let key = match c {
                    '⌫' => Key::Backspace,
                    '↺' => Key::Reset,
                    c => Key::Char(c),
                };
                typed.press(key, keywords)
            })
            .collect()
    }

    fn expansions(words: &[&str], text: &str) -> Vec<Expansion> {
        type_text(&mut Typed::default(), &keywords(words), text)
    }

    fn at_once(keyword: &str, erase: usize) -> Expansion {
        Expansion {
            keyword: keyword.into(),
            erase,
            then_space: false,
        }
    }

    #[test]
    fn expands_at_once_when_no_other_keyword_starts_with_it() {
        assert_eq!(expansions(&["sig"], ";sig"), [at_once("sig", 4)]);
        assert_eq!(expansions(&["sig"], "Best, ;sig"), [at_once("sig", 4)]);
        assert!(expansions(&["sig"], ";si").is_empty());
        assert!(expansions(&["sig"], "sig").is_empty());
    }

    #[test]
    fn waits_for_a_space_when_a_longer_keyword_starts_with_it() {
        let words = ["sig", "sig2"];
        assert!(expansions(&words, ";sig").is_empty());
        assert_eq!(
            expansions(&words, ";sig "),
            [Expansion {
                keyword: "sig".into(),
                erase: 5,
                then_space: true
            }]
        );
        assert_eq!(expansions(&words, ";sig2"), [at_once("sig2", 5)]);
        assert!(expansions(&words, ";sigx ").is_empty());
        // Only a space confirms it.
        assert!(expansions(&words, ";sig.").is_empty());
        let replaced = expansions(&words, ";sig ").remove(0);
        assert_eq!(replaced.replacement("Regards".into()), "Regards ");
        assert_eq!(at_once("sig2", 5).replacement("Bye".into()), "Bye");
    }

    #[test]
    fn needs_a_character_that_is_not_a_letter_or_digit_before_the_trigger() {
        for text in ["a;sig", "1;sig", "é;sig", "日;sig"] {
            assert!(expansions(&["sig"], text).is_empty(), "{text}");
        }
        for text in [" ;sig", "(;sig", "-;sig", ";;sig", "😀;sig", "\t;sig"] {
            assert_eq!(expansions(&["sig"], text), [at_once("sig", 4)], "{text}");
        }
    }

    #[test]
    fn backspace_removes_one_character() {
        assert_eq!(expansions(&["sig"], ";sgi⌫⌫ig"), [at_once("sig", 4)]);
        assert_eq!(expansions(&["sig"], ";six⌫g"), [at_once("sig", 4)]);
        assert_eq!(expansions(&["sig"], ";sig⌫").len(), 1);
        // Removing the letter before the trigger makes it count.
        assert_eq!(expansions(&["sig"], "a⌫;sig"), [at_once("sig", 4)]);
        // Backspace with nothing typed changes nothing.
        assert_eq!(expansions(&["sig"], "⌫⌫;sig"), [at_once("sig", 4)]);
    }

    #[test]
    fn a_reset_forgets_the_typed_text() {
        assert!(expansions(&["sig"], ";si↺g").is_empty());
        assert!(expansions(&["sig"], ";↺sig").is_empty());
        // What came before the reset is unknown; the trigger starts afresh.
        assert_eq!(expansions(&["sig"], "word↺;sig"), [at_once("sig", 4)]);
        assert!(expansions(&["sig", "sig2"], ";sig↺ ").is_empty());
    }

    #[test]
    fn keywords_match_case_sensitively() {
        assert!(expansions(&["sig"], ";SIG").is_empty());
        assert!(expansions(&["sig"], ";Sig").is_empty());
        assert_eq!(expansions(&["Sig"], ";Sig"), [at_once("Sig", 4)]);
    }

    #[test]
    fn only_snippets_with_a_keyword_expand() {
        let items = [
            item(LibraryKind::Quicklink, "jira"),
            item(LibraryKind::Snippet, ""),
            item(LibraryKind::Snippet, "sig"),
            // A quicklink keyword never makes a snippet wait for a space.
            item(LibraryKind::Quicklink, "sig2"),
        ];
        let keywords = Keywords::new(&items);
        let mut typed = Typed::default();
        assert!(type_text(&mut typed, &keywords, ";jira ;").is_empty());
        assert_eq!(
            type_text(&mut typed, &keywords, " ;sig"),
            [at_once("sig", 4)]
        );
        assert_eq!(snippet(&items, "sig").map(|item| item.id), Some(Some(1)));
        assert!(snippet(&items, "jira").is_none());
        assert!(snippet(&items, "").is_none());
    }

    #[test]
    fn non_ascii_and_emoji_keywords_erase_one_backspace_per_character() {
        assert_eq!(expansions(&["café"], ";café"), [at_once("café", 5)]);
        assert_eq!(expansions(&["日本"], ";日本"), [at_once("日本", 3)]);
        assert_eq!(expansions(&["🙂"], ";🙂"), [at_once("🙂", 2)]);
        assert_eq!(expansions(&["🙂", "🙂🙂"], ";🙂🙂"), [at_once("🙂🙂", 3)]);
        assert_eq!(
            keys_for(&"🙂".encode_utf16().collect::<Vec<_>>()),
            [Key::Char('🙂')]
        );
    }

    #[test]
    fn the_longest_trigger_wins() {
        // `;;x` also ends with `;x`.
        assert_eq!(expansions(&["x", ";x"], ";;x"), [at_once(";x", 3)]);
        assert_eq!(expansions(&["x", ";x"], " ;x"), [at_once("x", 2)]);
    }

    #[test]
    fn the_typed_text_clears_after_an_expansion() {
        let keywords = keywords(&["sig"]);
        let mut typed = Typed::default();
        assert!(type_text(&mut typed, &keywords, "a;sig").is_empty());
        assert_eq!(
            type_text(&mut typed, &keywords, "↺;sig"),
            [at_once("sig", 4)]
        );
        // Nothing of the first trigger is left to complete a second one.
        assert!(type_text(&mut typed, &keywords, "sig").is_empty());
        assert_eq!(
            type_text(&mut typed, &keywords, " ;sig"),
            [at_once("sig", 4)]
        );
        // Right after an expansion, the next trigger starts the typed text.
        assert_eq!(
            type_text(&mut typed, &keywords, ";sig"),
            [at_once("sig", 4)]
        );
    }

    #[test]
    fn fills_the_snippet_with_the_clipboard_on_one_line() {
        let mut items = vec![item(LibraryKind::Snippet, "cb")];
        items[0].text = "Run:\n{clipboard}{query}".into();
        let now = chrono::Local::now();
        let copied = || Some("\ngit status\r\n\r\n\tcurl example.com/run.sh | sh\r".to_owned());
        let text = at_once("cb", 3).text(&items, now, copied).unwrap();
        assert_eq!(text, "Run:\ngit status curl example.com/run.sh | sh");
        // Only the snippet's own line break presses Return.
        let returns = pieces(&text, 20)
            .into_iter()
            .filter(|piece| *piece == Piece::Return)
            .count();
        assert_eq!(returns, 1);
        // A secret copy reads as none, and fills nothing.
        assert_eq!(
            at_once("cb", 3).text(&items, now, || None).as_deref(),
            Some("Run:\n")
        );
        let confirmed = Expansion {
            keyword: "cb".into(),
            erase: 4,
            then_space: true,
        };
        assert_eq!(
            confirmed.text(&items, now, || None).as_deref(),
            Some("Run:\n ")
        );
        // The snippet was deleted since.
        assert_eq!(at_once("gone", 5).text(&items, now, copied), None);
    }

    #[test]
    fn keeps_at_most_64_characters() {
        let keywords = keywords(&["sig"]);
        let mut typed = Typed::default();
        let long = "x".repeat(200);
        assert!(type_text(&mut typed, &keywords, &long).is_empty());
        assert_eq!(typed.chars.len(), MAX_TYPED);
        assert_eq!(typed.chars.capacity(), MAX_TYPED);
        // A trigger after many characters still works.
        assert_eq!(
            type_text(&mut typed, &keywords, " ;sig"),
            [at_once("sig", 4)]
        );

        // A trigger that fills the whole buffer counts only if nothing was
        // dropped before it: the dropped character may be a letter.
        let longest = "k".repeat(MAX_TYPED - 1);
        assert_eq!(
            expansions(&[longest.as_str()], &format!(";{longest}")),
            [at_once(&longest, MAX_TYPED)]
        );
        assert!(expansions(&[longest.as_str()], &format!("x;{longest}")).is_empty());
    }

    #[test]
    fn clearing_empties_and_keeps_the_one_allocation() {
        let mut typed = Typed::default();
        type_text(&mut typed, &Keywords::default(), "secret⌫⌫");
        typed.clear();
        assert!(typed.chars.is_empty());
        assert_eq!(typed.chars.capacity(), MAX_TYPED);
    }

    #[test]
    fn reads_what_a_key_typed() {
        let utf16 = |text: &str| text.encode_utf16().collect::<Vec<u16>>();
        assert_eq!(keys_for(&utf16("a")), [Key::Char('a')]);
        assert_eq!(keys_for(&utf16(" ")), [Key::Char(' ')]);
        assert_eq!(keys_for(&utf16("é")), [Key::Char('é')]);
        assert_eq!(keys_for(&utf16("ab")), [Key::Char('a'), Key::Char('b')]);
        // A dead key types nothing yet.
        assert_eq!(keys_for(&[]), [Key::Reset]);
        // Return, Tab, Escape, and macOS arrows and Forward Delete.
        for typed in ["\r", "\t", "\u{1b}", "\u{3}", "\u{F702}", "\u{F728}"] {
            assert_eq!(keys_for(&utf16(typed)), [Key::Reset], "{typed:?}");
        }
        // A lone half of a surrogate pair is not text.
        assert_eq!(keys_for(&[0xD83D]), [Key::Reset]);
    }

    #[test]
    fn splits_text_into_pieces_to_type() {
        let text = |text: &str| Piece::Text(text.encode_utf16().collect());
        assert_eq!(
            pieces("Hi\nthere\r\n\r", 20),
            [
                text("Hi"),
                Piece::Return,
                text("there"),
                Piece::Return,
                Piece::Return
            ]
        );
        assert_eq!(
            pieces(&"x".repeat(45), 20),
            [
                text(&"x".repeat(20)),
                text(&"x".repeat(20)),
                text(&"x".repeat(5))
            ]
        );
        // An emoji is two units and is never split between pieces.
        let nineteen = "x".repeat(19);
        assert_eq!(
            pieces(&format!("{nineteen}😀y"), 20),
            [text(&nineteen), text("😀y")]
        );
        // Tab stays; other control characters have no key.
        assert_eq!(pieces("a\tb\u{0}\u{7}c", 20), [text("a\tbc")]);
        assert_eq!(pieces("", 20), []);
        assert_eq!(pieces("😀", 1), [text("😀")]);
    }
}
