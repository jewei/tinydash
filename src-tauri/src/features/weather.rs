//! The weather widget: current conditions for one city from Open-Meteo,
//! which needs no account. The city name goes to its geocoding service,
//! and the city's coordinates to its forecast service; nothing else does.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use url::Url;

use crate::{
    error::{Error, Result},
    features::download,
};

const GEOCODING: &str = "https://geocoding-api.open-meteo.com/v1/search";
const FORECAST: &str = "https://api.open-meteo.com/v1/forecast";
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;
/// Weather older than this is downloaded again when the launcher opens.
pub const MAX_AGE_SECONDS: i64 = 30 * 60;
/// The longest city name the setting keeps.
pub const MAX_CITY_CHARS: usize = 100;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

/// Downloaded weather, cached in the database for offline use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    /// The city as the setting names it, so a new city is downloaded again.
    pub city: String,
    /// The place Open-Meteo found, such as `Singapore`.
    pub place: String,
    pub temperature: f64,
    pub high: f64,
    pub low: f64,
    pub rain_chance: Option<u8>,
    /// The WMO weather code.
    pub code: u8,
    pub is_day: bool,
    /// Unix seconds of the download.
    pub fetched_at: i64,
}

impl Weather {
    pub fn is_for(&self, city: &str) -> bool {
        self.city.to_lowercase() == city.trim().to_lowercase()
    }

    pub fn is_stale(&self, now: i64) -> bool {
        now - self.fetched_at >= MAX_AGE_SECONDS
    }
}

/// The icons the launcher draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WeatherIcon {
    ClearDay,
    ClearNight,
    PartlyDay,
    PartlyNight,
    Cloudy,
    Fog,
    Rain,
    Thunder,
    Snow,
}

/// What the weather card shows.
#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum WeatherView {
    /// No city is set.
    NoCity,
    Loading {
        city: String,
    },
    Ready {
        place: String,
        /// Whole degrees in the chosen unit.
        temperature: i32,
        high: i32,
        low: i32,
        unit: TemperatureUnit,
        condition: String,
        icon: WeatherIcon,
        rain_chance: Option<u8>,
        /// Unix seconds of the download.
        updated_at: i64,
        /// The latest download failed, so this is older weather.
        offline: bool,
    },
    NotFound {
        city: String,
    },
    Failed {
        city: String,
        message: String,
    },
}

impl WeatherView {
    pub fn ready(weather: &Weather, unit: TemperatureUnit, offline: bool) -> Self {
        let degrees = |celsius: f64| {
            let value = match unit {
                TemperatureUnit::Celsius => celsius,
                TemperatureUnit::Fahrenheit => celsius * 9.0 / 5.0 + 32.0,
            };
            // Temperatures on Earth fit an i32 by far.
            value.round() as i32
        };
        let (condition, icon) = describe(weather.code, weather.is_day);
        Self::Ready {
            place: weather.place.clone(),
            temperature: degrees(weather.temperature),
            high: degrees(weather.high),
            low: degrees(weather.low),
            unit,
            condition: condition.into(),
            icon,
            rain_chance: weather.rain_chance,
            updated_at: weather.fetched_at,
            offline,
        }
    }
}

/// Words and an icon for a WMO weather code, as Open-Meteo documents them.
fn describe(code: u8, is_day: bool) -> (&'static str, WeatherIcon) {
    use WeatherIcon::*;
    let (clear, partly) = if is_day {
        (ClearDay, PartlyDay)
    } else {
        (ClearNight, PartlyNight)
    };
    match code {
        0 => ("Clear", clear),
        1 => ("Mostly clear", partly),
        2 => ("Partly cloudy", partly),
        3 => ("Cloudy", Cloudy),
        45 | 48 => ("Fog", Fog),
        51 | 53 | 55 => ("Drizzle", Rain),
        56 | 57 => ("Freezing drizzle", Rain),
        61 => ("Light rain", Rain),
        63 => ("Rain", Rain),
        65 => ("Heavy rain", Rain),
        66 | 67 => ("Freezing rain", Rain),
        71 => ("Light snow", Snow),
        73 | 77 => ("Snow", Snow),
        75 => ("Heavy snow", Snow),
        80..=82 => ("Rain showers", Rain),
        85 | 86 => ("Snow showers", Snow),
        95 => ("Thunderstorm", Thunder),
        96 | 99 => ("Thunderstorm with hail", Thunder),
        _ => ("Unknown", Cloudy),
    }
}

/// The place a geocoding search found first, or `None` when it found none.
fn parse_place(json: &[u8]) -> Result<Option<(String, f64, f64)>> {
    #[derive(Deserialize)]
    struct Response {
        #[serde(default)]
        results: Vec<Place>,
    }
    #[derive(Deserialize)]
    struct Place {
        name: String,
        latitude: f64,
        longitude: f64,
    }
    let response: Response = serde_json::from_slice(json)?;
    Ok(response.results.into_iter().next().and_then(|place| {
        let valid =
            (-90.0..=90.0).contains(&place.latitude) && (-180.0..=180.0).contains(&place.longitude);
        valid.then_some((place.name, place.latitude, place.longitude))
    }))
}

fn parse_forecast(json: &[u8], city: &str, place: String, now: i64) -> Result<Weather> {
    #[derive(Deserialize)]
    struct Response {
        current: Current,
        daily: Daily,
    }
    #[derive(Deserialize)]
    struct Current {
        temperature_2m: f64,
        weather_code: u8,
        is_day: u8,
    }
    #[derive(Deserialize)]
    struct Daily {
        temperature_2m_max: Vec<f64>,
        temperature_2m_min: Vec<f64>,
        precipitation_probability_max: Vec<Option<f64>>,
    }
    let response: Response = serde_json::from_slice(json)?;
    let today = |values: &[f64]| {
        values
            .first()
            .copied()
            .filter(|value| value.is_finite())
            .ok_or_else(|| Error::msg("The weather service sent unexpected data."))
    };
    let current = response.current;
    if !current.temperature_2m.is_finite() {
        return Err(Error::msg("The weather service sent unexpected data."));
    }
    Ok(Weather {
        city: city.trim().to_owned(),
        place,
        temperature: current.temperature_2m,
        high: today(&response.daily.temperature_2m_max)?,
        low: today(&response.daily.temperature_2m_min)?,
        rain_chance: response
            .daily
            .precipitation_probability_max
            .first()
            .copied()
            .flatten()
            // A percentage, so the cast cannot lose more than the fraction.
            .map(|chance| chance.clamp(0.0, 100.0).round() as u8),
        code: current.weather_code,
        is_day: current.is_day != 0,
        fetched_at: now,
    })
}

/// Find `city` and download its weather: `None` when no place has that
/// name. Blocks for up to twenty seconds.
pub fn fetch(city: &str, now: i64) -> Result<Option<Weather>> {
    let search = Url::parse_with_params(
        GEOCODING,
        [("name", city.trim()), ("count", "1"), ("format", "json")],
    )
    .map_err(|error| Error::msg(error.to_string()))?;
    let found = download::get(search.as_str(), MAX_RESPONSE_BYTES, "the weather")?;
    let Some((place, latitude, longitude)) = parse_place(&found)? else {
        return Ok(None);
    };
    let forecast = Url::parse_with_params(
        FORECAST,
        [
            ("latitude", latitude.to_string().as_str()),
            ("longitude", longitude.to_string().as_str()),
            ("current", "temperature_2m,weather_code,is_day"),
            (
                "daily",
                "temperature_2m_max,temperature_2m_min,precipitation_probability_max",
            ),
            ("timezone", "auto"),
            ("forecast_days", "1"),
        ],
    )
    .map_err(|error| Error::msg(error.to_string()))?;
    let body = download::get(forecast.as_str(), MAX_RESPONSE_BYTES, "the weather")?;
    parse_forecast(&body, city, place, now).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORECAST_JSON: &str = r#"{"latitude":1.29,"longitude":103.85,
        "current":{"time":"2026-10-07T15:00","temperature_2m":31.4,"weather_code":2,"is_day":1},
        "daily":{"time":["2026-10-07"],"temperature_2m_max":[33.2],"temperature_2m_min":[25.6],
        "precipitation_probability_max":[40]}}"#;

    fn singapore() -> Weather {
        parse_forecast(
            FORECAST_JSON.as_bytes(),
            " singapore ",
            "Singapore".into(),
            100,
        )
        .unwrap()
    }

    #[test]
    fn reads_a_place_and_its_forecast() {
        let found = br#"{"results":[{"id":1880252,"name":"Singapore","latitude":1.28967,"longitude":103.85007,"country":"Singapore"}]}"#;
        assert_eq!(
            parse_place(found).unwrap(),
            Some(("Singapore".into(), 1.28967, 103.85007))
        );
        assert_eq!(parse_place(br#"{"generationtime_ms":0.5}"#).unwrap(), None);
        assert!(parse_place(b"<html>").is_err());

        let weather = singapore();
        assert_eq!(weather.city, "singapore");
        assert!(weather.is_for("Singapore") && !weather.is_for("Tokyo"));
        assert_eq!((weather.high, weather.low), (33.2, 25.6));
        assert_eq!(weather.rain_chance, Some(40));
        assert!(!weather.is_stale(100 + MAX_AGE_SECONDS - 1));
        assert!(weather.is_stale(100 + MAX_AGE_SECONDS));

        let missing_day = FORECAST_JSON.replace("[33.2]", "[]");
        assert!(parse_forecast(missing_day.as_bytes(), "x", "X".into(), 0).is_err());
        let no_rain = FORECAST_JSON.replace("[40]", "[null]");
        let weather = parse_forecast(no_rain.as_bytes(), "x", "X".into(), 0).unwrap();
        assert_eq!(weather.rain_chance, None);
    }

    #[test]
    fn shows_whole_degrees_in_the_chosen_unit() {
        let WeatherView::Ready {
            temperature,
            high,
            condition,
            icon,
            ..
        } = WeatherView::ready(&singapore(), TemperatureUnit::Fahrenheit, false)
        else {
            unreachable!()
        };
        assert_eq!((temperature, high), (89, 92));
        assert_eq!(
            (condition.as_str(), icon),
            ("Partly cloudy", WeatherIcon::PartlyDay)
        );
        assert_eq!(describe(0, false), ("Clear", WeatherIcon::ClearNight));
        assert_eq!(describe(96, true).1, WeatherIcon::Thunder);
        assert_eq!(describe(200, true).0, "Unknown");
    }
}
