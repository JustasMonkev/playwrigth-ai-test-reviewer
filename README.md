# Playwright AI Test Reviewer

`pw-review` turns Playwright traces into small, deterministic evidence reports for coding agents. Written in Rust; local analysis needs no browser, Node runtime, API key, or model call.

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

## Network evidence

```sh
pw-review analyze trace.zip --slow-ms 1000 --max-network 20
pw-review compare before.zip after.zip --prompt
```

Network logs expose observed HTTP errors, transport failures, and slow requests. The default threshold is 1,000 ms (inclusive). Reports count all requests but retain only failed/slow request details: method, URL, status, elapsed milliseconds, available wait timing, and transport error code. Failures rank first, then longest duration; a later higher-priority request can replace a retained item. These are individual request samples, not endpoint percentiles or proof of server-side latency.

URL credentials, queries, and fragments are removed before text clipping. Headers, cookies, and bodies are never included; paths can still contain application data. Missing/negative durations remain unknown, not zero. HTTP 400–599 and explicit transport errors count as network failures; missing/negative status alone denotes an incomplete observation. Aborted requests are reported but can be intentional.

`network` is omitted when there are no network records or log entries. An empty network log is reported with zero requests, not treated as verified healthy traffic. Playwright can carry network logs across trace chunks, so counts describe the included observations, not traffic exclusive to one test. Comparisons include before/after network reports without claiming a complete introduced/resolved network inventory. Network findings do not change action failure counts or `--fail-on-error` exit status.

## GitHub Actions

Authenticate with `gh auth login`, then provide an explicit repository, run ID, and artifact name:

```sh
pw-review github 123456789 --repo owner/repo --artifact playwright-report
```

The command retrieves the named artifact through the installed GitHub CLI, analyzes its trace ZIPs, and prints a report. Decompressed bytes and parsed events include network logs. Failure details and network evidence each have a cumulative retention budget across every trace in the artifact; failure totals are still counted after the detail cap is reached. It does not post comments, create reviews, or modify the repository. Downloads and processing are bounded; temporary files are cleaned up. Artifacts must be available to the authenticated account and not expired.

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

Supported inputs include Playwright trace ZIPs and raw trace event streams. Events may be newline-delimited, concatenated JSON objects, or a JSON array. A ZIP can contain multiple `.trace` and `.network` streams. Raw `.network` logs and inline `resource-snapshot` records are also supported. Independent streams retain their own action and error state. Invalid JSON, unsupported content, corrupt archives, and resource-limit violations return errors rather than a fabricated successful test result.

Browser and test-runner copies of a step are merged only when step ID, recording scope, stream roles, timing, and available selector/location evidence agree. Streams in different archive directories are separate recordings. Clock metadata is normalized for matching, with a one-millisecond allowance for integer wall-clock rounding. Ambiguous or missing cross-stream timing evidence leaves actions distinct. A runner error immediately repeating its failed action is counted once; identical standalone errors are independent.

Reports use `schema_version: 1`. Summary counts describe **trace actions**, not test-run pass/fail totals; an incomplete action is not a passed action. A trace containing no observed failure is not proof the entire test suite passed. Comparison matches failures by action, selector, and source location, not ephemeral call IDs or archive size. Identical repeated failures are matched by occurrence; changed/missing source metadata can prevent a match.

`truncated: true` means retained evidence was reduced. A comparison with truncated evidence is not a complete regression inventory. Error messages and selectors can contain application data; inspect reports before sharing them.

| Option | Controls |
| --- | --- |
| `--max-bytes` | Input/decompressed trace byte budget |
| `--max-events` | Trace event budget |
| `--max-failures` | Retained failure details |
| `--max-network` | Retained failed/slow requests (default 20; shared across artifact traces) |
| `--slow-ms` | Inclusive slow-request threshold (default 1,000 ms) |
| `--max-text` | Text detail length |
| `--max-output-bytes` | Serialized output budget; oversized output fails rather than emitting broken JSON |

See `pw-review --help` for defaults and GitHub-specific bounds. Limits are operational safeguards, not a claim of constant memory: ZIP metadata, active actions, and unmatched correlation candidates still consume memory within the input limits. The crate forbids unsafe Rust in project code; dependencies are separately maintained.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
```

The GitHub workflow runs these on Linux and macOS. GitHub artifact retrieval requires a Unix host for bounded subprocess-tree cleanup. The test suite includes nine small captured trace fixtures, so compatibility checks need only Cargo. Local execution evidence and limitations are recorded in [validation](docs/validation.md).

```text
rust/
├── main.rs       CLI and output contract
├── github.rs     bounded authenticated artifact retrieval
├── trace.rs      archive/stream parsing and shared usage budgets
├── trace/
│   ├── actions.rs action correlation and compact evidence
│   ├── events.rs  supported event wire types
│   ├── frames.rs  bounded JSON framing
│   └── tests.rs   parser unit tests
├── report.rs     compact report and semantic comparison
└── lib.rs        public analysis entry point
```

The repository contains Rust source and Cargo tooling only. There is no web server, frontend dependency tree, or model-provider client. Screenshot/DOM interpretation remains outside the CLI.

Library callers can use `analyze(&Path, &Limits) -> Result<Report>` for one input or `analyze_all(&[PathBuf], &Limits) -> Result<Vec<Report>>` for a batch sharing byte, event, and retained-evidence budgets. Reports preserve input order. A batch fails on the first invalid or over-budget input without returning partial reports; separate inputs are not deduplicated against one another.
