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
    ToggleAppearance,
    EmptyTrash,
    Logout,
    ShowDesktop,
    ToggleMute,
}

impl SystemCommand {
    pub const ALL: [Self; 10] = [
        Self::Lock,
        Self::Sleep,
        Self::Restart,
        Self::Shutdown,
        Self::Settings,
        Self::ToggleAppearance,
        Self::EmptyTrash,
        Self::Logout,
        Self::ShowDesktop,
        Self::ToggleMute,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Lock => "system:lock",
            Self::Sleep => "system:sleep",
            Self::Restart => "system:restart",
            Self::Shutdown => "system:shutdown",
            Self::Settings => "system:settings",
            Self::ToggleAppearance => "system:appearance",
            Self::EmptyTrash => "system:empty-trash",
            Self::Logout => "system:logout",
            Self::ShowDesktop => "system:desktop",
            Self::ToggleMute => "system:mute",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Lock => "Lock screen",
            Self::Sleep => "Sleep",
            Self::Restart => "Restart",
            Self::Shutdown => "Shut down",
            Self::Settings => "Open system settings",
            Self::ToggleAppearance => "Toggle system appearance",
            Self::EmptyTrash if cfg!(target_os = "windows") => "Empty Recycle Bin",
            Self::EmptyTrash => "Empty Trash",
            Self::Logout => "Log out",
            Self::ShowDesktop => "Show desktop",
            Self::ToggleMute => "Toggle mute",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Lock => "Lock your current session",
            Self::Sleep => "Put this computer to sleep · Confirmation required",
            Self::Restart => "Restart this computer · Confirmation required",
            Self::Shutdown => "Turn off this computer · Confirmation required",
            Self::Settings => "Open your operating system settings",
            Self::ToggleAppearance => "Switch the system between light and dark appearance",
            Self::EmptyTrash => "Permanently delete trashed items · Confirmation required",
            Self::Logout => "Log out of your account · Confirmation required",
            Self::ShowDesktop => "Move windows aside to show the desktop",
            Self::ToggleMute => "Mute or unmute system sound output",
        }
    }

    fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Lock => &["lock computer", "lock session"],
            Self::Sleep => &["suspend", "standby"],
            Self::Restart => &["reboot"],
            Self::Shutdown => &["shutdown", "power off", "turn off"],
            Self::Settings => &["preferences", "control panel"],
            Self::ToggleAppearance => &["appearance", "dark mode", "light mode", "toggle theme"],
            Self::EmptyTrash => &["empty trash", "empty recycle bin", "trash", "recycle bin"],
            Self::Logout => &["logout", "log off", "sign out", "sign off"],
            Self::ShowDesktop => &["desktop", "reveal desktop", "hide windows"],
            Self::ToggleMute => &["mute", "unmute", "toggle sound", "sound output", "volume"],
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
            Self::EmptyTrash => (
                if cfg!(target_os = "windows") {
                    "Empty the Recycle Bin?"
                } else {
                    "Empty the Trash?"
                },
                "This permanently deletes all trashed items, including items on connected drives. You cannot undo this action.",
                self.title(),
            ),
            Self::Logout => (
                "Log out of your account?",
                "Save your work before you continue. This closes applications and ends your current session.",
                "Log out",
            ),
            Self::Lock
            | Self::Settings
            | Self::ToggleAppearance
            | Self::ShowDesktop
            | Self::ToggleMute => return None,
        };
        Some(ActionConfirmation {
            title: title.into(),
            description: description.into(),
            confirm_label: confirm_label.into(),
        })
    }

    pub fn check_confirmation(self, confirmed: bool) -> Result<()> {
        if !confirmed && self.confirmation().is_some() {
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
    /// Recognize at least two characters of a command name or alias prefix.
    pub fn is_prefix_query(query: &str) -> bool {
        let query = ranking::normalize(query);
        query.len() >= 2
            && SystemCommand::ALL.iter().any(|command| {
                std::iter::once(command.title())
                    .chain(command.aliases().iter().copied())
                    .any(|term| {
                        term.get(..query.len())
                            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&query))
                    })
            })
    }

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
        #[cfg(test)]
        super::search_work::record(super::search_work::Provider::System, self.commands.len());
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
    fn prefix_queries_accept_names_and_aliases_from_two_characters() {
        for command in SystemCommand::ALL {
            for term in std::iter::once(command.title()).chain(command.aliases().iter().copied()) {
                for length in 2..=term.len() {
                    assert!(SystemCommandProvider::is_prefix_query(&term[..length]));
                }
                assert!(SystemCommandProvider::is_prefix_query(&format!(
                    "  {}  ",
                    term.to_uppercase().replace(' ', "   ")
                )));
            }
        }
        for query in ["", "s", "r", "sa", "rs", "sleep notes", "app:/sleep", "日"] {
            assert!(!SystemCommandProvider::is_prefix_query(query));
        }
    }

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
            ("dark mode", "system:appearance"),
            ("light mode", "system:appearance"),
            ("empty trash", "system:empty-trash"),
            ("recycle bin", "system:empty-trash"),
            ("sign out", "system:logout"),
            ("show desktop", "system:desktop"),
            ("unmute", "system:mute"),
        ] {
            let results = provider.search(query, &mut matcher);
            assert!(
                results.iter().any(|result| result.id == expected),
                "{query}"
            );
        }
        assert_eq!(
            provider.search("", &mut matcher).len(),
            SystemCommand::ALL.len()
        );
        assert!(provider.search("delete all files", &mut matcher).is_empty());
        assert!(provider.get("system:arbitrary command").is_none());
        assert!(
            SystemCommandProvider::new(vec![SystemCommand::Settings])
                .get("system:shutdown")
                .is_none()
        );
    }

    #[test]
    fn disruptive_commands_require_explicit_confirmation_and_expose_dialog_text() {
        for command in SystemCommand::ALL {
            let required = matches!(
                command,
                SystemCommand::Sleep
                    | SystemCommand::Restart
                    | SystemCommand::Shutdown
                    | SystemCommand::EmptyTrash
                    | SystemCommand::Logout
            );
            assert_eq!(command.confirmation().is_some(), required);
            assert_eq!(command.check_confirmation(false).is_err(), required);
            assert!(command.check_confirmation(true).is_ok());
        }
    }
}
