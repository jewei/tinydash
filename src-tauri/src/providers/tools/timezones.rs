use chrono::{
    DateTime, Datelike, Days, FixedOffset, LocalResult, NaiveDate, NaiveTime, TimeZone, Utc,
    Weekday,
};
use chrono_tz::Tz;

use crate::launcher::result::ToolDetail;

const ALIASES: &[(&str, &str)] = &[
    ("japan", "Asia/Tokyo"),
    ("malaysia", "Asia/Kuala_Lumpur"),
    ("kl", "Asia/Kuala_Lumpur"),
    ("singapore", "Asia/Singapore"),
    ("sgt", "Asia/Singapore"),
    ("argentina", "America/Argentina/Buenos_Aires"),
    ("arg", "America/Argentina/Buenos_Aires"),
    ("art", "America/Argentina/Buenos_Aires"),
    ("gambia", "Africa/Banjul"),
    ("the gambia", "Africa/Banjul"),
    ("bahamas", "America/Nassau"),
    ("the bahamas", "America/Nassau"),
    ("uk", "Europe/London"),
    ("britain", "Europe/London"),
    ("england", "Europe/London"),
    ("india", "Asia/Kolkata"),
    ("mumbai", "Asia/Kolkata"),
    ("delhi", "Asia/Kolkata"),
    ("bangalore", "Asia/Kolkata"),
    ("china", "Asia/Shanghai"),
    ("beijing", "Asia/Shanghai"),
    ("korea", "Asia/Seoul"),
    ("nyc", "America/New_York"),
    ("new york city", "America/New_York"),
    ("boston", "America/New_York"),
    ("san francisco", "America/Los_Angeles"),
    ("seattle", "America/Los_Angeles"),
    ("la", "America/Los_Angeles"),
    ("eastern", "America/New_York"),
    ("et", "America/New_York"),
    ("us eastern", "America/New_York"),
    ("pacific", "America/Los_Angeles"),
    ("pacific time", "America/Los_Angeles"),
    ("pacific standard time", "Etc/GMT+8"),
    ("pacific daylight time", "Etc/GMT+7"),
    ("pt", "America/Los_Angeles"),
    ("us pacific", "America/Los_Angeles"),
    ("central", "America/Chicago"),
    ("us central", "America/Chicago"),
    ("mountain", "America/Denver"),
    ("utc", "UTC"),
    ("gmt", "GMT"),
    ("est", "EST"),
    ("pst", "Etc/GMT+8"),
    ("edt", "Etc/GMT+4"),
    ("pdt", "Etc/GMT+7"),
    ("jst", "Asia/Tokyo"),
    ("germany", "Europe/Berlin"),
    ("france", "Europe/Paris"),
    ("italy", "Europe/Rome"),
    ("spain", "Europe/Madrid"),
    ("portugal", "Europe/Lisbon"),
    ("new zealand", "Pacific/Auckland"),
    ("uae", "Asia/Dubai"),
    ("thailand", "Asia/Bangkok"),
    ("vietnam", "Asia/Ho_Chi_Minh"),
    ("philippines", "Asia/Manila"),
    ("taiwan", "Asia/Taipei"),
    ("south africa", "Africa/Johannesburg"),
];

fn normalized(value: &str) -> String {
    value.to_lowercase().replace('_', " ")
}
fn city(zone: Tz) -> String {
    zone.name()
        .rsplit('/')
        .next()
        .unwrap_or(zone.name())
        .replace('_', " ")
}

fn zones(location: &str) -> Vec<Tz> {
    let name = normalized(location.trim());
    let name = name.strip_prefix("the ").unwrap_or(&name);
    if let Some((_, zone)) = ALIASES.iter().find(|(alias, _)| *alias == name) {
        return vec![zone.parse().expect("known timezone")];
    }
    let region: &[&str] = match name {
        "us" | "usa" | "united states" => &[
            "America/New_York",
            "America/Chicago",
            "America/Denver",
            "America/Los_Angeles",
            "America/Anchorage",
            "Pacific/Honolulu",
        ],
        "australia" => &[
            "Australia/Sydney",
            "Australia/Melbourne",
            "Australia/Brisbane",
            "Australia/Adelaide",
            "Australia/Darwin",
            "Australia/Perth",
            "Australia/Hobart",
        ],
        "canada" => &[
            "America/Toronto",
            "America/Vancouver",
            "America/Winnipeg",
            "America/Edmonton",
            "America/Halifax",
            "America/St_Johns",
        ],
        "ist" => &["Asia/Kolkata", "Europe/Dublin", "Asia/Jerusalem"],
        "cst" => &["America/Chicago", "Asia/Shanghai", "America/Havana"],
        _ => &[],
    };
    if !region.is_empty() {
        return region
            .iter()
            .map(|name| name.parse().expect("known timezone"))
            .collect();
    }
    let exact: Vec<_> = chrono_tz::TZ_VARIANTS
        .iter()
        .copied()
        .filter(|zone| normalized(zone.name()) == name || normalized(&city(*zone)) == name)
        .collect();
    if !exact.is_empty() {
        return exact;
    }
    if name.len() < 2 {
        return vec![];
    }
    chrono_tz::TZ_VARIANTS
        .iter()
        .copied()
        .filter(|zone| {
            normalized(zone.name()).starts_with(&format!("{name}/"))
                || normalized(&city(*zone)).starts_with(name)
        })
        .take(30)
        .collect()
}

fn clock(value: &str) -> Option<NaiveTime> {
    let lower = value.to_ascii_lowercase();
    if lower == "noon" {
        return NaiveTime::from_hms_opt(12, 0, 0);
    }
    if lower == "midnight" {
        return NaiveTime::from_hms_opt(0, 0, 0);
    }
    let (value, meridiem) = if let Some(value) = lower.strip_suffix("am") {
        (value, Some(false))
    } else if let Some(value) = lower.strip_suffix("pm") {
        (value, Some(true))
    } else {
        (lower.as_str(), None)
    };
    let (hour, minute) = value.split_once(':').unwrap_or((value, "0"));
    let mut hour: u32 = hour.parse().ok()?;
    let minute: u32 = minute.parse().ok()?;
    if let Some(pm) = meridiem {
        if !(1..=12).contains(&hour) {
            return None;
        }
        hour = hour % 12 + if pm { 12 } else { 0 };
    }
    NaiveTime::from_hms_opt(hour, minute, 0)
}

fn input_text(query: &str) -> String {
    query
        .trim()
        .to_ascii_lowercase()
        .replace("a.m.", "am")
        .replace("p.m.", "pm")
        .replace("a.m", "am")
        .replace("p.m", "pm")
}

fn query_without_time_qualifier(query: &str) -> String {
    for prefix in ["the current ", "current ", "the "] {
        if let Some(rest) = query.strip_prefix(prefix)
            && ["time", "date", "datetime"]
                .iter()
                .any(|kind| rest == *kind || rest.starts_with(&format!("{kind} ")))
        {
            return rest.to_owned();
        }
    }
    query.to_owned()
}

fn explicit_clock(value: &str) -> bool {
    clock(value).is_some()
        && (value.contains(':')
            || value.ends_with("am")
            || value.ends_with("pm")
            || matches!(value, "noon" | "midnight"))
}

fn date_unit(value: &str) -> Option<u64> {
    match value {
        "day" | "days" => Some(1),
        "week" | "weeks" => Some(7),
        _ => None,
    }
}

fn weekday(value: &str) -> Option<Weekday> {
    match value {
        "monday" | "mon" => Some(Weekday::Mon),
        "tuesday" | "tue" => Some(Weekday::Tue),
        "wednesday" | "wed" => Some(Weekday::Wed),
        "thursday" | "thu" => Some(Weekday::Thu),
        "friday" | "fri" => Some(Weekday::Fri),
        "saturday" | "sat" => Some(Weekday::Sat),
        "sunday" | "sun" => Some(Weekday::Sun),
        _ => None,
    }
}

fn month(value: &str) -> Option<u32> {
    match value.trim_end_matches('.') {
        "january" | "jan" => Some(1),
        "february" | "feb" => Some(2),
        "march" | "mar" => Some(3),
        "april" | "apr" => Some(4),
        "may" => Some(5),
        "june" | "jun" => Some(6),
        "july" | "jul" => Some(7),
        "august" | "aug" => Some(8),
        "september" | "sep" | "sept" => Some(9),
        "october" | "oct" => Some(10),
        "november" | "nov" => Some(11),
        "december" | "dec" => Some(12),
        _ => None,
    }
}

fn day_number(value: &str) -> Option<u32> {
    let value = value
        .strip_suffix("st")
        .or_else(|| value.strip_suffix("nd"))
        .or_else(|| value.strip_suffix("rd"))
        .or_else(|| value.strip_suffix("th"))
        .unwrap_or(value);
    value.parse().ok().filter(|day| (1..=31).contains(day))
}

fn natural_date(value: &str, today: NaiveDate) -> Option<NaiveDate> {
    let cleaned = value.replace(',', " ");
    let parts: Vec<_> = cleaned.split_whitespace().collect();
    let (year, month, day) = match parts.as_slice() {
        [first, second] => {
            if let Some(month) = month(first) {
                (today.year(), month, day_number(second)?)
            } else {
                (today.year(), month(second)?, day_number(first)?)
            }
        }
        [first, second, year] => {
            let year = year.parse().ok()?;
            if let Some(month) = month(first) {
                (year, month, day_number(second)?)
            } else {
                (year, month(second)?, day_number(first)?)
            }
        }
        _ => return None,
    };
    NaiveDate::from_ymd_opt(year, month, day)
}

fn starts_with_date(value: &str) -> bool {
    let parts: Vec<_> = value.split_whitespace().collect();
    let Some(first) = parts.first().copied() else {
        return false;
    };
    matches!(first, "today" | "tomorrow" | "yesterday")
        || weekday(first).is_some()
        || month(first).is_some()
        || (parts.get(1).is_some_and(|part| month(part).is_some()) && day_number(first).is_some())
        || (first == "next"
            && parts
                .get(1)
                .is_some_and(|part| *part == "week" || weekday(part).is_some()))
        || (first == "in" && parts.get(2).is_some_and(|part| date_unit(part).is_some()))
        || (first.len() == 10
            && first.bytes().enumerate().all(|(index, byte)| {
                if matches!(index, 4 | 7) {
                    byte == b'-'
                } else {
                    byte.is_ascii_digit()
                }
            }))
}

pub fn is_candidate(query: &str) -> bool {
    let lower = query_without_time_qualifier(&input_text(query));
    if ["time", "date", "datetime"]
        .iter()
        .any(|prefix| lower == *prefix || lower.starts_with(&format!("{prefix} ")))
        || (starts_with_date(&lower)
            && date(&lower, NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()).is_ok())
    {
        return true;
    }
    let parts: Vec<_> = lower.split_whitespace().collect();
    let has_clock = parts.iter().any(|part| explicit_clock(part));
    has_clock && (parts.len() > 1)
}

fn date(value: &str, today: NaiveDate) -> Result<NaiveDate, String> {
    let value = value.trim().trim_end_matches(" at").replace('+', " + ");
    let parts: Vec<_> = value.split_whitespace().collect();
    let first_offset = parts
        .iter()
        .position(|part| matches!(*part, "+" | "-"))
        .unwrap_or(parts.len());
    let mut result = base_date(&parts[..first_offset].join(" "), today)?;
    for offset in parts[first_offset..].chunks(3) {
        let [sign, count, unit] = offset else {
            return Err("Use a date followed by + or - and a number of days or weeks.".into());
        };
        let count = count
            .parse::<u64>()
            .ok()
            .zip(date_unit(unit))
            .and_then(|(count, unit)| count.checked_mul(unit))
            .filter(|days| *days <= 36_600)
            .ok_or("Use a whole number of days or weeks, up to 36,600 days per step.")?;
        result = match *sign {
            "+" => result.checked_add_days(Days::new(count)),
            "-" => result.checked_sub_days(Days::new(count)),
            _ => return Err("Use + to add days or weeks, or - to subtract them.".into()),
        }
        .ok_or("This date is outside the supported range.")?;
    }
    Ok(result)
}

fn base_date(value: &str, today: NaiveDate) -> Result<NaiveDate, String> {
    let value = value.trim().trim_end_matches(" at").to_ascii_lowercase();
    let days = match value.as_str() {
        "" | "today" | "at" => Some(0),
        "tomorrow" => Some(1),
        "yesterday" => Some(-1),
        "next week" => Some(7),
        _ => None,
    };
    if let Some(days) = days {
        return today
            .checked_add_signed(chrono::Duration::days(days))
            .ok_or_else(|| "This date is outside the supported range.".into());
    }
    let parts: Vec<_> = value.split_whitespace().collect();
    if parts.len() == 3 && parts[0] == "in" && date_unit(parts[2]).is_some() {
        let count: u64 = parts[1]
            .parse()
            .map_err(|_| "Use a whole number of days or weeks, such as in 2 weeks.")?;
        if let Some(count) = count.checked_mul(date_unit(parts[2]).unwrap())
            && count <= 36_600
            && let Some(date) = today.checked_add_days(Days::new(count))
        {
            return Ok(date);
        }
    }
    if let Some(day) = weekday(value.strip_prefix("next ").unwrap_or(&value)) {
        let mut delta =
            (7 + day.num_days_from_monday() - today.weekday().num_days_from_monday()) % 7;
        if delta == 0 && value.starts_with("next ") {
            delta = 7;
        }
        return today
            .checked_add_days(Days::new(delta.into()))
            .ok_or_else(|| "This date is outside the supported range.".into());
    }
    if let Some(date) = natural_date(&value, today) {
        return Ok(date);
    }
    NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|_| {
        "Use today, tomorrow, a weekday, a month name, in 2 weeks, or YYYY-MM-DD. You can add or subtract days or weeks.".into()
    })
}

fn split_target_location(location: &str) -> (String, Option<String>) {
    for separator in [" to ", " in "] {
        if let Some((source, target)) = location.rsplit_once(separator)
            && !source.trim().is_empty()
            && !target.trim().is_empty()
            && !zones(source).is_empty()
            && zones(target).len() == 1
        {
            return (source.trim().to_owned(), Some(target.trim().to_owned()));
        }
    }
    (location.trim().to_owned(), None)
}

#[derive(Debug)]
pub struct TimeResult {
    pub title: String,
    pub subtitle: String,
    pub copy: String,
    pub detail: ToolDetail,
}

pub fn calculate(
    query: &str,
    now: DateTime<Utc>,
    local: impl Fn(DateTime<Utc>) -> DateTime<FixedOffset>,
) -> Result<Vec<TimeResult>, String> {
    let lower = query_without_time_qualifier(&input_text(query));
    let text = ["datetime ", "date ", "time "]
        .iter()
        .find_map(|prefix| lower.strip_prefix(prefix))
        .unwrap_or(&lower)
        .trim();
    let text = if starts_with_date(text) {
        text
    } else {
        text.strip_prefix("in ").unwrap_or(text)
    };
    if text.is_empty() || matches!(text, "time" | "date" | "datetime") {
        return Err("Try next Friday + 2 weeks, time in Tokyo, or 10am Pacific Time.".into());
    }
    let parts: Vec<_> = text.split_whitespace().collect();
    // A date's day count is not a clock. Prefer explicit clock tokens before bare hours.
    let explicit = parts.iter().position(|part| explicit_clock(part));
    let today = local(now).date_naive();
    let date_answer = if explicit.is_none() && starts_with_date(text) {
        date(text, today).ok()
    } else {
        None
    };
    let clock_index = explicit.or_else(|| {
        if date_answer.is_some() {
            return None;
        }
        parts
            .iter()
            .enumerate()
            .find(|(index, part)| {
                clock(part).is_some()
                    && parts
                        .get(index + 1)
                        .is_none_or(|next| date_unit(next).is_none())
            })
            .map(|(index, _)| index)
    });
    if clock_index.is_none() && starts_with_date(text) {
        let answer = match date_answer {
            Some(answer) => answer,
            None => date(text, today)?,
        };
        let result = answer.format("%a, %d %b %Y").to_string();
        let based_on = today.format("%a, %d %b %Y").to_string();
        return Ok(vec![TimeResult {
            title: result.clone(),
            subtitle: format!("{} · based on your local date, {}", query.trim(), based_on),
            copy: answer.format("%Y-%m-%d").to_string(),
            detail: ToolDetail::DateCalculation {
                expression: query.trim().into(),
                based_on,
                result,
            },
        }]);
    }
    let (location, selected_time, selected_date) = if let Some(index) = clock_index {
        let mut consumed = index + 1;
        let mut token = parts[index].to_owned();
        if parts
            .get(consumed)
            .is_some_and(|part| matches!(*part, "am" | "pm"))
        {
            token.push_str(parts[consumed]);
            consumed += 1;
        }
        let time = clock(&token).ok_or("Use a time such as 3pm or 15:30.")?;
        let location = parts[consumed..].join(" ");
        (
            location.strip_prefix("in ").unwrap_or(&location).to_owned(),
            Some(time),
            parts[..index].join(" "),
        )
    } else {
        (text.to_owned(), None, String::new())
    };
    let (location, target_location) = split_target_location(&location);
    let target_zones = target_location.as_deref().map(zones).unwrap_or_default();
    if target_location.is_some() && target_zones.len() != 1 {
        return Err("Target zone not found. Try a city or an IANA name.".into());
    }
    let target_zone = target_zones.into_iter().next();
    let zones = zones(&location);
    if zones.is_empty() {
        return Err("City or region not found. Try Tokyo, London, US, or an IANA name such as America/New_York.".into());
    }
    let mut output = Vec::new();
    for zone in zones {
        let (times, ambiguous) = if let Some(time) = selected_time {
            let date = date(&selected_date, now.with_timezone(&zone).date_naive())?;
            match zone.from_local_datetime(&date.and_time(time)) {
                LocalResult::Single(time) => (vec![time], false),
                LocalResult::Ambiguous(first, second) => (vec![first, second], true),
                LocalResult::None => {
                    return Err(format!(
                        "{} {} does not exist in {} because the clocks move forward. Try another time.",
                        date,
                        time.format("%H:%M"),
                        city(zone)
                    ));
                }
            }
        } else {
            (vec![now.with_timezone(&zone)], false)
        };
        for source in times {
            let target_name = target_zone.map(city);
            let converting = selected_time.is_some() || target_zone.is_some();
            let converted = target_zone.map_or_else(
                || local(source.with_timezone(&Utc)),
                |target| source.with_timezone(&target).fixed_offset(),
            );
            let source_text = format!(
                "{} · {} · UTC{}",
                source.format("%a, %d %b %Y · %H:%M"),
                city(zone),
                source.format("%:z")
            );
            let local_text = target_name.as_deref().map_or_else(
                || {
                    format!(
                        "{} · UTC{}",
                        converted.format("%a, %d %b %Y · %H:%M"),
                        converted.format("%:z")
                    )
                },
                |target| {
                    format!(
                        "{} · {} · UTC{}",
                        converted.format("%a, %d %b %Y · %H:%M"),
                        target,
                        converted.format("%:z")
                    )
                },
            );
            output.push(TimeResult {
                title: if converting {
                    target_name.as_deref().map_or_else(
                        || format!("{} · your local time", converted.format("%H:%M")),
                        |target| format!("{} · {} time", converted.format("%H:%M"), target),
                    )
                } else {
                    format!("{} · {}", source.format("%H:%M"), city(zone))
                },
                subtitle: if converting {
                    format!(
                        "{} · {}{}",
                        source_text,
                        target_name.as_deref().map_or_else(
                            || converted.format("%d %b locally").to_string(),
                            |target| format!("{} {}", converted.format("%d %b"), target),
                        ),
                        if ambiguous { " · occurs twice" } else { "" }
                    )
                } else {
                    format!(
                        "{} · {} · UTC{}",
                        source.format("%a, %d %b %Y"),
                        zone.name(),
                        source.format("%:z")
                    )
                },
                copy: if converting {
                    local_text.clone()
                } else {
                    source_text.clone()
                },
                detail: ToolDetail::Timezone {
                    source: source_text,
                    local: local_text,
                    source_zone: zone.name().into(),
                    target_zone: target_zone.map(|zone| zone.name().into()),
                    ambiguous,
                },
            });
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 17, 12, 0, 0).unwrap()
    }
    fn local(time: DateTime<Utc>) -> DateTime<FixedOffset> {
        time.with_timezone(&chrono_tz::Asia::Kuala_Lumpur)
            .fixed_offset()
    }

    #[test]
    fn shows_current_city_region_and_iana_times() {
        assert!(
            calculate("time in tokyo", now(), local).unwrap()[0]
                .title
                .starts_with("21:00")
        );
        assert_eq!(calculate("time in us", now(), local).unwrap().len(), 6);
        assert!(
            calculate("Asia/Kuala_Lumpur", now(), local).unwrap()[0]
                .title
                .starts_with("20:00")
        );
        assert_eq!(
            calculate("time in san francisco", now(), local)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn accepts_unambiguous_abbreviations_and_qualified_place_queries() {
        assert!(
            calculate("time in SGT", now(), local).unwrap()[0]
                .title
                .starts_with("20:00")
        );
        assert!(
            calculate("time in ART", now(), local).unwrap()[0]
                .title
                .starts_with("09:00")
        );
        assert!(is_candidate("the current time in the gambia"));
        assert!(
            calculate("the current time in the gambia", now(), local).unwrap()[0]
                .title
                .contains("Banjul")
        );
    }

    #[test]
    fn parses_month_name_dates_and_explicit_target_zones() {
        let current = calculate("time in Tokyo to London", now(), local).unwrap();
        assert!(current[0].title.starts_with("13:00"));
        assert!(current[0].copy.contains("London"));
        for query in [
            "September 18",
            "18 Sep",
            "September 18 2026",
            "18th September 2026",
            "September 18, 2026",
        ] {
            assert!(is_candidate(query), "{query}");
            let result = calculate(query, now(), local).unwrap();
            assert_eq!(result[0].copy, "2026-09-18", "{query}");
            assert!(matches!(
                result[0].detail,
                ToolDetail::DateCalculation { .. }
            ));
        }
        assert_eq!(
            calculate("September 18 + 2 days", now(), local).unwrap()[0].copy,
            "2026-09-20"
        );
        assert!(calculate("September 31", now(), local).is_err());
        let result = calculate("September 18, 2026 10am Tokyo", now(), local).unwrap();
        assert!(result[0].copy.contains("18 Sep 2026"));

        let result = calculate("18 Sep 2026 10am Tokyo to London", now(), local).unwrap();
        assert!(result[0].title.starts_with("02:00"));
        assert!(result[0].title.contains("London time"));
        assert!(
            matches!(&result[0].detail, ToolDetail::Timezone { target_zone: Some(zone), .. } if zone == "Europe/London")
        );
        assert!(result[0].copy.contains("18 Sep 2026"));

        let result = calculate("2026-12-15 10am Tokyo to New York", now(), local).unwrap();
        assert!(result[0].copy.contains("14 Dec 2026"));
    }
    #[test]
    fn relative_dates_are_dates_in_the_source_city_and_convert_across_midnight() {
        let result = calculate("tomorrow 3pm london", now(), local).unwrap();
        assert!(result[0].title.starts_with("22:00"));
        assert!(result[0].copy.contains("18 Sep 2026"));
        assert!(
            calculate("in 2 days 11pm london", now(), local).unwrap()[0]
                .copy
                .contains("20 Sep 2026")
        );
        assert!(
            calculate("next monday 3 pm london", now(), local).unwrap()[0]
                .copy
                .contains("21 Sep 2026")
        );
        assert!(
            calculate("yesterday 15:00 london", now(), local).unwrap()[0]
                .copy
                .contains("16 Sep 2026")
        );
    }
    #[test]
    fn date_arithmetic_uses_the_local_calendar_and_copies_an_iso_date() {
        let result = calculate("next friday + 2 week", now(), local).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Fri, 02 Oct 2026");
        assert_eq!(result[0].copy, "2026-10-02");
        assert!(matches!(
            &result[0].detail,
            ToolDetail::DateCalculation { expression, based_on, result }
                if expression == "next friday + 2 week"
                    && based_on == "Thu, 17 Sep 2026"
                    && result == "Fri, 02 Oct 2026"
        ));
        let friday_locally = Utc.with_ymd_and_hms(2026, 9, 17, 23, 30, 0).unwrap();
        assert_eq!(
            calculate("next Friday + 2 weeks", friday_locally, local).unwrap()[0].copy,
            "2026-10-09",
            "Next Friday must be after today, using the local date"
        );
        assert_eq!(
            calculate("date today + 1 day", friday_locally, local).unwrap()[0].copy,
            "2026-09-19"
        );
        assert_eq!(
            calculate("datetime in 2 weeks", now(), local).unwrap()[0].copy,
            "2026-10-01"
        );
    }

    #[test]
    fn date_arithmetic_handles_leap_days_subtraction_and_invalid_offsets() {
        for (query, expected) in [
            ("2028-02-28 + 1 day", "2028-02-29"),
            ("2028-02-28 + 1 week - 2 days", "2028-03-04"),
            ("2026-01-01 - 1 day", "2025-12-31"),
            ("today +2 weeks", "2026-10-01"),
        ] {
            assert_eq!(
                calculate(query, now(), local).unwrap()[0].copy,
                expected,
                "{query}"
            );
        }
        for query in [
            "2026-02-30 + 1 day",
            "today + 2 months",
            "today + 1.5 weeks",
            "today + -1 week",
            "today + 2 weeks +",
            "today + 50000 days",
            "today + 18446744073709551615 weeks",
        ] {
            assert!(calculate(query, now(), local).is_err(), "{query}");
        }
    }

    #[test]
    fn pacific_time_accepts_meridiem_punctuation_and_uses_the_selected_date() {
        let results = calculate("10:00 a.m. Pacific Time", now(), local).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].title.starts_with("01:00"));
        assert!(results[0].copy.contains("18 Sep 2026"));
        assert!(matches!(
            &results[0].detail,
            ToolDetail::Timezone { source, source_zone, .. }
                if source.contains("UTC-07:00") && source_zone == "America/Los_Angeles"
        ));
        let winter = calculate("2026-12-15 10:00 a.m. Pacific Time", now(), local).unwrap();
        assert!(winter[0].title.starts_with("02:00"));
        assert!(winter[0].copy.contains("16 Dec 2026"));
        assert!(winter[0].subtitle.contains("UTC-08:00"));
        for query in [
            "10:00a.m. Pacific Time",
            "10 a.m. Pacific Time",
            "10am Pacific",
        ] {
            assert_eq!(
                calculate(query, now(), local).unwrap()[0].copy,
                results[0].copy
            );
        }
        assert!(
            calculate("2026-10-25 + 2 weeks 10:00 a.m. Pacific Time", now(), local).unwrap()[0]
                .copy
                .contains("09 Nov 2026 · 02:00")
        );
        assert!(
            calculate("10:00 a.m. Pacific Standard Time", now(), local).unwrap()[0]
                .title
                .starts_with("02:00")
        );
        assert!(
            calculate("10:00 p.m. Pacific Time", now(), local).unwrap()[0]
                .title
                .starts_with("13:00")
        );
    }

    #[test]
    fn pacific_clock_changes_keep_missing_and_repeated_times_explicit() {
        assert!(
            calculate("2026-03-08 2:30 a.m. Pacific Time", now(), local)
                .unwrap_err()
                .contains("clocks move forward")
        );
        let results = calculate("2026-11-01 1:30 a.m. Pacific Time", now(), local).unwrap();
        assert_eq!(results.len(), 2);
        assert_ne!(results[0].copy, results[1].copy);
        assert!(results.iter().all(|result| matches!(
            result.detail,
            ToolDetail::Timezone {
                ambiguous: true,
                ..
            }
        )));
    }

    #[test]
    fn all_mode_recognizes_dates_without_taking_ordinary_file_searches() {
        for query in [
            "next friday + 2 week",
            "today",
            "2028-02-28 + 1 day",
            "10:00 a.m. Pacific Time",
            "date tomorrow",
        ] {
            assert!(is_candidate(query), "{query}");
        }
        for query in [
            "Friday notes",
            "tomorrow report",
            "timezones.rs",
            "12 + 8",
            "Finder",
        ] {
            assert!(!is_candidate(query), "{query}");
        }
    }
    #[test]
    fn daylight_saving_gaps_are_rejected_and_both_repeated_times_are_explicit() {
        assert!(
            calculate("2026-03-29 1:30 london", now(), local)
                .unwrap_err()
                .contains("clocks move forward")
        );
        let results = calculate("2026-10-25 1:30 london", now(), local).unwrap();
        assert_eq!(results.len(), 2);
        assert_ne!(results[0].copy, results[1].copy);
        assert!(matches!(
            results[0].detail,
            ToolDetail::Timezone {
                ambiguous: true,
                ..
            }
        ));
        assert!(calculate("2026-02-30 3pm london", now(), local).is_err());
        assert!(calculate("time in imaginary", now(), local).is_err());
        assert!(calculate("tomorrow 25pm london", now(), local).is_err());
        for query in [
            "10am Atlantis",
            "10am Tokyo to Atlantis",
            "10am Atlantis to London",
            "10am Tokyo to US",
        ] {
            assert!(calculate(query, now(), local).is_err(), "{query}");
        }
    }
}
