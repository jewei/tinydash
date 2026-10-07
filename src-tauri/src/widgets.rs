//! The widget pane: what the launcher shows next to an empty All search.
//! Each widget is `None` while its switch in Settings is off.

use serde::Serialize;
use ts_rs::TS;

use crate::{
    error::{Error, Result},
    features::datetime::{self, CityClock},
    platform,
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
/// card, so the others still show.
pub fn load(state: &State) -> Result<Widgets> {
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
    })
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
        let names: Vec<String> = load(&on)
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
        assert!(load(&off).unwrap().clocks.is_none());
    }

    #[test]
    fn saves_a_note_up_to_the_limit() {
        let state = State::for_tests(Settings {
            show_notepad: true,
            ..Settings::default()
        });
        assert_eq!(load(&state).unwrap().note.as_deref(), Some(""));
        let longest = "é".repeat(MAX_NOTE_CHARS);
        save_note(&state, &longest).unwrap();
        assert!(save_note(&state, &format!("{longest}!")).is_err());
        assert_eq!(load(&state).unwrap().note, Some(longest));
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
}
