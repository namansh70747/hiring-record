use record_core::{
    apply, create_replay_bundle, evaluate_report, load_fixture, state_from_fixture, verify_bundle,
    Command, RefusalCode,
};

fn opened() -> record_core::State {
    state_from_fixture(&load_fixture(&record_core::fixture_path()).unwrap())
}

#[test]
fn absent_hold_interview_and_receipt_are_not_fabricated() {
    let state = record_core::State::empty();
    let report = evaluate_report(&state);
    let hold = report
        .lines
        .iter()
        .find(|l| l.criterion_id == "op-hold")
        .unwrap();
    assert_eq!(hold.naive, "No hold recorded");
    assert!(hold.refusal.is_none());
    let interview = report
        .lines
        .iter()
        .find(|l| l.criterion_id == "op-rubric")
        .unwrap();
    assert_eq!(interview.naive, "No interview to evaluate");
    assert!(interview.refusal.is_none());
    let receipt = report
        .lines
        .iter()
        .find(|l| l.criterion_id == "op-ack")
        .unwrap();
    assert_eq!(receipt.naive, "No delivery requested");
    assert!(receipt.refusal.is_none());
}

#[test]
fn supporter_rule_is_not_hard_coded_to_one_name() {
    let mut state = record_core::State::empty();
    apply(
        &mut state,
        Command::OpenRole {
            role_id: "custom".into(),
            standard: record_core::StandardVersion {
                id: "v1".into(),
                effective_at: "2026-01-01".into(),
                criteria: vec![record_core::Criterion {
                    id: "build".into(),
                    text: "Built systems".into(),
                    added_at: "2026-01-01".into(),
                    added_by: "alex".into(),
                }],
            },
        },
    )
    .unwrap();
    apply(
        &mut state,
        Command::AddPerson {
            person: record_core::Person {
                id: "sam".into(),
                name: "Sam".into(),
                place: "hired".into(),
            },
        },
    )
    .unwrap();
    apply(
        &mut state,
        Command::AddPerson {
            person: record_core::Person {
                id: "ria".into(),
                name: "Ria".into(),
                place: "rejected".into(),
            },
        },
    )
    .unwrap();
    for (id, person, met) in [("s1", "sam", true), ("s2", "ria", false)] {
        apply(
            &mut state,
            Command::RecordSourceAndAssessment {
                source: record_core::SourceMoment {
                    id: id.into(),
                    person: person.into(),
                    kind: "note".into(),
                    timestamp: "00:00:01".into(),
                    quote: "Synthetic evidence.".into(),
                },
                assessment: record_core::Assessment {
                    id: format!("a-{id}"),
                    person: person.into(),
                    version: "v1".into(),
                    criterion: "build".into(),
                    met,
                    citation: Some(id.into()),
                    unknown: false,
                },
            },
        )
        .unwrap();
    }
    apply(
        &mut state,
        Command::RecordDecision {
            decision: record_core::Decision {
                person: "sam".into(),
                outcome: record_core::Outcome::Hire,
                at: "2026-02-01".into(),
                version: "v1".into(),
            },
        },
    )
    .unwrap();
    apply(
        &mut state,
        Command::RecordDecision {
            decision: record_core::Decision {
                person: "ria".into(),
                outcome: record_core::Outcome::Reject,
                at: "2026-02-02".into(),
                version: "v1".into(),
            },
        },
    )
    .unwrap();
    apply(
        &mut state,
        Command::RemoveSupporter {
            supporter_id: "alex".into(),
        },
    )
    .unwrap();
    let line = evaluate_report(&state)
        .lines
        .into_iter()
        .find(|l| l.criterion_id == "build")
        .unwrap();
    assert_eq!(line.naive, "1 of 1 hires");
    assert_eq!(line.refusal, Some(RefusalCode::RSupportGone));
}

#[test]
fn unknown_assessment_does_not_become_not_met() {
    let mut state = opened();
    let before = state.evidence.len();
    apply(
        &mut state,
        Command::Assess {
            assessment: record_core::Assessment {
                id: "unknown-ada".into(),
                person: "ada".into(),
                version: "A".into(),
                criterion: "built_from_zero".into(),
                met: false,
                citation: None,
                unknown: true,
            },
        },
    )
    .unwrap();
    assert_eq!(state.evidence.len(), before);
    assert!(state
        .assessments
        .iter()
        .any(|a| a.id == "unknown-ada" && a.unknown));
}

#[test]
fn partial_acknowledgement_keeps_r_no_ack_until_complete() {
    let mut state = opened();
    apply(
        &mut state,
        Command::Approve {
            proposal_id: "B".into(),
            new_version_id: "B2".into(),
        },
    )
    .unwrap();
    state.receipts[0].expected_acks = 2;
    apply(
        &mut state,
        Command::RecordAcknowledgement {
            receipt_id: "nw-B2".into(),
            acknowledgement_id: "ack-1".into(),
        },
    )
    .unwrap();
    let pending = evaluate_report(&state)
        .lines
        .into_iter()
        .find(|l| l.criterion_id == "op-ack")
        .unwrap();
    assert_eq!(pending.refusal, Some(RefusalCode::RNoAck));
    assert!(pending.reason.contains("acknowledged=1"));
    assert!(pending.reason.contains("expected=2"));
    apply(
        &mut state,
        Command::RecordAcknowledgement {
            receipt_id: "nw-B2".into(),
            acknowledgement_id: "ack-1".into(),
        },
    )
    .unwrap_err();
    apply(
        &mut state,
        Command::RecordAcknowledgement {
            receipt_id: "nw-B2".into(),
            acknowledgement_id: "ack-2".into(),
        },
    )
    .unwrap();
    assert!(state.receipts[0].acknowledged);
}

#[test]
fn replayable_bundle_uses_its_own_baseline() {
    let mut state = opened();
    apply(
        &mut state,
        Command::Dismiss {
            proposal_id: "B".into(),
        },
    )
    .unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&[9; 32]);
    let bundle = create_replay_bundle(&state, &key, "report-2026").unwrap();
    let checked = verify_bundle(&bundle, Some(&bundle.public_key)).unwrap();
    assert!(checked.signature_valid && checked.chain_valid && checked.trusted_signer);
    assert_eq!(checked.replay, "verified");
    let legacy = record_core::create_bundle(&state, &key).unwrap();
    assert_eq!(verify_bundle(&legacy, None).unwrap().replay, "unsupported");
}

#[test]
fn provenance_does_not_change_the_v1_hash() {
    let mut state = opened();
    let mut event = record_core::decide(
        &state,
        Command::Dismiss {
            proposal_id: "B".into(),
        },
    )
    .unwrap();
    let hash = event.hash.clone();
    record_core::attach_provenance(&mut event, None, Some("synthetic-operator".into()));
    assert_eq!(event.hash, hash);
    assert_eq!(record_core::hash_event(&event), hash);
    record_core::evolve(&mut state, event);
    assert_eq!(state.events[0].envelope.as_ref().unwrap().recorded_at, None);
}

#[test]
fn comparison_does_not_append_live_events() {
    let initial = opened();
    let mut current = initial.clone();
    apply(
        &mut current,
        Command::Approve {
            proposal_id: "B".into(),
            new_version_id: "B2".into(),
        },
    )
    .unwrap();
    let root = current.events.last().unwrap().hash.clone();
    let _ = record_core::compare_other_path(&initial, &current).unwrap();
    assert_eq!(current.events.last().unwrap().hash, root);
    assert_eq!(current.events.len(), 1);
}
