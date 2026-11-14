// @ts-check
import { accessSync, constants, statSync } from 'node:fs';
import { homedir } from 'node:os';
import { posix, win32 } from 'node:path';

/**
 * @typedef {object} InvocationOptions
 * @property {string} [platform]
 * @property {Record<string, string | undefined>} [env]
 * @property {string} [execPath]
 * @property {(candidate: string) => boolean} [isFile]
 * @property {string[]} [cliCandidates] Additional npm CLI JavaScript locations.
 */

/**
 * Preserve POSIX names; on Windows the last spelling/value overrides any alias.
 * @param {Record<string, string | undefined>} env
 * @param {string} [platform]
 * @returns {Record<string, string | undefined>}
 */
export function normalizeEnvironment(env, platform = process.platform) {
  if (platform !== 'win32') return { ...env };
  /** @type {Map<string, [string, string|undefined]>} */
  const names = new Map();
  for (const [name, value] of Object.entries(env)) names.set(name.toLowerCase(), [name, value]);
  return Object.fromEntries(names.values());
}

/** @param {Record<string, string | undefined>} env @param {string} name @param {string} platform */
function envValue(env, name, platform) {
  if (platform !== 'win32') return env[name];
  const key = Object.keys(env).findLast(candidate => candidate.toLowerCase() === name.toLowerCase());
  return key === undefined ? undefined : env[key];
}

/** @param {string} candidate */
function isReadableFile(candidate) {
  try {
    accessSync(candidate, constants.R_OK);
    return statSync(candidate).isFile();
  } catch {
    return false;
  }
}

/** @param {'npm'|'npx'} name @param {string} command @param {InvocationOptions} options */
function npmCliCandidates(name, command, options) {
  const platform = options.platform ?? process.platform;
  const paths = platform === 'win32' ? win32 : posix;
  const env = options.env ?? process.env;
  const nodeDir = paths.dirname(options.execPath ?? process.execPath);
  const cli = `${name}-cli.js`;
  const hint = envValue(env, 'npm_execpath', platform);
  const hinted = hint && /^(?:npm|npx)-cli\.js$/i.test(paths.basename(hint))
    ? [paths.join(paths.dirname(hint), cli)] : [];
  const standard = [paths.join(nodeDir, 'node_modules', 'npm', 'bin', cli)];
  if (platform === 'win32') {
    const appData = envValue(env, 'APPDATA', platform);
    if (appData) standard.push(paths.join(appData, 'npm', 'node_modules', 'npm', 'bin', cli));
  } else {
    standard.push(
      paths.join(nodeDir, '..', 'lib', 'node_modules', 'npm', 'bin', cli),
      paths.join(nodeDir, '..', 'share', 'nodejs', 'npm', 'bin', cli),
      paths.join(nodeDir, '..', 'share', 'npm', 'bin', cli),
    );
  }
  if (paths.dirname(command) !== '.') {
    standard.push(paths.join(paths.dirname(command), 'node_modules', 'npm', 'bin', cli));
  }
  return [...new Set([...hinted, ...(options.cliCandidates ?? []), ...standard])];
}

/**
 * Resolve npm/npx to JavaScript CLI entry points so spawn does not need a shell
 * or a Windows .cmd wrapper. Every argument retains its original boundary.
 * @param {string} command
 * @param {readonly string[]} args
 * @param {InvocationOptions} [options]
 * @returns {{ command: string, args: string[] }}
 */
export function commandInvocation(command, args, options = {}) {
  const platform = options.platform ?? process.platform;
  const paths = platform === 'win32' ? win32 : posix;
  const basename = paths.basename(command);
  const name = platform === 'win32' ? basename.toLowerCase().replace(/\.cmd$/, '') : basename;
  if (name !== 'npm' && name !== 'npx') return { command, args: [...args] };
  const isFile = options.isFile ?? isReadableFile;
  const cli = npmCliCandidates(name, command, options).find(candidate =>
    /\.(?:c|m)?js$/i.test(candidate) && isFile(candidate));
  if (!cli) throw new Error(`${name} CLI JavaScript not found; install npm with Node or set npm_execpath`);
  return { command: options.execPath ?? process.execPath, args: [cli, ...args] };
}

/** @param {Record<string, string | undefined>} env @param {string} home */
function windowsBrowserPaths(env, home) {
  const drive = envValue(env, 'SystemDrive', 'win32') || 'C:';
  const roots = [...new Set([
    envValue(env, 'ProgramFiles', 'win32') || win32.join(`${drive}\\`, 'Program Files'),
    envValue(env, 'ProgramFiles(x86)', 'win32') || win32.join(`${drive}\\`, 'Program Files (x86)'),
    envValue(env, 'LOCALAPPDATA', 'win32') || (home ? win32.join(home, 'AppData', 'Local') : ''),
  ].filter(Boolean))];
  const applications = [
    ['Google', 'Chrome', 'Application', 'chrome.exe'],
    ['Microsoft', 'Edge', 'Application', 'msedge.exe'],
    ['Chromium', 'Application', 'chrome.exe'],
    ['BraveSoftware', 'Brave-Browser', 'Application', 'brave.exe'],
  ];
  return applications.flatMap(parts => roots.map(root => win32.join(root, ...parts)));
}

/** @param {string} home */
function macBrowserPaths(home) {
  const roots = ['/Applications', ...(home ? [posix.join(home, 'Applications')] : [])];
  const applications = [
    ['Google Chrome.app', 'Google Chrome'], ['Chromium.app', 'Chromium'],
    ['Microsoft Edge.app', 'Microsoft Edge'], ['Brave Browser.app', 'Brave Browser'],
  ];
  return roots.flatMap(root => applications.map(([bundle, executable]) =>
    posix.join(root, bundle ?? '', 'Contents', 'MacOS', executable ?? '')));
}

/**
 * Installed-browser locations only; explicit CHROME_BIN/CHROME_PATH overrides
 * remain the responsibility of the caller, before these fallback candidates.
 * @param {{ platform?: string, env?: Record<string, string | undefined>, home?: string }} [options]
 * @returns {string[]}
 */
export function browserCandidatePaths(options = {}) {
  const platform = options.platform ?? process.platform;
  const env = options.env ?? process.env;
  const home = options.home ?? envValue(env, platform === 'win32' ? 'USERPROFILE' : 'HOME', platform) ?? homedir();
  if (platform === 'win32') return windowsBrowserPaths(env, home);
  if (platform === 'darwin') return [...new Set(macBrowserPaths(home))];
  if (platform !== 'linux') return [];
  return [
    ...(home ? [posix.join(home, '.local', 'bin', 'chromium')] : []),
    '/usr/bin/chromium', '/usr/bin/chromium-browser', '/usr/bin/google-chrome',
    '/usr/bin/google-chrome-stable', '/usr/bin/brave-browser', '/usr/bin/brave',
    '/usr/bin/microsoft-edge', '/usr/bin/microsoft-edge-stable', '/snap/bin/chromium',
    '/opt/google/chrome/chrome', '/opt/brave.com/brave/brave-browser',
  ];
}
