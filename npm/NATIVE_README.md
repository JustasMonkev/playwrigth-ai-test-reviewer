# Playwright AI Test Reviewer — native binary

This package contains the prebuilt Rust `pw-review` executable for the operating
system and architecture listed in its package metadata. It is a native dependency
of [playwright-ai-test-reviewer](https://www.npmjs.com/package/playwright-ai-test-reviewer),
not the npm command-line launcher.

## Install and run

Install the main package; npm selects its matching native dependency automatically:

```sh
npm install --global playwright-ai-test-reviewer
pw-review --help
pw-review results results.json --fail-on-error
pw-review analyze test-results/example/trace.zip
```

Keep optional dependencies enabled. The main package requires Node.js 22+ and
npm 11+; no Rust compiler, install script, or model credentials are needed.
Installing this native package alone does not create a `pw-review` command on PATH.
Its executable is stored at `bin/pw-review` and can also be run directly without Node.

`pw-review` produces compact evidence from Playwright results and traces, including
failures, retries, browser errors, and network observations. Analysis runs locally
without model calls. This native package includes no JavaScript launcher.

[Source and usage](https://github.com/JustasMonkev/playwrigth-ai-test-reviewer)
· [Distribution details](https://github.com/JustasMonkev/playwrigth-ai-test-reviewer/blob/main/docs/npm-distribution.md)

Licensed under MIT; see `LICENSE` in this package.
