//! Pure hiring-record engine. No I/O, no clock, no randomness.
//! Time and identifiers arrive on commands and fixtures.

#![forbid(unsafe_code)]

mod bundle;
mod engine;
mod model;
mod report;
mod simulation;

pub use bundle::{
    create_bundle, create_replay_bundle, verify_bundle, Bundle, BundlePayload, SourceReference,
    Verification,
};
pub use engine::{
    apply, attach_provenance, compare_other_path, counterfactual_approve, decide, evolve,
    preview_change, replay_event, same_chain,
};
pub use model::*;
pub use report::evaluate_report;
pub use simulation::{
    minimize_schedule, run_schedule, simulate, ScheduledStep, SimulationResult, SimulationStep,
};

pub const RULES_VERSION: &str = "2026.11";

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Fixture {
    pub role_id: String,
    pub standard: StandardVersion,
    pub people: Vec<Person>,
    pub decisions: Vec<Decision>,
    pub evidence: Vec<Evidence>,
    pub assessments: Vec<Assessment>,
    #[serde(default)]
    pub sources: Vec<SourceMoment>,
    pub interviews: Vec<InterviewBinding>,
    pub proposal: Proposal,
}

pub fn load_fixture(path: &Path) -> Result<Fixture, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn state_from_fixture(fixture: &Fixture) -> State {
    let mut state = State {
        role_id: fixture.role_id.clone(),
        active: fixture.standard.clone(),
        people: fixture.people.clone(),
        decisions: fixture.decisions.clone(),
        evidence: fixture.evidence.clone(),
        assessments: fixture.assessments.clone(),
        sources: fixture.sources.clone(),
        interviews: fixture.interviews.clone(),
        proposals: vec![fixture.proposal.clone()],
        receipts: vec![],
        hold_confirmed: false,
        hold_pending: false,
        support_present: true,
        quote_declined: false,
        link_operations: vec![],
        events: vec![],
        facts: vec![],
        overrides: vec![],
        quotes: vec![],
        holds: vec![],
        removed_supporters: vec![],
        baseline_digest: String::new(),
        baseline_schema: String::new(),
        rules_version: String::new(),
        opening_json: String::new(),
    };
    seal_opening(&mut state);
    state
}

/// Record the immutable replay baseline. Later events must not call this again.
pub fn seal_opening(state: &mut State) {
    state.opening_json.clear();
    state.baseline_schema = "hr-baseline/v1".into();
    state.rules_version = RULES_VERSION.into();
    state.baseline_digest = blake3::hash(canonical_baseline(state).as_bytes())
        .to_hex()
        .to_string();
    state.opening_json = serde_json::to_string(state).unwrap_or_default();
}

pub fn canonical_baseline(state: &State) -> String {
    let criteria = state
        .active
        .criteria
        .iter()
        .map(|c| format!("{}|{}|{}|{}", c.id, c.text, c.added_at, c.added_by))
        .collect::<Vec<_>>()
        .join(",");
    let people = state
        .people
        .iter()
        .map(|p| format!("{}|{}|{}", p.id, p.name, p.place))
        .collect::<Vec<_>>()
        .join(",");
    let decisions = state
        .decisions
        .iter()
        .map(|d| {
            format!(
                "{}|{}|{}|{}",
                d.person,
                match d.outcome {
                    Outcome::Hire => "hire",
                    Outcome::Reject => "reject",
                },
                d.at,
                d.version
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let assessments = state
        .assessments
        .iter()
        .map(|a| {
            format!(
                "{}|{}|{}|{}|{}|{}|{}",
                a.id,
                a.person,
                a.version,
                a.criterion,
                a.met,
                a.unknown,
                a.citation.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let interviews = state
        .interviews
        .iter()
        .map(|i| format!("{}|{}|{}", i.person, i.rubric_version, i.status))
        .collect::<Vec<_>>()
        .join(",");
    let proposals = state
        .proposals
        .iter()
        .map(|p| format!("{}|{}|{}", p.id, p.base, p.add.len()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "hr-baseline/v1\nrole={}\nstandard={}\neffective={}\ncriteria={criteria}\npeople={people}\ndecisions={decisions}\nassessments={assessments}\ninterviews={interviews}\nproposals={proposals}\n",
        state.role_id, state.active.id, state.active.effective_at
    )
}

/// Canonical encoding for this revision.
/// Fields are domain-separated so key order cannot change the hash.
/// RFC 8785 test vectors named in the blueprint are a later contract.
pub fn canonical_event(event: &Event) -> String {
    format!(
        "hr-event/v1\nseq={}\ntype={}\npayload={}\nprev={}",
        event.seq,
        event.kind.as_str(),
        event.payload,
        event.prev_hash
    )
}

pub fn hash_event(event: &Event) -> String {
    blake3::hash(canonical_event(event).as_bytes())
        .to_hex()
        .to_string()
}

pub fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/role.json")
}
