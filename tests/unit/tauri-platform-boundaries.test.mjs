import assert from 'node:assert/strict';
import { after, before, test } from 'node:test';
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// These are build-metadata fixtures, not native library or runtime substitutes.
// Compile the actual build.rs with the real vcpkg crate, then vary Cargo's target
// environment independently of the host to catch accidental host cfg selection.
const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const buildScript = join(root, 'rust/crates/chord-platform/build.rs');
/** @type {string} */
let directory;
/** @type {string} */
let executable;
before(() => {
  assert.ok(existsSync(buildScript), 'platform must discover MSVC Hunspell through the installed vcpkg port');
  directory = mkdtempSync(join(tmpdir(), 'chord-platform-build-'));
  writeFileSync(join(directory, 'Cargo.toml'), `[package]\nname="chord-platform-build-contract"\nversion="0.0.0"\nedition="2021"\n[workspace]\n[[bin]]\nname="chord-platform-build-contract"\npath=${JSON.stringify(buildScript)}\n[dependencies]\nvcpkg="=0.2.15"\n`);
  const args = ['build', '--offline', '--manifest-path', join(directory, 'Cargo.toml')];
  /** @type {NodeJS.ProcessEnv} */
  const env = { ...process.env, CARGO_BUILD_JOBS: '4', CHORD_CARGO_LOCK_HELD: '1' };
  const result = process.platform === 'linux'
    ? spawnSync('flock', ['/tmp/chord-cargo.lock', 'cargo', ...args], { env, encoding: 'utf8' })
    : spawnSync('cargo', args, { env, encoding: 'utf8' });
  assert.equal(result.status, 0, `actual build script must compile:\n${result.stdout}\n${result.stderr}`);
  const target = process.env.CARGO_TARGET_DIR ? resolve(root, process.env.CARGO_TARGET_DIR) : join(directory, 'target');
  executable = join(target, 'debug', `chord-platform-build-contract${process.platform === 'win32' ? '.exe' : ''}`);
});
after(() => { if (directory) rmSync(directory, { recursive: true, force: true }); });

/** @param {string} triplet @returns {string} */
function installedPort(triplet) {
  const vcpkg = join(directory, triplet);
  writeFileSync(join(directory, '.vcpkg-root'), '');
  for (const path of ['lib/pkgconfig', 'bin', 'include']) mkdirSync(join(vcpkg, 'installed', triplet, path), { recursive: true });
  const prefix = join(vcpkg, 'installed');
  for (const path of ['vcpkg/info', 'vcpkg/updates']) mkdirSync(join(prefix, path), { recursive: true });
  writeFileSync(join(vcpkg, '.vcpkg-root'), '');
  let status = '';
  /** @type {Array<[string, string]>} */
  const portLibraries = [['hunspell', 'metadata_hunspell'], ['libiconv', 'metadata_iconv']];
  for (const [port, stem] of portLibraries) {
    status += `Package: ${port}\nVersion: 1.0\nArchitecture: ${triplet}\nMulti-Arch: same\nStatus: install ok installed\n${port === 'hunspell' ? 'Depends: libiconv\n' : ''}\n`;
    const lib = `${triplet}/lib/${stem}.lib`;
    const dll = `${triplet}/bin/${stem}.dll`;
    writeFileSync(join(prefix, lib), '');
    writeFileSync(join(prefix, dll), '');
    writeFileSync(join(prefix, 'vcpkg/info', `${port}_1.0_${triplet}.list`), `${lib}\n${dll}\n`);
  }
  writeFileSync(join(prefix, 'vcpkg/status'), status);
  return vcpkg;
}

/**
 * @param {string} target
 * @param {string} vcpkg
 */
function run(target, vcpkg) {
  /** @type {NodeJS.ProcessEnv} */
  const env = { ...process.env, TARGET: target,
    CARGO_CFG_TARGET_OS: target.includes('windows') ? 'windows' : 'linux',
    CARGO_CFG_TARGET_ENV: target.endsWith('msvc') ? 'msvc' : 'gnu',
    CARGO_CFG_TARGET_FEATURE: '', VCPKGRS_DYNAMIC: '1', VCPKG_ROOT: vcpkg,
  };
  delete env.VCPKGRS_TRIPLET;
  delete env.VCPKG_TARGET_TRIPLET;
  delete env.VCPKGRS_DISABLE;
  delete env.NO_VCPKG;
  delete env.VCPKGRS_NO_HUNSPELL;
  return spawnSync(executable, [], { env, encoding: 'utf8' });
}

/** @type {Array<[string, string]>} */
const nativeTargets = [['x86_64-pc-windows-msvc', 'x64-windows'], ['aarch64-pc-windows-msvc', 'arm64-windows']];
for (const [target, triplet] of nativeTargets) {
  test(`MSVC discovery reads installed port names and dependencies for ${target}`, () => {
    const result = run(target, installedPort(triplet));
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /cargo:rustc-link-lib=metadata_hunspell(?:\r?\n|$)/);
    assert.match(result.stdout, /cargo:rustc-link-lib=metadata_iconv(?:\r?\n|$)/);
    assert.ok(result.stdout.includes(join('installed', triplet, 'lib')));
    assert.doesNotMatch(result.stdout, /cargo:rustc-link-lib=hunspell(?:\r?\n|$)/);
  });
}
test('MSVC missing port fails, while non-MSVC targets preserve system linkage', () => {
  const missing = join(directory, 'missing');
  assert.notEqual(run('x86_64-pc-windows-msvc', missing).status, 0);
  const unix = run('x86_64-unknown-linux-gnu', missing);
  assert.equal(unix.status, 0, unix.stderr);
  assert.doesNotMatch(unix.stdout, /cargo:rustc-link-lib=/);
  const mingw = run('x86_64-pc-windows-gnu', missing);
  assert.equal(mingw.status, 0, mingw.stderr);
  assert.doesNotMatch(mingw.stdout, /cargo:rustc-link-lib=/);
});

test('product Windows and macOS icons contain valid native resource containers', () => {
  const icon = readFileSync(join(root, 'rust/gui-spike/icons/icon.ico'));
  assert.equal(icon.readUInt16LE(0), 0);
  assert.equal(icon.readUInt16LE(2), 1);
  const count = icon.readUInt16LE(4);
  assert.ok(count > 0);
  assert.ok(6 + 16 * count <= icon.length);
  const sizes = [];
  for (let i = 0; i < count; i += 1) {
    const offset = 6 + 16 * i;
    const width = icon[offset] || 256;
    const height = icon[offset + 1] || 256;
    sizes.push(width);
    assert.equal(width, height);
    const bytes = icon.readUInt32LE(offset + 8);
    const start = icon.readUInt32LE(offset + 12);
    assert.ok(bytes > 0 && start >= 6 + 16 * count && start + bytes <= icon.length);
  }
  assert.ok(sizes.includes(32));
  assert.ok(sizes.includes(256));
  const icns = readFileSync(join(root, 'rust/gui-spike/icons/icon.icns'));
  assert.equal(icns.toString('ascii', 0, 4), 'icns');
  assert.equal(icns.readUInt32BE(4), icns.length);
  let offset = 8;
  let chunks = 0;
  while (offset < icns.length) {
    const length = icns.readUInt32BE(offset + 4);
    assert.ok(length > 8 && offset + length <= icns.length);
    offset += length;
    chunks += 1;
  }
  assert.equal(offset, icns.length);
  assert.ok(chunks > 0);
});
