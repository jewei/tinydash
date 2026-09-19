use std::sync::{Mutex, atomic::Ordering};

use tauri::{AppHandle, Manager};

use super::{LauncherState, query::SearchMode};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LaunchRequest {
    #[default]
    Launcher,
    Category(SearchMode),
    Settings,
    Background,
}

pub struct Startup(pub Mutex<LaunchRequest>);

impl LaunchRequest {
    pub fn parse(args: impl IntoIterator<Item = String>) -> anyhow::Result<Self> {
        let mut args = args.into_iter();
        let mut request = None;
        while let Some(arg) = args.next() {
            // Launch Services can supply its process serial number on macOS.
            if cfg!(target_os = "macos") && arg.starts_with("-psn_") {
                continue;
            }
            anyhow::ensure!(
                request.is_none(),
                "Use only one of --settings, --background, or --mode CATEGORY."
            );
            request = Some(match arg.as_str() {
                "--settings" => Self::Settings,
                "--background" => Self::Background,
                "--mode" => {
                    let mode = args
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("Supply a category after --mode."))?;
                    Self::Category(serde_json::from_value(serde_json::Value::String(mode))
                        .map_err(|_| anyhow::anyhow!("Unknown category. Use all, apps, files, clipboard, calculator, system, emoji, password, timezone, url, or web."))?)
                }
                _ => anyhow::bail!("Unknown option: {arg}. Use --help for supported options."),
            });
        }
        Ok(request.unwrap_or_default())
    }

    pub fn mode(self) -> Option<SearchMode> {
        match self {
            Self::Category(mode) => Some(mode),
            _ => None,
        }
    }

    pub async fn open(self, app: AppHandle) -> Result<(), String> {
        match self {
            Self::Background => Ok(()),
            Self::Settings => super::preferences::open_settings(app).await,
            Self::Category(mode) => {
                super::window::show_category(&app, mode).map_err(|e| e.to_string())
            }
            Self::Launcher => super::window::show(&app).map_err(|e| e.to_string()),
        }
    }
}

pub fn activate(app: &AppHandle, request: LaunchRequest) {
    // An OS login task must not open an already running launcher.
    if request == LaunchRequest::Background {
        return;
    }
    if !app
        .try_state::<LauncherState>()
        .is_some_and(|state| state.ready.load(Ordering::Acquire))
    {
        if let Ok(mut initial) = app.state::<Startup>().0.lock() {
            *initial = request;
        }
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = request.open(app).await {
            tracing::warn!(%error, "Could not open TinyDash");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> anyhow::Result<LaunchRequest> {
        LaunchRequest::parse(args.iter().map(|arg| (*arg).into()))
    }

    #[test]
    fn accepts_one_explicit_route_and_rejects_incomplete_or_conflicting_requests() {
        assert_eq!(parse(&[]).unwrap(), LaunchRequest::Launcher);
        assert_eq!(
            parse(&["--mode", "clipboard"]).unwrap(),
            LaunchRequest::Category(SearchMode::Clipboard)
        );
        assert_eq!(parse(&["--background"]).unwrap(), LaunchRequest::Background);
        assert_eq!(parse(&["--settings"]).unwrap(), LaunchRequest::Settings);
        for args in [
            vec!["--mode"],
            vec!["--mode", "unknown"],
            vec!["--background", "--settings"],
            vec!["--mode", "apps", "--mode", "files"],
            vec!["clipboard"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
}
