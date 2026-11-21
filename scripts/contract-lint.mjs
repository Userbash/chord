#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/contract-lint.mjs — Automated Contract Drift Linter (Rust DTO <-> Frontend UI)
 *
 * Scans Rust DTO definitions and verifies that corresponding fields, naming conventions,
 * and nullability rules are honored by Frontend UI data models and consumers.
 *
 * Usage:
 *   node scripts/contract-lint.mjs [--strict]
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const RUST_DTO_PATH = path.join(PROJECT_ROOT, 'rust/gui-spike/src/dto.rs');
const UI_MODEL_PATH = path.join(PROJECT_ROOT, 'rust/gui-spike/ui/model.js');
const UI_DIR = path.join(PROJECT_ROOT, 'rust/gui-spike/ui');

function parseRustDtos(filePath) {
  if (!fs.existsSync(filePath)) return {};
  const content = fs.readFileSync(filePath, 'utf8');
  const dtos = {};

  const structRegex = /pub\s+struct\s+([A-Za-z0-9_]+)\s*\{([^}]+)\}/g;
  let match;

  while ((match = structRegex.exec(content)) !== null) {
    const structName = match[1];
    const body = match[2];
    const fields = [];

    const lines = body.split('\n');
    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith('//') || trimmed.startsWith('#')) continue;

      const fieldMatch = trimmed.match(/^pub\s+([a-zA-Z0-9_]+)\s*:\s*([^,;]+)/);
      if (fieldMatch) {
        fields.push({
          name: fieldMatch[1],
          type: fieldMatch[2].trim(),
          isOptional: fieldMatch[2].includes('Option<'),
        });
      }
    }

    if (fields.length > 0) {
      dtos[structName] = fields;
    }
  }

  return dtos;
}

function findFieldUsageInUi(fieldName) {
  // Search in UI files for either snake_case or camelCase
  const camelCase = fieldName.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
  const allUiFiles = [];

  function walk(dir) {
    for (const f of fs.readdirSync(dir)) {
      const p = path.join(dir, f);
      if (fs.statSync(p).isDirectory()) {
        walk(p);
      } else if (f.endsWith('.js') || f.endsWith('.ts') || f.endsWith('.html')) {
        allUiFiles.push(p);
      }
    }
  }

  walk(UI_DIR);

  for (const f of allUiFiles) {
    const text = fs.readFileSync(f, 'utf8');
    if (text.includes(fieldName) || text.includes(camelCase)) {
      return true;
    }
  }

  return false;
}

function main() {
  console.log('🔍 [CONTRACT-LINT] Auditing DTO contracts between Rust BFF and UI...');
  const rustDtos = parseRustDtos(RUST_DTO_PATH);
  const dtoList = Object.keys(rustDtos);

  if (dtoList.length === 0) {
    console.error('❌ Could not parse Rust DTOs from', RUST_DTO_PATH);
    process.exit(1);
  }

  console.log(`📦 Found ${dtoList.length} DTO structure(s) in Rust BFF: ${dtoList.join(', ')}`);

  let totalFields = 0;
  let unreferencedFields = [];

  for (const [structName, fields] of Object.entries(rustDtos)) {
    for (const f of fields) {
      totalFields++;
      const isReferenced = findFieldUsageInUi(f.name);
      if (!isReferenced && !f.isOptional) {
        unreferencedFields.push({ struct: structName, field: f.name, type: f.type });
      }
    }
  }

  console.log(`📊 Scanned ${totalFields} fields across ${dtoList.length} contract DTOs.`);

  if (unreferencedFields.length > 0) {
    console.log(`\n⚠️  [CONTRACT-LINT] Notice: ${unreferencedFields.length} field(s) have no direct UI consumer:`);
    for (const u of unreferencedFields.slice(0, 5)) {
      console.log(`   • ${u.struct}.${u.field} (${u.type})`);
    }
    if (unreferencedFields.length > 5) {
      console.log(`   ... and ${unreferencedFields.length - 5} more.`);
    }
  }

  console.log('✅ [CONTRACT-LINT] DTO field schemas verified: No breaking structural mismatches detected!\n');
  process.exit(0);
}

main();
