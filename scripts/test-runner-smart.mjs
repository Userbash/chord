#!/usr/bin/env node
// @ts-nocheck
/**
 * test-runner-smart.mjs — Smart Targeted Test Runner
 *
 * Runs targeted tests for changed files without wasting time on full workspace builds,
 * providing instant feedback and zero token cost.
 *
 * Usage:
 *   node scripts/test-runner-smart.mjs [file1 file2 ...]
 *   node scripts/test-runner-smart.mjs --git-diff
 */

import { execSync, spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

// Map pattern to targeted test commands
const TEST_ROUTING = [
  // UI Signals
  {
    pattern: /rust\/gui-spike\/ui\/state\/signals\.js/,
    commands: [['node', ['--test', 'tests/unit/signals.test.mjs']]],
  },
  // UI Features (JS)
  {
    pattern: /rust\/gui-spike\/ui\/(features|modules|state)\/.*\.js/,
    commands: [
      ['npm', ['run', 'typecheck']],
    ],
  },
  // Chat plugin
  {
    pattern: /rust\/gui-spike\/src\/chat_plugin\.rs/,
    commands: [
      ['cargo', ['test', '--manifest-path', 'rust/gui-spike/Cargo.toml', '--offline', '--bin', 'chord-client', 'chat_plugin']],
      ['cargo', ['test', '--manifest-path', 'rust/gui-spike/Cargo.toml', '--offline', '--test', 'messages_contract']],
    ],
  },
  // Guilds plugin
  {
    pattern: /rust\/gui-spike\/src\/guilds_plugin\.rs/,
    commands: [
      ['cargo', ['test', '--manifest-path', 'rust/gui-spike/Cargo.toml', '--offline', '--test', 'guilds_contract']],
      ['cargo', ['test', '--manifest-path', 'rust/gui-spike/Cargo.toml', '--offline', '--test', 'channels_contract']],
    ],
  },
  // Voice plugin & voice devices
  {
    pattern: /rust\/gui-spike\/src\/(voice_plugin|voice_devices)\.rs/,
    commands: [
      ['cargo', ['test', '--manifest-path', 'rust/gui-spike/Cargo.toml', '--offline', '--test', 'voice_devices_contract']],
    ],
  },
  // Bindings / Specta
  {
    pattern: /rust\/gui-spike\/src\/bindings\.rs/,
    commands: [
      ['cargo', ['check', '--manifest-path', 'rust/gui-spike/Cargo.toml', '--offline']],
    ],
  },
  // chord-platform crate
  {
    pattern: /rust\/crates\/chord-platform\//,
    commands: [
      ['cargo', ['test', '--manifest-path', 'rust/crates/chord-platform/Cargo.toml', '--offline']],
    ],
  },
  // chord-store crate
  {
    pattern: /rust\/crates\/chord-store\//,
    commands: [
      ['cargo', ['test', '--manifest-path', 'rust/crates/chord-store/Cargo.toml', '--offline']],
    ],
  },
  // chord-core crate
  {
    pattern: /rust\/crates\/chord-core\//,
    commands: [
      ['cargo', ['test', '--manifest-path', 'rust/crates/chord-core/Cargo.toml', '--offline']],
    ],
  },
];

export function getChangedFiles() {
  try {
    const statusOut = execSync('git status --porcelain', { cwd: PROJECT_ROOT, encoding: 'utf8' });
    const files = statusOut
      .split('\n')
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => line.replace(/^(\?\?|[MADRCU?!\s]{1,2})\s+/, '').trim());
    return files;
  } catch (err) {
    console.error('Failed to get git status:', err.message);
    return [];
  }
}

export function resolveTestCommands(files) {
  const commandsToRun = [];
  const seen = new Set();

  for (const file of files) {
    for (const route of TEST_ROUTING) {
      if (route.pattern.test(file)) {
        for (const cmd of route.commands) {
          const key = cmd[0] + ' ' + cmd[1].join(' ');
          if (!seen.has(key)) {
            seen.add(key);
            commandsToRun.push(cmd);
          }
        }
      }
    }
  }

  // Fallback: If rust file changed and no specific route matched
  const hasRust = files.some((f) => f.endsWith('.rs'));
  if (hasRust && commandsToRun.length === 0) {
    commandsToRun.push(['cargo', ['check', '--manifest-path', 'rust/gui-spike/Cargo.toml', '--offline']]);
  }

  // Fallback: If js file changed and no specific route matched
  const hasJs = files.some((f) => f.endsWith('.js') || f.endsWith('.mjs'));
  if (hasJs && commandsToRun.length === 0) {
    commandsToRun.push(['npm', ['run', 'typecheck']]);
  }

  return commandsToRun;
}

export function runTests(files) {
  const commands = resolveTestCommands(files);
  if (commands.length === 0) {
    console.log('⚡ No targeted tests required for specified files.');
    return { success: true, count: 0, outputs: [] };
  }

  console.log(`\n🧪 RUNNING TARGETED TESTS (${commands.length} test suites):`);
  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');

  const outputs = [];
  for (const [bin, args] of commands) {
    const fullCmd = `${bin} ${args.join(' ')}`;
    console.log(`▶ Running: ${fullCmd}`);

    const res = spawnSync(bin, args, {
      cwd: PROJECT_ROOT,
      stdio: 'inherit',
      env: { ...process.env, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || '4' },
    });

    if (res.status !== 0) {
      console.error(`❌ TEST FAILED: ${fullCmd} (exit ${res.status})`);
      return { success: false, failedCommand: fullCmd, exitCode: res.status };
    }
    console.log(`✅ PASSED: ${fullCmd}\n`);
    outputs.push(fullCmd);
  }

  console.log('━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━');
  console.log(`🎉 ALL ${commands.length} TARGETED TEST SUITES PASSED!\n`);
  return { success: true, count: commands.length, outputs };
}

// CLI handler
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  let files = [];
  if (args.includes('--git-diff') || args.length === 0) {
    files = getChangedFiles();
    console.log(`Detected git changes (${files.length} files):`, files);
  } else {
    files = args;
  }

  const result = runTests(files);
  process.exit(result.success ? 0 : 1);
}
