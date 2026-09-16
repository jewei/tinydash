use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ResultKind {
    App,
    File,
    Calculation,
    Emoji,
    Clipboard,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Launch,
    Open,
    Reveal,
    Copy,
    Delete,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub kind: ResultKind,
    pub title: String,
    pub subtitle: String,
    pub score: u32,
    pub icon: Option<String>,
    pub primary_action: Action,
    pub secondary_actions: Vec<Action>,
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
}
