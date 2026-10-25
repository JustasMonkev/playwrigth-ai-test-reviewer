# Rust CLI validation

Validation host: macOS arm64, Rust/Cargo 1.97.0, Node 26.5.0. The Rust implementation and independent review used GPT-6 Astra at xhigh; a separate GPT-6.1 Sol xhigh worker wrote and ran integration/browser checks. These are agent-assisted engineering checks, not runtime model API benchmarks.

## Rerunnable checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
node scripts/validate-desktop.mjs
```

For the browser check, explicitly set `PW_REVIEW_PLAYWRIGHT` to an installed `@playwright/test` package directory. Optional variables: `PW_REVIEW_BROWSER` for a Chromium executable, `PW_REVIEW_CLI` for the built binary, `PW_REVIEW_PLAYWRIGHT_REPO` and `PW_REVIEW_GRAFANA_REPO` for checkout paths. The script creates its fixtures in a temporary directory and removes them; it does not install dependencies or modify either repository.

The final local gate passed 40 Rust tests (9 core, 6 CLI unit, 25 integration), formatting, warning-free Clippy, release build, and the JS checker.

## Behavior coverage

| Requirement or failure mode | Evidence |
| --- | --- |
| Compact JSON and exit codes | CLI integration tests; opt-in failure exit preserves the report |
| Corrupt/missing/unsupported input | Parser and CLI tests require explicit errors, no passing fallback |
| Before/after correlation, source, selector, logs | Trace contract tests with interleaved actions and multiple streams |
| Soft assertions survive later success | Regression test and a real Chromium trace |
| Stable comparison | Different call IDs/error wording; introduced, resolved, persistent failures |
| Bounded input and output | Compressed-input expansion, event budgets, UTF-8 text bounds, failure cap, no partial JSON |
| GitHub integration | Executable `gh` test double serves real nested ZIP bytes; success, failure, traversal, byte limit, timeout with descendant, cleanup |
| Legacy compatibility | Six actual ZIP fixtures from the Desktop Playwright checkout |

The timeout regression launches a child holding inherited pipes, verifies that the command exits promptly, and checks temporary-file cleanup. GitHub tests do not require credentials or network access. Live authentication succeeded, but the public Grafana artifacts endpoint returned an empty response; live artifact download remains unverified.

## Desktop Playwright and Grafana

The read-only Desktop Playwright checkout contained these historical trace fixtures; all six were parsed successfully:

- `tests/assets/test-trace1.zip`
- `tests/assets/test-trace2.zip`
- `tests/assets/trace-1.31.zip`
- `tests/assets/trace-1.37.zip`
- `tests/assets/trace-library-1.46.zip`
- `tests/assets/trace-remote-time-diff.zip`

The 1.37 fixture exposes an incomplete action; it remains incomplete rather than being counted as passed.

Grafana's local manifest pins `@playwright/test` 1.56.1. Using that version and cached Chromium headless shell 1248, the validation script generated actual traces:

| Browser case | Actions | Observed failures | Soft failures |
| --- | ---: | ---: | ---: |
| Passing browser action | 14 | 0 | 0 |
| Failed browser assertion | 15 | 1 | 0 |
| Soft failure followed by successful click/assertion | 19 | 1 | 1 |

Both deliberately failing tests are expected; the validation script verifies their trace evidence and exits successfully. Comparing the failed trace with itself yields one persistent failure and no introduced/resolved failures. Browser and runner copies of the same failed step are deduplicated.

**Scope limit:** this proves trace compatibility with Grafana's pinned Playwright version, not Grafana application behavior. Neither checkout had `node_modules`; no Grafana server was listening on ports 3000/3001, and no Grafana Playwright artifact was available. Neither repository's complete test suite was run. The unchanged legacy web application was not rebuilt.

## Performance sample

One warm local release run processed 100,000 sequential actions (200,000 JSON events, 15,966,718 bytes), with only the final action failing. The output was 420 bytes of valid JSON. `/usr/bin/time -l` reported 0.10 seconds elapsed and 2,621,440 bytes maximum resident set size (2.5 MiB). A subsequent run measured 0.39 seconds and 2,670,592 bytes maximum resident set size. The stripped executable was approximately 1.2 MiB.

The input generator wrote alternating `before` and `after` events, unique numeric-string call IDs, `apiName: locator.click`, selector `#save`, and monotonic start/end times; the final `after` contained `Timeout waiting for #save`. Reproduce timing with:

```sh
cargo build --locked --release
/usr/bin/time -l target/release/pw-review analyze benchmark.trace > report.json
```

This is a synthetic single-host sample, not a universal latency/memory promise. Concurrent incomplete actions, ZIP metadata, long error text, and different storage change resource use. Output bytes are measured, not estimated model tokens. Parsing is local and incurs no model API usage.

## Review constraints

Rust received manual lazy-clean review plus rustfmt and Clippy; the JS checker does not validate Rust. The validation script is also scanned with the lazy-clean checker. The project forbids authored `unsafe` code. Six direct dependencies cover error context, CLI arguments, JSON serialization, ZIP decoding, and Unix process-group cleanup; no async runtime, HTTP client, provider SDK, database, or daemon is needed.

Default budgets are 256 MiB of input/decompressed trace data, one million events, 20 retained failures, 2,048 bytes per retained text field, eight logs per action, and 64 KiB of output. Individual JSON objects have an 8 MiB hard ceiling even when their DOM payload is ignored. Limits preserve aggregate failure counts and mark truncated evidence; input-budget errors never masquerade as complete reports. Exact error text, including safely JSON-escaped ANSI sequences, is retained. Screenshot/DOM interpretation remains the job of Playwright Trace Viewer or a separate visual agent.
