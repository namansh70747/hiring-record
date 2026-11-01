use crate::model::*;
use crate::{canonical_event, hash_event};

pub fn decide(state: &State, command: Command) -> Result<Event, Rejection> {
    match &command {
        Command::Acknowledge { receipt_id }
            if !state
                .receipts
                .iter()
                .any(|r| r.id == *receipt_id && !r.acknowledged) =>
        {
            return Err(Rejection::ReceiptUnavailable(receipt_id.clone()));
        }
        Command::ConfirmHold if state.hold_confirmed => return Err(Rejection::AlreadyRecorded),
        Command::RemoveSupport if !state.support_present => return Err(Rejection::AlreadyRecorded),
        _ => {}
    }
    if let Command::Dismiss { proposal_id } | Command::Approve { proposal_id, .. } = &command {
        let proposal = state
            .proposals
            .iter()
            .find(|p| p.id == *proposal_id)
            .ok_or_else(|| Rejection::ProposalMissing(proposal_id.clone()))?;
        if proposal.status != ProposalStatus::Open {
            return Err(Rejection::ProposalNotOpen(proposal.id.clone()));
        }
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
        Command::Acknowledge { receipt_id } => (
            EventKind::ReceiptAcknowledged,
            format!("receipt={receipt_id}"),
        ),
        Command::ConfirmHold => (EventKind::HoldConfirmed, "hold=debrief".into()),
        Command::RemoveSupport => (EventKind::SupportRemoved, "person=mina".into()),
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
                id: version_id.clone(),
                effective_at,
                criteria,
            };
            state.receipts.push(Receipt {
                id: format!("nw-{version_id}"),
                label: "Northwind stage write".into(),
                acknowledged: false,
            });
        }
        EventKind::ReceiptAcknowledged => {
            let id = payload_field(&event.payload, "receipt");
            if let Some(receipt) = state.receipts.iter_mut().find(|r| r.id == id) {
                receipt.acknowledged = true;
            }
        }
        EventKind::HoldConfirmed => state.hold_confirmed = true,
        EventKind::SupportRemoved => state.support_present = false,
    }
    state.events.push(event);
}

pub fn counterfactual_approve(state: &State) -> Result<Report, Rejection> {
    if state
        .proposals
        .iter()
        .any(|p| p.id == "B" && p.status == ProposalStatus::Approved)
    {
        return Ok(crate::evaluate_report(state));
    }
    let mut branch = state.clone();
    if let Some(proposal) = branch.proposals.iter_mut().find(|p| p.id == "B") {
        proposal.status = ProposalStatus::Open;
    }
    apply(
        &mut branch,
        Command::Approve {
            proposal_id: "B".into(),
            new_version_id: "B2-hypothetical".into(),
        },
    )?;
    Ok(crate::evaluate_report(&branch))
}

/// Compare the opposite proposal decision from the original fixture. Independent
/// hold and support events are replayed; an acknowledgement belonging to the
/// approved path is omitted when the proposal is dismissed.
pub fn compare_other_path(initial: &State, current: &State) -> Result<Comparison, Rejection> {
    let proposal = current
        .proposals
        .iter()
        .find(|p| p.id == "B")
        .ok_or_else(|| Rejection::ProposalMissing("B".into()))?;
    if !initial.events.is_empty()
        || initial.role_id != current.role_id
        || initial.active.id != proposal.base
    {
        return Err(Rejection::BaselineMismatch);
    }
    let dismiss = proposal.status == ProposalStatus::Approved;
    let mut branch = initial.clone();
    if dismiss {
        apply(
            &mut branch,
            Command::Dismiss {
                proposal_id: "B".into(),
            },
        )?;
    } else {
        apply(
            &mut branch,
            Command::Approve {
                proposal_id: "B".into(),
                new_version_id: "B2-hypothetical".into(),
            },
        )?;
    }
    for event in &current.events {
        match event.kind {
            EventKind::HoldConfirmed => {
                apply(&mut branch, Command::ConfirmHold)?;
            }
            EventKind::SupportRemoved => {
                apply(&mut branch, Command::RemoveSupport)?;
            }
            _ => {}
        }
    }
    Ok(Comparison {
        path: if dismiss { "dismissed" } else { "approved" }.into(),
        stale_assessments: branch
            .assessments
            .iter()
            .filter(|a| branch.is_stale(a))
            .count(),
        report: crate::evaluate_report(&branch),
    })
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
