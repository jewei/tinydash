//! Result IDs of results that can be pinned and ranked: `<prefix>:<key>`,
//! such as `app:/Applications/Safari.app` or `clip:42`. Pins and usage store
//! these strings, so a shipped prefix must never change.
//!
//! Instant answers (calculations, times, passwords, URLs, web searches) have
//! throwaway IDs and no `Source`.

use std::fmt::Display;

use super::Category;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    App,
    File,
    Clip,
    Snippet,
    Link,
    Emoji,
    System,
}

impl Source {
    /// Every source. `parse` searches this list, so a new variant must be added here.
    const ALL: [Self; 7] = [
        Self::App,
        Self::File,
        Self::Clip,
        Self::Snippet,
        Self::Link,
        Self::Emoji,
        Self::System,
    ];

    fn prefix(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::File => "file",
            Self::Clip => "clip",
            Self::Snippet => "snippet",
            Self::Link => "link",
            Self::Emoji => "emoji",
            Self::System => "system",
        }
    }

    pub fn id(self, key: impl Display) -> String {
        format!("{}:{key}", self.prefix())
    }

    /// Split an ID into its source and key.
    pub fn parse(id: &str) -> Option<(Self, &str)> {
        let (prefix, key) = id.split_once(':')?;
        let source = Self::ALL.into_iter().find(|s| s.prefix() == prefix)?;
        Some((source, key))
    }

    pub fn category(self) -> Category {
        match self {
            Self::App => Category::Apps,
            Self::File => Category::Files,
            Self::Clip => Category::Clipboard,
            Self::Snippet | Self::Link => Category::Snippets,
            Self::Emoji => Category::Emoji,
            Self::System => Category::System,
        }
    }

    /// Running the result means the user wants it again soon. Clipboard
    /// entries already sort by recency.
    pub fn learns_from_use(self) -> bool {
        match self {
            Self::App | Self::File | Self::Snippet | Self::Link | Self::Emoji | Self::System => {
                true
            }
            Self::Clip => false,
        }
    }

    /// The user may hide it from results for good. A clipboard entry is
    /// deleted instead.
    pub fn hideable(self) -> bool {
        match self {
            Self::App | Self::File | Self::Snippet | Self::Link | Self::Emoji | Self::System => {
                true
            }
            Self::Clip => false,
        }
    }

    /// The user may give it an alias. A clipboard entry has no lasting name.
    pub fn aliasable(self) -> bool {
        match self {
            Self::App | Self::File | Self::Snippet | Self::Link | Self::Emoji | Self::System => {
                true
            }
            Self::Clip => false,
        }
    }

    /// May appear among the suggestions in an empty All. Clipboard text and
    /// system commands should never be one keystroke away by accident.
    pub fn suggestible(self) -> bool {
        match self {
            Self::App | Self::File | Self::Snippet | Self::Link | Self::Emoji => true,
            Self::Clip | Self::System => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for source in Source::ALL {
            let id = source.id("a:b");
            assert_eq!(Source::parse(&id), Some((source, "a:b")));
        }
        assert_eq!(Source::parse("calc:1+1"), None);
        assert_eq!(Source::parse("no prefix"), None);
    }
}
