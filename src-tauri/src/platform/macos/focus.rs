use std::sync::Mutex;

use objc2::rc::Retained;
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

#[derive(Default)]
pub struct LauncherFocus(Mutex<Option<PreviousApplication>>);

impl LauncherFocus {
    pub fn remember(&self) {
        let frontmost = NSWorkspace::sharedWorkspace().frontmostApplication();
        // Reopening an already focused palette must keep the original app.
        if let Some(app) = frontmost
            && app != NSRunningApplication::currentApplication()
        {
            *self.0.lock().unwrap_or_else(|error| error.into_inner()) =
                Some(PreviousApplication(app));
        }
    }

    pub fn take(&self) -> Option<PreviousApplication> {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }
}

pub struct PreviousApplication(Retained<NSRunningApplication>);

impl PreviousApplication {
    pub fn restore(self) {
        // A user click or an action can activate another app before hide finishes.
        // Only return focus while TinyDash is still the active application.
        if self.0.isTerminated()
            || !NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .is_some_and(|app| app == NSRunningApplication::currentApplication())
        {
            return;
        }
        // Empty options restore the last key window without raising every window
        // or forcing activation over another application.
        if !self
            .0
            .activateWithOptions(NSApplicationActivationOptions::empty())
        {
            tracing::debug!("Could not restore the previous application");
        }
    }
}
