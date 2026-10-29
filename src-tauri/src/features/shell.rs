//! `>` and a command run that command in the user's terminal app, while
//! Settings > Search allows it. The command is the text after `>`, as
//! typed: no placeholder is filled, so clipboard text never becomes part of
//! a command. Nothing runs until the user picks the row.

use crate::{
    error::{Error, Result},
    search::result::{Action, Icon, ResultAction, ResultKind, SearchResult, Symbol},
};

/// The longest command, in characters. `check` refuses longer ones.
const MAX_COMMAND_CHARS: usize = 1000;

/// Where the command runs, for labels. Windows runs it in PowerShell, whose
/// syntax differs from a Unix shell's.
const TERMINAL: &str = if cfg!(windows) {
    "PowerShell"
} else {
    "Terminal"
};

/// The row for `> command`, or no rows for `>` alone; `None` when the query
/// does not start with `>`. Like `=` and `?`, `>` asks for this and nothing
/// else, so Enter runs the command or nothing, never another result.
pub fn answer(query: &str) -> Option<Vec<SearchResult>> {
    let command = query.trim_start().strip_prefix('>')?.trim();
    if command.is_empty() {
        return Some(Vec::new());
    }
    let run = format!("Run in {TERMINAL}");
    // A command that cannot run says why before Enter does.
    let subtitle = check(command).map_or_else(|refused| refused.to_string(), |()| run.clone());
    Some(vec![SearchResult {
        id: format!("shell:{command}"),
        kind: ResultKind::Shell,
        title: command.to_owned(),
        subtitle,
        icon: Icon::Symbol {
            name: Symbol::Terminal,
        },
        actions: vec![
            ResultAction::new(
                run,
                Action::RunInTerminal {
                    command: command.to_owned(),
                },
            ),
            ResultAction::new(
                "Copy Command",
                Action::Copy {
                    text: command.to_owned(),
                },
            ),
        ],
        pinned: false,
    }])
}

/// Refuse a command that is empty, longer than [`MAX_COMMAND_CHARS`], or
/// holds a control character: a line break would start a second command.
pub fn check(command: &str) -> Result<()> {
    if command.trim().is_empty() {
        return Err(Error::msg("Type a command after >."));
    }
    if command.chars().count() > MAX_COMMAND_CHARS {
        return Err(Error::msg(format!(
            "A command can have up to {MAX_COMMAND_CHARS} characters. Run a longer one in {TERMINAL} yourself."
        )));
    }
    if command.chars().any(char::is_control) {
        return Err(Error::msg(
            "A command must be one line, without tabs or other control characters.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions(query: &str) -> Vec<Action> {
        answer(query).unwrap()[0]
            .actions
            .iter()
            .map(|entry| entry.action.clone())
            .collect()
    }

    #[test]
    fn a_command_after_the_sign_runs_or_copies_as_typed() {
        let rows = answer("  >  git pull --rebase ").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "git pull --rebase");
        assert_eq!(rows[0].subtitle, format!("Run in {TERMINAL}"));
        assert_eq!(rows[0].kind, ResultKind::Shell);
        let labels: Vec<&str> = rows[0].actions.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(
            labels,
            [format!("Run in {TERMINAL}").as_str(), "Copy Command"]
        );
        assert_eq!(
            actions(">git pull --rebase"),
            [
                Action::RunInTerminal {
                    command: "git pull --rebase".into()
                },
                Action::Copy {
                    text: "git pull --rebase".into()
                },
            ]
        );
        // Placeholders stay as typed: clipboard text never joins a command.
        assert_eq!(
            actions("> echo {clipboard} {date}")[0],
            Action::RunInTerminal {
                command: "echo {clipboard} {date}".into()
            }
        );
    }

    #[test]
    fn the_sign_alone_offers_nothing_to_run() {
        for query in [">", " > ", ">   "] {
            assert_eq!(answer(query), Some(Vec::new()), "{query:?}");
        }
        for query in ["", "git pull", "a > b", "=> x", "?"] {
            assert_eq!(answer(query), None, "{query:?}");
        }
    }

    #[test]
    fn refuses_empty_long_and_multi_line_commands() {
        assert!(check("ls -la").is_ok());
        assert!(check(&"x".repeat(MAX_COMMAND_CHARS)).is_ok());
        // Characters, not bytes: each "é" is two bytes.
        assert!(check(&"é".repeat(MAX_COMMAND_CHARS)).is_ok());
        let too_long = check(&"x".repeat(MAX_COMMAND_CHARS + 1)).unwrap_err();
        assert!(too_long.to_string().contains("up to 1000"), "{too_long}");
        for command in [
            "",
            "   ",
            "ls\nrm -rf ~",
            "ls\r",
            "ls\0",
            "ls\tx",
            "ls\u{85}",
        ] {
            assert!(check(command).is_err(), "{command:?}");
        }
    }

    #[test]
    fn a_command_that_cannot_run_says_why_in_its_row() {
        let long = format!("> {}", "x".repeat(MAX_COMMAND_CHARS + 1));
        let row = &answer(&long).unwrap()[0];
        assert!(row.subtitle.starts_with("A command can have up to"));
        let row = &answer("> ls\tx").unwrap()[0];
        assert!(row.subtitle.starts_with("A command must be one line"));
    }
}
