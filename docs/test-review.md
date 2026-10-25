# Reviewing tests with agents

Use test results to determine outcomes, then traces to investigate what happened.
Neither a quiet console nor an empty network log proves a test passed.

## Actual outcomes

Enable Playwright's JSON reporter and pass its output to:

```sh
pw-review results results.json --fail-on-error
```

The report preserves test identity, expected status, outcome, and retry attempts.
An expected failure differs from an unexpected pass. A test recovering on retry
is reported as flaky, not as an uninterrupted pass. Interrupted and timed-out
attempts remain visible. Runner-level errors are separate from test failures.
Trace attachment paths are metadata only: this command does not open them.

`--fail-on-error` opts into exit 1 for unexpected results, interrupted attempts,
or runner errors. It does not reproduce every Playwright configuration policy
or independently prove the CI job succeeded. Flaky results remain visible
without being promoted to unexpected failures.

## Explicit run history

```sh
pw-review history record results.json --store .review-history --run-id ci-123 --commit abc123
pw-review history show --store .review-history
```

Recording is opt-in and refuses to overwrite an existing run ID. Use a distinct
ID for each run and the actual commit identifier. Keep the store outside shared
artifacts if its test names or paths contain private information.
The example `.review-history/` location is Git-ignored. Reads have byte and
record limits; no old runs are automatically deleted to make a read fit.
Recording rejects truncated test identities so unrelated tests cannot collapse
into one history key; raise `--max-text` when an identity exceeds the default.

History separates retry recoveries from mixed observed outcomes across runs.
Different outcomes on different commits are not proof of flakiness: code,
configuration, data, or the environment may have changed. `history show` reports
historical observations; `--fail-on-error` does not turn an old failure into the
exit status of a current run. `history record` uses the supplied result's status.

## Console and network investigation

```sh
pw-review analyze trace.zip --max-console 20 --max-network 20 --max-endpoints 20
pw-review compare before.zip after.zip --slow-ms 1000
```

Console errors and uncaught browser exceptions supplement action failures.
Warnings and routine console messages are not included. Network details can
include candidate actions whose recorded page and time interval match the
request. Those are temporal associations, not proof that an action caused a
request or that the request caused a failure. Missing or incompatible timing
evidence leaves the association unknown.

Endpoint aggregates group an exact redacted URL and HTTP method. They retain
request counts and available latency statistics; unknown durations are not
treated as zero. Comparisons expose sample counts and changes in mean latency.
They do not establish comparable workloads or server-side root causes. No
automatic dynamic-route grouping or percentile estimate is invented.
Endpoint caps apply across artifact traces. Omitted-request counts and truncation
flags expose missing aggregates; clipped URL prefixes are never used as group keys.

## Visual evidence on demand

```sh
pw-review evidence list trace.zip --max-evidence 20
pw-review evidence extract trace.zip --id EVIDENCE_ID --output screenshot.jpeg
```

Use an ID returned by `evidence list`. The catalog contains references, not image
bytes. Extraction writes one selected screenshot or raw DOM snapshot to a new
file; it does not overwrite an existing file, execute HTML, interpret the image,
or retrieve remote resources. Use a JSON output filename for a DOM snapshot.
Screenshot and snapshot availability depends on what the trace actually captured.
Extraction requires an existing output directory with no symlink components;
use its canonical path when a system temporary-directory alias is a symlink.

The agent can inspect an extracted screenshot with its own image tool. Raw DOM
snapshots are Playwright's recorded representation, not a standalone rendered
page. Original snapshots and images may contain application data.

## Keeping context small

Start with compact JSON; avoid `--pretty` when feeding an agent. Request visual
evidence only when text is insufficient. Set retention limits for failure,
console, network, endpoint, and visual details; read truncation indicators before
drawing conclusions. `--max-output-bytes` rejects an oversized response instead
of emitting partial JSON. Output limits do not roll back an explicitly requested
history recording or evidence extraction that has already succeeded.

The CLI makes no model calls. Its output consumes tokens only when an agent reads
it; actual token counts depend on the output and the model's tokenizer.
