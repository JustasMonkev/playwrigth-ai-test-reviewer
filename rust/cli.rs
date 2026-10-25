use clap::{Parser, Subcommand};
use playwright_ai_test_reviewer::Limits;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "pw-review",
    version,
    about = "Bounded Playwright test-review evidence for coding agents",
    long_about = "Review Playwright JSON test results, local history, and trace evidence. List and extract captured visuals on demand. Compact versioned JSON goes to stdout; errors go to stderr as JSON. No model API or API key is needed. GitHub access uses your authenticated gh CLI and never posts comments or changes remote state.",
    after_help = "Examples:\n  pw-review analyze trace.zip\n  pw-review analyze trace.zip --prompt\n  pw-review compare before.zip after.zip --fail-on-error\n  pw-review github 123456 --repo owner/repo --artifact playwright-traces\n\nExit codes: 0 = report produced; 1 = observed action or test-result failures with --fail-on-error; 2 = invalid input, runtime error or exceeded limit. Comparison failure status uses the AFTER trace. Limits reject incomplete input; bounded evidence may be marked truncated."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,

    #[arg(long, global = true, help = "Indent JSON instead of compact output")]
    pub(crate) pretty: bool,

    #[arg(
        long,
        global = true,
        help = "Prefix JSON with short agent review instructions"
    )]
    pub(crate) prompt: bool,

    #[arg(
        long,
        global = true,
        help = "Exit 1 for action failures or unexpected/interrupted/runner-error test results"
    )]
    pub(crate) fail_on_error: bool,

    #[arg(long, global = true, default_value_t = Limits::default().max_bytes, help = "Maximum input and decompressed trace bytes; also bounds GitHub download and extraction")]
    pub(crate) max_bytes: u64,

    #[arg(long, global = true, default_value_t = Limits::default().max_events, help = "Maximum parsed trace events or result/history records and attempts")]
    pub(crate) max_events: usize,

    #[arg(long, global = true, default_value_t = Limits::default().max_failures, help = "Maximum retained failure details or result error/attachment details")]
    pub(crate) max_failures: usize,

    #[arg(long, global = true, default_value_t = Limits::default().max_text, help = "Maximum bytes per retained text field")]
    pub(crate) max_text: usize,

    #[arg(long, global = true, default_value_t = Limits::default().max_network, help = "Maximum retained failed/slow network requests across an artifact batch")]
    pub(crate) max_network: usize,

    #[arg(long, global = true, default_value_t = Limits::default().slow_ms, help = "Network request duration threshold in milliseconds")]
    pub(crate) slow_ms: u64,

    #[arg(
        long,
        global = true,
        default_value_t = 20,
        help = "Maximum retained console errors across artifact traces"
    )]
    pub(crate) max_console: usize,

    #[arg(
        long,
        global = true,
        default_value_t = 20,
        help = "Maximum distinct endpoint aggregates"
    )]
    pub(crate) max_endpoints: usize,

    #[arg(
        long,
        global = true,
        default_value_t = 100,
        help = "Maximum visual evidence catalog items"
    )]
    pub(crate) max_evidence: usize,

    #[arg(
        long,
        global = true,
        default_value_t = 65_536,
        help = "Maximum stdout bytes, including prompt/newline; fail without partial JSON if exceeded"
    )]
    pub(crate) max_output_bytes: usize,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    #[command(about = "Review actual test outcomes from a Playwright JSON reporter file")]
    Results { report: PathBuf },

    #[command(about = "Record and inspect explicit local test-run history")]
    History {
        #[command(subcommand)]
        command: HistoryCommand,
    },

    #[command(about = "List or extract visual evidence only when needed")]
    Evidence {
        #[command(subcommand)]
        command: EvidenceCommand,
    },
    #[command(about = "Extract failures from a local Playwright trace")]
    Analyze {
        #[arg(
            value_name = "TRACE",
            help = "Playwright trace ZIP or JSONL .trace file"
        )]
        trace: PathBuf,
    },
    #[command(about = "Compare failure identities between two local traces")]
    Compare {
        #[arg(
            value_name = "BEFORE",
            help = "Baseline trace ZIP or JSONL .trace file"
        )]
        before: PathBuf,
        #[arg(value_name = "AFTER", help = "Current trace ZIP or JSONL .trace file")]
        after: PathBuf,
    },
    #[command(
        about = "Download and review a named GitHub Actions artifact through gh",
        long_about = "Read a named GitHub Actions artifact with your authenticated gh CLI. Downloads are bounded and ZIP paths are validated before extraction into a private temporary directory. Reviews contained trace ZIPs (up to 100), or a directly uploaded trace directory. Temporary files are removed when the command finishes. No remote writes are performed."
    )]
    Github {
        #[arg(value_name = "RUN_ID", value_parser = clap::value_parser!(u64).range(1..), help = "GitHub Actions workflow run ID")]
        run_id: u64,
        #[arg(
            long,
            value_name = "OWNER/REPO",
            help = "Repository on the gh CLI's configured GitHub host"
        )]
        repo: String,
        #[arg(long, value_name = "NAME", help = "Exact artifact name, not a glob")]
        artifact: String,
        #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u64).range(1..=3600), help = "Combined gh subprocess deadline in seconds")]
        timeout_secs: u64,
    },
}

#[derive(Subcommand)]
pub(crate) enum HistoryCommand {
    #[command(about = "Record a JSON reporter result without replacing an existing run")]
    Record {
        report: PathBuf,
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        commit: String,
    },
    #[command(about = "Summarize observed outcomes and retry recoveries across stored runs")]
    Show {
        #[arg(long)]
        store: PathBuf,
    },
}

#[derive(Subcommand)]
pub(crate) enum EvidenceCommand {
    #[command(about = "List screenshot and DOM snapshot references without loading image bytes")]
    List { trace: PathBuf },
    #[command(about = "Extract one screenshot or raw DOM snapshot to a new local file")]
    Extract {
        trace: PathBuf,
        #[arg(long)]
        id: String,
        #[arg(long)]
        output: PathBuf,
    },
}
