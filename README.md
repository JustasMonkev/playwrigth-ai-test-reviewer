# Playwright AI Test Reviewer

`pw-review` turns Playwright traces into small, deterministic evidence reports for coding agents. Written in Rust; local analysis needs no browser, Node runtime, API key, or model call. The existing [web viewer](docs/legacy-web.md) remains available separately.

## Install

Install the toolchain in `rust-toolchain.toml`, then:

```sh
cargo install --locked --path .
pw-review --help
```

## Agent workflow

```sh
pw-review analyze test-results/example/trace.zip
pw-review compare baseline.zip failed.zip
pw-review analyze trace.zip --prompt --max-output-bytes 12000
pw-review analyze trace.zip --fail-on-error
```

JSON is compact by default; use `--pretty` for a human-readable report. `--prompt` adds a short instruction for your agent. Give that output to GPT-6.1 Sol, GPT-6 Astra, or another agent alongside the relevant test source. Model selection and reasoning effort belong to the agent host, not this CLI. No model credentials or provider SDKs are embedded.

The report keeps failed actions, selectors, error messages, source locations when present, and bounded call logs. It skips screenshot bytes, DOM snapshots, network bodies, and successful-action detail. This reduces context size without claiming a tokenizer-independent token count or an AI diagnosis. Use Playwright Trace Viewer when visual evidence is needed.

## GitHub Actions

Authenticate with `gh auth login`, then provide an explicit repository, run ID, and artifact name:

```sh
pw-review github 123456789 --repo owner/repo --artifact playwright-report
```

The command retrieves the named artifact through the installed GitHub CLI, analyzes its trace ZIPs, and prints a report. It does not post comments, create reviews, or modify the repository. Downloads and processing are bounded; temporary files are cleaned up. Artifacts must be available to the authenticated account and not expired.

For CI, capture the exit status separately from the JSON:

```sh
pw-review analyze trace.zip --fail-on-error > review.json
```

| Exit | Meaning |
| --- | --- |
| `0` | Report produced; failures do not change the exit code unless requested |
| `1` | Trace failures observed with `--fail-on-error` |
| `2` | Invalid input, exceeded limit, or runtime error |

Operational errors are JSON on stderr. `--help` and `--version` are plain text.

## Input and output contracts

Supported inputs include Playwright trace ZIPs and raw trace event streams. Events may be newline-delimited, concatenated JSON objects, or a JSON array. A ZIP can contain multiple `.trace` streams. Invalid JSON, unsupported content, corrupt archives, and resource-limit violations return errors rather than a fabricated successful test result.

Reports use `schema_version: 1`. Summary counts describe **trace actions**, not test-run pass/fail totals; an incomplete action is not a passed action. A trace containing no observed failure is not proof the entire test suite passed. Comparison matches failures by action, selector, and source location, not ephemeral call IDs or archive size. Identical repeated failures are matched by occurrence; changed/missing source metadata can prevent a match.

`truncated: true` means retained evidence was reduced. A comparison with truncated evidence is not a complete regression inventory. Error messages and selectors can contain application data; inspect reports before sharing them.

| Option | Controls |
| --- | --- |
| `--max-bytes` | Input/decompressed trace byte budget |
| `--max-events` | Trace event budget |
| `--max-failures` | Retained failure details |
| `--max-text` | Text detail length |
| `--max-output-bytes` | Serialized output budget; oversized output fails rather than emitting broken JSON |

See `pw-review --help` for defaults and GitHub-specific bounds. Limits are operational safeguards, not a claim of constant memory: ZIP metadata and active action state still consume memory within the input limits. The crate forbids unsafe Rust in project code; dependencies are separately maintained.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

The GitHub workflow runs these on Linux and macOS. GitHub artifact retrieval requires a Unix host for bounded subprocess-tree cleanup. Local execution evidence and limitations are recorded in [validation](docs/validation.md).

```text
rust/
├── main.rs       CLI and output contract
├── github.rs     bounded authenticated artifact retrieval
├── trace.rs      trace parsing and action correlation
├── report.rs     compact report and semantic comparison
└── lib.rs        public analysis entry point
```

The legacy React app remains in `src/` and `utils/`; its npm dependencies are not needed to build or run the CLI. Browser-based screenshot comparison and Gemini calls have not been ported into the headless CLI.
