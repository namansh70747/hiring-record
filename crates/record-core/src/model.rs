use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardVersion {
    pub id: String,
    pub effective_at: String,
    pub criteria: Vec<Criterion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Criterion {
    pub id: String,
    pub text: String,
    pub added_at: String,
    pub added_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub place: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Hire,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub person: String,
    pub outcome: Outcome,
    pub at: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub person: String,
    pub criterion: String,
    pub met: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assessment {
    pub id: String,
    pub person: String,
    pub version: String,
    pub criterion: String,
    pub met: bool,
    pub citation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterviewBinding {
    pub person: String,
    pub rubric_version: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Open,
    Dismissed,
    Approved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub base: String,
    pub add: Vec<Criterion>,
    #[serde(default = "open_status")]
    pub status: ProposalStatus,
}

fn open_status() -> ProposalStatus {
    ProposalStatus::Open
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub role_id: String,
    pub active: StandardVersion,
    pub people: Vec<Person>,
    pub decisions: Vec<Decision>,
    pub evidence: Vec<Evidence>,
    pub assessments: Vec<Assessment>,
    pub interviews: Vec<InterviewBinding>,
    pub proposals: Vec<Proposal>,
    pub events: Vec<Event>,
}

impl State {
    pub fn is_stale(&self, assessment: &Assessment) -> bool {
        assessment.version != self.active.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    Dismiss {
        proposal_id: String,
    },
    Approve {
        proposal_id: String,
        new_version_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    ProposalDismissed,
    StandardApproved,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::ProposalDismissed => "proposal_dismissed",
            EventKind::StandardApproved => "standard_approved",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub kind: EventKind,
    pub payload: String,
    pub prev_hash: String,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Rejection {
    #[error("proposal {0} is not open")]
    ProposalNotOpen(String),
    #[error("proposal {0} was not found")]
    ProposalMissing(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactPreview {
    pub proposal_id: String,
    pub criteria_added: Vec<String>,
    pub assessments_that_would_become_stale: usize,
    pub interviews_bound_to_earlier_rubric: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefusalCode {
    /// Added after the hires being explained. Cannot be reported as predicted.
    RAfterHire,
    /// Met by every reject in the decision set. Cannot be reported as predicted.
    RRejectsMet,
    RSupportGone,
    RStale,
    RRubricOld,
    RUncited,
    RNoAck,
    RNoOp,
    RDeclined,
    RUnconfirmed,
}

impl RefusalCode {
    pub fn as_str(self) -> &'static str {
        match self {
            RefusalCode::RAfterHire => "R-AFTER-HIRE",
            RefusalCode::RRejectsMet => "R-REJECTS-MET",
            RefusalCode::RSupportGone => "R-SUPPORT-GONE",
            RefusalCode::RStale => "R-STALE",
            RefusalCode::RRubricOld => "R-RUBRIC-OLD",
            RefusalCode::RUncited => "R-UNCITED",
            RefusalCode::RNoAck => "R-NO-ACK",
            RefusalCode::RNoOp => "R-NO-OP",
            RefusalCode::RDeclined => "R-DECLINED",
            RefusalCode::RUnconfirmed => "R-UNCONFIRMED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimLine {
    pub criterion_id: String,
    pub text: String,
    pub naive: String,
    pub hires_met: usize,
    pub hires_total: usize,
    pub eligible: bool,
    pub refusal: Option<RefusalCode>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub role_id: String,
    pub standard_id: String,
    pub lines: Vec<ClaimLine>,
}
