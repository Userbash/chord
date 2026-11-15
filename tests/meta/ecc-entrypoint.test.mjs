// @ts-check
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {
  chmodSync, copyFileSync, lstatSync, mkdirSync, mkdtempSync,
  readlinkSync, rmSync, symlinkSync, writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const source = resolve(dirname(fileURLToPath(import.meta.url)), '../../scripts/ecc');
const posixOnly = process.platform === 'win32'
  ? 'Run POSIX entrypoint contracts in WSL, Linux or macOS.' : false;

/**
 * Create a relocated checkout without requiring an installed ECC or provider.
 * @param {import('node:test').TestContext} t
 */
function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'ecc-entrypoint-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const checkout = join(root, 'relocated checkout with spaces');
  const scripts = join(checkout, 'scripts');
  const cwd = join(root, 'unrelated working directory');
  const home = join(root, 'isolated home');
  for (const directory of [scripts, cwd, home]) mkdirSync(directory, { recursive: true });
  const launcher = join(scripts, 'ecc');
  const stat = lstatSync(source);
  if (stat.isSymbolicLink()) symlinkSync(readlinkSync(source), launcher);
  else {
    copyFileSync(source, launcher);
    chmodSync(launcher, stat.mode & 0o777);
  }
  return { root, scripts, cwd, home, launcher };
}

/**
 * A local process oracle prints its identity and exact argument bytes.
 * @param {string} directory
 * @param {string} tag
 */
function backend(directory, tag, code = 0) {
  mkdirSync(directory, { recursive: true });
  const executable = join(directory, 'ecc');
  writeFileSync(executable, `#!/bin/sh\nprintf '%s\\n' '${tag}' "$$"\nprintf '%s\\0' "$@"\nexit ${code}\n`);
  chmodSync(executable, 0o755);
  return executable;
}

/**
 * @param {ReturnType<typeof fixture>} f
 * @param {string[]} args
 * @param {NodeJS.ProcessEnv} [overrides]
 */
function invoke(f, args, overrides = {}) {
  return spawnSync(f.launcher, args, {
    cwd: f.cwd,
    env: { HOME: f.home, PATH: f.scripts, ...overrides },
    encoding: 'utf8',
    timeout: 3000,
  });
}

/**
 * @param {ReturnType<typeof invoke>} result
 * @param {string} tag
 * @param {string[]} args
 */
function assertBackend(result, tag, args, code = 0) {
  assert.equal(result.error, undefined, result.error?.message);
  assert.equal(result.signal, null);
  assert.equal(result.status, code, result.stderr);
  const [actualTag, actualPid, ...rest] = result.stdout.split('\n');
  assert.equal(actualTag, tag);
  assert.equal(Number(actualPid), result.pid, 'launcher must exec, preserving process identity');
  assert.deepEqual(rest.join('\n').split('\0').slice(0, -1), args);
  assert.equal(result.stderr, '');
}

test('ECC entrypoint is an executable regular file, not a machine-specific symlink', { skip: posixOnly }, () => {
  const stat = lstatSync(source);
  assert.equal(stat.isSymbolicLink(), false);
  assert.equal(stat.isFile(), true);
  assert.equal(stat.mode & 0o777, 0o755);
});

test('ECC_BIN wins and preserves literal arguments, exit status and exec identity', { skip: posixOnly }, (t) => {
  const f = fixture(t);
  const explicit = backend(join(f.root, 'explicit backend with spaces'), 'explicit', 37);
  const pathBackend = dirname(backend(join(f.root, 'path backend'), 'path'));
  backend(join(f.home, '.local/bin'), 'home');
  const args = ['--task', 'two words', '$(printf injected)', '`printf injected`', '; exit 99', '*', '', 'Unicode: Ж', 'line\nbreak'];
  assertBackend(invoke(f, args, { ECC_BIN: explicit, PATH: pathBackend }), 'explicit', args, 37);
});

test('PATH discovery skips this launcher and continues to a real backend', { skip: posixOnly }, (t) => {
  const f = fixture(t);
  const directory = dirname(backend(join(f.root, 'PATH backend with spaces'), 'path', 19));
  backend(join(f.home, '.local/bin'), 'home');
  const args = ['--version', 'literal argument'];
  assertBackend(invoke(f, args, { PATH: `${f.scripts}:${directory}` }), 'path', args, 19);
});

test('PATH directory glob characters are literal and do not select another backend', { skip: posixOnly }, (t) => {
  const f = fixture(t);
  backend(join(f.root, 'backend-a'), 'decoy');
  const directory = dirname(backend(join(f.root, 'backend-*'), 'literal'));
  assertBackend(invoke(f, ['--help'], { PATH: directory }), 'literal', ['--help']);
});

test('user-local backend is used when PATH only contains the launcher', { skip: posixOnly }, (t) => {
  const f = fixture(t);
  backend(join(f.home, '.local/bin'), 'home', 23);
  assertBackend(invoke(f, ['--version']), 'home', ['--version'], 23);
});

test('missing backend returns an English environment error with exit 2', { skip: posixOnly }, (t) => {
  const f = fixture(t);
  const result = invoke(f, ['--version']);
  assert.equal(result.error, undefined, result.error?.message);
  assert.equal(result.signal, null);
  assert.equal(result.status, 2, result.stderr);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /ECC CLI unavailable/);
  assert.match(result.stderr, /ECC_BIN/);
});

test('invalid explicit ECC_BIN fails instead of silently selecting another CLI', { skip: posixOnly }, (t) => {
  const f = fixture(t);
  const directory = dirname(backend(join(f.root, 'path backend'), 'wrong-backend'));
  backend(join(f.home, '.local/bin'), 'wrong-home');
  const result = invoke(f, ['--version'], { ECC_BIN: join(f.root, 'missing ECC'), PATH: directory });
  assert.equal(result.error, undefined, result.error?.message);
  assert.equal(result.status, 2, result.stderr);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /ECC CLI unavailable/);
});

test('explicit alias to this launcher cannot recurse', { skip: posixOnly }, (t) => {
  const f = fixture(t);
  const alias = join(f.root, 'self alias');
  symlinkSync(f.launcher, alias);
  const result = invoke(f, ['--version'], { ECC_BIN: alias });
  assert.equal(result.error, undefined, result.error?.message);
  assert.equal(result.signal, null);
  assert.equal(result.status, 2, result.stderr);
  assert.equal(result.stdout, '');
  assert.match(result.stderr, /ECC CLI unavailable/);
});
