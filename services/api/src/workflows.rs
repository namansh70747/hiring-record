//! Transactional outbox dispatch and authenticated, idempotent workflow callbacks.
use crate::{versioned_snapshot, AppState};
use axum::{extract::State, http::StatusCode, Json};
use ed25519_dalek::{Signer, Verifier};
use serde::Deserialize;

pub fn start(app: AppState, ingress: String) {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("HTTP client");
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            interval.tick().await;
            let Some(store) = &app.store else {
                return;
            };
            let Ok(jobs) = store.pending_workflows().await else {
                continue;
            };
            for job in jobs {
                let Some(id) = job["workflow_id"].as_str() else {
                    continue;
                };
                let payload = job.to_string();
                let signature = hex::encode(app.signing_key.sign(payload.as_bytes()).to_bytes());
                let response = client
                    .post(format!(
                        "{}/restate/send/ReceiptDelivery/{id}/run",
                        ingress.trim_end_matches('/')
                    ))
                    .json(&serde_json::json!({"payload":payload,"signature":signature}))
                    .send()
                    .await;
                let error = match response {
                    Ok(response) if response.status().is_success() => None,
                    Ok(response) => Some(format!("Restate HTTP {}", response.status())),
                    Err(_) => Some("Restate unavailable; dispatch will retry".into()),
                };
                let _ = store.record_dispatch(id, error.as_deref()).await;
            }
        }
    });
}
#[derive(Deserialize)]
pub struct Callback {
    payload: String,
    signature: String,
}
#[derive(Deserialize)]
struct Job {
    workflow_id: String,
    session_id: String,
    generation: i64,
    receipt_id: String,
    #[serde(default)]
    kind: String,
}
pub async fn complete(
    State(app): State<AppState>,
    Json(callback): Json<Callback>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let signature = ed25519_dalek::Signature::from_slice(
        &hex::decode(&callback.signature).map_err(|_| StatusCode::UNAUTHORIZED)?,
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?;
    app.signing_key
        .verifying_key()
        .verify(callback.payload.as_bytes(), &signature)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let job: Job = serde_json::from_str(&callback.payload).map_err(|_| StatusCode::BAD_REQUEST)?;
    let store = app.store.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let (generation, mut state) = store.load(&job.session_id).await.map_err(|e| e.status())?;
    if generation != job.generation {
        return Ok(Json(
            serde_json::json!({"status":"superseded by scenario reset"}),
        ));
    }
    let command = if job.kind == "hold" {
        if state.hold_confirmed {
            return Ok(Json(serde_json::json!({"status":"already confirmed"})));
        }
        if !state.hold_pending {
            return Ok(Json(serde_json::json!({"status":"superseded"})));
        }
        record_core::Command::ConfirmHold
    } else {
        let receipt = state
            .receipts
            .iter()
            .find(|r| r.id == job.receipt_id)
            .ok_or(StatusCode::CONFLICT)?;
        if receipt.acknowledged {
            return Ok(Json(serde_json::json!({"status":"already acknowledged"})));
        }
        record_core::Command::Acknowledge {
            receipt_id: job.receipt_id,
        }
    };
    let event = record_core::decide(&state, command).map_err(|_| StatusCode::CONFLICT)?;
    record_core::evolve(&mut state, event.clone());
    store
        .append_once(
            &job.session_id,
            generation,
            &event,
            &format!("workflow-{}", job.workflow_id),
            "workflow-ack",
            versioned_snapshot(&state, state.events.len(), generation),
        )
        .await
        .map_err(|e| e.status())?;
    Ok(Json(serde_json::json!({"status":"completed"})))
}
