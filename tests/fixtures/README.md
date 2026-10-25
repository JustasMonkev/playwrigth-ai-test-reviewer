# Captured trace fixtures

These ZIPs are replay data, not synthetic event generators or executable test source. `tests/real_traces.rs` checks observable outcomes with Cargo alone.

## Browser captures

`browser-pass.zip`, `browser-hard.zip`, and `browser-soft.zip` came from real Playwright 1.56.1 runs on macOS arm64 using cached Chromium headless shell 1248. They were captured on 2026-10-25 in a temporary workspace, outside either Desktop repository. The scenarios set page content and perform a passing button click/assertion, a failing heading assertion, and a soft heading failure followed by successful button actions. Two deliberately failing browser tests are expected. The Playwright version matches the local Grafana checkout's dependency pin; these are not Grafana application traces.

## Historical captures

The other six ZIPs derive from `tests/assets/` of [microsoft/playwright](https://github.com/microsoft/playwright/tree/91a765d1681cb3eeb0d0fb89fefec3cf0d21bd9b/tests/assets), revision `91a765d1681cb3eeb0d0fb89fefec3cf0d21bd9b`, read from the existing Desktop checkout. Its Apache-2.0 license is retained in [LICENSE-Playwright.txt](LICENSE-Playwright.txt).

## Minimization and provenance

Only `.trace` entries are retained. Within each stream, the original bytes of `context-options`, `before`, `after`, `action`, `log`, and `error` records remain unchanged and in order. Resources, source files, screenshots, snapshots, network records, and unrelated event types are omitted. ZIP metadata is rebuilt deterministically. Record counts therefore differ from the full originals; no retained event values or action IDs were rewritten.

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
