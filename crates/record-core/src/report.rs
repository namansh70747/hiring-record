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

    let added_after_hires = !hires.is_empty()
        && hires.iter().all(|d| criterion.added_at.as_str() > d.at.as_str());
    let every_reject_met = !rejects.is_empty()
        && rejects.iter().all(|d| met(state, &d.person, &criterion.id));

    let (eligible, refusal, reason) = if added_after_hires {
        (
            false,
            Some(RefusalCode::RAfterHire),
            format!(
                "{} was added {}, after those hires. The count stays. It cannot be reported as predicted.",
                criterion.text, criterion.added_at
            ),
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
    let (ack_ok, ack_code, ack_reason) = if total > 0 && acked == total {
        (true, None, "Every receipt in this batch has an acknowledgement.".into())
    } else {
        (
            false,
            Some(RefusalCode::RNoAck),
            "A receipt is still unacknowledged. The operation stays pending.".into(),
        )
    };
    let stale = state.assessments.iter().filter(|a| state.is_stale(a)).count();
    let (stale_ok, stale_code, stale_reason) = if stale == 0 {
        (true, None, "Assessments match the active standard.".into())
    } else {
        (
            false,
            Some(RefusalCode::RStale),
            format!("{stale} assessments still cite an older standard and cannot be read as current."),
        )
    };
    let rubric_old = state.interviews.iter().any(|i| i.rubric_version != state.active.id);
    let uncited = state.assessments.iter().any(|a| a.citation.is_none());
    let mut lines = vec![
        line("op-ack", "Acknowledged by Northwind", &format!("{acked} of {total} receipts"), ack_ok, ack_code, ack_reason),
        line("op-sync", "Synced to Northwind", "0 operations", false, Some(RefusalCode::RNoOp), "Northwind declares no push operation, so this cannot be reported as synced.".into()),
        line(
            "op-hold",
            "Debrief booked",
            if state.hold_confirmed { "confirmed" } else { "1 hold" },
            state.hold_confirmed,
            if state.hold_confirmed { None } else { Some(RefusalCode::RUnconfirmed) },
            if state.hold_confirmed { "A person confirmed the hold.".into() } else { "The hold has no confirmation, so it is not a booking.".into() },
        ),
        line("op-stale", "Assessments current", &format!("{stale} stale"), stale_ok, stale_code, stale_reason),
        line(
            "op-rubric",
            "Interview on the active standard",
            if rubric_old { "bound earlier" } else { "bound to active" },
            !rubric_old,
            if rubric_old { Some(RefusalCode::RRubricOld) } else { None },
            if rubric_old { "The open interview keeps the rubric it started on.".into() } else { "The interview rubric matches the active standard.".into() },
        ),
        line(
            "op-cite",
            "Scores cite a source moment",
            if uncited { "missing" } else { "cited" },
            !uncited,
            if uncited { Some(RefusalCode::RUncited) } else { None },
            if uncited { "A score cites a moment that is not in the record.".into() } else { "Every assessment on this role cites a source.".into() },
        ),
    ];
    if state.quote_declined {
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
