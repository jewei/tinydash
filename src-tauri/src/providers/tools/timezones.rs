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
    if let Some((_, zone)) = ALIASES.iter().find(|(alias, _)| *alias == name) {
        return vec![zone.parse().expect("known timezone")];
    }
    let region: &[&str] = match name.as_str() {
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
                || normalized(&city(*zone)).starts_with(&name)
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

pub fn is_candidate(query: &str) -> bool {
    let lower = query.to_ascii_lowercase();
    if lower == "time" || lower.starts_with("time ") {
        return true;
    }
    let parts: Vec<_> = lower.split_whitespace().collect();
    let has_clock = parts.iter().any(|part| {
        clock(part).is_some()
            && (part.contains(':')
                || part.ends_with("am")
                || part.ends_with("pm")
                || matches!(*part, "noon" | "midnight"))
    });
    has_clock && (parts.len() > 1)
}

fn date(value: &str, today: NaiveDate) -> Result<NaiveDate, String> {
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
    if parts.len() == 3 && parts[0] == "in" && matches!(parts[2], "day" | "days") {
        let count: u64 = parts[1]
            .parse()
            .map_err(|_| "Use a number of days, such as in 2 days 3pm London.")?;
        if count <= 36_600
            && let Some(date) = today.checked_add_days(Days::new(count))
        {
            return Ok(date);
        }
    }
    let weekday = value.strip_prefix("next ").unwrap_or(&value);
    let weekday = match weekday {
        "monday" | "mon" => Some(Weekday::Mon),
        "tuesday" | "tue" => Some(Weekday::Tue),
        "wednesday" | "wed" => Some(Weekday::Wed),
        "thursday" | "thu" => Some(Weekday::Thu),
        "friday" | "fri" => Some(Weekday::Fri),
        "saturday" | "sat" => Some(Weekday::Sat),
        "sunday" | "sun" => Some(Weekday::Sun),
        _ => None,
    };
    if let Some(day) = weekday {
        let mut delta =
            (7 + day.num_days_from_monday() - today.weekday().num_days_from_monday()) % 7;
        if delta == 0 && value.starts_with("next ") {
            delta = 7;
        }
        return today
            .checked_add_days(Days::new(delta.into()))
            .ok_or_else(|| "This date is outside the supported range.".into());
    }
    NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|_| {
        "Use today, tomorrow, yesterday, a weekday, in 2 days, or YYYY-MM-DD before the time."
            .into()
    })
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
    let lower = query.trim().to_ascii_lowercase();
    let text = lower
        .strip_prefix("time ")
        .map(|text| text.trim().strip_prefix("in ").unwrap_or(text.trim()))
        .unwrap_or(&lower);
    if text.is_empty() || text == "time" {
        return Err("Try time in Tokyo or tomorrow 3pm London.".into());
    }
    let parts: Vec<_> = text.split_whitespace().collect();
    // A date's day count is not a clock. Prefer explicit clock tokens before bare hours.
    let explicit = parts.iter().position(|part| {
        clock(part).is_some()
            && (part.contains(':')
                || part.ends_with("am")
                || part.ends_with("pm")
                || matches!(*part, "noon" | "midnight"))
    });
    let clock_index = explicit.or_else(|| {
        parts
            .iter()
            .enumerate()
            .find(|(index, part)| {
                clock(part).is_some()
                    && parts
                        .get(index + 1)
                        .is_none_or(|next| !matches!(*next, "day" | "days"))
            })
            .map(|(index, _)| index)
    });
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
            let converted = local(source.with_timezone(&Utc));
            let source_text = format!(
                "{} · {} · UTC{}",
                source.format("%a, %d %b %Y · %H:%M"),
                city(zone),
                source.format("%:z")
            );
            let local_text = format!(
                "{} · UTC{}",
                converted.format("%a, %d %b %Y · %H:%M"),
                converted.format("%:z")
            );
            output.push(TimeResult {
                title: if selected_time.is_some() {
                    format!("{} · your local time", converted.format("%H:%M"))
                } else {
                    format!("{} · {}", source.format("%H:%M"), city(zone))
                },
                subtitle: if selected_time.is_some() {
                    format!(
                        "{} · {}{}",
                        source_text,
                        converted.format("%d %b locally"),
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
                copy: if selected_time.is_some() {
                    local_text.clone()
                } else {
                    source_text.clone()
                },
                detail: ToolDetail::Timezone {
                    source: source_text,
                    local: local_text,
                    source_zone: zone.name().into(),
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
    }
}
