#!/usr/bin/env node
// @ts-nocheck
/**
 * monolith-inspector.mjs — Monolith Decomposition & Parity Tracker
 *
 * Tracks extraction progress of functions from:
 *   - rust/gui-spike/src/data.rs (Rust backend monolith)
 *   - rust/gui-spike/ui/app.js (JS frontend monolith)
 *
 * Usage:
 *   node scripts/monolith-inspector.mjs
 *   node scripts/monolith-inspector.mjs --domain voice
 *   node scripts/monolith-inspector.mjs --json
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const DATA_RS = path.join(PROJECT_ROOT, 'rust/gui-spike/src/data.rs');
const APP_JS = path.join(PROJECT_ROOT, 'rust/gui-spike/ui/app.js');

const DOMAIN_KEYWORDS = {
  voice: ['voice', 'rtc', 'audio', 'mic', 'stream', 'speaking', 'sdp', 'codec', 'dave', 'media'],
  gateway: ['gateway', 'dispatch', 'op_', 'identify', 'heartbeat', 'resume', 'ingest', 'session'],
  guilds: ['guild', 'channel', 'role', 'member', 'category', 'invite', 'permission'],
  chat: ['message', 'reaction', 'pin', 'thread', 'content', 'send_', 'edit_', 'delete_'],
  search: ['search', 'fts', 'query', 'filter', 'index'],
  auth_settings: ['auth', 'login', 'token', 'mfa', 'settings', 'config', 'preference'],
};

function analyzeRustData(filePath) {
  if (!fs.existsSync(filePath)) return { lines: 0, functions: [] };
  const content = fs.readFileSync(filePath, 'utf8');
  const lines = content.split('\n');

  const fnRegex = /^\s*(pub(?:\s*\([^)]+\))?\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)\s*\(/;
  const functions = [];

  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    const match = l.match(fnRegex);
    if (match) {
      const isPub = Boolean(match[1]);
      const name = match[2];

      let domain = 'other';
      const lower = name.toLowerCase();
      for (const [dom, kwList] of Object.entries(DOMAIN_KEYWORDS)) {
        if (kwList.some((kw) => lower.includes(kw))) {
          domain = dom;
          break;
        }
      }

      functions.push({
        name,
        isPub,
        line: i + 1,
        domain,
      });
    }
  }

  return { lines: lines.length, functions };
}

function analyzeJsApp(filePath) {
  if (!fs.existsSync(filePath)) return { lines: 0, functions: [] };
  const content = fs.readFileSync(filePath, 'utf8');
  const lines = content.split('\n');

  const fnRegex = /^\s*(?:export\s+)?(?:async\s+)?function\s+([a-zA-Z0-9_]+)\s*\(/;
  const constFnRegex = /^\s*(?:const|let|var)\s+([a-zA-Z0-9_]+)\s*=\s*(?:async\s*)?\([^)]*\)\s*=>/;
  const functions = [];

  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    let match = l.match(fnRegex) || l.match(constFnRegex);
    if (match) {
      const name = match[1];
      let domain = 'other';
      const lower = name.toLowerCase();
      for (const [dom, kwList] of Object.entries(DOMAIN_KEYWORDS)) {
        if (kwList.some((kw) => lower.includes(kw))) {
          domain = dom;
          break;
        }
      }

      functions.push({
        name,
        line: i + 1,
        domain,
      });
    }
  }

  return { lines: lines.length, functions };
}

export function inspectMonolith(targetDomain = null) {
  const rust = analyzeRustData(DATA_RS);
  const js = analyzeJsApp(APP_JS);

  const rustByDomain = {};
  for (const f of rust.functions) {
    rustByDomain[f.domain] = (rustByDomain[f.domain] || 0) + 1;
  }

  const jsByDomain = {};
  for (const f of js.functions) {
    jsByDomain[f.domain] = (jsByDomain[f.domain] || 0) + 1;
  }

  return {
    rust: {
      file: 'rust/gui-spike/src/data.rs',
      lines: rust.lines,
      totalFunctions: rust.functions.length,
      pubFunctions: rust.functions.filter((f) => f.isPub).length,
      privFunctions: rust.functions.filter((f) => !f.isPub).length,
      byDomain: rustByDomain,
      functions: targetDomain ? rust.functions.filter((f) => f.domain === targetDomain) : rust.functions,
    },
    js: {
      file: 'rust/gui-spike/ui/app.js',
      lines: js.lines,
      totalFunctions: js.functions.length,
      byDomain: jsByDomain,
      functions: targetDomain ? js.functions.filter((f) => f.domain === targetDomain) : js.functions,
    },
  };
}

function main() {
  const args = process.argv.slice(2);
  const domain = args.find((a, i) => args[i - 1] === '--domain');
  const jsonMode = args.includes('--json');

  const report = inspectMonolith(domain);

  if (jsonMode) {
    console.log(JSON.stringify(report, null, 2));
    return;
  }

  console.log(`\n================================================================`);
  console.log(`🏛️  MONOLITH DECOMPOSITION & FUNCTION INVENTORY`);
  console.log(`================================================================`);
  console.log(`🦀 Rust Monolith (${report.rust.file}):`);
  console.log(`   Lines: ${report.rust.lines.toLocaleString()} lines`);
  console.log(`   Total Functions: ${report.rust.totalFunctions} (Pub: ${report.rust.pubFunctions}, Priv: ${report.rust.privFunctions})`);
  console.log(`   Domain breakdown:`);
  for (const [dom, cnt] of Object.entries(report.rust.byDomain)) {
    console.log(`     • ${dom.padEnd(14)}: ${cnt} functions`);
  }

  console.log(`\n🌐 Frontend Monolith (${report.js.file}):`);
  console.log(`   Lines: ${report.js.lines.toLocaleString()} lines`);
  console.log(`   Total Functions: ${report.js.totalFunctions}`);
  console.log(`   Domain breakdown:`);
  for (const [dom, cnt] of Object.entries(report.js.byDomain)) {
    console.log(`     • ${dom.padEnd(14)}: ${cnt} functions`);
  }

  if (domain) {
    console.log(`\nFiltered Functions for domain "${domain}":`);
    console.log(`--- Rust (${report.rust.functions.length} functions):`);
    for (const f of report.rust.functions) {
      console.log(`  L${f.line.toString().padEnd(5)} | ${f.isPub ? 'pub  ' : 'priv '} | ${f.name}`);
    }
  }

  console.log(`================================================================\n`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main();
}
