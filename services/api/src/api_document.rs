use record_core::*;
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};
#[derive(Serialize, ToSchema)]
pub struct RoleList {
    pub role_ids: Vec<String>,
}
#[derive(Serialize, ToSchema)]
pub struct PersonView {
    pub id: String,
    pub name: String,
    pub place: String,
    pub stale: bool,
    pub incomplete: bool,
    pub assessment: Option<Assessment>,
    pub decision: Option<Decision>,
    pub interview: Option<InterviewBinding>,
    pub source: Option<SourceMoment>,
    pub evidence: Vec<Evidence>,
}
#[derive(Serialize, ToSchema)]
pub struct ViewDocument {
    pub banner: String,
    pub standard_id: String,
    pub standard_effective_at: String,
    pub proposal_status: String,
    pub proposal: Option<Proposal>,
    pub criteria: Vec<Criterion>,
    pub people: Vec<PersonView>,
    pub preview: Option<ImpactPreview>,
    pub report: Report,
    pub interviews: Vec<InterviewBinding>,
    pub receipts: Vec<Receipt>,
    pub hold_confirmed: bool,
    pub support_present: bool,
    pub events: Vec<Event>,
    pub engine_state: State,
    pub quotes: Vec<Quote>,
    pub facts: Vec<SourcedFact>,
    pub overrides: Vec<DecisionOverride>,
    pub at: usize,
    pub live_total: usize,
    pub version: String,
}
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Hiring Record synthetic API",
        version = "1.0.0",
        description = "Synthetic data only. Browser scenarios use signed HttpOnly cookies. Mutations require Idempotency-Key and If-Match (generation:sequence). MCP reads a separate approved example; it cannot select browser scenarios."
    ),
    paths(
        crate::view,
        crate::report,
        crate::bundle,
        crate::hypothetical,
        crate::command,
        crate::approve,
        crate::dismiss,
        crate::ack,
        crate::confirm_hold,
        crate::remove_support,
        crate::reset,
        crate::new_session,
        crate::signing_key,
        crate::connection_check,
        crate::simulate,
        crate::workflow_status,
        crate::stream,
        crate::create_role,
        crate::list_roles,
        crate::role_view,
        crate::role_report,
        crate::role_bundle
    ),
    components(schemas(
        ViewDocument,
        PersonView,
        RoleList,
        crate::CreateRole,
        Command,
        Bundle,
        SimulationResult,
        Comparison
    ))
)]
struct ApiDoc;
pub fn document() -> String {
    let mut value = serde_json::to_value(ApiDoc::openapi()).expect("OpenAPI");
    value["paths"]["/mcp"] = serde_json::json!({"post":{"summary":"Read-only MCP Streamable HTTP","responses":{"200":{"description":"MCP JSON-RPC result"}}}});
    serde_json::to_string_pretty(&value).expect("OpenAPI JSON")
}
