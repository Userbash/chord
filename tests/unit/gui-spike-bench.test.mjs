// @ts-check
// RED (RW-36 / B07, P0-01): реализация — scripts/gui-spike-bench.mjs.
// Контракт (waves/RW-36/TZ.md §3): summarize(samples) — чистая функция, p95 по
// nearest-rank (ceil(0.95*n)-1 в отсортированном массиве), пустой вход → нули.
// CLI: exit 0 успех, 1 ошибка аргументов, 2 среда/процесс недоступны.
//
// RW-36 / B07 (W36b, ревью P1-3/P2-6/P2-9): режим --tree (сумма RSS по дереву
// процессов), прямой контракт percentile95 и табличные тесты parseStatus.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  aggregateTree,
  parseBenchLog,
  parseCliArgs,
  parseStatus,
  percentile95,
  summarize,
} from "../../scripts/gui-spike-bench.mjs";

const script = fileURLToPath(new URL("../../scripts/gui-spike-bench.mjs", import.meta.url));

/**
 * @param {number[]} rss
 * @returns {Array<{rss_kb:number,hwm_kb:number,threads:number,t_ms:number}>}
 */
function samples(rss) {
  return rss.map((value, index) => ({
    rss_kb: value,
    hwm_kb: 4096,
    threads: 7,
    t_ms: index * 100,
  }));
}

/** @param {string[]} args */
function runCli(args) {
  return spawnSync(process.execPath, [script, ...args], {
    encoding: "utf8",
    timeout: 10_000,
  });
}

/**
 * Запускает отдельный node-процесс с заданным кодом и ждёт события `spawn`,
 * чтобы успеть получить pid до того, как процесс что-то сделает.
 * @param {string} code
 * @returns {Promise<import("node:child_process").ChildProcess>}
 */
function startNode(code) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(process.execPath, ["-e", code], { stdio: "ignore" });
    child.once("spawn", () => resolvePromise(child));
    child.once("error", rejectPromise);
  });
}

/**
 * Ждёт появления хотя бы одного потомка у pid (best-effort, не дольше timeoutMs).
 * @param {number} pid
 * @param {number} timeoutMs
 * @returns {Promise<boolean>}
 */
async function waitForChildren(pid, timeoutMs) {
  const path = `/proc/${pid}/task/${pid}/children`;
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const children = readFileSync(path, "utf8").trim();
      if (children !== "") return true;
    } catch {
      // /proc может не поддерживать children — тогда тест скипнется по false.
      return false;
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 25));
  }
  return false;
}

/** @param {import("node:child_process").ChildProcess} child */
function killQuietly(child) {
  if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL");
}

describe("summarize", () => {
  it("обычный набор: min/max/avg/p95, hwm, threads_max, duration_ms", () => {
    const summary = summarize(samples([100, 200, 300, 400, 500]));
    assert.equal(summary.samples, 5);
    assert.deepEqual(summary.rss_kb, { min: 100, max: 500, avg: 300, p95: 500 });
    assert.equal(summary.hwm_kb, 4096);
    assert.equal(summary.threads_max, 7);
    assert.equal(summary.duration_ms, 400);
  });

  it("пустой вход не бросает и отдаёт нули", () => {
    const summary = summarize([]);
    assert.equal(summary.samples, 0);
    assert.deepEqual(summary.rss_kb, { min: 0, max: 0, avg: 0, p95: 0 });
    assert.equal(summary.hwm_kb, 0);
    assert.equal(summary.threads_max, 0);
    assert.equal(summary.duration_ms, 0);
  });

  it("один элемент: все статистики равны ему, duration 0", () => {
    const summary = summarize(samples([777]));
    assert.equal(summary.samples, 1);
    assert.deepEqual(summary.rss_kb, { min: 777, max: 777, avg: 777, p95: 777 });
    assert.equal(summary.duration_ms, 0);
  });

  it("p95 по nearest-rank: 20 значений → 19-е, не максимум", () => {
    const values = Array.from({ length: 20 }, (_, index) => (index + 1) * 10);
    assert.equal(summarize(samples(values)).rss_kb.p95, 190);
  });

  it("p95 на 100 значениях 1..100 → 95", () => {
    const values = Array.from({ length: 100 }, (_, index) => index + 1);
    const summary = summarize(samples(values));
    assert.equal(summary.rss_kb.p95, 95);
    assert.equal(summary.rss_kb.min, 1);
    assert.equal(summary.rss_kb.max, 100);
    assert.equal(summary.rss_kb.avg, 51);
  });

  it("порядок входа не влияет на статистики и вход не мутируется", () => {
    const input = samples([300, 100, 500, 200, 400]);
    const copy = structuredClone(input);
    const summary = summarize(input);
    assert.deepEqual(summary.rss_kb, { min: 100, max: 500, avg: 300, p95: 500 });
    assert.deepEqual(input, copy);
  });

  it("avg округляется до целого kB, hwm и threads берутся максимумом", () => {
    const input = [
      { rss_kb: 100, hwm_kb: 900, threads: 3, t_ms: 0 },
      { rss_kb: 101, hwm_kb: 1200, threads: 11, t_ms: 200 },
    ];
    const summary = summarize(input);
    assert.equal(summary.rss_kb.avg, 101);
    assert.equal(summary.hwm_kb, 1200);
    assert.equal(summary.threads_max, 11);
  });
});

describe("CLI: коды выхода", () => {
  it("без --pid → exit 1 и русское сообщение", () => {
    const result = runCli([]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /--pid/);
    assert.equal(result.stdout, "");
  });

  it("нечисловой --pid → exit 1", () => {
    const result = runCli(["--pid", "abc"]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /--pid/);
  });

  it("нулевой --duration-ms → exit 1", () => {
    const result = runCli(["--pid", String(process.pid), "--duration-ms", "0"]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /--duration-ms/);
  });

  it("неизвестный аргумент → exit 1", () => {
    const result = runCli(["--pid", String(process.pid), "--nope"]);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /--nope/);
  });

  it("несуществующий PID → exit 2 без записи в процесс", (t) => {
    const pid = 4_000_000;
    if (existsSync(`/proc/${pid}`)) {
      t.skip(`/proc/${pid} существует — не проверить недоступный процесс`);
      return;
    }
    const result = runCli(["--pid", String(pid), "--duration-ms", "100", "--interval-ms", "50"]);
    assert.equal(result.status, 2);
    assert.match(result.stderr, new RegExp(`/proc/${pid}`));
    assert.match(result.stderr, /недоступ/);
  });
});

describe("CLI: живой замер своего процесса", { skip: process.platform !== "linux" }, () => {
  it("пишет JSON в stdout и в --json, поля контракта на месте", () => {
    const dir = mkdtempSync(join(tmpdir(), "gui-spike-bench-"));
    const out = join(dir, "out.json");
    try {
      const result = runCli([
        "--pid", String(process.pid),
        "--duration-ms", "160",
        "--interval-ms", "40",
        "--json", out,
      ]);
      assert.equal(result.status, 0, result.stderr);
      const parsed = JSON.parse(result.stdout);
      assert.equal(parsed.pid, process.pid);
      assert.ok(parsed.samples >= 2, `samples=${parsed.samples}`);
      assert.ok(parsed.rss_kb.min > 0);
      assert.ok(parsed.rss_kb.min <= parsed.rss_kb.avg && parsed.rss_kb.avg <= parsed.rss_kb.max);
      assert.ok(parsed.rss_kb.p95 >= parsed.rss_kb.min && parsed.rss_kb.p95 <= parsed.rss_kb.max);
      assert.ok(parsed.hwm_kb >= parsed.rss_kb.max);
      assert.ok(parsed.threads_max >= 1);
      assert.ok(parsed.duration_ms >= 40, `duration_ms=${parsed.duration_ms}`);
      assert.deepEqual(Object.keys(parsed).sort(), [
        "duration_ms", "hwm_kb", "pid", "rss_kb", "samples", "threads_max",
      ]);
      assert.deepEqual(JSON.parse(readFileSync(out, "utf8")), parsed);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("без --tree поля processes/skipped отсутствуют (обратная совместимость)", () => {
    const result = runCli([
      "--pid", String(process.pid),
      "--duration-ms", "100",
      "--interval-ms", "50",
    ]);
    assert.equal(result.status, 0, result.stderr);
    const parsed = JSON.parse(result.stdout);
    assert.equal(Object.hasOwn(parsed, "processes"), false, JSON.stringify(parsed));
    assert.equal(Object.hasOwn(parsed, "skipped"), false, JSON.stringify(parsed));
  });
});

describe("CLI: --tree (P1-3, сумма RSS по дереву)", { skip: process.platform !== "linux" }, () => {
  it("--tree на своём процессе: processes >= 1, rss_kb.avg > 0, есть skipped", () => {
    const result = runCli([
      "--pid", String(process.pid),
      "--duration-ms", "160",
      "--interval-ms", "40",
      "--tree",
    ]);
    assert.equal(result.status, 0, result.stderr);
    const parsed = JSON.parse(result.stdout);
    assert.equal(parsed.pid, process.pid);
    assert.ok(Number.isInteger(parsed.processes) && parsed.processes >= 1, JSON.stringify(parsed));
    assert.ok(parsed.rss_kb.avg > 0);
    assert.ok(Number.isInteger(parsed.skipped) && parsed.skipped >= 0, JSON.stringify(parsed));
  });

  it("--tree --pid <дочерний node>: дерево из одного процесса считается", async () => {
    const child = await startNode("setTimeout(() => {}, 5000)");
    try {
      assert.ok(typeof child.pid === "number");
      const result = runCli([
        "--pid", String(child.pid),
        "--duration-ms", "160",
        "--interval-ms", "40",
        "--tree",
      ]);
      assert.equal(result.status, 0, result.stderr);
      const parsed = JSON.parse(result.stdout);
      assert.equal(parsed.pid, child.pid);
      assert.ok(parsed.processes >= 1, JSON.stringify(parsed));
      assert.ok(parsed.rss_kb.avg > 0);
      assert.ok(parsed.threads_max >= 1);
    } finally {
      killQuietly(child);
    }
  });

  it("--tree рекурсивен: родитель + его потомок → processes >= 2", async (t) => {
    const parentCode = [
      'const { spawn } = require("node:child_process");',
      'spawn(process.execPath, ["-e", "setTimeout(() => {}, 5000)"], { stdio: "ignore" });',
      "setTimeout(() => {}, 5000);",
    ].join("\n");
    const parent = await startNode(parentCode);
    try {
      assert.ok(typeof parent.pid === "number");
      const hasChild = await waitForChildren(parent.pid, 3000);
      if (!hasChild) {
        t.skip("/proc/<pid>/task/<pid>/children недоступен — дерево не проверить");
        return;
      }
      const result = runCli([
        "--pid", String(parent.pid),
        "--duration-ms", "160",
        "--interval-ms", "40",
        "--tree",
      ]);
      assert.equal(result.status, 0, result.stderr);
      const parsed = JSON.parse(result.stdout);
      assert.ok(parsed.processes >= 2, JSON.stringify(parsed));
      assert.ok(parsed.rss_kb.avg > 0);
    } finally {
      killQuietly(parent);
    }
  });

  it("--tree с несуществующим PID → exit 2 (процесс, а не аргументы)", (t) => {
    const pid = 4_000_000;
    if (existsSync(`/proc/${pid}`)) {
      t.skip(`/proc/${pid} существует — не проверить недоступный процесс`);
      return;
    }
    const result = runCli(["--pid", String(pid), "--duration-ms", "100", "--tree"]);
    assert.equal(result.status, 2);
    assert.match(result.stderr, new RegExp(`/proc/${pid}`));
  });
});

describe("aggregateTree (P1-3: сумма RSS, максимум HWM, сумма потоков)", () => {
  it("rss и threads суммируются, hwm берётся максимумом (не суммой)", () => {
    const merged = aggregateTree([
      { rss_kb: 100, hwm_kb: 900, threads: 3 },
      { rss_kb: 200, hwm_kb: 1200, threads: 5 },
      { rss_kb: 50, hwm_kb: 1000, threads: 2 },
    ]);
    assert.deepEqual(merged, { rss_kb: 350, hwm_kb: 1200, threads: 10 });
  });

  it("пустой список → нули", () => {
    assert.deepEqual(aggregateTree([]), { rss_kb: 0, hwm_kb: 0, threads: 0 });
  });
});

describe("percentile95 (P2-6, прямой контракт nearest-rank)", () => {
  it("n=20 → 19-й элемент (190), не максимум", () => {
    const values = Array.from({ length: 20 }, (_, index) => (index + 1) * 10);
    assert.equal(percentile95(values), 190);
  });

  it("n=100, значения 1..100 → 95", () => {
    const values = Array.from({ length: 100 }, (_, index) => index + 1);
    assert.equal(percentile95(values), 95);
  });

  it("n=1 → единственный элемент", () => {
    assert.equal(percentile95([42]), 42);
  });

  it("пустой массив → 0 (без исключения)", () => {
    assert.equal(percentile95([]), 0);
  });
});

describe("parseStatus (P2-9, парсер /proc/<pid>/status)", () => {
  /** @param {string[]} lines */
  const status = (lines) => lines.join("\n");

  /**
   * @param {() => unknown} fn
   * @returns {string} сообщение брошенного исключения
   */
  const thrownMessage = (fn) => {
    try {
      fn();
    } catch (error) {
      return error instanceof Error ? error.message : String(error);
    }
    assert.fail("ожидалось исключение, но его не было");
  };

  const valid = status([
    "Name:\tnode",
    "VmPeak:\t  900000 kB",
    "VmRSS:\t  102400 kB",
    "VmHWM:\t  204800 kB",
    "Threads:\t7",
    "voluntary_ctxt_switches:\t3",
  ]);

  it("валидный статус → rss_kb/hwm_kb/threads", () => {
    assert.deepEqual(parseStatus(valid), { rss_kb: 102400, hwm_kb: 204800, threads: 7 });
  });

  it("значения без суффикса kB и с разными пробелами тоже принимаются", () => {
    const text = status(["VmRSS:\t102400", "VmHWM:   204800", "Threads: 12"]);
    assert.deepEqual(parseStatus(text), { rss_kb: 102400, hwm_kb: 204800, threads: 12 });
  });

  it("нет VmRSS → throws и называет только VmRSS", () => {
    const text = status(["VmHWM:\t  204800 kB", "Threads:\t7"]);
    const message = thrownMessage(() => parseStatus(text));
    assert.match(message, /VmRSS/);
    assert.doesNotMatch(message, /VmHWM/);
  });

  it("нет Threads → throws и называет поле Threads", () => {
    const text = status(["VmRSS:\t  102400 kB", "VmHWM:\t  204800 kB"]);
    assert.match(thrownMessage(() => parseStatus(text)), /Threads/);
  });

  it("отрицательные значения не матчатся \\d+ → throws с именем поля", () => {
    const text = status(["VmRSS:\t  -1024 kB", "VmHWM:\t  204800 kB", "Threads:\t7"]);
    assert.match(thrownMessage(() => parseStatus(text)), /VmRSS/);
  });

  it("нечисловые значения → throws с именем поля", () => {
    const text = status(["VmRSS:\t  nope", "VmHWM:\t  204800 kB", "Threads:\t7"]);
    assert.match(thrownMessage(() => parseStatus(text)), /VmRSS/);
  });

  it("пустой текст → throws и перечисляет все три поля", () => {
    assert.match(thrownMessage(() => parseStatus("")), /VmRSS.*VmHWM.*Threads/);
  });
});

describe("parseCliArgs (контракт флагов)", () => {
  it("значения по умолчанию: --pid обязателен, --tree выключен", () => {
    const options = parseCliArgs(["--pid", "123"]);
    assert.equal(options.pid, 123);
    assert.equal(options.durationMs, 5000);
    assert.equal(options.intervalMs, 200);
    assert.equal(options.jsonPath, null);
    assert.equal(options.tree, false);
  });

  it("--tree включает режим дерева, --json разрешает путь", () => {
    const options = parseCliArgs(["--pid", "7", "--tree", "--json", "/tmp/x.json"]);
    assert.equal(options.tree, true);
    assert.equal(options.jsonPath, "/tmp/x.json");
  });

  it("без --pid → throws с именем флага", () => {
    assert.throws(() => parseCliArgs([]), /--pid/);
  });

  it("границы --pid: 0 и 2^22+1 отвергаются", () => {
    assert.throws(() => parseCliArgs(["--pid", "0"]), /--pid/);
    assert.throws(() => parseCliArgs(["--pid", String(2 ** 22 + 1)]), /--pid/);
  });

  it("--duration-ms вне диапазона → throws", () => {
    assert.throws(() => parseCliArgs(["--pid", "1", "--duration-ms", "0"]), /--duration-ms/);
    assert.throws(
      () => parseCliArgs(["--pid", "1", "--duration-ms", String(3_600_001)]),
      /--duration-ms/,
    );
  });

  it("--json с пустым путём → throws", () => {
    assert.throws(() => parseCliArgs(["--pid", "1", "--json", ""]), /--json/);
  });
});

describe("parseBenchLog / --bench-log (FPS из app.log)", () => {
  const bench = 'BENCH {"frontend_ready_ms":21,"chat":{"frames":300,"avgMs":16.67,"p95Ms":18,"fps":60},"members":{"frames":299,"avgMs":16.67,"p95Ms":18,"fps":60}}';

  it("берёт последнюю строку BENCH и возвращает объект", () => {
    const text = `noise\n${bench}\nGBM warning\n${bench.replace('"fps":60}', '"fps":59}')}\n`;
    const parsed = parseBenchLog(text);
    assert.equal(parsed?.chat?.fps, 59);
    assert.equal(parsed?.frontend_ready_ms, 21);
  });

  it("нет строки BENCH → null (autobench не запускался)", () => {
    assert.equal(parseBenchLog("window-ready\nUI_SNAPSHOT {}\n"), null);
  });

  it("битый JSON в BENCH → throws с BENCH в сообщении", () => {
    assert.throws(() => parseBenchLog("BENCH {not json}\n"), /BENCH/);
  });

  it("BENCH не объект (массив/число) → throws", () => {
    assert.throws(() => parseBenchLog("BENCH [1,2]\n"), /BENCH/);
    assert.throws(() => parseBenchLog("BENCH 42\n"), /BENCH/);
  });

  it("--bench-log разрешает путь", () => {
    const options = parseCliArgs(["--pid", "1", "--bench-log", "/tmp/app.log"]);
    assert.equal(options.benchLogPath, "/tmp/app.log");
  });

  it("--bench-log с пустым путём → throws", () => {
    assert.throws(() => parseCliArgs(["--pid", "1", "--bench-log", ""]), /--bench-log/);
  });

  it("CLI: --bench-log добавляет bench (FPS/p95) в итоговый JSON", () => {
    const dir = mkdtempSync(join(tmpdir(), "gui-bench-log-"));
    const log = join(dir, "app.log");
    const out = join(dir, "out.json");
    try {
      writeFileSync(log, `window-ready\n${bench}\n`);
      const result = runCli([
        "--pid", String(process.pid),
        "--duration-ms", "160", "--interval-ms", "40",
        "--bench-log", log, "--json", out,
      ]);
      assert.equal(result.status, 0, result.stderr);
      const parsed = JSON.parse(result.stdout);
      assert.equal(parsed.bench.chat.fps, 60);
      assert.equal(parsed.bench.members.p95Ms, 18);
      assert.deepEqual(JSON.parse(readFileSync(out, "utf8")), parsed);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("CLI: --bench-log без BENCH → exit 1 (FPS запрошены, но не найдены)", () => {
    const dir = mkdtempSync(join(tmpdir(), "gui-bench-nolog-"));
    const log = join(dir, "app.log");
    try {
      writeFileSync(log, "window-ready\n");
      const result = runCli([
        "--pid", String(process.pid),
        "--duration-ms", "120", "--interval-ms", "40",
        "--bench-log", log,
      ]);
      assert.equal(result.status, 1, result.stderr);
      assert.match(result.stderr, /BENCH/);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});


/* =============================================================================
 * DP-02 / D-04: индекс высот — O(period) память, а не O(rows), и поиск строки
 * по offset не деградирует на 100 000 строк. Бюджет времени намеренно грубый
 * (порядок величины): это защита от случайного O(n) обхода, а не бенчмарк.
 * ========================================================================== */
describe("HeightIndex (D-04): бюджет памяти/времени", () => {
  it("prefix-sum хранит один период, а не все 100 000 строк", async () => {
    const { CHAT_HEIGHT_PERIOD, createChatHeights, createMemberHeights } = await import(
      "../../rust/gui-spike/ui/model.js"
    );
    const chat = createChatHeights(100_000);
    assert.ok(chat.prefix instanceof Float64Array);
    assert.equal(chat.prefix.length, CHAT_HEIGHT_PERIOD + 1, "prefix-sum растёт с числом строк");
    const member = createMemberHeights(24_229);
    assert.equal(member.prefix.length, 2, "member prefix-sum не O(1)");
    // 100k × 8 байт = 800 КБ; периодический индекс — 1565 × 8 ≈ 12.5 КБ.
    assert.ok(chat.prefix.byteLength < 20_000, `prefix-sum ${chat.prefix.byteLength} байт`);
  });

  it("top/indexAt на 100 000 строк укладываются в бюджет (нет O(n)-обхода)", async () => {
    const { createChatHeights } = await import("../../rust/gui-spike/ui/model.js");
    const heights = createChatHeights(100_000);
    const started = performance.now();
    let checksum = 0;
    for (let probe = 0; probe < 20_000; probe++) {
      const index = (probe * 7919) % 100_000;
      const top = heights.top(index);
      checksum += heights.indexAt(top) === index ? 1 : 0;
    }
    const elapsed = performance.now() - started;
    assert.equal(checksum, 20_000, "indexAt(top(i)) ≠ i — обратный поиск сломан");
    assert.ok(elapsed < 5_000, `20 000 пар top/indexAt заняли ${Math.round(elapsed)} ms`);
    // totalHeight — замкнутая формула по периоду, без суммирования 100k высот.
    assert.equal(heights.totalHeight, heights.top(100_000), "totalHeight ≠ top(totalRows)");
  });
});
