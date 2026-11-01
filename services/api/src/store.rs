//! Optional PostgreSQL storage for synthetic demo event streams.
//! Resets advance a generation; prior events remain in the database.
use record_core::{evolve, hash_event, Event, State};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};

use crate::fresh;

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

#[derive(Debug)]
pub enum StoreError {
    Database(sqlx::Error),
    Corrupt,
    Conflict,
}
impl From<sqlx::Error> for StoreError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}
impl StoreError {
    pub fn status(&self) -> axum::http::StatusCode {
        match self {
            Self::Conflict => axum::http::StatusCode::CONFLICT,
            Self::Database(error) => {
                eprintln!("demo storage unavailable: {error}");
                axum::http::StatusCode::SERVICE_UNAVAILABLE
            }
            Self::Corrupt => {
                eprintln!("demo event stream failed integrity checks");
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

impl Store {
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new().max_connections(8).connect(url).await?;
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(481715902)")
            .execute(&mut *tx)
            .await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS hr_demo_streams (session_id TEXT PRIMARY KEY, generation BIGINT NOT NULL DEFAULT 0, sequence BIGINT NOT NULL DEFAULT 0)").execute(&mut *tx).await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS hr_demo_events (session_id TEXT NOT NULL REFERENCES hr_demo_streams(session_id), generation BIGINT NOT NULL, sequence BIGINT NOT NULL, event JSONB NOT NULL, PRIMARY KEY (session_id, generation, sequence))").execute(&mut *tx).await?;
        sqlx::query("CREATE OR REPLACE FUNCTION hr_demo_prevent_event_mutation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'demo events are append-only'; END $$").execute(&mut *tx).await?;
        sqlx::query("DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'hr_demo_events_append_only' AND tgrelid = 'hr_demo_events'::regclass) THEN CREATE TRIGGER hr_demo_events_append_only BEFORE UPDATE OR DELETE ON hr_demo_events FOR EACH ROW EXECUTE FUNCTION hr_demo_prevent_event_mutation(); END IF; END $$").execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Self { pool })
    }
    pub async fn load(&self, id: &str) -> Result<(i64, State), StoreError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO hr_demo_streams (session_id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query(
            "SELECT generation, sequence FROM hr_demo_streams WHERE session_id = $1 FOR SHARE",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        let generation: i64 = row.try_get("generation")?;
        let sequence: i64 = row.try_get("sequence")?;
        let rows = sqlx::query("SELECT event FROM hr_demo_events WHERE session_id = $1 AND generation = $2 ORDER BY sequence").bind(id).bind(generation).fetch_all(&mut *tx).await?;
        if sequence < 0 || rows.len() != sequence as usize {
            return Err(StoreError::Corrupt);
        }
        let mut state = fresh();
        let mut previous = "genesis".to_string();
        for (index, row) in rows.iter().enumerate() {
            let sqlx::types::Json(event): sqlx::types::Json<Event> = row.try_get("event")?;
            if event.seq != index as u64 + 1
                || event.prev_hash != previous
                || hash_event(&event) != event.hash
            {
                return Err(StoreError::Corrupt);
            }
            previous.clone_from(&event.hash);
            evolve(&mut state, event);
        }
        tx.commit().await?;
        Ok((generation, state))
    }
    pub async fn append(&self, id: &str, generation: i64, event: &Event) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query("UPDATE hr_demo_streams SET sequence = sequence + 1 WHERE session_id = $1 AND generation = $2 AND sequence = $3")
            .bind(id).bind(generation).bind(event.seq as i64 - 1).execute(&mut *tx).await?;
        if updated.rows_affected() != 1 {
            return Err(StoreError::Conflict);
        }
        sqlx::query("INSERT INTO hr_demo_events (session_id, generation, sequence, event) VALUES ($1, $2, $3, $4)")
            .bind(id).bind(generation).bind(event.seq as i64).bind(sqlx::types::Json(event)).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn reset(&self, id: &str) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO hr_demo_streams (session_id, generation, sequence) VALUES ($1, 0, 0) ON CONFLICT (session_id) DO UPDATE SET generation = hr_demo_streams.generation + 1, sequence = 0")
            .bind(id).execute(&self.pool).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use record_core::{decide, Command};
    #[tokio::test]
    async fn postgres_replays_conflicts_and_preserves_reset_generations() {
        let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
            return;
        };
        let store = Store::connect(&url).await.expect("test database");
        let id = format!(
            "test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let (generation, initial) = store.load(&id).await.unwrap();
        let event = decide(
            &initial,
            Command::Approve {
                proposal_id: "B".into(),
                new_version_id: "B2".into(),
            },
        )
        .unwrap();
        store.append(&id, generation, &event).await.unwrap();
        assert!(matches!(
            store.append(&id, generation, &event).await,
            Err(StoreError::Conflict)
        ));
        let (_, loaded) = store.load(&id).await.unwrap();
        assert_eq!(loaded.active.id, "B2");
        assert_eq!(loaded.events.len(), 1);
        store.reset(&id).await.unwrap();
        let (next_generation, reset) = store.load(&id).await.unwrap();
        assert_eq!(next_generation, generation + 1);
        assert_eq!(reset.active.id, "A");
        assert!(reset.events.is_empty());
        assert!(matches!(
            store.append(&id, generation, &event).await,
            Err(StoreError::Conflict)
        ));
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM hr_demo_events WHERE session_id = $1")
                .bind(&id)
                .fetch_one(&store.pool)
                .await
                .unwrap();
        assert_eq!(count, 1, "reset retains prior-generation events");
        assert!(
            sqlx::query("DELETE FROM hr_demo_events WHERE session_id = $1")
                .bind(&id)
                .execute(&store.pool)
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn postgres_concurrent_decisions_have_one_winner() {
        let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
            return;
        };
        let store = Store::connect(&url).await.unwrap();
        let id = format!(
            "race-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let (generation, initial) = store.load(&id).await.unwrap();
        let approve = decide(
            &initial,
            Command::Approve {
                proposal_id: "B".into(),
                new_version_id: "B2".into(),
            },
        )
        .unwrap();
        let dismiss = decide(
            &initial,
            Command::Dismiss {
                proposal_id: "B".into(),
            },
        )
        .unwrap();
        let (first, second) = tokio::join!(
            store.append(&id, generation, &approve),
            store.append(&id, generation, &dismiss)
        );
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
        assert!(
            matches!(first, Err(StoreError::Conflict))
                || matches!(second, Err(StoreError::Conflict))
        );
        assert_eq!(store.load(&id).await.unwrap().1.events.len(), 1);
    }
}
