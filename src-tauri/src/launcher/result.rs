use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ResultKind {
    App,
    File,
    Calculation,
    Emoji,
    Clipboard,
    SystemCommand,
    Password,
    Timezone,
    CleanedUrl,
    WebSearch,
}

impl ResultKind {
    pub fn category(self) -> super::query::SearchMode {
        use super::query::SearchMode;
        match self {
            Self::App => SearchMode::Apps,
            Self::File => SearchMode::Files,
            Self::Calculation => SearchMode::Calculator,
            Self::Emoji => SearchMode::Emoji,
            Self::Clipboard => SearchMode::Clipboard,
            Self::SystemCommand => SearchMode::System,
            Self::Password => SearchMode::Password,
            Self::Timezone => SearchMode::Timezone,
            Self::CleanedUrl => SearchMode::Url,
            Self::WebSearch => SearchMode::Web,
        }
    }

    pub fn has_query_pin(self) -> bool {
        matches!(
            self,
            Self::Calculation
                | Self::Password
                | Self::Timezone
                | Self::CleanedUrl
                | Self::WebSearch
        )
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Launch,
    Open,
    Reveal,
    Copy,
    Delete,
    Run,
    Regenerate,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionConfirmation {
    pub title: String,
    pub description: String,
    pub confirm_label: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub kind: ResultKind,
    pub title: String,
    pub subtitle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub score: u32,
    pub icon: Option<String>,
    pub primary_action: Action,
    pub secondary_actions: Vec<Action>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin: Option<super::pins::ResultPin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<ActionConfirmation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<ToolDetail>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ToolDetail {
    Password {
        variant: String,
        entropy_bits: u32,
        strength: String,
    },
    Timezone {
        source: String,
        local: String,
        source_zone: String,
        ambiguous: bool,
    },
    DateCalculation {
        expression: String,
        based_on: String,
        result: String,
    },
    CleanedUrl {
        original: String,
        removed: usize,
    },
    WebSearch {
        engine: String,
        query: String,
        url: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: usize,
    pub indexing: bool,
    pub index_error: Option<String>,
    pub notice: Option<String>,
    pub storage_error: Option<String>,
    pub files: super::files::FileStatus,
    pub currency: super::currency::CurrencyStatus,
}
