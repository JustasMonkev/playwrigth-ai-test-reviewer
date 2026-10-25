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
