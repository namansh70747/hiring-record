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
        _ => {
            eprintln!("usage: record play [fixtures/role.json]");
            ExitCode::from(2)
        }
    }
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
    let current = state.assessments.iter().filter(|a| !state.is_stale(a)).count();
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
    let stale = again.assessments.iter().filter(|a| again.is_stale(a)).count();
    println!(
        "proposal B approved as {}  stale assessments: {stale}  priya rubric: {}",
        again.active.id, again.interviews[0].rubric_version
    );
    for line in evaluate_report(&again).lines {
        let verdict = match line.refusal {
            Some(code) => format!("predicted: ineligible  {}", code.as_str()),
            None => "predicted: eligible".to_string(),
        };
        println!(
            "naive  {:<16} {}   {verdict}",
            line.text, line.naive
        );
    }
    Ok(())
}
