#!/usr/bin/env node
// @ts-check
// RW-36 / B07 (P0-01): замер RSS/HWM/Threads GUI-спайка Tauri по /proc/<pid>/status.
// Контракт — waves/RW-36/TZ.md §3. Модуль только читает /proc, в процесс не пишет.
//
//   node scripts/gui-spike-bench.mjs --pid <PID> [--duration-ms 5000]
//        [--interval-ms 200] [--json out.json] [--tree]
//
// --tree: суммарный RSS по дереву процессов (родитель + WebKitWebProcess/
// WebKitNetworkProcess и прочие потомки рекурсивно). В JSON добавляются только
// в этом режиме поля `processes` (максимум процессов за замер) и `skipped`
// (суммарно пропущенные/неполные чтения /proc). Без --tree контракт JSON не меняется.
//
// Коды выхода: 0 — успех, 1 — ошибка аргументов, 2 — среда/процесс недоступны.
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Один снимок /proc/<pid>/status (или агрегат по дереву в режиме --tree).
 * @typedef {{
 *   rss_kb: number,
 *   hwm_kb: number,
 *   threads: number,
 *   t_ms: number,
 *   processes?: number,
 *   skipped?: number,
 * }} BenchSample
 * t_ms — смещение от начала замера, мс. processes/skipped — только для --tree.
 */

/** @typedef {{ min: number, max: number, avg: number, p95: number }} RssStats */
/**
 * @typedef {{
 *   samples: number,
 *   rss_kb: RssStats,
 *   hwm_kb: number,
 *   threads_max: number,
 *   duration_ms: number,
 * }} BenchSummary
 */

export const EXIT_OK = 0;
export const EXIT_ARGS = 1;
export const EXIT_ENV = 2;

const DEFAULT_DURATION_MS = 5000;
const DEFAULT_INTERVAL_MS = 200;
const MAX_DURATION_MS = 3_600_000;
const MAX_INTERVAL_MS = 60_000;

/** Буфер для синхронного ожидания между сэмплами (без таймеров и зависимостей). */
const sleeper = new Int32Array(new SharedArrayBuffer(4));

/** @param {number} ms */
function sleep(ms) {
  Atomics.wait(sleeper, 0, 0, ms);
}

/**
 * p95 по nearest-rank: ceil(0.95 * n) - 1 в отсортированном массиве.
 * Для n=1 даёт единственный элемент, для n=20 — 19-й (не максимум).
 * Эталон для UI: `ui/app.js` должен использовать ту же формулу, иначе p95 кадра
 * в шапке и в metrics.json несравнимы (ревью P2-6).
 * @param {number[]} sorted неубывающий массив; порядок не проверяется
 * @returns {number}
 */
export function percentile95(sorted) {
  if (sorted.length === 0) return 0;
  const rank = Math.ceil(0.95 * sorted.length);
  return sorted[Math.max(0, rank - 1)] ?? 0;
}

/**
 * Чистая сводка по сэмплам: без чтения /proc, без побочных эффектов.
 * Пустой вход → нули (а не исключение): спайк мог не успеть снять ни одного сэмпла.
 * duration_ms — размах между первым и последним сэмплом.
 * @param {ReadonlyArray<BenchSample>} samples
 * @returns {BenchSummary}
 */
export function summarize(samples) {
  if (samples.length === 0) {
    return {
      samples: 0,
      rss_kb: { min: 0, max: 0, avg: 0, p95: 0 },
      hwm_kb: 0,
      threads_max: 0,
      duration_ms: 0,
    };
  }

  let min = Number.POSITIVE_INFINITY;
  let max = Number.NEGATIVE_INFINITY;
  let sum = 0;
  let hwm = 0;
  let threadsMax = 0;
  /** @type {number[]} */
  const rss = [];
  for (const sample of samples) {
    rss.push(sample.rss_kb);
    if (sample.rss_kb < min) min = sample.rss_kb;
    if (sample.rss_kb > max) max = sample.rss_kb;
    sum += sample.rss_kb;
    if (sample.hwm_kb > hwm) hwm = sample.hwm_kb;
    if (sample.threads > threadsMax) threadsMax = sample.threads;
  }

  rss.sort((left, right) => left - right);
  const first = samples[0];
  const last = samples[samples.length - 1];
  return {
    samples: samples.length,
    rss_kb: { min, max, avg: Math.round(sum / samples.length), p95: percentile95(rss) },
    hwm_kb: hwm,
    threads_max: threadsMax,
    duration_ms: (last?.t_ms ?? 0) - (first?.t_ms ?? 0),
  };
}

/**
 * Разбирает текст /proc/<pid>/status. VmRSS/VmHWM — в kB, Threads — штуки.
 * @param {string} text
 * @returns {{ rss_kb: number, hwm_kb: number, threads: number }}
 */
export function parseStatus(text) {
  /** @param {string} name @returns {number|null} */
  const field = (name) => {
    const match = text.match(new RegExp(`^${name}:\\s+(\\d+)(?:\\s+kB)?\\s*$`, "m"));
    const raw = match?.[1];
    return raw === undefined ? null : Number.parseInt(raw, 10);
  };
  const rss = field("VmRSS");
  const hwm = field("VmHWM");
  const threads = field("Threads");
  if (rss === null || hwm === null || threads === null) {
    const missing = [
      rss === null ? "VmRSS" : "",
      hwm === null ? "VmHWM" : "",
      threads === null ? "Threads" : "",
    ].filter(Boolean).join(", ");
    throw new Error(`в /proc/<pid>/status нет полей: ${missing}`);
  }
  return { rss_kb: rss, hwm_kb: hwm, threads };
}

/**
 * @param {number} pid
 * @returns {{ rss_kb: number, hwm_kb: number, threads: number }}
 */
function readStatus(pid) {
  const path = `/proc/${pid}/status`;
  /** @type {string} */
  let text;
  try {
    text = readFileSync(path, "utf8");
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`${path} недоступен: ${reason}`);
  }
  try {
    return parseStatus(text);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`${path}: ${reason}`);
  }
}

/**
 * Дочерние PID процесса из /proc/<pid>/task/<task>/children по всем потокам:
 * потомка мог породить любой поток, а не только главный.
 * `incomplete` — хотя бы один список детей прочитать не удалось (процесс/поток
 * завершился, урезанный /proc): дерево могло быть неполным, это идёт в skipped,
 * чтобы не занизить RAM молча (ревью P1-3).
 * @param {number} pid
 * @returns {{ children: number[], incomplete: boolean }}
 */
function readChildren(pid) {
  /** @type {string[]} */
  let tasks;
  let incomplete = false;
  try {
    tasks = readdirSync(`/proc/${pid}/task`);
  } catch {
    tasks = [String(pid)]; // /proc урезан: пробуем хотя бы главный поток
    incomplete = true;
  }
  /** @type {number[]} */
  const children = [];
  for (const task of tasks) {
    try {
      const text = readFileSync(`/proc/${pid}/task/${task}/children`, "utf8");
      for (const raw of text.split(/\s+/)) {
        const child = Number.parseInt(raw, 10);
        if (Number.isSafeInteger(child) && child > 0) children.push(child);
      }
    } catch {
      // Поток завершился между readdir и чтением — дерево могло быть неполным.
      incomplete = true;
    }
  }
  return { children, incomplete };
}

/**
 * Агрегирует статусы процессов дерева в один снимок.
 * rss_kb — сумма RSS (стек = все процессы), hwm_kb — максимум VmHWM (пиковый RSS
 * разных процессов не складывается, иначе память завышается), threads — сумма.
 * @param {ReadonlyArray<{ rss_kb: number, hwm_kb: number, threads: number }>} statuses
 * @returns {{ rss_kb: number, hwm_kb: number, threads: number }}
 */
export function aggregateTree(statuses) {
  let rss = 0;
  let hwm = 0;
  let threads = 0;
  for (const status of statuses) {
    rss += status.rss_kb;
    threads += status.threads;
    if (status.hwm_kb > hwm) hwm = status.hwm_kb;
  }
  return { rss_kb: rss, hwm_kb: hwm, threads };
}

/**
 * Агрегат по дереву процессов: pid + все потомки рекурсивно.
 *
 * Семантика метрик — в aggregateTree: RSS суммируется (именно это включает
 * WebKitWebProcess/NetworkProcess в метрику стека), HWM берётся максимумом.
 *
 * Недоступный /proc потомка (гонка с его exit, урезанный /proc) не валит замер:
 * пропуск учитывается в skipped, чтобы метрика не занижалась молча (best-effort,
 * см. TZ §3). Корневой PID недоступен → ошибка наверх (exit 2), как и без --tree.
 * @param {number} pid
 * @returns {{ rss_kb: number, hwm_kb: number, threads: number, processes: number, skipped: number }}
 */
export function readTreeStatus(pid) {
  /** @type {Array<{ rss_kb: number, hwm_kb: number, threads: number }>} */
  const statuses = [];
  let skipped = 0;
  /** @type {Set<number>} */
  const seen = new Set();
  /** @type {number[]} */
  const stack = [pid];
  while (stack.length > 0) {
    const current = stack.pop();
    if (current === undefined || seen.has(current)) continue;
    seen.add(current);
    // Потомков читаем до status: если сам процесс уже умер, его дети всё равно
    // должны попасть в обход (и в skipped), а не выпасть из дерева молча.
    const { children, incomplete } = readChildren(current);
    try {
      statuses.push(readStatus(current));
    } catch (error) {
      if (current === pid) throw error;
      skipped += 1; // процесс исчез между обходом и чтением
      stack.push(...children);
      continue;
    }
    if (incomplete) skipped += 1; // счётчики прочитаны, но список детей — нет
    stack.push(...children);
  }
  return { ...aggregateTree(statuses), processes: statuses.length, skipped };
}

/**
 * Снимает сэмплы до истечения durationMs: первый — сразу, затем раз в intervalMs.
 * @param {number} pid
 * @param {number} durationMs
 * @param {number} intervalMs
 * @param {boolean} tree
 * @returns {BenchSample[]}
 */
function collectSamples(pid, durationMs, intervalMs, tree) {
  const started = performance.now();
  /** @type {BenchSample[]} */
  const samples = [];
  for (;;) {
    const status = tree ? readTreeStatus(pid) : readStatus(pid);
    const elapsed = Math.round(performance.now() - started);
    samples.push({ ...status, t_ms: elapsed });
    const remaining = durationMs - elapsed;
    if (remaining <= 0) break;
    sleep(Math.max(1, Math.min(intervalMs, remaining)));
  }
  return samples;
}

/**
 * @param {string} raw
 * @param {string} flag
 * @param {number} max
 * @returns {number}
 */
function positiveInt(raw, flag, max) {
  if (!/^\d+$/.test(raw)) throw new Error(`${flag}: ожидается целое число, получено «${raw}»`);
  const value = Number.parseInt(raw, 10);
  if (!Number.isSafeInteger(value) || value < 1 || value > max) {
    throw new Error(`${flag}: допустимый диапазон 1..${max}, получено ${raw}`);
  }
  return value;
}

/**
 * @typedef {{
 *   frontend_ready_ms?: number,
 *   chat?: { frames?: number, avgMs?: number, p95Ms?: number, fps?: number },
 *   members?: { frames?: number, avgMs?: number, p95Ms?: number, fps?: number },
 * }} BenchLog
 */

/**
 * Извлекает последний `BENCH {...}` из app.log (его пишет ui/app.js).
 * Нет строки → null (autobench не запускался); битый JSON → ошибка, а не тихий null.
 * @param {string} text
 * @returns {BenchLog | null}
 */
export function parseBenchLog(text) {
  const lines = text.split(/\r?\n/).filter((line) => line.startsWith("BENCH "));
  const last = lines.at(-1);
  if (last === undefined) return null;
  const raw = last.slice("BENCH ".length);
  /** @type {unknown} */
  let value;
  try {
    value = JSON.parse(raw);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`BENCH: некорректный JSON: ${reason}`);
  }
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("BENCH: ожидается JSON-объект");
  }
  return /** @type {BenchLog} */ (value);
}

/**
 * @param {string[]} argv
 * @returns {{ pid: number, durationMs: number, intervalMs: number, jsonPath: string|null, tree: boolean, benchLogPath: string|null }}
 */
export function parseCliArgs(argv) {
  /** @type {number|null} */
  let pid = null;
  let durationMs = DEFAULT_DURATION_MS;
  let intervalMs = DEFAULT_INTERVAL_MS;
  /** @type {string|null} */
  let jsonPath = null;
  /** @type {string|null} */
  let benchLogPath = null;
  let tree = false;

  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index] ?? "";
    /** @param {string} name @returns {string} */
    const value = (name) => {
      const next = argv[index + 1];
      if (next === undefined) throw new Error(`${name}: не указано значение`);
      index += 1;
      return next;
    };
    if (flag === "--pid") pid = positiveInt(value("--pid"), "--pid", 2 ** 22);
    else if (flag === "--duration-ms") durationMs = positiveInt(value(flag), flag, MAX_DURATION_MS);
    else if (flag === "--interval-ms") intervalMs = positiveInt(value(flag), flag, MAX_INTERVAL_MS);
    else if (flag === "--tree") tree = true;
    else if (flag === "--json") {
      const path = value(flag);
      if (path.trim() === "") throw new Error("--json: пустой путь");
      jsonPath = resolve(path);
    } else if (flag === "--bench-log") {
      const path = value(flag);
      if (path.trim() === "") throw new Error("--bench-log: пустой путь");
      benchLogPath = resolve(path);
    } else throw new Error(`неизвестный аргумент: ${flag}`);
  }

  if (pid === null) throw new Error("обязательный аргумент --pid <PID> не задан");
  return { pid, durationMs, intervalMs, jsonPath, tree, benchLogPath };
}

/**
 * @param {string[]} argv
 * @returns {number} код выхода
 */
function main(argv) {
  /** @type {ReturnType<typeof parseCliArgs>} */
  let options;
  try {
    options = parseCliArgs(argv);
  } catch (error) {
    console.error(`gui-spike-bench: ${error instanceof Error ? error.message : String(error)}`);
    return EXIT_ARGS;
  }

  /** @type {BenchSample[]} */
  let samples;
  try {
    samples = collectSamples(options.pid, options.durationMs, options.intervalMs, options.tree);
  } catch (error) {
    // Процесс завершился или /proc закрыт: замер невалиден, это среда, а не аргументы.
    console.error(`gui-spike-bench: ${error instanceof Error ? error.message : String(error)}`);
    console.error("gui-spike-bench: замер невозможен, процесс или /proc недоступны (skip-aware)");
    return EXIT_ENV;
  }

  /** @type {{ pid: number } & BenchSummary & { processes?: number, skipped?: number, bench?: BenchLog | null }} */
  let result = { pid: options.pid, ...summarize(samples) };
  if (options.tree) {
    // Состав дерева меняется между сэмплами (WebKit доспавнивает процессы),
    // поэтому фиксируем максимум, а не последний снимок: цель — не занизить стек.
    // skipped — суммарные пропуски чтений /proc по всем сэмплам.
    let processes = 0;
    let skipped = 0;
    for (const sample of samples) {
      processes = Math.max(processes, sample.processes ?? 0);
      skipped += sample.skipped ?? 0;
    }
    result = { ...result, processes, skipped };
  }
  if (options.benchLogPath !== null) {
    /** @type {string} */
    let text;
    try {
      text = readFileSync(options.benchLogPath, "utf8");
    } catch (error) {
      console.error(`gui-spike-bench: не удалось прочитать ${options.benchLogPath}: ${
        error instanceof Error ? error.message : String(error)}`);
      return EXIT_ENV;
    }
    /** @type {BenchLog | null} */
    let bench;
    try {
      bench = parseBenchLog(text);
    } catch (error) {
      console.error(`gui-spike-bench: ${error instanceof Error ? error.message : String(error)}`);
      return EXIT_ENV;
    }
    if (bench === null) {
      console.error("gui-spike-bench: в логе нет строки BENCH — FPS/p95 не зафиксированы");
      return EXIT_ARGS;
    }
    result = { ...result, bench };
  }
  const json = JSON.stringify(result);
  if (options.jsonPath !== null) {
    try {
      writeFileSync(options.jsonPath, `${json}\n`);
    } catch (error) {
      console.error(`gui-spike-bench: не удалось записать ${options.jsonPath}: ${
        error instanceof Error ? error.message : String(error)}`);
      return EXIT_ENV;
    }
  }
  console.log(json);
  return EXIT_OK;
}

const invokedDirectly = process.argv[1] !== undefined
  && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  process.exitCode = main(process.argv.slice(2));
}
