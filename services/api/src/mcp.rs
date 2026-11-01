//! Read-only MCP access to synthetic scenarios. No production data or write tools.
use rmcp::{
    handler::server::wrapper::Parameters, schemars, tool, tool_handler, tool_router, ErrorData,
    ServerHandler,
};
use serde::Deserialize;

use crate::{fresh, snapshot, AppState};
use record_core::{apply, evaluate_report, Command, State};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ScenarioArgs {
    /// Must be omitted. Browser scenarios are private; tools read the shared synthetic example.
    pub session_id: Option<String>,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RefusalArgs {
    /// A refusal code such as R-AFTER-HIRE or R-REJECTS-MET.
    pub code: String,
    /// Must be omitted. Browser scenarios are private; tools read the shared synthetic example.
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct VerifyArgs {
    pub bundle_json: String,
    pub trusted_key: Option<String>,
}

#[derive(Clone)]
pub struct RecordTools {
    app: AppState,
}

impl RecordTools {
    pub fn new(app: AppState) -> Self {
        Self { app }
    }
    async fn scenario(&self, id: Option<String>) -> Result<State, ErrorData> {
        if id.is_some() {
            return Err(ErrorData::invalid_params("Browser scenarios are private. Omit session_id to inspect the shared synthetic example.", None));
        }
        let mut state = fresh();
        apply(
            &mut state,
            Command::Approve {
                proposal_id: "B".into(),
                new_version_id: "B2".into(),
            },
        )
        .map_err(|_| ErrorData::internal_error("Could not prepare sample", None))?;
        Ok(state)
    }
}

#[tool_router]
impl RecordTools {
    #[tool(
        description = "Verify a signed synthetic JSON bundle: checks signature, event chain and optionally a trusted public key. Does not establish source accuracy."
    )]
    async fn verify_report(
        &self,
        Parameters(args): Parameters<VerifyArgs>,
    ) -> Result<String, ErrorData> {
        if args.bundle_json.len() > 1_000_000 {
            return Err(ErrorData::invalid_params(
                "Bundle exceeds 1 MB MCP limit",
                None,
            ));
        }
        let bundle: record_core::Bundle = serde_json::from_str(&args.bundle_json)
            .map_err(|_| ErrorData::invalid_params("Invalid bundle JSON", None))?;
        let result = record_core::verify_bundle(&bundle, args.trusted_key.as_deref())
            .map_err(|e| ErrorData::invalid_params(e, None))?;
        Ok(serde_json::json!({"signature_valid":result.signature_valid,"chain_valid":result.chain_valid,"trusted_signer":result.trusted_signer,"event_count":result.event_count,"role_id":result.role_id}).to_string())
    }
    #[tool(description = "Check readiness. This server is local, synthetic and read-only.")]
    async fn connection_check(&self) -> String {
        serde_json::json!({"connected":true,"ready":self.app.ready().await,"data":"synthetic","writes":false,"scope":"local demo"}).to_string()
    }
    #[tool(
        description = "Get the hiring role, active standard, proposal and impact preview. Omit session_id for the approved example."
    )]
    async fn get_role(
        &self,
        Parameters(args): Parameters<ScenarioArgs>,
    ) -> Result<String, ErrorData> {
        let state = self.scenario(args.session_id).await?;
        let value = snapshot(&state, state.events.len());
        Ok(serde_json::json!({"synthetic":true,"role_id":state.role_id,"standard_id":state.active.id,"criteria":state.active.criteria,"proposal":value["proposal"],"preview":value["preview"]}).to_string())
    }
    #[tool(
        description = "List synthetic people and their assessment currency for the selected scenario."
    )]
    async fn get_people(
        &self,
        Parameters(args): Parameters<ScenarioArgs>,
    ) -> Result<String, ErrorData> {
        let state = self.scenario(args.session_id).await?;
        Ok(serde_json::json!({"synthetic":true,"people":snapshot(&state,state.events.len())["people"]}).to_string())
    }
    #[tool(
        description = "Evaluate the report using the same Rust rules engine as the workspace. Counts are descriptive, not proof of prediction."
    )]
    async fn get_report(
        &self,
        Parameters(args): Parameters<ScenarioArgs>,
    ) -> Result<String, ErrorData> {
        let state = self.scenario(args.session_id).await?;
        serde_json::to_string(&evaluate_report(&state))
            .map_err(|_| ErrorData::internal_error("Could not serialize report", None))
    }
    #[tool(description = "List hash-chained events for the selected synthetic scenario.")]
    async fn list_events(
        &self,
        Parameters(args): Parameters<ScenarioArgs>,
    ) -> Result<String, ErrorData> {
        let state = self.scenario(args.session_id).await?;
        serde_json::to_string(&state.events)
            .map_err(|_| ErrorData::internal_error("Could not serialize events", None))
    }
    #[tool(
        description = "Explain a refusal code in the selected scenario. Returns the underlying descriptive count and reason."
    )]
    async fn explain_refusal(
        &self,
        Parameters(args): Parameters<RefusalArgs>,
    ) -> Result<String, ErrorData> {
        let state = self.scenario(args.session_id).await?;
        let report = evaluate_report(&state);
        let code = args.code.trim().to_ascii_uppercase();
        let lines: Vec<_> = report
            .lines
            .into_iter()
            .filter(|line| line.refusal.is_some_and(|r| r.as_str() == code))
            .collect();
        if lines.is_empty() {
            return Err(ErrorData::invalid_params(
                "No claim has that refusal code in this scenario",
                None,
            ));
        }
        serde_json::to_string(&lines)
            .map_err(|_| ErrorData::internal_error("Could not serialize explanation", None))
    }
}

#[tool_handler(
    name = "hiring-record",
    version = "0.2.0",
    instructions = "Read-only synthetic hiring scenario. Omit session_id for the approved example. Browser scenarios are private. No real candidates or write tools."
)]
impl ServerHandler for RecordTools {}
