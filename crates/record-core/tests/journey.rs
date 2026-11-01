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
    let versions_before: Vec<_> = state
        .assessments
        .iter()
        .map(|a| a.version.clone())
        .collect();
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
        state.interviews[0].rubric_version, "A",
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
fn acknowledgement_stays_pending_until_the_receipt_is_acked() {
    let mut state = opened();
    apply(
        &mut state,
        Command::Approve {
            proposal_id: "B".into(),
            new_version_id: "B2".into(),
        },
    )
    .unwrap();
    let pending = evaluate_report(&state);
    let ack = pending
        .lines
        .iter()
        .find(|l| l.criterion_id == "op-ack")
        .unwrap();
    assert_eq!(ack.refusal, Some(RefusalCode::RNoAck));
    apply(
        &mut state,
        Command::Acknowledge {
            receipt_id: "nw-B2".into(),
        },
    )
    .unwrap();
    let done = evaluate_report(&state);
    let ack = done
        .lines
        .iter()
        .find(|l| l.criterion_id == "op-ack")
        .unwrap();
    assert!(ack.eligible);
    let synced = done
        .lines
        .iter()
        .find(|l| l.criterion_id == "op-sync")
        .unwrap();
    assert_eq!(synced.refusal, Some(RefusalCode::RNoOp));
}

#[test]
fn counterfactual_does_not_change_the_live_record() {
    let mut state = opened();
    apply(
        &mut state,
        Command::Dismiss {
            proposal_id: "B".into(),
        },
    )
    .unwrap();
    let events = state.events.len();
    let hypothetical = record_core::counterfactual_approve(&state).unwrap();
    assert_eq!(state.events.len(), events);
    assert_eq!(state.active.id, "A");
    assert!(hypothetical
        .lines
        .iter()
        .any(|l| l.criterion_id == "led_a_launch" && !l.eligible));
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

#[test]
fn signed_bundle_detects_modified_payload_and_distinguishes_trust() {
    let mut state = opened();
    apply(
        &mut state,
        Command::Approve {
            proposal_id: "B".into(),
            new_version_id: "B2".into(),
        },
    )
    .unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let mut bundle = record_core::create_bundle(&state, &key).unwrap();
    let checked = record_core::verify_bundle(&bundle, Some(&bundle.public_key)).unwrap();
    assert!(checked.signature_valid && checked.chain_valid && checked.trusted_signer);
    assert!(
        !record_core::verify_bundle(&bundle, None)
            .unwrap()
            .trusted_signer
    );
    bundle.payload = bundle.payload.replace("3 of 3 hires", "4 of 3 hires");
    assert!(
        !record_core::verify_bundle(&bundle, None)
            .unwrap()
            .signature_valid
    );
}

#[test]
fn signed_bundle_detects_a_broken_event_chain_even_with_a_valid_signature() {
    use ed25519_dalek::Signer;
    let mut state = opened();
    apply(
        &mut state,
        Command::Dismiss {
            proposal_id: "B".into(),
        },
    )
    .unwrap();
    let key = ed25519_dalek::SigningKey::from_bytes(&[8; 32]);
    let mut bundle = record_core::create_bundle(&state, &key).unwrap();
    let mut payload: record_core::BundlePayload = serde_json::from_str(&bundle.payload).unwrap();
    payload.events[0].prev_hash = "wrong".into();
    bundle.payload = serde_json::to_string(&payload).unwrap();
    bundle.signature = hex::encode(key.sign(bundle.payload.as_bytes()).to_bytes());
    let checked = record_core::verify_bundle(&bundle, Some(&bundle.public_key)).unwrap();
    assert!(checked.signature_valid && checked.trusted_signer);
    assert!(!checked.chain_valid);
}

#[test]
fn missing_evidence_does_not_become_negative_evidence() {
    let mut state = opened();
    state
        .evidence
        .retain(|e| !(e.person == "dee" && e.criterion == "built_from_zero"));
    let report = evaluate_report(&state);
    let line = report
        .lines
        .iter()
        .find(|l| l.criterion_id == "built_from_zero")
        .unwrap();
    assert!(!line.eligible);
    assert_eq!(line.refusal, Some(RefusalCode::RIncompleteEvidence));
    assert_eq!(line.naive, "3 of 3 hires");
}

#[test]
fn empty_comparison_group_does_not_support_a_prediction() {
    let mut state = opened();
    state.decisions.retain(|d| d.outcome == Outcome::Hire);
    let report = evaluate_report(&state);
    let line = report
        .lines
        .iter()
        .find(|l| l.criterion_id == "built_from_zero")
        .unwrap();
    assert_eq!(line.refusal, Some(RefusalCode::RInsufficientData));
}

#[test]
fn a_criterion_after_any_hire_cannot_explain_all_prior_hires() {
    let mut state = opened();
    state
        .decisions
        .iter_mut()
        .find(|d| d.person == "ada")
        .unwrap()
        .at = "2026-08-01".into();
    let report = evaluate_report(&state);
    let line = report
        .lines
        .iter()
        .find(|l| l.criterion_id == "built_from_zero")
        .unwrap();
    assert_eq!(line.refusal, Some(RefusalCode::RAfterHire));
}

#[test]
fn simulation_replays_the_same_seed_and_preserves_invariants() {
    let state = opened();
    let first = record_core::simulate(&state, 125, 100).unwrap();
    let second = record_core::simulate(&state, 125, 100).unwrap();
    assert_eq!(first, second);
    assert!(first.failures.is_empty());
    assert!(first.checks >= 500);
    assert!(first.trace.iter().any(|step| step.path == "approved"));
    assert!(first.trace.iter().any(|step| step.path == "dismissed"));
}

#[test]
fn citations_must_resolve_to_a_source_for_the_same_person() {
    let mut state = opened();
    state.assessments[0].citation = Some("src-priya".into());
    let report = evaluate_report(&state);
    let line = report
        .lines
        .iter()
        .find(|l| l.criterion_id == "op-cite")
        .unwrap();
    assert_eq!(line.refusal, Some(RefusalCode::RUncited));
    state.assessments[0].citation = Some("src-not-a-real-moment".into());
    let report = evaluate_report(&state);
    assert_eq!(
        report
            .lines
            .iter()
            .find(|l| l.criterion_id == "op-cite")
            .unwrap()
            .refusal,
        Some(RefusalCode::RUncited)
    );
}

#[test]
fn removed_support_changes_eligibility_without_erasing_historical_count() {
    let mut state = opened();
    let before = evaluate_report(&state);
    let prior = before
        .lines
        .iter()
        .find(|line| line.criterion_id == "built_from_zero")
        .unwrap();
    assert!(prior.eligible);
    apply(&mut state, Command::RemoveSupport).unwrap();
    let after = evaluate_report(&state);
    let changed = after
        .lines
        .iter()
        .find(|line| line.criterion_id == "built_from_zero")
        .unwrap();
    assert_eq!(changed.naive, prior.naive);
    assert_eq!(changed.refusal, Some(RefusalCode::RSupportGone));
    let mut replay = opened();
    for event in &state.events {
        record_core::evolve(&mut replay, event.clone());
    }
    assert_eq!(replay, state);
}

#[test]
fn comparing_an_approved_path_replays_dismissal_and_independent_events() {
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
    apply(
        &mut current,
        Command::Acknowledge {
            receipt_id: "nw-B2".into(),
        },
    )
    .unwrap();
    apply(&mut current, Command::RemoveSupport).unwrap();
    let comparison = record_core::compare_other_path(&initial, &current).unwrap();
    assert_eq!(comparison.path, "dismissed");
    assert_eq!(comparison.report.standard_id, "A");
    assert_eq!(comparison.stale_assessments, 0);
    assert_eq!(
        comparison
            .report
            .lines
            .iter()
            .find(|l| l.criterion_id == "built_from_zero")
            .unwrap()
            .refusal,
        Some(RefusalCode::RSupportGone)
    );
    assert_eq!(
        current.events.len(),
        3,
        "comparison does not mutate the live path"
    );
}

#[test]
fn quote_and_fact_commands_preserve_history_and_replay() {
    let fixture = load_fixture(&record_core::fixture_path()).unwrap();
    let initial = state_from_fixture(&fixture);
    let mut state = initial.clone();
    let original = state.assessments.clone();
    apply(
        &mut state,
        Command::OfferQuote {
            quote: record_core::Quote {
                id: "q-priya".into(),
                person: "priya".into(),
                amount_minor: 25000,
                currency: "USD".into(),
                status: "offered".into(),
            },
        },
    )
    .unwrap();
    apply(
        &mut state,
        Command::DeclineQuote {
            quote_id: "q-priya".into(),
        },
    )
    .unwrap();
    assert!(state.receipts.is_empty());
    assert!(evaluate_report(&state)
        .lines
        .iter()
        .any(|l| l.refusal == Some(RefusalCode::RDeclined)));
    assert!(apply(
        &mut state,
        Command::AcceptQuote {
            quote_id: "q-priya".into()
        }
    )
    .is_err());
    apply(
        &mut state,
        Command::RecordFact {
            fact: record_core::SourcedFact {
                id: "f1".into(),
                person: "priya".into(),
                key: "availability".into(),
                value: "June".into(),
                source_id: "src-priya".into(),
            },
        },
    )
    .unwrap();
    apply(
        &mut state,
        Command::RecordFact {
            fact: record_core::SourcedFact {
                id: "f2".into(),
                person: "priya".into(),
                key: "availability".into(),
                value: "July".into(),
                source_id: "src-priya".into(),
            },
        },
    )
    .unwrap();
    assert_eq!(
        state.facts.len(),
        2,
        "conflicting sourced values are not overwritten"
    );
    assert_eq!(state.assessments, original);
    let mut replay = initial;
    for event in &state.events {
        record_core::evolve(&mut replay, event.clone());
    }
    assert_eq!(state, replay);
}

#[test]
fn accepted_quote_creates_pending_receipt_and_rejects_duplicate_resolution() {
    let fixture = load_fixture(&record_core::fixture_path()).unwrap();
    let mut state = state_from_fixture(&fixture);
    apply(
        &mut state,
        Command::OfferQuote {
            quote: record_core::Quote {
                id: "quote-1".into(),
                person: "priya".into(),
                amount_minor: 1000,
                currency: "INR".into(),
                status: "offered".into(),
            },
        },
    )
    .unwrap();
    apply(
        &mut state,
        Command::AcceptQuote {
            quote_id: "quote-1".into(),
        },
    )
    .unwrap();
    assert!(!state.receipts[0].acknowledged);
    assert!(apply(
        &mut state,
        Command::AcceptQuote {
            quote_id: "quote-1".into()
        }
    )
    .is_err());
    apply(
        &mut state,
        Command::Acknowledge {
            receipt_id: "quote-quote-1".into(),
        },
    )
    .unwrap();
    assert!(state.receipts[0].acknowledged);
}

proptest::proptest! {
    #[test]
    fn arbitrary_command_interleavings_preserve_replay_and_assessment_history(actions in proptest::collection::vec(0u8..5, 0..60)) {
        let initial=opened(); let mut state=initial.clone();
        for action in actions {
            let before=state.clone();
            let cmd=match action {
                0=>Command::Dismiss{proposal_id:"B".into()},
                1=>Command::Approve{proposal_id:"B".into(),new_version_id:"B2".into()},
                2=>Command::Acknowledge{receipt_id:"nw-B2".into()},
                3=>Command::ConfirmHold,
                _=>Command::RemoveSupport,
            };
            if apply(&mut state,cmd).is_err() { proptest::prop_assert_eq!(&state,&before); }
            proptest::prop_assert_eq!(&state.assessments,&initial.assessments);
            proptest::prop_assert_eq!(&state.interviews,&initial.interviews);
            if state.active.id=="B2" {proptest::prop_assert!(state.assessments.iter().all(|a|state.is_stale(a)));}
            else {proptest::prop_assert!(state.assessments.iter().all(|a|!state.is_stale(a)));}
            let mut replay=initial.clone();
            for event in &state.events {record_core::replay_event(&mut replay,event.clone()).unwrap();}
            proptest::prop_assert_eq!(&replay,&state);
        }
    }
}

#[test]
fn replay_rejects_hash_valid_but_semantically_tampered_events() {
    let mut state = opened();
    let mut event = record_core::decide(&state, Command::ConfirmHold).unwrap();
    event.payload = "hold=unexpected".into();
    event.hash = record_core::hash_event(&event);
    assert!(record_core::replay_event(&mut state, event).is_err());
    assert!(state.events.is_empty());
}

#[test]
fn counterfactual_preserves_independent_quote_history() {
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
    apply(
        &mut current,
        Command::OfferQuote {
            quote: record_core::Quote {
                id: "test-quote".into(),
                person: "priya".into(),
                amount_minor: 500,
                currency: "USD".into(),
                status: "offered".into(),
            },
        },
    )
    .unwrap();
    apply(
        &mut current,
        Command::AcceptQuote {
            quote_id: "test-quote".into(),
        },
    )
    .unwrap();
    let comparison = record_core::compare_other_path(&initial, &current).unwrap();
    assert_eq!(comparison.path, "dismissed");
    assert!(comparison
        .report
        .lines
        .iter()
        .any(|l| l.criterion_id == "op-quote-test-quote" && l.eligible));
    assert!(comparison.omitted_events.is_empty());
    assert_eq!(current.active.id, "B2");
}

#[test]
fn same_day_criterion_and_hire_cannot_establish_temporal_order() {
    let mut state = opened();
    state.active.criteria[0].added_at = state
        .decisions
        .iter()
        .find(|d| d.outcome == Outcome::Hire)
        .unwrap()
        .at
        .clone();
    let line = evaluate_report(&state)
        .lines
        .into_iter()
        .find(|l| l.criterion_id == "built_from_zero")
        .unwrap();
    assert_eq!(line.refusal, Some(RefusalCode::RInsufficientData));
}

#[test]
fn verifier_accepts_a_valid_v1_payload_without_source_references() {
    use ed25519_dalek::Signer;
    let state = opened();
    let key = ed25519_dalek::SigningKey::from_bytes(&[12; 32]);
    let mut bundle = record_core::create_bundle(&state, &key).unwrap();
    let mut payload: serde_json::Value = serde_json::from_str(&bundle.payload).unwrap();
    payload["format"] = "hiring-record-payload/v1".into();
    payload.as_object_mut().unwrap().remove("source_references");
    bundle.payload = serde_json::to_string(&payload).unwrap();
    bundle.signature = hex::encode(key.sign(bundle.payload.as_bytes()).to_bytes());
    let verification = record_core::verify_bundle(&bundle, Some(&bundle.public_key)).unwrap();
    assert!(
        verification.signature_valid && verification.chain_valid && verification.trusted_signer
    );
}

#[test]
fn new_role_preserves_predecision_evidence_after_reassessment() {
    let mut state = record_core::State::empty();
    let criterion = record_core::Criterion {
        id: "build".into(),
        text: "Built systems".into(),
        added_at: "2026-01-01".into(),
        added_by: "alex".into(),
    };
    apply(
        &mut state,
        Command::OpenRole {
            role_id: "custom".into(),
            standard: record_core::StandardVersion {
                id: "v1".into(),
                effective_at: "2026-01-01".into(),
                criteria: vec![criterion],
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
                place: "in_review".into(),
            },
        },
    )
    .unwrap();
    apply(
        &mut state,
        Command::RecordSource {
            source: record_core::SourceMoment {
                id: "source".into(),
                person: "sam".into(),
                kind: "Synthetic note".into(),
                timestamp: "00:01:00".into(),
                quote: "Built a system.".into(),
            },
        },
    )
    .unwrap();
    let assessment = record_core::Assessment {
        id: "a1".into(),
        person: "sam".into(),
        version: "v1".into(),
        criterion: "build".into(),
        met: true,
        citation: Some("source".into()),
    };
    apply(
        &mut state,
        Command::Assess {
            assessment: assessment.clone(),
        },
    )
    .unwrap();
    apply(
        &mut state,
        Command::RecordDecision {
            decision: record_core::Decision {
                person: "sam".into(),
                outcome: Outcome::Hire,
                at: "2026-02-01".into(),
                version: "v1".into(),
            },
        },
    )
    .unwrap();
    apply(
        &mut state,
        Command::Assess {
            assessment: record_core::Assessment {
                id: "a2".into(),
                met: false,
                ..assessment
            },
        },
    )
    .unwrap();
    assert_eq!(state.assessments.len(), 2);
    assert_eq!(state.latest_assessments().len(), 1);
    assert_eq!(evaluate_report(&state).lines[0].naive, "1 of 1 hires");
    let mut replay = record_core::State::empty();
    for event in state.events.clone() {
        record_core::replay_event(&mut replay, event).unwrap();
    }
    assert_eq!(replay, state);
}
