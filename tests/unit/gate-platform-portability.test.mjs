// @ts-check
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { browserCandidatePaths, commandInvocation, normalizeEnvironment } from '../../scripts/gate-platform.mjs';

test('npm_execpath wins over independently injected and distribution CLI candidates', () => {
  const cli = '/custom npm/bin/npm-cli.js';
  /** @type {string[]} */
  const probed = [];
  const result = commandInvocation('npm', ['run', 'typecheck'], {
    platform: 'linux', execPath: '/opt/node/bin/node', env: { npm_execpath: cli },
    cliCandidates: ['/another/npm-cli.js'],
    isFile: candidate => { probed.push(candidate); return true; },
  });
  assert.deepEqual(result, { command: '/opt/node/bin/node', args: [cli, 'run', 'typecheck'] });
  assert.deepEqual(probed, [cli]);
});

test('npx resolves its own sibling CLI rather than running npm with npx arguments', () => {
  const cli = '/custom npm/bin/npx-cli.js';
  assert.deepEqual(commandInvocation('npx', ['--version'], {
    platform: 'linux', execPath: '/opt/node/bin/node',
    env: { npm_execpath: '/custom npm/bin/npm-cli.js' }, isFile: candidate => candidate === cli,
  }), { command: '/opt/node/bin/node', args: [cli, '--version'] });
});

test('Node distribution layouts resolve on Linux, Debian, and Windows using a fake filesystem', () => {
  const fixtures = [
    { platform: 'linux', execPath: '/opt/node/bin/node', cli: '/opt/node/lib/node_modules/npm/bin/npm-cli.js' },
    { platform: 'linux', execPath: '/usr/bin/node', cli: '/usr/share/nodejs/npm/bin/npm-cli.js' },
    { platform: 'win32', execPath: 'C:\\Program Files\\nodejs\\node.exe', cli: 'C:\\Program Files\\nodejs\\node_modules\\npm\\bin\\npm-cli.js' },
  ];
  for (const { platform, execPath, cli } of fixtures) {
    assert.deepEqual(commandInvocation(platform === 'win32' ? 'npm.cmd' : 'npm', ['--version'], {
      platform, execPath, env: {}, isFile: candidate => candidate === cli,
    }), { command: execPath, args: [cli, '--version'] });
  }
});

test('Windows wrapper paths and case-insensitive environment names resolve the npm CLI', () => {
  const cli = 'D:\\npm install\\bin\\npm-cli.js';
  assert.deepEqual(commandInvocation('C:\\tools\\NPM.CMD', ['run', 'check'], {
    platform: 'win32', execPath: 'C:\\Node\\node.exe', env: { NPM_EXECPATH: cli },
    isFile: candidate => candidate === cli,
  }), { command: 'C:\\Node\\node.exe', args: [cli, 'run', 'check'] });
});

test('Windows resolves the last mixed-case npm override while POSIX uses the exact name', () => {
  assert.deepEqual(commandInvocation('npm', ['--version'], {
    platform: 'win32', execPath: 'C:\\Node\\node.exe',
    env: { npm_execpath: 'C:\\old\\npm-cli.js', NPM_EXECPATH: 'C:\\new\\npm-cli.js' },
    isFile: () => true,
  }), { command: 'C:\\Node\\node.exe', args: ['C:\\new\\npm-cli.js', '--version'] });
  assert.deepEqual(commandInvocation('npm', ['--version'], {
    platform: 'linux', execPath: '/node/bin/node',
    env: { NPM_EXECPATH: '/wrong/npm-cli.js', npm_execpath: '/exact/npm-cli.js' },
    isFile: () => true,
  }), { command: '/node/bin/node', args: ['/exact/npm-cli.js', '--version'] });
  assert.equal(browserCandidatePaths({ platform: 'linux', env: { home: '/wrong', HOME: '/exact' } })[0],
    '/exact/.local/bin/chromium');
});

test('Windows normalizes aliases once while POSIX names and input objects stay intact', () => {
  const env = Object.freeze({ Path: 'old', PATH: 'new', Empty: 'filled', EMPTY: '', Gone: 'present', GONE: undefined });
  assert.deepEqual(normalizeEnvironment(env, 'win32'), { PATH: 'new', EMPTY: '', GONE: undefined });
  assert.deepEqual(normalizeEnvironment(env, 'linux'), env);
  assert.deepEqual(normalizeEnvironment(env, 'darwin'), env);
  assert.deepEqual(env, { Path: 'old', PATH: 'new', Empty: 'filled', EMPTY: '', Gone: 'present', GONE: undefined });
});

test('missing or non-JavaScript npm hints fall back to real CLI files, never command wrappers', () => {
  const cli = '/injected/npm-cli.js';
  for (const npm_execpath of ['/missing/npm-cli.js', '/tools/npm.cmd']) {
    assert.deepEqual(commandInvocation('npm', [], {
      platform: 'linux', execPath: '/opt/node/bin/node', env: { npm_execpath },
      cliCandidates: [cli], isFile: candidate => candidate === cli || candidate.endsWith('.cmd'),
    }), { command: '/opt/node/bin/node', args: [cli] });
  }
  assert.throws(() => commandInvocation('npm', [], {
    platform: 'linux', execPath: '/node/bin/node', env: {}, isFile: () => false,
  }), /npm.*CLI.*not found/i);
});

test('non-npm commands and their literal arguments remain unchanged', () => {
  const args = Object.freeze(['a b', 'x&y', '$(echo no)', '"quoted"', 'a\nb']);
  for (const command of ['node', 'cargo', '/tools/npm-helper', 'cmd.exe']) {
    assert.deepEqual(commandInvocation(command, args, {
      isFile: () => { throw new Error('non-npm commands must not inspect npm files'); },
    }), { command, args });
  }
  assert.deepEqual(args, ['a b', 'x&y', '$(echo no)', '"quoted"', 'a\nb']);
});

test('resolved npm and npx pass argument boundaries literally to a real Node process', t => {
  const fixture = mkdtempSync(join(tmpdir(), 'gate invocation & '));
  t.after(() => {
    assert.equal(dirname(fixture), resolve(tmpdir()));
    assert.ok(basename(fixture).startsWith('gate invocation & '));
    rmSync(fixture, { recursive: true, force: true });
  });
  const cliDir = join(fixture, 'node_modules', 'npm', 'bin');
  mkdirSync(cliDir, { recursive: true });
  const args = Object.freeze(['two words', '& echo injected', '$(echo injected)', '%PATH%', '^caret', '"quotes"', 'line\nbreak', '']);
  for (const name of ['npm', 'npx']) {
    const cli = join(cliDir, `${name}-cli.js`);
    writeFileSync(cli, 'process.stdout.write(JSON.stringify(process.argv.slice(2)));\n');
    const invocation = commandInvocation(name, args, { env: { npm_execpath: cli } });
    const result = spawnSync(invocation.command, invocation.args, { encoding: 'utf8', shell: false, timeout: 15000 });
    assert.equal(result.error, undefined);
    assert.equal(result.status, 0, result.stderr);
    assert.deepEqual(JSON.parse(result.stdout), args);
  }
});

test('a directory named npm-cli.js cannot be used as a JavaScript CLI', t => {
  const fixture = mkdtempSync(join(tmpdir(), 'gate CLI directory '));
  t.after(() => {
    assert.equal(dirname(fixture), resolve(tmpdir()));
    assert.ok(basename(fixture).startsWith('gate CLI directory '));
    rmSync(fixture, { recursive: true, force: true });
  });
  const invalid = join(fixture, 'npm-cli.js');
  const cli = join(fixture, 'real', 'npm-cli.js');
  mkdirSync(invalid);
  mkdirSync(dirname(cli));
  writeFileSync(cli, 'process.stdout.write("ok");\n');
  assert.deepEqual(commandInvocation('npm', [], { env: { npm_execpath: invalid }, cliCandidates: [cli] }), {
    command: process.execPath, args: [cli],
  });
});

test('installed npm and npx CLI JavaScript execute on the current native Node runtime', () => {
  for (const command of ['npm', 'npx']) {
    const invocation = commandInvocation(command, ['--version']);
    assert.equal(invocation.command, process.execPath);
    assert.match(invocation.args[0] ?? '', new RegExp(`${command}-cli\\.js$`, 'i'));
    const result = spawnSync(invocation.command, invocation.args, { encoding: 'utf8', shell: false, timeout: 15000 });
    assert.equal(result.error, undefined);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout.trim(), /^\d+\.\d+\.\d+/);
  }
});

test('Linux browser candidates preserve standard installations and derive the local path from home', () => {
  assert.deepEqual(browserCandidatePaths({ platform: 'linux', env: {}, home: '/home/Ada Lovelace' }), [
    '/home/Ada Lovelace/.local/bin/chromium',
    '/usr/bin/chromium', '/usr/bin/chromium-browser', '/usr/bin/google-chrome',
    '/usr/bin/google-chrome-stable', '/usr/bin/brave-browser', '/usr/bin/brave',
    '/usr/bin/microsoft-edge', '/usr/bin/microsoft-edge-stable', '/snap/bin/chromium',
    '/opt/google/chrome/chrome', '/opt/brave.com/brave/brave-browser',
  ]);
});

test('Windows browser candidates cover real Chrome, Edge, Chromium, and Brave layouts', () => {
  const paths = browserCandidatePaths({ platform: 'win32', home: 'D:\\Users\\Ada', env: {
    PROGRAMFILES: 'C:\\Program Files', 'ProgramFiles(x86)': 'D:\\Apps (x86)',
    localappdata: 'D:\\Users\\Ada\\AppData\\Local', CHROME_BIN: 'explicit-only.exe', CHROME_PATH: 'also-explicit.exe',
  } });
  assert.deepEqual(paths, [
    'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
    'D:\\Apps (x86)\\Google\\Chrome\\Application\\chrome.exe',
    'D:\\Users\\Ada\\AppData\\Local\\Google\\Chrome\\Application\\chrome.exe',
    'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
    'D:\\Apps (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
    'D:\\Users\\Ada\\AppData\\Local\\Microsoft\\Edge\\Application\\msedge.exe',
    'C:\\Program Files\\Chromium\\Application\\chrome.exe',
    'D:\\Apps (x86)\\Chromium\\Application\\chrome.exe',
    'D:\\Users\\Ada\\AppData\\Local\\Chromium\\Application\\chrome.exe',
    'C:\\Program Files\\BraveSoftware\\Brave-Browser\\Application\\brave.exe',
    'D:\\Apps (x86)\\BraveSoftware\\Brave-Browser\\Application\\brave.exe',
    'D:\\Users\\Ada\\AppData\\Local\\BraveSoftware\\Brave-Browser\\Application\\brave.exe',
  ]);
});

test('Windows browser paths have sensible defaults without optional installation variables', () => {
  const paths = browserCandidatePaths({ platform: 'win32', home: 'C:\\Users\\Ada', env: {} });
  assert.ok(paths.includes('C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe'));
  assert.ok(paths.includes('C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe'));
  assert.ok(paths.includes('C:\\Users\\Ada\\AppData\\Local\\Chromium\\Application\\chrome.exe'));
  assert.equal(new Set(paths).size, paths.length);
});

test('macOS candidates use executable paths inside system and user app bundles', () => {
  assert.deepEqual(browserCandidatePaths({ platform: 'darwin', env: {}, home: '/Users/Ada' }), [
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Chromium.app/Contents/MacOS/Chromium',
    '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
    '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
    '/Users/Ada/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Users/Ada/Applications/Chromium.app/Contents/MacOS/Chromium',
    '/Users/Ada/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
    '/Users/Ada/Applications/Brave Browser.app/Contents/MacOS/Brave Browser',
  ]);
  assert.deepEqual(browserCandidatePaths({ platform: 'unsupported', env: {}, home: '/home/test' }), []);
});
