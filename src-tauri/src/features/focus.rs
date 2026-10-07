//! The focus timer: Pomodoro sessions with short breaks, and a long break
//! after the last session of a cycle. Times are Unix milliseconds that the
//! caller passes in, so the logic never waits and tests need no clock.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::search::result::{Action, ResultAction};

const MINUTE_MS: i64 = 60 * 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Phase {
    Focus,
    ShortBreak,
    LongBreak,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TimerState {
    /// Ready to start a focus session.
    Idle,
    Running,
    Paused,
    /// A focus session ended; its break waits for the user.
    Done,
}

/// What the user asks of the timer. Each is checked against the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FocusControl {
    /// Start focus, resume after a pause, or start the break that is due.
    Start,
    Pause,
    /// End a break, or skip the break that is due, and get ready for the
    /// next session.
    SkipBreak,
    /// Start the cycle over: session 1, not running.
    Reset,
}

/// Phase lengths and the sessions in a cycle, from the settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lengths {
    pub focus_minutes: u16,
    pub short_break_minutes: u16,
    pub long_break_minutes: u16,
    pub sessions: u8,
}

/// A phase that ended on its own, for the notification.
#[derive(Debug, PartialEq, Eq)]
pub struct Finished {
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FocusTimer {
    pub phase: Phase,
    pub state: TimerState,
    /// Time left when not running.
    pub remaining_ms: i64,
    /// While running, when the phase ends; the launcher counts down to it.
    pub ends_at_ms: Option<i64>,
    /// The length of the phase, for the progress ring.
    pub total_ms: i64,
    /// The session of the cycle, from 1.
    pub session: u8,
    pub sessions: u8,
    /// What the user can do now; the first is the main one.
    pub actions: Vec<ResultAction>,
}

impl FocusTimer {
    pub fn new(lengths: Lengths) -> Self {
        let mut timer = Self {
            phase: Phase::Focus,
            state: TimerState::Idle,
            remaining_ms: 0,
            ends_at_ms: None,
            total_ms: 0,
            session: 1,
            sessions: lengths.sessions,
            actions: Vec::new(),
        };
        timer.ready(Phase::Focus, lengths);
        timer
    }

    /// New lengths from the settings. A phase on its way keeps its length;
    /// the new ones count from the next phase.
    pub fn set_lengths(&mut self, lengths: Lengths) {
        self.sessions = lengths.sessions;
        self.session = self.session.min(lengths.sessions);
        if self.state == TimerState::Idle {
            self.ready(Phase::Focus, lengths);
        } else {
            self.actions = self.available();
        }
    }

    pub fn control(
        &mut self,
        control: FocusControl,
        lengths: Lengths,
        now: i64,
    ) -> Result<(), String> {
        match (control, self.state) {
            (FocusControl::Start, TimerState::Idle | TimerState::Paused) => {
                self.state = TimerState::Running;
                self.ends_at_ms = Some(now + self.remaining_ms);
            }
            (FocusControl::Start, TimerState::Done) => {
                let phase = if self.session >= self.sessions {
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                };
                self.ready(phase, lengths);
                self.state = TimerState::Running;
                self.ends_at_ms = Some(now + self.remaining_ms);
            }
            (FocusControl::Pause, TimerState::Running) => {
                self.remaining_ms = self.left(now);
                self.state = TimerState::Paused;
                self.ends_at_ms = None;
            }
            (FocusControl::SkipBreak, _) if self.on_break() || self.state == TimerState::Done => {
                self.next_session(lengths);
            }
            (FocusControl::Reset, _) => {
                *self = Self::new(lengths);
                return Ok(());
            }
            _ => return Err("The focus timer changed. Try again.".into()),
        }
        self.actions = self.available();
        Ok(())
    }

    /// End the phase if its time is up: a focus session waits for its break,
    /// and a break readies the next session.
    pub fn finish_if_due(&mut self, lengths: Lengths, now: i64) -> Option<Finished> {
        if self.state != TimerState::Running || self.left(now) > 0 {
            return None;
        }
        if self.phase == Phase::Focus {
            self.state = TimerState::Done;
            self.remaining_ms = 0;
            self.ends_at_ms = None;
            self.actions = self.available();
            let (minutes, kind) = if self.session >= self.sessions {
                (lengths.long_break_minutes, "long break")
            } else {
                (lengths.short_break_minutes, "break")
            };
            return Some(Finished {
                title: "Focus session done".into(),
                body: format!("Time for a {minutes}-minute {kind}."),
            });
        }
        self.next_session(lengths);
        Some(Finished {
            title: "Break is over".into(),
            body: format!(
                "Ready for focus session {} of {}?",
                self.session, self.sessions
            ),
        })
    }

    fn on_break(&self) -> bool {
        self.phase != Phase::Focus
    }

    fn left(&self, now: i64) -> i64 {
        self.ends_at_ms
            .map_or(self.remaining_ms, |end| (end - now).max(0))
    }

    fn next_session(&mut self, lengths: Lengths) {
        self.session = if self.phase == Phase::LongBreak || self.session >= self.sessions {
            1
        } else {
            self.session + 1
        };
        self.ready(Phase::Focus, lengths);
    }

    /// Stop at the start of `phase`, at its full length.
    fn ready(&mut self, phase: Phase, lengths: Lengths) {
        let minutes = match phase {
            Phase::Focus => lengths.focus_minutes,
            Phase::ShortBreak => lengths.short_break_minutes,
            Phase::LongBreak => lengths.long_break_minutes,
        };
        self.phase = phase;
        self.state = TimerState::Idle;
        self.total_ms = i64::from(minutes) * MINUTE_MS;
        self.remaining_ms = self.total_ms;
        self.ends_at_ms = None;
        self.actions = self.available();
    }

    fn available(&self) -> Vec<ResultAction> {
        let action = |label: &str, control| ResultAction::new(label, Action::Focus { control });
        let mut actions = match self.state {
            TimerState::Idle => vec![action("Start Focus", FocusControl::Start)],
            TimerState::Running => vec![action("Pause Timer", FocusControl::Pause)],
            TimerState::Paused => vec![action("Resume Timer", FocusControl::Start)],
            TimerState::Done => vec![action("Start Break", FocusControl::Start)],
        };
        if self.on_break() || self.state == TimerState::Done {
            actions.push(action("Skip Break", FocusControl::SkipBreak));
        }
        let fresh = self.state == TimerState::Idle && self.session == 1;
        if !fresh {
            actions.push(action("Reset Timer", FocusControl::Reset));
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENGTHS: Lengths = Lengths {
        focus_minutes: 25,
        short_break_minutes: 5,
        long_break_minutes: 15,
        sessions: 2,
    };

    fn labels(timer: &FocusTimer) -> Vec<&str> {
        timer.actions.iter().map(|a| a.label.as_str()).collect()
    }

    fn start(timer: &mut FocusTimer, now: i64) {
        timer.control(FocusControl::Start, LENGTHS, now).unwrap();
    }

    #[test]
    fn runs_a_cycle_of_sessions_and_breaks() {
        let mut timer = FocusTimer::new(LENGTHS);
        assert_eq!(labels(&timer), ["Start Focus"]);
        start(&mut timer, 0);
        assert_eq!(timer.ends_at_ms, Some(25 * MINUTE_MS));
        assert_eq!(timer.finish_if_due(LENGTHS, 25 * MINUTE_MS - 1), None);

        let done = timer.finish_if_due(LENGTHS, 25 * MINUTE_MS).unwrap();
        assert_eq!(done.body, "Time for a 5-minute break.");
        assert_eq!(timer.state, TimerState::Done);
        assert_eq!(labels(&timer), ["Start Break", "Skip Break", "Reset Timer"]);

        start(&mut timer, 30 * MINUTE_MS);
        assert_eq!(
            (timer.phase, timer.total_ms),
            (Phase::ShortBreak, 5 * MINUTE_MS)
        );
        let over = timer.finish_if_due(LENGTHS, 35 * MINUTE_MS).unwrap();
        assert_eq!(over.body, "Ready for focus session 2 of 2?");
        assert_eq!(
            (timer.phase, timer.state, timer.session),
            (Phase::Focus, TimerState::Idle, 2)
        );

        // The last session of the cycle earns the long break, then session 1.
        start(&mut timer, 0);
        let done = timer.finish_if_due(LENGTHS, 25 * MINUTE_MS).unwrap();
        assert_eq!(done.body, "Time for a 15-minute long break.");
        start(&mut timer, 25 * MINUTE_MS);
        assert_eq!(timer.phase, Phase::LongBreak);
        timer.finish_if_due(LENGTHS, 40 * MINUTE_MS).unwrap();
        assert_eq!(timer.session, 1);
    }

    #[test]
    fn pauses_resumes_skips_and_resets() {
        let mut timer = FocusTimer::new(LENGTHS);
        start(&mut timer, 0);
        timer
            .control(FocusControl::Pause, LENGTHS, 10 * MINUTE_MS)
            .unwrap();
        assert_eq!(timer.remaining_ms, 15 * MINUTE_MS);
        assert_eq!(labels(&timer), ["Resume Timer", "Reset Timer"]);
        // A paused timer never finishes, however long it waits.
        assert_eq!(timer.finish_if_due(LENGTHS, 99 * MINUTE_MS), None);
        start(&mut timer, 20 * MINUTE_MS);
        assert_eq!(timer.ends_at_ms, Some(35 * MINUTE_MS));

        assert!(timer.control(FocusControl::SkipBreak, LENGTHS, 0).is_err());
        timer.finish_if_due(LENGTHS, 35 * MINUTE_MS).unwrap();
        timer.control(FocusControl::SkipBreak, LENGTHS, 0).unwrap();
        assert_eq!((timer.phase, timer.session), (Phase::Focus, 2));

        timer.control(FocusControl::Reset, LENGTHS, 0).unwrap();
        assert_eq!(timer, FocusTimer::new(LENGTHS));
        assert!(timer.control(FocusControl::Pause, LENGTHS, 0).is_err());
    }

    #[test]
    fn new_lengths_wait_for_the_next_phase() {
        let longer = Lengths {
            focus_minutes: 50,
            sessions: 1,
            ..LENGTHS
        };
        let mut timer = FocusTimer::new(LENGTHS);
        timer.set_lengths(longer);
        assert_eq!((timer.remaining_ms, timer.sessions), (50 * MINUTE_MS, 1));

        let mut running = FocusTimer::new(LENGTHS);
        start(&mut running, 0);
        running.set_lengths(longer);
        assert_eq!(running.ends_at_ms, Some(25 * MINUTE_MS));
    }
}
