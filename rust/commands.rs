use crate::cli::{Commands, EvidenceCommand, HistoryCommand};
use crate::github;
use anyhow::{Context, Result};
use playwright_ai_test_reviewer::{
    Limits, analyze, analyze_all, analyze_results, compare, extract_evidence, list_evidence,
    read_history, record_history,
};
use serde_json::{Value, json};

pub(crate) fn run(command: &Commands, limits: &Limits) -> Result<(Value, bool)> {
    match command {
        Commands::Analyze { trace } => {
            let report = analyze(trace, limits)?;
            let failed = report.has_failures();
            Ok((
                json!({"schema_version":1,"command":"analyze","report":report}),
                failed,
            ))
        }
        Commands::Compare { before, after } => {
            let before = analyze(before, limits).context("cannot analyze BEFORE trace")?;
            let after = analyze(after, limits).context("cannot analyze AFTER trace")?;
            let failed = after.has_failures();
            Ok((
                json!({"schema_version":1,"command":"compare","comparison":compare(&before, &after)}),
                failed,
            ))
        }
        Commands::Github {
            run_id,
            repo,
            artifact,
            timeout_secs,
        } => {
            let downloaded =
                github::download(repo, *run_id, artifact, limits.max_bytes, *timeout_secs)?;
            let mut reports = Vec::new();
            let mut failed = false;
            let paths = downloaded
                .traces
                .iter()
                .map(|(_, path)| path.clone())
                .collect::<Vec<_>>();
            let analyzed = analyze_all(&paths, limits).context("cannot analyze artifact traces")?;
            for ((name, _), mut report) in downloaded.traces.iter().zip(analyzed) {
                report.source = name.clone();
                failed |= report.has_failures();
                reports.push(json!({"trace":name,"report":report}));
            }
            Ok((
                json!({"schema_version":1,"command":"github","repo":repo,"run_id":run_id,"artifact":artifact,"reports":reports}),
                failed,
            ))
        }
        Commands::Results { report } => {
            let report = analyze_results(report, limits)?;
            let failed = report.has_failures();
            Ok((
                json!({"schema_version":1,"command":"results","report":report}),
                failed,
            ))
        }
        Commands::History { command } => match command {
            HistoryCommand::Record {
                report,
                store,
                run_id,
                commit,
            } => {
                let report = analyze_results(report, limits)?;
                let failed = report.has_failures();
                let run = record_history(store, run_id, commit, &report, limits)?;
                Ok((
                    json!({"schema_version":1,"command":"history record","run":run}),
                    failed,
                ))
            }
            HistoryCommand::Show { store } => {
                let history = read_history(store, limits)?;
                Ok((
                    json!({"schema_version":1,"command":"history show","history":history}),
                    false,
                ))
            }
        },
        Commands::Evidence { command } => {
            let value = match command {
                EvidenceCommand::List { trace } => {
                    json!({"schema_version":1,"command":"evidence list","catalog":list_evidence(trace, limits)?})
                }
                EvidenceCommand::Extract { trace, id, output } => {
                    json!({"schema_version":1,"command":"evidence extract","evidence":extract_evidence(trace, id, output, limits)?})
                }
            };
            Ok((value, false))
        }
    }
}
