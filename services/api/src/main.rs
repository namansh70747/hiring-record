use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::StatusCode;
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
        .route("/health", get(|| async { "ok" }))
        .route("/v1/roles/sept/report", get(report))
        .route("/v1/roles/sept/proposals/B/dismiss", post(dismiss))
        .route("/v1/roles/sept/proposals/B/approve", post(approve))
        .with_state(state)
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
