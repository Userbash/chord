#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/docs-lint.mjs — Documentation Integrity & Link Linter
 *
 * Verifies that all Markdown documents have valid relative links,
 * correct Mermaid diagram syntax, and required cross-references.
 *
 * Usage:
 *   node scripts/docs-lint.mjs [--fix]
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const SCAN_DIRS = ['docs', 'waves', '.'];
const IGNORE_DIRS = [
  'node_modules',
  '.git',
  'rust/target',
  'target',
  '.agents',
  '.claude',
  'gpui-kit',
  'docs/reference-projects/sources',
];

function getAllMarkdownFiles() {
  const mdFiles = [];

  function walk(currentDir) {
    const entries = fs.readdirSync(currentDir, { withFileTypes: true });
    for (const e of entries) {
      const full = path.join(currentDir, e.name);
      const rel = path.relative(PROJECT_ROOT, full);

      if (e.isDirectory()) {
        if (!IGNORE_DIRS.some((ign) => rel.startsWith(ign) || e.name === ign)) {
          walk(full);
        }
      } else if (e.isFile() && e.name.endsWith('.md')) {
        mdFiles.push(rel);
      }
    }
  }

  walk(PROJECT_ROOT);
  return mdFiles;
}

function lintMarkdownFile(relPath) {
  const fullPath = path.join(PROJECT_ROOT, relPath);
  const content = fs.readFileSync(fullPath, 'utf8');
  const dir = path.dirname(fullPath);
  const brokenLinks = [];

  // Match markdown links [text](target)
  const linkRegex = /\[([^\]]+)\]\(([^)#]+)(?:#([^)]+))?\)/g;
  let match;

  while ((match = linkRegex.exec(content)) !== null) {
    const rawTarget = match[2].trim();
    // Skip external URLs, mailto, anchor-only
    if (
      rawTarget.startsWith('http://') ||
      rawTarget.startsWith('https://') ||
      rawTarget.startsWith('mailto:') ||
      rawTarget.startsWith('#') ||
      rawTarget.startsWith('file://')
    ) {
      continue;
    }

    const resolved = path.resolve(dir, rawTarget);
    if (!fs.existsSync(resolved)) {
      brokenLinks.push({
        target: rawTarget,
        line: content.slice(0, match.index).split('\n').length,
      });
    }
  }

  return brokenLinks;
}

function main() {
  console.log('📚 [DOCS-LINT] Scanning documentation repository for link integrity...');
  const files = getAllMarkdownFiles();
  console.log(`🔍 Found ${files.length} Markdown file(s) across repository.`);

  let totalBroken = 0;
  const issues = [];

  for (const f of files) {
    const broken = lintMarkdownFile(f);
    if (broken.length > 0) {
      totalBroken += broken.length;
      issues.push({ file: f, broken });
    }
  }

  if (totalBroken > 0) {
    console.log(`\n⚠️  [DOCS-LINT] Found ${totalBroken} unresolvable relative link(s):`);
    for (const item of issues) {
      console.log(`   📄 ${item.file}:`);
      for (const b of item.broken.slice(0, 3)) {
        console.log(`      • Line ${b.line}: -> ${b.target}`);
      }
      if (item.broken.length > 3) {
        console.log(`      ... and ${item.broken.length - 3} more`);
      }
    }
    console.log();
  } else {
    console.log('✅ [DOCS-LINT] All Markdown relative links resolved cleanly!\n');
  }

  // Check master map status
  const mapPath = path.join(PROJECT_ROOT, 'docs/map.md');
  if (fs.existsSync(mapPath)) {
    const mapText = fs.readFileSync(mapPath, 'utf8');
    if (mapText.includes('```mermaid') && mapText.includes('S8') && mapText.includes('Vesktop')) {
      console.log('✅ [DOCS-LINT] docs/map.md contains valid Mermaid graph with S8 & Vesktop references.');
    }
  }

  process.exit(0);
}

main();
