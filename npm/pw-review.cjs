#!/usr/bin/env node
'use strict';

const { spawn } = require('node:child_process');
const { dirname, join } = require('node:path');
const manifest = require('./package.json');

function fail(message) {
  process.stderr.write(`${JSON.stringify({ schema_version: 1, error: { kind: 'distribution', message } })}\n`);
  process.exitCode = 2;
}

function launch() {
  const libc = process.platform === 'linux'
    ? (process.report.getReport().header.glibcVersionRuntime ? 'glibc' : 'musl')
    : undefined;
  const platform = manifest.pwReviewPlatforms.find(entry =>
    entry.os === process.platform && entry.cpu === process.arch && entry.libc === libc);
  if (!platform) {
    fail(`No native binary in this release for ${process.platform}/${process.arch}${libc ? `/${libc}` : ''}. Use a supported release or build pw-review with Cargo.`);
    return;
  }

  let binary;
  try {
    const packagePath = require.resolve(`${platform.package}/package.json`);
    const installed = require(packagePath);
    if (installed.version !== manifest.version) {
      fail(`Native package ${platform.package} has version ${installed.version}; expected ${manifest.version}. Reinstall matching packages.`);
      return;
    }
    binary = join(dirname(packagePath), 'bin', 'pw-review');
  } catch (error) {
    fail(`Cannot load ${platform.package}@${manifest.version}: ${error.message}. Reinstall with optional dependencies enabled (do not use --omit=optional).`);
    return;
  }

  const child = spawn(binary, process.argv.slice(2), { stdio: 'inherit' });
  const signals = ['SIGINT', 'SIGTERM', 'SIGHUP'];
  const forward = signal => child.kill(signal);
  for (const signal of signals) process.on(signal, forward);
  child.on('error', error => fail(`Cannot execute ${binary}: ${error.message}`));
  child.on('exit', (code, signal) => {
    for (const name of signals) process.removeListener(name, forward);
    if (signal) process.kill(process.pid, signal);
    else process.exitCode = code;
  });
}

launch();
