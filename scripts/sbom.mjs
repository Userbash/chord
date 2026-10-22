#!/usr/bin/env node
// @ts-check

import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const allowlist = new Set([
  'MIT', 'Apache-2.0', 'BSD-2-Clause', 'BSD-3-Clause', 'ISC', 'Unicode-3.0',
  'Zlib', 'MPL-2.0', 'CC0-1.0', 'Unlicense', '0BSD', 'CDLA-Permissive-2.0',
]);
const exceptions = new Set(['LLVM-exception']);

/**
 * Exact licenses for platform crates that are in Cargo.lock but are not extracted
 * into the local registry cache on this host. Versions are pinned by Cargo.lock.
 * @type {Map<string,string>}
 */
const knownLicenses = new Map([
  ['r-efi@6.0.0', 'MIT OR Apache-2.0 OR LGPL-2.1-or-later'],
  ['wasi@0.11.1+wasi-snapshot-preview1', 'Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT'],
  ['windows-link@0.2.1', 'MIT OR Apache-2.0'],
  ['windows-sys@0.52.0', 'MIT OR Apache-2.0'],
  ['windows-sys@0.61.2', 'MIT OR Apache-2.0'],
  ['windows-targets@0.52.6', 'MIT OR Apache-2.0'],
  ['windows_aarch64_gnullvm@0.52.6', 'MIT OR Apache-2.0'],
  ['windows_aarch64_msvc@0.52.6', 'MIT OR Apache-2.0'],
  ['windows_i686_gnu@0.52.6', 'MIT OR Apache-2.0'],
  ['windows_i686_gnullvm@0.52.6', 'MIT OR Apache-2.0'],
  ['windows_i686_msvc@0.52.6', 'MIT OR Apache-2.0'],
  ['windows_x86_64_gnu@0.52.6', 'MIT OR Apache-2.0'],
  ['windows_x86_64_gnullvm@0.52.6', 'MIT OR Apache-2.0'],
  ['windows_x86_64_msvc@0.52.6', 'MIT OR Apache-2.0'],
]);

/** @typedef {{name:string,version:string,source?:string,dependencies:string[]}} LockPackage */
/** @typedef {{id:string,name:string,version:string,license?:string,source?:string}} MetaPackage */

/** @param {string} text */
function hostTarget(text) {
  const match = text.match(/^host:\s*(\S+)$/m);
  if (!match?.[1]) throw new Error('rustc -vV: не найден host target');
  return match[1];
}

/** Host triple without requiring a child rustc in restricted sandboxes.
 * @returns {string} */
function detectTarget() {
  if (process.env.SBOM_TARGET) return process.env.SBOM_TARGET;
  try {
    const output = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
    if (typeof output !== 'string') throw new Error('rustc -vV: пустой вывод');
    return hostTarget(output);
  } catch {
    /** @type {Record<string, Record<string, string>>} */
    const fallback = {
      linux: { x64: 'x86_64-unknown-linux-gnu', arm64: 'aarch64-unknown-linux-gnu' },
      darwin: { x64: 'x86_64-apple-darwin', arm64: 'aarch64-apple-darwin' },
      win32: { x64: 'x86_64-pc-windows-msvc', arm64: 'aarch64-pc-windows-msvc' },
    };
    const target = fallback[process.platform]?.[process.arch];
    if (!target) throw new Error(`не удалось определить host target: ${process.platform}/${process.arch}`);
    return target;
  }
}

/**
 * Parse the package records we need from Cargo.lock without a TOML dependency.
 * @param {string} text
 * @returns {LockPackage[]}
 */
function parseLock(text) {
  /** @type {LockPackage[]} */
  const packages = [];
  for (const block of text.split(/^\[\[package\]\]\s*$/m).slice(1)) {
    /** @param {string} key */
    const scalar = (key) => block.match(new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, 'm'))?.[1];
    const name = scalar('name');
    const version = scalar('version');
    if (!name || !version) throw new Error('Cargo.lock: package без name/version');
    const dependencyBlock = block.match(/^dependencies\s*=\s*\[([\s\S]*?)^\]/m)?.[1] ?? '';
    const dependencies = [...dependencyBlock.matchAll(/"([^"]+)"/g)].map((match) => match[1] ?? '');
    packages.push({ name, version, source: scalar('source'), dependencies });
  }
  if (!packages.length) throw new Error('Cargo.lock: не найдено ни одного package');
  return packages;
}

/** @returns {{packages: MetaPackage[], workspace: string[]}} */
function loadMetadata() {
  const target = detectTarget();
  /** @type {string[]} */
  const args = [
    'metadata', '--offline', '--format-version', '1', '--filter-platform', target,
    '--manifest-path', join(root, 'rust', 'Cargo.toml'),
  ];
  /** @type {string} */
  let text;
  try {
    text = execFileSync('cargo', args, {
      cwd: root, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024,
    });
  } catch (error) {
    // Some sandboxes report EPERM after Cargo has exited 0 and emitted complete JSON.
    if (!(error && typeof error === 'object' && 'status' in error && error.status === 0 && 'stdout' in error)) {
      throw error;
    }
    text = String(error.stdout);
  }
  const parsed = JSON.parse(text);
  return { packages: parsed.packages, workspace: parsed.workspace_members };
}

/** @param {string} name @param {string} version */
function spdxId(name, version) {
  return `SPDXRef-Package-${name}-${version}`.replace(/[^A-Za-z0-9.-]/g, '-');
}

/**
 * Tokenize an SPDX expression without silently dropping invalid characters.
 * @param {string} expression
 * @returns {string[]|null}
 */
function tokenize(expression) {
  const tokens = [];
  let index = 0;
  while (index < expression.length) {
    const char = expression[index];
    if (char === undefined) break;
    if (/\s/.test(char)) {
      index += 1;
      continue;
    }
    if (char === '(' || char === ')') {
      tokens.push(char);
      index += 1;
      continue;
    }
    const match = expression.slice(index).match(/^[A-Za-z0-9.+-]+/);
    if (!match) return null;
    tokens.push(match[0]);
    index += match[0].length;
  }
  return tokens;
}

/**
 * Strict SPDX expression parser: OR needs one allowed branch, AND needs all,
 * WITH needs an allowed base license and an allowed exception.
 * @param {string} expression
 */
function allowedLicense(expression) {
  const tokens = tokenize(expression);
  if (tokens === null) return false;
  let at = 0;
  const peek = () => tokens[at];
  const take = () => tokens[at++];
  /** @returns {boolean|null} null means syntax error, not just a denied license. */
  const allowedAtom = () => {
    const id = take();
    if (!id || id === '(' || id === ')' || /^(AND|OR|WITH)$/i.test(id)) return null;
    if (peek()?.toUpperCase() === 'WITH') {
      take();
      const exception = take();
      if (!exception || exception === '(' || exception === ')' || /^(AND|OR|WITH)$/i.test(exception)) {
        return null;
      }
      return allowlist.has(id) && exceptions.has(exception);
    }
    return allowlist.has(id);
  };
  /** @returns {boolean|null} */
  const allowedFactor = () => {
    if (peek() === '(') {
      take();
      const value = allowedOr();
      if (value === null || take() !== ')') return null;
      return value;
    }
    return allowedAtom();
  };
  /** @returns {boolean|null} */
  const allowedAnd = () => {
    let value = allowedFactor();
    if (value === null) return null;
    while (peek()?.toUpperCase() === 'AND') {
      take();
      const right = allowedFactor();
      if (right === null) return null;
      value = value && right;
    }
    return value;
  };
  /** @returns {boolean|null} */
  const allowedOr = () => {
    let value = allowedAnd();
    if (value === null) return null;
    while (peek()?.toUpperCase() === 'OR') {
      take();
      const right = allowedAnd();
      if (right === null) return null;
      value = value || right;
    }
    return value;
  };
  return allowedOr() === true && at === tokens.length;
}

/**
 * Normalizes the legacy Cargo slash-form license ("MIT/Apache-2.0") into SPDX form
 * ("MIT OR Apache-2.0"). The slash meant "or"; SPDX license ids never contain a slash.
 * @param {string} expression
 * @returns {string}
 */
function normalizeLicense(expression) {
  if (!expression.includes('/')) return expression;
  if (/\b(OR|AND|WITH)\b/i.test(expression)) return expression;
  return expression
    .split('/')
    .map((part) => part.trim())
    .filter(Boolean)
    .join(' OR ');
}

/**
 * @param {string} lockText
 * @param {LockPackage[]} lockPackages
 * @param {ReturnType<typeof loadMetadata>} metadata
 */
function buildDocument(lockText, lockPackages, metadata) {
  const byId = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
  const metadataByKey = new Map(metadata.packages.map((pkg) => [`${pkg.name}@${pkg.version}`, pkg]));
  const workspacePackages = metadata.workspace.flatMap((id) => {
    const pkg = byId.get(id);
    return pkg ? [pkg] : [];
  });
  const workspaceKeys = new Set(workspacePackages.map((pkg) => `${pkg.name}@${pkg.version}`));
  /** @type {Map<string, LockPackage[]>} */
  const byName = new Map();
  for (const pkg of lockPackages) {
    const list = byName.get(pkg.name) ?? [];
    list.push(pkg);
    byName.set(pkg.name, list);
  }

  const packages = lockPackages
    .slice()
    .sort((left, right) => `${left.name}@${left.version}`.localeCompare(`${right.name}@${right.version}`))
    .map((pkg) => {
      const key = `${pkg.name}@${pkg.version}`;
      const meta = metadataByKey.get(key);
      const license = normalizeLicense(meta?.license || knownLicenses.get(key) || 'NOASSERTION');
      return {
        name: pkg.name,
        SPDXID: spdxId(pkg.name, pkg.version),
        versionInfo: pkg.version,
        downloadLocation: pkg.source?.startsWith('registry+')
          ? `https://crates.io/api/v1/crates/${encodeURIComponent(pkg.name)}/${encodeURIComponent(pkg.version)}/download`
          : 'NOASSERTION',
        filesAnalyzed: false,
        licenseDeclared: license,
        licenseConcluded: 'NOASSERTION',
        copyrightText: 'NOASSERTION',
        externalRefs: [{
          referenceCategory: 'PACKAGE-MANAGER',
          referenceType: 'purl',
          referenceLocator: `pkg:cargo/${encodeURIComponent(pkg.name)}@${encodeURIComponent(pkg.version)}`,
        }],
      };
    });

  /** @type {Array<{spdxElementId:string,relationshipType:string,relatedSpdxElement:string}>} */
  const relationships = [];
  /** @type {Set<string>} */
  const seen = new Set();
  /** @param {string} from @param {string} type @param {string} to */
  const addRelationship = (from, type, to) => {
    const key = `${from}|${type}|${to}`;
    if (seen.has(key)) return;
    seen.add(key);
    relationships.push({ spdxElementId: from, relationshipType: type, relatedSpdxElement: to });
  };
  for (const key of workspaceKeys) {
    const pkg = metadataByKey.get(key);
    if (pkg) addRelationship('SPDXRef-DOCUMENT', 'DESCRIBES', spdxId(pkg.name, pkg.version));
  }
  for (const pkg of lockPackages) {
    const from = spdxId(pkg.name, pkg.version);
    for (const dependency of pkg.dependencies) {
      const [name = '', version] = dependency.split(/\s+/);
      if (!name) throw new Error(`Cargo.lock: пустая зависимость ${dependency}`);
      const targets = (byName.get(name) ?? []).filter((target) => !version || target.version === version);
      if (!targets.length) throw new Error(`Cargo.lock: зависимость ${dependency} не найдена`);
      for (const target of targets) {
        addRelationship(from, 'DEPENDS_ON', spdxId(target.name, target.version));
      }
    }
  }

  const rawEpoch = process.env.SOURCE_DATE_EPOCH ?? '0';
  if (!/^\d+$/.test(rawEpoch)) throw new Error('SOURCE_DATE_EPOCH должен быть неотрицательным целым');
  const epoch = Number.parseInt(rawEpoch, 10);
  if (!Number.isSafeInteger(epoch)) throw new Error('SOURCE_DATE_EPOCH слишком велик');
  return {
    spdxVersion: 'SPDX-2.3',
    dataLicense: 'CC0-1.0',
    SPDXID: 'SPDXRef-DOCUMENT',
    name: 'discord-lab-rust-workspace',
    documentNamespace: `https://discord-lab.local/spdx/${createHash('sha256').update(lockText).digest('hex')}`,
    creationInfo: {
      created: new Date(epoch * 1000).toISOString().replace('.000Z', 'Z'),
      creators: ['Tool: discord-lab-sbom'],
      licenseListVersion: '3.25',
    },
    packages,
    relationships,
  };
}

function main() {
  const args = process.argv.slice(2);
  let output = join(root, 'rust', 'dist', 'sbom.spdx.json');
  let check = false;
  let dryRun = false;
  /** @type {string|null} */
  let checkLicense = null;
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--output') {
      const value = args[i + 1];
      if (!value) throw new Error('--output требует путь');
      output = resolve(value);
      i += 1;
    } else if (arg === '--check') check = true;
    else if (arg === '--dry-run') dryRun = true;
    else if (arg === '--check-license') {
      const value = args[i + 1];
      if (!value) throw new Error('--check-license требует ID');
      checkLicense = value;
      i += 1;
    } else throw new Error(`Неизвестный аргумент: ${arg}`);
  }
  if (checkLicense !== null) {
    if (allowedLicense(checkLicense)) return 0;
    console.error(`sbom: запрещённая лицензия: ${checkLicense}`);
    return 1;
  }

  const lockText = readFileSync(join(root, 'rust', 'Cargo.lock'), 'utf8');
  const lockPackages = parseLock(lockText);
  const document = buildDocument(lockText, lockPackages, loadMetadata());
  if (check) {
    const rejected = document.packages.filter((pkg) => !allowedLicense(pkg.licenseDeclared));
    if (rejected.length) {
      for (const pkg of rejected) console.error(`sbom: ${pkg.name}@${pkg.versionInfo}: ${pkg.licenseDeclared}`);
      return 1;
    }
  }
  if (dryRun) {
    console.log(`sbom: SPDX-2.3, ${document.packages.length} packages, output=${output}`);
    return 0;
  }
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, `${JSON.stringify(document, null, 2)}\n`);
  console.log(`sbom: создан ${output} (${document.packages.length} packages)`);
  return 0;
}

try {
  process.exitCode = main();
} catch (error) {
  console.error(`sbom: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
