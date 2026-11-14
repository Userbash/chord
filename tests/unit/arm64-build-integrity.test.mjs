// @ts-check
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = fileURLToPath(new URL('../../', import.meta.url));
const script = join(root, 'scripts/build-arm64.sh');
const linux = { skip: process.platform !== 'linux' && 'requires real Linux packaging tools' };

/** @param {import('node:test').TestContext} t */
function fixture(t) {
  const path = mkdtempSync(join(tmpdir(), 'arm64 integrity & '));
  t.after(() => rmSync(path, { recursive: true, force: true }));
  return path;
}

/** @param {string[]} args @param {NodeJS.ProcessEnv} [env] */
function build(args, env = {}) {
  return spawnSync('/bin/bash', [script, ...args], {
    cwd: root, encoding: 'utf8', timeout: 60_000,
    env: { ...process.env, PATH: `${dirname(process.execPath)}:${process.env.PATH}`, ...env },
  });
}

test('missing Cargo cannot manufacture a successful GUI release', linux, t => {
  const dir = fixture(t);
  const out = join(dir, 'unpublished output');
  const result = build(['--out-dir', out, '--report-json', join(dir, 'report.json')], {
    CARGO: join(dir, 'missing cargo'),
  });
  t.diagnostic(`missing Cargo exit=${result.status}; output created=${existsSync(out)}`);
  assert.equal(result.status, 2, result.stderr || result.stdout);
  assert.match(result.stderr, /EnvFail:.*Cargo/i);
  assert.equal(existsSync(out), false, 'preflight must not publish any artifacts');
  assert.equal(existsSync(join(dir, 'report.json')), false);
});

test('an unavailable real runtime fails before touching existing outputs', linux, t => {
  const dir = fixture(t);
  const marker = join(dir, 'SHA256SUMS');
  writeFileSync(marker, 'unchanged independent oracle\n');
  const result = build(['--out-dir', dir], {
    CHORD_ARM64_RUNTIME_FILE: join(dir, 'missing official runtime'),
  });
  assert.equal(result.status, 2, result.stderr || result.stdout);
  assert.match(result.stderr, /EnvFail:/);
  assert.equal(readFileSync(marker, 'utf8'), 'unchanged independent oracle\n');
  assert.equal(existsSync(join(dir, 'discord-gpui-aarch64')), false);
});

test('all preserved hardware profiles plan an actual locked Cargo build', linux, t => {
  const dir = fixture(t);
  /** @type {Array<[string, string]>} */
  const profiles = [['generic', 'generic'], ['rpi5', 'cortex-a76'], ['asahi', 'apple-m1']];
  for (const [profile, cpu] of profiles) {
    const out = join(dir, profile);
    const result = build(['--dry-run', '--profile', profile, '--out-dir', out]);
    assert.equal(result.status, 0, result.stderr);
    const plan = JSON.parse(result.stdout);
    assert.equal(plan.target_cpu, cpu);
    assert.deepEqual(plan.cargo.slice(1, 3), ['build', '--locked']);
    assert.ok(plan.cargo.includes(join(root, 'rust/chord-gpui/Cargo.toml')));
    assert.ok(plan.cargo.includes('aarch64-unknown-linux-gnu'));
    assert.ok(plan.cargo.includes('chord-gpui'));
    assert.ok(plan.cargo.includes('--message-format=json-render-diagnostics'));
    assert.ok(plan.appimagetool.includes('--runtime-file'));
    assert.equal(existsSync(out), false);
  }
});

test('production packaging contains neither generated client nor invented runtime', () => {
  const paths = ['scripts/build-arm64.sh', 'scripts/arm64/build.py', 'scripts/arm64/verify.py'];
  const text = paths.map(path => readFileSync(join(root, path), 'utf8')).join('\n');
  assert.doesNotMatch(text, /SMOKE_JSON_LINE|AI2PAYLOAD|RUNTIME_IDENT|vmrss_mb.*10\.4|ffreestanding/);
  assert.doesNotMatch(text, /https?:\/\/.*(?:latest|continuous)\/.*runtime/);
});

test('official release asset hashes and immutable source commits are pinned', () => {
  const pins = JSON.parse(readFileSync(join(root, 'packaging/arm64/toolchain-pins.json'), 'utf8'));
  assert.equal(pins.runtime.repository, 'AppImage/type2-runtime');
  assert.equal(pins.runtime.commit, 'fedfad51acc2860b2916a1eef32ba4a500ff31ef');
  assert.equal(pins.runtime.asset_id, 657373827);
  assert.equal(pins.runtime.sha256, '9a72a744d221576c9347ec074243acc5c3c7568896b12714e0e82e9f0bf47900');
  assert.equal(pins.appimagetool.commit, '11d54c9acc7f246df1398050a4b880314ac76862');
  assert.equal(pins.appimagetool.assets.x86_64.sha256,
    'e8c1169c4af66c3184d7ee110713d4be04befe671fa358e2c1591ee840553b03');
  assert.equal(pins.appimagetool.assets.aarch64.sha256,
    'd6a061996de690434996c2a523eafedd0a23780aaf02d69ce364537e71607e94');
});

/** @param {string} source @param {string[]} args */
function oracle(source, args) {
  return spawnSync('python3', ['-c', `import sys; sys.path.insert(0, sys.argv.pop(1)); ${source}`,
    join(root, 'scripts/arm64'), ...args], { encoding: 'utf8', timeout: 10_000 });
}

test('the independent ELF check rejects a real host executable of the wrong architecture', linux, () => {
  const result = oracle('from verify import validate_elf; validate_elf(sys.argv[1])', ['/bin/true']);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /EM_AARCH64/);
  assert.doesNotMatch(result.stderr, /ModuleNotFoundError|SyntaxError/);
});

test('a truncated ELF fails validation instead of becoming a release', linux, t => {
  const path = join(fixture(t), 'truncated');
  writeFileSync(path, readFileSync('/bin/true').subarray(0, 40));
  const result = oracle('from verify import validate_elf; validate_elf(sys.argv[1])', [path]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /ELF/);
  assert.doesNotMatch(result.stderr, /ModuleNotFoundError|SyntaxError/);
});

test('a changed local runtime cannot satisfy the official content pin', linux, t => {
  const path = join(fixture(t), 'corrupt runtime');
  writeFileSync(path, 'deliberate corruption, never a package fixture');
  const result = oracle('import json; from verify import check_pinned_file; ' +
    'check_pinned_file(sys.argv[1], json.load(open(sys.argv[2]))["runtime"])',
  [path, join(root, 'packaging/arm64/toolchain-pins.json')]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /SHA-256.*mismatch/);
  assert.doesNotMatch(result.stderr, /ModuleNotFoundError|SyntaxError/);
});

test('publication replaces a destination link without writing through it', linux, t => {
  const dir = fixture(t);
  const stage = join(dir, 'stage');
  const out = join(dir, 'out');
  mkdirSync(stage);
  mkdirSync(out);
  const original = join(dir, 'independent source oracle.txt');
  writeFileSync(original, 'original oracle\n');
  symlinkSync(original, join(out, 'leaf.txt'));
  writeFileSync(join(stage, 'leaf.txt'), 'new output\n');
  const digest = createHash('sha256').update('new output\n').digest('hex');
  writeFileSync(join(stage, 'SHA256SUMS'), `${digest}  leaf.txt\n`);
  const result = oracle('from pathlib import Path; from types import SimpleNamespace; ' +
    'from build import publish; publish(SimpleNamespace(output=Path(sys.argv[2]), cargo_config=None), Path(sys.argv[1]))',
  [stage, out]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(original, 'utf8'), 'original oracle\n');
  assert.equal(readFileSync(join(out, 'leaf.txt'), 'utf8'), 'new output\n');
});

test('verify reports cannot overwrite the independent checksum oracle', linux, t => {
  const out = fixture(t);
  const sums = join(out, 'SHA256SUMS');
  writeFileSync(sums, 'unchanged checksum oracle\n');
  const result = build(['--verify', '--out-dir', out, '--report-json', sums]);
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /report-json.*overwrite/);
  assert.equal(readFileSync(sums, 'utf8'), 'unchanged checksum oracle\n');
});

test('a zero exit without a Cargo compiler artifact cannot reuse a stale executable', linux, t => {
  const result = oracle('import os; from pathlib import Path; from build import run_cargo; ' +
    'run_cargo(["/bin/true"], {"flock":"/usr/bin/flock"}, dict(os.environ), Path("/bin/true"), sys.argv[1])',
  [join(fixture(t), 'negative probe.lock')]);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Cargo did not report.*chord-gpui/);
  assert.doesNotMatch(result.stderr, /ImportError|SyntaxError/);
});

test('verification checks retained checksum entries as well as required outputs', linux, t => {
  const dir = fixture(t);
  for (const name of ['required.txt', 'retained.txt']) writeFileSync(join(dir, name), `${name}\n`);
  writeFileSync(join(dir, 'SHA256SUMS'), ['required.txt', 'retained.txt'].map(name =>
    `${createHash('sha256').update(`${name}\n`).digest('hex')}  ${name}\n`).join(''));
  const source = 'from pathlib import Path; from verify import verify_checksums; ' +
    'verify_checksums(Path(sys.argv[1]), ["required.txt"])';
  const control = oracle(source, [dir]);
  assert.equal(control.status, 0, control.stderr);
  writeFileSync(join(dir, 'retained.txt'), 'corrupt retained output\n');
  const corrupted = oracle(source, [dir]);
  assert.notEqual(corrupted.status, 0);
  assert.match(corrupted.stderr, /SHA-256 mismatch: retained\.txt/);
  assert.doesNotMatch(corrupted.stderr, /ImportError|SyntaxError/);
});
