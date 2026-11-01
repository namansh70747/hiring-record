use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    body::Body,
    extract::{Query, State},
    http::{HeaderMap, Method, Request, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use record_core::{
    apply, decide, evaluate_report, load_fixture, state_from_fixture, Command, ProposalStatus,
    Report, State as RoleState,
};
use serde::Deserialize;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

mod mcp;
mod store;

#[derive(Clone)]
struct AppState {
    roles: Arc<Mutex<HashMap<String, RoleState>>>,
    signing_key: Arc<ed25519_dalek::SigningKey>,
    store: Option<store::Store>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            roles: Arc::new(Mutex::new(HashMap::new())),
            signing_key: Arc::new(ed25519_dalek::SigningKey::generate(&mut rand_core::OsRng)),
            store: None,
        }
    }
}

impl AppState {
    async fn ready(&self) -> bool {
        match &self.store {
            Some(store) => store.healthy().await,
            None => true,
        }
    }

    async fn read_scenario(&self, id: &str) -> Result<(i64, RoleState), StatusCode> {
        if let Some(store) = &self.store {
            store.load(id).await.map_err(|error| error.status())
        } else {
            let mut roles = self.roles.lock().expect("role lock");
            Ok((0, roles.entry(id.to_owned()).or_insert_with(fresh).clone()))
        }
    }
}

#[tokio::main]
async fn main() {
    let mut state = AppState::default();
    if let Ok(url) = std::env::var("DATABASE_URL") {
        state.store = Some(
            store::Store::connect(&url)
                .await
                .expect("could not connect to configured PostgreSQL"),
        );
        println!("Synthetic scenario storage: PostgreSQL");
    } else {
        println!("Synthetic scenario storage: in-memory");
    }
    let app = router(state);
    let listener = TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("port 8080 is available");
    println!("Hiring Record: http://127.0.0.1:8080");
    axum::serve(listener, app).await.expect("server");
}

fn fresh() -> RoleState {
    let fixture = load_fixture(&record_core::fixture_path()).expect("fixtures/role.json");
    state_from_fixture(&fixture)
}

fn session(headers: &HeaderMap) -> Result<String, StatusCode> {
    let Some(value) = headers.get("x-demo-session") else {
        return Ok("default".into());
    };
    let value = value.to_str().map_err(|_| StatusCode::BAD_REQUEST)?;
    if value.is_empty()
        || value.len() > 80
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(value.to_owned())
}

async fn guard_origin(request: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        if let Some(origin) = request.headers().get("origin") {
            if !matches!(
                origin.to_str(),
                Ok("http://127.0.0.1:8080" | "http://localhost:8080")
            ) {
                return Err(StatusCode::FORBIDDEN);
            }
        }
    }
    Ok(next.run(request).await)
}

fn router(state: AppState) -> Router {
    let web = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/out");
    let mcp_state = state.clone();
    let mcp_service = rmcp::transport::streamable_http_server::StreamableHttpService::new(
        move || Ok(mcp::RecordTools::new(mcp_state.clone())),
        rmcp::transport::streamable_http_server::session::local::LocalSessionManager::default()
            .into(),
        rmcp::transport::streamable_http_server::StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true),
    );
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ready", get(connection_check))
        .route("/v1/roles/sept/report", get(report))
        .route("/v1/roles/sept/view", get(view))
        .route("/v1/roles/sept/bundle", get(bundle))
        .route("/v1/signing-key", get(signing_key))
        .route("/v1/simulate", get(simulate))
        .route("/v1/roles/sept/counterfactual", get(hypothetical))
        .route("/v1/roles/sept/proposals/B/dismiss", post(dismiss))
        .route("/v1/roles/sept/proposals/B/approve", post(approve))
        .route("/v1/roles/sept/reset", post(reset))
        .route("/v1/receipts/nw-B2/ack", post(ack))
        .route("/v1/holds/debrief/confirm", post(confirm_hold))
        .route("/v1/support/mina/remove", post(remove_support))
        .route("/api/connection_check", get(connection_check))
        .nest_service("/mcp", mcp_service)
        .fallback_service(ServeDir::new(web).append_index_html_on_directories(true))
        .layer(middleware::from_fn(guard_origin))
        .with_state(state)
}

#[derive(Deserialize)]
struct ViewQuery {
    at: Option<usize>,
}

async fn view(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ViewQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let (_, state) = app.read_scenario(&session(&headers)?).await?;
    let total = state.events.len();
    let at = query.at.unwrap_or(total);
    if at > total {
        return Err(StatusCode::BAD_REQUEST);
    }
    if at == total {
        return Ok(Json(snapshot(&state, total)));
    }
    let mut historical = fresh();
    for event in state.events.iter().take(at) {
        record_core::evolve(&mut historical, event.clone());
    }
    Ok(Json(snapshot(&historical, total)))
}

async fn hypothetical(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<record_core::Comparison>, StatusCode> {
    let (_, state) = app.read_scenario(&session(&headers)?).await?;
    record_core::compare_other_path(&fresh(), &state)
        .map(Json)
        .map_err(|_| StatusCode::CONFLICT)
}

async fn reset(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = session(&headers)?;
    if let Some(store) = &app.store {
        store.reset(&id).await.map_err(|error| error.status())?;
    } else {
        app.roles.lock().expect("role lock").insert(id, fresh());
    }
    Ok(Json(snapshot(&fresh(), 0)))
}

async fn connection_check(State(app): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let ready = app.ready().await;
    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(
            serde_json::json!({ "connected": true, "ready": ready, "writes": false, "data": "synthetic", "transport": "MCP Streamable HTTP at /mcp" }),
        ),
    )
}

#[derive(Deserialize)]
struct SimulationQuery {
    seed: Option<u64>,
    rounds: Option<usize>,
}

async fn simulate(
    Query(query): Query<SimulationQuery>,
) -> Result<Json<record_core::SimulationResult>, StatusCode> {
    let seed = query.seed.unwrap_or(42);
    let rounds = query.rounds.unwrap_or(100);
    record_core::simulate(&fresh(), seed, rounds)
        .map(Json)
        .map_err(|_| StatusCode::BAD_REQUEST)
}

async fn signing_key(State(app): State<AppState>) -> Json<serde_json::Value> {
    Json(
        serde_json::json!({ "algorithm": "Ed25519", "public_key": app.signing_key.verifying_key().as_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>(), "lifetime": "This synthetic demo key changes when the server restarts." }),
    )
}

async fn bundle(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<record_core::Bundle>, StatusCode> {
    let (_, state) = app.read_scenario(&session(&headers)?).await?;
    record_core::create_bundle(&state, &app.signing_key)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn snapshot(state: &RoleState, live_total: usize) -> serde_json::Value {
    let proposal = state.proposals.iter().find(|p| p.id == "B");
    let status = match proposal.map(|p| &p.status) {
        Some(ProposalStatus::Open) => "open",
        Some(ProposalStatus::Dismissed) => "dismissed",
        Some(ProposalStatus::Approved) => "approved",
        None => "missing",
    };
    let people = state.people.iter().map(|person| {
        let assessment = state.assessments.iter().find(|a| a.person == person.id);
        let stale = assessment.map(|a| state.is_stale(a)).unwrap_or(false);
        let decision = state.decisions.iter().find(|d| d.person == person.id);
        let interview = state.interviews.iter().find(|i| i.person == person.id);
        serde_json::json!({
            "id": person.id, "name": person.name, "place": person.place, "stale": stale,
            "assessment": assessment, "decision": decision, "interview": interview,
            "source": assessment.and_then(|a| a.citation.as_ref()).and_then(|id| state.sources.iter().find(|s| s.id == *id && s.person == person.id)),
            "evidence": state.evidence.iter().filter(|e| e.person == person.id).collect::<Vec<_>>()
        })
    }).collect::<Vec<_>>();
    serde_json::json!({
        "banner": "Synthetic people and events. Northwind is a simulated system.",
        "standard_id": state.active.id, "standard_effective_at": state.active.effective_at,
        "proposal_status": status, "proposal": proposal, "criteria": state.active.criteria,
        "people": people, "preview": record_core::preview_change(state, "B").ok(),
        "report": evaluate_report(state), "interviews": state.interviews,
        "receipts": state.receipts, "hold_confirmed": state.hold_confirmed,
        "support_present": state.support_present, "events": state.events,
        "engine_state": state,
        "at": state.events.len(), "live_total": live_total
    })
}

async fn report(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Report>, StatusCode> {
    let (_, state) = app.read_scenario(&session(&headers)?).await?;
    Ok(Json(evaluate_report(&state)))
}

async fn dismiss(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    mutate(
        app,
        headers,
        Command::Dismiss {
            proposal_id: "B".into(),
        },
    )
    .await
}
async fn approve(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    mutate(
        app,
        headers,
        Command::Approve {
            proposal_id: "B".into(),
            new_version_id: "B2".into(),
        },
    )
    .await
}
async fn ack(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    mutate(
        app,
        headers,
        Command::Acknowledge {
            receipt_id: "nw-B2".into(),
        },
    )
    .await
}
async fn confirm_hold(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    mutate(app, headers, Command::ConfirmHold).await
}
async fn remove_support(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    mutate(app, headers, Command::RemoveSupport).await
}
async fn mutate(
    app: AppState,
    headers: HeaderMap,
    command: Command,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = session(&headers)?;
    if let Some(store) = &app.store {
        let (generation, mut state) = store.load(&id).await.map_err(|error| error.status())?;
        let event = decide(&state, command).map_err(|_| StatusCode::CONFLICT)?;
        store
            .append(&id, generation, &event)
            .await
            .map_err(|error| error.status())?;
        record_core::evolve(&mut state, event);
        Ok(Json(snapshot(&state, state.events.len())))
    } else {
        let mut roles = app.roles.lock().expect("role lock");
        let state = roles.entry(id).or_insert_with(fresh);
        apply(state, command).map_err(|_| StatusCode::CONFLICT)?;
        Ok(Json(snapshot(state, state.events.len())))
    }
}

#[cfg(test)]
mod contract {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn json(response: axum::response::Response) -> serde_json::Value {
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
    }
    fn request(method: &str, path: &str, session: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(path)
            .header("x-demo-session", session)
            .body(Body::empty())
            .unwrap()
    }
    #[tokio::test]
    async fn independent_sessions_and_historical_view() {
        let app = router(AppState::default());
        let a = app
            .clone()
            .oneshot(request(
                "POST",
                "/v1/roles/sept/proposals/B/approve",
                "alpha",
            ))
            .await
            .unwrap();
        assert_eq!(a.status(), StatusCode::OK);
        let approved = json(a).await;
        assert_eq!(approved["standard_id"], "B2");
        let b = json(
            app.clone()
                .oneshot(request("GET", "/v1/roles/sept/view", "beta"))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(b["standard_id"], "A");
        let old = json(
            app.clone()
                .oneshot(request("GET", "/v1/roles/sept/view?at=0", "alpha"))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(old["standard_id"], "A");
        assert_eq!(old["live_total"], 1);
        let launch = approved["report"]["lines"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["criterion_id"] == "led_a_launch")
            .unwrap();
        assert_eq!(launch["naive"], "3 of 3 hires");
        assert_eq!(launch["refusal"], "R-AFTER-HIRE");
    }
    #[tokio::test]
    async fn acknowledgement_requires_an_existing_pending_receipt() {
        let app = router(AppState::default());
        let r = app
            .clone()
            .oneshot(request("POST", "/v1/receipts/nw-B2/ack", "alpha"))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::CONFLICT);
        let before = json(
            app.oneshot(request("GET", "/v1/roles/sept/view", "alpha"))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(before["events"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn invalid_session_and_cross_origin_mutation_are_rejected() {
        let app = router(AppState::default());
        let invalid = app
            .clone()
            .oneshot(request("GET", "/v1/roles/sept/view", "not valid!"))
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
        let cross_origin = Request::builder()
            .method("POST")
            .uri("/v1/roles/sept/reset")
            .header("origin", "https://untrusted.example")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.oneshot(cross_origin).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn mcp_exposes_only_read_tools_and_explains_refusals() {
        let app = router(AppState::default());
        let rpc = |body: &str| {
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("host", "127.0.0.1:8080")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("mcp-protocol-version", "2025-11-25")
                .body(Body::from(body.to_owned()))
                .unwrap()
        };
        let list = json(
            app.clone()
                .oneshot(rpc(
                    r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#,
                ))
                .await
                .unwrap(),
        )
        .await;
        let names: Vec<_> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"explain_refusal"));
        assert!(!names
            .iter()
            .any(|name| name.contains("approve") || name.contains("dismiss")));
        let explanation = json(app.oneshot(rpc(r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"explain_refusal","arguments":{"code":"R-AFTER-HIRE"}}}"#)).await.unwrap()).await;
        assert!(explanation["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("R-AFTER-HIRE"));
    }
}
