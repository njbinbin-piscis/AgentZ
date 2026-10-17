//! Durable local trigger occurrences. No scheduler or external action dispatch is performed here.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissedWindow {
    Skip,
    Latest,
}

impl MissedWindow {
    fn as_str(self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Latest => "latest",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub id: String,
    pub task_id: String,
    pub interval_ms: i64,
    pub next_due_ms: i64,
    pub missed: MissedWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub trigger_id: String,
    pub generation: i64,
    pub due_ms: i64,
    pub task_id: String,
}

impl Occurrence {
    /// Stable AZ-025 action key for this exact trigger generation and due slot.
    /// Length prefixes avoid ambiguity when trigger IDs contain separators.
    pub fn action_key(&self) -> String {
        let mut hash = Sha256::new();
        for value in [&self.trigger_id, &self.task_id] {
            hash.update((value.len() as u64).to_be_bytes());
            hash.update(value.as_bytes());
        }
        hash.update(self.generation.to_be_bytes());
        hash.update(self.due_ms.to_be_bytes());
        format!("automation:{:x}", hash.finalize())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnresolvedState {
    Claimed,
    Dispatched,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedOccurrence {
    pub occurrence: Occurrence,
    pub state: UnresolvedState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    Succeeded,
    Failed,
    Unknown,
}

impl DispatchOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

pub struct TriggerStore {
    conn: Connection,
}

impl TriggerStore {
    pub fn open(project: &Path) -> Result<Self, String> {
        let dir = project.join(".agentz");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let conn = Connection::open(dir.join("automation.db")).map_err(|e| e.to_string())?;
        conn.busy_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS triggers (
            id TEXT PRIMARY KEY NOT NULL, task_id TEXT NOT NULL, generation INTEGER NOT NULL,
            enabled INTEGER NOT NULL, interval_ms INTEGER NOT NULL, initial_due_ms INTEGER NOT NULL,
            next_due_ms INTEGER NOT NULL,
            missed TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS occurrences (
            trigger_id TEXT NOT NULL, generation INTEGER NOT NULL, due_ms INTEGER NOT NULL,
            task_id TEXT NOT NULL, state TEXT NOT NULL,
            PRIMARY KEY(trigger_id,generation,due_ms));",
        )
        .map_err(|e| e.to_string())?;
        Ok(Self { conn })
    }

    pub fn active(&self, id: &str) -> Result<Option<Trigger>, String> {
        let row: Option<(String, i64, i64, String)> = self.conn.query_row(
            "SELECT task_id,interval_ms,next_due_ms,missed FROM triggers WHERE id=?1 AND enabled=1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).optional().map_err(|e| e.to_string())?;
        row.map(|(task_id, interval_ms, next_due_ms, missed)| {
            let missed = match missed.as_str() {
                "skip" => MissedWindow::Skip,
                "latest" => MissedWindow::Latest,
                _ => return Err("unknown missed-window policy".into()),
            };
            Ok(Trigger {
                id: id.into(),
                task_id,
                interval_ms,
                next_due_ms,
                missed,
            })
        })
        .transpose()
    }

    /// Register is idempotent for an unchanged trigger. A changed definition creates
    /// a new generation so old occurrences can never authorize new dispatch.
    pub fn register(&mut self, trigger: &Trigger) -> Result<i64, String> {
        if trigger.id.is_empty()
            || trigger.task_id.is_empty()
            || trigger.interval_ms <= 0
            || trigger.next_due_ms < 0
        {
            return Err("invalid automation trigger".into());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let prior: Option<(String, i64, i64, i64, String, i64)> = tx.query_row(
            "SELECT task_id,generation,interval_ms,initial_due_ms,missed,enabled FROM triggers WHERE id=?1",
            [&trigger.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        ).optional().map_err(|e| e.to_string())?;
        let generation = match prior {
            Some((task, generation, interval, due, missed, enabled))
                if task == trigger.task_id
                    && interval == trigger.interval_ms
                    && due == trigger.next_due_ms
                    && missed == trigger.missed.as_str()
                    && enabled == 1 =>
            {
                generation
            }
            Some((_, generation, _, _, _, _)) => {
                let next = generation
                    .checked_add(1)
                    .ok_or("trigger generation overflow")?;
                tx.execute("UPDATE triggers SET task_id=?2,generation=?3,enabled=1,interval_ms=?4,initial_due_ms=?5,next_due_ms=?5,missed=?6 WHERE id=?1",
                    params![trigger.id, trigger.task_id, next, trigger.interval_ms, trigger.next_due_ms, trigger.missed.as_str()]).map_err(|e| e.to_string())?;
                next
            }
            None => {
                tx.execute(
                    "INSERT INTO triggers VALUES(?1,?2,1,1,?3,?4,?4,?5)",
                    params![
                        trigger.id,
                        trigger.task_id,
                        trigger.interval_ms,
                        trigger.next_due_ms,
                        trigger.missed.as_str()
                    ],
                )
                .map_err(|e| e.to_string())?;
                1
            }
        };
        tx.commit().map_err(|e| e.to_string())?;
        Ok(generation)
    }

    pub fn disable(&mut self, id: &str) -> Result<(), String> {
        let changed = self
            .conn
            .execute(
                "UPDATE triggers SET enabled=0,generation=generation+1 WHERE id=?1 AND enabled=1",
                [id],
            )
            .map_err(|e| e.to_string())?;
        if changed == 0 {
            return Err("enabled trigger not found".into());
        }
        Ok(())
    }

    /// Claims a due slot once and advances the schedule atomically. Claims left
    /// unknown after a crash are intentionally never replayed automatically.
    pub fn claim_due(&mut self, now_ms: i64) -> Result<Option<Occurrence>, String> {
        if now_ms < 0 {
            return Err("invalid automation clock".into());
        }
        loop {
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
            let row: Option<(String, String, i64, i64, i64, String)> = tx.query_row(
            "SELECT id,task_id,generation,interval_ms,next_due_ms,missed FROM triggers WHERE enabled=1 AND next_due_ms<=?1 ORDER BY next_due_ms,id LIMIT 1",
            [now_ms], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))
        ).optional().map_err(|e| e.to_string())?;
            let Some((id, task_id, generation, interval, next_due, missed)) = row else {
                return Ok(None);
            };
            let elapsed = (now_ms as i128 - next_due as i128) / interval as i128;
            let slot = next_due as i128 + elapsed * interval as i128;
            let following = slot + interval as i128;
            let slot = i64::try_from(slot).map_err(|e| e.to_string())?;
            let following = i64::try_from(following).map_err(|e| e.to_string())?;
            tx.execute(
                "UPDATE triggers SET next_due_ms=?2 WHERE id=?1",
                params![id, following],
            )
            .map_err(|e| e.to_string())?;
            if missed == "skip" && elapsed > 0 {
                tx.commit().map_err(|e| e.to_string())?;
                // A skipped trigger must not hide another trigger due in the same scan.
                continue;
            }
            if missed != "skip" && missed != "latest" {
                return Err("unknown missed-window policy".into());
            }
            let occurrence = Occurrence {
                trigger_id: id,
                generation,
                due_ms: slot,
                task_id,
            };
            tx.execute(
                "INSERT INTO occurrences VALUES(?1,?2,?3,?4,'claimed')",
                params![
                    occurrence.trigger_id,
                    occurrence.generation,
                    occurrence.due_ms,
                    occurrence.task_id
                ],
            )
            .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            return Ok(Some(occurrence));
        }
    }

    /// Just before local dispatch, reject a disabled/redefined trigger or a
    /// previously dispatched claim. External actions still require AZ-025 permits.
    pub fn begin_dispatch(&mut self, occurrence: &Occurrence) -> Result<bool, String> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let changed = tx.execute("UPDATE occurrences SET state='dispatched' WHERE trigger_id=?1 AND generation=?2 AND due_ms=?3 AND task_id=?4 AND state='claimed' AND EXISTS(SELECT 1 FROM triggers WHERE id=?1 AND generation=?2 AND enabled=1)",
            params![occurrence.trigger_id, occurrence.generation, occurrence.due_ms, occurrence.task_id]).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(changed == 1)
    }

    /// Records a terminal observation. A failed or unknown outcome remains
    /// terminal: retrying a side effect requires a new explicit permit.
    pub fn finish_dispatch(
        &mut self,
        occurrence: &Occurrence,
        outcome: DispatchOutcome,
    ) -> Result<bool, String> {
        let changed = self.conn.execute(
            "UPDATE occurrences SET state=?5 WHERE trigger_id=?1 AND generation=?2 AND due_ms=?3 AND task_id=?4 AND state='dispatched'",
            params![occurrence.trigger_id, occurrence.generation, occurrence.due_ms, occurrence.task_id, outcome.as_str()],
        ).map_err(|e| e.to_string())?;
        Ok(changed == 1)
    }

    /// Inspect pre-crash claims and in-flight dispatches for explicit operator
    /// reconciliation. Listing them does not re-arm or retry any occurrence.
    pub fn unresolved(&self) -> Result<Vec<UnresolvedOccurrence>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT trigger_id,generation,due_ms,task_id,state FROM occurrences WHERE state IN ('claimed','dispatched') ORDER BY due_ms,trigger_id,generation",
        ).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let state: String = row.get(4)?;
                Ok(UnresolvedOccurrence {
                    occurrence: Occurrence {
                        trigger_id: row.get(0)?,
                        generation: row.get(1)?,
                        due_ms: row.get(2)?,
                        task_id: row.get(3)?,
                    },
                    state: if state == "claimed" {
                        UnresolvedState::Claimed
                    } else {
                        UnresolvedState::Dispatched
                    },
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("agentz-trigger-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn trigger(missed: MissedWindow) -> Trigger {
        Trigger {
            id: "daily".into(),
            task_id: "task-a".into(),
            interval_ms: 100,
            next_due_ms: 100,
            missed,
        }
    }

    #[test]
    fn restart_and_reregister_do_not_replay_a_dispatched_occurrence() {
        let path = fixture();
        let first = {
            let mut store = TriggerStore::open(&path).unwrap();
            assert_eq!(store.register(&trigger(MissedWindow::Latest)).unwrap(), 1);
            let claim = store.claim_due(350).unwrap().unwrap();
            assert_eq!(claim.due_ms, 300);
            assert!(store.begin_dispatch(&claim).unwrap());
            claim
        };
        let mut store = TriggerStore::open(&path).unwrap();
        assert_eq!(store.register(&trigger(MissedWindow::Latest)).unwrap(), 1);
        assert!(!store.begin_dispatch(&first).unwrap());
        assert!(store.claim_due(350).unwrap().is_none());
        assert_eq!(store.claim_due(400).unwrap().unwrap().due_ms, 400);
        drop(store);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn skipped_windows_are_explained_by_advancing_to_the_next_slot() {
        let path = fixture();
        let mut store = TriggerStore::open(&path).unwrap();
        store.register(&trigger(MissedWindow::Skip)).unwrap();
        assert!(store.claim_due(350).unwrap().is_none());
        assert!(store.claim_due(399).unwrap().is_none());
        assert_eq!(store.claim_due(400).unwrap().unwrap().due_ms, 400);
        drop(store);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn disable_invalidates_a_claim_before_dispatch() {
        let path = fixture();
        let mut store = TriggerStore::open(&path).unwrap();
        store.register(&trigger(MissedWindow::Latest)).unwrap();
        let claim = store.claim_due(100).unwrap().unwrap();
        store.disable("daily").unwrap();
        assert!(!store.begin_dispatch(&claim).unwrap());
        assert!(store.claim_due(100).unwrap().is_none());
        assert_eq!(store.register(&trigger(MissedWindow::Latest)).unwrap(), 3);
        assert!(!store.begin_dispatch(&claim).unwrap());
        drop(store);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn separate_connections_cannot_claim_the_same_slot_twice() {
        let path = fixture();
        let mut a = TriggerStore::open(&path).unwrap();
        let mut b = TriggerStore::open(&path).unwrap();
        a.register(&trigger(MissedWindow::Latest)).unwrap();
        assert!(a.claim_due(100).unwrap().is_some());
        assert!(b.claim_due(100).unwrap().is_none());
        drop(a);
        drop(b);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn terminal_outcomes_cannot_be_replayed_or_overwritten() {
        let path = fixture();
        let mut store = TriggerStore::open(&path).unwrap();
        store.register(&trigger(MissedWindow::Latest)).unwrap();
        let claim = store.claim_due(100).unwrap().unwrap();
        assert!(!store
            .finish_dispatch(&claim, DispatchOutcome::Succeeded)
            .unwrap());
        assert!(store.begin_dispatch(&claim).unwrap());
        assert!(store
            .finish_dispatch(&claim, DispatchOutcome::Unknown)
            .unwrap());
        assert!(!store
            .finish_dispatch(&claim, DispatchOutcome::Succeeded)
            .unwrap());
        assert!(!store.begin_dispatch(&claim).unwrap());
        drop(store);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn restart_surfaces_unresolved_without_rearming_them() {
        let path = fixture();
        let claim = {
            let mut store = TriggerStore::open(&path).unwrap();
            store.register(&trigger(MissedWindow::Latest)).unwrap();
            let claim = store.claim_due(100).unwrap().unwrap();
            assert!(store.begin_dispatch(&claim).unwrap());
            claim
        };
        let mut store = TriggerStore::open(&path).unwrap();
        assert_eq!(
            store.unresolved().unwrap(),
            vec![UnresolvedOccurrence {
                occurrence: claim.clone(),
                state: UnresolvedState::Dispatched,
            }]
        );
        assert!(!store.begin_dispatch(&claim).unwrap());
        assert!(store.claim_due(100).unwrap().is_none());
        assert!(store
            .finish_dispatch(&claim, DispatchOutcome::Unknown)
            .unwrap());
        assert!(store.unresolved().unwrap().is_empty());
        drop(store);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn action_key_is_stable_for_one_occurrence_and_distinct_across_slots() {
        let first = Occurrence {
            trigger_id: "a:b".into(),
            generation: 1,
            due_ms: 100,
            task_id: "task".into(),
        };
        let second = Occurrence {
            due_ms: 200,
            ..first.clone()
        };
        assert_eq!(first.action_key(), first.clone().action_key());
        assert_ne!(first.action_key(), second.action_key());
        assert_ne!(
            first.action_key(),
            Occurrence {
                trigger_id: "a".into(),
                task_id: "b:task".into(),
                ..first.clone()
            }
            .action_key()
        );
    }

    #[test]
    fn skipped_trigger_does_not_starve_another_due_trigger() {
        let path = fixture();
        let mut store = TriggerStore::open(&path).unwrap();
        store.register(&trigger(MissedWindow::Skip)).unwrap();
        store
            .register(&Trigger {
                id: "later".into(),
                task_id: "task-b".into(),
                interval_ms: 100,
                next_due_ms: 200,
                missed: MissedWindow::Latest,
            })
            .unwrap();
        let claim = store.claim_due(350).unwrap().unwrap();
        assert_eq!(claim.trigger_id, "later");
        assert_eq!(claim.due_ms, 300);
        assert!(store.claim_due(350).unwrap().is_none());
        assert!(store.claim_due(-1).is_err());
        drop(store);
        let _ = std::fs::remove_dir_all(path);
    }
}
