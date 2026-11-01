use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event as SseEvent, KeepAlive},
        Response, Sse,
    },
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

mod api_document;
mod mcp;
mod security;
mod store;
mod workflows;

type RequestResults = HashMap<(String, String), (String, serde_json::Value)>;

#[derive(Clone)]
struct AppState {
    roles: Arc<Mutex<HashMap<String, (i64, RoleState)>>>,
    signing_key: Arc<ed25519_dalek::SigningKey>,
    store: Option<store::Store>,
    public_origin: String,
    streams: Arc<tokio::sync::Semaphore>,
    results: Arc<Mutex<RequestResults>>,
    request_limits: Arc<Mutex<HashMap<String, (u64, u32)>>>,
    workflow_enabled: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            roles: Arc::new(Mutex::new(HashMap::new())),
            signing_key: Arc::new({
                #[cfg(test)]
                {
                    ed25519_dalek::SigningKey::from_bytes(&[7; 32])
                }
                #[cfg(not(test))]
                {
                    ed25519_dalek::SigningKey::generate(&mut rand_core::OsRng)
                }
            }),
            store: None,
            streams: Arc::new(tokio::sync::Semaphore::new(128)),
            results: Arc::new(Mutex::new(HashMap::new())),
            public_origin: "http://127.0.0.1:8080".into(),
            request_limits: Arc::new(Mutex::new(HashMap::new())),
            workflow_enabled: false,
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
            if roles.len() >= 1000 && !roles.contains_key(id) {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
            Ok(roles
                .entry(id.to_owned())
                .or_insert_with(|| (0, fresh()))
                .clone())
        }
    }
}

#[tokio::main]
async fn main() {
    if std::env::args().any(|a| a == "--openapi") {
        println!("{}", api_document::document());
        return;
    }
    let mut state = AppState {
        signing_key: Arc::new(security::load_key()),
        public_origin: std::env::var("PUBLIC_ORIGIN")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".into()),
        ..AppState::default()
    };
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
    if let Ok(ingress) = std::env::var("RESTATE_INGRESS") {
        state.workflow_enabled = state.store.is_some();
        workflows::start(state.clone(), ingress);
    }
    let app = router(state);
    let listener = TcpListener::bind(
        std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".into()),
    )
    .await
    .expect("port 8080 is available");
    println!("Hiring Record: http://127.0.0.1:8080");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .expect("server");
}

fn fresh() -> RoleState {
    let path = std::env::var_os("RECORD_FIXTURE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(record_core::fixture_path);
    let fixture = load_fixture(&path).expect("synthetic role fixture");
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

async fn guard_origin(
    State(app): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) {
        if let Some(origin) = request.headers().get("origin") {
            if !origin.to_str().is_ok_and(|origin| {
                origin == app.public_origin
                    || origin == "http://localhost:8080"
                        && app.public_origin == "http://127.0.0.1:8080"
            }) {
                return Err(StatusCode::FORBIDDEN);
            }
        }
    }
    Ok(next.run(request).await)
}

fn router(state: AppState) -> Router {
    let web = std::env::var_os("RECORD_WEB_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/out"));
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
        .route("/internal/workflow/complete", post(workflows::complete))
        .route("/v1/workflows", get(workflow_status))
        .route("/v1/roles", get(list_roles).post(create_role))
        .route("/v1/roles/{role_id}/view", get(role_view))
        .route("/v1/roles/{role_id}/report", get(role_report))
        .route("/v1/roles/{role_id}/bundle", get(role_bundle))
        .route("/v1/roles/{role_id}/commands", post(command))
        .route("/health", get(|| async { "ok" }))
        .route("/ready", get(connection_check))
        .route("/v1/session/new", post(new_session))
        .route("/v1/roles/sept/report", get(report))
        .route("/v1/roles/sept/view", get(view))
        .route("/v1/roles/sept/stream", get(stream))
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
        .layer(DefaultBodyLimit::max(1_000_000))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            security::protect,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), guard_origin))
        .with_state(state)
}

#[derive(Deserialize)]
struct ViewQuery {
    at: Option<usize>,
}

#[utoipa::path(get, path="/v1/roles/sept/view", responses((status=200,body=api_document::ViewDocument)), params(("at"=Option<usize>,Query,description="Historical event index")))]
async fn view(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ViewQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let (generation, state) = app.read_scenario(&session(&headers)?).await?;
    let total = state.events.len();
    let at = query.at.unwrap_or(total);
    if at > total {
        return Err(StatusCode::BAD_REQUEST);
    }
    if at == total {
        return Ok(Json(versioned_snapshot(&state, total, generation)));
    }
    let mut historical = fresh();
    for event in state.events.iter().take(at) {
        record_core::evolve(&mut historical, event.clone());
    }
    Ok(Json(versioned_snapshot(&historical, total, generation)))
}

#[utoipa::path(get,path="/v1/roles/sept/stream",responses((status=200,description="SSE snapshot events with generation:sequence IDs; Last-Event-ID supports reconnect")))]
async fn stream(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    let id = session(&headers)?;
    let permit = app
        .streams
        .clone()
        .try_acquire_owned()
        .map_err(|_| StatusCode::TOO_MANY_REQUESTS)?;
    let mut previous = headers
        .get("last-event-id")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let stream = async_stream::stream! {
        let _permit=permit;
        let started=std::time::Instant::now();
        let mut interval=tokio::time::interval(std::time::Duration::from_secs(1));
        while started.elapsed().as_secs()<3600 {
            interval.tick().await;
            match app.read_scenario(&id).await {
                Ok((generation,state)) => {
                    let version=format!("{generation}:{}",state.events.len());
                    if version!=previous {
                        previous=version.clone();
                        let event=SseEvent::default().event("snapshot").id(version).json_data(versioned_snapshot(&state,state.events.len(),generation));
                        if let Ok(event)=event { yield Ok::<_,std::convert::Infallible>(event); }
                    }
                },
                Err(_) => { yield Ok(SseEvent::default().event("unavailable").data("Record temporarily unavailable")); break; }
            }
        }
    };
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15))))
}

#[utoipa::path(get,path="/v1/roles/sept/counterfactual",responses((status=200,body=record_core::Comparison)))]
async fn hypothetical(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<record_core::Comparison>, StatusCode> {
    let (_, state) = app.read_scenario(&session(&headers)?).await?;
    record_core::compare_other_path(&fresh(), &state)
        .map(Json)
        .map_err(|_| StatusCode::CONFLICT)
}

#[derive(Deserialize, utoipa::ToSchema)]
struct CreateRole {
    role_id: String,
    standard: record_core::StandardVersion,
}

fn role_stream_id(session: &str, role: &str) -> Result<String, StatusCode> {
    if role.is_empty()
        || role.len() > 100
        || !role
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(if role == "sept" {
        session.to_owned()
    } else {
        format!("{session}~{role}")
    })
}

async fn existing_role_stream(
    app: &AppState,
    session: &str,
    role: &str,
) -> Result<String, StatusCode> {
    let id = role_stream_id(session, role)?;
    if role != "sept" {
        let exists = match &app.store {
            Some(store) => store.exists(&id).await.map_err(|e| e.status())?,
            None => app.roles.lock().expect("role lock").contains_key(&id),
        };
        if !exists {
            return Err(StatusCode::NOT_FOUND);
        }
    }
    Ok(id)
}

#[utoipa::path(post,path="/v1/roles",request_body=CreateRole,params(("Idempotency-Key"=String,Header),("If-Match"=String,Header)),responses((status=200,body=api_document::ViewDocument),(status=409,description="Role or version conflict")))]
async fn create_role(
    State(app): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateRole>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if input.role_id == "sept" {
        return Err(StatusCode::CONFLICT);
    }
    let session_id = session(&headers)?;
    let id = role_stream_id(&session_id, &input.role_id)?;
    let command = Command::OpenRole {
        role_id: input.role_id,
        standard: input.standard,
    };
    decide(&RoleState::empty(), command.clone()).map_err(|_| StatusCode::BAD_REQUEST)?;
    if headers.get("idempotency-key").is_none()
        || headers.get("if-match").and_then(|v| v.to_str().ok()) != Some("0:0")
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(store) = &app.store {
        store
            .ensure_role(&id, &session_id)
            .await
            .map_err(|e| e.status())?;
    } else {
        let mut roles = app.roles.lock().expect("role lock");
        let prefix = format!("{session_id}~");
        if !roles.contains_key(&id)
            && (roles.len() >= 1000
                || roles.keys().filter(|k| k.starts_with(&prefix)).count() >= 20)
        {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        roles
            .entry(id.clone())
            .or_insert_with(|| (0, RoleState::empty()));
    }
    mutate_stream(app, headers, command, id).await
}

#[utoipa::path(get,path="/v1/roles",responses((status=200,body=api_document::RoleList)))]
async fn list_roles(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<api_document::RoleList>, StatusCode> {
    let session_id = session(&headers)?;
    let mut ids = match &app.store {
        Some(store) => store.role_ids(&session_id).await.map_err(|e| e.status())?,
        None => {
            let prefix = format!("{session_id}~");
            app.roles
                .lock()
                .expect("role lock")
                .keys()
                .filter_map(|id| id.strip_prefix(&prefix).map(str::to_owned))
                .collect()
        }
    };
    let mut role_ids = vec!["sept".into()];
    ids.sort();
    for role in ids {
        let id = role_stream_id(&session_id, &role)?;
        if app.read_scenario(&id).await?.1.role_id == role {
            role_ids.push(role);
        }
    }
    Ok(Json(api_document::RoleList { role_ids }))
}

#[utoipa::path(get,path="/v1/roles/{role_id}/view",params(("role_id"=String,Path),("at"=Option<usize>,Query)),responses((status=200,body=api_document::ViewDocument),(status=404,description="Role not found")))]
async fn role_view(
    State(app): State<AppState>,
    Path(role_id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<ViewQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = existing_role_stream(&app, &session(&headers)?, &role_id).await?;
    let (generation, state) = app.read_scenario(&id).await?;
    if state.role_id != role_id {
        return Err(StatusCode::NOT_FOUND);
    }
    let total = state.events.len();
    let at = query.at.unwrap_or(total);
    if at > total {
        return Err(StatusCode::BAD_REQUEST);
    }
    if at == total {
        return Ok(Json(versioned_snapshot(&state, total, generation)));
    }
    let mut historical = if role_id == "sept" {
        fresh()
    } else {
        RoleState::empty()
    };
    for event in state.events.iter().take(at) {
        record_core::replay_event(&mut historical, event.clone())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    Ok(Json(versioned_snapshot(&historical, total, generation)))
}

#[utoipa::path(get,path="/v1/roles/{role_id}/report",params(("role_id"=String,Path)),responses((status=200,body=record_core::Report)))]
async fn role_report(
    State(app): State<AppState>,
    Path(role_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Report>, StatusCode> {
    let id = existing_role_stream(&app, &session(&headers)?, &role_id).await?;
    let (_, state) = app.read_scenario(&id).await?;
    if state.role_id != role_id {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(evaluate_report(&state)))
}

#[utoipa::path(get,path="/v1/roles/{role_id}/bundle",params(("role_id"=String,Path)),responses((status=200,body=record_core::Bundle)))]
async fn role_bundle(
    State(app): State<AppState>,
    Path(role_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<record_core::Bundle>, StatusCode> {
    let id = existing_role_stream(&app, &session(&headers)?, &role_id).await?;
    let (_, state) = app.read_scenario(&id).await?;
    if state.role_id != role_id {
        return Err(StatusCode::NOT_FOUND);
    }
    record_core::create_bundle(&state, &app.signing_key)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[utoipa::path(post,path="/v1/roles/{role_id}/commands",request_body=record_core::Command,params(("role_id"=String,Path),("Idempotency-Key"=String,Header),("If-Match"=String,Header)),responses((status=200,body=api_document::ViewDocument),(status=409,description="Version conflict")))]
async fn command(
    State(app): State<AppState>,
    Path(role_id): Path<String>,
    headers: HeaderMap,
    Json(command): Json<Command>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = existing_role_stream(&app, &session(&headers)?, &role_id).await?;
    let (_, state) = app.read_scenario(&id).await?;
    if state.role_id != role_id {
        return Err(StatusCode::NOT_FOUND);
    }
    mutate_stream(app, headers, command, id).await
}

#[utoipa::path(get,path="/v1/workflows",responses((status=200,body=serde_json::Value)))]
async fn workflow_status(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = session(&headers)?;
    match &app.store {
        Some(store) => store.workflows(&id).await.map(Json).map_err(|e| e.status()),
        None => Ok(Json(
            serde_json::json!({"mode":"manual simulation","workflows":[]}),
        )),
    }
}

#[utoipa::path(post,path="/v1/session/new",responses((status=200,body=api_document::ViewDocument)))]
async fn new_session(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let (generation, state) = app.read_scenario(&session(&headers)?).await?;
    Ok(Json(versioned_snapshot(
        &state,
        state.events.len(),
        generation,
    )))
}

#[utoipa::path(post,path="/v1/roles/sept/reset",responses((status=200,body=api_document::ViewDocument)))]
async fn reset(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = session(&headers)?;
    let key = headers
        .get("idempotency-key")
        .and_then(|h| h.to_str().ok())
        .filter(|k| !k.is_empty() && k.len() <= 128)
        .ok_or(StatusCode::BAD_REQUEST)?;
    let expected = headers
        .get("if-match")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::PRECONDITION_REQUIRED)?;
    let fingerprint = format!("reset|{expected}");
    if let Some(store) = &app.store {
        if let Some(response) = store
            .cached(&id, key, &fingerprint)
            .await
            .map_err(|e| e.status())?
        {
            return Ok(Json(response));
        }
        let (generation, state) = store.load(&id).await.map_err(|e| e.status())?;
        if expected != format!("{generation}:{}", state.events.len()) {
            if let Some(response) = store
                .cached(&id, key, &fingerprint)
                .await
                .map_err(|e| e.status())?
            {
                return Ok(Json(response));
            }
            return Err(StatusCode::CONFLICT);
        }
        let response = versioned_snapshot(&fresh(), 0, generation + 1);
        store
            .reset_once(
                &id,
                generation,
                state.events.len(),
                key,
                &fingerprint,
                response,
            )
            .await
            .map(Json)
            .map_err(|e| e.status())
    } else {
        let mut roles = app.roles.lock().expect("role lock");
        let mut results = app.results.lock().expect("results lock");
        if let Some((original, response)) = results.get(&(id.clone(), key.into())) {
            return if original == &fingerprint {
                Ok(Json(response.clone()))
            } else {
                Err(StatusCode::CONFLICT)
            };
        }
        if results.keys().filter(|(session, _)| session == &id).count() >= 200 {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        let (generation, state) = roles.entry(id.clone()).or_insert_with(|| (0, fresh()));
        if expected != format!("{generation}:{}", state.events.len()) {
            return Err(StatusCode::CONFLICT);
        }
        *generation += 1;
        *state = fresh();
        let response = versioned_snapshot(state, 0, *generation);
        results.insert((id, key.into()), (fingerprint, response.clone()));
        Ok(Json(response))
    }
}

#[utoipa::path(get,path="/ready",responses((status=200,body=serde_json::Value)))]
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

#[utoipa::path(get,path="/v1/simulate",params(("seed"=Option<u64>,Query),("rounds"=Option<usize>,Query)),responses((status=200,body=record_core::SimulationResult)))]
async fn simulate(
    Query(query): Query<SimulationQuery>,
) -> Result<Json<record_core::SimulationResult>, StatusCode> {
    let seed = query.seed.unwrap_or(42);
    let rounds = query.rounds.unwrap_or(100);
    record_core::simulate(&fresh(), seed, rounds)
        .map(Json)
        .map_err(|_| StatusCode::BAD_REQUEST)
}

#[utoipa::path(get,path="/v1/signing-key",responses((status=200,body=serde_json::Value)))]
async fn signing_key(State(app): State<AppState>) -> Json<serde_json::Value> {
    Json(
        serde_json::json!({ "algorithm": "Ed25519", "public_key": app.signing_key.verifying_key().as_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>(), "lifetime": "Persisted local key. Operators can configure SIGNING_KEY_FILE." }),
    )
}

#[utoipa::path(get, path="/v1/roles/sept/bundle", responses((status=200,body=record_core::Bundle)))]
async fn bundle(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<record_core::Bundle>, StatusCode> {
    let (_, state) = app.read_scenario(&session(&headers)?).await?;
    record_core::create_bundle(&state, &app.signing_key)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn versioned_snapshot(state: &RoleState, live_total: usize, generation: i64) -> serde_json::Value {
    let mut result = snapshot(state, live_total);
    result["version"] = serde_json::json!(format!("{generation}:{live_total}"));
    result
}

fn snapshot(state: &RoleState, live_total: usize) -> serde_json::Value {
    let proposal = state
        .proposals
        .iter()
        .rev()
        .find(|p| p.status == ProposalStatus::Open)
        .or_else(|| state.proposals.last());
    let status = match proposal.map(|p| &p.status) {
        Some(ProposalStatus::Open) => "open",
        Some(ProposalStatus::Dismissed) => "dismissed",
        Some(ProposalStatus::Approved) => "approved",
        None => "missing",
    };
    let people = state
        .people
        .iter()
        .map(|person| {
            let assessment = state
                .assessments
                .iter()
                .rev()
                .find(|a| a.person == person.id);
            api_document::PersonView {
                id: person.id.clone(),
                name: person.name.clone(),
                place: person.place.clone(),
                stale: state
                    .latest_assessments()
                    .iter()
                    .any(|a| a.person == person.id && state.is_stale(a)),
                incomplete: state.active.criteria.iter().any(|criterion| {
                    !state.assessments.iter().any(|a| {
                        a.person == person.id
                            && a.criterion == criterion.id
                            && a.version == state.active.id
                    })
                }),
                assessment: assessment.cloned(),
                decision: state
                    .decisions
                    .iter()
                    .find(|d| d.person == person.id)
                    .cloned(),
                interview: state
                    .interviews
                    .iter()
                    .find(|i| i.person == person.id)
                    .cloned(),
                source: assessment
                    .and_then(|a| a.citation.as_ref())
                    .and_then(|id| {
                        state
                            .sources
                            .iter()
                            .find(|s| s.id == *id && s.person == person.id)
                    })
                    .cloned(),
                evidence: state
                    .evidence
                    .iter()
                    .filter(|e| e.person == person.id)
                    .cloned()
                    .collect(),
            }
        })
        .collect();
    serde_json::to_value(api_document::ViewDocument {
        banner: "Synthetic people and events. Northwind is a simulated system.".into(),
        standard_id: state.active.id.clone(),
        standard_effective_at: state.active.effective_at.clone(),
        proposal_status: status.into(),
        proposal: proposal.cloned(),
        criteria: state.active.criteria.clone(),
        people,
        preview: proposal.and_then(|p| record_core::preview_change(state, &p.id).ok()),
        report: evaluate_report(state),
        interviews: state.interviews.clone(),
        receipts: state.receipts.clone(),
        hold_confirmed: state.hold_confirmed,
        hold_pending: state.hold_pending,
        support_present: state.support_present,
        events: state.events.clone(),
        engine_state: state.clone(),
        quotes: state.quotes.clone(),
        facts: state.facts.clone(),
        overrides: state.overrides.clone(),
        at: state.events.len(),
        live_total,
        version: format!("0:{live_total}"),
    })
    .expect("serialize view")
}

#[utoipa::path(get, path="/v1/roles/sept/report", responses((status=200,body=record_core::Report)))]
async fn report(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Report>, StatusCode> {
    let (_, state) = app.read_scenario(&session(&headers)?).await?;
    Ok(Json(evaluate_report(&state)))
}

#[utoipa::path(post,path="/v1/roles/sept/proposals/B/dismiss",responses((status=200,body=api_document::ViewDocument)))]
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
#[utoipa::path(post,path="/v1/roles/sept/proposals/B/approve",responses((status=200,body=api_document::ViewDocument)))]
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
#[utoipa::path(post,path="/v1/receipts/nw-B2/ack",responses((status=200,body=api_document::ViewDocument)))]
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
#[utoipa::path(post,path="/v1/holds/debrief/confirm",responses((status=200,body=api_document::ViewDocument)))]
async fn confirm_hold(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let command = if app.workflow_enabled {
        Command::RequestHoldConfirmation
    } else {
        Command::ConfirmHold
    };
    mutate(app, headers, command).await
}
#[utoipa::path(post,path="/v1/support/mina/remove",responses((status=200,body=api_document::ViewDocument)))]
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
    mutate_stream(app, headers, command, id).await
}

async fn mutate_stream(
    app: AppState,
    headers: HeaderMap,
    command: Command,
    id: String,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let key = headers
        .get("idempotency-key")
        .and_then(|h| h.to_str().ok())
        .filter(|k| !k.is_empty() && k.len() <= 128)
        .ok_or(StatusCode::BAD_REQUEST)?;
    let expected = headers
        .get("if-match")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::PRECONDITION_REQUIRED)?;
    let fingerprint = format!(
        "{}|{}",
        serde_json::to_string(&command).map_err(|_| StatusCode::BAD_REQUEST)?,
        expected
    );
    if let Some(store) = &app.store {
        if let Some(response) = store
            .cached(&id, key, &fingerprint)
            .await
            .map_err(|e| e.status())?
        {
            return Ok(Json(response));
        }
        let (generation, mut state) = store.load(&id).await.map_err(|error| error.status())?;
        if expected != format!("{generation}:{}", state.events.len()) {
            if let Some(response) = store
                .cached(&id, key, &fingerprint)
                .await
                .map_err(|e| e.status())?
            {
                return Ok(Json(response));
            }
            return Err(StatusCode::CONFLICT);
        }
        let event = decide(&state, command).map_err(|_| StatusCode::CONFLICT)?;
        record_core::evolve(&mut state, event.clone());
        let response = versioned_snapshot(&state, state.events.len(), generation);
        store
            .append_once(&id, generation, &event, key, &fingerprint, response)
            .await
            .map(Json)
            .map_err(|e| e.status())
    } else {
        let mut roles = app.roles.lock().expect("role lock");
        let mut results = app.results.lock().expect("results lock");
        if let Some((original, response)) = results.get(&(id.clone(), key.into())) {
            return if original == &fingerprint {
                Ok(Json(response.clone()))
            } else {
                Err(StatusCode::CONFLICT)
            };
        }
        if results.keys().filter(|(session, _)| session == &id).count() >= 200 {
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        let (generation, state) = roles.entry(id.clone()).or_insert_with(|| (0, fresh()));
        if expected != format!("{generation}:{}", state.events.len()) {
            return Err(StatusCode::CONFLICT);
        }
        apply(state, command).map_err(|_| StatusCode::CONFLICT)?;
        let response = versioned_snapshot(state, state.events.len(), *generation);
        results.insert((id, key.into()), (fingerprint, response.clone()));
        Ok(Json(response))
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
            .header(
                "idempotency-key",
                format!("test-{}", path.replace('/', "-")),
            )
            .header("if-match", "0:0")
            .header(
                "cookie",
                format!(
                    "hr_session={}",
                    if session.contains('!') {
                        "invalid".into()
                    } else {
                        security::mint(
                            &ed25519_dalek::SigningKey::from_bytes(&[7; 32]),
                            &format!("{:0<64}", hex::encode(session.as_bytes())),
                            security::now() + 3600,
                        )
                    }
                ),
            )
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
        assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);
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
    async fn unsigned_headers_cannot_select_another_browser_scenario() {
        let app = router(AppState::default());
        let first = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/roles/sept/view")
                    .header("x-demo-session", "victim")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let cookie = first.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        assert!(first.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .contains("HttpOnly"));
        let mut approve = Request::builder()
            .method("POST")
            .uri("/v1/roles/sept/proposals/B/approve")
            .header("cookie", &cookie)
            .header("idempotency-key", "one")
            .header("if-match", "0:0")
            .body(Body::empty())
            .unwrap();
        approve
            .headers_mut()
            .insert("x-demo-session", "victim".parse().unwrap());
        assert_eq!(
            app.clone().oneshot(approve).await.unwrap().status(),
            StatusCode::OK
        );
        let other = json(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/v1/roles/sept/view")
                        .header("x-demo-session", "victim")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(other["standard_id"], "A");
        let own = json(
            app.oneshot(
                Request::builder()
                    .uri("/v1/roles/sept/view")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(own["standard_id"], "B2");
    }

    #[tokio::test]
    async fn retry_returns_original_response_without_appending_twice() {
        let mut state = AppState::default();
        if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
            state.store = Some(store::Store::connect(&url).await.unwrap());
        }
        if let Ok(ingress) = std::env::var("RESTATE_INGRESS") {
            workflows::start(state.clone(), ingress);
        }
        let app = router(state);
        let session = format!(
            "retry-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        // Session IDs in this test helper are hex-encoded and limited to 32 bytes.
        let session = &session[..session.len().min(31)];
        let first = app
            .clone()
            .oneshot(request(
                "POST",
                "/v1/roles/sept/proposals/B/approve",
                session,
            ))
            .await
            .unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        let first = json(first).await;
        let repeated = app
            .clone()
            .oneshot(request(
                "POST",
                "/v1/roles/sept/proposals/B/approve",
                session,
            ))
            .await
            .unwrap();
        assert_eq!(repeated.status(), StatusCode::OK);
        assert_eq!(json(repeated).await, first);
        let current = json(
            app.oneshot(request("GET", "/v1/roles/sept/view", session))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(current["events"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn simultaneous_identical_retries_share_one_committed_result() {
        let mut state = AppState::default();
        if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
            state.store = Some(store::Store::connect(&url).await.unwrap());
        }
        let app = router(state);
        let session = format!(
            "race-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let (first, second) = tokio::join!(
            app.clone().oneshot(request(
                "POST",
                "/v1/roles/sept/proposals/B/approve",
                &session
            )),
            app.clone().oneshot(request(
                "POST",
                "/v1/roles/sept/proposals/B/approve",
                &session
            ))
        );
        let (first, second) = (first.unwrap(), second.unwrap());
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(second.status(), StatusCode::OK);
        assert_eq!(json(first).await, json(second).await);
        let current = json(
            app.oneshot(request("GET", "/v1/roles/sept/view", &session))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(current["events"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn custom_roles_are_event_sourced_and_isolated_by_visitor() {
        let mut state = AppState::default();
        if let Ok(url) = std::env::var("TEST_DATABASE_URL") {
            state.store = Some(store::Store::connect(&url).await.unwrap());
        }
        let app = router(state);
        let name = format!(
            "u{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let mut create = request("POST", "/v1/roles", &name);
        create
            .headers_mut()
            .insert("content-type", "application/json".parse().unwrap());
        *create.body_mut()=Body::from(serde_json::json!({"role_id":"engineer","standard":{"id":"v1","effective_at":"2026-01-01","criteria":[{"id":"build","text":"Built systems","added_at":"2026-01-01","added_by":"alex"}]}}).to_string());
        let created = app.clone().oneshot(create).await.unwrap();
        assert_eq!(created.status(), StatusCode::OK);
        let created = json(created).await;
        assert_eq!(created["report"]["role_id"], "engineer");
        assert_eq!(created["version"], "0:1");
        let mut add = request("POST", "/v1/roles/engineer/commands", &name);
        add.headers_mut()
            .insert("content-type", "application/json".parse().unwrap());
        add.headers_mut().insert("if-match", "0:1".parse().unwrap());
        *add.body_mut()=Body::from(serde_json::json!({"type":"add_person","person":{"id":"sam","name":"Sam","place":"in_review"}}).to_string());
        let added = app.clone().oneshot(add).await.unwrap();
        assert_eq!(added.status(), StatusCode::OK);
        assert_eq!(json(added).await["people"][0]["name"], "Sam");
        let loaded = json(
            app.clone()
                .oneshot(request("GET", "/v1/roles/engineer/view", &name))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(loaded["events"].as_array().unwrap().len(), 2);
        let list = json(
            app.clone()
                .oneshot(request("GET", "/v1/roles", &name))
                .await
                .unwrap(),
        )
        .await;
        assert!(list["role_ids"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("engineer")));
        let other = app
            .oneshot(request(
                "GET",
                "/v1/roles/engineer/view",
                "different-visitor",
            ))
            .await
            .unwrap();
        assert_eq!(other.status(), StatusCode::NOT_FOUND);
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
