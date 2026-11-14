// @ts-check
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { accessSync, chmodSync, constants, copyFileSync, mkdirSync, mkdtempSync, rmSync, rmdirSync, statSync, symlinkSync, unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { resolveBrowserBin, runCommand } from '../../scripts/gate.mjs';

const gateUrl = new URL('../../scripts/gate.mjs', import.meta.url);

/** @param {import('node:test').TestContext} t */
function fixtureDirectory(t) {
  const fixture = mkdtempSync(join(tmpdir(), 'gate integration & '));
  t.after(() => {
    assert.equal(dirname(fixture), resolve(tmpdir()));
    assert.ok(basename(fixture).startsWith('gate integration & '));
    rmSync(fixture, { recursive: true, force: true });
  });
  return fixture;
}

test('importing the real gate module exposes deliberate APIs without running CLI phases', () => {
  const code = `const gate = await import(${JSON.stringify(gateUrl.href)}); process.stdout.write(JSON.stringify(Object.keys(gate).sort()));`;
  for (const extraArgs of [[], ['--', 'gate-import-only-nonexistent-entry']]) {
    const result = spawnSync(process.execPath, ['--input-type=module', '-e', code, ...extraArgs], { encoding: 'utf8', timeout: 15000 });
    assert.equal(result.error, undefined);
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stderr, '');
    assert.deepEqual(JSON.parse(result.stdout), ['resolveBrowserBin', 'runCommand']);
  }
});

test('gate runCommand uses the real portable helper and preserves literal npm/npx arguments', t => {
  const fixture = fixtureDirectory(t);
  const args = ['two words', '& echo no', '$(echo no)', '%PATH%', '"quoted"', 'line\nbreak', ''];
  for (const name of ['npm', 'npx']) {
    const cli = join(fixture, `${name}-cli.js`);
    writeFileSync(cli, 'process.stdout.write(JSON.stringify(process.argv.slice(2)));\n');
    const result = runCommand(name, args, { env: { npm_execpath: cli }, timeoutMs: 15000 });
    assert.equal(result.ok, true, result.error ?? result.output);
    assert.equal(result.status, 0);
    assert.equal(result.error, null);
    assert.deepEqual(JSON.parse(result.output), args);
    assert.ok(result.ms >= 0 && Number.isFinite(result.ms));
  }
});

test('gate mixed-case overrides reach the chosen CLI and its child environment on Windows only', t => {
  const fixture = fixtureDirectory(t);
  const oldCli = join(fixture, 'old', 'npm-cli.js');
  const newCli = join(fixture, 'new', 'npm-cli.js');
  const args = ['two words', '& echo no', '$(echo no)'];
  /** @type {Array<[string, string]>} */
  const cliFixtures = [[oldCli, 'old'], [newCli, 'new']];
  for (const [cli, identity] of cliFixtures) {
    mkdirSync(dirname(cli), { recursive: true });
    writeFileSync(cli, `process.stdout.write(JSON.stringify({cli:${JSON.stringify(identity)},args:process.argv.slice(2),value:process.env.GATE_OVERRIDE_VALUE}));\n`);
  }
  /** @type {Array<[string, string]>} */
  const names = [['npm_execpath', 'NPM_EXECPATH'], ['NPM_EXECPATH', 'npm_execpath']];
  for (const [inheritedName, overrideName] of names) {
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) =>
      !['npm_execpath', 'gate_override_value'].includes(key.toLowerCase())));
    env[inheritedName] = oldCli;
    env.GATE_OVERRIDE_VALUE = 'base';
    const code = `const gate = await import(${JSON.stringify(gateUrl.href)});
      process.stdout.write(JSON.stringify(gate.runCommand('npm', ${JSON.stringify(args)}, {
        env: {${JSON.stringify(overrideName)}:${JSON.stringify(newCli)}, Gate_Override_Value:'override'}, timeoutMs:15000
      })));`;
    const child = spawnSync(process.execPath, ['--input-type=module', '-e', code], { encoding: 'utf8', env, timeout: 15000 });
    assert.equal(child.error, undefined);
    assert.equal(child.status, 0, child.stderr);
    const result = JSON.parse(child.stdout);
    assert.equal(result.ok, true, result.error ?? result.output);
    assert.deepEqual(JSON.parse(result.output), {
      cli: process.platform === 'win32' || overrideName === 'npm_execpath' ? 'new' : 'old', args,
      value: process.platform === 'win32' ? 'override' : 'base',
    });
  }
});

test('gate runs installed npm/npx on the native runtime without batch wrappers or a shell', () => {
  if (process.platform === 'win32') {
    const wrapper = join(dirname(process.execPath), 'npm.cmd');
    const raw = spawnSync(wrapper, ['--version'], { encoding: 'utf8', shell: false, timeout: 15000 });
    assert.equal(raw.status, null);
    assert.equal(/** @type {NodeJS.ErrnoException|undefined} */ (raw.error)?.code, 'EINVAL');
  }
  for (const name of ['npm', 'npx']) {
    const result = runCommand(name, ['--version'], { timeoutMs: 15000 });
    assert.equal(result.ok, true, result.error ?? result.output);
    assert.equal(result.status, 0);
    assert.equal(result.error, null);
    assert.match(result.output.trim(), /^\d+\.\d+\.\d+/);
  }
});

test('gate reports thrown spawn errors through the existing failed-result contract', () => {
  const result = runCommand('\0', [], { timeoutMs: 15000 });
  assert.deepEqual(Object.keys(result).sort(), ['error', 'ms', 'ok', 'output', 'status']);
  assert.equal(result.ok, false);
  assert.equal(result.status, null);
  assert.equal(typeof result.error, 'string');
  assert.match(result.error ?? '', /null|invalid|argument/i);
  assert.equal(result.output, '');
  assert.ok(result.ms >= 0 && Number.isFinite(result.ms));
});

test('gate reports npm CLI resolution failures through the same failed-result contract', () => {
  const code = `import fs from 'node:fs'; import { syncBuiltinESMExports } from 'node:module';
    const gate = await import(${JSON.stringify(gateUrl.href)});
    const stat = fs.statSync;
    fs.statSync = (file, ...args) => { if (String(file).endsWith('npm-cli.js')) throw new Error('CLI unavailable'); return stat(file, ...args); };
    syncBuiltinESMExports();
    process.stdout.write(JSON.stringify(gate.runCommand('npm', ['--version'], { timeoutMs: 15000 })));`;
  const child = spawnSync(process.execPath, ['--input-type=module', '-e', code], { encoding: 'utf8', timeout: 15000 });
  assert.equal(child.error, undefined);
  assert.equal(child.status, 0, child.stderr);
  const result = JSON.parse(child.stdout);
  assert.equal(result.ok, false);
  assert.equal(result.status, null);
  assert.match(result.error, /npm CLI JavaScript not found/);
  assert.equal(result.output, '');
  assert.ok(result.ms >= 0 && Number.isFinite(result.ms));
});

test('gate retains nonzero exit status, stdout, and stderr for ordinary commands', () => {
  const result = runCommand(process.execPath, ['-e', 'process.stdout.write("out"); process.stderr.write("err"); process.exitCode = 7;']);
  assert.equal(result.ok, false);
  assert.equal(result.status, 7);
  assert.equal(result.error, null);
  assert.equal(result.output, 'outerr');
});

test('gate browser overrides retain priority and reject an explicitly missing browser', t => {
  const fixture = fixtureDirectory(t);
  const explicit = join(fixture, 'explicit-browser.exe');
  const fallback = join(fixture, 'fallback-browser.exe');
  const missing = join(fixture, 'missing-browser.exe');
  for (const file of [explicit, fallback]) {
    writeFileSync(file, 'browser fixture; never launched');
    chmodSync(file, 0o755);
  }
  const options = { knownPaths: [fallback], allowFlatpak: false };
  assert.deepEqual(resolveBrowserBin({ CHROME_BIN: explicit, CHROME_PATH: fallback, PATH: '' }, options), {
    bin: explicit, explicitMissing: null, source: 'env',
  });
  assert.deepEqual(resolveBrowserBin({ CHROME_PATH: explicit, PATH: '' }, options), {
    bin: explicit, explicitMissing: null, source: 'env',
  });
  assert.deepEqual(resolveBrowserBin({ CHROME_BIN: missing, PATH: '' }, options), {
    bin: null, explicitMissing: missing, source: 'env-missing',
  });
  assert.deepEqual(resolveBrowserBin({ PATH: '' }, options), {
    bin: fallback, explicitMissing: null, source: 'known-path',
  });
});

test('gate default browser discovery uses the current users platform installation paths', t => {
  const fixture = fixtureDirectory(t);
  const browser = process.platform === 'win32'
    ? join(fixture, 'local-app-data', 'Google', 'Chrome', 'Application', 'chrome.exe')
    : process.platform === 'darwin'
      ? join(fixture, 'Applications', 'Google Chrome.app', 'Contents', 'MacOS', 'Google Chrome')
      : join(fixture, '.local', 'bin', 'chromium');
  mkdirSync(dirname(browser), { recursive: true });
  writeFileSync(browser, 'browser fixture; never launched');
  chmodSync(browser, 0o755);
  const code = `const gate = await import(${JSON.stringify(gateUrl.href)}); process.stdout.write(JSON.stringify(gate.resolveBrowserBin({PATH:''}, {allowFlatpak:false})));`;
  const result = spawnSync(process.execPath, ['--input-type=module', '-e', code], {
    encoding: 'utf8', timeout: 15000,
    env: { ...process.env, HOME: fixture, USERPROFILE: fixture, LOCALAPPDATA: join(fixture, 'local-app-data') },
  });
  assert.equal(result.error, undefined);
  assert.equal(result.status, 0, result.stderr);
  const systemPaths = process.platform === 'win32' ? [
    join(process.env.ProgramFiles || 'C:\\Program Files', 'Google', 'Chrome', 'Application', 'chrome.exe'),
    join(process.env['ProgramFiles(x86)'] || 'C:\\Program Files (x86)', 'Google', 'Chrome', 'Application', 'chrome.exe'),
  ] : process.platform === 'darwin' ? [
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', '/Applications/Chromium.app/Contents/MacOS/Chromium',
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge', '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
  ] : [];
  const installedBrowser = systemPaths.find(candidate => {
    try { accessSync(candidate, constants.X_OK); return statSync(candidate).isFile(); } catch { return false; }
  });
  assert.deepEqual(JSON.parse(result.stdout), { bin: installedBrowser ?? browser, explicitMissing: null, source: 'known-path' });
});

test('standalone gate help and invalid-scope exit behavior remain available', () => {
  /** @type {Array<[string[], number]>} */
  const cases = [[['--help'], 0], [['--scope', 'invalid'], 1]];
  for (const [args, status] of cases) {
    const result = spawnSync(process.execPath, [fileURLToPath(gateUrl), ...args], { encoding: 'utf8', timeout: 15000 });
    assert.equal(result.error, undefined);
    assert.equal(result.status, status, result.stderr);
    assert.match(result.stdout, /worker|release|qualify/);
    assert.equal(result.stderr, '');
  }
});

test('standalone gate help also runs through a symlinked or junction checkout', t => {
  const fixture = fixtureDirectory(t);
  const project = join(fixture, 'project');
  const scripts = join(project, 'scripts');
  mkdirSync(scripts, { recursive: true });
  copyFileSync(fileURLToPath(gateUrl), join(scripts, 'gate.mjs'));
  copyFileSync(fileURLToPath(new URL('../../scripts/gate-platform.mjs', import.meta.url)), join(scripts, 'gate-platform.mjs'));
  copyFileSync(fileURLToPath(new URL('../../package.json', import.meta.url)), join(project, 'package.json'));
  const alias = join(fixture, 'checkout-alias');
  symlinkSync(project, alias, process.platform === 'win32' ? 'junction' : 'dir');
  try {
    const result = spawnSync(process.execPath, [join(alias, 'scripts', 'gate.mjs'), '--help'], { encoding: 'utf8', timeout: 15000 });
    assert.equal(result.error, undefined);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /worker|release|qualify/);
    assert.equal(result.stderr, '');
  } finally {
    if (process.platform === 'win32') rmdirSync(alias);
    else unlinkSync(alias);
  }
});
