// @ts-check
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = fileURLToPath(new URL('../../', import.meta.url));
const linux = { skip: process.platform !== 'linux' && 'requires Linux ARM64 sysroot/configuration semantics' };

/** @param {string} profile */
function configuration(profile) {
  // Exercise real configuration generation with an isolated sysroot directory.
  // This checks compiler inputs only; no SDK, binary or native qualification is implied.
  const source = `
import json, sys, tempfile, tomllib
from pathlib import Path
from types import SimpleNamespace
sys.path.insert(0, str(Path(sys.argv[1]) / 'scripts/arm64'))
from build import TARGET, build_plan, cargo_configuration, cross_environment
with tempfile.TemporaryDirectory(prefix='chord arm cpu ') as temporary:
    sysroot = Path(temporary)
    (sysroot / 'usr/include').mkdir(parents=True)
    args = SimpleNamespace(profile=sys.argv[2], sysroot=str(sysroot), pkg_config_libdir=None,
        appimagetool=None, runtime_file=None, output=sysroot / 'unpublished', version='1.0.0')
    tools = {'clang':'clang', 'clang++':'clang++', 'llvm-ar':'llvm-ar'}
    values, flags = cross_environment(args, tools)
    generated = tomllib.loads(cargo_configuration(args, tools, values, flags))
    checked_in = tomllib.loads((Path(sys.argv[1]) / 'packaging/arm64/cargo-config.toml').read_text())
    print(json.dumps({'target':TARGET, 'generated':generated, 'checked_in':checked_in,
        'values':values, 'flags':flags, 'plan':build_plan(args)}))
`;
  const result = spawnSync('python3', ['-c', source, root, profile], {
    encoding: 'utf8', timeout: 10_000,
  });
  assert.equal(result.status, 0, result.stderr || String(result.error));
  return JSON.parse(result.stdout);
}

/** @param {string[]} flags */
function noForcedOptionalExtensions(flags) {
  const explicit = flags.filter(flag => flag.startsWith('target-feature='));
  for (const flag of explicit) assert.doesNotMatch(flag, /\+(?:crc|aes|sha2|crypto)(?:,|$)/);
}

test('generated generic Rust and C inputs preserve the ARMv8-A baseline', linux, () => {
  const result = configuration('generic');
  const target = result.generated.target[result.target];
  noForcedOptionalExtensions(target.rustflags);
  assert.ok(target.rustflags.includes('target-cpu=generic'));
  assert.deepEqual(result.flags.filter(/** @param {string} flag */ flag => flag.startsWith('-march=')),
    ['-march=armv8-a']);
  assert.equal(result.values.CFLAGS_aarch64_unknown_linux_gnu,
    result.values.CXXFLAGS_aarch64_unknown_linux_gnu);
});

test('checked-in generic configuration has the same portable compiler contract', linux, () => {
  const { checked_in: config } = configuration('generic');
  assert.equal(config.build.target, 'aarch64-unknown-linux-gnu');
  noForcedOptionalExtensions(config.target['aarch64-unknown-linux-gnu'].rustflags);
  assert.ok(config.target['aarch64-unknown-linux-gnu'].rustflags.includes('target-cpu=generic'));
  for (const name of ['CFLAGS_aarch64_unknown_linux_gnu', 'CXXFLAGS_aarch64_unknown_linux_gnu']) {
    assert.match(config.env[name], /(?:^| )-march=armv8-a(?: |$)/);
    assert.doesNotMatch(config.env[name], /\+(?:crc|aes|sha2|crypto)/);
  }
  assert.equal(config.env.CFLAGS_aarch64_unknown_linux_gnu, config.env.CXXFLAGS_aarch64_unknown_linux_gnu);
});

/** @type {Array<[string, string]>} */
const optimizedProfiles = [['rpi5', 'cortex-a76'], ['asahi', 'apple-m1']];
for (const [profile, cpu] of optimizedProfiles) {
  test(`${profile} retains explicit CPU tuning without blanket feature overrides`, linux, () => {
    const result = configuration(profile);
    const flags = result.generated.target[result.target].rustflags;
    assert.equal(result.plan.target_cpu, cpu);
    assert.ok(flags.includes(`target-cpu=${cpu}`));
    noForcedOptionalExtensions(flags);
    assert.deepEqual(result.flags.filter(/** @param {string} flag */ flag => flag.startsWith('-mcpu=')),
      [`-mcpu=${cpu}`]);
    assert.equal(result.values.CFLAGS_aarch64_unknown_linux_gnu,
      result.values.CXXFLAGS_aarch64_unknown_linux_gnu);
  });
}

test('every profile preserves target, link hardening and forced target build environment', linux, () => {
  for (const profile of ['generic', 'rpi5', 'asahi']) {
    const result = configuration(profile);
    const config = result.generated;
    assert.equal(config.build.target, 'aarch64-unknown-linux-gnu');
    const flags = config.target[result.target].rustflags;
    for (const required of ['link-arg=--target=aarch64-unknown-linux-gnu',
      'link-arg=-fuse-ld=lld', 'link-arg=-Wl,--build-id=sha1',
      'link-arg=-Wl,-z,relro,-z,now,-z,noexecstack',
      'link-arg=-Wl,-z,max-page-size=16384,-z,common-page-size=16384']) {
      assert.ok(flags.includes(required), `${profile}: missing ${required}`);
    }
    for (const [key, value] of Object.entries(result.values)) {
      assert.deepEqual(config.env[key], { value, force: true });
    }
  }
});

test('profile plans retain a locked GUI build and honest native-validation state', linux, () => {
  for (const profile of ['generic', 'rpi5', 'asahi']) {
    const { plan } = configuration(profile);
    assert.equal(plan.target, 'aarch64-unknown-linux-gnu');
    assert.equal(plan.native_validation, 'not_run');
    assert.equal(plan.self_contained, false);
    assert.equal(plan.dry_run, true);
    assert.ok(plan.cargo.includes('--locked'));
    assert.ok(plan.cargo.includes('chord-gpui'));
    assert.ok(plan.appimagetool.includes('--runtime-file'));
  }
});
