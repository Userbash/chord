#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/smart-typecheck.mjs
 *
 * Token-saving TypeScript typecheck wrapper for AI workers (LIB-SIMPLIFY-06 / 603.2).
 * Condenses `tsc` output into a 3-line summary on success and caps cascading
 * diagnostics on failure while preserving exact file, line, and TS error codes.
 *
 * Usage:
 *   node scripts/smart-typecheck.mjs
 *   node scripts/smart-typecheck.mjs -p tsconfig.json
 *   node scripts/smart-typecheck.mjs --help
 */

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const DEFAULT_PROJECTS = [
  "tsconfig.json",
  "tsconfig.agent.json",
  "rust/gui-spike/ui/tsconfig.json",
];
const MAX_ERROR_LINES = 25;

function printUsage() {
  console.log(
    [
      "Usage: node scripts/smart-typecheck.mjs [-p <tsconfig> | --project <tsconfig>] [--help]",
      "Runs tsc --noEmit with compact, token-saving output.",
    ].join("\n"),
  );
}

function parseArgs(argv) {
  const projects = [];
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      return { help: true, projects: [] };
    }
    if (arg === "-p" || arg === "--project") {
      const next = argv[i + 1];
      if (!next) {
        console.error(`Missing argument for ${arg}`);
        process.exit(1);
      }
      projects.push(next);
      i++;
      continue;
    }
    console.error(`Unknown argument: ${arg}`);
    process.exit(1);
  }
  return {
    help: false,
    projects: projects.length > 0 ? projects : DEFAULT_PROJECTS.filter((p) => fs.existsSync(p)),
  };
}

function resolveTscBin() {
  const localBin = path.resolve(process.cwd(), "node_modules/.bin/tsc");
  return fs.existsSync(localBin) ? localBin : "tsc";
}

function runTsc(tscBin, projectPath) {
  const res = spawnSync(tscBin, ["--noEmit", "-p", projectPath], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (res.error && res.error.code === "ENOENT") {
    console.error("tsc binary not found in node_modules/.bin or PATH");
    process.exit(2);
  }
  return res;
}

function extractErrorLines(output) {
  const allLines = output.split(/\r?\n/).filter((l) => l.trim().length > 0);
  const tsLines = allLines.filter((l) => /error TS\d+:/.test(l));
  return tsLines.length > 0 ? tsLines : allLines;
}

const { help, projects } = parseArgs(process.argv.slice(2));
if (help) {
  printUsage();
  process.exit(0);
}
if (projects.length === 0) {
  console.error("No tsconfig files found to check.");
  process.exit(1);
}

const tscBin = resolveTscBin();
const startMs = Date.now();
const failures = [];

for (const project of projects) {
  const res = runTsc(tscBin, project);
  if (res.status !== 0) {
    failures.push({
      project,
      code: res.status ?? 1,
      lines: extractErrorLines(`${res.stdout || ""}\n${res.stderr || ""}`),
    });
  }
}

const elapsedMs = Date.now() - startMs;

if (failures.length === 0) {
  console.log("=========================================");
  console.log(`✔ TYPECHECK SUCCESS (Exit Code 0, ${projects.length} project(s), ${elapsedMs}ms)`);
  console.log(`  Checked: ${projects.join(", ")}`);
  console.log("=========================================");
  process.exit(0);
}

console.error("=========================================");
console.error(`✖ TYPECHECK FAILED (${failures.length}/${projects.length} project(s), ${elapsedMs}ms)`);
let printed = 0;
let truncated = false;
for (const fail of failures) {
  console.error(`-- ${fail.project} (${fail.lines.length} error line(s)) --`);
  for (const line of fail.lines) {
    if (printed >= MAX_ERROR_LINES) {
      truncated = true;
      break;
    }
    console.error(line);
    printed++;
  }
  if (truncated) break;
}
if (truncated) {
  console.error("... [вывод обрезан скриптом smart-typecheck для экономии токенов]");
}
console.error("=========================================");
process.exit(failures[0].code || 1);
