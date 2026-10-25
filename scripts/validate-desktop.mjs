import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const playwrightPackage = process.env.PW_REVIEW_PLAYWRIGHT;
assert(playwrightPackage, 'Set PW_REVIEW_PLAYWRIGHT to an installed @playwright/test package directory.');
const packageDir = path.resolve(playwrightPackage);
const playwrightVersion = JSON.parse(readFileSync(path.join(packageDir, 'package.json'), 'utf8')).version;
const cli = process.env.PW_REVIEW_CLI ?? fileURLToPath(new URL('../target/debug/pw-review', import.meta.url));
const repository = process.env.PW_REVIEW_PLAYWRIGHT_REPO ?? path.join(homedir(), 'Desktop/playwright');
const grafanaRepository = process.env.PW_REVIEW_GRAFANA_REPO ?? path.join(homedir(), 'Desktop/grafana');
const grafanaManifest = JSON.parse(readFileSync(path.join(grafanaRepository, 'package.json'), 'utf8'));
assert.equal(playwrightVersion, grafanaManifest.devDependencies['@playwright/test'], 'Use the Playwright version pinned by the Grafana checkout.');
const directory = mkdtempSync(path.join(tmpdir(), 'pw-review-browser-validation-'));

function invoke(args) {
  const result = spawnSync(cli, args, { encoding: 'utf8', timeout: 30_000, maxBuffer: 2 * 1024 * 1024 });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stderr, '');
  assert(Buffer.byteLength(result.stdout) <= 65_536);
  return JSON.parse(result.stdout);
}

try {
  writeFileSync(path.join(directory, 'playwright.config.cjs'), `
const { defineConfig } = require(${JSON.stringify(packageDir)});
module.exports = defineConfig({
  testDir: __dirname,
  testMatch: 'cases.spec.cjs',
  outputDir: ${JSON.stringify(path.join(directory, 'results'))},
  workers: 1,
  reporter: 'list',
  use: { trace: 'on', launchOptions: ${JSON.stringify(process.env.PW_REVIEW_BROWSER ? { executablePath: process.env.PW_REVIEW_BROWSER } : {})} },
});
`);
  writeFileSync(path.join(directory, 'cases.spec.cjs'), `
const { test, expect } = require(${JSON.stringify(packageDir)});
test('passing browser action', async ({ page }) => {
  await page.setContent('<button>Save</button>');
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('button')).toBeVisible();
});
test('failed browser assertion', async ({ page }) => {
  await page.setContent('<h1>Pending</h1>');
  await expect(page.getByRole('heading')).toHaveText('Saved', { timeout: 100 });
});
test('soft failure followed by success', async ({ page }) => {
  await page.setContent('<h1>Pending</h1><button>Continue</button>');
  await expect.soft(page.getByRole('heading')).toHaveText('Saved', { timeout: 100 });
  await page.getByRole('button').click();
  await expect(page.getByRole('button')).toBeVisible();
});
`);
  const browserTests = spawnSync(process.execPath, [
    path.join(packageDir, 'cli.js'), 'test', '--config', path.join(directory, 'playwright.config.cjs'),
  ], { encoding: 'utf8', timeout: 60_000, maxBuffer: 2 * 1024 * 1024 });
  assert.ifError(browserTests.error);
  assert.equal(browserTests.status, 1, browserTests.stderr);
  assert.match(browserTests.stdout, /1 passed/);
  assert.match(browserTests.stdout, /2 failed/);

  const cases = [];
  let failedTrace;
  for (const name of readdirSync(path.join(directory, 'results')).sort()) {
    const trace = path.join(directory, 'results', name, 'trace.zip');
    if (!existsSync(trace)) continue;
    const { report } = invoke(['analyze', trace]);
    const expectedFailures = name.includes('passing-browser-action') ? 0 : 1;
    assert.equal(report.summary.failed, expectedFailures, name);
    assert.equal(report.failures.length, expectedFailures, name);
    assert.equal(report.summary.soft_failures, name.includes('soft-failure') ? 1 : 0, name);
    if (expectedFailures) {
      assert(report.failures.some(failure => failure.selector?.includes('heading')), name);
    }
    if (name.includes('failed-browser-assertion')) failedTrace = trace;
    cases.push({ name, summary: report.summary });
  }
  assert.equal(cases.length, 3);
  assert(failedTrace);
  const { comparison } = invoke(['compare', failedTrace, failedTrace]);
  assert.equal(comparison.persistent.length, 1);
  assert.deepEqual(comparison.introduced, []);
  assert.deepEqual(comparison.resolved, []);

  const fixtures = [];
  for (const name of ['test-trace1.zip', 'test-trace2.zip', 'trace-1.31.zip', 'trace-1.37.zip', 'trace-library-1.46.zip', 'trace-remote-time-diff.zip']) {
    const trace = path.join(repository, 'tests/assets', name);
    const { report } = invoke(['analyze', trace]);
    assert.equal(report.schema_version, 1);
    assert(report.summary.events > 0, name);
    fixtures.push({ name, summary: report.summary });
  }
  console.log(JSON.stringify({
    playwright_version: playwrightVersion,
    browser: process.env.PW_REVIEW_BROWSER ?? 'Playwright default',
    generated_cases: cases,
    historical_fixture_repository: repository,
    historical_fixtures: fixtures,
    comparison_verified: true,
    grafana_repository: grafanaRepository,
    grafana_pinned_playwright: grafanaManifest.devDependencies['@playwright/test'],
    grafana_scope: 'Pinned Playwright compatibility only; the Grafana application suite is not run.',
  }, null, 2));
} finally {
  rmSync(directory, { recursive: true, force: true });
}
