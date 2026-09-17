use rusqlite::OptionalExtension;

use super::{Database, Error, Result};
use crate::currency::Rates;

impl Database {
    pub fn load_rates(&self) -> Result<Option<Rates>> {
        let snapshot: Option<String> = self
            .connection
            .query_row(
                "SELECT snapshot FROM currency_rates WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        snapshot
            .map(|json| {
                let rates: Rates = serde_json::from_str(&json)
                    .map_err(|error| Error::Currency(error.to_string()))?;
                rates
                    .validate(crate::ranking::now())
                    .map_err(|error| Error::Currency(error.to_string()))?;
                Ok(rates)
            })
            .transpose()
    }

    pub fn save_rates(&self, rates: &Rates) -> Result<()> {
        rates
            .validate(crate::ranking::now())
            .map_err(|error| Error::Currency(error.to_string()))?;
        let snapshot =
            serde_json::to_string(rates).map_err(|error| Error::Currency(error.to_string()))?;
        self.connection.execute("INSERT INTO currency_rates (id, snapshot) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET snapshot = excluded.snapshot", [snapshot])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_survive_reopen_and_bad_updates_preserve_the_cache() {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("rates.sqlite3");
        let mut rates = crate::currency::fixture();
        rates.fetched_at = crate::ranking::now();
        {
            let db = Database::open(&path).expect("database");
            assert!(db.load_rates().expect("empty").is_none());
            db.save_rates(&rates).expect("save");
            rates.values.insert("USD".into(), f64::NAN);
            assert!(db.save_rates(&rates).is_err());
        }
        let db = Database::open(&path).expect("reopen");
        let saved = db.load_rates().expect("read").expect("cached");
        assert_eq!(saved.rate("USD").expect("USD"), 1.25);
        assert_eq!(saved.date, "2026-09-16");
    }
}
