# Playwright AI Test Reviewer

`pw-review` turns Playwright JSON results and trace ZIPs into bounded evidence for coding agents. Analysis runs in Rust. This package only selects and launches an exact-version native optional package; it does not download binaries, compile code, or call a model.

Requires Node.js 22+ and npm 11+. Each release includes only the native targets listed in its `pwReviewPlatforms` package metadata. macOS and glibc Linux on arm64/x64 are the available packaging targets; Windows and musl Linux are not supported.

Once a matching release is published:

```sh
npm install --save-dev --save-exact playwright-ai-test-reviewer
npx --no-install pw-review --help
npx --no-install pw-review results results.json --fail-on-error
npx --no-install pw-review analyze test-results/example/trace.zip
```

Keep optional dependencies enabled. Installation works with `--ignore-scripts`; there are no lifecycle scripts or runtime npm dependencies other than the native package. A missing, incompatible, or mismatched-version native package produces a JSON diagnostic on stderr and exit status 2.

The native CLI owns all arguments, input parsing, output, and exit status: 0 for a produced report, 1 for failures when `--fail-on-error` is requested, and 2 for invalid input or operational errors. Native stdout/stderr and stdin are inherited unchanged. Signals are forwarded to the native process.

For source builds, unpublished local tarball installation, supported target requirements, and manual release instructions, see [npm distribution](https://github.com/JustasMonkev/playwrigth-ai-test-reviewer/blob/main/docs/npm-distribution.md). No npm release is implied by the presence of this source tree.
