use record_core::{
    apply, evaluate_report, load_fixture, preview_change, state_from_fixture, Command, Outcome,
    ProposalStatus, RefusalCode,
};

fn opened() -> record_core::State {
    let fixture = load_fixture(&record_core::fixture_path()).expect("fixture");
    state_from_fixture(&fixture)
}

#[test]
fn dismiss_leaves_scores_current() {
    let mut state = opened();
    let before: Vec<_> = state
        .assessments
        .iter()
        .map(|a| (a.id.clone(), a.version.clone(), a.met))
        .collect();
    apply(
        &mut state,
        Command::Dismiss {
            proposal_id: "B".into(),
        },
    )
    .unwrap();
    assert_eq!(state.active.id, "A");
    assert!(state
        .proposals
        .iter()
        .any(|p| p.id == "B" && p.status == ProposalStatus::Dismissed));
    let after: Vec<_> = state
        .assessments
        .iter()
        .map(|a| (a.id.clone(), a.version.clone(), a.met))
        .collect();
    assert_eq!(before, after);
    assert!(state.assessments.iter().all(|a| !state.is_stale(a)));
    assert!(state.events.len() == 1);
}

#[test]
fn approve_makes_earlier_assessments_stale_without_rewriting_them() {
    let mut state = opened();
    let versions_before: Vec<_> = state.assessments.iter().map(|a| a.version.clone()).collect();
    let preview = preview_change(&state, "B").unwrap();
    assert_eq!(preview.assessments_that_would_become_stale, 8);
    assert_eq!(preview.interviews_bound_to_earlier_rubric, 1);

    apply(
        &mut state,
        Command::Approve {
            proposal_id: "B".into(),
            new_version_id: "B2".into(),
        },
    )
    .unwrap();

    assert_eq!(state.active.id, "B2");
    assert_eq!(
        versions_before,
        state
            .assessments
            .iter()
            .map(|a| a.version.clone())
            .collect::<Vec<_>>()
    );
    assert!(state.assessments.iter().all(|a| state.is_stale(a)));
    assert_eq!(
        state.interviews[0].rubric_version,
        "A",
        "an open interview keeps the rubric it started on"
    );
}

#[test]
fn report_prints_naive_counts_and_refuses_both_predicted_claims() {
    let mut state = opened();
    apply(
        &mut state,
        Command::Approve {
            proposal_id: "B".into(),
            new_version_id: "B2".into(),
        },
    )
    .unwrap();
    let report = evaluate_report(&state);
    let launch = report
        .lines
        .iter()
        .find(|l| l.criterion_id == "led_a_launch")
        .unwrap();
    assert_eq!(launch.naive, "3 of 3 hires");
    assert!(!launch.eligible);
    assert_eq!(launch.refusal, Some(RefusalCode::RAfterHire));

    let hands = report
        .lines
        .iter()
        .find(|l| l.criterion_id == "still_hands_on")
        .unwrap();
    assert_eq!(hands.naive, "3 of 3 hires");
    assert!(!hands.eligible);
    assert_eq!(hands.refusal, Some(RefusalCode::RRejectsMet));

    let built = report
        .lines
        .iter()
        .find(|l| l.criterion_id == "built_from_zero")
        .unwrap();
    assert_eq!(built.naive, "3 of 3 hires");
    assert!(built.eligible);
    assert!(built.refusal.is_none());

    assert!(state
        .decisions
        .iter()
        .any(|d| d.person == "dee" && d.outcome == Outcome::Reject));
}

#[test]
fn second_approve_is_rejected() {
    let mut state = opened();
    apply(
        &mut state,
        Command::Approve {
            proposal_id: "B".into(),
            new_version_id: "B2".into(),
        },
    )
    .unwrap();
    let err = apply(
        &mut state,
        Command::Approve {
            proposal_id: "B".into(),
            new_version_id: "B3".into(),
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("not open"));
    assert_eq!(state.events.len(), 1);
}

#[test]
fn event_hash_repeats() {
    let mut state = opened();
    apply(
        &mut state,
        Command::Dismiss {
            proposal_id: "B".into(),
        },
    )
    .unwrap();
    let event = &state.events[0];
    assert_eq!(event.hash, record_core::hash_event(event));
    assert_eq!(event.prev_hash, "genesis");
}
