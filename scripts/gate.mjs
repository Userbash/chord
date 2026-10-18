#!/usr/bin/env node
// @ts-check
// Гейт discord-lab — адаптация производственного процесса (PROCESS.md, раздел «Гейты»).
//
// Скоупы:
//   --scope worker   быстрый гейт воркера: типы → мета → unit → smoke;
//   --scope release  гейт выпуска: worker + e2e (повтор при падении) + покрытие + golden-сверка;
//   --scope qualify  qualification: release + сборка tarball + изолированный прогон пакета
//                    + запись файла qualification (sha256; пересборка пакета делает файл недействительным).
//
// Утилиты:
//   --freeze-golden                записать md5-манифест golden-каталогов (.gate/golden.manifest);
//   --verify-qualification <file>  сверить tarball и коммит с записью qualification;
//   --help                         справка.
//
// Артефакты: .gate/<scope>-<timestamp>.json, qualification — .gate/qualification-<version>-<timestamp>.json.
// Вердикты: passed | passed_with_skips | failed. «Пропуск прохождением не считается»: все skip
// попадают в warnings и в отчёт. Выход: 0 — passed/passed_with_skips, 1 — failed/ошибка аргументов.

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const gateDir = join(root, ".gate");

const pkg = /** @type {{ name: string, version: string }} */ (
  JSON.parse(readFileSync(join(root, "package.json"), "utf8"))
);

/**
 * @typedef {object} TestCounts
 * @property {number | null} tests
 * @property {number | null} pass
 * @property {number | null} fail
 * @property {number | null} skipped
 *
 * @typedef {object} PhaseResult
 * @property {string} name
 * @property {boolean} ok
 * @property {number} ms
 * @property {TestCounts | null} counts
 * @property {string} tail
 * @property {string | null} note
 * @property {Record<string, unknown>} [extra]
 *
 * @typedef {object} Phase
 * @property {string} name
 * @property {string} [command]
 * @property {string[]} [args]
 * @property {number} [retries]
 * @property {() => PhaseResult} [run]
 */

/**
 * @param {string} command
 * @param {string[]} args
 * @param {{ cwd?: string, timeoutMs?: number }} [options]
 */
function runCommand(command, args, options = {}) {
  const startedMs = Date.now();
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? root,
    encoding: "utf8",
    timeout: options.timeoutMs ?? 900000,
    maxBuffer: 512 * 1024 * 1024,
  });
  return {
    ok: !result.error && result.status === 0,
    status: result.status,
    error: result.error ? String(result.error.message) : null,
    output: `${result.stdout ?? ""}${result.stderr ?? ""}`,
    ms: Date.now() - startedMs,
  };
}

/** @param {string} output @returns {TestCounts | null} */
function testCounts(output) {
  /** @param {string} label */
  const pick = (label) => {
    const match = output.match(new RegExp(`ℹ ${label} (\\d+)`, "u"));
    return match ? Number(match[1]) : null;
  };
  const counts = { tests: pick("tests"), pass: pick("pass"), fail: pick("fail"), skipped: pick("skipped") };
  return counts.tests === null && counts.pass === null ? null : counts;
}

/** @param {string} text @param {number} lines */
function tail(text, lines) {
  const all = text.split("\n");
  return all.slice(Math.max(0, all.length - lines)).join("\n");
}

/** @param {string[]} args @returns {string | null} */
function git(args) {
  const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : null;
}

function gitInfo() {
  const status = git(["status", "--porcelain"]);
  return {
    commit: git(["rev-parse", "HEAD"]),
    branch: git(["rev-parse", "--abbrev-ref", "HEAD"]),
    dirty: status === null ? null : status.length > 0,
  };
}

/** @type {Phase[]} */
const commonPhases = [
  { name: "typecheck", command: "npm", args: ["run", "typecheck"] },
  { name: "meta", command: "node", args: ["--test", "tests/meta/**/*.test.mjs"] },
  { name: "unit", command: "node", args: ["--test", "tests/unit/**/*.test.mjs"] },
  { name: "smoke", command: "node", args: ["--test", "tests/smoke/**/*.test.mjs"] },
];

/** @returns {PhaseResult} */
function goldenPhase() {
  const startedMs = Date.now();
  const manifest = join(gateDir, "golden.manifest");
  if (!existsSync(manifest)) {
    return {
      name: "golden", ok: true, ms: 0, counts: null, tail: "",
      note: "манифеста нет — сверка пропущена (npm run golden:freeze)",
    };
  }
  const result = runCommand("md5sum", ["-c", ".gate/golden.manifest"], { timeoutMs: 300000 });
  return {
    name: "golden", ok: result.ok, ms: Date.now() - startedMs, counts: null,
    tail: tail(result.output, result.ok ? 3 : 40), note: null,
  };
}

/**
 * @param {boolean} ok
 * @param {number} startedMs
 * @param {string} report
 * @param {Record<string, unknown>} [extra]
 * @returns {PhaseResult}
 */
function countPackResult(ok, startedMs, report, extra) {
  return /** @type {PhaseResult} */ ({
    name: "pack", ok, ms: Date.now() - startedMs, counts: null, tail: report, note: null,
    ...(extra ? { extra } : {}),
  });
}

/** @returns {PhaseResult} */
function packPhase() {
  const startedMs = Date.now();
  const result = runCommand("npm", ["pack", "--pack-destination", ".gate"]);
  const tarballName = `${pkg.name}-${pkg.version}.tgz`;
  const tarball = join(gateDir, tarballName);
  if (!existsSync(tarball)) {
    return countPackResult(false, startedMs, `npm pack не создал ${tarballName}\n${tail(result.output, 40)}`);
  }
  const sha256 = createHash("sha256").update(readFileSync(tarball)).digest("hex");
  const listing = runCommand("tar", ["-tzf", tarball]);
  const entries = listing.output.split("\n").filter(Boolean);
  const forbidden = entries.filter((entry) => /^package\/(logs|functions|evidence|node_modules|\.gate)\//.test(entry));
  const hasCli = entries.includes("package/bin/discord-lab.mjs");
  const hasCliSource = entries.includes("package/src/cli/run.mjs");

  const workDir = mkdtempSync(join(tmpdir(), "discord-lab-qualify-"));
  try {
    const extract = runCommand("tar", ["-xzf", tarball, "-C", workDir]);
    const packageDir = join(workDir, "package");
    const cliPath = join(packageDir, "bin", "discord-lab.mjs");
    const help = runCommand("node", [cliPath, "--help"], { cwd: packageDir });
    const version = runCommand("node", [cliPath, "--version"], { cwd: packageDir });
    const inventory = runCommand("node", [cliPath, "inventory", "--pid", "99999999"], { cwd: packageDir });
    const helpOk = help.ok && help.output.includes("Команды:");
    const versionOk = version.ok && version.output.trim() === pkg.version;
    const inventoryOk = inventory.status === 2 && /Frida недоступна|пропущено/.test(inventory.output);
    const ok = extract.ok && forbidden.length === 0 && hasCli && hasCliSource && helpOk && versionOk && inventoryOk;
    const report = [
      `tarball: ${tarballName}`,
      `sha256: ${sha256}`,
      `файлов в архиве: ${entries.length}; запрещённых: ${forbidden.length}`,
      `bin/cli: ${hasCli}; src/cli/run.mjs: ${hasCliSource}`,
      `--help: ${helpOk} (код ${help.status})`,
      `--version: ${versionOk} (код ${version.status}: ${version.output.trim()})`,
      `inventory --pid 99999999: ${inventoryOk} (код ${inventory.status})`,
      forbidden.length > 0 ? `запрещённые пути: ${forbidden.slice(0, 10).join(", ")}` : "",
    ].filter(Boolean).join("\n");
    return countPackResult(ok, startedMs, report, { tarball: tarballName, sha256 });
  } finally {
    rmSync(workDir, { recursive: true, force: true });
  }
}

/** @param {string} file @returns {boolean} */
function verifyQualification(file) {
  const absolute = resolve(file);
  const record = /** @type {{ tarball?: string, sha256?: string, gitCommit?: string | null, verdict?: string }} */ (
    JSON.parse(readFileSync(absolute, "utf8"))
  );
  if (!record.tarball || !record.sha256) {
    console.error("Запись не содержит tarball/sha256 — это не qualification-файл.");
    return false;
  }
  const tarball = join(dirname(absolute), record.tarball);
  if (!existsSync(tarball)) {
    console.error(`Нет tarball: ${tarball}`);
    return false;
  }
  const sha256 = createHash("sha256").update(readFileSync(tarball)).digest("hex");
  const head = git(["rev-parse", "HEAD"]);
  const shaOk = sha256 === record.sha256;
  const commitOk = !head || head === record.gitCommit;
  const verdictOk = String(record.verdict ?? "").startsWith("passed");
  console.log(`tarball: ${record.tarball}`);
  console.log(`sha256: ${shaOk ? "совпадает" : `РАСХОЖДЕНИЕ (${sha256})`}`);
  console.log(`коммит: ${commitOk ? "совпадает" : `РАСХОЖДЕНИЕ (HEAD ${head} ≠ ${record.gitCommit ?? "?"})`}`);
  console.log(`вердикт записи: ${record.verdict}`);
  return shaOk && commitOk && verdictOk;
}

function freezeGolden() {
  mkdirSync(gateDir, { recursive: true });
  const dirs = ["logs", "functions", "evidence"].filter((dir) => existsSync(join(root, dir)));
  if (dirs.length === 0) {
    console.error("Нет golden-каталогов (logs/functions/evidence) — нечего фиксировать.");
    process.exitCode = 1;
    return;
  }
  /** @type {string[]} */
  const lines = [];
  for (const dir of dirs) {
    const listing = runCommand("find", [dir, "-type", "f"]);
    const files = listing.output.split("\n").filter(Boolean).sort();
    for (const file of files) {
      const digest = createHash("md5").update(readFileSync(join(root, file))).digest("hex");
      lines.push(`${digest}  ${file}`);
    }
  }
  writeFileSync(join(gateDir, "golden.manifest"), `${lines.join("\n")}\n`);
  console.log(`Манифест golden: ${lines.length} файлов → .gate/golden.manifest`);
}

/** @param {Phase} phase @returns {PhaseResult} */
function executePhase(phase) {
  if (phase.run) return phase.run();
  /** @type {{ attempt: number, ok: boolean, status: number | null, ms: number, counts: TestCounts | null }[]} */
  const attempts = [];
  const maxAttempts = 1 + (phase.retries ?? 0);
  let last = null;
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    last = runCommand(phase.command ?? "", phase.args ?? []);
    attempts.push({ attempt, ok: last.ok, status: last.status, ms: last.ms, counts: testCounts(last.output) });
    if (last.ok) break;
    if (attempt < maxAttempts) console.error(`  ↻ повтор фазы ${phase.name} (попытка ${attempt + 1} из ${maxAttempts})`);
  }
  const finalAttempt = attempts[attempts.length - 1];
  return {
    name: phase.name,
    ok: Boolean(finalAttempt?.ok),
    ms: attempts.reduce((sum, item) => sum + item.ms, 0),
    counts: finalAttempt?.counts ?? null,
    tail: tail(last?.output ?? "", finalAttempt?.ok ? 5 : 60),
    note: attempts.length > 1 ? `попыток: ${attempts.length}; первый прогон: ${attempts[0]?.ok ? "ok" : "fail"}` : null,
  };
}

/** @param {string[]} argv */
function parseCli(argv) {
  /** @type {{ scope: string | null, freezeGolden: boolean, verify: string | null, help: boolean }} */
  const options = { scope: null, freezeGolden: false, verify: null, help: false };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--scope") {
      index += 1;
      options.scope = argv[index] ?? "";
    } else if (arg === "--freeze-golden") options.freezeGolden = true;
    else if (arg === "--verify-qualification") {
      index += 1;
      options.verify = argv[index] ?? "";
    } else if (arg === "--help" || arg === "-h") options.help = true;
    else throw new Error(`Неизвестный аргумент: ${arg}`);
  }
  return options;
}

function usage() {
  console.log(`Гейт discord-lab
Использование:
  node scripts/gate.mjs --scope worker|release|qualify
  node scripts/gate.mjs --freeze-golden
  node scripts/gate.mjs --verify-qualification <qualification.json>

Скоупы:
  worker   типы → мета → unit → smoke (быстрый, для воркеров)
  release  worker + e2e (повтор при падении) + покрытие + golden-сверка
  qualify  release + сборка tarball + изолированный прогон пакета (запись qualification)

Отчёты: .gate/<scope>-<timestamp>.json; qualification — .gate/qualification-<version>-<timestamp>.json.
«Пропуск прохождением не считается»: skip попадают в warnings отчёта.`);
}

/** @returns {number} */
function main() {
  const options = parseCli(process.argv.slice(2));
  if (options.help) {
    usage();
    return 0;
  }
  if (options.freezeGolden) {
    freezeGolden();
    return process.exitCode ? 1 : 0;
  }
  if (options.verify) return verifyQualification(options.verify) ? 0 : 1;
  if (!options.scope || !["worker", "release", "qualify"].includes(options.scope)) {
    usage();
    return 1;
  }

  mkdirSync(gateDir, { recursive: true });
  /** @type {Phase[]} */
  const phases = [...commonPhases];
  if (options.scope !== "worker") {
    phases.push({ name: "e2e", command: "node", args: ["--test", "tests/e2e/**/*.test.mjs"], retries: 1 });
    phases.push({ name: "coverage", command: "npm", args: ["run", "test:coverage"] });
    phases.push({ name: "golden", run: goldenPhase });
  }
  if (options.scope === "qualify") {
    phases.push({ name: "pack", run: packPhase });
  }

  const startedAt = new Date().toISOString();
  const results = phases.map(executePhase);

  let totalSkipped = 0;
  /** @type {string[]} */
  const warnings = [];
  console.log("");
  for (const result of results) {
    const countPart = result.counts
      ? ` (тесты ${result.counts.tests ?? "?"}: pass ${result.counts.pass ?? "?"}, fail ${result.counts.fail ?? "?"}, skip ${result.counts.skipped ?? "?"})`
      : "";
    const notePart = result.note ? ` [${result.note}]` : "";
    console.log(`${result.ok ? "✔" : "✖"} ${result.name} — ${(result.ms / 1000).toFixed(1)} с${countPart}${notePart}`);
    if (!result.ok || result.name === "pack") {
      console.log(result.tail.split("\n").map((line) => `   ${line}`).join("\n"));
    }
    const skipped = result.counts?.skipped ?? 0;
    if (skipped > 0) {
      totalSkipped += skipped;
      warnings.push(`${result.name}: пропущено ${skipped} тестов — пропуск прохождением не считается`);
    }
  }

  const allOk = results.every((result) => result.ok);
  const verdict = !allOk ? "failed" : totalSkipped > 0 ? "passed_with_skips" : "passed";
  const packResult = results.find((result) => result.name === "pack");

  const record = {
    schema: 1,
    tool: "discord-lab gate",
    scope: options.scope,
    startedAt,
    finishedAt: new Date().toISOString(),
    git: gitInfo(),
    gitCommit: git(["rev-parse", "HEAD"]),
    package: { name: pkg.name, version: pkg.version },
    verdict,
    warnings,
    phases: results.map((result) => ({
      name: result.name,
      ok: result.ok,
      ms: result.ms,
      counts: result.counts,
      note: result.note,
      extra: result.extra ?? null,
    })),
  };
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const scopeFile = join(gateDir, `${options.scope}-${stamp}.json`);
  writeFileSync(scopeFile, `${JSON.stringify(record, null, 2)}\n`);
  console.log(`\nОтчёт гейта: ${scopeFile}`);
  if (options.scope === "qualify") {
    const qualificationFile = join(gateDir, `qualification-${pkg.version}-${stamp}.json`);
    writeFileSync(qualificationFile, `${JSON.stringify({ ...record, tarball: packResult?.extra?.tarball ?? null, sha256: packResult?.extra?.sha256 ?? null }, null, 2)}\n`);
    console.log(`Qualification: ${qualificationFile}`);
    console.log(`Проверка при выкладке: node scripts/gate.mjs --verify-qualification ${qualificationFile}`);
  }
  for (const warning of warnings) console.log(`⚠ ${warning}`);
  console.log(`Вердикт: ${verdict}`);
  return allOk ? 0 : 1;
}

try {
  process.exitCode = main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
