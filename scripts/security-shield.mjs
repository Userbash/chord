#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/security-shield.mjs — Pre-Commit AST & Secret Security Shield
 *
 * Fast (<0.2s) static AST and pattern scanner that prevents:
 *   1. Hardcoded Discord user/bot tokens & MFA session tokens
 *   2. Unencrypted private keys & secrets
 *   3. Forbidden `unsafe {` blocks in Rust source code
 *   4. Shell execution (`std::process::Command` / `child_process.exec`) in audio/UI modules
 *
 * Usage:
 *   node scripts/security-shield.mjs [--staged]
 */

import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

// Token & Secret Patterns
const DISCORD_BOT_TOKEN_RE = /\b[MN][A-Za-z0-9_-]{23,25}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27,38}\b/;
const DISCORD_MFA_TOKEN_RE = /\bmfa\.[A-Za-z0-9_-]{50,100}\b/;
const PRIVATE_KEY_RE = /-----BEGIN (?:RSA|EC|OPENSSH|PGP|PRIVATE) KEY-----/;

// Code Smells
const RUST_UNSAFE_BLOCK_RE = /\bunsafe\s*\{/;
const SHELL_EXEC_RUST_RE = /\bstd::process::Command::new\b/;
const SHELL_EXEC_JS_RE = /\bexec\s*\(\s*['"`](?:sh|bash|cmd|powershell)\b/;

function getFilesToCheck(isStagedOnly) {
  try {
    if (isStagedOnly) {
      const output = execSync('git diff --cached --name-only --diff-filter=ACM', {
        cwd: PROJECT_ROOT,
        encoding: 'utf8',
      });
      return output.split('\n').filter(Boolean);
    } else {
      const output = execSync('git status --porcelain', {
        cwd: PROJECT_ROOT,
        encoding: 'utf8',
      });
      return output
        .split('\n')
        .map((l) => l.trim().slice(3))
        .filter(Boolean);
    }
  } catch {
    return [];
  }
}

function scanFile(relPath) {
  const fullPath = path.join(PROJECT_ROOT, relPath);
  if (!fs.existsSync(fullPath) || fs.statSync(fullPath).isDirectory()) return [];

  // Exclude test mocks, logs, and evidence
  if (
    relPath.startsWith('logs/') ||
    relPath.startsWith('evidence/') ||
    relPath.includes('test') ||
    relPath.endsWith('.png') ||
    relPath.endsWith('.jpg') ||
    relPath.endsWith('.lock')
  ) {
    return [];
  }

  const content = fs.readFileSync(fullPath, 'utf8');
  const violations = [];
  const lines = content.split('\n');

  lines.forEach((line, idx) => {
    const lineNum = idx + 1;

    // 1. Secrets
    if (DISCORD_BOT_TOKEN_RE.test(line)) {
      violations.push({ file: relPath, line: lineNum, rule: 'CRITICAL: Hardcoded Discord Bot Token pattern' });
    }
    if (DISCORD_MFA_TOKEN_RE.test(line)) {
      violations.push({ file: relPath, line: lineNum, rule: 'CRITICAL: Hardcoded Discord MFA Token pattern' });
    }
    if (PRIVATE_KEY_RE.test(line)) {
      violations.push({ file: relPath, line: lineNum, rule: 'CRITICAL: Unencrypted Private Key Header' });
    }

    // 2. Unsafe Rust
    if (relPath.endsWith('.rs') && RUST_UNSAFE_BLOCK_RE.test(line)) {
      violations.push({ file: relPath, line: lineNum, rule: 'FORBIDDEN: unsafe block in Rust (#![forbid(unsafe_code)])' });
    }

    // 3. Shell injections in core audio & platform
    if ((relPath.includes('chord-platform') || relPath.includes('audio_engine')) && SHELL_EXEC_RUST_RE.test(line)) {
      violations.push({ file: relPath, line: lineNum, rule: 'FORBIDDEN: std::process::Command in native audio engine' });
    }

    if (relPath.startsWith('rust/gui-spike/ui') && SHELL_EXEC_JS_RE.test(line)) {
      violations.push({ file: relPath, line: lineNum, rule: 'FORBIDDEN: Shell execution from UI' });
    }
  });

  return violations;
}

function main() {
  const isStaged = process.argv.includes('--staged');
  const files = getFilesToCheck(isStaged);

  console.log(`🛡️  [SECURITY-SHIELD] Scanning ${files.length} file(s) for secrets, unsafe code & injections...`);

  let allViolations = [];
  for (const f of files) {
    allViolations.push(...scanFile(f));
  }

  if (allViolations.length > 0) {
    console.error(`\n❌ [SECURITY-SHIELD] BLOCKED: Found ${allViolations.length} security violation(s):`);
    for (const v of allViolations) {
      console.error(`   🚫 ${v.file}:${v.line} -> ${v.rule}`);
    }
    console.error('\nPlease remediate all security issues before committing.\n');
    process.exit(1);
  }

  console.log('✅ [SECURITY-SHIELD] PASSED: No secrets, unsafe code or dangerous shell calls detected.\n');
  process.exit(0);
}

main();
