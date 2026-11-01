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

    let lines = state
        .active
        .criteria
        .iter()
        .map(|criterion| claim_line(state, criterion, &hires, &rejects))
        .collect();

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

fn met(state: &State, person: &str, criterion: &str) -> bool {
    state
        .evidence
        .iter()
        .any(|e| e.person == person && e.criterion == criterion && e.met)
}
