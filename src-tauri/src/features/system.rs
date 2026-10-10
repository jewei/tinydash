//! Power, session, and TinyDash commands.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::search::{
    Context,
    id::Source,
    matcher::Matcher,
    result::{Action, Icon, ResultAction, ResultKind, Scored, SearchResult, Symbol},
};

/// Commands the platform layer runs. See `platform::run_system_command`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SystemCommand {
    Lock,
    Sleep,
    Restart,
    ShutDown,
    LogOut,
    EmptyTrash,
    OpenSystemSettings,
    SleepDisplays,
    ToggleDarkMode,
    OpenTrash,
}

struct Command {
    /// Stable key in the result ID `system:<key>`.
    key: &'static str,
    name: &'static str,
    description: &'static str,
    aliases: &'static [&'static str],
    symbol: Symbol,
    action: Action,
    /// Commands that end the session or delete data ask first.
    confirm: Option<&'static str>,
}

const TRASH: &str = if cfg!(windows) {
    "Empty Recycle Bin"
} else {
    "Empty Trash"
};

const OPEN_TRASH: &str = if cfg!(windows) {
    "Open Recycle Bin"
} else {
    "Open Trash"
};

const SLEEP_DISPLAYS: &str = if cfg!(target_os = "macos") {
    "Sleep Displays"
} else {
    "Turn Off Displays"
};

const fn system(command: SystemCommand) -> Action {
    Action::System { command }
}

const COMMANDS: &[Command] = &[
    Command {
        key: "lock",
        name: "Lock Screen",
        description: "Lock this computer",
        aliases: &["lock"],
        symbol: Symbol::Lock,
        action: system(SystemCommand::Lock),
        confirm: None,
    },
    Command {
        key: "sleep",
        name: "Sleep",
        description: "Put the computer to sleep",
        aliases: &["suspend"],
        symbol: Symbol::Moon,
        action: system(SystemCommand::Sleep),
        confirm: Some("Put the computer to sleep?"),
    },
    Command {
        key: "restart",
        name: "Restart",
        description: "Restart the computer",
        aliases: &["reboot"],
        symbol: Symbol::Restart,
        action: system(SystemCommand::Restart),
        confirm: Some("Restart the computer? Apps will be asked to quit."),
    },
    Command {
        key: "shut-down",
        name: "Shut Down",
        description: "Turn off the computer",
        aliases: &["shutdown", "power off", "turn off"],
        symbol: Symbol::Power,
        action: system(SystemCommand::ShutDown),
        confirm: Some("Shut down the computer? Apps will be asked to quit."),
    },
    Command {
        key: "log-out",
        name: "Log Out",
        description: "End this session",
        aliases: &["logout", "sign out"],
        symbol: Symbol::LogOut,
        action: system(SystemCommand::LogOut),
        confirm: Some("Log out? Apps will be asked to quit."),
    },
    Command {
        key: "empty-trash",
        name: TRASH,
        description: "Permanently delete trashed items",
        aliases: &["empty trash", "empty recycle bin", "trash", "recycle bin"],
        symbol: Symbol::Trash,
        action: system(SystemCommand::EmptyTrash),
        confirm: Some("Permanently delete everything in the trash? This cannot be undone."),
    },
    Command {
        key: "open-trash",
        name: OPEN_TRASH,
        description: "Show the items in the trash",
        aliases: &["trash", "recycle bin", "bin"],
        symbol: Symbol::Trash,
        action: system(SystemCommand::OpenTrash),
        confirm: None,
    },
    Command {
        key: "sleep-displays",
        name: SLEEP_DISPLAYS,
        description: "Turn off the screens; the computer keeps running",
        aliases: &["display off", "screen off", "monitor off", "sleep displays"],
        symbol: Symbol::Display,
        action: system(SystemCommand::SleepDisplays),
        confirm: None,
    },
    Command {
        key: "toggle-dark-mode",
        name: "Toggle Dark Mode",
        description: "Switch the system between light and dark",
        aliases: &["dark mode", "light mode", "appearance", "theme"],
        symbol: Symbol::Contrast,
        action: system(SystemCommand::ToggleDarkMode),
        confirm: None,
    },
    Command {
        key: "clear-clipboard",
        name: "Clear Clipboard History",
        description: "Delete every entry except pinned ones",
        aliases: &["clear history", "delete clipboard"],
        symbol: Symbol::Trash,
        action: Action::ClearClipboard,
        confirm: Some("Delete all clipboard history except pinned entries?"),
    },
    Command {
        key: "settings",
        name: "System Settings",
        description: "Open the operating system settings",
        aliases: &["preferences", "control panel"],
        symbol: Symbol::Settings,
        action: system(SystemCommand::OpenSystemSettings),
        confirm: None,
    },
    Command {
        key: "tinydash-settings",
        name: "TinyDash Settings",
        description: "Shortcut, clipboard, files, and more",
        aliases: &["preferences", "configure"],
        symbol: Symbol::Settings,
        action: Action::OpenSettings,
        confirm: None,
    },
    Command {
        key: "quit-tinydash",
        name: "Quit TinyDash",
        description: "Stop TinyDash until you open it again",
        aliases: &["exit"],
        symbol: Symbol::Quit,
        action: Action::Quit,
        confirm: None,
    },
];

fn result(command: &Command, ctx: &Context) -> SearchResult {
    let id = Source::System.id(command.key);
    let mut run = ResultAction::new(command.name, command.action.clone());
    run.confirm = command.confirm.map(String::from);
    SearchResult {
        id: id.clone(),
        kind: ResultKind::System,
        title: command.name.into(),
        subtitle: command.description.into(),
        icon: Icon::Symbol {
            name: command.symbol,
        },
        actions: vec![run, ctx.pin_action(&id)],
        pinned: ctx.pinned(&id),
    }
}

pub fn search(matcher: &mut Matcher, ctx: &Context) -> Vec<Scored> {
    COMMANDS
        .iter()
        .filter_map(|command| {
            let score = matcher.best(command.name, command.aliases.iter().copied())?;
            Some(Scored {
                score: score + ctx.boost(&Source::System.id(command.key)),
                result: result(command, ctx),
            })
        })
        .collect()
}

pub fn browse(ctx: &Context) -> Vec<SearchResult> {
    COMMANDS
        .iter()
        .map(|command| result(command, ctx))
        .collect()
}

pub fn get(key: &str, ctx: &Context) -> Option<SearchResult> {
    COMMANDS
        .iter()
        .find(|command| command.key == key)
        .map(|command| result(command, ctx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::usage::{Pins, Usage};

    fn titles(query: &str) -> Vec<String> {
        let (usage, pins) = (Usage::default(), Pins::default());
        let ctx = Context {
            usage: &usage,
            pins: &pins,
            now: 0,
            skin_tone: 0,
        };
        let mut hits = search(&mut Matcher::new(query), &ctx);
        hits.sort_by_key(|hit| std::cmp::Reverse(hit.score));
        hits.into_iter().map(|hit| hit.result.title).collect()
    }

    #[test]
    fn aliases_find_commands() {
        assert_eq!(titles("reboot")[0], "Restart");
        assert_eq!(titles("suspend")[0], "Sleep");
        assert_eq!(titles("sign out")[0], "Log Out");
        assert_eq!(titles("dark mode")[0], "Toggle Dark Mode");
        assert_eq!(titles("screen off")[0], SLEEP_DISPLAYS);
    }

    #[test]
    fn destructive_commands_ask_first() {
        let (usage, pins) = (Usage::default(), Pins::default());
        let ctx = Context {
            usage: &usage,
            pins: &pins,
            now: 0,
            skin_tone: 0,
        };
        let confirmed = |key: &str| get(key, &ctx).unwrap().actions[0].confirm.is_some();
        assert!(confirmed("shut-down"));
        assert!(confirmed("empty-trash"));
        assert!(!confirmed("lock"));
    }
}
