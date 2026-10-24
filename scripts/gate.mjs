#!/usr/bin/env node
// @ts-check
// Гейт discord-lab — адаптация производственного процесса (PROCESS.md, раздел «Гейты»).
//
// Скоупы:
//   --scope worker   быстрый гейт воркера: типы → мета → unit → smoke;
//   --scope release  гейт выпуска: worker + e2e (повтор при падении) + покрытие + live-keyring
//                    (обязательная фаза, LIVE=1, failOnSkips) + golden-сверка;
//   --scope qualify  qualification: release + сборка tarball + изолированный прогон пакета
//                    + запись файла qualification (sha256; пересборка пакета делает файл недействительным).
//
// Утилиты:
//   --freeze-golden                записать md5-манифест golden-каталогов (.gate/golden.manifest);
//   --verify-qualification <file>  сверить tarball и коммит с записью qualification;
//   --help                         справка.
//
// Артефакты: .gate/<scope>-<timestamp>.json, qualification — .gate/qualification-<version>-<timestamp>.json.
// Вердикты: passed | passed_with_skips | passed_with_unmet | failed. «Пропуск прохождением
// не считается»: skip попадают в warnings, а e2e/live-наборы объявлены с failOnSkips (skip ⇒
// фаза failed). Живой Discord — отдельная фаза e2e-live: без DISCORD_LAB_LIVE_CDP это unmet,
// а не skip и не pass. Приёмка считается по счётчикам (`e2e: {fixture, live}` + acceptance), а
// не по вердикту. Выход: 0 — прогон принят (в т.ч. worker), 1 — failed/unmet/ошибка аргументов.

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync,
} from "node:fs";
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
 * @typedef {object} PhaseAttempt
 * @property {number} attempt
 * @property {boolean} ok
 * @property {number | null} status
 * @property {number} ms
 * @property {TestCounts | null} counts
 * @property {string[]} reasons
 * @property {string} tail
 *
 * @typedef {object} PhaseResult
 * @property {string} name
 * @property {boolean} ok
 * @property {boolean} unmet живая проверка не выполнялась (нет среды), это не pass
 * @property {"ignored" | null} [skipKind] `ignored` — ожидаемый ignored-набор (Rust), не e2e-skip
 * @property {number} ms
 * @property {TestCounts | null} counts
 * @property {string} tail
 * @property {string | null} note
 * @property {PhaseAttempt[]} [attempts] все попытки с их выводом (повтор не теряется)
 * @property {Record<string, unknown>} [extra]
 *
 * @typedef {object} Phase
 * @property {string} name
 * @property {string} [command]
 * @property {string[]} [args]
 * @property {string} [cwd]
 * @property {number} [timeoutMs]
 * @property {number} [retries]
 * @property {"cargo"} [countsKind]
 * @property {"ignored"} [skipKind] ожидаемый ignored-набор: не превращает вердикт в skip
 * @property {string} [requiresEnv] env живой фазы: нет значения ⇒ unmet, а не запуск/skip
 * @property {Record<string, string>} [env] дополнительное окружение фазы
 * @property {boolean} [failOnSkips] `skipped > 0` ⇒ фаза failed (e2e и live-наборы)
 * @property {number} [minPass] минимум выполненных тестов; меньше ⇒ «e2e_unmet»
 * @property {() => string[]} [preflight] диагностика до запуска (попадает в note фазы)
 * @property {() => PhaseResult} [run]
 */

/**
 * @param {string} command
 * @param {string[]} args
 * @param {{ cwd?: string, timeoutMs?: number, env?: Record<string, string> }} [options]
 */
function runCommand(command, args, options = {}) {
  const startedMs = Date.now();
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? root,
    encoding: "utf8",
    timeout: options.timeoutMs ?? 900000,
    maxBuffer: 512 * 1024 * 1024,
    ...(options.env ? { env: { ...process.env, ...options.env } } : {}),
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

/**
 * Счётчики `cargo test`: в workspace несколько бинарников, поэтому строки
 * `test result: ... N passed; M failed; K ignored` суммируются.
 * @param {string} output
 * @returns {TestCounts | null}
 */
function cargoTestCounts(output) {
  let tests = 0;
  let pass = 0;
  let fail = 0;
  let skipped = 0;
  let found = false;
  for (const match of output.matchAll(/test result: [^.]*\. (\d+) passed; (\d+) failed; (\d+) ignored/gu)) {
    found = true;
    pass += Number(match[1]);
    fail += Number(match[2]);
    skipped += Number(match[3]);
  }
  if (!found) return null;
  tests = pass + fail + skipped;
  return { tests, pass, fail, skipped };
}

/** @param {string} text @param {number} lines */
function tail(text, lines) {
  const all = text.split("\n");
  return all.slice(Math.max(0, all.length - lines)).join("\n");
}

/**
 * Правило «skip ≠ pass» для одной фазы. Чистая функция: её поведение проверяет
 * meta-тест `tests/meta/test-policy.test.mjs` на синтетических счётчиках.
 * @param {{ failOnSkips?: boolean, minPass?: number }} phase
 * @param {TestCounts | null} counts
 * @param {boolean} exitOk
 * @returns {{ ok: boolean, skipFailure: boolean, unmet: boolean, reasons: string[] }}
 */
function phaseOutcome(phase, counts, exitOk) {
  const reasons = [];
  if (!exitOk) reasons.push("код выхода ≠ 0");
  const skipped = counts?.skipped ?? 0;
  const skipFailure = Boolean(phase.failOnSkips) && skipped > 0;
  if (skipFailure) reasons.push(`failOnSkips: skip ${skipped} — пропуск прохождением не считается`);
  const pass = counts?.pass ?? 0;
  const unmet = typeof phase.minPass === "number" && pass < phase.minPass;
  if (unmet) reasons.push(`e2e_unmet: pass ${pass} < ${phase.minPass} — набор не выполнен`);
  return { ok: exitOk && !skipFailure && !unmet, skipFailure, unmet, reasons };
}

/**
 * Вердикт прогона. `unmet` (живая среда не задана) — отдельный исход, а не skip и не pass.
 * @param {{ ok: boolean, unmet?: boolean, skipKind?: string | null, counts: TestCounts | null }[]} results
 * @returns {"passed" | "passed_with_skips" | "passed_with_unmet" | "failed"}
 */
function computeVerdict(results) {
  if (results.some((result) => !result.ok && !result.unmet)) return "failed";
  if (results.some((result) => result.unmet)) return "passed_with_unmet";
  const unexpected = results.filter(
    (result) => (result.counts?.skipped ?? 0) > 0 && result.skipKind !== "ignored",
  );
  return unexpected.length > 0 ? "passed_with_skips" : "passed";
}

/**
 * Статус e2e-набора по счётчикам, а не по вердикту: met | unmet | absent.
 * `absent` — фазы нет; `unmet` — не выполнялась или выполнялась без pass.
 * @param {{ name: string, ok: boolean, unmet?: boolean, counts: TestCounts | null }[]} results
 * @param {string} name
 * @returns {"met" | "unmet" | "absent"}
 */
function e2eStatus(results, name) {
  const result = results.find((item) => item.name === name);
  if (!result) return "absent";
  if (result.unmet) return "unmet";
  const counts = result.counts;
  if (!result.ok || !counts) return "unmet";
  if ((counts.pass ?? 0) <= 0 || (counts.fail ?? 0) > 0 || (counts.skipped ?? 0) > 0) return "unmet";
  return "met";
}

/**
 * Приёмка прогона: fixture e2e доказан, live — для qualify (T3). Вердикт `passed_with_skips`
 * сам по себе приёмкой не является, если e2e не выполнен.
 * @param {"worker" | "release" | "qualify"} scope
 * @param {string} verdict
 * @param {"met" | "unmet" | "absent"} fixtureE2e
 * @param {"met" | "unmet" | "absent"} liveE2e
 * @returns {{ accepted: boolean, reason: string | null } | null}
 */
function computeAcceptance(scope, verdict, fixtureE2e, liveE2e) {
  if (scope === "worker") return null;
  if (verdict === "failed") {
    return { accepted: false, reason: "есть упавшие фазы — прогон не является приёмкой" };
  }
  if (fixtureE2e !== "met") {
    return { accepted: false, reason: `e2e.fixture=${fixtureE2e}: fixture-набор не доказан (skip ≠ pass)` };
  }
  if (scope === "qualify" && liveE2e !== "met") {
    return { accepted: false, reason: `e2e.live=${liveE2e}: live-проверка не выполнена — unmet, а не pass` };
  }
  return { accepted: true, reason: null };
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
  { name: "ui-tsc", command: "npx", args: ["tsc", "-p", "rust/gui-spike/ui/tsconfig.json"] },
  { name: "meta", command: "node", args: ["--test", "tests/meta/**/*.test.mjs"] },
  { name: "unit", command: "node", args: ["--test", "tests/unit/**/*.test.mjs"] },
  {
    name: "spike-rust",
    command: "cargo",
    args: ["test", "--offline", "--manifest-path", "rust/gui-spike/Cargo.toml"],
    skipKind: "ignored",
  },
  {
    name: "workspace-rust",
    command: "cargo",
    args: ["test", "--offline", "--workspace"],
    cwd: "rust",
    timeoutMs: 900000,
    countsKind: "cargo",
    // Rust `ignored` — не «нет среды»: live-кейсы keyring гоняет отдельная фаза. Такие skip
    // попадают в warnings/ignored, но не превращают вердикт в e2e-skip (QA-M2-09).
    skipKind: "ignored",
  },
  { name: "smoke", command: "node", args: ["--test", "--test-concurrency=1", "tests/smoke/**/*.test.mjs"] },
];

/**
 * Окружение live-фазы keyring: флаг выставляет сам гейт, только в release/qualify.
 * @returns {Record<string, string>}
 */
function keyringLiveEnv() {
  return { DISCORD_LAB_KEYRING_LIVE: "1" };
}

/**
 * Preflight live-фазы (PLAT-02): env-флаг и CLI `secret-tool` до запуска cargo, чтобы
 * `not-enabled`/`tool-missing` были видны в note, а не только в упавшем кейсе.
 * @returns {string[]}
 */
function keyringLivePreflight() {
  const notes = ["env: DISCORD_LAB_KEYRING_LIVE=1 (выставлен фазой)"];
  const tool = runCommand("sh", ["-c", "command -v secret-tool"], { timeoutMs: 15000 });
  const path = tool.ok ? (tool.output.split("\n")[0] ?? "").trim() : "";
  if (!path) {
    notes.push("secret-tool: нет в PATH — k07/k08 упадут с KEYRING-LIVE: tool-missing");
    return notes;
  }
  const version = runCommand("secret-tool", ["--version"], { timeoutMs: 15000 });
  notes.push(
    `secret-tool: ${path} (${version.ok ? version.output.trim() : `--version код ${version.status}`})`,
  );
  return notes;
}

/**
 * Live-набор keyring: только release/qualify, только `--ignored`. Worker его не запускает,
 * поэтому в дефолтном гейте те же 12 кейсов видны как `skip 12`, а не как прошедшие (PLAT-02).
 * @type {Phase}
 */
const keyringLivePhase = {
  name: "keyring-live",
  command: "cargo",
  args: ["test", "--offline", "-p", "discord-platform", "--test", "keyring_contract", "--", "--ignored"],
  cwd: "rust",
  timeoutMs: 900000,
  countsKind: "cargo",
  failOnSkips: true,
  env: keyringLiveEnv(),
  preflight: keyringLivePreflight,
};

/**
 * Диагностика e2e до запуска: отсутствие браузера — infra fail, а не повод принять skip.
 * @returns {string[]}
 */
function browserPreflight() {
  const explicit = process.env.CHROME_BIN ?? process.env.CHROME_PATH ?? "";
  if (explicit) {
    return [
      existsSync(explicit)
        ? `браузер: ${explicit}`
        : `e2e_unmet: ${explicit} не существует — тесты уйдут в skip и фаза будет failed`,
    ];
  }
  const probe = runCommand("sh", [
    "-c",
    "command -v chromium || command -v chromium-browser || command -v google-chrome || command -v chrome || command -v brave || true",
  ], { timeoutMs: 15000 });
  const found = probe.output.split("\n").map((line) => line.trim()).find(Boolean) ?? "";
  if (!found) {
    return [
      "e2e_unmet: браузера нет (CHROME_BIN не задан, chromium/chrome/brave в PATH нет) — " +
        "тесты уйдут в skip, фаза e2e станет failed",
    ];
  }
  return [`браузер: ${found}`];
}

/**
 * Fixture-файлы e2e: все `tests/e2e/*.e2e.test.mjs`, кроме `live-*`. Живой Discord идёт
 * отдельной фазой e2e-live и в T2 (release) не входит.
 * @returns {string[]}
 */
function fixtureE2eFiles() {
  const dir = join(root, "tests", "e2e");
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(".e2e.test.mjs") && !entry.name.startsWith("live-"))
    .map((entry) => `tests/e2e/${entry.name}`)
    .sort();
}

/**
 * Fixture-набор e2e — обязательная фаза release/qualify. `failOnSkips`: без браузера тесты
 * уходят в skip, и это failed, а не `passed_with_skips`. `minPass` закрывает случай «0 тестов».
 * @returns {Phase}
 */
function e2eFixturePhase() {
  const files = fixtureE2eFiles();
  if (files.length === 0) {
    return {
      name: "e2e",
      run: () => ({
        name: "e2e", ok: false, unmet: false, skipKind: null, ms: 0, counts: null, attempts: [],
        tail: "", note: "e2e_unmet: fixture-файлов tests/e2e/*.e2e.test.mjs нет",
      }),
    };
  }
  return {
    name: "e2e",
    command: "node",
    args: ["--test", "--test-concurrency=1", ...files],
    retries: 1,
    failOnSkips: true,
    minPass: 1,
    preflight: () => [`набор: ${files.length} fixture-файлов (live-* исключены)`, ...browserPreflight()],
  };
}

/**
 * Live-фаза T3: без `DISCORD_LAB_LIVE_CDP` — unmet (не запуск и не skip). Только qualify.
 * @type {Phase}
 */
const e2eLivePhase = {
  name: "e2e-live",
  command: "node",
  args: ["--test", "--test-concurrency=1", "tests/e2e/live-discord.e2e.test.mjs"],
  requiresEnv: "DISCORD_LAB_LIVE_CDP",
  failOnSkips: true,
  preflight: () => [
    process.env.DISCORD_LAB_LIVE_CDP
      ? `live endpoint: ${process.env.DISCORD_LAB_LIVE_CDP}`
      : "unmet: DISCORD_LAB_LIVE_CDP не задан — live-проверка не выполнялась",
  ],
};

/** @returns {PhaseResult} */
function goldenPhase() {
  const startedMs = Date.now();
  const manifest = join(gateDir, "golden.manifest");
  if (!existsSync(manifest)) {
    return {
      name: "golden", ok: true, unmet: false, ms: 0, counts: null, tail: "",
      note: "манифеста нет — сверка пропущена (npm run golden:freeze)",
    };
  }
  const result = runCommand("md5sum", ["-c", ".gate/golden.manifest"], { timeoutMs: 300000 });
  return {
    name: "golden", ok: result.ok, unmet: false, ms: Date.now() - startedMs, counts: null,
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
    name: "pack", ok, unmet: false, ms: Date.now() - startedMs, counts: null, tail: report, note: null,
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
  const record = /** @type {{ tarball?: string, sha256?: string, gitCommit?: string | null, verdict?: string, acceptance?: { accepted?: boolean } | null }} */ (
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
  // Приёмка — только `passed` и подтверждённая acceptance; `passed_with_skips`/`unmet` не годятся.
  const acceptance = /** @type {{ accepted?: boolean } | undefined} */ (record.acceptance);
  const verdictOk = acceptance ? acceptance.accepted === true : record.verdict === "passed";
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
  const preflight = phase.preflight ? phase.preflight() : [];
  // Живая фаза без среды — unmet: не запускаем и не выдаём skip за прохождение.
  if (phase.requiresEnv && !process.env[phase.requiresEnv]) {
    return {
      name: phase.name,
      ok: false,
      unmet: true,
      skipKind: phase.skipKind ?? null,
      ms: 0,
      counts: null,
      attempts: [],
      tail: "",
      note: [...preflight, `unmet: ${phase.requiresEnv} не задан — live-проверка не выполнялась`].join("; "),
    };
  }
  /** @type {PhaseAttempt[]} */
  const attempts = [];
  const maxAttempts = 1 + (phase.retries ?? 0);
  let last = null;
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    last = runCommand(phase.command ?? "", phase.args ?? [], {
      cwd: phase.cwd,
      timeoutMs: phase.timeoutMs,
      env: phase.env,
    });
    const counts = phase.countsKind === "cargo" ? cargoTestCounts(last.output) : testCounts(last.output);
    const outcome = phaseOutcome(phase, counts, last.ok);
    // Попытка сохраняется целиком (счётчики, причина, свой вывод): повтор не прячет первый прогон.
    attempts.push({
      attempt,
      ok: outcome.ok,
      status: last.status,
      ms: last.ms,
      counts,
      reasons: outcome.reasons,
      tail: tail(last.output, 20),
    });
    if (last.ok) break;
    if (attempt < maxAttempts) console.error(`  ↻ повтор фазы ${phase.name} (попытка ${attempt + 1} из ${maxAttempts})`);
  }
  const finalAttempt = attempts[attempts.length - 1];
  const skipped = finalAttempt?.counts?.skipped ?? 0;
  // failOnSkips: для e2e/live-наборов skip — не «нет среды», а невыполненная проверка.
  const skipFailure = Boolean(phase.failOnSkips) && skipped > 0;
  const outcome = phaseOutcome(phase, finalAttempt?.counts ?? null, Boolean(finalAttempt?.ok));
  const notes = [...preflight];
  if (attempts.length > 1) {
    notes.push(`попыток: ${attempts.length}; первый прогон: ${attempts[0]?.ok ? "ok" : "fail"}`);
  }
  notes.push(...outcome.reasons);
  return {
    name: phase.name,
    ok: Boolean(finalAttempt?.ok) && !skipFailure && !outcome.unmet,
    unmet: false,
    skipKind: phase.skipKind ?? null,
    ms: attempts.reduce((sum, item) => sum + item.ms, 0),
    counts: finalAttempt?.counts ?? null,
    attempts,
    tail: tail(last?.output ?? "", finalAttempt?.ok ? 5 : 60),
    note: notes.length > 0 ? notes.join("; ") : null,
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
  worker   типы → ui-tsc → мета → unit → Rust (workspace + gui-spike) → smoke (быстрый, для воркеров)
  release  worker + e2e fixture (failOnSkips, последовательно) + покрытие + golden + keyring-live (LIVE=1)
  qualify  release + e2e-live (DISCORD_LAB_LIVE_CDP, без него — unmet) + tarball (запись qualification)

Отчёты: .gate/<scope>-<timestamp>.json; qualification — .gate/qualification-<version>-<timestamp>.json.
«Пропуск прохождением не считается»: skip попадают в warnings отчёта, а в e2e/live-фазах
(failOnSkips) дают failed. Живая фаза без env — unmet, а не skip и не pass.
Приёмка — по полям e2e.fixture/e2e.live и acceptance, а не по вердикту.`);
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
    // Fixture e2e (T2): обязательная фаза с failOnSkips; live-* в неё не входит.
    phases.push(e2eFixturePhase());
    phases.push({ name: "coverage", command: "npm", args: ["run", "test:coverage"] });
    phases.push(keyringLivePhase);
    phases.push({ name: "golden", run: goldenPhase });
  }
  if (options.scope === "qualify") {
    // T3: живой Discord — отдельная фаза; без DISCORD_LAB_LIVE_CDP она unmet.
    phases.push(e2eLivePhase);
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
    const mark = result.unmet ? "⚪" : result.ok ? "✔" : "✖";
    console.log(`${mark} ${result.name} — ${(result.ms / 1000).toFixed(1)} с${countPart}${notePart}`);
    if (!result.ok || result.name === "pack") {
      console.log(result.tail.split("\n").map((line) => `   ${line}`).join("\n"));
    }
    const skipped = result.counts?.skipped ?? 0;
    if (skipped > 0) {
      totalSkipped += skipped;
      warnings.push(result.skipKind === "ignored"
        ? `${result.name}: ignored ${skipped} — ожидаемый ignored-набор (live-кейсы гоняет отдельная фаза), не e2e-skip`
        : `${result.name}: пропущено ${skipped} тестов — пропуск прохождением не считается`);
    }
  }

  const verdict = computeVerdict(results);
  const fixtureE2e = options.scope === "worker" ? "absent" : e2eStatus(results, "e2e");
  const liveE2e = results.some((result) => result.name === "e2e-live")
    ? e2eStatus(results, "e2e-live")
    : "absent";
  const acceptance = computeAcceptance(
    /** @type {"worker" | "release" | "qualify"} */ (options.scope), verdict, fixtureE2e, liveE2e,
  );
  /** Ожидаемые Rust ignored-наборы: видны в отчёте, но не смешаны с e2e-пропусками. */
  const ignored = results
    .filter((result) => result.skipKind === "ignored" && (result.counts?.skipped ?? 0) > 0)
    .map((result) => ({ phase: result.name, skipped: result.counts?.skipped ?? 0 }));
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
    e2e: options.scope === "worker" ? null : { fixture: fixtureE2e, live: liveE2e },
    acceptance,
    ignored,
    totalSkipped,
    warnings,
    phases: results.map((result) => ({
      name: result.name,
      ok: result.ok,
      unmet: result.unmet,
      skipKind: result.skipKind ?? null,
      ms: result.ms,
      counts: result.counts,
      note: result.note,
      extra: result.extra ?? null,
      attempts: result.attempts ?? [],
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
  if (record.e2e) console.log(`e2e: fixture=${fixtureE2e}, live=${liveE2e}`);
  console.log(`Вердикт: ${verdict}`);
  console.log(acceptance
    ? `Приёмка: ${acceptance.accepted ? "принято" : `НЕ принято — ${acceptance.reason}`}`
    : "Приёмка: н/д (worker-гейт)");
  const accepted = acceptance === null || acceptance.accepted;
  return results.every((result) => result.ok || result.unmet === true) && accepted ? 0 : 1;
}

try {
  process.exitCode = main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
