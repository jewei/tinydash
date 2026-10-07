//! Runs the focus timer: a thread sleeps until the running phase ends,
//! then notifies the user and tells the launcher to show the next phase.

use std::{
    sync::{Condvar, Mutex, MutexGuard},
    time::Duration,
};

use tauri::{AppHandle, Manager};

use crate::{
    error::{Error, Result},
    events,
    features::focus::{Finished, FocusControl, FocusTimer, Lengths},
    platform,
    settings::Settings,
    state::State,
};

/// The longest wait between clock checks. Waits measure elapsed time, which
/// stops while the computer sleeps, so a phase that ends during sleep ends
/// this soon after waking.
const MAX_WAIT: Duration = Duration::from_secs(30);

pub struct FocusClock {
    timer: Mutex<FocusTimer>,
    /// Wakes the thread when the timer changes, so it waits for the new end.
    changed: Condvar,
}

impl FocusClock {
    pub fn new(settings: &Settings) -> Self {
        Self {
            timer: Mutex::new(FocusTimer::new(lengths(settings))),
            changed: Condvar::new(),
        }
    }

    pub fn get(&self) -> FocusTimer {
        self.lock().clone()
    }

    /// Follow changed settings. Turning the widget off resets the timer, so
    /// a timer nobody sees never notifies.
    pub fn apply(&self, old: &Settings, new: &Settings) {
        let mut timer = self.lock();
        if old.show_focus_timer && !new.show_focus_timer {
            *timer = FocusTimer::new(lengths(new));
        } else if lengths(old) != lengths(new) {
            timer.set_lengths(lengths(new));
        }
        self.changed.notify_all();
    }

    fn lock(&self) -> MutexGuard<'_, FocusTimer> {
        self.timer.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn lengths(settings: &Settings) -> Lengths {
    Lengths {
        focus_minutes: settings.focus_minutes,
        short_break_minutes: settings.short_break_minutes,
        long_break_minutes: settings.long_break_minutes,
        sessions: settings.sessions_before_long_break,
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Start, pause, skip, or reset, as the user asked.
pub fn control(app: &AppHandle, control: FocusControl) -> Result<()> {
    let state = app.state::<State>();
    if !state.settings.get().show_focus_timer {
        return Err(Error::msg(
            "The focus timer is off. Turn it on in Settings > Widgets.",
        ));
    }
    let lengths = lengths(&state.settings.get());
    state
        .focus
        .lock()
        .control(control, lengths, now_ms())
        .map_err(Error::Message)?;
    state.focus.changed.notify_all();
    events::widgets_changed(app);
    Ok(())
}

/// Start the thread that ends phases on time.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("focus timer".into())
        .spawn(move || run_forever(&app));
    if let Err(error) = spawned {
        tracing::warn!(%error, "The focus timer is unavailable");
    }
}

fn run_forever(app: &AppHandle) {
    let state = app.state::<State>();
    run(
        &state.focus,
        || lengths(&state.settings.get()),
        now_ms,
        |finished| {
            notify(app, &finished.title, &finished.body);
            events::widgets_changed(app);
        },
    );
}

/// End each phase on time and report it, forever.
fn run(
    clock: &FocusClock,
    lengths: impl Fn() -> Lengths,
    now_ms: impl Fn() -> i64,
    mut on_finish: impl FnMut(Finished),
) {
    let mut timer = clock.lock();
    loop {
        let now = now_ms();
        if let Some(finished) = timer.finish_if_due(lengths(), now) {
            drop(timer);
            on_finish(finished);
            timer = clock.lock();
            continue;
        }
        timer = match timer.ends_at_ms {
            Some(end) => {
                let wait =
                    Duration::from_millis(u64::try_from(end - now).unwrap_or(0)).min(MAX_WAIT);
                clock
                    .changed
                    .wait_timeout(timer, wait)
                    .unwrap_or_else(|e| e.into_inner())
                    .0
            }
            None => clock.changed.wait(timer).unwrap_or_else(|e| e.into_inner()),
        };
    }
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    if let Err(error) = platform::notify(app, title, body) {
        tracing::warn!(%error, "Could not show the focus timer notification");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::focus::TimerState;

    #[test]
    fn turning_the_widget_off_resets_a_running_timer() {
        let on = Settings {
            show_focus_timer: true,
            ..Settings::default()
        };
        let clock = FocusClock::new(&on);
        clock
            .lock()
            .control(FocusControl::Start, lengths(&on), 0)
            .unwrap();

        let longer = Settings {
            focus_minutes: 50,
            ..on.clone()
        };
        clock.apply(&on, &longer);
        assert_eq!(clock.get().state, TimerState::Running);

        let off = Settings {
            show_focus_timer: false,
            ..longer.clone()
        };
        clock.apply(&longer, &off);
        assert_eq!(clock.get(), FocusTimer::new(lengths(&off)));
    }

    #[test]
    fn the_thread_reports_a_focus_end_and_then_a_break_end() {
        use std::sync::{
            Arc,
            atomic::{AtomicI64, Ordering},
            mpsc,
        };
        let settings = Settings {
            show_focus_timer: true,
            focus_minutes: 1,
            short_break_minutes: 1,
            ..Settings::default()
        };
        let clock = Arc::new(FocusClock::new(&settings));
        let now = Arc::new(AtomicI64::new(0));
        let (sent, finished) = mpsc::channel();
        {
            let (clock, now, lengths) = (Arc::clone(&clock), Arc::clone(&now), lengths(&settings));
            std::thread::spawn(move || {
                run(
                    &clock,
                    || lengths,
                    || now.load(Ordering::SeqCst),
                    |done| {
                        sent.send(done.title).ok();
                    },
                );
            });
        }
        let control = |control, at: i64| {
            now.store(at, Ordering::SeqCst);
            clock
                .lock()
                .control(control, lengths(&settings), at)
                .unwrap();
            clock.changed.notify_all();
        };
        let wait = || finished.recv_timeout(Duration::from_secs(5)).unwrap();
        let pass_time = |at: i64| {
            now.store(at, Ordering::SeqCst);
            clock.changed.notify_all();
        };

        control(FocusControl::Start, 0);
        pass_time(60_000);
        assert_eq!(wait(), "Focus session done");
        control(FocusControl::Start, 70_000);
        pass_time(130_000);
        assert_eq!(wait(), "Break is over");
    }
}
