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

/// Refuse to run `command` while shell commands are off (`enabled`), or
/// when `check` refuses it. `actions.rs` asks this before it runs one,
/// because the setting may have changed since the row showed.
pub fn runnable(enabled: bool, command: &str) -> Result<()> {
    if !enabled {
        return Err(Error::msg(
            "Shell commands are off. Turn on Run shell commands in Settings > Search.",
        ));
    }
    check(command)
}

/// Refuse a command that is empty, longer than [`MAX_COMMAND_CHARS`], or
/// holds a control character, a line or paragraph separator, or a direction
/// mark: a line break would start a second command, and a direction mark
/// can make the row show text that differs from what the shell reads.
fn check(command: &str) -> Result<()> {
    if command.trim().is_empty() {
        return Err(Error::msg("Type a command after >."));
    }
    if command.chars().count() > MAX_COMMAND_CHARS {
        return Err(Error::msg(format!(
            "A command can have up to {MAX_COMMAND_CHARS} characters. Run a longer one in {TERMINAL} yourself."
        )));
    }
    if command
        .chars()
        .any(|character| character.is_control() || matches!(character, '\u{2028}' | '\u{2029}'))
    {
        return Err(Error::msg(
            "A command must be one line, without tabs or other control characters.",
        ));
    }
    if command.chars().any(is_direction_mark) {
        return Err(Error::msg(
            "The command has an invisible direction mark, which can hide what it does. Type the command instead of pasting it.",
        ));
    }
    Ok(())
}

/// The invisible marks that change the order in which the row shows text,
/// but not the order in which the shell reads it. The zero width joiner
/// (U+200D) is not one: emoji need it.
fn is_direction_mark(character: char) -> bool {
    matches!(
        character,
        '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
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
            // U+202E shows the rest of the line in reverse order.
            "rm \u{202E}~ fr-",
            "echo \u{2066}a\u{2069}",
            "echo \u{200F}a",
            "echo \u{061C}a",
            "ls\u{2028}rm -rf ~",
            "ls\u{2029}",
        ] {
            assert!(check(command).is_err(), "{command:?}");
        }
        // An emoji sequence joins its parts with U+200D.
        assert!(check("echo 👩\u{200D}💻").is_ok());
    }

    #[test]
    fn runs_only_while_the_setting_is_on() {
        let off = runnable(false, "ls").unwrap_err();
        assert!(
            off.to_string().starts_with("Shell commands are off"),
            "{off}"
        );
        assert!(runnable(true, "ls -la").is_ok());
        for enabled in [true, false] {
            assert!(runnable(enabled, "ls\nrm -rf ~").is_err(), "{enabled}");
            assert!(runnable(enabled, "").is_err(), "{enabled}");
        }
    }

    #[test]
    fn a_command_that_cannot_run_says_why_in_its_row() {
        let long = format!("> {}", "x".repeat(MAX_COMMAND_CHARS + 1));
        let row = &answer(&long).unwrap()[0];
        assert!(row.subtitle.starts_with("A command can have up to"));
        let row = &answer("> ls\tx").unwrap()[0];
        assert!(row.subtitle.starts_with("A command must be one line"));
        // A direction mark is invisible, so its message names it.
        for command in [
            "echo \u{200F}a",
            "rm \u{202E}~ fr-",
            "echo \u{2066}a\u{2069}",
        ] {
            let row = &answer(&format!("> {command}")).unwrap()[0];
            assert!(
                row.subtitle.contains("invisible direction mark"),
                "{command:?}: {}",
                row.subtitle
            );
        }
        let row = &answer("> ls\u{2028}rm -rf ~").unwrap()[0];
        assert!(row.subtitle.starts_with("A command must be one line"));
    }
}
