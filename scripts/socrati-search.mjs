#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/socrati-search.mjs — Compact Semantic Code Search for AI & Developers
 *
 * Queries SocratiCode semantic + BM25 search and returns condensed, high-relevance
 * snippets to minimize LLM token consumption while finding exact implementations.
 *
 * Usage:
 *   node scripts/socrati-search.mjs "<search_query>" [--limit 3]
 */

import { spawn } from 'node:child_process';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const SOCRATICODE_BIN = '/home/sanya/.local/bin/socraticode';

async function callSocratiCodeSearch(query, limit = 3) {
  return new Promise((resolve, reject) => {
    if (!fs.existsSync(SOCRATICODE_BIN)) {
      return reject(new Error('SocratiCode binary not found'));
    }

    const proc = spawn(SOCRATICODE_BIN, [], {
      cwd: PROJECT_ROOT,
      stdio: ['pipe', 'pipe', 'ignore'],
    });

    let stdoutData = '';
    const timeout = setTimeout(() => {
      proc.kill();
      reject(new Error('SocratiCode search timed out'));
    }, 10000);

    proc.stdout.on('data', (chunk) => {
      stdoutData += chunk.toString('utf8');
      const lines = stdoutData.split('\n');
      stdoutData = lines.pop() || '';

      for (const line of lines) {
        if (!line.trim()) continue;
        try {
          const msg = JSON.parse(line.trim());
          if (msg.id === 1) {
            const toolReq = {
              jsonrpc: '2.0',
              id: 2,
              method: 'tools/call',
              params: {
                name: 'codebase_search',
                arguments: {
                  project_path: PROJECT_ROOT,
                  query: query,
                  limit: limit,
                },
              },
            };
            proc.stdin.write(JSON.stringify(toolReq) + '\n');
          } else if (msg.id === 2) {
            clearTimeout(timeout);
            proc.kill();
            if (msg.error) {
              return reject(new Error(msg.error.message));
            }
            return resolve(msg.result);
          }
        } catch {}
      }
    });

    proc.on('error', (err) => {
      clearTimeout(timeout);
      reject(err);
    });

    const initReq = {
      jsonrpc: '2.0',
      id: 1,
      method: 'initialize',
      params: {
        protocolVersion: '2024-11-05',
        capabilities: {},
        clientInfo: { name: 'socrati-search-cli', version: '1.0' },
      },
    };
    proc.stdin.write(JSON.stringify(initReq) + '\n');
  });
}

async function main() {
  const query = process.argv[2];
  const limitArg = process.argv.find((a, i) => process.argv[i - 1] === '--limit') || '3';
  const limit = parseInt(limitArg, 10);

  if (!query) {
    console.error('Usage: node scripts/socrati-search.mjs "<search_query>" [--limit 3]');
    process.exit(1);
  }

  console.log(`🔎 [SOCRATI-SEARCH] Query: "${query}" (Top ${limit} matches)...`);

  try {
    const res = await callSocratiCodeSearch(query, limit);
    const rawText = res.content?.[0]?.text || 'No matches found.';

    // Parse and condense matches
    const sections = rawText.split(/---\s+([^\n]+)\s+---/g);
    if (sections.length <= 1) {
      console.log('   (No matches found)');
      return;
    }

    console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');
    for (let i = 1; i < sections.length; i += 2) {
      const header = sections[i].trim();
      const body = (sections[i + 1] || '').trim();
      const lines = body.split('\n');
      const condensedSnippet = lines.slice(0, 8).join('\n');

      console.log(`📄 Match: ${header}`);
      console.log(condensedSnippet);
      if (lines.length > 8) {
        console.log(`   ... [${lines.length - 8} more lines omitted for token brevity]`);
      }
      console.log('-------------------------------------------------------------');
    }
  } catch (err) {
    console.error(`⚠️  [SOCRATI-SEARCH] Failed: ${err.message}`);
    process.exit(1);
  }
}

main();
