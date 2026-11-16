#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/socrati-cycles.mjs — SocratiCode Circular Dependency Auditor
 *
 * Queries SocratiCode codebase_graph_circular to detect circular imports,
 * preventing spaghetti architecture and compile-time deadlocks.
 *
 * Usage:
 *   node scripts/socrati-cycles.mjs [--max-cycles 350] [--filter <path_substring>]
 */

import { spawn } from 'node:child_process';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const SOCRATICODE_BIN = '/home/sanya/.local/bin/socraticode';

async function callSocratiCodeTool(toolName, args) {
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
      reject(new Error('SocratiCode request timed out'));
    }, 15000);

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
                name: toolName,
                arguments: args,
              },
            };
            proc.stdin.write(JSON.stringify(toolReq) + '\n');
          } else if (msg.id === 2) {
            clearTimeout(timeout);
            proc.kill();
            if (msg.error) {
              return reject(new Error(msg.error.message || 'Tool execution error'));
            }
            return resolve(msg.result);
          }
        } catch {
          // ignore non-json notifications
        }
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
        clientInfo: { name: 'socrati-cycles-cli', version: '1.0' },
      },
    };
    proc.stdin.write(JSON.stringify(initReq) + '\n');
  });
}

async function main() {
  const filterArg = process.argv.find((_, i, arr) => arr[i - 1] === '--filter') || '';
  console.log('🔄 [SOCRATI-CYCLES] Auditing circular dependencies via SocratiCode AST graph...');

  try {
    const res = await callSocratiCodeTool('codebase_graph_circular', {
      projectPath: PROJECT_ROOT,
    });

    const text = res.content?.[0]?.text || '';
    const lines = text.split('\n').filter(l => l.trim().startsWith('Cycle '));

    console.log(`📊 Total detected circular dependency chains: ${lines.length}`);

    const filtered = filterArg
      ? lines.filter(l => l.includes(filterArg))
      : lines;

    if (filterArg) {
      console.log(`🔍 Filter '${filterArg}': ${filtered.length} matching cycles`);
    }

    if (filtered.length > 0) {
      console.log('\n⚠️  Sample circular dependency chains:');
      for (const c of filtered.slice(0, 10)) {
        console.log(`   ${c.trim()}`);
      }
      if (filtered.length > 10) {
        console.log(`   ... and ${filtered.length - 10} more cycles.`);
      }
    } else {
      console.log('✅ No circular dependencies detected in the filtered scope.');
    }
  } catch (err) {
    console.error(`⚠️  [SOCRATI-CYCLES] Tool unavailable: ${err.message}`);
    process.exit(1);
  }
}

main();
