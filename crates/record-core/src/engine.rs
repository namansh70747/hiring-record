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
        Command::RequestHoldConfirmation if state.hold_confirmed || state.hold_pending => {
            return Err(Rejection::AlreadyRecorded)
        }
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
        if proposal.base != state.active.id {
            return Err(Rejection::Invalid(
                "proposal base is no longer active".into(),
            ));
        }
    }
    validate_command(state, &command)?;

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
        command => (
            EventKind::DomainCommandV2,
            serde_json::json!({"schema":2,"command":command}).to_string(),
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
        EventKind::DomainCommandV2 => {
            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&event.payload) {
                if let Ok(command) = serde_json::from_value::<Command>(payload["command"].clone()) {
                    evolve_domain(state, command);
                }
            }
        }
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
        EventKind::HoldConfirmed => {
            state.hold_confirmed = true;
            state.hold_pending = false;
        }
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
    let mut omitted_events = Vec::new();
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
            EventKind::DomainCommandV2 => {
                let parsed = serde_json::from_str::<serde_json::Value>(&event.payload)
                    .ok()
                    .and_then(|value| {
                        serde_json::from_value::<Command>(value["command"].clone()).ok()
                    });
                match parsed {
                    Some(
                        command @ (Command::RecordSource { .. }
                        | Command::RecordFact { .. }
                        | Command::OverrideDecision { .. }
                        | Command::RequestHoldConfirmation
                        | Command::OfferQuote { .. }
                        | Command::AcceptQuote { .. }
                        | Command::DeclineQuote { .. }),
                    ) => {
                        if apply(&mut branch, command).is_err() {
                            omitted_events.push(format!(
                                "Event {} could not be applied to the alternate path.",
                                event.seq
                            ));
                        }
                    }
                    _ => omitted_events.push(format!(
                        "Event {} depends on the original standard and was not replayed.",
                        event.seq
                    )),
                }
            }
            EventKind::ReceiptAcknowledged => {
                let id = payload_field(&event.payload, "receipt");
                if branch
                    .receipts
                    .iter()
                    .any(|r| r.id == id && !r.acknowledged)
                {
                    apply(&mut branch, Command::Acknowledge { receipt_id: id })?;
                } else {
                    omitted_events.push(format!(
                        "Event {} acknowledges a receipt absent from the alternate path.",
                        event.seq
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(Comparison {
        path: if dismiss { "dismissed" } else { "approved" }.into(),
        stale_assessments: branch
            .latest_assessments()
            .into_iter()
            .filter(|a| branch.is_stale(a))
            .count(),
        report: crate::evaluate_report(&branch),
        omitted_events,
    })
}

pub fn apply(state: &mut State, command: Command) -> Result<&Event, Rejection> {
    let event = decide(state, command)?;
    evolve(state, event);
    Ok(state.events.last().expect("event was appended"))
}

/// Replay validates the recorded transition as well as its hash before applying it.
/// Existing v1 event payloads remain supported alongside the v2 command envelope.
pub fn replay_event(state: &mut State, event: Event) -> Result<(), Rejection> {
    let command = match event.kind {
        EventKind::ProposalDismissed => Command::Dismiss {
            proposal_id: payload_field(&event.payload, "proposal"),
        },
        EventKind::StandardApproved => Command::Approve {
            proposal_id: payload_field(&event.payload, "proposal"),
            new_version_id: payload_field(&event.payload, "version"),
        },
        EventKind::ReceiptAcknowledged => Command::Acknowledge {
            receipt_id: payload_field(&event.payload, "receipt"),
        },
        EventKind::HoldConfirmed => Command::ConfirmHold,
        EventKind::SupportRemoved => Command::RemoveSupport,
        EventKind::DomainCommandV2 => {
            let envelope: serde_json::Value = serde_json::from_str(&event.payload)
                .map_err(|_| Rejection::Invalid("invalid event envelope".into()))?;
            if envelope["schema"] != 2 {
                return Err(Rejection::Invalid("unsupported event schema".into()));
            }
            serde_json::from_value(envelope["command"].clone())
                .map_err(|_| Rejection::Invalid("invalid event command".into()))?
        }
    };
    let expected = decide(state, command)?;
    if expected != event {
        return Err(Rejection::Invalid(
            "event does not match the expected transition or hash".into(),
        ));
    }
    evolve(state, event);
    Ok(())
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
        .latest_assessments()
        .into_iter()
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

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
}
fn valid_date(date: &str) -> bool {
    let b = date.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || !b
            .iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    let year = date[0..4].parse::<u32>().unwrap_or(0);
    let month = date[5..7].parse::<u32>().unwrap_or(0);
    let day = date[8..10].parse::<u32>().unwrap_or(0);
    let limit = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    };
    year > 0 && day > 0 && day <= limit
}
fn validate_command(state: &State, command: &Command) -> Result<(), Rejection> {
    let invalid = |message: &str| Rejection::Invalid(message.into());
    let person_exists = |id: &str| state.people.iter().any(|p| p.id == id);
    match command {
        Command::OpenRole { role_id, standard } => {
            if !state.role_id.is_empty()
                || !valid_id(role_id)
                || !valid_id(&standard.id)
                || !valid_date(&standard.effective_at)
                || standard.criteria.is_empty()
                || standard.criteria.len() > 50
            {
                return Err(invalid(
                    "new roles need a unique ID, a dated initial standard and criteria",
                ));
            }
            let mut ids = std::collections::HashSet::new();
            for criterion in &standard.criteria {
                if !valid_id(&criterion.id)
                    || !ids.insert(&criterion.id)
                    || criterion.text.trim().is_empty()
                    || criterion.text.len() > 300
                    || !valid_date(&criterion.added_at)
                    || !valid_id(&criterion.added_by)
                {
                    return Err(invalid(
                        "initial criteria require unique IDs, text, valid dates and authors",
                    ));
                }
            }
        }
        Command::AddPerson { person } => {
            if state.role_id.is_empty()
                || !valid_id(&person.id)
                || person.name.trim().is_empty()
                || person.name.len() > 100
                || !["in_review", "hired", "rejected"].contains(&person.place.as_str())
                || state.people.iter().any(|p| p.id == person.id)
            {
                return Err(invalid(
                    "person needs a unique ID, name and supported stage",
                ));
            }
        }
        Command::Propose { proposal } => {
            if !valid_id(&proposal.id)
                || state.proposals.iter().any(|p| p.id == proposal.id)
                || proposal.base != state.active.id
                || proposal.status != ProposalStatus::Open
                || proposal.add.is_empty()
                || proposal.add.len() > 10
            {
                return Err(invalid(
                    "proposal must have a unique ID, active base and new criteria",
                ));
            }
            let mut ids = std::collections::HashSet::new();
            for c in &proposal.add {
                if !valid_id(&c.id)
                    || !ids.insert(&c.id)
                    || c.text.trim().is_empty()
                    || c.text.len() > 300
                    || !valid_date(&c.added_at)
                    || !valid_id(&c.added_by)
                    || state.active.criteria.iter().any(|old| old.id == c.id)
                {
                    return Err(invalid(
                        "criterion IDs must be unique and new, with nonempty text",
                    ));
                }
            }
        }
        Command::RecordSource { source } => {
            if !valid_id(&source.id)
                || !person_exists(&source.person)
                || source.quote.trim().is_empty()
                || source.quote.len() > 5000
                || source.kind.trim().is_empty()
                || source.kind.len() > 100
                || source.timestamp.len() > 64
                || state.sources.iter().any(|s| s.id == source.id)
            {
                return Err(invalid("source needs a unique ID, known person and quote"));
            }
        }
        Command::Assess { assessment } => {
            if !valid_id(&assessment.id)
                || state.assessments.iter().any(|a| a.id == assessment.id)
                || !person_exists(&assessment.person)
                || assessment.version != state.active.id
                || !state
                    .active
                    .criteria
                    .iter()
                    .any(|c| c.id == assessment.criterion)
            {
                return Err(invalid("assessment must reference a known person and active criterion, with a unique ID"));
            }
            if let Some(citation) = &assessment.citation {
                if !state
                    .sources
                    .iter()
                    .any(|s| s.id == *citation && s.person == assessment.person)
                {
                    return Err(invalid(
                        "citation belongs to a different person or does not exist",
                    ));
                }
            }
        }
        Command::RecordFact { fact } => {
            if !valid_id(&fact.id)
                || state.facts.iter().any(|f| f.id == fact.id)
                || fact.key.trim().is_empty()
                || fact.key.len() > 100
                || fact.value.trim().is_empty()
                || fact.value.len() > 1000
                || !state
                    .sources
                    .iter()
                    .any(|s| s.id == fact.source_id && s.person == fact.person)
            {
                return Err(invalid("fact needs a unique ID and a matching source; conflicting facts remain separate"));
            }
        }
        Command::RecordDecision { decision } => {
            if !person_exists(&decision.person)
                || decision.version != state.active.id
                || !valid_date(&decision.at)
                || state.decisions.iter().any(|d| d.person == decision.person)
            {
                return Err(invalid(
                    "decision requires a known undecided person and active standard",
                ));
            }
        }
        Command::OverrideDecision {
            decision_override: o,
        } => {
            if !valid_id(&o.id)
                || !person_exists(&o.person)
                || o.reason.trim().is_empty()
                || o.reason.len() > 1000
                || !valid_date(&o.at)
                || state.overrides.iter().any(|old| old.id == o.id)
            {
                return Err(invalid(
                    "override needs a unique ID, known person and human reason",
                ));
            }
        }
        Command::BindInterview { binding } => {
            if !person_exists(&binding.person)
                || binding.rubric_version != state.active.id
                || state.interviews.iter().any(|i| i.person == binding.person)
                || binding.status != "open"
            {
                return Err(invalid("interview must bind once to the current standard"));
            }
        }
        Command::OfferQuote { quote } => {
            if !valid_id(&quote.id)
                || !person_exists(&quote.person)
                || quote.amount_minor == 0
                || quote.amount_minor > 100000000
                || !["USD", "INR", "EUR", "GBP"].contains(&quote.currency.as_str())
                || quote.status != "offered"
                || state.quotes.iter().any(|q| q.id == quote.id)
            {
                return Err(invalid(
                    "quote needs a unique ID, known person, positive amount and supported currency",
                ));
            }
        }
        Command::AcceptQuote { quote_id } | Command::DeclineQuote { quote_id } => {
            if !state
                .quotes
                .iter()
                .any(|q| q.id == *quote_id && q.status == "offered")
            {
                return Err(invalid("only an offered quote can be accepted or declined"));
            }
        }
        Command::Approve { new_version_id, .. }
            if !valid_id(new_version_id) || new_version_id == &state.active.id =>
        {
            return Err(invalid("new standard ID must be distinct and valid"));
        }
        _ => {}
    }
    Ok(())
}
fn evolve_domain(state: &mut State, command: Command) {
    match command {
        Command::OpenRole { role_id, standard } => {
            state.role_id = role_id;
            state.active = standard;
        }
        Command::AddPerson { person } => state.people.push(person),
        Command::Propose { proposal } => state.proposals.push(proposal),
        Command::RecordSource { source } => state.sources.push(source),
        Command::Assess { assessment } => {
            // Capture pre-decision evidence. Later reassessments must not rewrite historical decisions.
            if !state
                .decisions
                .iter()
                .any(|decision| decision.person == assessment.person)
            {
                state.evidence.retain(|e| {
                    e.person != assessment.person || e.criterion != assessment.criterion
                });
                state.evidence.push(Evidence {
                    person: assessment.person.clone(),
                    criterion: assessment.criterion.clone(),
                    met: assessment.met,
                });
            }
            state.assessments.push(assessment);
        }
        Command::RecordFact { fact } => state.facts.push(fact),
        Command::RecordDecision { decision } => {
            if let Some(person) = state.people.iter_mut().find(|p| p.id == decision.person) {
                person.place = match decision.outcome {
                    Outcome::Hire => "hired",
                    Outcome::Reject => "rejected",
                }
                .into();
            }
            state.decisions.push(decision);
        }
        Command::OverrideDecision { decision_override } => state.overrides.push(decision_override),
        Command::BindInterview { binding } => state.interviews.push(binding),
        Command::RequestHoldConfirmation => state.hold_pending = true,
        Command::OfferQuote { quote } => state.quotes.push(quote),
        Command::AcceptQuote { quote_id } => {
            if let Some(quote) = state.quotes.iter_mut().find(|q| q.id == quote_id) {
                quote.status = "accepted".into();
            }
            state.receipts.push(Receipt {
                id: format!("quote-{quote_id}"),
                label: "Simulated quote delivery".into(),
                acknowledged: false,
            });
        }
        Command::DeclineQuote { quote_id } => {
            if let Some(quote) = state.quotes.iter_mut().find(|q| q.id == quote_id) {
                quote.status = "declined".into();
            }
            state.quote_declined = true;
        }
        _ => {}
    }
}
