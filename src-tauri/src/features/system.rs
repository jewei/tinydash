//! Power, session, and TinyDash commands.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    actions::Action,
    search::{
        Context,
        matcher::Matcher,
        result::{Icon, ResultAction, ResultKind, Scored, SearchResult, Symbol},
    },
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
}

struct Command {
    id: &'static str,
    name: &'static str,
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

const fn system(command: SystemCommand) -> Action {
    Action::System { command }
}

const COMMANDS: &[Command] = &[
    Command {
        id: "system:lock",
        name: "Lock Screen",
        aliases: &["lock"],
        symbol: Symbol::Lock,
        action: system(SystemCommand::Lock),
        confirm: None,
    },
    Command {
        id: "system:sleep",
        name: "Sleep",
        aliases: &["suspend"],
        symbol: Symbol::Moon,
        action: system(SystemCommand::Sleep),
        confirm: Some("Put the computer to sleep?"),
    },
    Command {
        id: "system:restart",
        name: "Restart",
        aliases: &["reboot"],
        symbol: Symbol::Restart,
        action: system(SystemCommand::Restart),
        confirm: Some("Restart the computer? Apps will be asked to quit."),
    },
    Command {
        id: "system:shut-down",
        name: "Shut Down",
        aliases: &["shutdown", "power off", "turn off"],
        symbol: Symbol::Power,
        action: system(SystemCommand::ShutDown),
        confirm: Some("Shut down the computer? Apps will be asked to quit."),
    },
    Command {
        id: "system:log-out",
        name: "Log Out",
        aliases: &["logout", "sign out"],
        symbol: Symbol::LogOut,
        action: system(SystemCommand::LogOut),
        confirm: Some("Log out? Apps will be asked to quit."),
    },
    Command {
        id: "system:empty-trash",
        name: TRASH,
        aliases: &["empty trash", "empty recycle bin", "trash", "recycle bin"],
        symbol: Symbol::Trash,
        action: system(SystemCommand::EmptyTrash),
        confirm: Some("Permanently delete everything in the trash? This cannot be undone."),
    },
    Command {
        id: "system:settings",
        name: "System Settings",
        aliases: &["preferences", "control panel"],
        symbol: Symbol::Settings,
        action: system(SystemCommand::OpenSystemSettings),
        confirm: None,
    },
    Command {
        id: "system:tinydash-settings",
        name: "TinyDash Settings",
        aliases: &["preferences", "configure"],
        symbol: Symbol::Settings,
        action: Action::OpenSettings,
        confirm: None,
    },
    Command {
        id: "system:quit-tinydash",
        name: "Quit TinyDash",
        aliases: &["exit"],
        symbol: Symbol::Quit,
        action: Action::Quit,
        confirm: None,
    },
];

fn result(command: &Command, ctx: &Context) -> SearchResult {
    let mut run = ResultAction::new(command.name, command.action.clone());
    run.confirm = command.confirm.map(String::from);
    SearchResult {
        id: command.id.into(),
        kind: ResultKind::System,
        title: command.name.into(),
        subtitle: "System".into(),
        icon: Icon::Symbol {
            name: command.symbol,
        },
        actions: vec![run, ctx.pin_action(command.id)],
        pinned: ctx.pinned(command.id),
    }
}

pub fn search(matcher: &mut Matcher, ctx: &Context) -> Vec<Scored> {
    COMMANDS
        .iter()
        .filter_map(|command| {
            let score = matcher.best(command.name, command.aliases.iter().copied())?;
            Some(Scored {
                score: score + ctx.boost(command.id),
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

pub fn get(id: &str, ctx: &Context) -> Option<SearchResult> {
    COMMANDS
        .iter()
        .find(|command| command.id == id)
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
        let confirmed = |id: &str| get(id, &ctx).unwrap().actions[0].confirm.is_some();
        assert!(confirmed("system:shut-down"));
        assert!(confirmed("system:empty-trash"));
        assert!(!confirmed("system:lock"));
    }
}
