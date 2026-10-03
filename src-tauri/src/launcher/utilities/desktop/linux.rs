use super::*;

#[derive(Clone)]
pub struct WindowTarget {
    id: u64,
    identity: super::super::process::ProcessInfo,
    original: Rect,
}
impl WindowTarget {
    pub fn label(&self) -> String {
        format!("{} (window {})", self.identity.name, self.id)
    }
    pub fn same_window(&self, other: &Self) -> bool {
        self.id == other.id && self.identity == other.identity
    }
    pub fn is_focused(&self) -> UtilityResult<bool> {
        x11()?;
        let id: u64 = run("xdotool", &["getactivewindow"])?
            .trim()
            .parse()
            .map_err(|_| "Invalid active window")?;
        Ok(id == self.id)
    }
}
fn x11() -> UtilityResult<()> {
    effects_allowed()?;
    if std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v == "wayland")
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
    {
        return Err(
            "Window management is unsupported on Wayland; no compositor-specific commands are sent"
                .into(),
        );
    }
    if std::env::var_os("DISPLAY").is_none() {
        return Err("An X11 display is required".into());
    }
    Ok(())
}
pub fn capture_window() -> UtilityResult<WindowTarget> {
    x11()?;
    let id: u64 = run("xdotool", &["getactivewindow"])?
        .trim()
        .parse()
        .map_err(|_| "Invalid active window")?;
    capture_target(id)
}
pub fn capture_target(id: u64) -> UtilityResult<WindowTarget> {
    x11()?;
    let pid: u32 = run("xdotool", &["getwindowpid", &id.to_string()])?
        .trim()
        .parse()
        .map_err(|_| "Window has no process identity")?;
    let identity = super::super::process::list()?
        .into_iter()
        .find(|p| p.pid == pid)
        .ok_or("Focus a non-system application other than TinyDash, then capture it")?;
    let geometry = run(
        "xdotool",
        &["getwindowgeometry", "--shell", &id.to_string()],
    )?;
    let value = |key: &str| -> UtilityResult<i32> {
        geometry
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .and_then(|n| n.parse().ok())
            .ok_or("Invalid window geometry".into())
    };
    Ok(WindowTarget {
        id,
        identity,
        original: Rect {
            x: value("X=")?,
            y: value("Y=")?,
            width: value("WIDTH=")?,
            height: value("HEIGHT=")?,
        },
    })
}
pub fn window(
    target: &WindowTarget,
    action: WindowAction,
    require_focus: bool,
) -> UtilityResult<()> {
    x11()?;
    super::super::process::validate(&target.identity)?;
    let id = target.id.to_string();
    let pid: u32 = run("xdotool", &["getwindowpid", &id])?
        .trim()
        .parse()
        .map_err(|_| "Target window no longer exists")?;
    if pid != target.identity.pid {
        return Err("Target window identity changed".into());
    }
    let desktop = run("xprop", &["-root", "_NET_CURRENT_DESKTOP"])?;
    let index: usize = desktop
        .split_once('=')
        .and_then(|(_, v)| v.trim().parse().ok())
        .ok_or("Desktop work area unavailable")?;
    let work = run("xprop", &["-root", "_NET_WORKAREA"])?;
    let values: Vec<i32> = work
        .split_once('=')
        .ok_or("Desktop work area unavailable")?
        .1
        .split(',')
        .map(|v| v.trim().parse())
        .collect::<Result<_, _>>()
        .map_err(|_| "Invalid desktop work area")?;
    let offset = index.checked_mul(4).ok_or("Invalid desktop index")?;
    let area = values
        .get(offset..offset + 4)
        .ok_or("Desktop work area unavailable")?;
    let rect = super::placement(
        target.original,
        Rect {
            x: area[0],
            y: area[1],
            width: area[2],
            height: area[3],
        },
        action,
    );
    let hex = format!("0x{:x}", target.id);
    if require_focus && !target.is_focused()? {
        return Err("Focus changed. No window placement was requested.".into());
    }
    run(
        "wmctrl",
        &["-ir", &hex, "-b", "remove,maximized_vert,maximized_horz"],
    )?;
    run(
        "wmctrl",
        &[
            "-ir",
            &hex,
            "-e",
            &format!("0,{},{},{},{}", rect.x, rect.y, rect.width, rect.height),
        ],
    )
    .map(|_| ())
}
pub fn media(action: MediaAction) -> UtilityResult<()> {
    match action {
        MediaAction::PlayPause => run("playerctl", &["play-pause"]),
        MediaAction::Next => run("playerctl", &["next"]),
        MediaAction::Previous => run("playerctl", &["previous"]),
        MediaAction::VolumeUp => run(
            "wpctl",
            &["set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", "5%+"],
        ),
        MediaAction::VolumeDown => run("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", "5%-"]),
        MediaAction::Mute => run("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"]),
    }
    .map(|_| ())
}
