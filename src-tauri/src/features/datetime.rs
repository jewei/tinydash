//! Offline date arithmetic and time zone conversion.
//!
//! - Current time: `now`, `time in tokyo`, `london time`
//! - Conversion: `10am pacific`, `9:30 tokyo to london`, `tomorrow 3pm in kl`
//! - Dates: `today + 3 days`, `next friday + 2 weeks`, `in 10 days`, `2028-02-28 + 1 week`
//! - City clocks for the widget pane: `Tokyo`, `kl`, `Europe/London`

use std::{collections::HashMap, sync::LazyLock};

use chrono::{
    DateTime, Datelike, Days, FixedOffset, Local, LocalResult, Months, NaiveDate, NaiveDateTime,
    NaiveTime, TimeZone, Weekday,
};
use chrono_tz::Tz;
use serde::Serialize;
use ts_rs::TS;

use crate::search::result::{Action, Icon, ResultAction, ResultKind, SearchResult, Symbol};

/// The most cities the clocks widget shows next to local time.
pub const MAX_CLOCK_CITIES: usize = 3;

/// A city's clock in the widget pane. The launcher keeps the time current
/// from the offset; it is read again each time the pane loads, so a clock
/// change shows by the next open.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CityClock {
    /// The place as the user typed it.
    pub name: String,
    /// Seconds east of UTC, now.
    pub offset_seconds: i32,
    /// How far it is from local time: `+8h`, `−5h 30m`, or `Same time`.
    pub difference: String,
}

/// The clock of a place the user named, or `None` when no time zone has
/// that name. Local time is not a city: the widget always shows it.
pub fn city_clock(name: &str, now: DateTime<Local>) -> Option<CityClock> {
    let zone = zone(&name.to_lowercase()).filter(|zone| !matches!(zone, Zone::Local))?;
    let there = zone.at(now).offset().local_minus_utc();
    let here = now.offset().local_minus_utc();
    Some(CityClock {
        name: name.trim().to_owned(),
        offset_seconds: there,
        difference: difference(there - here),
    })
}

fn difference(seconds: i32) -> String {
    if seconds == 0 {
        return "Same time".into();
    }
    let sign = if seconds < 0 { '−' } else { '+' };
    let minutes = seconds.abs() / 60;
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{sign}{minutes}m"),
        (hours, 0) => format!("{sign}{hours}h"),
        (hours, minutes) => format!("{sign}{hours}h {minutes}m"),
    }
}

pub fn answers(query: &str, now: DateTime<Local>) -> Vec<SearchResult> {
    let query = query.trim().to_lowercase();
    if let Some(answer) = current_time_place(&query).and_then(|place| current_time(place, now)) {
        return vec![answer];
    }
    if let Some(answers) = conversion(&query, now) {
        return answers;
    }
    date_math(&query, now.date_naive()).into_iter().collect()
}

#[derive(Clone, Copy)]
enum Zone {
    Local,
    Named(Tz),
    Fixed(FixedOffset),
}

impl Zone {
    fn label(self) -> String {
        match self {
            Self::Local => "Local time".into(),
            Self::Named(tz) => tz.name().rsplit('/').next().unwrap_or("").replace('_', " "),
            Self::Fixed(offset) => format!("UTC{}", offset_text(offset)),
        }
    }

    fn at(self, instant: DateTime<Local>) -> DateTime<FixedOffset> {
        match self {
            Self::Local => instant.fixed_offset(),
            Self::Named(tz) => instant.with_timezone(&tz).fixed_offset(),
            Self::Fixed(offset) => instant.with_timezone(&offset),
        }
    }

    /// Every instant this wall-clock time names: none in a daylight-saving
    /// gap, two when clocks fall back.
    fn instants(self, wall: NaiveDateTime) -> Vec<DateTime<FixedOffset>> {
        fn all<Z: TimeZone>(result: LocalResult<DateTime<Z>>) -> Vec<DateTime<FixedOffset>> {
            match result {
                LocalResult::Single(t) => vec![t.fixed_offset()],
                LocalResult::Ambiguous(a, b) => vec![a.fixed_offset(), b.fixed_offset()],
                LocalResult::None => vec![],
            }
        }
        match self {
            Self::Local => all(Local.from_local_datetime(&wall)),
            Self::Named(tz) => all(tz.from_local_datetime(&wall)),
            Self::Fixed(offset) => all(offset.from_local_datetime(&wall)),
        }
    }

    fn today(self, now: DateTime<Local>) -> NaiveDate {
        self.at(now).date_naive()
    }
}

fn offset_text(offset: FixedOffset) -> String {
    let seconds = offset.local_minus_utc();
    let sign = if seconds < 0 { '-' } else { '+' };
    let minutes = seconds.abs() / 60;
    format!("{sign}{:02}:{:02}", minutes / 60, minutes % 60)
}

/// Names that people type for zones, beyond IANA city names.
const ALIASES: &[(&str, &str)] = &[
    ("pacific", "America/Los_Angeles"),
    ("pacific time", "America/Los_Angeles"),
    ("pt", "America/Los_Angeles"),
    ("mountain", "America/Denver"),
    ("mountain time", "America/Denver"),
    ("central", "America/Chicago"),
    ("central time", "America/Chicago"),
    ("eastern", "America/New_York"),
    ("eastern time", "America/New_York"),
    ("et", "America/New_York"),
    ("nyc", "America/New_York"),
    ("sf", "America/Los_Angeles"),
    ("san francisco", "America/Los_Angeles"),
    ("seattle", "America/Los_Angeles"),
    ("la", "America/Los_Angeles"),
    ("boston", "America/New_York"),
    ("washington", "America/New_York"),
    ("uk", "Europe/London"),
    ("england", "Europe/London"),
    ("britain", "Europe/London"),
    ("france", "Europe/Paris"),
    ("germany", "Europe/Berlin"),
    ("spain", "Europe/Madrid"),
    ("italy", "Europe/Rome"),
    ("netherlands", "Europe/Amsterdam"),
    ("india", "Asia/Kolkata"),
    ("mumbai", "Asia/Kolkata"),
    ("delhi", "Asia/Kolkata"),
    ("bangalore", "Asia/Kolkata"),
    ("china", "Asia/Shanghai"),
    ("beijing", "Asia/Shanghai"),
    ("hong kong", "Asia/Hong_Kong"),
    ("taiwan", "Asia/Taipei"),
    ("japan", "Asia/Tokyo"),
    ("korea", "Asia/Seoul"),
    ("malaysia", "Asia/Kuala_Lumpur"),
    ("kl", "Asia/Kuala_Lumpur"),
    ("singapore", "Asia/Singapore"),
    ("thailand", "Asia/Bangkok"),
    ("vietnam", "Asia/Ho_Chi_Minh"),
    ("indonesia", "Asia/Jakarta"),
    ("philippines", "Asia/Manila"),
    ("australia", "Australia/Sydney"),
    ("new zealand", "Pacific/Auckland"),
    ("uae", "Asia/Dubai"),
    ("brazil", "America/Sao_Paulo"),
    ("argentina", "America/Argentina/Buenos_Aires"),
    ("mexico", "America/Mexico_City"),
    ("canada", "America/Toronto"),
    ("south africa", "Africa/Johannesburg"),
    ("utc", "UTC"),
    ("gmt", "Etc/GMT"),
];

/// Unambiguous abbreviations that name a fixed offset, in hours.
const FIXED: &[(&str, i32)] = &[
    ("pst", -8),
    ("pdt", -7),
    ("mst", -7),
    ("mdt", -6),
    ("est", -5),
    ("edt", -4),
    ("cet", 1),
    ("cest", 2),
    ("jst", 9),
    ("kst", 9),
    ("sgt", 8),
    ("myt", 8),
    ("aest", 10),
];

static CITIES: LazyLock<HashMap<String, Tz>> = LazyLock::new(|| {
    let mut cities = HashMap::new();
    for tz in chrono_tz::TZ_VARIANTS {
        let name = tz.name().to_lowercase();
        // `Etc/GMT+5` is UTC−5 (POSIX signs), so `gmt+5` must not find it by
        // its last part. The full name still works.
        if !name.starts_with("etc/")
            && let Some(city) = name.rsplit('/').next()
        {
            cities.entry(city.replace('_', " ")).or_insert(tz);
        }
        cities.insert(name, tz);
    }
    for (alias, zone) in ALIASES {
        if let Ok(tz) = zone.parse() {
            cities.insert((*alias).to_owned(), tz);
        }
    }
    cities
});

fn zone(place: &str) -> Option<Zone> {
    let place = place.trim().trim_end_matches(" time").trim();
    if matches!(place, "local" | "here") {
        return Some(Zone::Local);
    }
    if let Some((_, hours)) = FIXED.iter().find(|(name, _)| *name == place) {
        return FixedOffset::east_opt(hours * 3600).map(Zone::Fixed);
    }
    CITIES.get(place).copied().map(Zone::Named)
}

/// `time in X`, `now in X`, `X time`, or plain `now`/`time`.
fn current_time_place(query: &str) -> Option<&str> {
    if matches!(query, "now" | "time") {
        return Some("local");
    }
    ["time in ", "now in ", "time at "]
        .iter()
        .find_map(|prefix| query.strip_prefix(prefix))
        .or_else(|| query.strip_suffix(" time"))
}

fn current_time(place: &str, now: DateTime<Local>) -> Option<SearchResult> {
    let zone = zone(place)?;
    let there = zone.at(now);
    Some(answer(
        format!("{} · {}", there.format("%H:%M"), there.format("%A, %-d %B")),
        format!("{} (UTC{})", zone.label(), offset_text(*there.offset())),
        there.format("%H:%M").to_string(),
    ))
}

/// `[date] <clock> [source] [to|in target]`. At least one place is needed.
fn conversion(query: &str, now: DateTime<Local>) -> Option<Vec<SearchResult>> {
    let words: Vec<&str> = query.split_whitespace().collect();
    let (day, rest) = match words.first().copied().and_then(day_word) {
        Some(day) => (Some(day), &words[1..]),
        None => (None, &words[..]),
    };
    let (time, used) = clock(rest)?;
    let rest = &rest[used..];
    let split = rest.iter().position(|w| matches!(*w, "to" | "in"));
    let (source, target) = match split {
        Some(i) => (rest[..i].join(" "), rest[i + 1..].join(" ")),
        None => (rest.join(" "), String::new()),
    };
    if source.is_empty() && target.is_empty() {
        return None;
    }
    let source = if source.is_empty() {
        Zone::Local
    } else {
        zone(&source)?
    };
    let target = if target.is_empty() {
        Zone::Local
    } else {
        zone(&target)?
    };
    let date = match day {
        Some(DayWord::Date(date)) => date,
        Some(DayWord::Offset(days)) => source
            .today(now)
            .checked_add_signed(chrono::Duration::days(days))?,
        None => source.today(now),
    };
    let answers = source
        .instants(date.and_time(time))
        .into_iter()
        .map(|instant| {
            let there = target.at(instant.with_timezone(&Local));
            answer(
                format!("{} · {}", there.format("%H:%M"), there.format("%a %-d %b")),
                format!(
                    "{} {} (UTC{}) in {}",
                    instant.format("%H:%M %a %-d %b"),
                    source.label(),
                    offset_text(*instant.offset()),
                    target.label(),
                ),
                there.format("%H:%M").to_string(),
            )
        })
        .collect();
    Some(answers)
}

enum DayWord {
    Date(NaiveDate),
    Offset(i64),
}

fn day_word(word: &str) -> Option<DayWord> {
    match word {
        "today" => Some(DayWord::Offset(0)),
        "tomorrow" => Some(DayWord::Offset(1)),
        "yesterday" => Some(DayWord::Offset(-1)),
        _ => iso_date(word).map(DayWord::Date),
    }
}

/// `10am`, `10 am`, `10:30`, `10:30 p.m.`, `22:15`, `noon`, `midnight`.
/// Returns the time and how many words it used.
fn clock(words: &[&str]) -> Option<(NaiveTime, usize)> {
    let first = *words.first()?;
    match first {
        "noon" => return Some((NaiveTime::from_hms_opt(12, 0, 0)?, 1)),
        "midnight" => return Some((NaiveTime::from_hms_opt(0, 0, 0)?, 1)),
        _ => {}
    }
    let meridiem = |word: &str| match word.replace('.', "").as_str() {
        "am" => Some(false),
        "pm" => Some(true),
        _ => None,
    };
    let digits_end = first
        .find(|c: char| !c.is_ascii_digit() && c != ':')
        .unwrap_or(first.len());
    let (number, suffix) = first.split_at(digits_end);
    let (pm, used) = if suffix.is_empty() {
        match words.get(1).and_then(|word| meridiem(word)) {
            Some(pm) => (Some(pm), 2),
            None => (None, 1),
        }
    } else {
        (Some(meridiem(suffix)?), 1)
    };
    let (hour, minute) = match number.split_once(':') {
        Some((hour, minute)) if minute.len() == 2 => {
            (hour.parse::<u32>().ok()?, minute.parse().ok()?)
        }
        None if pm.is_some() => (number.parse::<u32>().ok()?, 0),
        _ => return None,
    };
    let hour = match pm {
        Some(_) if !(1..=12).contains(&hour) => return None,
        Some(true) => hour % 12 + 12,
        Some(false) => hour % 12,
        None => hour,
    };
    Some((NaiveTime::from_hms_opt(hour, minute, 0)?, used))
}

fn iso_date(word: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(word, "%Y-%m-%d").ok()
}

/// `base (+|-) n unit ...` or `in n unit`. A bare ISO date also answers,
/// with its weekday.
fn date_math(query: &str, today: NaiveDate) -> Option<SearchResult> {
    let spaced = query.replace('+', " + ");
    let mut words = spaced.split_whitespace();
    let first = words.next()?;
    let (base, base_label) = match first {
        "today" => (today, "today"),
        "tomorrow" => (today + Days::new(1), "tomorrow"),
        "yesterday" => (today - Days::new(1), "yesterday"),
        "in" => (today, "today"),
        "next" | "last" => {
            let weekday: Weekday = words.next()?.parse().ok()?;
            let date = if first == "next" {
                next_weekday(today, weekday)
            } else {
                last_weekday(today, weekday)
            };
            (date, first)
        }
        word => (iso_date(word)?, ""),
    };
    let mut date = base;
    if first == "in" {
        date = shift(date, 1, &mut words)?;
    }
    while let Some(operator) = words.next() {
        let sign = match operator {
            "+" => 1,
            "-" => -1,
            _ => return None,
        };
        date = shift(date, sign, &mut words)?;
    }
    let from = if base_label.is_empty() {
        String::new()
    } else {
        format!(" · {} is {}", base_label, today.format("%a %-d %b %Y"))
    };
    let days = (date - today).num_days();
    let relative = match days {
        0 => "today".to_owned(),
        1 => "tomorrow".to_owned(),
        -1 => "yesterday".to_owned(),
        n if n > 0 => format!("in {n} days"),
        n => format!("{} days ago", -n),
    };
    let mut result = answer(
        date.format("%A, %-d %B %Y").to_string(),
        format!("{relative}{from}"),
        date.format("%Y-%m-%d").to_string(),
    );
    result.actions[0].label = "Copy Date".into();
    Some(result)
}

/// Apply one `<n> <unit>` step, also written together as in `3d` or `2weeks`.
fn shift<'a>(
    date: NaiveDate,
    sign: i64,
    words: &mut impl Iterator<Item = &'a str>,
) -> Option<NaiveDate> {
    let word = words.next()?;
    let split = word
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(word.len());
    let (amount, unit) = match word.split_at(split) {
        (amount, "") => (amount, words.next()?),
        glued => glued,
    };
    let amount: u32 = amount.parse().ok()?;
    let unit = unit.trim_end_matches('s');
    match (unit, sign) {
        ("day" | "d", 1) => date.checked_add_days(Days::new(amount.into())),
        ("day" | "d", _) => date.checked_sub_days(Days::new(amount.into())),
        ("week" | "w", 1) => date.checked_add_days(Days::new(u64::from(amount) * 7)),
        ("week" | "w", _) => date.checked_sub_days(Days::new(u64::from(amount) * 7)),
        ("month" | "mo", 1) => date.checked_add_months(Months::new(amount)),
        ("month" | "mo", _) => date.checked_sub_months(Months::new(amount)),
        ("year" | "y", 1) => date.checked_add_months(Months::new(amount.checked_mul(12)?)),
        ("year" | "y", _) => date.checked_sub_months(Months::new(amount.checked_mul(12)?)),
        _ => None,
    }
}

/// The first such weekday after `today`.
fn next_weekday(today: NaiveDate, weekday: Weekday) -> NaiveDate {
    let ahead = (7 + weekday.num_days_from_monday() - today.weekday().num_days_from_monday()) % 7;
    today + Days::new(if ahead == 0 { 7 } else { ahead.into() })
}

/// The last such weekday before `today`.
fn last_weekday(today: NaiveDate, weekday: Weekday) -> NaiveDate {
    let behind = (7 + today.weekday().num_days_from_monday() - weekday.num_days_from_monday()) % 7;
    today - Days::new(if behind == 0 { 7 } else { behind.into() })
}

fn answer(title: String, subtitle: String, copy: String) -> SearchResult {
    SearchResult {
        id: format!("time:{title}"),
        kind: ResultKind::DateTime,
        title,
        subtitle,
        icon: Icon::Symbol {
            name: Symbol::Clock,
        },
        actions: vec![ResultAction::new("Copy Time", Action::Copy { text: copy })],
        pinned: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Saturday, 4 October 2026, 12:00 UTC, seen from the test machine's zone.
    fn now() -> DateTime<Local> {
        chrono::Utc
            .with_ymd_and_hms(2026, 10, 4, 12, 0, 0)
            .unwrap()
            .with_timezone(&Local)
    }

    fn date(text: &str) -> NaiveDate {
        iso_date(text).unwrap()
    }

    fn math(query: &str) -> Option<String> {
        date_math(query, date("2026-10-03"))
            .map(|r| r.actions[0].action.clone())
            .map(|action| match action {
                Action::Copy { text } => text,
                _ => unreachable!(),
            })
    }

    #[test]
    fn date_arithmetic() {
        assert_eq!(math("today + 3 days").as_deref(), Some("2026-10-06"));
        assert_eq!(math("in 2 weeks").as_deref(), Some("2026-10-17"));
        assert_eq!(math("next friday + 2 weeks").as_deref(), Some("2026-10-23"));
        assert_eq!(math("next saturday").as_deref(), Some("2026-10-10"));
        assert_eq!(math("last monday").as_deref(), Some("2026-09-28"));
        assert_eq!(math("2028-02-28 + 1 day").as_deref(), Some("2028-02-29"));
        assert_eq!(math("2026-01-31 + 1 month").as_deref(), Some("2026-02-28"));
        assert_eq!(math("2026-10-03 - 1 year").as_deref(), Some("2025-10-03"));
        assert_eq!(math("tomorrow+1d").as_deref(), Some("2026-10-05"));
    }

    #[test]
    fn ordinary_words_are_not_dates() {
        for query in [
            "in",
            "today plus",
            "next",
            "next week",
            "2026-13-01",
            "in bed",
        ] {
            assert_eq!(math(query), None, "{query}");
        }
    }

    #[test]
    fn parses_clocks() {
        let t = |h, m| NaiveTime::from_hms_opt(h, m, 0).unwrap();
        assert_eq!(clock(&["10am"]), Some((t(10, 0), 1)));
        assert_eq!(clock(&["10", "p.m."]), Some((t(22, 0), 2)));
        assert_eq!(clock(&["12am"]), Some((t(0, 0), 1)));
        assert_eq!(clock(&["9:30"]), Some((t(9, 30), 1)));
        assert_eq!(clock(&["noon"]), Some((t(12, 0), 1)));
        assert_eq!(clock(&["13pm"]), None);
        assert_eq!(clock(&["10"]), None);
    }

    #[test]
    fn converts_between_zones_with_daylight_saving() {
        // 2026-07-01 10:00 in Los Angeles (UTC-7) is 02:00 next day in Tokyo.
        let results = conversion("2026-07-01 10am pacific to tokyo", now()).unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            results[0].title.starts_with("02:00 · Thu 2 Jul"),
            "{}",
            results[0].title
        );
        // PST is a fixed offset even in summer.
        let results = conversion("2026-07-01 10am pst to utc", now()).unwrap();
        assert!(results[0].title.starts_with("18:00"));
    }

    #[test]
    fn clock_changes_give_no_or_two_answers() {
        // US clocks skip 02:00–03:00 on 2026-03-08 and repeat 01:00 on 2026-11-01.
        assert!(
            conversion("2026-03-08 2:30am new york to utc", now())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            conversion("2026-11-01 1:30am new york to utc", now())
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn a_trailing_time_word_still_converts() {
        let results = conversion("2026-07-01 10am pacific time to utc", now()).unwrap();
        assert!(results[0].title.starts_with("17:00"));
        assert_eq!(answers("10am pacific time", now()).len(), 1);
    }

    #[test]
    fn posix_etc_zones_are_not_found_by_offset() {
        assert!(zone("gmt+5").is_none());
        assert!(zone("etc/gmt+5").is_some());
    }

    #[test]
    fn city_clocks_know_their_offset_and_difference() {
        // On 4 October 2026, Tokyo is UTC+9 and New York is UTC−4.
        let utc = chrono::Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let tokyo = city_clock(" Tokyo ", utc.with_timezone(&Local)).unwrap();
        assert_eq!(tokyo.name, "Tokyo");
        assert_eq!(tokyo.offset_seconds, 9 * 3600);
        assert_eq!(city_clock("kl", now()).unwrap().offset_seconds, 8 * 3600);
        assert_eq!(city_clock("pst", now()).unwrap().offset_seconds, -8 * 3600);
        assert_eq!(
            city_clock("America/New_York", now())
                .unwrap()
                .offset_seconds,
            -4 * 3600
        );
        for unknown in ["local", "here", "Atlantis", ""] {
            assert_eq!(city_clock(unknown, now()), None, "{unknown}");
        }

        assert_eq!(difference(0), "Same time");
        assert_eq!(difference(8 * 3600), "+8h");
        assert_eq!(difference(-(5 * 3600 + 30 * 60)), "−5h 30m");
        assert_eq!(difference(45 * 60), "+45m");
    }

    #[test]
    fn current_time_needs_a_time_word() {
        assert!(!answers("time in tokyo", now()).is_empty());
        assert!(!answers("london time", now()).is_empty());
        assert!(!answers("now", now()).is_empty());
        for query in ["tokyo", "time machine", "10am", "meeting at 10am"] {
            assert!(answers(query, now()).is_empty(), "{query}");
        }
    }
}
