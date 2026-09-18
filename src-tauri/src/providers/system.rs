use nucleo_matcher::{
    Matcher, Utf32String,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::{
    error::{Error, Result},
    launcher::result::{Action, ActionConfirmation, ResultKind, SearchResult},
    ranking,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemCommand {
    Lock,
    Sleep,
    Restart,
    Shutdown,
    Settings,
}

impl SystemCommand {
    pub const ALL: [Self; 5] = [
        Self::Lock,
        Self::Sleep,
        Self::Restart,
        Self::Shutdown,
        Self::Settings,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Lock => "system:lock",
            Self::Sleep => "system:sleep",
            Self::Restart => "system:restart",
            Self::Shutdown => "system:shutdown",
            Self::Settings => "system:settings",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Lock => "Lock screen",
            Self::Sleep => "Sleep",
            Self::Restart => "Restart",
            Self::Shutdown => "Shut down",
            Self::Settings => "Open system settings",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Lock => "Lock your current session",
            Self::Sleep => "Put this computer to sleep · Confirmation required",
            Self::Restart => "Restart this computer · Confirmation required",
            Self::Shutdown => "Turn off this computer · Confirmation required",
            Self::Settings => "Open your operating system settings",
        }
    }

    fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Lock => &["lock computer", "lock session"],
            Self::Sleep => &["suspend", "standby"],
            Self::Restart => &["reboot"],
            Self::Shutdown => &["shutdown", "power off", "turn off"],
            Self::Settings => &["preferences", "control panel"],
        }
    }

    pub fn confirmation(self) -> Option<ActionConfirmation> {
        let (title, description, confirm_label) = match self {
            Self::Sleep => (
                "Put this computer to sleep?",
                "This pauses active work until you wake the computer.",
                "Sleep",
            ),
            Self::Restart => (
                "Restart this computer?",
                "Save your work before you continue. This closes applications and restarts the computer.",
                "Restart",
            ),
            Self::Shutdown => (
                "Shut down this computer?",
                "Save your work before you continue. This closes applications and turns off the computer.",
                "Shut down",
            ),
            Self::Lock | Self::Settings => return None,
        };
        Some(ActionConfirmation {
            title: title.into(),
            description: description.into(),
            confirm_label: confirm_label.into(),
        })
    }

    pub fn check_confirmation(self, confirmed: bool) -> Result<()> {
        if !confirmed && matches!(self, Self::Sleep | Self::Restart | Self::Shutdown) {
            return Err(Error::ConfirmationRequired);
        }
        Ok(())
    }
}

struct IndexedCommand {
    command: SystemCommand,
    terms: Vec<(Utf32String, String)>,
}

pub struct SystemCommandProvider {
    commands: Vec<IndexedCommand>,
}

impl Default for SystemCommandProvider {
    fn default() -> Self {
        Self::new(crate::platform::system_commands())
    }
}

impl SystemCommandProvider {
    pub fn new(commands: Vec<SystemCommand>) -> Self {
        Self {
            commands: commands
                .into_iter()
                .map(|command| IndexedCommand {
                    command,
                    terms: std::iter::once(command.title())
                        .chain(command.aliases().iter().copied())
                        .map(|term| (term.into(), ranking::normalize(term)))
                        .collect(),
                })
                .collect(),
        }
    }

    pub fn get(&self, id: &str) -> Option<SystemCommand> {
        self.commands
            .iter()
            .find(|entry| entry.command.id() == id)
            .map(|entry| entry.command)
    }

    pub fn search(&self, query: &str, matcher: &mut Matcher) -> Vec<SearchResult> {
        let query = ranking::normalize(query);
        let pattern = Pattern::new(
            &query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        self.commands
            .iter()
            .filter_map(|entry| {
                let score = if query.is_empty() {
                    0
                } else {
                    entry
                        .terms
                        .iter()
                        .enumerate()
                        .filter_map(|(index, (term, normalized))| {
                            pattern.score(term.slice(..), matcher).map(|score| {
                                ranking::name_score(score, normalized, &query)
                                    .saturating_sub(if index == 0 { 0 } else { 100 })
                            })
                        })
                        .max()?
                };
                let command = entry.command;
                Some(SearchResult {
                    id: command.id().into(),
                    kind: ResultKind::SystemCommand,
                    path: None,
                    title: command.title().into(),
                    subtitle: command.subtitle().into(),
                    score,
                    icon: None,
                    primary_action: Action::Run,
                    secondary_actions: vec![],
                    pin: None,
                    confirmation: command.confirmation(),
                    detail: None,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_names_aliases_and_abbreviations_but_not_unknown_commands() {
        let provider = SystemCommandProvider::new(SystemCommand::ALL.to_vec());
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        for (query, expected) in [
            ("ShUtDoWn", "system:shutdown"),
            ("reboot", "system:restart"),
            ("lck scr", "system:lock"),
            ("preferences", "system:settings"),
            ("suspend", "system:sleep"),
        ] {
            let results = provider.search(query, &mut matcher);
            assert!(
                results.iter().any(|result| result.id == expected),
                "{query}"
            );
        }
        assert_eq!(provider.search("", &mut matcher).len(), 5);
        assert!(provider.search("delete all files", &mut matcher).is_empty());
        assert!(provider.get("system:arbitrary command").is_none());
        assert!(
            SystemCommandProvider::new(vec![SystemCommand::Settings])
                .get("system:shutdown")
                .is_none()
        );
    }

    #[test]
    fn power_commands_require_explicit_confirmation_and_expose_dialog_text() {
        for command in SystemCommand::ALL {
            let required = matches!(
                command,
                SystemCommand::Sleep | SystemCommand::Restart | SystemCommand::Shutdown
            );
            assert_eq!(command.confirmation().is_some(), required);
            assert_eq!(command.check_confirmation(false).is_err(), required);
            assert!(command.check_confirmation(true).is_ok());
        }
    }
}
