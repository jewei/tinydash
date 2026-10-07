//! The widget pane: what the launcher shows next to an empty All search.
//! Each widget is `None` while its switch in Settings is off.

use serde::Serialize;
use ts_rs::TS;

use crate::{
    features::datetime::{self, CityClock},
    state::State,
};

#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Widgets {
    /// The cities next to local time. A city that names no time zone, as a
    /// hand-edited file may hold, is left out.
    pub clocks: Option<Vec<CityClock>>,
}

pub fn load(state: &State) -> Widgets {
    let settings = state.settings.get();
    let now = chrono::Local::now();
    Widgets {
        clocks: settings.show_clocks.then(|| {
            settings
                .clock_cities
                .iter()
                .filter_map(|city| datetime::city_clock(city, now))
                .collect()
        }),
    }
}

#[cfg(test)]
mod tests {
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
        assert!(load(&off).clocks.is_none());
    }
}
