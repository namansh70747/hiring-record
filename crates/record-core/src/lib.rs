//! Pure hiring-record engine. No I/O, no clock, no randomness.
//! Time and identifiers arrive on commands and fixtures.

#![forbid(unsafe_code)]

mod engine;
mod model;
mod report;

pub use engine::{apply, counterfactual_approve, decide, evolve, preview_change};
pub use model::*;
pub use report::evaluate_report;

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
    pub interviews: Vec<InterviewBinding>,
    pub proposal: Proposal,
}

pub fn load_fixture(path: &Path) -> Result<Fixture, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

pub fn state_from_fixture(fixture: &Fixture) -> State {
    State {
        role_id: fixture.role_id.clone(),
        active: fixture.standard.clone(),
        people: fixture.people.clone(),
        decisions: fixture.decisions.clone(),
        evidence: fixture.evidence.clone(),
        assessments: fixture.assessments.clone(),
        interviews: fixture.interviews.clone(),
        proposals: vec![fixture.proposal.clone()],
        receipts: vec![],
        hold_confirmed: false,
        support_present: true,
        quote_declined: false,
        link_operations: vec![],
        events: vec![],
    }
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
