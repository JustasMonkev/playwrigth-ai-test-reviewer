'use strict';

const assert = require('node:assert/strict');
const { spawn, spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { once } = require('node:events');
const { test } = require('node:test');

const root = path.resolve(__dirname, '..');
const targetDir = path.resolve(root, process.env.CARGO_TARGET_DIR || 'target');
const stager = path.join(targetDir, 'debug/examples/npm-stage');
const native = path.join(targetDir, 'debug/pw-review');
const osName = process.platform;
const rustCpu = { arm64: 'aarch64', x64: 'x86_64' }[process.arch];
const hostTarget = `${rustCpu}-${osName === 'darwin' ? 'apple-darwin' : 'unknown-linux-gnu'}`;

function run(command, args, options = {}) {
  const output = spawnSync(command, args, { encoding: 'utf8', timeout: 60000, ...options });
  assert.ifError(output.error);
  return output;
}

function success(output) {
  assert.equal(output.status, 0, output.stderr || output.stdout);
  return output.stdout;
}

function diagnostic(output, kind = 'distribution') {
  assert.equal(output.status, 2, output.stderr);
  assert.equal(output.stdout, '');
  const error = JSON.parse(output.stderr);
  assert.equal(error.schema_version, 1);
  assert.equal(error.error.kind, kind);
  return error.error.message;
}

function writeJson(file, value) {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function waitForFile(file) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      fs.unwatchFile(file);
      reject(new Error('native subprocess did not become ready within five seconds'));
    }, 5000);
    fs.watchFile(file, { interval: 20 }, stats => {
      if (stats.size > 0) {
        fs.unwatchFile(file);
        clearTimeout(timer);
        resolve();
      }
    });
  });
}

test('real npm tarballs preserve the native CLI contract', { timeout: 120000 }, async t => {
  assert.ok(rustCpu && ['darwin', 'linux'].includes(osName), 'suite needs a supported host');
  if (osName === 'linux') assert.ok(process.report.getReport().header.glibcVersionRuntime);
  assert.ok(fs.existsSync(stager), 'first: cargo build --locked --bin pw-review --example npm-stage');
  assert.ok(fs.existsSync(native));
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'pw review npm '));
  t.after(() => fs.rmSync(work, { recursive: true, force: true }));
  const stage = path.join(work, 'staged packages');
  success(run(stager, [stage, `${hostTarget}=${native}`]));
  const mainName = 'playwright-ai-test-reviewer';
  const mainDir = path.join(stage, mainName);
  const main = readJson(path.join(mainDir, 'package.json'));
  const platform = main.pwReviewPlatforms[0];
  const platformDir = path.join(stage, platform.package);
  const nativeManifest = readJson(path.join(platformDir, 'package.json'));
  const tarballs = path.join(work, 'tarballs');
  const consumer = path.join(work, 'consumer with spaces');
  fs.mkdirSync(tarballs);
  fs.mkdirSync(consumer);
  const env = {
    ...process.env,
    npm_config_cache: path.join(work, 'empty npm cache'),
    npm_config_offline: 'true',
    npm_config_update_notifier: 'false',
  };
  const npm = (args, cwd = work) => run('npm', args, { cwd, env });
  const pack = directory => {
    fs.writeFileSync(path.join(directory, 'must-not-ship.txt'), 'private development material');
    const packed = JSON.parse(success(npm(['pack', directory, '--json', '--ignore-scripts', '--pack-destination', tarballs])));
    assert.equal(Object.values(packed).length, 1);
    return Object.values(packed)[0];
  };
  const nativePack = pack(platformDir);
  const mainPack = pack(mainDir);

  await t.test('generated manifests and packed file lists are exact and minimal', () => {
    const expectedVersion = success(run(native, ['--version'])).trim().split(' ')[1];
    assert.equal(main.version, expectedVersion);
    assert.equal(nativeManifest.version, expectedVersion);
    assert.deepEqual(main.optionalDependencies, { [platform.package]: expectedVersion });
    assert.deepEqual(main.bin, { 'pw-review': 'pw-review.cjs' });
    assert.deepEqual(nativeManifest.os, [osName]);
    assert.deepEqual(nativeManifest.cpu, [process.arch]);
    assert.equal(platform.libc, osName === 'linux' ? 'glibc' : undefined);
    assert.deepEqual(nativeManifest.libc, osName === 'linux' ? ['glibc'] : undefined);
    for (const manifest of [main, nativeManifest]) {
      assert.equal(manifest.license, 'MIT');
      assert.equal(manifest.scripts, undefined);
      assert.equal(manifest.dependencies, undefined);
    }
    assert.deepEqual(mainPack.files.map(file => file.path).sort(), ['LICENSE', 'README.md', 'package.json', 'pw-review.cjs']);
    assert.deepEqual(nativePack.files.map(file => file.path).sort(), ['LICENSE', 'README.md', 'bin/pw-review', 'package.json']);
    assert.equal(nativePack.files.find(file => file.path === 'bin/pw-review').mode & 0o111, 0o111);
  });

  writeJson(path.join(consumer, 'package.json'), { name: 'offline-consumer', private: true });
  success(npm(['install', '--offline', '--ignore-scripts', '--no-audit', '--no-fund',
    path.join(tarballs, nativePack.filename), path.join(tarballs, mainPack.filename)], consumer));
  const installed = path.join(consumer, 'node_modules', mainName);
  const nativeInstalled = path.join(consumer, 'node_modules', platform.package);
  const cli = path.join(consumer, 'node_modules/.bin/pw-review');
  const invoke = args => run(cli, args, { cwd: consumer, env });

  await t.test('native package ships installation guidance after a real tarball install', () => {
    const readme = fs.readFileSync(path.join(nativeInstalled, 'README.md'), 'utf8');
    assert.match(readme, /prebuilt Rust/);
    assert.match(readme, /npm install --global playwright-ai-test-reviewer/);
    assert.match(readme, /pw-review --help/);
  });

  await t.test('offline install and npx run the actual native binary', () => {
    assert.equal(success(invoke(['--version'])), success(run(native, ['--version'])));
    const output = run('npx', ['--offline', '--no-install', 'pw-review', '--help'], { cwd: consumer, env });
    assert.equal(success(output), success(run(native, ['--help'])));
    const lock = readJson(path.join(consumer, 'package-lock.json'));
    assert.equal(lock.packages[`node_modules/${platform.package}`].version, main.version);
  });

  await t.test('pass/fail/invalid reports, options, and exit status match native byte for byte', () => {
    const fixture = name => path.join(root, 'tests/fixtures', name);
    const inputs = [
      ['results', fixture('results-playwright-1.56-pass.json'), '--fail-on-error'],
      ['results', fixture('results-playwright-1.56.json'), '--fail-on-error'],
      ['analyze', fixture('browser-hard.zip'), '--fail-on-error'],
      ['analyze', fixture('browser-pass.zip'), '--pretty', '--prompt'],
      ['analyze', fixture('browser-pass.zip'), '--max-output-bytes', '1'],
      ['--unknown'],
      ['analyze', path.join(work, 'missing trace.zip')],
    ];
    const statuses = new Set();
    for (const args of inputs) {
      const direct = run(native, args);
      const wrapped = invoke(args);
      assert.equal(wrapped.status, direct.status, args.join(' '));
      assert.equal(wrapped.stdout, direct.stdout);
      assert.equal(wrapped.stderr, direct.stderr);
      statuses.add(wrapped.status);
    }
    assert.deepEqual([...statuses].sort(), [0, 1, 2]);
  });

  await t.test('missing optional package, mismatched version, missing executable, unsupported release', () => {
    const hidden = `${nativeInstalled}.hidden`;
    fs.renameSync(nativeInstalled, hidden);
    try {
      assert.match(diagnostic(invoke(['--version'])), /optional dependencies enabled/);
    } finally {
      fs.renameSync(hidden, nativeInstalled);
    }
    const nativeJson = path.join(nativeInstalled, 'package.json');
    writeJson(nativeJson, { ...nativeManifest, version: '0.0.0' });
    try {
      assert.match(diagnostic(invoke(['--version'])), /expected/);
    } finally {
      writeJson(nativeJson, nativeManifest);
    }
    const binary = path.join(nativeInstalled, 'bin/pw-review');
    fs.renameSync(binary, `${binary}.hidden`);
    try {
      assert.match(diagnostic(invoke(['--version'])), /Cannot execute/);
    } finally {
      fs.renameSync(`${binary}.hidden`, binary);
    }
    writeJson(path.join(installed, 'package.json'), { ...main, pwReviewPlatforms: [] });
    try {
      assert.match(diagnostic(invoke(['--version'])), /No native binary in this release/);
    } finally {
      writeJson(path.join(installed, 'package.json'), main);
    }
  });

  await t.test('staging rejects bad inputs and never overwrites existing output', () => {
    const output = path.join(work, 'invalid stage');
    const wrongCpu = process.arch === 'arm64' ? 'x86_64' : 'aarch64';
    const wrongTarget = hostTarget.replace(rustCpu, wrongCpu);
    const tiny = path.join(work, 'not a binary');
    fs.writeFileSync(tiny, 'bad');
    for (const args of [
      [], ['not-an-assignment'], ['unsupported=anything'],
      [`${hostTarget}=${path.join(work, 'absent')}`], [`${hostTarget}=${work}`],
      [`${hostTarget}=${tiny}`], [`${wrongTarget}=${native}`],
      [`${hostTarget}=${native}`, `${hostTarget}=${native}`],
    ]) {
      assert.notEqual(run(stager, [output, ...args]).status, 0, args.join(' '));
      assert.equal(fs.existsSync(output), false);
    }
    const wrongVersion = run(stager, [output, `${hostTarget}=${process.execPath}`]);
    assert.notEqual(wrongVersion.status, 0);
    assert.match(wrongVersion.stderr, /host binary must report/);
    assert.equal(fs.existsSync(output), false);
    const sentinel = path.join(stage, 'keep.txt');
    fs.writeFileSync(sentinel, 'keep me');
    assert.notEqual(run(stager, [stage, `${hostTarget}=${native}`]).status, 0);
    assert.equal(fs.readFileSync(sentinel, 'utf8'), 'keep me');
    assert.deepEqual(readJson(path.join(mainDir, 'package.json')), main);
  });

  for (const termination of ['SIGTERM', 'SIGINT', 'SIGHUP']) {
    await t.test(`${termination} reaches the native process and preserves termination`, async signalTest => {
      const ready = path.join(work, `native-ready-${termination}`);
      const child = spawn(cli, ['github', '123', '--repo', 'owner/repo', '--artifact', 'traces'], {
        cwd: consumer,
        env: { ...env, PATH: `${path.join(__dirname, 'fixtures')}${path.delimiter}${process.env.PATH}`, PW_REVIEW_SIGNAL_READY: ready },
        stdio: ['ignore', 'pipe', 'pipe'],
      });
      let stderr = '';
      child.stderr.on('data', bytes => { stderr += bytes; });
      signalTest.after(() => {
        if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
        if (fs.existsSync(ready)) {
          const ghPid = Number(fs.readFileSync(ready, 'utf8').split(' ')[0]);
          try {
            process.kill(-ghPid, 'SIGKILL');
          } catch (error) {
            if (error.code !== 'ESRCH') throw error;
          }
        }
        child.stdout.destroy();
        child.stderr.destroy();
      });
      await waitForFile(ready);
      const nativePid = Number(fs.readFileSync(ready, 'utf8').split(' ')[1]);
      const exited = once(child, 'exit', { signal: AbortSignal.timeout(5000) });
      child.kill(termination);
      const [code, signal] = await exited;
      assert.equal(code, null, stderr);
      assert.equal(signal, termination, stderr);
      assert.throws(() => process.kill(nativePid, 0), { code: 'ESRCH' });
    });
  }
});
