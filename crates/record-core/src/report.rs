use crate::model::*;

pub fn evaluate_report(state: &State) -> Report {
    let hires: Vec<&Decision> = state
        .decisions
        .iter()
        .filter(|d| d.outcome == Outcome::Hire)
        .collect();
    let rejects: Vec<&Decision> = state
        .decisions
        .iter()
        .filter(|d| d.outcome == Outcome::Reject)
        .collect();

    let mut lines: Vec<ClaimLine> = state
        .active
        .criteria
        .iter()
        .map(|criterion| claim_line(state, criterion, &hires, &rejects))
        .collect();
    lines.extend(operation_lines(state));

    Report {
        role_id: state.role_id.clone(),
        standard_id: state.active.id.clone(),
        lines,
    }
}

fn claim_line(
    state: &State,
    criterion: &Criterion,
    hires: &[&Decision],
    rejects: &[&Decision],
) -> ClaimLine {
    let hires_met = hires
        .iter()
        .filter(|d| met(state, &d.person, &criterion.id))
        .count();
    let hires_total = hires.len();
    let naive = format!("{hires_met} of {hires_total} hires");

    let added_after_hires = hires
        .iter()
        .any(|d| criterion.added_at.as_str() > d.at.as_str());
    let same_day_as_hire = hires.iter().any(|d| criterion.added_at == d.at);
    let missing_evidence = hires.iter().chain(rejects.iter()).any(|d| {
        !state
            .evidence
            .iter()
            .any(|e| e.person == d.person && e.criterion == criterion.id)
    });
    let every_reject_met =
        !rejects.is_empty() && rejects.iter().all(|d| met(state, &d.person, &criterion.id));

    let (eligible, refusal, reason) = if added_after_hires {
        (
            false,
            Some(RefusalCode::RAfterHire),
            format!(
                "{} was added {}, after at least one of those hires. The count stays. It cannot be reported as predicted.",
                criterion.text, criterion.added_at
            ),
        )
    } else if same_day_as_hire {
        (
            false,
            Some(RefusalCode::RInsufficientData),
            "The criterion and a hire share a date; without times their order cannot be established.".into(),
        )
    } else if hires.is_empty() || rejects.is_empty() {
        (
            false,
            Some(RefusalCode::RInsufficientData),
            "A prediction comparison requires both hires and rejects in the decision set.".into(),
        )
    } else if missing_evidence {
        (
            false,
            Some(RefusalCode::RIncompleteEvidence),
            "Evidence is missing for at least one decision. Missing information is not evidence that a criterion was not met.".into(),
        )
    } else if every_reject_met {
        (
            false,
            Some(RefusalCode::RRejectsMet),
            format!(
                "{} was also met by every reject in this decision set. The count stays. It cannot be reported as predicted.",
                criterion.text
            ),
        )
    } else if criterion.added_by == "mina" && !state.support_present {
        (
            false,
            Some(RefusalCode::RSupportGone),
            format!(
                "{} kept its historical count. Mina, who added it, is no longer support, so it cannot be repeated as predicted.",
                criterion.text
            ),
        )
    } else {
        (
            true,
            None,
            format!(
                "{} was on the standard before those hires, and the rejects did not all meet it.",
                criterion.text
            ),
        )
    };

    ClaimLine {
        criterion_id: criterion.id.clone(),
        text: criterion.text.clone(),
        naive,
        hires_met,
        hires_total,
        eligible,
        refusal,
        reason,
    }
}

fn operation_lines(state: &State) -> Vec<ClaimLine> {
    let total = state.receipts.len();
    let acked = state.receipts.iter().filter(|r| r.acknowledged).count();
    let (ack_ok, ack_code, ack_reason) = if total == 0 {
        (
            false,
            Some(RefusalCode::RNoAck),
            "No receipt has been created, so there is no acknowledgement to report.".into(),
        )
    } else if acked == total {
        (
            true,
            None,
            "Every receipt in this batch has an acknowledgement.".into(),
        )
    } else {
        (
            false,
            Some(RefusalCode::RNoAck),
            "A receipt is still unacknowledged. The operation stays pending.".into(),
        )
    };
    let stale = state
        .latest_assessments()
        .into_iter()
        .filter(|a| state.is_stale(a))
        .count();
    let missing_current = state.people.iter().any(|person| {
        state.active.criteria.iter().any(|criterion| {
            !state.assessments.iter().any(|a| {
                a.person == person.id && a.criterion == criterion.id && a.version == state.active.id
            })
        })
    });
    let (stale_ok, stale_code, stale_reason) = if stale == 0 && missing_current {
        (
            false,
            Some(RefusalCode::RIncompleteEvidence),
            "At least one person has no assessment for a current criterion.".into(),
        )
    } else if stale == 0 {
        (true, None, "Assessments match the active standard.".into())
    } else {
        (
            false,
            Some(RefusalCode::RStale),
            format!(
                "{stale} assessments still cite an older standard and cannot be read as current."
            ),
        )
    };
    let rubric_old = state
        .interviews
        .iter()
        .any(|i| i.rubric_version != state.active.id);
    let uncited = state.assessments.iter().any(|assessment| {
        !assessment.citation.as_ref().is_some_and(|id| {
            state
                .sources
                .iter()
                .any(|source| source.id == *id && source.person == assessment.person)
        })
    });
    let mut lines = vec![
        line(
            "op-ack",
            "Acknowledged by Northwind",
            &format!("{acked} of {total} receipts"),
            ack_ok,
            ack_code,
            ack_reason,
        ),
        line(
            "op-sync",
            "Synced to Northwind",
            "0 operations",
            false,
            Some(RefusalCode::RNoOp),
            "Northwind declares no push operation, so this cannot be reported as synced.".into(),
        ),
        line(
            "op-hold",
            "Debrief booked",
            if state.hold_confirmed {
                "confirmed"
            } else {
                "1 hold"
            },
            state.hold_confirmed,
            if state.hold_confirmed {
                None
            } else {
                Some(RefusalCode::RUnconfirmed)
            },
            if state.hold_confirmed {
                "A person confirmed the hold.".into()
            } else {
                "The hold has no confirmation, so it is not a booking.".into()
            },
        ),
        line(
            "op-stale",
            "Assessments current",
            &format!("{stale} stale"),
            stale_ok,
            stale_code,
            stale_reason,
        ),
        line(
            "op-rubric",
            "Interview on the active standard",
            if rubric_old {
                "bound earlier"
            } else {
                "bound to active"
            },
            !rubric_old,
            if rubric_old {
                Some(RefusalCode::RRubricOld)
            } else {
                None
            },
            if rubric_old {
                "The open interview keeps the rubric it started on.".into()
            } else {
                "The interview rubric matches the active standard.".into()
            },
        ),
        line(
            "op-cite",
            "Scores cite a source moment",
            if uncited { "missing" } else { "cited" },
            !uncited,
            if uncited {
                Some(RefusalCode::RUncited)
            } else {
                None
            },
            if uncited {
                "At least one assessment has no matching source moment for that person.".into()
            } else {
                "Each assessment references a matching synthetic source moment. This check does not establish whether the interpretation is correct.".into()
            },
        ),
    ];
    for quote in &state.quotes {
        let (eligible, code, reason) = match quote.status.as_str() {
            "accepted" => (
                true,
                None,
                "The quote was accepted; its delivery receipt is checked separately.",
            ),
            "declined" => (
                false,
                Some(RefusalCode::RDeclined),
                "The quote was declined, so no delivery was requested for it.",
            ),
            _ => (
                false,
                Some(RefusalCode::RUnconfirmed),
                "The quote is still offered and has not been accepted.",
            ),
        };
        lines.push(line(
            &format!("op-quote-{}", quote.id),
            "Quote accepted",
            &quote.status,
            eligible,
            code,
            reason.into(),
        ));
    }
    if state.quote_declined && state.quotes.is_empty() {
        lines.push(line(
            "op-quote",
            "Quote accepted",
            "declined",
            false,
            Some(RefusalCode::RDeclined),
            "The quote was declined, so nothing was sent.".into(),
        ));
    }
    lines
}

fn line(
    id: &str,
    text: &str,
    naive: &str,
    eligible: bool,
    refusal: Option<RefusalCode>,
    reason: String,
) -> ClaimLine {
    ClaimLine {
        criterion_id: id.into(),
        text: text.into(),
        naive: naive.into(),
        hires_met: 0,
        hires_total: 0,
        eligible,
        refusal,
        reason,
    }
}

fn met(state: &State, person: &str, criterion: &str) -> bool {
    state
        .evidence
        .iter()
        .any(|e| e.person == person && e.criterion == criterion && e.met)
}
