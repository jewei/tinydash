//! The widget pane: what the launcher shows next to an empty All search.
//! Each widget is `None` while its switch in Settings is off.

use serde::Serialize;
use ts_rs::TS;

use crate::{
    error::{Error, Result},
    features::{
        clip_card::{self, ClipCard},
        datetime::{self, CityClock},
        focus::FocusTimer,
        weather::WeatherView,
    },
    platform,
    refresh::WeatherFailure,
    settings::Settings,
    state::State,
};

/// Below this share of free space, the disk widget warns.
const LOW_DISK_PERCENT: u64 = 10;
/// The longest scratch note, in characters. The launcher's field stops here too.
pub const MAX_NOTE_CHARS: usize = 10_000;

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Widgets {
    /// The cities next to local time. A city that names no time zone, as a
    /// hand-edited file may hold, is left out.
    pub clocks: Option<Vec<CityClock>>,
    /// The disk that holds the home folder.
    pub disk: Option<Disk>,
    /// The scratch note.
    pub note: Option<String>,
    pub focus: Option<FocusTimer>,
    pub weather: Option<WeatherView>,
    /// A card for the clipboard text, when it is a format a card shows.
    pub clip_card: Option<ClipCard>,
}

#[derive(Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Disk {
    Ready {
        name: String,
        total_bytes: u64,
        free_bytes: u64,
        /// Less than a tenth is free.
        low: bool,
    },
    /// The OS did not report the disk; the message says why.
    Unavailable { message: String },
}

impl Disk {
    fn read(home: &std::path::Path) -> Self {
        match platform::disk_space(home) {
            Ok(volume) => Self::Ready {
                low: volume.free_bytes.saturating_mul(100)
                    < volume.total_bytes.saturating_mul(LOW_DISK_PERCENT),
                name: volume.name,
                total_bytes: volume.total_bytes,
                free_bytes: volume.free_bytes,
            },
            Err(error) => Self::Unavailable {
                message: error.to_string(),
            },
        }
    }
}

/// Save the scratch note, refusing one over the limit.
pub fn save_note(state: &State, text: &str) -> Result<()> {
    if text.chars().count() > MAX_NOTE_CHARS {
        return Err(Error::msg(format!(
            "The note holds up to {MAX_NOTE_CHARS} characters. Shorten it to save it."
        )));
    }
    state.store.save_note(text)
}

/// Gather the widgets that are on. One that cannot load says so in its
/// card, so the others still show. `clipboard` reads the clipboard text,
/// only when clipboard cards are on.
pub fn load(state: &State, clipboard: impl FnOnce() -> Option<String>) -> Result<Widgets> {
    let settings = state.settings.get();
    let now = chrono::Local::now();
    Ok(Widgets {
        clocks: settings.show_clocks.then(|| {
            settings
                .clock_cities
                .iter()
                .filter_map(|city| datetime::city_clock(city, now))
                .collect()
        }),
        disk: settings
            .show_disk_space
            .then(|| Disk::read(&state.dirs.home)),
        note: settings
            .show_notepad
            .then(|| state.store.note())
            .transpose()?,
        focus: settings.show_focus_timer.then(|| state.focus.get()),
        weather: settings.show_weather.then(|| weather(state, &settings)),
        clip_card: settings
            .show_clipboard_cards
            .then(clipboard)
            .flatten()
            .and_then(|text| clip_card::detect(&text, now)),
    })
}

/// The latest weather for the city, or how getting it is going.
fn weather(state: &State, settings: &Settings) -> WeatherView {
    let city = settings.weather_city.trim().to_owned();
    if city.is_empty() {
        return WeatherView::NoCity;
    }
    let failure = state.freshness.weather_failure(&city);
    if let Some(weather) = state
        .weather
        .get()
        .as_ref()
        .as_ref()
        .filter(|w| w.is_for(&city))
    {
        return WeatherView::ready(weather, settings.temperature_unit, failure.is_some());
    }
    match failure {
        None => WeatherView::Loading { city },
        Some(WeatherFailure::NotFound) => WeatherView::NotFound { city },
        Some(WeatherFailure::Failed(message)) => WeatherView::Failed { city, message },
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::settings::Settings;

    #[test]
    fn a_widget_that_is_off_sends_nothing() {
        let cities = ["Tokyo", "Atlantis"].map(String::from).into();
        let on = State::for_tests(Settings {
            clock_cities: cities,
            ..Settings::default()
        });
        let names: Vec<String> = load(&on, || None)
            .unwrap()
            .clocks
            .unwrap()
            .into_iter()
            .map(|clock| clock.name)
            .collect();
        assert_eq!(names, ["Tokyo"]);

        let off = State::for_tests(Settings {
            show_clocks: false,
            ..Settings::default()
        });
        assert!(load(&off, || None).unwrap().clocks.is_none());
    }

    #[test]
    fn saves_a_note_up_to_the_limit() {
        let state = State::for_tests(Settings {
            show_notepad: true,
            ..Settings::default()
        });
        assert_eq!(load(&state, || None).unwrap().note.as_deref(), Some(""));
        let longest = "é".repeat(MAX_NOTE_CHARS);
        save_note(&state, &longest).unwrap();
        assert!(save_note(&state, &format!("{longest}!")).is_err());
        assert_eq!(load(&state, || None).unwrap().note, Some(longest));
    }

    #[test]
    fn reports_the_disk_of_the_home_folder() {
        let home = std::env::temp_dir();
        let Disk::Ready {
            total_bytes,
            free_bytes,
            name,
            ..
        } = Disk::read(&home)
        else {
            panic!("the temporary folder has a disk");
        };
        assert!(total_bytes > 0 && free_bytes <= total_bytes, "{name}");
        for missing in [home.join("tinydash-missing-folder"), PathBuf::new()] {
            assert!(matches!(Disk::read(&missing), Disk::Unavailable { .. }));
        }
    }

    #[test]
    fn weather_shows_only_for_the_city_in_the_settings() {
        use crate::features::weather::{TemperatureUnit, Weather};
        let settings = |city: &str| Settings {
            show_weather: true,
            weather_city: city.into(),
            ..Settings::default()
        };
        let state = State::for_tests(settings(""));
        assert_eq!(weather(&state, &settings("")), WeatherView::NoCity);
        assert_eq!(
            weather(&state, &settings("Oslo")),
            WeatherView::Loading {
                city: "Oslo".into()
            }
        );

        let oslo = Weather {
            city: "oslo".into(),
            place: "Oslo".into(),
            temperature: 4.4,
            high: 7.0,
            low: 1.0,
            rain_chance: None,
            code: 3,
            is_day: true,
            fetched_at: 0,
        };
        state.weather.set(Some(oslo.clone()));
        assert_eq!(
            weather(&state, &settings("Oslo")),
            WeatherView::ready(&oslo, TemperatureUnit::Celsius, false)
        );
        assert!(matches!(
            weather(&state, &settings("Bergen")),
            WeatherView::Loading { .. }
        ));
    }

    #[test]
    fn reads_the_clipboard_only_for_its_cards() {
        let on = State::for_tests(Settings {
            show_clipboard_cards: true,
            ..Settings::default()
        });
        let card = load(&on, || Some("#2F6F5E".into())).unwrap().clip_card;
        assert_eq!(card.unwrap().actions[0].label, "Copy HEX");
        assert!(
            load(&on, || Some("hello".into()))
                .unwrap()
                .clip_card
                .is_none()
        );

        let off = State::for_tests(Settings::default());
        let read = load(&off, || {
            panic!("cards are off, so nothing reads the clipboard")
        });
        assert!(read.unwrap().clip_card.is_none());
    }
}
