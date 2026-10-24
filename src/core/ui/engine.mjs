// @ts-check
// Движок UI-прогона (walk): шаг = клик по пункту навигации + скриншот + профиль.
// Контракт: QUALITY/WALK-PLAN.md §1, tests/unit/walk.test.mjs.
//
// Запрещено (см. §1.3 плана): Page.navigate, Target.*, Input.*, Network.*,
// Emulation.*, Runtime.terminateExecution. Движок не закрывает CDP — это делает CLI.

import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { captureScreenshot, evaluate, listElements } from "../cdp/client.mjs";
import { relativeToRoot, screenshotName } from "../paths.mjs";
import { clickNavItemExpression } from "./adapters.mjs";
import { isBlacklisted } from "./blacklist.mjs";
import { topFunctions } from "./profile.mjs";
import { buildWalkRecord, consoleIssues, parseVisited, pickNextItem, shouldBail } from "./walk-log.mjs";

/** @typedef {import("../types.js").WalkRecord} WalkRecord */
/** @typedef {import("../types.js").NavItem} NavItem */
/** @typedef {import("../types.js").CdpEvent} CdpEvent */
/** @typedef {{ name: string, navSelector: string, screenshotPrefix: string }} WalkAdapter */
/** @typedef {import("../types.js").CdpEventClient & {
 *   mark?: () => number,
 *   eventsSince?: (mark: number) => CdpEvent[],
 * }} WalkCdp */

/**
 * Итог прогона.
 * @typedef {object} WalkResult
 * @property {number} code код выхода (0/1/2 уровня движка)
 * @property {WalkRecord[]} records записанные шаги
 * @property {string[]} skipped пропущенные подписи (запрещённые/уже пройденные)
 * @property {number} steps число записанных шагов
 */

/**
 * Настройки прогона.
 * @typedef {object} WalkOptions
 * @property {WalkCdp} cdp открытая CDP-сессия страницы
 * @property {WalkAdapter} adapter адаптер приложения (fixture)
 * @property {string} outPath журнал JSONL (append после каждого шага)
 * @property {string} screensDir каталог PNG-скриншотов
 * @property {number} [settleMs] пауза после клика (по умолчанию 250)
 * @property {boolean} [profile] снимать CPU-профиль (по умолчанию true)
 * @property {number} [maxSteps] предохранитель числа шагов (по умолчанию 200)
 * @property {boolean} [fresh] игнорировать существующий журнал и усечь его
 * @property {(line: string) => void} [log] stdout-строка прогона
 * @property {(line: string) => void} [warn] stderr-предупреждение
 * @property {(ms: number) => Promise<void>} [sleep] пауза (в тестах — no-op)
 */

/** @param {number} ms @returns {Promise<void>} */
const defaultSleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * @param {unknown} error
 * @returns {string}
 */
function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * @param {unknown} error
 * @returns {boolean} ошибка «файла ещё нет»
 */
function isMissing(error) {
  return typeof error === "object" && error !== null && "code" in error && /** @type {any} */ (error).code === "ENOENT";
}

/**
 * @param {Buffer} bytes
 * @returns {boolean} сигнатура PNG (дешёвая защита от пустого ответа CDP)
 */
function isPng(bytes) {
  return bytes.length >= 8 &&
    bytes[0] === 0x89 && bytes[1] === 0x50 && bytes[2] === 0x4e && bytes[3] === 0x47;
}

/**
 * Строка stdout по шагу (паритет ui-walk-settings2).
 * @param {WalkRecord} record
 * @returns {string}
 */
function stepLine(record) {
  const top = record.topFunctions.slice(0, 3).map((item) => item.fn).join(", ");
  const suffix = record.error === null ? "" : ` | ОШИБКА: ${record.error}`;
  return `[${String(record.step).padStart(2, "0")}] ${record.label} | клик=${record.clicked} | ошибок: ${record.consoleErrors} | топ: ${top}${suffix}`;
}

/**
 * Выполняет UI-прогон по текущей странице.
 *
 * Шаг: `Runtime.evaluate` (список) → `Profiler.start` → `Runtime.evaluate` (клик) →
 * `sleep(settleMs)` → `Profiler.stop` → `Page.captureScreenshot` → запись PNG + JSONL.
 * `Runtime.enable` выполняется до первого клика (нужен для console-событий).
 * @param {WalkOptions} options
 * @returns {Promise<WalkResult>}
 */
export async function runWalk(options) {
  const {
    cdp,
    adapter,
    outPath,
    screensDir,
    settleMs = 250,
    profile = true,
    maxSteps = 200,
    fresh = false,
    log = () => {},
    warn = () => {},
    sleep = defaultSleep,
  } = options;

  /** @type {WalkRecord[]} */
  const records = [];
  /** @type {string[]} */
  const skipped = [];

  /** @param {number} code @returns {WalkResult} */
  const done = (code) => ({ code, records, skipped, steps: records.length });

  try {
    await mkdir(dirname(outPath), { recursive: true });
    await mkdir(screensDir, { recursive: true });
  } catch (error) {
    warn(`walk: не удалось подготовить каталоги: ${errorMessage(error)}`);
    return done(2);
  }

  /** @type {Set<string>} */
  let visited = new Set();
  let maxStep = 0;
  try {
    if (fresh) {
      await writeFile(outPath, "");
    } else {
      const state = parseVisited(await readFile(outPath, "utf8"));
      visited = state.labels;
      maxStep = state.maxStep;
    }
  } catch (error) {
    if (!(fresh === false && isMissing(error))) {
      warn(`walk: журнал недоступен (${outPath}): ${errorMessage(error)}`);
      return done(2);
    }
  }
  // «Уже пройдено» печатается только для шагов из журнала (resume),
  // а не для пунктов, посещённых в этом же прогоне.
  const preVisited = new Set(visited);

  try {
    await cdp.send("Runtime.enable");
    if (profile) await cdp.send("Profiler.enable");
    await cdp.send("Page.enable");
  } catch (error) {
    warn(`walk: ошибка CDP: ${errorMessage(error)}`);
    return done(2);
  }

  /** @type {Set<string>} */
  const reported = new Set();
  let performed = 0;
  let firstQuery = true;
  let code = 0;

  for (;;) {
    /** @type {NavItem[]} */
    let navItems;
    try {
      const items = await listElements(cdp, adapter.navSelector);
      navItems = items.map((item) => ({ text: item.text })).filter((item) => item.text.length > 0);
    } catch (error) {
      warn(`walk: ошибка CDP: ${errorMessage(error)}`);
      return done(2);
    }

    if (navItems.length === 0) {
      if (firstQuery) {
        warn(`walk: 0 пунктов навигации по селектору ${adapter.navSelector}`);
        return done(2);
      }
      break;
    }
    firstQuery = false;

    for (const item of navItems) {
      if (reported.has(item.text)) continue;
      if (isBlacklisted(item.text)) {
        skipped.push(item.text);
        reported.add(item.text);
        log(`ПРОПУСК (запрещено): ${item.text}`);
      } else if (preVisited.has(item.text)) {
        skipped.push(item.text);
        reported.add(item.text);
        log(`ПРОПУСК (уже пройдено): ${item.text}`);
      }
    }

    const next = pickNextItem(navItems, visited, isBlacklisted);
    if (!next) break;
    if (performed >= maxSteps) {
      warn(`walk: достигнут предел --max-steps (${maxSteps}); прогон остановлен`);
      break;
    }
    performed += 1;

    const step = maxStep + 1;
    visited.add(next.text);
    const record = buildWalkRecord(step, next.text);

    let profilerStarted = false;
    if (profile) {
      try {
        await cdp.send("Profiler.start");
        profilerStarted = true;
      } catch (error) {
        record.error = errorMessage(error);
      }
    }

    const mark = cdp.mark ? cdp.mark() : cdp.events.length;
    if (record.error === null) {
      try {
        const outcome = await evaluate(cdp, clickNavItemExpression(adapter.navSelector, next.text));
        record.clicked = outcome?.clicked === true;
        if (!record.clicked) {
          record.error = typeof outcome?.reason === "string" ? outcome.reason : "label not found";
        }
      } catch (error) {
        record.error = errorMessage(error);
      }
    }

    let bail = shouldBail(record.error);
    let shotFailed = false;
    if (!bail) {
      await sleep(settleMs);
      if (profilerStarted) {
        try {
          const stopped = await cdp.send("Profiler.stop");
          record.topFunctions = topFunctions(stopped?.profile ?? { nodes: [] }, 8);
        } catch (error) {
          if (record.error === null) record.error = errorMessage(error);
          bail = shouldBail(record.error);
        }
      }
      if (!bail) {
        try {
          const bytes = await captureScreenshot(cdp);
          if (!isPng(bytes)) throw new Error("CDP вернул не PNG");
          const path = join(screensDir, screenshotName(adapter.screenshotPrefix, step, record.label));
          await writeFile(path, bytes);
          record.screenshot = relativeToRoot(path);
        } catch (error) {
          record.error = `скриншот: ${errorMessage(error)}`;
          record.screenshot = null;
          shotFailed = true;
        }
      }
    }

    const events = cdp.eventsSince ? cdp.eventsSince(mark) : cdp.events.slice(mark);
    const issues = consoleIssues(events);
    record.consoleErrors = issues.count;
    record.consoleErrorTexts = issues.texts;

    try {
      await appendFile(outPath, `${JSON.stringify(record)}\n`);
    } catch (error) {
      warn(`walk: ошибка записи журнала (${outPath}): ${errorMessage(error)}`);
      return done(2);
    }
    records.push(record);
    maxStep = step;
    log(stepLine(record));

    if (shotFailed || bail) {
      code = 2;
      break;
    }
  }

  const failedSteps = records.filter((record) => record.error !== null).length;
  if (failedSteps > 0) {
    warn(`Шагов с ошибкой: ${failedSteps}`);
    code = 2;
  }
  return done(code);
}
