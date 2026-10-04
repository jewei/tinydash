use rusqlite::{OptionalExtension, params};

use super::{Database, Result};
use crate::providers::clipboard::ClipboardEntry;

/// Identity of one capture, not of its text, creation time, or display order.
/// Both fields are durable; recapturing deduplicated content advances revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureRevision {
    pub id: i64,
    pub revision: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupOutcome {
    Removed,
    Missing,
    Recaptured,
    Pinned,
}

fn entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClipboardEntry> {
    Ok(ClipboardEntry {
        id: row.get(0)?,
        content: row.get(1)?,
        created_at: row.get(2)?,
        last_used_at: row.get(3)?,
    })
}

impl Database {
    pub fn load_clipboard(&self) -> Result<Vec<ClipboardEntry>> {
        let mut statement = self.connection.prepare(
            "SELECT id, content, created_at, last_used_at FROM clipboard_history ORDER BY sort_order DESC")?;
        Ok(statement
            .query_map([], entry)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn prune_clipboard(&self, limit: usize) -> Result<Vec<i64>> {
        let mut statement = self.connection.prepare(
            "DELETE FROM clipboard_history WHERE id IN (
                SELECT id FROM clipboard_history WHERE pinned = 0 ORDER BY sort_order DESC LIMIT -1 OFFSET ?1
             ) RETURNING id",
        )?;
        Ok(statement
            .query_map([limit as i64], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Age retention uses the original capture time, never the last copy time.
    /// Pinned history is explicitly exempt. Zero keeps the existing unlimited age.
    pub fn prune_clipboard_retention(&self, days: u32, now: i64) -> Result<Vec<i64>> {
        if days == 0 {
            return Ok(Vec::new());
        }
        let cutoff = now.saturating_sub(i64::from(days) * 86_400);
        let mut statement = self.connection.prepare(
            "DELETE FROM clipboard_history WHERE pinned = 0 AND created_at <= ?1 RETURNING id",
        )?;
        Ok(statement
            .query_map([cutoff], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    #[cfg(test)]
    pub fn capture_clipboard(
        &mut self,
        content: &str,
        now: i64,
        limit: usize,
    ) -> Result<(ClipboardEntry, Vec<i64>)> {
        self.capture_clipboard_with_retention(content, now, limit, 0)
    }

    #[cfg(test)]
    pub fn capture_clipboard_with_retention(
        &mut self,
        content: &str,
        now: i64,
        limit: usize,
        retention_days: u32,
    ) -> Result<(ClipboardEntry, Vec<i64>)> {
        self.capture_clipboard_revision_with_retention(content, now, limit, retention_days)
            .map(|(entry, removed, _)| (entry, removed))
    }

    #[cfg(test)]
    pub fn capture_clipboard_revision(
        &mut self,
        content: &str,
        now: i64,
        limit: usize,
    ) -> Result<(ClipboardEntry, Vec<i64>, CaptureRevision)> {
        self.capture_clipboard_revision_with_retention(content, now, limit, 0)
    }

    pub fn capture_clipboard_revision_with_retention(
        &mut self,
        content: &str,
        now: i64,
        limit: usize,
        retention_days: u32,
    ) -> Result<(ClipboardEntry, Vec<i64>, CaptureRevision)> {
        let transaction = self.connection.transaction()?;
        // Prune before the upsert: re-copying expired text creates a fresh entry
        // rather than immediately removing the entry returned to the index.
        let mut removed = if retention_days == 0 {
            Vec::new()
        } else {
            let mut statement = transaction.prepare(
                "DELETE FROM clipboard_history WHERE pinned = 0 AND created_at <= ?1 RETURNING id",
            )?;
            statement
                .query_map(
                    [now.saturating_sub(i64::from(retention_days) * 86_400)],
                    |row| row.get(0),
                )?
                .collect::<rusqlite::Result<Vec<i64>>>()?
        };
        let (entry, revision) = transaction.query_row(
            "INSERT INTO clipboard_history (content, created_at, sort_order)
             VALUES (?1, ?2, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM clipboard_history))
             ON CONFLICT(content) DO UPDATE SET sort_order = excluded.sort_order,
                 capture_revision = clipboard_history.capture_revision + 1
             RETURNING id, content, created_at, last_used_at, capture_revision",
            params![content, now],
            |row| Ok((entry(row)?, row.get(4)?)),
        )?;
        removed.extend({
            let mut statement = transaction.prepare(
                "DELETE FROM clipboard_history WHERE id IN (
                    SELECT id FROM clipboard_history WHERE pinned = 0 ORDER BY sort_order DESC LIMIT -1 OFFSET ?1
                 ) RETURNING id",
            )?;
            statement
                .query_map([limit as i64], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<i64>>>()?
        });
        transaction.commit()?;
        let capture = CaptureRevision {
            id: entry.id,
            revision,
        };
        Ok((entry, removed, capture))
    }

    pub fn touch_clipboard(&mut self, id: i64, now: i64) -> Result<Option<ClipboardEntry>> {
        Ok(self
            .connection
            .query_row(
                "UPDATE clipboard_history SET last_used_at = ?2,
                sort_order = (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM clipboard_history)
             WHERE id = ?1 RETURNING id, content, created_at, last_used_at",
                params![id, now],
                entry,
            )
            .optional()?)
    }

    pub fn delete_clipboard(&self, id: i64) -> Result<()> {
        self.connection
            .execute("DELETE FROM clipboard_history WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Recheck capture identity and pins under the same write transaction as
    /// deletion. A recapture, including on another connection, wins over a stale
    /// intent. Pinned captures remain pending so unpinning can finish cleanup.
    pub fn cleanup_clipboard(&mut self, capture: CaptureRevision) -> Result<CleanupOutcome> {
        let transaction = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current: Option<(i64, bool)> = transaction
            .query_row(
                "SELECT capture_revision, pinned FROM clipboard_history WHERE id = ?1",
                [capture.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let outcome = match current {
            None => CleanupOutcome::Missing,
            Some((revision, _)) if revision != capture.revision => CleanupOutcome::Recaptured,
            Some((_, true)) => CleanupOutcome::Pinned,
            Some((_, false)) => {
                transaction.execute(
                    "DELETE FROM clipboard_history WHERE id = ?1 AND capture_revision = ?2 AND pinned = 0",
                    params![capture.id, capture.revision],
                )?;
                CleanupOutcome::Removed
            }
        };
        transaction.commit()?;
        Ok(outcome)
    }

    pub fn clear_clipboard(&self) -> Result<()> {
        self.connection
            .execute("DELETE FROM clipboard_history", [])?;
        Ok(())
    }

    pub fn clear_unpinned_clipboard(&self) -> Result<Vec<i64>> {
        let mut statement = self
            .connection
            .prepare("DELETE FROM clipboard_history WHERE pinned = 0 RETURNING id")?;
        Ok(statement
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launcher::query::SearchMode;
    use crate::ranking::Usage;

    #[test]
    fn persists_exact_text_deduplicates_and_orders_even_with_equal_or_reversed_clocks() {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("state.sqlite3");
        let original = "  Café 🚀\n  preserved\n";
        let first_id;
        {
            let mut database = Database::open(&path).expect("open");
            first_id = database
                .capture_clipboard(original, 100, 100)
                .expect("first")
                .0
                .id;
            database
                .capture_clipboard("second", 100, 100)
                .expect("second");
            let (entry, removed) = database
                .capture_clipboard(original, 90, 100)
                .expect("recopy");
            assert_eq!(entry.id, first_id);
            assert_eq!(entry.created_at, 100);
            assert!(removed.is_empty());
            assert_eq!(database.load_clipboard().expect("load").len(), 2);
        }
        let mut database = Database::open(&path).expect("reopen");
        let entries = database.load_clipboard().expect("load");
        assert_eq!(entries[0].content, original);
        let used = database
            .touch_clipboard(entries[1].id, 101)
            .expect("touch")
            .expect("entry");
        assert_eq!(used.last_used_at, Some(101));
        assert_eq!(database.load_clipboard().expect("load")[0].id, used.id);
        assert!(
            database
                .touch_clipboard(999, 100)
                .expect("expired")
                .is_none()
        );
    }

    #[test]
    fn age_retention_preserves_pins_and_zero_means_unlimited() {
        let directory = tempfile::tempdir().unwrap();
        let mut db = Database::open(&directory.path().join("history.sqlite3")).unwrap();
        let old = db.capture_clipboard("old", 1, 100).unwrap().0;
        let pinned = db.capture_clipboard("pinned", 1, 100).unwrap().0;
        let recent = db.capture_clipboard("recent", 86_402, 100).unwrap().0;
        db.set_pinned(
            &format!("clipboard:{}", pinned.id),
            SearchMode::Clipboard,
            true,
        )
        .unwrap();
        assert!(
            db.prune_clipboard_retention(0, i64::MAX)
                .unwrap()
                .is_empty()
        );
        assert!(
            db.prune_clipboard_retention(u32::MAX, 0)
                .unwrap()
                .is_empty()
        );
        db.touch_clipboard(old.id, 86_400).unwrap();
        assert_eq!(db.prune_clipboard_retention(1, 86_401).unwrap(), [old.id]);
        let ids: Vec<_> = db
            .load_clipboard()
            .unwrap()
            .iter()
            .map(|entry| entry.id)
            .collect();
        assert!(ids.contains(&pinned.id));
        assert!(ids.contains(&recent.id));
        assert_eq!(db.load_pins().unwrap().len(), 1);
    }

    #[test]
    fn recapture_expired_text_is_fresh_and_failed_capture_rolls_back_pruning() {
        let directory = tempfile::tempdir().unwrap();
        let mut db = Database::open(&directory.path().join("history.sqlite3")).unwrap();
        let old = db.capture_clipboard("old", 1, 100).unwrap().0;
        assert!(
            db.capture_clipboard_with_retention(&"x".repeat(16_385), 86_401, 100, 1)
                .is_err()
        );
        assert_eq!(db.load_clipboard().unwrap()[0].id, old.id);
        let (fresh, removed) = db
            .capture_clipboard_with_retention("old", 86_401, 100, 1)
            .unwrap();
        assert_ne!(fresh.id, old.id);
        assert_eq!(fresh.created_at, 86_401);
        assert_eq!(removed, [old.id]);
    }

    #[test]
    fn capture_revision_ignores_touches_and_survives_reopen_order_reuse_and_clock_reversal() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.sqlite3");
        let mut database = Database::open(&path).unwrap();
        let (entry, _, first) = database
            .capture_clipboard_revision("synthetic", 100, 100)
            .unwrap();
        database.touch_clipboard(first.id, 101).unwrap();
        database
            .set_pinned(
                &format!("clipboard:{}", first.id),
                SearchMode::Clipboard,
                true,
            )
            .unwrap();
        let revision: i64 = database
            .connection
            .query_row(
                "SELECT capture_revision FROM clipboard_history WHERE id = ?1",
                [first.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(revision, first.revision);
        drop(database);
        let mut database = Database::open(&path).unwrap();
        // sort_order can be reused and wall clocks can go backwards; neither
        // defines identity. Use the production capture write after both changes.
        database
            .connection
            .execute("UPDATE clipboard_history SET sort_order = 0", [])
            .unwrap();
        let (recaptured, _, second) = database
            .capture_clipboard_revision("synthetic", 90, 100)
            .unwrap();
        assert_eq!(recaptured.id, entry.id);
        assert_eq!(recaptured.created_at, entry.created_at);
        assert_eq!(second.revision, first.revision + 1);
        assert_eq!(
            database.cleanup_clipboard(first).unwrap(),
            CleanupOutcome::Recaptured
        );
        assert_eq!(
            database.cleanup_clipboard(second).unwrap(),
            CleanupOutcome::Pinned
        );
        database
            .set_pinned(
                &format!("clipboard:{}", first.id),
                SearchMode::Clipboard,
                false,
            )
            .unwrap();
        assert_eq!(
            database.cleanup_clipboard(second).unwrap(),
            CleanupOutcome::Removed
        );
        let (_, _, recreated) = database
            .capture_clipboard_revision("synthetic", 100, 100)
            .unwrap();
        assert!(recreated.id > first.id);
        assert_eq!(
            database.cleanup_clipboard(second).unwrap(),
            CleanupOutcome::Missing
        );
    }

    #[test]
    fn exhausted_revision_fails_capture_without_wrapping_or_changing_saved_text() {
        let directory = tempfile::tempdir().unwrap();
        let mut database = Database::open(&directory.path().join("state.sqlite3")).unwrap();
        let (_, _, capture) = database
            .capture_clipboard_revision("synthetic", 100, 100)
            .unwrap();
        database
            .connection
            .execute(
                "UPDATE clipboard_history SET capture_revision = ?1 WHERE id = ?2",
                params![i64::MAX, capture.id],
            )
            .unwrap();
        assert!(
            database
                .capture_clipboard_revision("synthetic", 101, 100)
                .is_err()
        );
        let revision: i64 = database
            .connection
            .query_row(
                "SELECT capture_revision FROM clipboard_history WHERE id = ?1",
                [capture.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(revision, i64::MAX);
        assert_eq!(database.load_clipboard().unwrap()[0].content, "synthetic");
    }

    #[test]
    fn locked_delete_and_clear_preserve_entries() {
        let directory = tempfile::tempdir().expect("directory");
        let path = directory.path().join("state.sqlite3");
        let mut database = Database::open(&path).expect("open");
        let id = database
            .capture_clipboard("kept", 1, 100)
            .expect("capture")
            .0
            .id;
        let other = rusqlite::Connection::open(&path).expect("other connection");
        other.execute_batch("BEGIN IMMEDIATE").expect("lock");
        assert!(database.delete_clipboard(id).is_err());
        assert!(database.clear_clipboard().is_err());
        other.execute_batch("ROLLBACK").expect("unlock");
        assert_eq!(database.load_clipboard().expect("load")[0].content, "kept");
    }

    #[test]
    fn caps_deletes_and_clears_without_reusing_ids_or_removing_usage() {
        let directory = tempfile::tempdir().expect("directory");
        let mut database = Database::open(&directory.path().join("state.sqlite3")).expect("open");
        database
            .save_usage(
                "emoji:🚀",
                Usage {
                    count: 1,
                    last_used_at: 1,
                },
            )
            .expect("usage");
        let first = database
            .capture_clipboard("first", 1, 2)
            .expect("first")
            .0
            .id;
        database.capture_clipboard("second", 1, 2).expect("second");
        let (third, removed) = database
            .capture_clipboard("'; DROP TABLE clipboard_history; --", 1, 2)
            .expect("third");
        assert_eq!(removed, [first]);
        database.delete_clipboard(third.id).expect("delete");
        assert_eq!(database.load_clipboard().expect("load").len(), 1);
        database.clear_clipboard().expect("clear");
        assert!(database.load_clipboard().expect("load").is_empty());
        assert_eq!(database.load_usage().expect("usage").len(), 1);
        let next = database.capture_clipboard("next", 1, 100).expect("next").0;
        assert!(next.id > third.id, "stale UI IDs cannot refer to new text");
        database.capture_clipboard("last", 1, 100).expect("last");
        assert_eq!(database.prune_clipboard(1).expect("lower limit"), [next.id]);
        assert!(
            database
                .capture_clipboard(&"a".repeat(16_385), 1, 100)
                .is_err()
        );
        assert_eq!(database.load_clipboard().expect("load").len(), 1);
    }

    #[test]
    fn deleting_a_cleared_capture_keeps_a_pinned_entry() {
        let directory = tempfile::tempdir().expect("directory");
        let mut database = Database::open(&directory.path().join("state.sqlite3")).expect("open");
        let pinned = database
            .capture_clipboard_revision("pinned", 1, 100)
            .expect("pinned")
            .2;
        let cleared = database
            .capture_clipboard_revision("cleared", 2, 100)
            .expect("cleared")
            .2;
        database
            .set_pinned(&format!("clipboard:{}", pinned.id), SearchMode::All, true)
            .expect("pin");

        assert_eq!(
            database.cleanup_clipboard(pinned).unwrap(),
            CleanupOutcome::Pinned
        );
        assert_eq!(
            database.cleanup_clipboard(cleared).unwrap(),
            CleanupOutcome::Removed
        );
        assert_eq!(
            database.cleanup_clipboard(cleared).unwrap(),
            CleanupOutcome::Missing
        );
        assert_eq!(
            database
                .load_clipboard()
                .expect("load")
                .into_iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            [pinned.id]
        );
    }

    #[test]
    fn clear_unpinned_preserves_pins_in_all_and_clipboard() {
        let directory = tempfile::tempdir().expect("directory");
        let mut database = Database::open(&directory.path().join("state.sqlite3")).expect("open");
        let all = database.capture_clipboard("all", 1, 100).expect("all").0;
        let clipboard = database
            .capture_clipboard("clipboard", 2, 100)
            .expect("clipboard")
            .0;
        let removed = database
            .capture_clipboard("removed", 3, 100)
            .expect("removed")
            .0;
        database
            .set_pinned(&format!("clipboard:{}", all.id), SearchMode::All, true)
            .expect("all pin");
        database
            .set_pinned(
                &format!("clipboard:{}", clipboard.id),
                SearchMode::Clipboard,
                true,
            )
            .expect("clipboard pin");

        assert_eq!(
            database.clear_unpinned_clipboard().expect("clear"),
            [removed.id]
        );
        assert_eq!(
            database
                .load_clipboard()
                .expect("load")
                .into_iter()
                .map(|entry| entry.id)
                .collect::<Vec<_>>(),
            [clipboard.id, all.id]
        );
        assert_eq!(
            database.load_pins().expect("pins").len(),
            2,
            "clear must keep both pin categories"
        );
    }
}
