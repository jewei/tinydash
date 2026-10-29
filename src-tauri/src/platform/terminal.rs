//! The parts of `run_in_terminal` that need no OS: the macOS script, the
//! Windows PowerShell arguments, and the Linux terminal list. Each OS file
//! uses its own; tests build all three on every OS.
//!
//! The command never goes into a string that another parser reads with
//! other quoting rules. On macOS it is a line of its own in a script, on
//! Windows it is encoded, and on Linux it is one argument of its own.

#[cfg(any(target_os = "linux", test))]
use std::path::{Path, PathBuf};

/// A script that the app for `.command` files runs (Terminal, or iTerm if
/// the user chose it). zsh, the macOS default shell, reads the command as
/// the line that the user typed. The script deletes itself first, so the
/// command does not stay on disk, and the trap then opens the user's login
/// shell when the command ends, exits, has a syntax error, or is stopped
/// with Control+C.
#[cfg(any(target_os = "macos", test))]
pub(super) fn macos_script(command: &str) -> String {
    format!(
        "#!/bin/zsh\nrm -f -- \"$0\"\ntrap 'exec \"${{SHELL:-/bin/zsh}}\" -l' EXIT INT\n{command}\n"
    )
}

/// PowerShell's arguments to run `command` and then stay open.
/// `-EncodedCommand` takes Base64 of UTF-16LE, so neither the Windows
/// command line nor PowerShell's own parameters can change the command.
#[cfg(any(windows, test))]
pub(super) fn powershell_arguments(command: &str) -> [String; 4] {
    let utf16: Vec<u8> = command.encode_utf16().flat_map(u16::to_le_bytes).collect();
    [
        "-NoLogo".into(),
        "-NoExit".into(),
        "-EncodedCommand".into(),
        base64(&utf16),
    ]
}

/// Standard Base64 with padding (RFC 4648).
#[cfg(any(windows, test))]
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let group = chunk
            .iter()
            .enumerate()
            .fold(0u32, |group, (index, &byte)| {
                group | (u32::from(byte) << (16 - 8 * index))
            });
        // A chunk of n bytes fills n + 1 characters; `=` pads the rest.
        for index in 0..4 {
            text.push(if index <= chunk.len() {
                char::from(ALPHABET[((group >> (18 - 6 * index)) & 0x3f) as usize])
            } else {
                '='
            });
        }
    }
    text
}

/// The terminals that Linux tries in this order after `$TERMINAL`, each
/// with the arguments after which it runs a program and that program's own
/// arguments.
#[cfg(any(target_os = "linux", test))]
const TERMINALS: &[(&str, &[&str])] = &[
    ("x-terminal-emulator", &["-e"]),
    ("gnome-terminal", &["--"]),
    ("konsole", &["-e"]),
    ("xfce4-terminal", &["-x"]),
    ("kitty", &[]),
    ("alacritty", &["-e"]),
    ("wezterm", &["start", "--"]),
    ("foot", &["--"]),
    ("xterm", &["-e"]),
];

/// The `sh` script that runs the command, which it gets as `$1`. `command
/// eval` keeps `sh` running after a syntax error, and the trap then opens
/// the user's login shell, also after `exit` or Control+C.
#[cfg(any(target_os = "linux", test))]
const LINUX_SCRIPT: &str = r#"trap 'exec "${SHELL:-sh}" -l' EXIT INT; command eval "$1""#;

/// The terminal that runs `command`, and its arguments: `$TERMINAL`
/// (`preferred`) when `find` finds it, else the first of [`TERMINALS`]
/// that `find` finds. `find` gives the full path of a program that can run.
/// A terminal not in the list gets `-e`, which most terminals take.
#[cfg(any(target_os = "linux", test))]
pub(super) fn linux_terminal(
    preferred: Option<&str>,
    find: impl Fn(&str) -> Option<PathBuf>,
    command: &str,
) -> Option<(PathBuf, Vec<String>)> {
    let flags_of = |name: &str| {
        TERMINALS
            .iter()
            .find(|&&(known, _)| {
                Path::new(name)
                    .file_name()
                    .is_some_and(|file| file == known)
            })
            .map_or(&["-e"][..], |&(_, flags)| flags)
    };
    let (program, flags) = preferred
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .and_then(|name| Some((find(name)?, flags_of(name))))
        .or_else(|| {
            TERMINALS
                .iter()
                .find_map(|&(name, flags)| Some((find(name)?, flags)))
        })?;
    let arguments = flags
        .iter()
        .copied()
        .chain(["sh", "-c", LINUX_SCRIPT, "sh", command])
        .map(str::to_owned)
        .collect();
    Some((program, arguments))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_macos_script_holds_the_command_as_its_own_line() {
        let command = r#"echo "{clipboard}" 'it''s' $HOME \"#;
        assert_eq!(
            macos_script(command),
            format!(
                "#!/bin/zsh\nrm -f -- \"$0\"\ntrap 'exec \"${{SHELL:-/bin/zsh}}\" -l' EXIT INT\n{command}\n"
            )
        );
        assert_eq!(macos_script(command).lines().last(), Some(command));
    }

    #[test]
    fn encodes_base64() {
        for (bytes, text) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(bytes.as_bytes()), text, "{bytes}");
        }
        assert_eq!(base64(&[0xfb, 0xff, 0xfe]), "+//+");
    }

    #[test]
    fn powershell_gets_the_command_as_utf16_in_base64() {
        // `dir` in UTF-16LE is the bytes 64 00 69 00 72 00.
        assert_eq!(
            powershell_arguments("dir"),
            ["-NoLogo", "-NoExit", "-EncodedCommand", "ZABpAHIA"]
        );
        // Quotes and signs stay inside the encoding, and 🚀, outside the
        // Basic Multilingual Plane, takes two UTF-16 units. The expected
        // text comes from Python's `base64` and `utf-16-le` codecs.
        let [.., encoded] = powershell_arguments("echo \"a b\" & 🚀");
        assert_eq!(encoded, "ZQBjAGgAbwAgACIAYQAgAGIAIgAgACYAIAA92IDe");
    }

    #[test]
    fn linux_uses_terminal_first_then_the_first_terminal_found() {
        // A name is found in /usr/bin; a path is found where it is.
        let installed = |names: &'static [&'static str]| {
            move |name: &str| {
                names
                    .contains(&name)
                    .then(|| Path::new("/usr/bin").join(name))
            }
        };
        let tail = |command: &str| ["sh", "-c", LINUX_SCRIPT, "sh", command].map(str::to_owned);
        let command = r#"git pull && echo "done: $HOME" '{clipboard}'"#;

        let (program, arguments) =
            linux_terminal(None, installed(&["konsole", "xterm"]), command).unwrap();
        assert_eq!(program, Path::new("/usr/bin/konsole"));
        assert_eq!(arguments[0], "-e");
        assert_eq!(arguments[1..], tail(command));

        // $TERMINAL wins when it runs, with its own way to take a program.
        let both = installed(&["gnome-terminal", "kitty"]);
        let (program, arguments) = linux_terminal(Some("kitty"), both, command).unwrap();
        assert_eq!(program, Path::new("/usr/bin/kitty"));
        assert_eq!(arguments, tail(command));
        let (_, arguments) =
            linux_terminal(Some("wezterm"), installed(&["wezterm"]), command).unwrap();
        assert_eq!(arguments[..2], ["start", "--"]);
        // One TinyDash does not know gets `-e`.
        let (program, arguments) =
            linux_terminal(Some("/opt/st"), installed(&["/opt/st"]), command).unwrap();
        assert_eq!(program, Path::new("/opt/st"));
        assert_eq!(arguments[0], "-e");
        let (_, arguments) = linux_terminal(
            Some("/usr/local/bin/xfce4-terminal"),
            installed(&["/usr/local/bin/xfce4-terminal"]),
            command,
        )
        .unwrap();
        assert_eq!(arguments[0], "-x");
        // A $TERMINAL that is missing or blank leaves the choice to the list.
        for preferred in [Some("ghostty"), Some("  "), None] {
            let found = linux_terminal(preferred, installed(&["gnome-terminal"]), command);
            let (program, arguments) = found.unwrap();
            assert_eq!(program, Path::new("/usr/bin/gnome-terminal"));
            assert_eq!(arguments[0], "--");
        }
        assert_eq!(linux_terminal(Some("kitty"), installed(&[]), command), None);
    }

    #[test]
    fn the_list_tries_each_terminal_in_order() {
        let names: Vec<&str> = TERMINALS.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            [
                "x-terminal-emulator",
                "gnome-terminal",
                "konsole",
                "xfce4-terminal",
                "kitty",
                "alacritty",
                "wezterm",
                "foot",
                "xterm"
            ]
        );
        // With each terminal and the ones after it installed, it is chosen.
        for (index, name) in names.iter().enumerate() {
            let later = &names[index..];
            let find = |asked: &str| later.contains(&asked).then(|| PathBuf::from(asked));
            let (program, _) = linux_terminal(None, find, "ls").unwrap();
            assert_eq!(program, PathBuf::from(name));
        }
    }

    /// Runs the Linux script in `sh`, with `echo` as the login shell, which
    /// prints `-l` where the user's shell would open. Windows has no `sh`.
    #[cfg(unix)]
    #[test]
    fn the_linux_script_runs_the_command_then_opens_the_login_shell() {
        let run = |command: &str| {
            let output = std::process::Command::new("sh")
                .args(["-c", LINUX_SCRIPT, "sh", command])
                .env("SHELL", "echo")
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap();
            String::from_utf8(output.stdout).unwrap()
        };
        assert_eq!(run("echo '{clipboard}' \"$0\""), "{clipboard} sh\n-l\n");
        assert_eq!(run("cd / && pwd"), "/\n-l\n");
        // Exit and a syntax error still leave the shell open.
        assert_eq!(run("exit 3"), "-l\n");
        assert_eq!(run("echo it's"), "-l\n");
    }
}
