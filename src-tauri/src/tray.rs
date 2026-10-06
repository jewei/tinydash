//! The tray (menu bar) icon and its menu.

use tauri::{
    AppHandle,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::{error::Result, platform, window};

const ID: &str = "tinydash";

pub fn set_visible(app: &AppHandle, visible: bool) -> Result<()> {
    match app.tray_by_id(ID) {
        Some(tray) => tray.set_visible(visible)?,
        None if visible => create(app)?,
        None => {}
    }
    Ok(())
}

fn create(app: &AppHandle) -> Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", "Open TinyDash", true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?,
            &MenuItem::with_id(app, "quit", "Quit TinyDash", true, None::<&str>)?,
        ],
    )?;
    // macOS tints a black template image for the menu bar; other desktops
    // show icons as drawn, so they get the colored app icon.
    let template = platform::TEMPLATE_TRAY_ICON;
    let icon = match app.default_window_icon() {
        Some(icon) if !template => icon.clone(),
        _ => template_icon(),
    };
    TrayIconBuilder::with_id(ID)
        .icon(icon)
        .icon_as_template(template)
        .tooltip("TinyDash")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let result = match event.id.as_ref() {
                "open" => window::show(app, None),
                "settings" => window::open_settings(app),
                "quit" => {
                    app.exit(0);
                    Ok(())
                }
                _ => Ok(()),
            };
            if let Err(error) = result {
                tracing::warn!(%error, "Tray action failed");
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
                && let Err(error) = window::show(tray.app_handle(), None)
            {
                tracing::warn!(%error, "Could not open the launcher");
            }
        })
        .build(app)?;
    Ok(())
}

/// Two short dashes, drawn as a 36 px template image so macOS tints it.
fn template_icon() -> Image<'static> {
    const SIZE: usize = 36;
    let mut rgba = vec![0_u8; SIZE * SIZE * 4];
    let dashes = [(6..26, 11..17), (12..32, 20..26)];
    for (xs, ys) in dashes {
        for y in ys {
            for x in xs.clone() {
                let pixel = (y * SIZE + x) * 4;
                rgba[pixel..pixel + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    Image::new_owned(rgba, SIZE as u32, SIZE as u32)
}
