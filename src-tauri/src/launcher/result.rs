use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ResultKind {
    App,
    Calculation,
    Emoji,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Launch,
    Reveal,
    Copy,
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
}
