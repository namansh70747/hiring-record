//! Reproducible state-machine simulations. No network, clock, or randomness from the host.
use crate::{apply, evaluate_report, evolve, hash_event, Command, State};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
pub struct SimulationResult {
    pub seed: u64,
    pub rounds: usize,
    pub checks: usize,
    pub failures: Vec<String>,
    pub trace: Vec<SimulationStep>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
pub struct SimulationStep {
    pub round: usize,
    pub path: String,
    pub stale: usize,
    pub acknowledged: bool,
    pub event_count: usize,
    pub root: String,
    pub refusals: Vec<String>,
    pub fault: String,
}

pub fn simulate(initial: &State, seed: u64, rounds: usize) -> Result<SimulationResult, String> {
    if !initial.events.is_empty() {
        return Err("Simulation requires an initial state without events".into());
    }
    if !(1..=10_000).contains(&rounds) {
        return Err("Rounds must be between 1 and 10000".into());
    }
    let mut random = if seed == 0 { 0x9e3779b97f4a7c15 } else { seed };
    let mut result = SimulationResult {
        seed,
        rounds,
        checks: 0,
        failures: vec![],
        trace: vec![],
    };
    for round in 1..=rounds {
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        let approved = random & 1 == 1;
        let ack = random & 2 == 2;
        let fault = match (random >> 2) % 4 {
            0 => "out-of-order acknowledgement",
            1 => "duplicate decision delivery",
            2 => "restart before acknowledgement",
            _ => "delayed acknowledgement",
        };
        let mut state = initial.clone();
        if fault == "out-of-order acknowledgement" && state.receipts.is_empty() {
            let before = state.clone();
            let rejected = apply(
                &mut state,
                Command::Acknowledge {
                    receipt_id: "nw-B2".into(),
                },
            )
            .is_err();
            check(
                &mut result,
                rejected && state == before,
                round,
                "out-of-order acknowledgement rejected",
            );
        }
        let command = if approved {
            Command::Approve {
                proposal_id: "B".into(),
                new_version_id: "B2".into(),
            }
        } else {
            Command::Dismiss {
                proposal_id: "B".into(),
            }
        };
        apply(&mut state, command.clone()).map_err(|e| e.to_string())?;
        if fault == "duplicate decision delivery" {
            let before = state.clone();
            let rejected = apply(&mut state, command).is_err();
            check(
                &mut result,
                rejected && state == before,
                round,
                "duplicate decision rejected",
            );
        }
        if fault == "restart before acknowledgement" {
            let mut recovered = initial.clone();
            for event in &state.events {
                crate::replay_event(&mut recovered, event.clone()).map_err(|e| e.to_string())?;
            }
            check(&mut result, recovered == state, round, "restart recovery");
            state = recovered;
        }
        let stale = state
            .assessments
            .iter()
            .filter(|a| state.is_stale(a))
            .count();
        let expected_stale = if approved {
            initial.assessments.len()
        } else {
            0
        };
        check(
            &mut result,
            stale == expected_stale,
            round,
            "assessment currency",
        );
        check(
            &mut result,
            state.assessments == initial.assessments,
            round,
            "original assessments unchanged",
        );
        check(
            &mut result,
            state.interviews == initial.interviews,
            round,
            "interview binding unchanged",
        );
        if approved && ack {
            apply(
                &mut state,
                Command::Acknowledge {
                    receipt_id: "nw-B2".into(),
                },
            )
            .map_err(|e| e.to_string())?;
            let count = state.events.len();
            let duplicate = apply(
                &mut state,
                Command::Acknowledge {
                    receipt_id: "nw-B2".into(),
                },
            )
            .is_err();
            check(
                &mut result,
                duplicate && state.events.len() == count,
                round,
                "duplicate acknowledgement rejected",
            );
        }
        let mut replayed = initial.clone();
        let mut previous = "genesis".to_owned();
        let mut chain = true;
        for (index, event) in state.events.iter().enumerate() {
            chain &= event.seq == index as u64 + 1
                && event.prev_hash == previous
                && hash_event(event) == event.hash;
            previous.clone_from(&event.hash);
            evolve(&mut replayed, event.clone());
        }
        check(&mut result, chain, round, "event hash chain");
        check(&mut result, replayed == state, round, "event replay");
        if result.trace.len() < 12 {
            result.trace.push(SimulationStep {
                round,
                path: if approved { "approved" } else { "dismissed" }.into(),
                stale,
                acknowledged: approved && ack,
                event_count: state.events.len(),
                root: previous,
                refusals: evaluate_report(&state)
                    .lines
                    .iter()
                    .filter_map(|line| line.refusal.map(|code| code.as_str().to_owned()))
                    .collect(),
                fault: fault.into(),
            });
        }
    }
    Ok(result)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduledStep {
    pub index: usize,
    pub action: String,
    pub fault: String,
    pub ok: bool,
    pub event_count: usize,
    pub root: String,
}

/// Stateful schedule used by the lab. The simple `simulate` regression mode stays capped.
pub fn run_schedule(initial: &State, actions: &[String]) -> Result<Vec<ScheduledStep>, String> {
    if !initial.events.is_empty() {
        return Err("schedule requires an opening state".into());
    }
    let mut state = initial.clone();
    let mut steps = Vec::new();
    for (index, action) in actions.iter().enumerate() {
        let fault = if action.ends_with(":duplicate") {
            "duplicate"
        } else {
            ""
        };
        let name = action.trim_end_matches(":duplicate");
        let command = match name {
            "approve" => Command::Approve {
                proposal_id: "B".into(),
                new_version_id: "B2".into(),
            },
            "dismiss" => Command::Dismiss {
                proposal_id: "B".into(),
            },
            "ack" => Command::Acknowledge {
                receipt_id: "nw-B2".into(),
            },
            other => return Err(format!("unsupported schedule action {other}")),
        };
        let ok = apply(&mut state, command.clone()).is_ok();
        if fault == "duplicate" {
            let _ = apply(&mut state, command);
        }
        steps.push(ScheduledStep {
            index,
            action: action.clone(),
            fault: fault.into(),
            ok,
            event_count: state.events.len(),
            root: state
                .events
                .last()
                .map(|e| e.hash.clone())
                .unwrap_or_else(|| "genesis".into()),
        });
    }
    Ok(steps)
}

/// Drop a middle step while the failure remains. This is a minimized reproducer, not a proof of minimality.
pub fn minimize_schedule(
    initial: &State,
    actions: &[String],
    fails: impl Fn(&[ScheduledStep]) -> bool,
) -> Result<Vec<String>, String> {
    let mut current = actions.to_vec();
    let mut index = 0;
    while index < current.len() {
        let mut candidate = current.clone();
        candidate.remove(index);
        if candidate.is_empty() {
            break;
        }
        let steps = run_schedule(initial, &candidate)?;
        if fails(&steps) {
            current = candidate;
        } else {
            index += 1;
        }
    }
    Ok(current)
}

fn check(result: &mut SimulationResult, ok: bool, round: usize, invariant: &str) {
    result.checks += 1;
    if !ok {
        result.failures.push(format!("round {round}: {invariant}"));
    }
}
