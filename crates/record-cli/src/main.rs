use record_core::{
    apply, evaluate_report, load_fixture, state_from_fixture, Command, ProposalStatus,
};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("play") => {
            let path = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(record_core::fixture_path);
            match play(&path) {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::from(1)
                }
            }
        }
        Some("verify") => {
            let Some(path) = args.next() else {
                eprintln!("usage: record verify <bundle.json> [--trust-key <hex-public-key>]");
                return ExitCode::from(2);
            };
            let remaining: Vec<_> = args.collect();
            let trusted = match remaining.as_slice() {
                [] => None,
                [flag, key] if flag == "--trust-key" => Some(key.as_str()),
                _ => {
                    eprintln!("usage: record verify <bundle.json> [--trust-key <hex-public-key>]");
                    return ExitCode::from(2);
                }
            };
            match verify(&path, trusted) {
                Ok(true) => ExitCode::SUCCESS,
                Ok(false) => ExitCode::from(1),
                Err(err) => {
                    eprintln!("verification failed: {err}");
                    ExitCode::from(1)
                }
            }
        }
        Some("simulate") => {
            let seed = args.next().unwrap_or_else(|| "42".into()).parse::<u64>();
            let rounds = args.next().unwrap_or_else(|| "100".into()).parse::<usize>();
            match (seed, rounds) {
                (Ok(seed), Ok(rounds)) => match run_simulation(seed, rounds) {
                    Ok(true) => ExitCode::SUCCESS,
                    Ok(false) => ExitCode::from(1),
                    Err(error) => {
                        eprintln!("simulation failed: {error}");
                        ExitCode::from(1)
                    }
                },
                _ => {
                    eprintln!("usage: record simulate [seed] [rounds]");
                    ExitCode::from(2)
                }
            }
        }
        _ => {
            eprintln!("usage: record play [fixture] | record simulate [seed] [rounds] | record verify <bundle.json> [--trust-key <hex-public-key>]");
            ExitCode::from(2)
        }
    }
}

fn run_simulation(seed: u64, rounds: usize) -> Result<bool, String> {
    let fixture = load_fixture(&record_core::fixture_path())?;
    let result = record_core::simulate(&state_from_fixture(&fixture), seed, rounds)?;
    println!(
        "seed {}  rounds {}  checks {}  failures {}",
        result.seed,
        result.rounds,
        result.checks,
        result.failures.len()
    );
    for step in result.trace {
        println!(
            "{:02}  {:9}  stale {:2}  events {}  {}",
            step.round, step.path, step.stale, step.event_count, step.root
        );
    }
    for failure in &result.failures {
        eprintln!("{failure}");
    }
    Ok(result.failures.is_empty())
}

fn verify(path: &str, trusted: Option<&str>) -> Result<bool, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() > 10_000_000 {
        return Err("bundle exceeds 10 MB limit".into());
    }
    let bundle: record_core::Bundle = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let result = record_core::verify_bundle(&bundle, trusted)?;
    println!("role: {}  events: {}", result.role_id, result.event_count);
    println!(
        "signature: {}",
        if result.signature_valid {
            "valid"
        } else {
            "INVALID"
        }
    );
    println!(
        "event chain: {}",
        if result.chain_valid {
            "consistent"
        } else {
            "INVALID"
        }
    );
    println!(
        "signer: {}",
        if result.trusted_signer {
            "matches supplied trust key"
        } else {
            "unknown (compare public key independently)"
        }
    );
    println!("public key: {}", bundle.public_key);
    println!("Source evidence and factual accuracy are not verified by this bundle.");
    Ok(
        result.signature_valid
            && result.chain_valid
            && (trusted.is_none() || result.trusted_signer),
    )
}

fn play(path: &std::path::Path) -> Result<(), String> {
    let fixture = load_fixture(path)?;
    let mut state = state_from_fixture(&fixture);
    let in_flight: Vec<_> = state
        .people
        .iter()
        .filter(|p| p.place == "in_review")
        .map(|p| p.name.as_str())
        .collect();
    println!("role {}  bar {} current", state.role_id, state.active.id);
    println!("in flight on {}: {}", state.active.id, in_flight.join(", "));

    apply(
        &mut state,
        Command::Dismiss {
            proposal_id: "B".into(),
        },
    )
    .map_err(|e| e.to_string())?;
    let dismissed = state
        .proposals
        .iter()
        .find(|p| p.status == ProposalStatus::Dismissed)
        .map(|p| p.id.as_str())
        .unwrap_or("none");
    let current = state
        .assessments
        .iter()
        .filter(|a| !state.is_stale(a))
        .count();
    println!("proposal {dismissed} dismissed  assessments still current: {current}  rescored: 0");

    let mut again = state_from_fixture(&fixture);
    apply(
        &mut again,
        Command::Approve {
            proposal_id: "B".into(),
            new_version_id: "B2".into(),
        },
    )
    .map_err(|e| e.to_string())?;
    let stale = again
        .assessments
        .iter()
        .filter(|a| again.is_stale(a))
        .count();
    println!(
        "proposal B approved as {}  stale assessments: {stale}  priya rubric: {}",
        again.active.id, again.interviews[0].rubric_version
    );
    for line in evaluate_report(&again).lines {
        let verdict = match line.refusal {
            Some(code) => format!("predicted: ineligible  {}", code.as_str()),
            None => "predicted: eligible".to_string(),
        };
        println!("naive  {:<16} {}   {verdict}", line.text, line.naive);
    }
    Ok(())
}
