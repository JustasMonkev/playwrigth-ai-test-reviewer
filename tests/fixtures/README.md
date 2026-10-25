# Captured trace fixtures

These ZIPs are replay data, not synthetic event generators or executable test source. `tests/real_traces.rs` checks observable outcomes with Cargo alone.

## Browser captures

`browser-pass.zip`, `browser-hard.zip`, and `browser-soft.zip` came from real Playwright 1.56.1 runs on macOS arm64 using cached Chromium headless shell 1248. They were captured on 2026-10-25 in a temporary workspace, outside either Desktop repository. The scenarios set page content and perform a passing button click/assertion, a failing heading assertion, and a soft heading failure followed by successful button actions. Two deliberately failing browser tests are expected. The Playwright version matches the local Grafana checkout's dependency pin; these are not Grafana application traces.

## Historical captures

The other six ZIPs derive from `tests/assets/` of [microsoft/playwright](https://github.com/microsoft/playwright/tree/91a765d1681cb3eeb0d0fb89fefec3cf0d21bd9b/tests/assets), revision `91a765d1681cb3eeb0d0fb89fefec3cf0d21bd9b`, read from the existing Desktop checkout. Its Apache-2.0 license is retained in [LICENSE-Playwright.txt](LICENSE-Playwright.txt).

## Minimization and provenance

For the nine action-only fixtures above, only `.trace` entries are retained. Within each stream, the original bytes of `context-options`, `before`, `after`, `action`, `log`, and `error` records remain unchanged and in order. Resources, source files, screenshots, snapshots, network records, and unrelated event types are omitted. ZIP metadata is rebuilt deterministically. Record counts therefore differ from the full originals; no retained event values or action IDs were rewritten.

Original archive SHA-256 values:

| Captured fixture | Original SHA-256 |
| --- | --- |
| `browser-pass.zip` | `1e5d539fcb77ea33517bf3fc0f969ddf0117ee0d2d6168c21e115c3db8c5dfe5` |
| `browser-hard.zip` | `41e81bc413398a5dbe733a35ff49de8805d5d4a5b9292ed13fa74287dcadfde4` |
| `browser-soft.zip` | `c97154c0db0e8cd39e4c817f8fb720931f4e0bb1e2fa7566d9e3a514311efd43` |
| `test-trace1.zip` | `69032521791187ba72af42bbef0177de961b7c25dfe4f98dfe860754e69ca48f` |
| `test-trace2.zip` | `d4322d84d437b0bd8e097045c08140b46c6c603b5d0e27cbbf4c8c4fa544a981` |
| `trace-1.31.zip` | `47da957658a9d32846e76a3c42452e8b4890d2de19790ec4e7239a866e20f8c4` |
| `trace-1.37.zip` | `e8e846902f58089d8b96e4349ae63fa4bbd9cdbb721ac0f29e1de6253ea645df` |
| `trace-library-1.46.zip` | `606b18674e2ef8bad012d98dd34ac371a6c9c36111af0686f813ffc2c671b48b` |
| `trace-remote-time-diff.zip` | `cf72b06f4c1c289214a826cbf8d012cadc7f73090a604d2c36c61a0db5d272ac` |

## Network capture

`browser-network.zip` is a real loopback-only Playwright 1.56.1 / Chromium 156.0.8078.4 capture made on 2026-10-25 with tracing snapshots enabled, screenshots and sources disabled. A temporary HTTP server served a normal page, fast 200, delayed 200, HTTP 500, connection termination, and aborted request. This is not a Grafana trace. `tests/network_contract.rs` replays it with Cargo alone.

The ZIP retains the original context-options and goto before/after lines (source trace lines 1, 5, 8), plus all six resource-snapshot records. Network records retain request method/URL, response status/failure text, total time, timing phases, page reference, and timestamps. Retained JSON value tokens were verified byte-identical to the original; surrounding object syntax was reconstructed. Headers, cookies, content references, resource bodies, snapshots, source, stacks, and console records were removed. The temporary generator is not shipped.

- Original ZIP SHA-256: `58a62f2bc0aeaaf5249f65761f454e401de18bb4dfc37d1b425061c06b76c91f`
- Minimized ZIP SHA-256: `9370ad603004e1b42c3a71573e20f2e76b65b8eeb43125575795d455165e39f8`
- Retained `.trace` bytes: 773; `.network` bytes: 2,115.
- Slow 200: 1,202.295 ms total, 1,201.761 ms wait. HTTP 500: 3.182 ms. Failed/aborted requests retain status/time `-1` and `net::ERR_EMPTY_RESPONSE` / `net::ERR_ABORTED` respectively.

## Test-result and visual captures

All captures below were made on 2026-10-25 using installed Playwright 1.56.1. They are real recorded outputs, not invented test outcomes. The temporary JavaScript producers are not shipped as repository tooling.

`results-playwright-1.56.json` comes from seven actual tests and eleven attempts: two expected, three unexpected, one flaky retry recovery, and one skipped. The cases include an expected failure, an unexpected pass, an assertion failure, and a timeout. The Playwright process exited 1. After correcting the three unexpected cases, `results-playwright-1.56-pass.json` records seven tests/eight attempts: five expected, zero unexpected, one flaky, one skipped; the process exited 0. Minimized fixtures retain outcomes, retry numbers, durations and diagnostic messages; unused reporter fields are omitted and machine-specific paths/source import paths are redacted.

| Capture | Original JSON SHA-256 | Fixture SHA-256 |
| --- | --- | --- |
| Failing run | `f89c661800c703ebb2606532a1ecc1d8b6e1fad6bf36addb5043eb5501dc0b19` | `6d936d2a6893e54528a60f2961d081db1792b633b88f57768bc92532719879be` |
| Passing run | `a1e1acfcb2aa4bf171ed13df266e103961b9f4a9cd1cb0c784c83f4d359db8fc` | `cfd4f919b596368b8f0e86a7a6fe8bd5f3466d848ccadf2fac19bcf8f0069d3f` |

`browser-visual.zip` retains two byte-original records from a fresh Chromium capture: the final screencast frame and the DOM snapshot after a button click, plus its original 8,776-byte JPEG. The 619-byte trace preserves original event values; ZIP metadata was rebuilt. Original archive SHA-256: `4c03b1d1b7cdf563883f6c3ed1e81f33834cbed89a92c840a88471587cfc36e3`; minimized archive SHA-256: `3088593e3a5ab869e180f479e76f25253df4c160d1b803e8e1ae1b6c64bc97f1`.

`browser-console.trace` retains the two byte-original console-error/page-error records from that same capture, including the producer's nested serialized exception shape. Fixture SHA-256: `de61ebf989cdf6dd3d47725507919bcdefac6fcdf6bb8baea98f30ab49766127`.

`browser-review.zip` is a separate fresh loopback capture with a button-triggered 1,151.709 ms request, HTTP 503 response, console error and uncaught exception. It retains 28 original action/context/log/diagnostic lines and three resource snapshots (method/URL, status, timings, page reference and monotonic start). Network object syntax is rebuilt without headers/cookies/bodies; retained values are unchanged. Its real page IDs and clocks exercise action/request overlap through the CLI. Original archive SHA-256: `b8d40bd47749f911eafb23f11d0fa72342c0c33baf5e35ace7688e7031085f9a`; minimized archive SHA-256: `786a306bb7b6be6632700874eed42bb2e0a2d54389ec7c200f297d5f789c18ce`.
