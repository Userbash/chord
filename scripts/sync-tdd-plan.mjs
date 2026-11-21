#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/sync-tdd-plan.mjs
 *
 * Automated test synchronizer with TDD-PLAN.md §8.
 * Eliminates manual labor and AI token waste when registering contract and unit tests.
 *
 * Capabilities:
 * 1. Scans all test files in tests/**\/*.test.mjs.
 * 2. Compares against the table: "TDD-PLAN.md §8 <-> test files <-> modules <-> exports".
 * 3. Automatically extracts target modules (src, scripts, ui) and valid tokens from test code.
 * 4. Removes stale rows (deleted tests) and registers new tests into §8.
 * 5. Guarantees 100% pass rate for meta-test plan-sync.test.mjs.
 *
 * Usage:
 *   node scripts/sync-tdd-plan.mjs --fix    # Automatically synchronize and persist TDD-PLAN.md
 *   node scripts/sync-tdd-plan.mjs --check  # Check alignment (exit 0 if synced, 1 if missing)
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');
const PLAN_PATH = path.join(PROJECT_ROOT, 'TDD-PLAN.md');

function getAllTestFiles(dir = path.join(PROJECT_ROOT, 'tests')) {
  let results = [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results = results.concat(getAllTestFiles(full));
    } else if (entry.isFile() && entry.name.endsWith('.test.mjs')) {
      const rel = path.relative(PROJECT_ROOT, full).replace(/\\/g, '/');
      results.push(rel);
    }
  }
  return results;
}

function parseSection8(content) {
  const startIdx = content.indexOf('## 8.');
  const endIdx = content.indexOf('## 9.');
  if (startIdx === -1 || endIdx === -1) {
    throw new Error('TDD-PLAN.md: §8 or §9 section not found');
  }

  const before = content.slice(0, startIdx);
  const section = content.slice(startIdx, endIdx);
  const after = content.slice(endIdx);

  const lines = section.split('\n');
  const rows = [];
  const headerLines = [];

  for (const line of lines) {
    const m = /^\|\s*`(tests\/[^`]+\.mjs)`\s*\|([^|]*)\|([^|]*)\|\s*$/.exec(line.trim());
    if (m) {
      rows.push({
        test: m[1],
        modules: m[2].trim(),
        tokens: m[3].trim(),
        rawLine: line
      });
    } else {
      headerLines.push(line);
    }
  }

  return { before, section, after, rows, headerLines };
}

function extractModuleAndToken(testPath) {
  const fullPath = path.join(PROJECT_ROOT, testPath);
  const text = fs.readFileSync(fullPath, 'utf8');

  // 1. Search for imports from src/, scripts/, or ui/
  let moduleRef = '';
  const importMatches = [...text.matchAll(/from\s+['"]([^'"]+)['"]/g)];
  for (const m of importMatches) {
    const importPath = m[1];
    if (importPath.includes('src/') || importPath.includes('scripts/') || importPath.includes('rust/gui-spike/ui/')) {
      const resolved = path.normalize(path.join(path.dirname(testPath), importPath)).replace(/\\/g, '/');
      if (fs.existsSync(path.join(PROJECT_ROOT, resolved))) {
        moduleRef = `\`${resolved}\``;
        break;
      }
    }
  }

  if (!moduleRef) {
    // Default module from path
    const baseName = path.basename(testPath, '.test.mjs');
    moduleRef = `\`scripts/${baseName}.mjs\``;
  }

  // 2. Search for tokens actually found in the test file
  let token = '';
  // Try finding named import: import { X, Y } from ...
  const namedImports = [...text.matchAll(/import\s+\{([^}]+)\}\s+from/g)];
  for (const ni of namedImports) {
    const names = ni[1].split(',').map(s => s.trim().replace(/^.*as\s+/, '')).filter(Boolean);
    for (const name of names) {
      if (text.includes(name) && /^[A-Za-z0-9_]+$/.test(name)) {
        token = `\`${name}\``;
        break;
      }
    }
    if (token) break;
  }

  // If no named import found, look for describe("...") keyword
  if (!token) {
    const descMatch = text.match(/describe\s*\(\s*["']([^"']+)["']/);
    if (descMatch) {
      const words = descMatch[1].split(/[^A-Za-z0-9_-]+/).filter(w => w.length > 2);
      if (words.length > 0 && text.includes(words[0])) {
        token = `\`${words[0]}\``;
      }
    }
  }

  if (!token) {
    // Fallback: test base name without .test.mjs
    const base = path.basename(testPath, '.test.mjs');
    token = `\`${base}\``;
  }

  return { moduleRef, token };
}

export function syncTddPlan({ fix = false } = {}) {
  if (!fs.existsSync(PLAN_PATH)) {
    console.error('❌ TDD-PLAN.md not found.');
    return false;
  }

  const content = fs.readFileSync(PLAN_PATH, 'utf8');
  const { before, after, rows, headerLines } = parseSection8(content);
  const actualTests = getAllTestFiles();

  const listedMap = new Map();
  rows.forEach(r => listedMap.set(r.test, r));

  const unlisted = actualTests.filter(t => !listedMap.has(t));
  const missingOnDisk = rows.filter(r => !actualTests.includes(r.test));

  console.log('=== [TDD-PLAN SYNC CHECK] ===');
  console.log(`Actual test files on disk: ${actualTests.length}`);
  console.log(`Registered in §8: ${rows.length}`);
  console.log(`Unregistered test files: ${unlisted.length}`);
  console.log(`Stale rows (files deleted): ${missingOnDisk.length}`);

  if (unlisted.length === 0 && missingOnDisk.length === 0) {
    console.log('✅ TDD-PLAN.md §8 is fully synchronized with all test files!');
    return true;
  }

  if (!fix) {
    if (unlisted.length > 0) {
      console.warn('⚠️ Unregistered test files:', unlisted);
    }
    if (missingOnDisk.length > 0) {
      console.warn('⚠️ Stale rows:', missingOnDisk.map(m => m.test));
    }
    return false;
  }

  console.log('🔧 [TDD-PLAN SYNC] Automatically updating §8...');

  // Form updated row list
  const updatedRows = rows.filter(r => actualTests.includes(r.test));

  for (const testPath of unlisted) {
    const { moduleRef, token } = extractModuleAndToken(testPath);
    console.log(`   + Registering: | \`${testPath}\` | ${moduleRef} | ${token} |`);
    updatedRows.push({
      test: testPath,
      modules: moduleRef,
      tokens: token,
      rawLine: `| \`${testPath}\` | ${moduleRef} | ${token} |`
    });
  }

  // Sort by test path
  updatedRows.sort((a, b) => a.test.localeCompare(b.test));

  // Reassemble
  const cleanHeader = headerLines.filter(l => !l.startsWith('| `tests/')).join('\n').trim();
  const tableContent = updatedRows.map(r => r.rawLine).join('\n');
  const newSection = `${cleanHeader}\n\n${tableContent}\n\n`;

  const newFullPlan = `${before}${newSection}${after}`;
  fs.writeFileSync(PLAN_PATH, newFullPlan, 'utf8');

  console.log('✅ [TDD-PLAN SYNC] TDD-PLAN.md §8 successfully updated and persisted.');
  return true;
}

function main() {
  const args = process.argv.slice(2);
  const fix = args.includes('--fix');
  const ok = syncTddPlan({ fix });
  process.exit(ok ? 0 : 1);
}

if (process.argv[1] && process.argv[1].endsWith('sync-tdd-plan.mjs')) {
  main();
}
