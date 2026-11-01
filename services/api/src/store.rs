//! Optional PostgreSQL storage for synthetic demo event streams.
//! Resets advance a generation; prior events remain in the database.
use record_core::{hash_event, replay_event, Event, State};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};

use crate::fresh;

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

pub struct SealRecord<'a> {
    pub seal_id: &'a str,
    pub session_id: &'a str,
    pub role_id: &'a str,
    pub generation: i64,
    pub sequence: i64,
    pub rules_version: &'a str,
    pub bundle: &'a serde_json::Value,
}

#[derive(Debug)]
pub enum StoreError {
    Database(sqlx::Error),
    Corrupt,
    Conflict,
    Capacity,
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
            Self::Capacity => axum::http::StatusCode::TOO_MANY_REQUESTS,
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
    pub async fn healthy(&self) -> bool {
        matches!(
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                sqlx::query("SELECT 1").execute(&self.pool)
            )
            .await,
            Ok(Ok(_))
        )
    }

    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new().max_connections(8).connect(url).await?;
        let mut tx = pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(481715902)")
            .execute(&mut *tx)
            .await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS hr_schema_migrations (version BIGINT PRIMARY KEY, applied_at TIMESTAMPTZ NOT NULL DEFAULT now())").execute(&mut *tx).await?;
        let applied: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM hr_schema_migrations WHERE version = 1)",
        )
        .fetch_one(&mut *tx)
        .await?;
        if !applied {
            sqlx::raw_sql(include_str!("../migrations/0001_demo_event_store.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO hr_schema_migrations (version) VALUES (1)")
                .execute(&mut *tx)
                .await?;
        }
        let applied: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM hr_schema_migrations WHERE version = 2)",
        )
        .fetch_one(&mut *tx)
        .await?;
        if !applied {
            sqlx::raw_sql(include_str!("../migrations/0002_request_results.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO hr_schema_migrations (version) VALUES (2)")
                .execute(&mut *tx)
                .await?;
        }
        let applied: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM hr_schema_migrations WHERE version = 3)",
        )
        .fetch_one(&mut *tx)
        .await?;
        if !applied {
            sqlx::raw_sql(include_str!("../migrations/0003_workflow_outbox.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO hr_schema_migrations (version) VALUES (3)")
                .execute(&mut *tx)
                .await?;
        }
        let applied: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM hr_schema_migrations WHERE version = 4)",
        )
        .fetch_one(&mut *tx)
        .await?;
        if !applied {
            sqlx::raw_sql(include_str!("../migrations/0004_role_baselines.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO hr_schema_migrations (version) VALUES (4)")
                .execute(&mut *tx)
                .await?;
        }
        let applied: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM hr_schema_migrations WHERE version = 5)",
        )
        .fetch_one(&mut *tx)
        .await?;
        if !applied {
            sqlx::raw_sql(include_str!("../migrations/0005_workflow_kinds.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO hr_schema_migrations (version) VALUES (5)")
                .execute(&mut *tx)
                .await?;
        }
        for (version, sql) in [
            (
                6_i64,
                include_str!("../migrations/0006_sessions_and_ownership.sql"),
            ),
            (
                7,
                include_str!("../migrations/0007_event_envelopes_and_projection.sql"),
            ),
            (8, include_str!("../migrations/0008_workflow_lifecycle.sql")),
            (9, include_str!("../migrations/0009_report_seals.sql")),
            (
                10,
                include_str!("../migrations/0010_shared_limits_and_mcp_grants.sql"),
            ),
            (
                11,
                include_str!("../migrations/0011_operator_audit_and_retention.sql"),
            ),
        ] {
            let applied: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM hr_schema_migrations WHERE version = $1)",
            )
            .bind(version)
            .fetch_one(&mut *tx)
            .await?;
            if !applied {
                sqlx::raw_sql(sql).execute(&mut *tx).await?;
                sqlx::query("INSERT INTO hr_schema_migrations (version) VALUES ($1)")
                    .bind(version)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(Self { pool })
    }
    pub async fn load(&self, id: &str) -> Result<(i64, State), StoreError> {
        let mut tx = self.pool.begin().await?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM hr_demo_streams WHERE session_id=$1)")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        if !exists {
            sqlx::query("SELECT pg_advisory_xact_lock(481715903)")
                .execute(&mut *tx)
                .await?;
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hr_demo_streams")
                .fetch_one(&mut *tx)
                .await?;
            if count >= 1000 {
                return Err(StoreError::Capacity);
            }
        }
        sqlx::query("INSERT INTO hr_demo_streams (session_id) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let row = sqlx::query(
            "SELECT generation, sequence, baseline FROM hr_demo_streams WHERE session_id = $1 FOR SHARE",
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
        let baseline: Option<sqlx::types::Json<State>> = row.try_get("baseline")?;
        let mut state = baseline.map(|v| v.0).unwrap_or_else(fresh);
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
            replay_event(&mut state, event).map_err(|_| StoreError::Corrupt)?;
        }
        tx.commit().await?;
        Ok((generation, state))
    }
    pub async fn exists(&self, id: &str) -> Result<bool, StoreError> {
        Ok(
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM hr_demo_streams WHERE session_id=$1)")
                .bind(id)
                .fetch_one(&self.pool)
                .await?,
        )
    }
    pub async fn role_ids(&self, session: &str) -> Result<Vec<String>, StoreError> {
        let prefix = format!("{session}~");
        let rows:Vec<String>=sqlx::query_scalar("SELECT session_id FROM hr_demo_streams WHERE session_id LIKE $1 ORDER BY session_id LIMIT 20").bind(format!("{prefix}%")).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .filter_map(|id| id.strip_prefix(&prefix).map(str::to_owned))
            .collect())
    }
    pub async fn ensure_role(&self, id: &str, session: &str) -> Result<(), StoreError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(481715903)")
            .execute(&mut *tx)
            .await?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM hr_demo_streams WHERE session_id=$1)")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        if !exists {
            let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM hr_demo_streams")
                .fetch_one(&mut *tx)
                .await?;
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM hr_demo_streams WHERE session_id LIKE $1")
                    .bind(format!("{session}~%"))
                    .fetch_one(&mut *tx)
                    .await?;
            if total >= 1000 || count >= 20 {
                return Err(StoreError::Capacity);
            }
            sqlx::query("INSERT INTO hr_demo_streams(session_id,baseline) VALUES($1,$2)")
                .bind(id)
                .bind(sqlx::types::Json(State::empty()))
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    #[cfg(test)]
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
    pub async fn cached(
        &self,
        id: &str,
        key: &str,
        fingerprint: &str,
    ) -> Result<Option<serde_json::Value>, StoreError> {
        let row = sqlx::query("SELECT fingerprint, response FROM hr_request_results WHERE session_id=$1 AND request_key=$2")
            .bind(id).bind(key).fetch_optional(&self.pool).await?;
        match row {
            Some(row) if row.try_get::<String, _>("fingerprint")? == fingerprint => {
                Ok(Some(row.try_get("response")?))
            }
            Some(_) => Err(StoreError::Conflict),
            None => Ok(None),
        }
    }
    pub async fn append_once(
        &self,
        id: &str,
        generation: i64,
        event: &Event,
        key: &str,
        fingerprint: &str,
        response: serde_json::Value,
    ) -> Result<serde_json::Value, StoreError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT session_id FROM hr_demo_streams WHERE session_id=$1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        if let Some(row)=sqlx::query("SELECT fingerprint,response FROM hr_request_results WHERE session_id=$1 AND request_key=$2").bind(id).bind(key).fetch_optional(&mut *tx).await? {
            if row.try_get::<String,_>("fingerprint")? != fingerprint { return Err(StoreError::Conflict); }
            return Ok(row.try_get("response")?);
        }
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM hr_request_results WHERE session_id=$1")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        if count >= 200 {
            return Err(StoreError::Capacity);
        }
        let updated=sqlx::query("UPDATE hr_demo_streams SET sequence=sequence+1 WHERE session_id=$1 AND generation=$2 AND sequence=$3").bind(id).bind(generation).bind(event.seq as i64-1).execute(&mut *tx).await?;
        if updated.rows_affected() != 1 {
            return Err(StoreError::Conflict);
        }
        sqlx::query("INSERT INTO hr_demo_events (session_id,generation,sequence,event) VALUES ($1,$2,$3,$4)").bind(id).bind(generation).bind(event.seq as i64).bind(sqlx::types::Json(event)).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO hr_request_results (session_id,request_key,fingerprint,response) VALUES ($1,$2,$3,$4)").bind(id).bind(key).bind(fingerprint).bind(&response).execute(&mut *tx).await?;
        let domain_command = if event.kind == record_core::EventKind::DomainCommandV2 {
            serde_json::from_str::<serde_json::Value>(&event.payload).ok()
        } else {
            None
        };
        let command_type = domain_command
            .as_ref()
            .and_then(|v| v["command"]["type"].as_str());
        if event.kind == record_core::EventKind::StandardApproved
            || command_type == Some("accept_quote")
        {
            let snapshot: serde_json::Value = response.clone();
            if let Some(receipts) = snapshot["receipts"].as_array() {
                for receipt in receipts.iter().rev().take(1) {
                    if let Some(receipt_id) = receipt["id"].as_str() {
                        let workflow_id = format!("{id}-{generation}-{}", event.seq);
                        sqlx::query("INSERT INTO hr_workflow_outbox(workflow_id,session_id,generation,receipt_id) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING")
                            .bind(workflow_id).bind(id).bind(generation).bind(receipt_id).execute(&mut *tx).await?;
                    }
                }
            }
        }
        if command_type == Some("request_hold_confirmation") {
            let workflow_id = format!("{id}-{generation}-{}", event.seq);
            sqlx::query("INSERT INTO hr_workflow_outbox(workflow_id,session_id,generation,receipt_id,kind) VALUES($1,$2,$3,'debrief','hold') ON CONFLICT DO NOTHING")
                .bind(workflow_id).bind(id).bind(generation).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(response)
    }
    pub async fn pending_workflows(&self) -> Result<Vec<serde_json::Value>, StoreError> {
        let rows=sqlx::query("SELECT workflow_id,session_id,generation,receipt_id,kind FROM hr_workflow_outbox WHERE NOT dispatched ORDER BY created_at LIMIT 10").fetch_all(&self.pool).await?;
        rows.into_iter().map(|r| Ok(serde_json::json!({"workflow_id":r.try_get::<String,_>("workflow_id")?,"session_id":r.try_get::<String,_>("session_id")?,"generation":r.try_get::<i64,_>("generation")?,"receipt_id":r.try_get::<String,_>("receipt_id")?,"kind":r.try_get::<String,_>("kind")?,"delay_ms":15000}))).collect()
    }
    pub async fn record_dispatch(&self, id: &str, error: Option<&str>) -> Result<(), StoreError> {
        sqlx::query("UPDATE hr_workflow_outbox SET attempts=attempts+1,dispatched=$2,last_error=$3 WHERE workflow_id=$1").bind(id).bind(error.is_none()).bind(error).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn workflows(&self, id: &str) -> Result<serde_json::Value, StoreError> {
        let rows=sqlx::query("SELECT workflow_id,generation,receipt_id,dispatched,attempts,last_error FROM hr_workflow_outbox WHERE session_id=$1 ORDER BY created_at DESC LIMIT 20").bind(id).fetch_all(&self.pool).await?;
        let values:Result<Vec<_>,sqlx::Error>=rows.iter().map(|r|Ok(serde_json::json!({"workflow_id":r.try_get::<String,_>("workflow_id")?,"generation":r.try_get::<i64,_>("generation")?,"receipt_id":r.try_get::<String,_>("receipt_id")?,"dispatched":r.try_get::<bool,_>("dispatched")?,"attempts":r.try_get::<i32,_>("attempts")?,"last_error":r.try_get::<Option<String>,_>("last_error")?}))).collect();
        Ok(serde_json::json!({"mode":"durable outbox","workflows":values?}))
    }
    pub async fn reset_once(
        &self,
        id: &str,
        generation: i64,
        sequence: usize,
        key: &str,
        fingerprint: &str,
        response: serde_json::Value,
    ) -> Result<serde_json::Value, StoreError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT session_id FROM hr_demo_streams WHERE session_id=$1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        if let Some(row)=sqlx::query("SELECT fingerprint,response FROM hr_request_results WHERE session_id=$1 AND request_key=$2").bind(id).bind(key).fetch_optional(&mut *tx).await? {
            if row.try_get::<String,_>("fingerprint")?!=fingerprint {return Err(StoreError::Conflict);}
            return Ok(row.try_get("response")?);
        }
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM hr_request_results WHERE session_id=$1")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        if count >= 200 {
            return Err(StoreError::Capacity);
        }
        let update=sqlx::query("UPDATE hr_demo_streams SET generation=generation+1,sequence=0 WHERE session_id=$1 AND generation=$2 AND sequence=$3").bind(id).bind(generation).bind(sequence as i64).execute(&mut *tx).await?;
        if update.rows_affected() != 1 {
            return Err(StoreError::Conflict);
        }
        sqlx::query("INSERT INTO hr_request_results(session_id,request_key,fingerprint,response) VALUES($1,$2,$3,$4)").bind(id).bind(key).bind(fingerprint).bind(&response).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(response)
    }
    pub async fn allow_request(&self, key: &str, limit: i32) -> Result<bool, StoreError> {
        let hits: i32 = sqlx::query_scalar(
            "INSERT INTO hr_rate_buckets (bucket_key, window_start, hits, expires_at)
             VALUES ($1, now(), 1, now() + interval '60 seconds')
             ON CONFLICT (bucket_key) DO UPDATE SET
               hits = CASE WHEN hr_rate_buckets.expires_at < now() THEN 1 ELSE hr_rate_buckets.hits + 1 END,
               window_start = CASE WHEN hr_rate_buckets.expires_at < now() THEN now() ELSE hr_rate_buckets.window_start END,
               expires_at = CASE WHEN hr_rate_buckets.expires_at < now() THEN now() + interval '60 seconds' ELSE hr_rate_buckets.expires_at END
             RETURNING hits",
        )
        .bind(key)
        .fetch_one(&self.pool)
        .await?;
        Ok(hits <= limit)
    }

    pub async fn save_seal(&self, seal: &SealRecord<'_>) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO hr_report_seals (seal_id, session_id, role_id, generation, sequence, rules_version, bundle)
             VALUES ($1,$2,$3,$4,$5,$6,$7)
             ON CONFLICT (seal_id) DO NOTHING",
        )
        .bind(seal.seal_id)
        .bind(seal.session_id)
        .bind(seal.role_id)
        .bind(seal.generation)
        .bind(seal.sequence)
        .bind(seal.rules_version)
        .bind(seal.bundle)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_seal(
        &self,
        seal_id: &str,
        session_id: &str,
    ) -> Result<Option<serde_json::Value>, StoreError> {
        let row =
            sqlx::query("SELECT bundle FROM hr_report_seals WHERE seal_id=$1 AND session_id=$2")
                .bind(seal_id)
                .bind(session_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(match row {
            Some(row) => Some(row.try_get("bundle")?),
            None => None,
        })
    }

    #[cfg(test)]
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
