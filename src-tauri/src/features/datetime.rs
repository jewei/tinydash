//! Offline date arithmetic and time zone conversion.
//!
//! - Current time: `now`, `time in tokyo`, `london time`
//! - Conversion: `10am pacific`, `9:30 tokyo to london`, `tomorrow 3pm in kl`
//! - Dates: `today + 3 days`, `next friday + 2 weeks`, `in 10 days`, `2028-02-28 + 1 week`,
//!   `3 days from now`, `a week ago`, `2 weeks after dec 25`
//! - Day counts: `days until dec 25`, `days since 2026-01-01`, `days between 1 mar and 30 may`
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
    let today = now.date_naive();
    day_count(&query, today)
        .or_else(|| date_math(&query, today))
        .into_iter()
        .collect()
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

/// Whether the whole query is a full `YYYY-MM-DD` date or a clock time, such
/// as `2026-10-17` or `3pm`. The calculator leaves these alone: fend reads them
/// as a subtraction or a length. Shorter dates, such as `2026-1-5`, stay
/// subtractions.
pub fn is_date_or_clock(query: &str) -> bool {
    let query = query.trim().to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    let full_date = query.len() == 10 && iso_date(&query).is_some();
    full_date || clock(&words).is_some_and(|(_, used)| used == words.len())
}

/// A 4-digit year and a 1- or 2-digit month and day, such as `2026-10-17` or
/// `2026-1-5`. chrono's `%Y-%m-%d` alone also accepts `12-3-4`, which is a
/// subtraction.
fn iso_date(word: &str) -> Option<NaiveDate> {
    let digits = |part: Option<&str>, lengths: std::ops::RangeInclusive<usize>| {
        part.is_some_and(|part| {
            lengths.contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_digit())
        })
    };
    let mut parts = word.split('-');
    let shaped = digits(parts.next(), 4..=4)
        && digits(parts.next(), 1..=2)
        && digits(parts.next(), 1..=2)
        && parts.next().is_none();
    if !shaped {
        return None;
    }
    NaiveDate::parse_from_str(word, "%Y-%m-%d").ok()
}

/// `base (+|-) n unit ...`, or a phrase that `relative_phrase` reads. A bare
/// date also answers, with its weekday.
fn date_math(query: &str, today: NaiveDate) -> Option<SearchResult> {
    let query = relative_phrase(query).unwrap_or_else(|| query.to_owned());
    let spaced = query.replace('+', " + ");
    let words: Vec<&str> = spaced.split_whitespace().flat_map(split_minus).collect();
    let end = words
        .iter()
        .position(|word| matches!(*word, "+" | "-"))
        .unwrap_or(words.len());
    let base = named_day(&words[..end], today)?;
    let mut date = base.date;
    let mut steps = words[end..].iter().copied();
    while let Some(operator) = steps.next() {
        let sign = match operator {
            "+" => 1,
            "-" => -1,
            _ => return None,
        };
        date = shift(date, sign, &mut steps)?;
    }
    let from = match &base.label {
        Some(label) => format!(" · {label} is {}", base.date.format("%a %-d %b %Y")),
        None => String::new(),
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

/// `in 10 days`, `3 days from now`, `a week ago`, `2 weeks from tomorrow`,
/// `2 months 3 days after dec 25`, or `3 days before next friday` in the
/// `base (+|-) n unit ...` form.
fn relative_phrase(query: &str) -> Option<String> {
    if let Some(amount) = query.strip_prefix("in ") {
        // `in 2 weeks + 1 day` already has its signs.
        let steps = steps(amount).map_or_else(|| amount.to_owned(), |steps| steps.join(" + "));
        return Some(format!("today + {steps}"));
    }
    let (amount, sign, base) = match query.strip_suffix(" ago") {
        Some(amount) => (amount, '-', "today"),
        None => [(" after ", '+'), (" from ", '+'), (" before ", '-')]
            .iter()
            .find_map(|(word, sign)| {
                let (amount, base) = query.split_once(word)?;
                Some((amount, *sign, if base == "now" { "today" } else { base }))
            })?,
    };
    let steps: Vec<String> = steps(amount)?
        .iter()
        .map(|step| format!("{sign} {step}"))
        .collect();
    Some(format!("{base} {}", steps.join(" ")))
}

/// `2 months 3 days`, `2 months and 3 days`, `a week`, or `3d` as steps
/// such as `2 months`, for `shift` to check.
fn steps(amount: &str) -> Option<Vec<String>> {
    let mut words = amount
        .split_whitespace()
        .map(|word| word.trim_end_matches(','))
        .filter(|word| *word != "and");
    let mut steps = Vec::new();
    while let Some(word) = words.next() {
        let number = if matches!(word, "a" | "an") {
            "1"
        } else {
            word
        };
        if number.bytes().all(|byte| byte.is_ascii_digit()) {
            steps.push(format!("{number} {}", words.next()?));
        } else if number.starts_with(|c: char| c.is_ascii_digit()) {
            steps.push(number.to_owned());
        } else {
            return None;
        }
    }
    (!steps.is_empty()).then_some(steps)
}

/// `days until dec 25`, `days till next friday`, `days since 2026-01-01`,
/// or `days between 1 mar and 30 may`.
fn day_count(query: &str, today: NaiveDate) -> Option<SearchResult> {
    let rest = query.strip_prefix("days ")?;
    let day = |text: &str| named_day(&text.split_whitespace().collect::<Vec<_>>(), today);
    let (from, to, subtitle) = if let Some(rest) = rest.strip_prefix("between ") {
        let (first, second) = rest.split_once(" and ")?;
        let (first, second) = (day(first)?, day(second)?);
        // Without years, `days between dec 20 and jan 5` counts to the
        // January after.
        let second = if !first.has_year && !second.has_year && second.date < first.date {
            let (month, day_of_month) = (second.date.month(), second.date.day());
            let date = in_year(month, day_of_month, first.date.year(), 1)?;
            if date < first.date {
                in_year(month, day_of_month, first.date.year() + 1, 1)?
            } else {
                date
            }
        } else {
            second.date
        };
        let first = first.date;
        let subtitle = format!(
            "from {} to {}",
            first.format("%a %-d %b %Y"),
            second.format("%a %-d %b %Y")
        );
        (first, second, subtitle)
    } else {
        let (target, ahead) = match rest.split_once(' ')? {
            ("until" | "till", target) => (target, true),
            ("since", target) => (target, false),
            _ => return None,
        };
        let day = day(target)?;
        // Without a year, `days until jan 1` in October means the coming
        // one, and `days since dec 25` in October the last one.
        let (month, day_of_month) = (day.date.month(), day.date.day());
        let date = match (day.has_year, ahead) {
            (false, true) if day.date < today => {
                in_year(month, day_of_month, day.date.year() + 1, 1)?
            }
            (false, false) if day.date > today => {
                in_year(month, day_of_month, day.date.year() - 1, -1)?
            }
            _ => day.date,
        };
        let word = if date >= today { "until" } else { "since" };
        (
            today,
            date,
            format!("{word} {}", date.format("%A, %-d %B %Y")),
        )
    };
    let days = (to - from).num_days().abs();
    let weeks = match (days / 7, days % 7) {
        (0, _) => String::new(),
        (weeks, 0) => format!("{} · ", plural(weeks, "week")),
        (weeks, rest) => format!("{} {} · ", plural(weeks, "week"), plural(rest, "day")),
    };
    let mut result = answer(
        plural(days, "day"),
        format!("{weeks}{subtitle}"),
        days.to_string(),
    );
    result.actions[0].label = "Copy Days".into();
    Some(result)
}

fn plural(count: i64, unit: &str) -> String {
    if count == 1 {
        format!("1 {unit}")
    } else {
        format!("{count} {unit}s")
    }
}

/// A day the user named.
struct NamedDay {
    date: NaiveDate,
    /// The words of a day relative to today, such as `next friday`.
    label: Option<String>,
    /// Whether the user typed the year.
    has_year: bool,
}

/// `today`, `tomorrow`, `yesterday`, `next friday`, `last monday`, an ISO
/// date, or a date with a month name, such as `25 dec` or `December 25, 2026`.
fn named_day(words: &[&str], today: NaiveDate) -> Option<NamedDay> {
    let relative = |date| {
        Some(NamedDay {
            date,
            label: Some(words.join(" ")),
            has_year: true,
        })
    };
    match *words {
        ["today"] => relative(today),
        ["tomorrow"] => relative(today + Days::new(1)),
        ["yesterday"] => relative(today - Days::new(1)),
        [which @ ("next" | "last"), name] => {
            let weekday: Weekday = name.parse().ok()?;
            relative(if which == "next" {
                next_weekday(today, weekday)
            } else {
                last_weekday(today, weekday)
            })
        }
        [word] => Some(NamedDay {
            date: iso_date(word)?,
            label: None,
            has_year: true,
        }),
        _ => month_date(words, today),
    }
}

const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

/// `25 dec`, `dec 25`, `25th december 2026`, or `December 25, 2026`.
/// Without a year, the date is in this year, or for `feb 29`, the next leap
/// year.
fn month_date(words: &[&str], today: NaiveDate) -> Option<NamedDay> {
    let (first, second, year) = match *words {
        [first, second] => (first, second, None),
        [first, second, year] => (first, second, Some(year)),
        _ => return None,
    };
    let (day, month) = match (day_of_month(first), month(second)) {
        (Some(day), Some(month)) => (day, month),
        _ => (day_of_month(second)?, month(first)?),
    };
    let year = match year {
        Some(year) if year.len() == 4 => Some(year.parse().ok()?),
        Some(_) => return None,
        None => None,
    };
    let date = match year {
        Some(year) => NaiveDate::from_ymd_opt(year, month, day)?,
        None => in_year(month, day, today.year(), 1)?,
    };
    Some(NamedDay {
        date,
        label: None,
        has_year: year.is_some(),
    })
}

/// The day in `year`, or in the first year after it (`step` 1) or before it
/// (`step` -1) that has the day: only leap years have 29 February.
fn in_year(month: u32, day: u32, year: i32, step: i32) -> Option<NaiveDate> {
    (0..8).find_map(|n| NaiveDate::from_ymd_opt(year.checked_add(n * step)?, month, day))
}

/// `25`, `25th`, or `25,`.
fn day_of_month(word: &str) -> Option<u32> {
    let word = word.trim_end_matches(',');
    let digits = ["st", "nd", "rd", "th"]
        .iter()
        .find_map(|suffix| word.strip_suffix(suffix))
        .unwrap_or(word);
    if digits.is_empty() || digits.len() > 2 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// A month by its name or a start of 3 letters or more: `dec`, `sept`.
fn month(word: &str) -> Option<u32> {
    let word = word.trim_end_matches(['.', ',']);
    if word.len() < 3 {
        return None;
    }
    let index = MONTHS.iter().position(|month| month.starts_with(word))?;
    u32::try_from(index + 1).ok()
}

/// Split a minus sign from the words it touches, as in `-3`, `today-3d`,
/// or `friday-1w`. The query cannot simply put spaces around every `-`, as
/// it does for `+`, because ISO dates contain `-`; a word that starts with
/// a digit keeps its `-`.
fn split_minus(word: &str) -> Vec<&str> {
    match word.split_once('-') {
        Some((before, after))
            if !after.is_empty() && before.chars().all(|c| c.is_ascii_alphabetic()) =>
        {
            let mut parts = Vec::with_capacity(3);
            if !before.is_empty() {
                parts.push(before);
            }
            parts.extend(["-", after]);
            parts
        }
        _ => vec![word],
    }
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
        assert_eq!(math("2026-10-17").as_deref(), Some("2026-10-17"));
        assert_eq!(math("2026-1-5 + 1 week").as_deref(), Some("2026-01-12"));
    }

    #[test]
    fn minus_sign_may_touch_its_number() {
        for (glued, spaced) in [
            ("today -3 days", "today - 3 days"),
            ("today-3d", "today - 3d"),
            ("next friday -1 week", "next friday - 1 week"),
            ("next friday-1w", "next friday - 1w"),
            ("2026-10-17 -2d", "2026-10-17 - 2d"),
        ] {
            assert!(math(spaced).is_some(), "{spaced}");
            assert_eq!(math(glued), math(spaced), "{glued}");
        }
    }

    #[test]
    fn date_math_subtitle_names_the_base_date() {
        let subtitle = |query| date_math(query, date("2026-10-03")).unwrap().subtitle;
        assert_eq!(
            subtitle("tomorrow + 1 day"),
            "in 2 days · tomorrow is Sun 4 Oct 2026"
        );
        assert_eq!(
            subtitle("yesterday - 1 day"),
            "2 days ago · yesterday is Fri 2 Oct 2026"
        );
        assert_eq!(
            subtitle("next friday + 2 weeks"),
            "in 20 days · next friday is Fri 9 Oct 2026"
        );
        assert_eq!(
            subtitle("last monday"),
            "5 days ago · last monday is Mon 28 Sep 2026"
        );
        assert_eq!(
            subtitle("in 2 weeks"),
            "in 14 days · today is Sat 3 Oct 2026"
        );
        assert_eq!(subtitle("2026-10-03 - 1 year"), "365 days ago");
    }

    #[test]
    fn reads_relative_phrases() {
        for (query, expected) in [
            ("3 days from now", "2026-10-06"),
            ("3d from today", "2026-10-06"),
            ("3 days ago", "2026-09-30"),
            ("a week ago", "2026-09-26"),
            ("2 weeks from tomorrow", "2026-10-18"),
            ("3 days after next friday", "2026-10-12"),
            ("2 months 3 days after 2026-06-05", "2026-08-08"),
            ("2 months and 3 days before 2026-06-05", "2026-04-02"),
            ("28 days before mar 12", "2026-02-12"),
            ("in 2 weeks + 1 day", "2026-10-18"),
            ("in a week", "2026-10-10"),
            ("in 2 months 3 days", "2026-12-06"),
            ("in 2 weeks and 1 day", "2026-10-18"),
        ] {
            assert_eq!(math(query).as_deref(), Some(expected), "{query}");
        }
        for query in [
            "3 days",
            "days ago",
            "3 apples ago",
            "a week from monday",
            "3 days after",
        ] {
            assert_eq!(math(query), None, "{query}");
        }
    }

    #[test]
    fn reads_dates_with_month_names() {
        for (query, expected) in [
            ("dec 25", "2026-12-25"),
            ("25 dec", "2026-12-25"),
            ("25th december 2027", "2027-12-25"),
            ("december 25, 2026 + 1 week", "2027-01-01"),
            ("Sept 1", "2026-09-01"),
        ] {
            assert_eq!(
                math(&query.to_lowercase()).as_deref(),
                Some(expected),
                "{query}"
            );
        }
        assert_eq!(math("feb 29").as_deref(), Some("2028-02-29"));
        for query in [
            "jan 2026",
            "may",
            "ma 5",
            "dec 32",
            "feb 30",
            "feb 29 2026",
            "dec 25 26",
            "june 5 notes",
        ] {
            assert_eq!(math(query), None, "{query}");
        }
    }

    #[test]
    fn counts_days() {
        let count = |query| {
            let result = day_count(query, date("2026-10-03")).unwrap();
            (result.title, result.subtitle)
        };
        let title = |query| count(query).0;
        assert_eq!(
            count("days until dec 25"),
            (
                "83 days".into(),
                "11 weeks 6 days · until Friday, 25 December 2026".into()
            )
        );
        assert_eq!(title("days until jan 1"), "90 days");
        assert_eq!(title("days till next friday"), "6 days");
        assert_eq!(title("days since jan 1"), "275 days");
        assert_eq!(title("days since dec 25"), "282 days");
        assert_eq!(
            count("days until 2026-10-01"),
            ("2 days".into(), "since Thursday, 1 October 2026".into())
        );
        assert_eq!(
            count("days until tomorrow"),
            ("1 day".into(), "until Sunday, 4 October 2026".into())
        );
        assert_eq!(
            count("days between 3 march and 30 may"),
            (
                "88 days".into(),
                "12 weeks 4 days · from Tue 3 Mar 2026 to Sat 30 May 2026".into()
            )
        );
        assert_eq!(title("days between 2026-01-15 and 2026-01-01"), "14 days");
        // Without years, the second date is the first one after the first.
        assert_eq!(
            count("days between dec 20 and jan 5"),
            (
                "16 days".into(),
                "2 weeks 2 days · from Sun 20 Dec 2026 to Tue 5 Jan 2027".into()
            )
        );
        assert_eq!(title("days between jan 5 and dec 20"), "349 days");
        assert_eq!(title("days between dec 20 and 2026-01-05"), "349 days");
        assert_eq!(title("days between feb 29 and mar 1"), "1 day");
        assert_eq!(title("days until feb 29"), "514 days");
        assert_eq!(title("days since feb 29"), "947 days");
        let in_march_2028 = date("2028-03-01");
        let after_leap_day = |query| day_count(query, in_march_2028).unwrap().title;
        assert_eq!(after_leap_day("days until feb 29"), "1460 days");
        assert_eq!(after_leap_day("days since feb 29"), "1 day");
        assert_eq!(
            count("days until 2026-10-17").1,
            "2 weeks · until Saturday, 17 October 2026"
        );
        for query in [
            "days",
            "days until",
            "days until soon",
            "days between today",
            "days ago",
        ] {
            assert!(day_count(query, date("2026-10-03")).is_none(), "{query}");
        }
        let result = &answers("Days until dec 25", now())[0];
        assert_eq!(result.actions[0].label, "Copy Days");
    }

    #[test]
    fn ordinary_words_are_not_dates() {
        for query in [
            "in",
            "today plus",
            "next",
            "next week",
            "2026-13-01",
            "12-3-4",
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
        let results = conversion("2026-7-1 10am pst to utc", now()).unwrap();
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
