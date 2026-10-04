//! Command-line arguments. A second `tinydash` process forwards its
//! arguments to the running one, so these also drive the running app.

use crate::search::Category;

pub const USAGE: &str = "\
Usage: tinydash [--settings | --background | --mode <category>]

  (no option)        Show the launcher, or toggle it if TinyDash is running
  --settings         Open Settings
  --background       Start without showing a window (used at login)
  --mode <category>  Show the launcher in a category with an empty query:
                     all, apps, files, clipboard, snippets, emoji, system
  --help             Print this help";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Launch {
    #[default]
    Launcher,
    Settings,
    Background,
    Category(Category),
}

pub fn parse<S: AsRef<str>>(args: &[S]) -> Result<Launch, String> {
    match args {
        [] => Ok(Launch::Launcher),
        [flag] if flag.as_ref() == "--settings" => Ok(Launch::Settings),
        [flag] if flag.as_ref() == "--background" => Ok(Launch::Background),
        [flag, name] if flag.as_ref() == "--mode" => Category::ALL
            .into_iter()
            .find(|category| category.name() == name.as_ref())
            .map(Launch::Category)
            .ok_or_else(|| format!("Unknown category “{}”.\n\n{USAGE}", name.as_ref())),
        _ => Err(USAGE.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_form() {
        assert_eq!(parse::<&str>(&[]), Ok(Launch::Launcher));
        assert_eq!(parse(&["--settings"]), Ok(Launch::Settings));
        assert_eq!(parse(&["--background"]), Ok(Launch::Background));
        assert_eq!(
            parse(&["--mode", "clipboard"]),
            Ok(Launch::Category(Category::Clipboard))
        );
        assert!(parse(&["--mode", "nope"]).is_err());
        assert!(parse(&["--settings", "--background"]).is_err());
    }
}
