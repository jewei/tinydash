//! The parts of `run_in_terminal` that need no OS: the macOS script, the
//! Windows PowerShell arguments, the Linux terminal list and environment,
//! and the private file that carries a command on macOS and Linux. Each OS
//! file uses its own; tests build them on every OS (the file on Unix).
//!
//! The command never goes into a string that another parser reads with
//! other quoting rules. On macOS it is a line of its own in a script, on
//! Windows it is encoded, and on Linux `sh` reads it as one line from a
//! file. On macOS and Linux it stays out of every program's arguments,
//! which other users of the computer can read.

#[cfg(any(target_os = "linux", test))]
use std::ffi::OsString;
#[cfg(any(unix, test))]
use std::path::{Path, PathBuf};

/// The folder under the app's data folder that holds the commands on their
/// way to a terminal.
#[cfg(unix)]
const COMMAND_FOLDER: &str = "shell-commands";

/// A command file still here after this long never ran, as each one is
/// deleted when it runs. A newer one stays, so the terminal has time to
/// start and read it.
#[cfg(unix)]
const COMMAND_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Write `contents` to a new file in a folder under `data`, each readable,
/// writable, and runnable by the user alone, after deleting the files there
/// that never ran.
#[cfg(unix)]
pub(super) fn command_file(data: &Path, contents: &str) -> std::io::Result<PathBuf> {
    use std::{
        io::Write,
        os::unix::fs::{OpenOptionsExt, PermissionsExt},
    };
    let folder = data.join(COMMAND_FOLDER);
    std::fs::create_dir_all(&folder)?;
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700))?;
    let now = std::time::SystemTime::now();
    remove_old_command_files(&folder, now);
    let stamp = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = folder.join(format!("{stamp}.command"));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&path)?;
    // The umask may have taken a bit away, and Terminal runs only a script
    // that can run.
    let written = file
        .set_permissions(std::fs::Permissions::from_mode(0o700))
        .and_then(|()| file.write_all(contents.as_bytes()));
    // A file that is not complete can hold part of the command, and the
    // caller gets no path to delete it.
    if let Err(error) = written {
        std::fs::remove_file(&path).ok();
        return Err(error);
    }
    Ok(path)
}

/// After a terminal ends: whether it failed before `sh` read the command
/// in `file`, which `sh` deletes when it starts. This deletes the file of
/// a command that never ran. A terminal that ends with an error after `sh`
/// read the file did not fail to start it: some, such as foot, end with the
/// status of the shell that the user closed.
#[cfg(any(target_os = "linux", test))]
pub(super) fn failed_before_reading(success: bool, file: &Path) -> bool {
    if success || !file.exists() {
        return false;
    }
    std::fs::remove_file(file).ok();
    true
}

/// Delete the command files in `folder` older than [`COMMAND_MAX_AGE`] at
/// `now`.
#[cfg(unix)]
fn remove_old_command_files(folder: &Path, now: std::time::SystemTime) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let old = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| {
                now.duration_since(modified)
                    .is_ok_and(|age| age > COMMAND_MAX_AGE)
            });
        if old
            && path
                .extension()
                .is_some_and(|extension| extension == "command")
            && let Err(error) = std::fs::remove_file(&path)
        {
            tracing::warn!(%error, "Could not delete an old command file");
        }
    }
}

/// A script that the app for `.command` files runs (Terminal, or iTerm if
/// the user chose it). zsh, the macOS default shell, reads the command as
/// the line that the user typed. The script deletes itself first, so the
/// command does not stay on disk, and the trap then opens the user's login
/// shell when the command ends, exits, has a syntax error, or is stopped
/// with Control+C. `cd` starts the command in the home folder, because the
/// app can start the script in another folder (iTerm uses its profile's).
#[cfg(any(target_os = "macos", test))]
pub(super) fn macos_script(command: &str) -> String {
    format!(
        "#!/bin/zsh\nrm -f -- \"$0\"\ntrap 'exec \"${{SHELL:-/bin/zsh}}\" -l' EXIT INT\ncd\n{command}\n"
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
/// arguments. Ptyxis (Fedora 41 and later, Ubuntu 25.10 and later) turns
/// what follows `--` into its `-x` command, and `--new-window` keeps it in
/// the running instance; GNOME Console (`kgx`) runs what follows `--` in a
/// new window.
#[cfg(any(target_os = "linux", test))]
const TERMINALS: &[(&str, &[&str])] = &[
    ("x-terminal-emulator", &["-e"]),
    ("gnome-terminal", &["--"]),
    ("ptyxis", &["--new-window", "--"]),
    ("kgx", &["--"]),
    ("konsole", &["-e"]),
    ("xfce4-terminal", &["-x"]),
    ("kitty", &[]),
    ("alacritty", &["-e"]),
    ("wezterm", &["start", "--"]),
    ("foot", &["--"]),
    ("xterm", &["-e"]),
];

/// The `sh` script that runs the command, which it reads as one line from
/// the file `$1` and then deletes. `command eval` keeps `sh` running after
/// a syntax error, and the trap then opens the user's login shell, also
/// after `exit` or Control+C.
#[cfg(any(target_os = "linux", test))]
const LINUX_SCRIPT: &str = r#"IFS= read -r tinydash_command < "$1"; rm -f -- "$1"; trap 'exec "${SHELL:-sh}" -l' EXIT INT; command eval "$tinydash_command""#;

/// The terminal that runs the command in `file`, and its arguments:
/// `$TERMINAL` (`preferred`) when `find` finds it, else the first of
/// [`TERMINALS`] that `find` finds. `find` gives the full path of a program
/// that can run. A terminal not in the list gets `-e`, which most terminals
/// take.
#[cfg(any(target_os = "linux", test))]
pub(super) fn linux_terminal(
    preferred: Option<&str>,
    find: impl Fn(&str) -> Option<PathBuf>,
    file: &Path,
) -> Option<(PathBuf, Vec<OsString>)> {
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
        .chain(&["sh", "-c", LINUX_SCRIPT, "sh"])
        .map(OsString::from)
        .chain([file.as_os_str().to_owned()])
        .collect();
    Some((program, arguments))
}

/// Variables that the AppImage runtime and its GTK hook set for TinyDash
/// alone. The hook replaced any value of the user's own for `GDK_BACKEND`
/// and `GTK_THEME`, so removing them gives the default back.
#[cfg(any(target_os = "linux", test))]
const APPIMAGE_ONLY: &[&str] = &[
    "APPDIR",
    "APPIMAGE",
    "ARGV0",
    "OWD",
    "GDK_BACKEND",
    "GTK_THEME",
];

/// The changes that give a program started from the AppImage build the
/// environment of the user's own programs, as `(name, value)`, where `None`
/// removes the variable. TinyDash's GTK needs the folders inside the
/// AppImage, such as in `XDG_DATA_DIRS` and `GIO_EXTRA_MODULES`, but a
/// terminal and the shell in it would keep them after TinyDash quits and
/// the AppImage is gone. Outside an AppImage, nothing changes.
#[cfg(any(target_os = "linux", test))]
pub(super) fn outside_appimage(
    environment: impl IntoIterator<Item = (String, String)>,
) -> Vec<(String, Option<String>)> {
    let environment: Vec<(String, String)> = environment.into_iter().collect();
    let value = |name: &str| {
        environment
            .iter()
            .find(|(own, _)| own == name)
            .map(|(_, value)| value.as_str())
    };
    // The root, or a relative folder, would match every path.
    let Some(appdir) = value("APPDIR")
        .filter(|appdir| {
            let appdir = Path::new(appdir);
            appdir.has_root() && appdir.parent().is_some()
        })
        .filter(|_| value("APPIMAGE").is_some())
    else {
        return Vec::new();
    };
    // The GTK hook sets `XDG_DATA_DIRS="$APPDIR/usr/share:/usr/share:$XDG_DATA_DIRS"`,
    // so the user's own value, empty when it was not set, follows its two
    // folders. Its `/usr/share` is outside the AppImage, but the user's
    // value may not have it there, or at all.
    let data_dirs_prefix = format!("{appdir}/usr/share:/usr/share:");
    let inside = |entry: &str| Path::new(entry).starts_with(appdir);
    environment
        .iter()
        .filter_map(|(name, value)| {
            if APPIMAGE_ONLY.contains(&name.as_str()) {
                return Some((name.clone(), None));
            }
            if !value.split(':').any(inside) {
                return None;
            }
            let own = if name == "XDG_DATA_DIRS" {
                value.strip_prefix(&data_dirs_prefix).unwrap_or(value)
            } else {
                value
            };
            let kept: Vec<&str> = own
                .split(':')
                .filter(|entry| !entry.is_empty() && !inside(entry))
                .collect();
            Some((name.clone(), (!kept.is_empty()).then(|| kept.join(":"))))
        })
        .collect()
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
                "#!/bin/zsh\nrm -f -- \"$0\"\ntrap 'exec \"${{SHELL:-/bin/zsh}}\" -l' EXIT INT\ncd\n{command}\n"
            )
        );
        assert_eq!(macos_script(command).lines().last(), Some(command));
    }

    #[test]
    fn a_terminal_failed_only_when_the_command_never_ran() {
        let file = std::env::temp_dir().join(format!(
            "tinydash-terminal-exit-{}.command",
            std::process::id()
        ));
        std::fs::write(&file, "ls\n").unwrap();
        // A terminal that ended well may not have started `sh` yet.
        assert!(!failed_before_reading(true, &file));
        assert!(file.exists());
        // One that ended with an error before `sh` read the file failed,
        // and the command goes.
        assert!(failed_before_reading(false, &file));
        assert!(!file.exists());
        // After `sh` read and deleted the file, an error is the status of
        // the shell that the user closed.
        assert!(!failed_before_reading(false, &file));
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
        let file = Path::new("/home/me/.local/share/tinydash/shell-commands/1.command");
        let tail = ["sh", "-c", LINUX_SCRIPT, "sh", file.to_str().unwrap()].map(OsString::from);

        let (program, arguments) =
            linux_terminal(None, installed(&["konsole", "xterm"]), file).unwrap();
        assert_eq!(program, Path::new("/usr/bin/konsole"));
        assert_eq!(arguments[0], "-e");
        assert_eq!(arguments[1..], tail);

        // $TERMINAL wins when it runs, with its own way to take a program.
        let both = installed(&["gnome-terminal", "kitty"]);
        let (program, arguments) = linux_terminal(Some("kitty"), both, file).unwrap();
        assert_eq!(program, Path::new("/usr/bin/kitty"));
        assert_eq!(arguments, tail);
        let (_, arguments) =
            linux_terminal(Some("wezterm"), installed(&["wezterm"]), file).unwrap();
        assert_eq!(arguments[..2], ["start", "--"]);
        let (_, arguments) = linux_terminal(Some("ptyxis"), installed(&["ptyxis"]), file).unwrap();
        assert_eq!(arguments[..2], ["--new-window", "--"]);
        assert_eq!(arguments[2..], tail);
        // One TinyDash does not know gets `-e`.
        let (program, arguments) =
            linux_terminal(Some("/opt/st"), installed(&["/opt/st"]), file).unwrap();
        assert_eq!(program, Path::new("/opt/st"));
        assert_eq!(arguments[0], "-e");
        let (_, arguments) = linux_terminal(
            Some("/usr/local/bin/xfce4-terminal"),
            installed(&["/usr/local/bin/xfce4-terminal"]),
            file,
        )
        .unwrap();
        assert_eq!(arguments[0], "-x");
        // A $TERMINAL that is missing or blank leaves the choice to the list.
        for preferred in [Some("ghostty"), Some("  "), None] {
            let found = linux_terminal(preferred, installed(&["gnome-terminal"]), file);
            let (program, arguments) = found.unwrap();
            assert_eq!(program, Path::new("/usr/bin/gnome-terminal"));
            assert_eq!(arguments[0], "--");
        }
        assert_eq!(linux_terminal(Some("kitty"), installed(&[]), file), None);
    }

    #[test]
    fn the_list_tries_each_terminal_in_order() {
        let names: Vec<&str> = TERMINALS.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            [
                "x-terminal-emulator",
                "gnome-terminal",
                "ptyxis",
                "kgx",
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
        let file = Path::new("/tmp/1.command");
        for (index, name) in names.iter().enumerate() {
            let later = &names[index..];
            let find = |asked: &str| later.contains(&asked).then(|| PathBuf::from(asked));
            let (program, arguments) = linux_terminal(None, find, file).unwrap();
            assert_eq!(program, PathBuf::from(name));
            assert_eq!(
                arguments.last().map(OsString::as_os_str),
                Some(file.as_os_str())
            );
        }
    }

    #[test]
    fn a_terminal_from_the_appimage_gets_the_users_environment() {
        let appdir = "/tmp/.mount_TinyDaAbC";
        let environment = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|&(name, value)| (name.to_owned(), value.to_owned()))
                .collect()
        };
        let mut changes = outside_appimage(environment(&[
            ("APPIMAGE", "/home/me/TinyDash.AppImage"),
            ("APPDIR", appdir),
            ("ARGV0", "TinyDash.AppImage"),
            ("OWD", "/home/me"),
            ("GDK_BACKEND", "x11"),
            ("GTK_THEME", "Adwaita:dark"),
            ("GTK_DATA_PREFIX", appdir),
            (
                "GIO_EXTRA_MODULES",
                "/tmp/.mount_TinyDaAbC//usr/lib/gio/modules",
            ),
            (
                "XDG_DATA_DIRS",
                "/tmp/.mount_TinyDaAbC/usr/share:/usr/share:/usr/local/share:/usr/share",
            ),
            ("HOME", "/home/me"),
            ("PATH", "/usr/bin:/bin"),
            // Another folder whose name only starts the same way stays.
            ("NOTES", "/tmp/.mount_TinyDaAbCD/notes"),
        ]));
        changes.sort();
        let removed = |name: &str| (name.to_owned(), None);
        assert_eq!(
            changes,
            [
                removed("APPDIR"),
                removed("APPIMAGE"),
                removed("ARGV0"),
                removed("GDK_BACKEND"),
                removed("GIO_EXTRA_MODULES"),
                removed("GTK_DATA_PREFIX"),
                removed("GTK_THEME"),
                removed("OWD"),
                (
                    "XDG_DATA_DIRS".to_owned(),
                    Some("/usr/local/share:/usr/share".to_owned())
                ),
            ]
        );
        // `XDG_DATA_DIRS` gets back the user's own value, as it was: none
        // when the session did not set it, so the default applies again,
        // and its own order otherwise. A value that the GTK hook did not
        // make loses only its folders inside the AppImage.
        let data_dirs = |value: &str| {
            outside_appimage(environment(&[
                ("APPIMAGE", "/home/me/TinyDash.AppImage"),
                ("APPDIR", appdir),
                ("XDG_DATA_DIRS", value),
            ]))
            .into_iter()
            .find(|(name, _)| name == "XDG_DATA_DIRS")
            .unwrap()
            .1
        };
        assert_eq!(
            data_dirs("/tmp/.mount_TinyDaAbC/usr/share:/usr/share:"),
            None
        );
        assert_eq!(
            data_dirs(
                "/tmp/.mount_TinyDaAbC/usr/share:/usr/share:/home/me/.local/share/flatpak/exports/share:/usr/local/share:/usr/share"
            )
            .as_deref(),
            Some("/home/me/.local/share/flatpak/exports/share:/usr/local/share:/usr/share")
        );
        assert_eq!(
            data_dirs("/tmp/.mount_TinyDaAbC/usr/share:/opt/share").as_deref(),
            Some("/opt/share")
        );
        // Outside an AppImage, or with an APPDIR that would match every
        // path, nothing changes.
        let plain = environment(&[("APPDIR", appdir), ("GDK_BACKEND", "x11")]);
        assert!(outside_appimage(plain).is_empty());
        for appdir in ["/", "relative", ""] {
            let odd = environment(&[
                ("APPIMAGE", "/a.AppImage"),
                ("APPDIR", appdir),
                ("PATH", "/bin"),
            ]);
            assert!(outside_appimage(odd).is_empty(), "{appdir}");
        }
    }

    /// A new, empty data folder for one test.
    #[cfg(unix)]
    fn data_folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("tinydash-terminal-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&folder).ok();
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[cfg(unix)]
    #[test]
    fn a_command_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let data = data_folder("private");
        let file = command_file(&data, "ls\n").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "ls\n");
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&file), 0o700);
        assert_eq!(mode(file.parent().unwrap()), 0o700);
        std::fs::remove_dir_all(&data).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn only_old_command_files_are_deleted() {
        let folder = data_folder("old-files");
        let now = std::time::SystemTime::now();
        let write = |name: &str, age_seconds: u64| {
            let path = folder.join(name);
            let file = std::fs::File::create(&path).unwrap();
            let age = std::time::Duration::from_secs(age_seconds);
            file.set_modified(now - age).unwrap();
            path
        };
        let old = write("1.command", 11 * 60);
        let new = write("2.command", 60);
        let other = write("notes.txt", 11 * 60);
        remove_old_command_files(&folder, now);
        assert!(!old.exists());
        assert!(new.exists() && other.exists());
        std::fs::remove_dir_all(&folder).unwrap();
    }

    /// Runs the Linux script in `sh` as a terminal would, with `echo` as the
    /// login shell, which prints `-l` where the user's shell would open.
    /// Windows has no `sh`.
    #[cfg(unix)]
    #[test]
    fn the_linux_script_reads_the_command_runs_it_then_opens_the_login_shell() {
        let data = data_folder("linux-script");
        let run = |command: &str| {
            let file = command_file(&data, &format!("{command}\n")).unwrap();
            let (_, arguments) =
                linux_terminal(None, |name| Some(PathBuf::from(name)), &file).unwrap();
            // Other users can read a program's arguments, so the command
            // stays in the file.
            assert!(
                !arguments
                    .iter()
                    .any(|argument| argument.to_string_lossy().contains(command)),
                "{command}"
            );
            let script = arguments
                .iter()
                .position(|argument| argument == "sh")
                .unwrap();
            let output = std::process::Command::new("sh")
                .args(&arguments[script + 1..])
                .env("SHELL", "echo")
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap();
            assert!(!file.exists(), "{command}");
            String::from_utf8(output.stdout).unwrap()
        };
        assert_eq!(run("echo '{clipboard}' \"$0\""), "{clipboard} sh\n-l\n");
        assert_eq!(run("cd / && pwd"), "/\n-l\n");
        // Spaces and backslashes stay as typed.
        assert_eq!(run("  printf '%s\\n' a\\ b"), "a b\n-l\n");
        // Exit and a syntax error still leave the shell open.
        assert_eq!(run("exit 3"), "-l\n");
        assert_eq!(run("echo it's"), "-l\n");
        std::fs::remove_dir_all(&data).unwrap();
    }
}
