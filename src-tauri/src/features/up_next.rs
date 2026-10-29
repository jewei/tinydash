//! The Up Next widget: the rest of today's events and the reminders due by
//! the end of today, from the OS calendars. The platform reads them
//! (EventKit on macOS); this module picks, orders, and labels them. Times
//! are Unix milliseconds, and the caller passes the time now, so tests need
//! no clock.

// Only the macOS platform reads calendars, so elsewhere nothing builds what
// it reads. Clippy on macOS still finds dead code here.
#![cfg_attr(
    not(target_os = "macos"),
    allow(dead_code, reason = "only the macOS platform reads calendars")
)]

use std::fmt::Display;

use chrono::{DateTime, NaiveDate, NaiveTime, TimeDelta, TimeZone};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::search::result::{Action, ResultAction};

/// The most events the widget shows.
const MAX_EVENTS: usize = 6;
/// The most reminders the widget shows.
const MAX_REMINDERS: usize = 6;
/// The most items of each kind the platform hands over, so a long list of
/// overdue reminders stays bounded before it is sorted.
pub const MAX_READ: usize = 500;
const MAX_TITLE_CHARS: usize = 80;
const MAX_SOURCE_CHARS: usize = 40;

/// An event as the platform reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub title: String,
    /// The name of its calendar.
    pub calendar: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub all_day: bool,
    /// The organizer canceled it, or the user declined it, so it is not on
    /// the user's day.
    pub declined: bool,
}

/// An incomplete reminder with a due date, as the platform reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Reminder {
    pub title: String,
    /// The name of its list.
    pub list: String,
    pub due: Due,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Due {
    /// A day without a time.
    Day(NaiveDate),
    /// A moment, in Unix milliseconds.
    At(i64),
}

/// Why one kind of item could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unreadable {
    /// The user has not answered the OS yet.
    NotAsked,
    /// The user said no, or allowed adding events only.
    Denied,
    /// A policy, such as Screen Time, blocks access.
    Restricted,
    /// Another app, such as a terminal, started TinyDash, so the OS answers
    /// for that app: TinyDash would read and ask with that app's access.
    StartedByAnotherApp,
    /// This build cannot ask: it lacks the usage strings, without which
    /// macOS ends an app that asks.
    CannotAsk,
    /// The OS did not answer; the message says why.
    Failed(String),
}

/// What the platform read: each kind, or why it could not.
#[derive(Debug)]
pub struct Agenda {
    pub events: Result<Vec<Event>, Unreadable>,
    pub reminders: Result<Vec<Reminder>, Unreadable>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UpNextKind {
    Event,
    Reminder,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpNextItem {
    pub kind: UpNextKind,
    pub title: String,
    /// When, in words: "All day", "Now", "10:30–11:00", "Today", "16:00",
    /// or "Overdue".
    pub time: String,
    /// The calendar or the reminder list.
    pub source: String,
    /// Opens Calendar for an event, Reminders for a reminder.
    pub action: ResultAction,
}

/// What the Up Next widget shows.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpNext {
    /// The events, then the reminders.
    pub items: Vec<UpNextItem>,
    /// Why a kind of item cannot show, and how to fix it.
    pub problems: Vec<String>,
    /// Asks the OS for access, while the user has not answered.
    pub ask: Option<ResultAction>,
    /// When the list changes as time passes: an event starts or ends, a
    /// reminder falls due, or the day ends. The pane loads again then.
    pub changes_at_ms: i64,
}

/// The time to read, in Unix milliseconds: from now to the end of today.
pub fn window<Tz: TimeZone>(now: &DateTime<Tz>) -> (i64, i64) {
    (now.timestamp_millis(), end_of_today(now))
}

/// Pick, order, and label what the platform read.
pub fn view<Tz: TimeZone>(agenda: Agenda, now: &DateTime<Tz>) -> UpNext
where
    Tz::Offset: Display,
{
    let zone = now.timezone();
    let now_ms = now.timestamp_millis();
    let end_ms = end_of_today(now);
    let today = now.date_naive();
    let mut changes = vec![end_ms];
    let mut items = Vec::new();

    let (mut events, events_problem) = split(agenda.events);
    events.retain(|event| !event.declined && event.end_ms > now_ms && event.start_ms < end_ms);
    events.sort_by(|a, b| {
        b.all_day
            .cmp(&a.all_day)
            .then(a.start_ms.cmp(&b.start_ms))
            .then(a.end_ms.cmp(&b.end_ms))
            .then_with(|| a.title.cmp(&b.title))
    });
    events.truncate(MAX_EVENTS);
    for event in events {
        let time = if event.all_day {
            "All day".to_owned()
        } else {
            changes.push(event.end_ms);
            if event.start_ms <= now_ms {
                "Now".to_owned()
            } else {
                changes.push(event.start_ms);
                span(&zone, event.start_ms, event.end_ms)
            }
        };
        items.push(item(UpNextKind::Event, &event.title, time, &event.calendar));
    }

    let (mut reminders, reminders_problem) = split(agenda.reminders);
    reminders.retain(|reminder| match reminder.due {
        Due::Day(day) => day <= today,
        Due::At(ms) => ms < end_ms,
    });
    let overdue = |due: Due| match due {
        Due::Day(day) => day < today,
        Due::At(ms) => ms <= now_ms,
    };
    // Overdue ones first, so a reminder due today without a time, which
    // sorts at midnight, does not split them.
    reminders.sort_by_cached_key(|reminder| {
        let due = match reminder.due {
            Due::Day(day) => start_of_day(&zone, day),
            Due::At(ms) => ms,
        };
        (!overdue(reminder.due), due, reminder.title.clone())
    });
    // A reminder that falls due moves up among the overdue ones, so one
    // that the limit leaves out can come in then.
    changes.extend(
        reminders
            .iter()
            .filter_map(|reminder| match reminder.due {
                Due::At(ms) if ms > now_ms => Some(ms),
                _ => None,
            })
            .min(),
    );
    reminders.truncate(MAX_REMINDERS);
    for reminder in reminders {
        let time = match reminder.due {
            due if overdue(due) => "Overdue".to_owned(),
            Due::Day(_) => "Today".to_owned(),
            Due::At(ms) => clock(&zone, ms),
        };
        items.push(item(
            UpNextKind::Reminder,
            &reminder.title,
            time,
            &reminder.list,
        ));
    }

    let not_asked = [&events_problem, &reminders_problem]
        .into_iter()
        .any(|problem| matches!(problem, Some(Unreadable::NotAsked)));
    UpNext {
        items,
        problems: problems(events_problem, reminders_problem),
        ask: not_asked.then(|| ResultAction::new("Allow Access", Action::AskCalendarAccess)),
        changes_at_ms: changes.into_iter().min().unwrap_or(end_ms),
    }
}

/// The action that opens the app of an item.
fn open_action(kind: UpNextKind) -> ResultAction {
    let label = match kind {
        UpNextKind::Event => "Open Calendar",
        UpNextKind::Reminder => "Open Reminders",
    };
    ResultAction::new(label, Action::OpenUpNext { kind })
}

fn item(kind: UpNextKind, title: &str, time: String, source: &str) -> UpNextItem {
    let title = cut(title, MAX_TITLE_CHARS);
    UpNextItem {
        kind,
        title: if title.is_empty() {
            "Untitled".to_owned()
        } else {
            title
        },
        time,
        source: cut(source, MAX_SOURCE_CHARS),
        action: open_action(kind),
    }
}

/// `text` on one line, with single spaces, cut to `max` characters.
fn cut(text: &str, max: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max {
        return text;
    }
    let mut cut: String = text.chars().take(max - 1).collect();
    cut.push('…');
    cut
}

fn split<T>(read: Result<Vec<T>, Unreadable>) -> (Vec<T>, Option<Unreadable>) {
    match read {
        Ok(items) => (items, None),
        Err(problem) => (Vec::new(), Some(problem)),
    }
}

/// What a problem is about, in words.
#[derive(Clone, Copy)]
enum Subject {
    Calendars,
    Reminders,
    Both,
}

impl Subject {
    fn words(self) -> &'static str {
        match self {
            Self::Calendars => "calendars",
            Self::Reminders => "reminders",
            Self::Both => "calendars and reminders",
        }
    }

    /// Where the privacy settings list TinyDash for them. macOS 12 calls
    /// those settings System Preferences > Security & Privacy, and later
    /// versions System Settings > Privacy & Security, so the text names
    /// neither path.
    fn place(self) -> &'static str {
        match self {
            Self::Calendars => "under Calendars",
            Self::Reminders => "under Reminders",
            Self::Both => "under Calendars and under Reminders",
        }
    }
}

/// One line per problem; the same problem for both kinds is one line.
fn problems(events: Option<Unreadable>, reminders: Option<Unreadable>) -> Vec<String> {
    match (events, reminders) {
        (Some(events), Some(reminders)) if events == reminders => {
            vec![message(&events, Subject::Both)]
        }
        (events, reminders) => [
            events.map(|problem| message(&problem, Subject::Calendars)),
            reminders.map(|problem| message(&problem, Subject::Reminders)),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

fn message(problem: &Unreadable, subject: Subject) -> String {
    let what = subject.words();
    match problem {
        Unreadable::NotAsked => format!("TinyDash needs your permission to read your {what}."),
        Unreadable::Denied => format!(
            "TinyDash cannot read your {what}. In the privacy settings of macOS, allow TinyDash {}.",
            subject.place()
        ),
        Unreadable::Restricted => {
            format!("Access to your {what} is restricted on this Mac, for example by Screen Time.")
        }
        Unreadable::StartedByAnotherApp => format!(
            "TinyDash cannot read your {what} when another app, such as a terminal, starts it. Quit TinyDash, then open it from Finder or Launchpad."
        ),
        Unreadable::CannotAsk => {
            format!("This build of TinyDash cannot ask to read your {what}. Use the installed app.")
        }
        Unreadable::Failed(reason) => format!("Could not read your {what}. {reason}"),
    }
}

/// "10:30–11:00", or one time for an event with no length.
fn span<Tz: TimeZone>(zone: &Tz, start_ms: i64, end_ms: i64) -> String
where
    Tz::Offset: Display,
{
    if end_ms <= start_ms {
        return clock(zone, start_ms);
    }
    format!("{}–{}", clock(zone, start_ms), clock(zone, end_ms))
}

fn clock<Tz: TimeZone>(zone: &Tz, ms: i64) -> String
where
    Tz::Offset: Display,
{
    zone.timestamp_millis_opt(ms)
        .single()
        .map(|time| time.format("%H:%M").to_string())
        .unwrap_or_default()
}

/// The first moment of tomorrow, in Unix milliseconds.
fn end_of_today<Tz: TimeZone>(now: &DateTime<Tz>) -> i64 {
    let today = now.date_naive();
    start_of_day(&now.timezone(), today.succ_opt().unwrap_or(today))
}

/// The first moment of `day` in `zone`. Where a clock change skips
/// midnight, the day starts at the first hour that exists.
fn start_of_day<Tz: TimeZone>(zone: &Tz, day: NaiveDate) -> i64 {
    let midnight = day.and_time(NaiveTime::MIN);
    (0..=2)
        .find_map(|hours| {
            zone.from_local_datetime(&(midnight + TimeDelta::hours(hours)))
                .earliest()
        })
        .map_or_else(
            || midnight.and_utc().timestamp_millis(),
            |start| start.timestamp_millis(),
        )
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;

    const HOUR: i64 = 3_600_000;

    /// 14:00 on 2026-03-10 in UTC+8.
    fn now() -> DateTime<FixedOffset> {
        FixedOffset::east_opt(8 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 3, 10, 14, 0, 0)
            .unwrap()
    }

    /// Unix milliseconds of a time on the day of `now`.
    fn at(hour: u32, minute: u32) -> i64 {
        now()
            .date_naive()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
            .and_local_timezone(now().timezone())
            .unwrap()
            .timestamp_millis()
    }

    fn event(title: &str, start_ms: i64, end_ms: i64) -> Event {
        Event {
            title: title.into(),
            calendar: "Work".into(),
            start_ms,
            end_ms,
            all_day: false,
            declined: false,
        }
    }

    fn all_day(title: &str) -> Event {
        Event {
            all_day: true,
            ..event(title, at(0, 0), at(23, 59))
        }
    }

    fn reminder(title: &str, due: Due) -> Reminder {
        Reminder {
            title: title.into(),
            list: "Home".into(),
            due,
        }
    }

    fn agenda(events: Vec<Event>, reminders: Vec<Reminder>) -> Agenda {
        Agenda {
            events: Ok(events),
            reminders: Ok(reminders),
        }
    }

    fn rows(view: &UpNext) -> Vec<(String, String)> {
        view.items
            .iter()
            .map(|item| (item.time.clone(), item.title.clone()))
            .collect()
    }

    fn row(time: &str, title: &str) -> (String, String) {
        (time.into(), title.into())
    }

    #[test]
    fn reads_from_now_to_midnight() {
        assert_eq!(
            window(&now()),
            (now().timestamp_millis(), at(0, 0) + 24 * HOUR)
        );
    }

    #[test]
    fn lists_all_day_events_first_then_the_rest_of_today() {
        let view = view(
            agenda(
                vec![
                    event("Lunch", at(12, 0), at(13, 0)),
                    event("Review", at(16, 30), at(17, 0)),
                    event("Standup", at(13, 30), at(14, 30)),
                    all_day("Holiday"),
                    event("Flight", at(15, 0), at(15, 0)),
                ],
                Vec::new(),
            ),
            &now(),
        );
        assert_eq!(
            rows(&view),
            [
                row("All day", "Holiday"),
                row("Now", "Standup"),
                row("15:00", "Flight"),
                row("16:30–17:00", "Review"),
            ]
        );
        let first = &view.items[0];
        assert_eq!(first.kind, UpNextKind::Event);
        assert_eq!(first.source, "Work");
        assert_eq!(
            first.action,
            ResultAction::new(
                "Open Calendar",
                Action::OpenUpNext {
                    kind: UpNextKind::Event
                }
            )
        );
        assert!(view.problems.is_empty() && view.ask.is_none());
    }

    #[test]
    fn shows_at_most_six_events_and_six_reminders() {
        let events = (0..9)
            .map(|n| event(&format!("Event {n}"), at(15, n), at(16, 0)))
            .collect();
        let reminders = (0..9)
            .map(|n| reminder(&format!("Task {n}"), Due::At(at(18, n))))
            .collect();
        let view = view(agenda(events, reminders), &now());
        let count = |kind| view.items.iter().filter(|item| item.kind == kind).count();
        assert_eq!(count(UpNextKind::Event), MAX_EVENTS);
        assert_eq!(count(UpNextKind::Reminder), MAX_REMINDERS);
        assert_eq!(view.items[5].title, "Event 5");
        assert_eq!(view.items[11].title, "Task 5");
    }

    #[test]
    fn leaves_out_canceled_and_declined_events() {
        let declined = |title: &str| Event {
            declined: true,
            ..event(title, at(14, 30), at(15, 0))
        };
        // Enough of them to fill the list, were they kept.
        let mut events: Vec<Event> = (0..MAX_EVENTS)
            .map(|n| declined(&format!("Skipped {n}")))
            .collect();
        events.push(event("Review", at(16, 0), at(17, 0)));
        let view = view(agenda(events, Vec::new()), &now());
        assert_eq!(rows(&view), [row("16:00–17:00", "Review")]);
        assert_eq!(view.changes_at_ms, at(16, 0));
    }

    #[test]
    fn orders_reminders_by_due_date_and_marks_overdue_ones() {
        let yesterday = now().date_naive().pred_opt().unwrap();
        let tomorrow = now().date_naive().succ_opt().unwrap();
        let view = view(
            agenda(
                Vec::new(),
                vec![
                    reminder("Call back", Due::At(at(16, 0))),
                    reminder("Water plants", Due::Day(now().date_naive())),
                    reminder("Pay rent", Due::Day(yesterday)),
                    reminder("Send report", Due::At(at(9, 15))),
                    reminder("Next week", Due::Day(tomorrow)),
                    reminder("Late tonight", Due::At(at(0, 0) + 24 * HOUR)),
                ],
            ),
            &now(),
        );
        assert_eq!(
            rows(&view),
            [
                row("Overdue", "Pay rent"),
                row("Overdue", "Send report"),
                row("Today", "Water plants"),
                row("16:00", "Call back"),
            ]
        );
        assert_eq!(view.items[0].source, "Home");
        assert_eq!(view.items[0].action.label, "Open Reminders");
    }

    #[test]
    fn loads_again_when_the_next_item_starts_ends_or_falls_due() {
        let reminders = vec![reminder("Call back", Due::At(at(14, 40)))];
        let events = vec![event("Standup", at(13, 30), at(14, 30)), all_day("Off")];
        let soon = view(agenda(events, reminders.clone()), &now());
        assert_eq!(soon.changes_at_ms, at(14, 30));

        let later = view(
            agenda(vec![event("Review", at(15, 0), at(16, 0))], reminders),
            &now(),
        );
        assert_eq!(later.changes_at_ms, at(14, 40));

        let free = view(agenda(vec![all_day("Off")], Vec::new()), &now());
        assert_eq!(free.changes_at_ms, at(0, 0) + 24 * HOUR);

        let next = view(
            agenda(vec![event("Review", at(15, 0), at(16, 0))], Vec::new()),
            &(now() + TimeDelta::minutes(61)),
        );
        assert_eq!(rows(&next), [row("Now", "Review")]);
        assert_eq!(next.changes_at_ms, at(16, 0));
    }

    #[test]
    fn loads_again_when_a_reminder_left_out_falls_due() {
        // At 16:00 the last one becomes overdue and moves to the top.
        let mut reminders: Vec<Reminder> = (0..MAX_REMINDERS)
            .map(|n| reminder(&format!("Task {n}"), Due::Day(now().date_naive())))
            .collect();
        reminders.push(reminder("Call back", Due::At(at(16, 0))));
        let before = view(agenda(Vec::new(), reminders.clone()), &now());
        assert!(!before.items.iter().any(|item| item.title == "Call back"));
        assert_eq!(before.changes_at_ms, at(16, 0));

        let after = view(
            agenda(Vec::new(), reminders),
            &(now() + TimeDelta::hours(2)),
        );
        assert_eq!(after.items[0].title, "Call back");
        assert_eq!(after.items[0].time, "Overdue");
    }

    #[test]
    fn cuts_long_titles_and_names_empty_ones() {
        let long = "word ".repeat(40);
        let view = view(
            agenda(
                vec![
                    event(&long, at(15, 0), at(16, 0)),
                    event(" \n", at(17, 0), at(18, 0)),
                ],
                Vec::new(),
            ),
            &now(),
        );
        let title = &view.items[0].title;
        assert_eq!(title.chars().count(), MAX_TITLE_CHARS);
        assert!(title.ends_with('…') && !title.contains("  "));
        assert_eq!(view.items[1].title, "Untitled");
    }

    #[test]
    fn says_why_a_kind_cannot_show_and_offers_to_ask() {
        let both_denied = view(
            Agenda {
                events: Err(Unreadable::Denied),
                reminders: Err(Unreadable::Denied),
            },
            &now(),
        );
        assert_eq!(
            both_denied.problems,
            [
                "TinyDash cannot read your calendars and reminders. In the privacy settings of macOS, allow TinyDash under Calendars and under Reminders."
            ]
        );
        assert!(both_denied.ask.is_none() && both_denied.items.is_empty());

        let mixed = view(
            Agenda {
                events: Ok(vec![event("Review", at(15, 0), at(16, 0))]),
                reminders: Err(Unreadable::Denied),
            },
            &now(),
        );
        assert_eq!(rows(&mixed), [row("15:00–16:00", "Review")]);
        assert_eq!(
            mixed.problems,
            [
                "TinyDash cannot read your reminders. In the privacy settings of macOS, allow TinyDash under Reminders."
            ]
        );

        let waiting = view(
            Agenda {
                events: Err(Unreadable::NotAsked),
                reminders: Err(Unreadable::Restricted),
            },
            &now(),
        );
        assert_eq!(
            waiting.problems,
            [
                "TinyDash needs your permission to read your calendars.",
                "Access to your reminders is restricted on this Mac, for example by Screen Time.",
            ]
        );
        assert_eq!(
            waiting.ask,
            Some(ResultAction::new("Allow Access", Action::AskCalendarAccess))
        );

        // A terminal would get the access, so there is nothing to allow.
        let from_terminal = view(
            Agenda {
                events: Err(Unreadable::StartedByAnotherApp),
                reminders: Err(Unreadable::StartedByAnotherApp),
            },
            &now(),
        );
        assert_eq!(
            from_terminal.problems,
            [
                "TinyDash cannot read your calendars and reminders when another app, such as a terminal, starts it. Quit TinyDash, then open it from Finder or Launchpad."
            ]
        );
        assert!(from_terminal.ask.is_none());
    }

    #[test]
    fn the_day_ends_at_the_next_midnight_even_when_a_clock_change_skips_it() {
        // Chile moves its clocks from 00:00 to 01:00 on 2026-09-06.
        let santiago = chrono_tz::America::Santiago;
        let evening = santiago.with_ymd_and_hms(2026, 9, 5, 22, 0, 0).unwrap();
        let (from, to) = window(&evening);
        assert_eq!(to - from, 2 * HOUR);
        let utc = DateTime::from_timestamp_millis(to).unwrap();
        assert_eq!(utc.to_rfc3339(), "2026-09-06T04:00:00+00:00");
    }
}
