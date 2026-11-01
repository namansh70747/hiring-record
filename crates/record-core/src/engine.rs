use crate::model::*;
use crate::{canonical_event, hash_event};

pub fn decide(state: &State, command: Command) -> Result<Event, Rejection> {
    let proposal = match &command {
        Command::Dismiss { proposal_id } | Command::Approve { proposal_id, .. } => {
            state
                .proposals
                .iter()
                .find(|p| p.id == *proposal_id)
                .ok_or_else(|| Rejection::ProposalMissing(proposal_id.clone()))?
        }
    };
    if proposal.status != ProposalStatus::Open {
        return Err(Rejection::ProposalNotOpen(proposal.id.clone()));
    }

    let (kind, payload) = match command {
        Command::Dismiss { proposal_id } => (
            EventKind::ProposalDismissed,
            format!("proposal={proposal_id}"),
        ),
        Command::Approve {
            proposal_id,
            new_version_id,
        } => (
            EventKind::StandardApproved,
            format!("proposal={proposal_id};version={new_version_id}"),
        ),
    };

    let seq = state.events.len() as u64 + 1;
    let prev_hash = state
        .events
        .last()
        .map(|e| e.hash.clone())
        .unwrap_or_else(|| "genesis".to_string());
    let mut event = Event {
        seq,
        kind,
        payload,
        prev_hash,
        hash: String::new(),
    };
    event.hash = hash_event(&event);
    debug_assert_eq!(event.hash, hash_event(&event));
    let _ = canonical_event(&event);
    Ok(event)
}

pub fn evolve(state: &mut State, event: Event) {
    match event.kind {
        EventKind::ProposalDismissed => {
            let id = payload_field(&event.payload, "proposal");
            if let Some(proposal) = state.proposals.iter_mut().find(|p| p.id == id) {
                proposal.status = ProposalStatus::Dismissed;
            }
        }
        EventKind::StandardApproved => {
            let id = payload_field(&event.payload, "proposal");
            let version_id = payload_field(&event.payload, "version");
            let added = state
                .proposals
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.add.clone())
                .unwrap_or_default();
            if let Some(proposal) = state.proposals.iter_mut().find(|p| p.id == id) {
                proposal.status = ProposalStatus::Approved;
            }
            let mut criteria = state.active.criteria.clone();
            criteria.extend(added);
            let effective_at = criteria
                .iter()
                .map(|c| c.added_at.as_str())
                .max()
                .unwrap_or(state.active.effective_at.as_str())
                .to_string();
            state.active = StandardVersion {
                id: version_id,
                effective_at,
                criteria,
            };
        }
    }
    state.events.push(event);
}

pub fn apply(state: &mut State, command: Command) -> Result<&Event, Rejection> {
    let event = decide(state, command)?;
    evolve(state, event);
    Ok(state.events.last().expect("event was appended"))
}

pub fn preview_change(state: &State, proposal_id: &str) -> Result<ImpactPreview, Rejection> {
    let proposal = state
        .proposals
        .iter()
        .find(|p| p.id == proposal_id)
        .ok_or_else(|| Rejection::ProposalMissing(proposal_id.to_string()))?;
    if proposal.status != ProposalStatus::Open {
        return Err(Rejection::ProposalNotOpen(proposal.id.clone()));
    }
    let current = assessments_on_active(state);
    let bound = state
        .interviews
        .iter()
        .filter(|i| i.rubric_version == state.active.id && i.status == "open")
        .count();
    Ok(ImpactPreview {
        proposal_id: proposal.id.clone(),
        criteria_added: proposal.add.iter().map(|c| c.text.clone()).collect(),
        assessments_that_would_become_stale: current,
        interviews_bound_to_earlier_rubric: bound,
    })
}

fn assessments_on_active(state: &State) -> usize {
    state
        .assessments
        .iter()
        .filter(|a| a.version == state.active.id)
        .count()
}

fn payload_field(payload: &str, key: &str) -> String {
    payload
        .split(';')
        .find_map(|part| part.strip_prefix(&format!("{key}=")))
        .unwrap_or("")
        .to_string()
}
