mod password;
mod timezones;
pub mod url_cleaner;
pub(crate) mod web;

use std::collections::{HashMap, VecDeque};
use zeroize::Zeroize;

use crate::{
    error::Error,
    launcher::{
        actions::ResolvedAction,
        query::{Query, SearchMode},
        result::{Action, ResultKind, SearchResult, ToolDetail},
    },
};

struct Output {
    kind: ResultKind,
    title: String,
    subtitle: String,
    value: String,
    detail: ToolDetail,
    open: Option<String>,
    password: Option<password::Spec>,
    web_keyword: Option<String>,
}

struct Issued {
    result: SearchResult,
    value: String,
    open: Option<String>,
    password: Option<password::Spec>,
    web_keyword: Option<String>,
}
impl Drop for Issued {
    fn drop(&mut self) {
        if self.password.is_some() {
            self.value.zeroize();
            self.result.title.zeroize();
        }
    }
}

#[derive(Default)]
pub struct ToolProvider {
    custom_web_searches: Vec<crate::settings::WebSearch>,
    issued: VecDeque<Issued>,
    next_id: u64,
    password_specs: Vec<password::Spec>,
    password_ids: Vec<String>,
    pinned_password_ids: HashMap<password::Spec, String>,
}

impl ToolProvider {
    pub fn set_web_searches(&mut self, searches: &[crate::settings::WebSearch]) {
        if self.custom_web_searches != searches {
            self.custom_web_searches = searches.to_vec();
            self.issued
                .retain(|entry| entry.result.kind != ResultKind::WebSearch);
        }
    }
    fn issue(&mut self, output: Output) -> SearchResult {
        let Output {
            kind,
            title,
            subtitle,
            value,
            detail,
            open,
            password,
            web_keyword,
        } = output;
        self.next_id = self.next_id.wrapping_add(1);
        let prefix = if password.is_some() {
            "password"
        } else {
            "tool"
        };
        let primary_action = if kind == ResultKind::WebSearch {
            Action::Open
        } else {
            Action::Copy
        };
        let result = SearchResult {
            id: format!("{prefix}:{}", self.next_id),
            kind,
            title,
            subtitle,
            score: 100_000,
            path: None,
            icon: None,
            primary_action,
            secondary_actions: if password.is_some() {
                vec![Action::Regenerate]
            } else if primary_action == Action::Open {
                vec![Action::Copy]
            } else if open.is_some() {
                vec![Action::Open]
            } else {
                vec![]
            },
            pin: None,
            confirmation: None,
            detail: Some(detail),
        };
        self.issued.push_back(Issued {
            result: result.clone(),
            value,
            open,
            password,
            web_keyword,
        });
        while self.issued.len() > 64 {
            if let Some(expired) = self.issued.pop_front()
                && let Some(spec) = expired.password
                && self.pinned_password_ids.get(&spec) == Some(&expired.result.id)
            {
                self.pinned_password_ids.remove(&spec);
            }
        }
        result
    }

    fn password(&mut self, spec: password::Spec) -> Result<SearchResult, String> {
        let (value, subtitle, detail) = password::generate(spec)?;
        let result = self.issue(Output {
            kind: ResultKind::Password,
            title: value.clone(),
            subtitle,
            value,
            detail,
            open: None,
            password: Some(spec),
            web_keyword: None,
        });
        self.pinned_password_ids.insert(spec, result.id.clone());
        Ok(result)
    }

    pub fn regenerate(&mut self, id: &str) -> Result<(), String> {
        let spec = self
            .issued
            .iter()
            .find(|entry| entry.result.id == id)
            .and_then(|entry| entry.password)
            .ok_or_else(|| Error::ResultExpired.to_string())?;
        let result = self.password(spec)?;
        if let Some(index) = self.password_ids.iter().position(|old| old == id) {
            self.password_ids[index] = result.id;
        }
        Ok(())
    }

    pub fn resolve(&self, id: &str, action: Action) -> crate::error::Result<ResolvedAction> {
        let entry = self
            .issued
            .iter()
            .find(|entry| entry.result.id == id)
            .ok_or(Error::ResultExpired)?;
        match action {
            Action::Copy => Ok(ResolvedAction::Copy(entry.value.clone())),
            Action::Open => entry
                .open
                .clone()
                .map(ResolvedAction::OpenUrl)
                .ok_or(Error::InvalidAction),
            Action::Regenerate if entry.password.is_some() => {
                Ok(ResolvedAction::RegeneratePassword(id.into()))
            }
            _ => Err(Error::InvalidAction),
        }
    }

    pub fn password_pin_query(&self, id: &str) -> Option<String> {
        let spec = self
            .issued
            .iter()
            .find(|entry| entry.result.id == id)?
            .password?;
        let command = match spec.style {
            password::Style::Symbols => "password symbols",
            password::Style::Alphanumeric => "password letters",
            password::Style::Words => "passphrase",
            password::Style::Pin => "pin",
        };
        Some(format!("{command} {}", spec.length))
    }

    pub fn web_pin(&self, id: &str) -> Option<(String, String)> {
        let entry = self.issued.iter().find(|entry| entry.result.id == id)?;
        let ToolDetail::WebSearch { query, .. } = entry.result.detail.as_ref()? else {
            return None;
        };
        Some((entry.web_keyword.clone()?, query.clone()))
    }

    pub fn search_web_pin(&mut self, keyword: &str, text: &str) -> Option<SearchResult> {
        if !web::keyword_available(keyword, &self.custom_web_searches) {
            return None;
        }
        let input = format!("{keyword} {text}");
        let query = Query::parse(&input, SearchMode::Web).ok()?;
        self.search_pinned(&query, 0)
    }

    pub fn search(&mut self, query: &Query<'_>) -> Option<Result<Vec<SearchResult>, String>> {
        self.search_selected(query, None)
    }

    pub fn search_pinned(&mut self, query: &Query<'_>, index: usize) -> Option<SearchResult> {
        self.search_selected(query, Some(index))?
            .ok()?
            .into_iter()
            .next()
    }

    fn search_selected(
        &mut self,
        query: &Query<'_>,
        selected: Option<usize>,
    ) -> Option<Result<Vec<SearchResult>, String>> {
        let text = query.text;
        let mode = query.mode;
        if mode == SearchMode::Password || (mode == SearchMode::All && password::is_candidate(text))
        {
            return Some((|| {
                let specs = password::parse(text)?;
                if let Some(index) = selected {
                    let spec = *specs
                        .get(index)
                        .ok_or_else(|| Error::ResultExpired.to_string())?;
                    if let Some(id) = self.pinned_password_ids.get(&spec)
                        && let Some(entry) = self.issued.iter().find(|entry| &entry.result.id == id)
                    {
                        return Ok(vec![entry.result.clone()]);
                    }
                    return self.password(spec).map(|result| vec![result]);
                }
                if self.password_specs == specs {
                    let cached: Option<Vec<_>> = self
                        .password_ids
                        .iter()
                        .map(|id| {
                            self.issued
                                .iter()
                                .find(|entry| &entry.result.id == id)
                                .map(|entry| entry.result.clone())
                        })
                        .collect();
                    if let Some(cached) = cached
                        && cached.len() == specs.len()
                    {
                        return Ok(cached);
                    }
                }
                let results: Vec<_> = specs
                    .iter()
                    .map(|&spec| self.password(spec))
                    .collect::<Result<_, _>>()?;
                self.password_specs = specs;
                self.password_ids = results.iter().map(|result| result.id.clone()).collect();
                Ok(results)
            })());
        }
        // Keep a value stable during refreshes, but start fresh after leaving
        // password search. Previously issued IDs remain valid for queued actions.
        if selected.is_none() {
            self.password_specs.clear();
            self.password_ids.clear();
        }
        if mode == SearchMode::Url || (mode == SearchMode::All && url_cleaner::is_candidate(text)) {
            return Some(url_cleaner::clean(text).map(|cleaned| {
                let subtitle = format!(
                    "{} tracking {} removed",
                    cleaned.removed,
                    if cleaned.removed == 1 {
                        "field"
                    } else {
                        "fields"
                    }
                );
                vec![self.issue(Output {
                    kind: ResultKind::CleanedUrl,
                    title: cleaned.value.clone(),
                    subtitle,
                    value: cleaned.value.clone(),
                    detail: ToolDetail::CleanedUrl {
                        original: cleaned.original,
                        removed: cleaned.removed,
                    },
                    open: Some(cleaned.value),
                    password: None,
                    web_keyword: None,
                })]
            }));
        }
        if mode == SearchMode::Timezone
            || (mode == SearchMode::All && timezones::is_candidate(text))
        {
            return Some(
                timezones::calculate(text, chrono::Utc::now(), |time| {
                    time.with_timezone(&chrono::Local).fixed_offset()
                })
                .map(|results| {
                    select(results, selected)
                        .map(|result| {
                            self.issue(Output {
                                kind: ResultKind::Timezone,
                                title: result.title,
                                subtitle: result.subtitle,
                                value: result.copy,
                                detail: result.detail,
                                open: None,
                                password: None,
                                web_keyword: None,
                            })
                        })
                        .collect()
                }),
            );
        }
        if mode == SearchMode::Web
            || (mode == SearchMode::All
                && (web::is_candidate(text)
                    || web::custom_candidate(text, &self.custom_web_searches)))
        {
            return Some(
                web::searches_with_custom(text, &self.custom_web_searches).map(|results| {
                    select(results, selected)
                        .map(
                            |web::SearchTarget {
                                 engine,
                                 text: query,
                                 url,
                                 keyword: web_keyword,
                             }| {
                                self.issue(Output {
                                    kind: ResultKind::WebSearch,
                                    title: format!("Search {engine}"),
                                    subtitle: query.clone(),
                                    value: url.clone(),
                                    detail: ToolDetail::WebSearch {
                                        engine,
                                        query,
                                        url: url.clone(),
                                    },
                                    open: Some(url),
                                    password: None,
                                    web_keyword,
                                })
                            },
                        )
                        .collect()
                }),
            );
        }
        None
    }
}

fn select<T>(results: Vec<T>, selected: Option<usize>) -> impl Iterator<Item = T> {
    results
        .into_iter()
        .enumerate()
        .filter(move |(index, _)| selected.is_none_or(|selected| selected == *index))
        .map(|(_, result)| result)
}
