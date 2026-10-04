//! Passwords, passphrases, and PINs from the OS random source.
//!
//! `password`, `password 24`, `passphrase 8`, `pin 4`. Copies are marked
//! secret, so clipboard managers (including TinyDash) skip them.

use std::sync::LazyLock;

use crate::{
    actions::Action,
    search::result::{Icon, ResultAction, ResultKind, SearchResult, Symbol},
};

const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*-_=+?";

static WORDS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    include_str!("../../data/eff_large_wordlist.txt")
        .lines()
        .filter_map(|line| line.split('\t').nth(1))
        .collect()
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Password,
    Passphrase,
    Pin,
}

impl Kind {
    /// Default, minimum, and maximum length (characters, words, or digits).
    fn lengths(self) -> (usize, usize, usize) {
        match self {
            Self::Password => (20, 8, 64),
            Self::Passphrase => (6, 3, 12),
            Self::Pin => (6, 4, 12),
        }
    }
}

pub fn answers(query: &str) -> Vec<SearchResult> {
    let mut words = query.split_whitespace();
    let kinds: &[Kind] = match words.next().map(str::to_lowercase).as_deref() {
        Some("password" | "pw") => &[Kind::Password, Kind::Passphrase, Kind::Pin],
        Some("passphrase") => &[Kind::Passphrase],
        Some("pin") => &[Kind::Pin],
        _ => return Vec::new(),
    };
    let length = match words.next() {
        Some(word) => match word.parse::<usize>() {
            Ok(length) => Some(length),
            Err(_) => return Vec::new(),
        },
        None => None,
    };
    if words.next().is_some() {
        return Vec::new();
    }
    // A length belongs to the first kind only: `password 24` is one password.
    let kinds = if length.is_some() { &kinds[..1] } else { kinds };
    kinds
        .iter()
        .filter_map(|&kind| {
            let (default, min, max) = kind.lengths();
            let length = length.unwrap_or(default);
            (min..=max).contains(&length).then(|| result(kind, length))
        })
        .collect()
}

fn result(kind: Kind, length: usize) -> SearchResult {
    let (value, bits, unit) = match kind {
        Kind::Password => {
            let value = password(length);
            let alphabet = [LOWER, UPPER, DIGITS, SYMBOLS].concat().len();
            (
                value,
                length as f64 * (alphabet as f64).log2(),
                "characters",
            )
        }
        Kind::Passphrase => {
            let words: Vec<_> = (0..length)
                .map(|_| WORDS[random_below(WORDS.len())])
                .collect();
            (
                words.join("-"),
                length as f64 * (WORDS.len() as f64).log2(),
                "words",
            )
        }
        Kind::Pin => {
            let value = (0..length).map(|_| pick(DIGITS)).collect();
            (value, length as f64 * 10f64.log2(), "digits")
        }
    };
    SearchResult {
        id: format!("password:{kind:?}:{length}"),
        kind: ResultKind::Password,
        title: value.clone(),
        subtitle: format!("{length} {unit} · {} bits", bits.floor()),
        icon: Icon::Symbol { name: Symbol::Key },
        actions: vec![ResultAction::new(
            "Copy Password",
            Action::CopySecret { text: value },
        )],
        pinned: false,
    }
}

/// A uniform password that contains every character class.
fn password(length: usize) -> String {
    let alphabet = [LOWER, UPPER, DIGITS, SYMBOLS].concat();
    loop {
        let value: String = (0..length).map(|_| pick(&alphabet)).collect();
        // Rejecting whole candidates keeps the result uniform over valid ones.
        if [LOWER, UPPER, DIGITS, SYMBOLS]
            .iter()
            .all(|class| value.chars().any(|c| class.contains(c)))
        {
            return value;
        }
    }
}

fn pick(alphabet: &str) -> char {
    let bytes = alphabet.as_bytes();
    char::from(bytes[random_below(bytes.len())])
}

/// A uniform integer in `0..bound`, without modulo bias.
fn random_below(bound: usize) -> usize {
    let bound = u32::try_from(bound).expect("small alphabet");
    let zone = u32::MAX - u32::MAX % bound;
    loop {
        let value = getrandom::u32().expect("the OS random source is available");
        if value < zone {
            return (value % bound) as usize;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copied(result: &SearchResult) -> &str {
        match &result.actions[0].action {
            Action::CopySecret { text } => text,
            other => panic!("unexpected action {other:?}"),
        }
    }

    #[test]
    fn generates_each_kind_within_bounds() {
        let all = answers("password");
        assert_eq!(all.len(), 3);
        let password = copied(&all[0]);
        assert_eq!(password.len(), 20);
        assert!(password.chars().any(|c| SYMBOLS.contains(c)));
        assert_eq!(copied(&all[1]).split('-').count(), 6);
        assert!(copied(&all[2]).chars().all(|c| c.is_ascii_digit()));

        assert_eq!(copied(&answers("pw 32")[0]).len(), 32);
        assert_eq!(copied(&answers("pin 4")[0]).len(), 4);
        assert_eq!(answers("passphrase 3").len(), 1);
    }

    #[test]
    fn rejects_other_queries_and_out_of_range_lengths() {
        for query in [
            "passwords",
            "password manager",
            "pin 2",
            "pw 999",
            "password 1 2",
        ] {
            assert!(answers(query).is_empty(), "{query}");
        }
    }

    #[test]
    fn the_wordlist_is_complete() {
        assert_eq!(WORDS.len(), 7776);
        assert_eq!(WORDS[0], "abacus");
    }
}
