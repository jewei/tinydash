use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SearchMode {
    #[default]
    All,
    Apps,
    Emoji,
    Calculator,
}

pub struct Query<'a> {
    pub text: &'a str,
    pub mode: SearchMode,
}

impl<'a> Query<'a> {
    pub fn parse(input: &'a str, mut mode: SearchMode) -> Result<Self> {
        if input.chars().count() > 256 {
            return Err(Error::QueryTooLong);
        }
        let mut text = input.trim();
        if mode == SearchMode::All {
            if text.starts_with(':') {
                mode = SearchMode::Emoji;
            } else if text.starts_with('=') {
                mode = SearchMode::Calculator;
            }
        }
        match mode {
            SearchMode::Emoji => text = text.trim_matches(':').trim(),
            SearchMode::Calculator => text = text.strip_prefix('=').unwrap_or(text).trim(),
            _ => {}
        }
        Ok(Self { text, mode })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_prefixes_without_changing_unit_case_or_app_punctuation() {
        let query = Query::parse(" = 32 C to F ", SearchMode::All).expect("query");
        assert_eq!(query.mode, SearchMode::Calculator);
        assert_eq!(query.text, "32 C to F");
        let query = Query::parse(" :rocket: ", SearchMode::All).expect("query");
        assert_eq!(query.mode, SearchMode::Emoji);
        assert_eq!(query.text, "rocket");
        assert_eq!(
            Query::parse(":App!", SearchMode::Apps).expect("query").text,
            ":App!"
        );
        assert!(Query::parse(&"a".repeat(257), SearchMode::All).is_err());
    }
}
