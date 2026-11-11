#!/usr/bin/env node
// @ts-check
/**
 * Автоматический анализатор логов `cargo-fuzz` + `AddressSanitizer` / `LeakSanitizer`
 * и генератор JSON/Markdown отчётов о паниках, выходах за границы буфера и утечках (TASK-903.3).
 */

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * @typedef {"panic" | "asan_oob" | "memory_leak" | "timeout_or_oom" | "fuzzer_signal"} FuzzCrashKind
 */

/**
 * @typedef {object} FuzzTargetSummary
 * @property {string} name
 * @property {string} logFile
 * @property {number} runsCompleted
 * @property {number} peakRssMb
 * @property {number} execPerSec
 * @property {boolean} passed
 * @property {FuzzCrashKind | null} crashKind
 */

/**
 * @typedef {object} FuzzCrashRecord
 * @property {string} target
 * @property {FuzzCrashKind} kind
 * @property {string} summary
 * @property {string} logFile
 * @property {string | null} artifactPath
 * @property {number | null} artifactSize
 * @property {string | null} artifactSha256
 * @property {string | null} artifactHexPreview
 * @property {string} reproduceCommand
 */

/**
 * @typedef {object} FuzzRunReport
 * @property {"passed" | "failed"} status
 * @property {string} generatedAt
 * @property {number} minRuns
 * @property {number} totalTargets
 * @property {FuzzTargetSummary[]} targets
 * @property {FuzzCrashRecord[]} crashes
 */

/**
 * Извлекает число завершённых итераций, пиковый RSS и скорость из лога `libFuzzer`.
 *
 * @param {string} logText
 * @returns {{ runsCompleted: number, peakRssMb: number, execPerSec: number }}
 */
function extractMetrics(logText) {
  let runsCompleted = 0;
  let peakRssMb = 0;
  let execPerSec = 0;

  const doneMatch = logText.match(/Done\s+(\d+)\s+runs\s+in\s+\d+\s+second/i);
  if (doneMatch?.[1]) {
    runsCompleted = Math.max(runsCompleted, Number.parseInt(doneMatch[1], 10));
  }

  const stepRegex = /^#(\d+)\s+\S+.*?exec\/s:\s*(\d+)\s+rss:\s*(\d+)Mb/gm;
  let match = stepRegex.exec(logText);
  while (match !== null) {
    const stepRuns = Number.parseInt(match[1] ?? "0", 10);
    const stepExec = Number.parseInt(match[2] ?? "0", 10);
    const stepRss = Number.parseInt(match[3] ?? "0", 10);
    if (stepRuns > runsCompleted) {
      runsCompleted = stepRuns;
    }
    if (stepRss > peakRssMb) {
      peakRssMb = stepRss;
    }
    if (stepExec > 0) {
      execPerSec = stepExec;
    }
    match = stepRegex.exec(logText);
  }

  return { runsCompleted, peakRssMb, execPerSec };
}

/**
 * Классифицирует сбой в логе `cargo-fuzz` / `ASan` / `LSan`.
 *
 * @param {string} logText
 * @returns {{ kind: FuzzCrashKind, summary: string } | null}
 */
function classifyCrash(logText) {
  const panicMatch = logText.match(/panicked at ([^\n]+)(?:\n([^\n=]+))?/);
  if (panicMatch) {
    const location = (panicMatch[1] ?? "").trim();
    const message = (panicMatch[2] ?? "").trim();
    const summary = message.length > 0 ? `${location} ${message}` : location;
    return { kind: "panic", summary };
  }

  if (
    /ERROR:\s*LeakSanitizer:\s*detected memory leaks/.test(logText) ||
    /SUMMARY:\s*AddressSanitizer:\s*\d+\s*byte\(s\)\s*leaked/.test(logText)
  ) {
    const leakSummary =
      logText.match(/SUMMARY:\s*AddressSanitizer:\s*([^\n]+)/)?.[1]?.trim() ??
      "LeakSanitizer: detected memory leaks";
    return { kind: "memory_leak", summary: leakSummary };
  }

  const asanMatch =
    logText.match(/ERROR:\s*AddressSanitizer:\s*([^\n]+)/) ??
    logText.match(/SUMMARY:\s*AddressSanitizer:\s*([^\n]+)/);
  if (asanMatch?.[1]) {
    return { kind: "asan_oob", summary: asanMatch[1].trim() };
  }

  const timeoutOrOom = logText.match(
    /ERROR:\s*libFuzzer:\s*(timeout|out-of-memory)([^\n]*)/i,
  );
  if (timeoutOrOom || /ALARM:\s*working on the last Unit/.test(logText)) {
    return {
      kind: "timeout_or_oom",
      summary: timeoutOrOom?.[0]?.trim() ?? "ALARM: превышен лимит времени на входной вектор",
    };
  }

  const signalMatch = logText.match(/ERROR:\s*libFuzzer:\s*([^\n]+)/);
  if (signalMatch?.[1]) {
    return { kind: "fuzzer_signal", summary: signalMatch[1].trim() };
  }

  return null;
}

/**
 * Находит входной артефакт краша на диске (из строки `Test unit written to ...` или каталога артефактов).
 *
 * @param {string} logText
 * @param {string | undefined} artifactDir
 * @param {string} targetName
 * @returns {string | null}
 */
function locateCrashArtifact(logText, artifactDir, targetName) {
  const writtenMatch = logText.match(/Test unit written to\s+(\S+)/);
  if (writtenMatch?.[1] && existsSync(writtenMatch[1])) {
    return writtenMatch[1];
  }
  if (!artifactDir) {
    return null;
  }
  const targetArtifactDir = join(artifactDir, targetName);
  if (!existsSync(targetArtifactDir) || !statSync(targetArtifactDir).isDirectory()) {
    return null;
  }
  const candidates = readdirSync(targetArtifactDir)
    .filter((file) => /^(crash|leak|oom|timeout)-/.test(file))
    .sort();
  const first = candidates[0];
  return first ? join(targetArtifactDir, first) : null;
}

/**
 * Анализирует каталог логов прогона `cargo-fuzz` и формирует структурированный отчёт.
 *
 * @param {{ logDir: string, artifactDir?: string, minRuns?: number }} options
 * @returns {FuzzRunReport}
 */
export function analyzeFuzzRun(options) {
  const minRuns = options.minRuns ?? 1_000_000;
  const entries = readdirSync(options.logDir)
    .filter((name) => name.endsWith(".log"))
    .sort();

  /** @type {FuzzTargetSummary[]} */
  const targets = [];
  /** @type {FuzzCrashRecord[]} */
  const crashes = [];

  for (const fileName of entries) {
    const targetName = basename(fileName, ".log");
    const logFile = join(options.logDir, fileName);
    const logText = readFileSync(logFile, "utf8");
    const metrics = extractMetrics(logText);
    const classified = classifyCrash(logText);

    if (classified !== null) {
      const artifactPath = locateCrashArtifact(logText, options.artifactDir, targetName);
      /** @type {number | null} */
      let artifactSize = null;
      /** @type {string | null} */
      let artifactSha256 = null;
      /** @type {string | null} */
      let artifactHexPreview = null;

      if (artifactPath !== null && existsSync(artifactPath)) {
        const raw = readFileSync(artifactPath);
        artifactSize = raw.byteLength;
        artifactSha256 = createHash("sha256").update(raw).digest("hex");
        artifactHexPreview = raw.subarray(0, 32).toString("hex");
      }

      const reproArg = artifactPath ?? `rust/fuzz/artifacts/${targetName}/<crash-file>`;
      crashes.push({
        target: targetName,
        kind: classified.kind,
        summary: classified.summary,
        logFile,
        artifactPath,
        artifactSize,
        artifactSha256,
        artifactHexPreview,
        reproduceCommand: `RUSTC_BOOTSTRAP=1 ./rust/.tools/bin/cargo-fuzz run --fuzz-dir rust/fuzz ${targetName} ${reproArg}`,
      });
    }

    const passed = classified === null && metrics.runsCompleted >= minRuns;
    targets.push({
      name: targetName,
      logFile,
      runsCompleted: metrics.runsCompleted,
      peakRssMb: metrics.peakRssMb,
      execPerSec: metrics.execPerSec,
      passed,
      crashKind: classified ? classified.kind : null,
    });
  }

  const allPassed =
    targets.length > 0 && crashes.length === 0 && targets.every((target) => target.passed);

  return {
    status: allPassed ? "passed" : "failed",
    generatedAt: new Date().toISOString(),
    minRuns,
    totalTargets: targets.length,
    targets,
    crashes,
  };
}

/**
 * Рендерит русскоязычный Markdown-отчёт фаззинга сетевых протоколов и криптографии DAVE MLS v1.
 *
 * @param {FuzzRunReport} report
 * @returns {string}
 */
export function renderMarkdownReport(report) {
  const lines = [
    "# Отчёт фаззинга протоколов и криптографии DAVE MLS v1",
    "",
    `- **Статус:** \`${report.status}\``,
    `- **Дата генерации:** \`${report.generatedAt}\``,
    `- **Целевой порог итераций на мишень:** \`${report.minRuns}\``,
    `- **Проверено мишеней:** \`${report.totalTargets}\``,
    `- **Обнаружено сбоев / утечек:** \`${report.crashes.length}\``,
    "",
    "## Сводка по fuzz-мишеням",
    "",
    "| Мишень | Итерации | Пик RSS (МБ) | Скорость (exec/s) | Статус |",
    "|---|---:|---:|---:|---|",
  ];

  for (const target of report.targets) {
    const badge = target.passed ? "PASS" : `FAIL (${target.crashKind ?? "insufficient_runs"})`;
    lines.push(
      `| \`${target.name}\` | ${target.runsCompleted} | ${target.peakRssMb} | ${target.execPerSec} | ${badge} |`,
    );
  }

  lines.push("");
  if (report.crashes.length === 0) {
    lines.push(
      "## Детали сбоев и утечек",
      "",
      "Паник, ошибок AddressSanitizer (OOB/UAF) и утечек памяти LeakSanitizer не обнаружено (`0` сбоев).",
      "",
    );
  } else {
    lines.push("## Детали сбоев и утечек", "");
    for (const crash of report.crashes) {
      lines.push(`### \`${crash.target}\` — \`${crash.kind}\``);
      lines.push(`- **Описание:** ${crash.summary}`);
      lines.push(`- **Лог:** \`${crash.logFile}\``);
      if (crash.artifactPath !== null) {
        lines.push(`- **Артефакт:** \`${crash.artifactPath}\` (${crash.artifactSize} байт)`);
        lines.push(`- **SHA-256:** \`${crash.artifactSha256}\``);
        lines.push(`- **Hex-превью:** \`${crash.artifactHexPreview}\``);
      }
      lines.push(`- **Команда воспроизведения:** \`${crash.reproduceCommand}\``);
      lines.push("");
    }
  }

  return lines.join("\n");
}

/**
 * Разбирает аргументы командной строки CLI.
 *
 * @param {string[]} argv
 * @returns {{ logDir: string, artifactDir: string, outJson: string, outMd: string, minRuns: number }}
 */
function parseCliArgs(argv) {
  let logDir = ".gate/fuzz-logs";
  let artifactDir = "rust/fuzz/artifacts";
  let outJson = ".gate/fuzz-report.json";
  let outMd = ".gate/fuzz-report.md";
  let minRuns = 1_000_000;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = argv[i + 1];
    if (arg === "--log-dir" && next) {
      logDir = next;
      i += 1;
    } else if (arg === "--artifact-dir" && next) {
      artifactDir = next;
      i += 1;
    } else if (arg === "--out-json" && next) {
      outJson = next;
      i += 1;
    } else if (arg === "--out-md" && next) {
      outMd = next;
      i += 1;
    } else if (arg === "--min-runs" && next) {
      const parsed = Number.parseInt(next, 10);
      if (!Number.isFinite(parsed) || parsed < 1) {
        throw new Error(`Некорректное значение --min-runs: ${next}`);
      }
      minRuns = parsed;
      i += 1;
    } else {
      throw new Error(`Неизвестный или неполный аргумент CLI: ${arg ?? ""}`);
    }
  }

  return {
    logDir: resolve(logDir),
    artifactDir: resolve(artifactDir),
    outJson: resolve(outJson),
    outMd: resolve(outMd),
    minRuns,
  };
}

function runCli() {
  /** @type {ReturnType<typeof parseCliArgs>} */
  let parsed;
  try {
    parsed = parseCliArgs(process.argv.slice(2));
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    process.stderr.write(`Ошибка аргументов fuzz-report: ${message}\n`);
    process.exit(1);
  }

  if (!existsSync(parsed.logDir) || !statSync(parsed.logDir).isDirectory()) {
    process.stderr.write(`Среда недоступна: каталог логов фаззинга не найден: ${parsed.logDir}\n`);
    process.exit(2);
  }

  const report = analyzeFuzzRun({
    logDir: parsed.logDir,
    artifactDir: parsed.artifactDir,
    minRuns: parsed.minRuns,
  });

  mkdirSync(dirname(parsed.outJson), { recursive: true });
  mkdirSync(dirname(parsed.outMd), { recursive: true });
  writeFileSync(parsed.outJson, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  writeFileSync(parsed.outMd, renderMarkdownReport(report), "utf8");

  process.stdout.write(
    `Отчёт фаззинга сохранён: статус=${report.status}, мишеней=${report.totalTargets}, сбоев=${report.crashes.length} (${parsed.outMd})\n`,
  );

  if (report.status !== "passed") {
    process.exit(1);
  }
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  runCli();
}
