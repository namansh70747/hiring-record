use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use record_core::{
    apply, evaluate_report, load_fixture, state_from_fixture, Command, Report, State as RoleState,
};
use tokio::net::TcpListener;

#[derive(Clone)]
struct AppState {
    role: Arc<Mutex<RoleState>>,
}

#[tokio::main]
async fn main() {
    let state = AppState {
        role: Arc::new(Mutex::new(fresh())),
    };
    let app = router(state);
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    println!("record-api http://127.0.0.1:8080");
    axum::serve(listener, app).await.unwrap();
}

fn fresh() -> RoleState {
    let fixture = load_fixture(&record_core::fixture_path()).expect("fixtures/role.json");
    state_from_fixture(&fixture)
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(page))
        .route("/health", get(|| async { "ok" }))
        .route("/v1/roles/sept/report", get(report))
        .route("/v1/roles/sept/view", get(view))
        .route("/v1/roles/sept/counterfactual", get(hypothetical))
        .route("/v1/roles/sept/proposals/B/dismiss", post(dismiss))
        .route("/v1/roles/sept/proposals/B/approve", post(approve))
        .route("/v1/roles/sept/reset", post(reset))
        .route("/v1/receipts/nw-B2/ack", post(ack))
        .route("/mcp/connection_check", get(connection_check))
        .route("/mcp/report", get(report))
        .with_state(state)
}

async fn page() -> Html<&'static str> {
    Html(include_str!("../../../apps/web/index.html"))
}

async fn view(State(app): State<AppState>) -> Json<serde_json::Value> {
    let state = app.role.lock().expect("role lock");
    Json(snapshot(&state))
}

async fn hypothetical(State(app): State<AppState>) -> Result<Json<Report>, StatusCode> {
    let state = app.role.lock().expect("role lock");
    record_core::counterfactual_approve(&state)
        .map(Json)
        .map_err(|_| StatusCode::CONFLICT)
}

async fn reset(State(app): State<AppState>) -> Json<serde_json::Value> {
    let mut state = app.role.lock().expect("role lock");
    *state = fresh();
    Json(snapshot(&state))
}

async fn ack(State(app): State<AppState>) -> Result<Json<Report>, StatusCode> {
    mutate(
        app,
        Command::Acknowledge {
            receipt_id: "nw-B2".into(),
        },
    )
}

async fn connection_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "connected": true,
        "ready": true,
        "writes": false,
        "data": "synthetic"
    }))
}

fn snapshot(state: &RoleState) -> serde_json::Value {
    let preview = record_core::preview_change(state, "B").ok();
    let proposal_status = state
        .proposals
        .iter()
        .find(|p| p.id == "B")
        .map(|p| format!("{p:?}"))
        .unwrap_or_else(|| "missing".into());
    let status = if proposal_status.contains("Open") {
        "open"
    } else if proposal_status.contains("Dismissed") {
        "dismissed"
    } else if proposal_status.contains("Approved") {
        "approved"
    } else {
        "missing"
    };
    let people = state
        .people
        .iter()
        .map(|person| {
            let assessment = state.assessments.iter().find(|a| a.person == person.id);
            let stale = assessment.map(|a| state.is_stale(a)).unwrap_or(false);
            serde_json::json!({
                "name": person.name,
                "place": person.place,
                "stale": stale,
                "assessment": assessment.map(|a| format!("scored on {}{}", a.version, if stale { " · stale" } else { "" })).unwrap_or_else(|| "no assessment".into())
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "banner": "Synthetic people. Northwind is a local stand-in with no push operation.",
        "standard_id": state.active.id,
        "proposal_status": status,
        "criteria": state.active.criteria,
        "people": people,
        "preview": preview,
        "report": evaluate_report(state),
        "rubric": state.interviews.first().map(|i| i.rubric_version.clone()).unwrap_or_default(),
        "receipts": state.receipts,
    })
}

async fn report(State(app): State<AppState>) -> Json<Report> {
    let state = app.role.lock().expect("role lock");
    Json(evaluate_report(&state))
}

async fn dismiss(State(app): State<AppState>) -> Result<Json<Report>, StatusCode> {
    mutate(
        app,
        Command::Dismiss {
            proposal_id: "B".into(),
        },
    )
}

async fn approve(State(app): State<AppState>) -> Result<Json<Report>, StatusCode> {
    mutate(
        app,
        Command::Approve {
            proposal_id: "B".into(),
            new_version_id: "B2".into(),
        },
    )
}

fn mutate(app: AppState, command: Command) -> Result<Json<Report>, StatusCode> {
    let mut state = app.role.lock().expect("role lock");
    apply(&mut state, command).map_err(|_| StatusCode::CONFLICT)?;
    Ok(Json(evaluate_report(&state)))
}

#[cfg(test)]
mod contract {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn http_report_matches_the_engine() {
        let app = router(AppState {
            role: Arc::new(Mutex::new(fresh())),
        });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/roles/sept/proposals/B/approve")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let json = body_json(response).await;
        let launch = json["lines"]
            .as_array()
            .unwrap()
            .iter()
            .find(|line| line["criterion_id"] == "led_a_launch")
            .unwrap();
        assert_eq!(launch["naive"], "3 of 3 hires");
        assert_eq!(launch["eligible"], false);
        assert_eq!(launch["refusal"], "RAfterHire");
    }
}
