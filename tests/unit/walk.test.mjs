// @ts-check
// RED (QA4-01b): реализация — src/core/ui/engine.mjs + src/core/ui/adapters.mjs.
// Контракт walk: QUALITY/WALK-PLAN.md §1 (fixture). Unit-уровень: без сокетов,
// без браузера, без live Discord. Все артефакты пишутся только в /tmp.

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { chmod, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { clickNavItemExpression, resolveAdapter } from "../../src/core/ui/adapters.mjs";
import { runWalk } from "../../src/core/ui/engine.mjs";
import { screenshotName } from "../../src/core/paths.mjs";
import { captureCli } from "../helpers/cli-capture.mjs";
import { installFakeCdp } from "../helpers/fake-cdp.mjs";
import { makeTmpDir } from "../helpers/tmp.mjs";

const { run } = await import("../../src/cli/run.mjs");

/** @typedef {import("../../src/core/types.js").WalkRecord} WalkRecord */
/** @typedef {import("../../src/core/types.js").CdpEvent} CdpEvent */

/** Байты с сигнатурой PNG: достаточно для проверки контракта записи. */
const PNG = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x00]);

const NAV_ITEMS = ["Мой аккаунт", "Профиль", "Уведомления", "Голос и видео", "Покинуть сервер", "Выйти"];
const NORMAL_ITEMS = NAV_ITEMS.slice(0, 4);
const PROFILE = {
  nodes: [
    { hitCount: 7, callFrame: { functionName: "onNavClick", url: "file:///app/app.js" } },
    { hitCount: 1, callFrame: { functionName: "(idle)", url: "" } },
  ],
};

/**
 * Фейковый CDP-клиент для уровня T1 (engine): real-сокеты не открываются.
 * @param {{
 *   items?: string[],
 *   itemsAfter?: string[],
 *   click?: (label: string) => any,
 *   consoleLabels?: Set<string>,
 *   profile?: any,
 *   shot?: () => any,
 *   navError?: Error,
 *   failRuntimeEnable?: boolean,
 *   failProfilerStart?: boolean,
 *   failProfilerStop?: boolean,
 * }} [options]
 */
function fakeCdp(options = {}) {
  let items = options.items ?? NAV_ITEMS;
  /** @type {Array<{ method: string, params: Record<string, unknown> }>} */
  const calls = [];
  /** @type {CdpEvent[]} */
  const events = [];
  /** @type {string[]} */
  const clicked = [];
  /** @type {any} */
  const cdp = {
    events,
    calls,
    clicked,
    mark: () => events.length,
    eventsSince: (/** @type {number} */ mark) => events.slice(mark),
    send: async (/** @type {string} */ method, /** @type {Record<string, unknown>} */ params = {}) => {
      calls.push({ method, params });
      if (method === "Runtime.enable" && options.failRuntimeEnable) throw new Error("Runtime.enable: CDP недоступен");
      if (method === "Profiler.start" && options.failProfilerStart) throw new Error("Profiler.start: сбой профиля");
      if (method === "Profiler.stop" && options.failProfilerStop) throw new Error("Profiler.stop: сбой профиля");
      if (method === "Runtime.evaluate") {
        if (options.navError) throw options.navError;
        const expression = String(params.expression);
        const label = items.find((text) => expression.includes(JSON.stringify(text)));
        if (expression.includes(".click()")) {
          if (label === undefined) return { result: { value: { clicked: false, reason: "label not found" } } };
          clicked.push(label);
          if (options.consoleLabels?.has(label)) {
            events.push({
              at: Date.now(),
              method: "Runtime.consoleAPICalled",
              params: { type: "error", args: [{ value: `fixture: ${label}` }] },
            });
          }
          const outcome = options.click ? options.click(label) : { clicked: true, text: label };
          items = options.itemsAfter ?? items;
          return { result: { value: outcome } };
        }
        return { result: { value: items.map((text) => ({ text })) } };
      }
      if (method === "Profiler.stop") return { profile: options.profile ?? PROFILE };
      if (method === "Page.captureScreenshot") {
        return options.shot ? options.shot() : { data: PNG.toString("base64") };
      }
      return {};
    },
    close: () => {},
  };
  return cdp;
}

/**
 * Прогон движка с фейковым CDP и временным каталогом.
 * @param {import("node:test").TestContext} t
 * @param {{
 *   engine?: Parameters<typeof fakeCdp>[0],
 *   opts?: { profile?: boolean, maxSteps?: number, fresh?: boolean },
 *   outPath?: (dir: string) => string,
 *   prepare?: (paths: { dir: string, outPath: string, screensDir: string }) => Promise<void>,
 * }} [options]
 */
async function walk(t, options = {}) {
  const dir = await makeTmpDir(t);
  const outPath = options.outPath ? options.outPath(dir) : join(dir, "ui-walk-fixture.jsonl");
  const screensDir = join(dir, "screens");
  await options.prepare?.({ dir, outPath, screensDir });
  const cdp = fakeCdp(options.engine);
  /** @type {string[]} */
  const out = [];
  /** @type {string[]} */
  const err = [];
  const result = await runWalk({
    cdp,
    adapter: resolveAdapter("fixture"),
    outPath,
    screensDir,
    settleMs: 0,
    profile: options.opts?.profile ?? true,
    maxSteps: options.opts?.maxSteps ?? 200,
    fresh: options.opts?.fresh ?? false,
    log: (/** @type {string} */ line) => out.push(line),
    warn: (/** @type {string} */ line) => err.push(line),
    sleep: async () => {},
  });
  return { dir, outPath, screensDir, cdp, result, stdout: out.join("\n"), stderr: err.join("\n") };
}

/**
 * Строки JSONL-журнала.
 * @param {string} path
 * @returns {Promise<any[]>}
 */
async function readRecords(path) {
  const text = await readFile(path, "utf8");
  return text.split("\n").filter((line) => line.trim()).map((line) => JSON.parse(line));
}

/**
 * Проверяет, что PNG-файл существует и имеет сигнатуру.
 * @param {string | null} path
 */
async function assertPng(path) {
  assert.ok(path, "в записи нет пути скриншота");
  const bytes = await readFile(path);
  assert.deepEqual([...bytes.subarray(0, 4)], [0x89, 0x50, 0x4e, 0x47]);
}

describe("walk: адаптер fixture", () => {
  it("resolveAdapter отдаёт селектор навигации и префикс скриншотов", () => {
    const adapter = resolveAdapter("fixture");
    assert.equal(adapter.navSelector, "#nav button");
    assert.equal(adapter.screenshotPrefix, "ui-fixture");
  });

  it("неизвестное приложение → ошибка со списком доступных", () => {
    assert.throws(
      () => resolveAdapter("nope"),
      /неизвестное приложение «nope» \(доступно: fixture\)/u,
    );
  });

  it("clickNavItemExpression: точное совпадение, прокрутка и клик", () => {
    const expression = clickNavItemExpression("#nav button", "Профиль");
    assert.ok(expression.includes("#nav button"));
    assert.ok(expression.includes(JSON.stringify("Профиль")));
    assert.match(expression, /scrollIntoView/u);
    assert.match(expression, /\.click\(\)/u);
  });
});

describe("walk: аргументы CLI", () => {
  it("без приложения → код 1", async () => {
    const result = await captureCli(() => run(["walk"]));
    assert.equal(result.code, 1);
    assert.match(result.stderr, /walk: укажите приложение \(доступно: fixture\)/u);
  });

  it("неизвестное приложение → код 1", async () => {
    const result = await captureCli(() => run(["walk", "nope"]));
    assert.equal(result.code, 1);
    assert.match(result.stderr, /walk: неизвестное приложение «nope» \(доступно: fixture\)/u);
  });

  it("пустой --url → код 1", async () => {
    const result = await captureCli(() => run(["walk", "fixture", "--url="]));
    assert.equal(result.code, 1);
    assert.match(result.stderr, /walk: --url/u);
  });

  for (const value of ["abc", "-1", "10001"]) {
    it(`--settle-ms ${value} → код 1`, async () => {
      const result = await captureCli(() => run(["walk", "fixture", "--settle-ms", value]));
      assert.equal(result.code, 1);
      assert.match(result.stderr, /Неверное значение --settle-ms/u);
    });
  }

  for (const value of ["0", "1001"]) {
    it(`--max-steps ${value} → код 1`, async () => {
      const result = await captureCli(() => run(["walk", "fixture", "--max-steps", value]));
      assert.equal(result.code, 1);
      assert.match(result.stderr, /Неверное значение --max-steps/u);
    });
  }
});

describe("walk: движок fixture", () => {
  it("кликает обычные пункты, запрещённые не трогает, пишет JSONL и PNG", async (t) => {
    const r = await walk(t, { engine: { consoleLabels: new Set(["Уведомления"]) } });

    assert.equal(r.result.code, 0, r.stderr);
    assert.equal(r.result.steps, 4);
    assert.deepEqual(r.result.records.map((record) => record.label), NORMAL_ITEMS);
    assert.deepEqual(r.result.records.map((record) => record.step), [1, 2, 3, 4]);
    assert.deepEqual(r.cdp.clicked, NORMAL_ITEMS);
    assert.deepEqual(Object.keys(r.result.records[0] ?? {}).sort(), [
      "clicked",
      "consoleErrorTexts",
      "consoleErrors",
      "error",
      "label",
      "screenshot",
      "step",
      "topFunctions",
    ]);

    /** @type {string[]} */
    const expressions = r.cdp.calls
      .filter((/** @type {{ method: string }} */ call) => call.method === "Runtime.evaluate")
      .map((/** @type {{ params: Record<string, unknown> }} */ call) => String(call.params.expression));
    for (const forbidden of ["Покинуть сервер", "Выйти"]) {
      assert.equal(
        expressions.filter((expression) => expression.includes(forbidden)).length,
        0,
        `запрещённый пункт «${forbidden}» не должен попадать в клик`,
      );
    }

    for (const record of r.result.records) {
      assert.equal(record.clicked, true, record.label);
      assert.equal(record.error, null);
      await assertPng(record.screenshot);
    }

    const byLabel = new Map(r.result.records.map((record) => [record.label, record]));
    assert.equal(byLabel.get("Уведомления")?.consoleErrors, 1);
    assert.equal(byLabel.get("Профиль")?.consoleErrors, 0);
    assert.ok((byLabel.get("Уведомления")?.consoleErrorTexts.length ?? 0) >= 1);
    assert.equal(byLabel.get("Профиль")?.topFunctions[0]?.fn, "onNavClick");

    const forbiddenLine = (r.stdout.match(/ПРОПУСК \(запрещено\): Покинуть сервер/gu) ?? []).length;
    assert.equal(forbiddenLine, 1, "ровно одна строка пропуска на пункт");
    assert.match(r.stdout, /\[01\] Мой аккаунт \| клик=true/u);

    const persisted = await readRecords(r.outPath);
    assert.equal(persisted.length, 4);
    assert.deepEqual(persisted.map((record) => record.label), NORMAL_ITEMS);

    assert.deepEqual(
      r.cdp.calls.slice(0, 3).map((/** @type {{ method: string }} */ call) => call.method),
      ["Runtime.enable", "Profiler.enable", "Page.enable"],
    );
    const methods = r.cdp.calls.map((/** @type {{ method: string }} */ call) => call.method);
    const startIndex = methods.indexOf("Profiler.start");
    assert.ok(startIndex > 0);
    assert.equal(methods[startIndex + 1], "Runtime.evaluate");
    assert.equal(methods[startIndex + 2], "Profiler.stop");
    assert.equal(methods[startIndex + 3], "Page.captureScreenshot");
  });

  it("--no-profile: без Profiler.* и с пустым топом", async (t) => {
    const r = await walk(t, { opts: { profile: false } });
    assert.equal(r.result.code, 0, r.stderr);
    assert.equal(r.cdp.calls.some((/** @type {{ method: string }} */ call) => call.method.startsWith("Profiler.")), false);
    for (const record of r.result.records) assert.deepEqual(record.topFunctions, []);
  });

  it("resume: докатывает только непосещённые и продолжает нумерацию", async (t) => {
    const r = await walk(t, {
      prepare: async ({ outPath }) => {
        await writeFile(outPath, [
          JSON.stringify({ step: 1, label: "Мой аккаунт", clicked: true }),
          JSON.stringify({ step: 2, label: "Профиль", clicked: true }),
        ].join("\n") + "\n");
      },
    });

    assert.equal(r.result.code, 0, r.stderr);
    assert.deepEqual(r.result.records.map((record) => record.label), ["Уведомления", "Голос и видео"]);
    assert.deepEqual(r.result.records.map((record) => record.step), [3, 4]);
    assert.deepEqual(r.cdp.clicked, ["Уведомления", "Голос и видео"]);
    assert.match(r.stdout, /ПРОПУСК \(уже пройдено\): Мой аккаунт/u);
    assert.equal((await readRecords(r.outPath)).length, 4);
  });

  it("--fresh: усекает журнал и проходит всё заново", async (t) => {
    const r = await walk(t, {
      opts: { fresh: true },
      prepare: async ({ outPath }) => {
        await writeFile(outPath, `${JSON.stringify({ step: 1, label: "Мой аккаунт", clicked: true })}\n`);
      },
    });

    assert.equal(r.result.code, 0, r.stderr);
    assert.deepEqual(r.result.records.map((record) => record.step), [1, 2, 3, 4]);
    assert.deepEqual(r.result.records.map((record) => record.label), NORMAL_ITEMS);
    assert.equal((await readRecords(r.outPath)).length, 4);
    assert.doesNotMatch(r.stdout, /уже пройдено/u);
  });

  it("пустая навигация → код 2 и нет записей", async (t) => {
    const r = await walk(t, { engine: { items: [] } });
    assert.equal(r.result.code, 2);
    assert.deepEqual(r.result.records, []);
    assert.match(r.stderr, /0 пунктов навигации/u);
  });

  it("ошибка Runtime.enable → код 2 без шагов", async (t) => {
    const r = await walk(t, { engine: { failRuntimeEnable: true } });
    assert.equal(r.result.code, 2);
    assert.deepEqual(r.result.records, []);
    assert.match(r.stderr, /ошибка CDP: Runtime\.enable/u);
  });

  it("ошибка списка навигации → код 2", async (t) => {
    const r = await walk(t, { engine: { navError: new Error("CDP list failed: 500") } });
    assert.equal(r.result.code, 2);
    assert.deepEqual(r.result.records, []);
    assert.match(r.stderr, /CDP list failed/u);
  });

  it("навигация исчезла после шага → штатное завершение", async (t) => {
    const r = await walk(t, { engine: { itemsAfter: [] } });
    assert.equal(r.result.code, 0, r.stderr);
    assert.equal(r.result.records.length, 1);
    assert.deepEqual(r.cdp.clicked, ["Мой аккаунт"]);
  });

  it("ошибка Profiler.start → шаг без клика, итог 2", async (t) => {
    const r = await walk(t, { engine: { failProfilerStart: true } });
    assert.equal(r.result.code, 2);
    assert.equal(r.result.records.length, 4);
    assert.deepEqual(r.cdp.clicked, []);
    assert.match(r.result.records[0]?.error ?? "", /Profiler\.start/u);
  });

  it("ошибка Profiler.stop → запись с ошибкой, итог 2", async (t) => {
    const r = await walk(t, { engine: { failProfilerStop: true } });
    assert.equal(r.result.code, 2);
    assert.equal(r.result.records.length, 4);
    assert.deepEqual(r.cdp.clicked, NORMAL_ITEMS);
    assert.match(r.result.records[0]?.error ?? "", /Profiler\.stop/u);
  });

  it("ошибка записи журнала → код 2", async (t) => {
    const r = await walk(t, {
      prepare: async ({ outPath }) => {
        await writeFile(outPath, "");
        await chmod(outPath, 0o444);
      },
    });
    assert.equal(r.result.code, 2);
    assert.deepEqual(r.result.records, []);
    assert.match(r.stderr, /журнал|EACCES/iu);
  });

  it("clicked:false → запись с ошибкой, прогон продолжается, итог 2", async (t) => {
    const r = await walk(t, {
      engine: {
        click: (label) => (label === "Профиль" ? { clicked: false, reason: "label not found" } : { clicked: true, text: label }),
      },
    });

    assert.equal(r.result.code, 2);
    assert.deepEqual(r.result.records.map((record) => record.label), NORMAL_ITEMS);
    const profile = r.result.records.find((record) => record.label === "Профиль");
    assert.equal(profile?.clicked, false);
    assert.match(profile?.error ?? "", /label not found/u);
    assert.match(r.stderr, /Шагов с ошибкой: 1/u);
  });

  it("потеря соединения (таймаут) → shouldBail: цикл прерван, код 2", async (t) => {
    const r = await walk(t, {
      engine: {
        click: (label) => {
          if (label === "Профиль") throw new Error("Runtime.evaluate: таймаут 15000 мс");
          return { clicked: true, text: label };
        },
      },
    });

    assert.equal(r.result.code, 2);
    assert.deepEqual(r.result.records.map((record) => record.label), ["Мой аккаунт", "Профиль"]);
    assert.match(r.result.records[1]?.error ?? "", /таймаут/u);
    assert.equal(r.cdp.clicked.includes("Уведомления"), false);
    assert.equal(r.result.records[1]?.screenshot, null);
  });

  it("ошибка записи PNG → код 2, screenshot null", async (t) => {
    const r = await walk(t, {
      prepare: async ({ screensDir }) => {
        await mkdir(join(screensDir, screenshotName("ui-fixture", 1, "Мой аккаунт")), { recursive: true });
      },
    });

    assert.equal(r.result.code, 2);
    assert.equal(r.result.records.length, 1);
    assert.match(r.result.records[0]?.error ?? "", /скриншот/u);
    assert.equal(r.result.records[0]?.screenshot, null);
  });

  it("битая сигнатура PNG → код 2", async (t) => {
    const r = await walk(t, {
      engine: { shot: () => ({ data: Buffer.from("not-a-png").toString("base64") }) },
    });

    assert.equal(r.result.code, 2);
    assert.match(r.result.records[0]?.error ?? "", /PNG/u);
    assert.equal(r.result.records[0]?.screenshot, null);
  });

  it("--max-steps — предохранитель: стоп с предупреждением, код 0", async (t) => {
    const r = await walk(t, { opts: { maxSteps: 2 } });
    assert.equal(r.result.code, 0, r.stderr);
    assert.deepEqual(r.result.records.map((record) => record.step), [1, 2]);
    assert.match(r.stderr, /--max-steps/u);
  });

  it("недоступный каталог журнала → код 2", async (t) => {
    const r = await walk(t, {
      outPath: (dir) => join(dir, "blocked", "walk.jsonl"),
      prepare: async ({ dir }) => {
        await writeFile(join(dir, "blocked"), "");
      },
    });

    assert.equal(r.result.code, 2);
    assert.deepEqual(r.result.records, []);
    assert.match(r.stderr, /каталог|ENOTDIR|not a directory/iu);
  });

  it("журнал не читается (путь — каталог) → код 2", async (t) => {
    const r = await walk(t, {
      prepare: async ({ outPath }) => {
        await mkdir(outPath, { recursive: true });
      },
    });

    assert.equal(r.result.code, 2);
    assert.match(r.stderr, /журнал|EISDIR|illegal/iu);
  });
});

/**
 * Ответчик CDP для CLI-прогона (T2): без сокетов, но через createCdpClient.
 * @param {string[]} items
 * @param {Set<string>} consoleLabels
 */
function walkResponder(items, consoleLabels) {
  /** @type {string[]} */
  const clicked = [];
  /**
   * @param {string} method
   * @param {Record<string, unknown>} params
   * @param {any} socket
   */
  const respond = (method, params, socket) => {
    if (method === "Runtime.evaluate") {
      const expression = String(params.expression);
      const label = items.find((text) => expression.includes(JSON.stringify(text)));
      if (expression.includes(".click()")) {
        if (label === undefined) return { result: { value: { clicked: false, reason: "label not found" } } };
        clicked.push(label);
        if (consoleLabels.has(label)) {
          socket.receive({ method: "Runtime.consoleAPICalled", params: { type: "error", args: [{ value: "boom" }] } });
        }
        return { result: { value: { clicked: true, text: label } } };
      }
      return { result: { value: items.map((text) => ({ text })) } };
    }
    if (method === "Profiler.stop") return { profile: PROFILE };
    if (method === "Page.captureScreenshot") return { data: PNG.toString("base64") };
    return {};
  };
  return { clicked, respond };
}

describe("walk: CLI через fake CDP", () => {
  it("walk fixture → код 0, журнал, PNG и пропуски в stdout", async (t) => {
    const dir = await makeTmpDir(t);
    const fake = walkResponder(NAV_ITEMS, new Set(["Уведомления"]));
    const stub = installFakeCdp({ respond: fake.respond });
    t.after(() => stub.restore());

    const out = join(dir, "ui-walk-fixture.jsonl");
    const screens = join(dir, "screens");
    const result = await captureCli(() => run(["walk", "fixture", "--out", out, "--screens", screens]));

    assert.equal(result.code, 0, result.stderr);
    assert.match(result.stdout, /ПРОПУСК \(запрещено\): Покинуть сервер/u);
    assert.match(result.stdout, /ПРОПУСК \(запрещено\): Выйти/u);
    assert.match(result.stdout, /Готово: 4 шагов/u);

    const records = await readRecords(out);
    assert.deepEqual(records.map((record) => record.label), NORMAL_ITEMS);
    for (const record of records) assert.equal(record.clicked, true);
    const screenshots = (await readdir(screens)).filter((name) => name.endsWith(".png"));
    assert.equal(screenshots.length, 4);

    assert.equal(new Set(stub.sockets()).size, 1, "одно WS-соединение на прогон");
    assert.ok(stub.calls.some((call) => call.method === "Runtime.enable"));
    assert.equal(stub.calls.some((call) => call.method === "Page.navigate"), false);
  });

  it("--fresh и --no-profile: журнал усечён, Profiler не вызывается", async (t) => {
    const dir = await makeTmpDir(t);
    const out = join(dir, "ui-walk-fixture.jsonl");
    const screens = join(dir, "screens");
    await writeFile(out, `${JSON.stringify({ step: 1, label: "Мой аккаунт", clicked: true })}\n`);
    const fake = walkResponder(NAV_ITEMS, new Set());
    const stub = installFakeCdp({ respond: fake.respond });
    t.after(() => stub.restore());

    const result = await captureCli(() =>
      run(["walk", "fixture", "--out", out, "--screens", screens, "--fresh", "--no-profile", "--settle-ms", "0"]));

    assert.equal(result.code, 0, result.stderr);
    const records = await readRecords(out);
    assert.deepEqual(records.map((record) => record.label), NORMAL_ITEMS);
    assert.deepEqual(records.map((record) => record.step), [1, 2, 3, 4]);
    for (const record of records) assert.deepEqual(record.topFunctions, []);
    assert.equal(stub.calls.some((call) => call.method.startsWith("Profiler.")), false);
  });

  it("нет подходящей страницы → код 2", async (t) => {
    const stub = installFakeCdp({ targets: [] });
    t.after(() => stub.restore());

    const dir = await makeTmpDir(t);
    const result = await captureCli(() => run(["walk", "fixture", "--out", join(dir, "walk.jsonl"), "--screens", join(dir, "screens")]));

    assert.equal(result.code, 2);
    assert.match(result.stderr, /Ошибка CDP endpoint/u);
  });

  it("ошибка шага → код 2 и счётчик ошибок", async (t) => {
    const dir = await makeTmpDir(t);
    const fake = walkResponder(NAV_ITEMS, new Set());
    const stub = installFakeCdp({
      respond: (method, params, socket) => {
        if (method === "Runtime.evaluate" && String(params.expression).includes(JSON.stringify("Профиль"))) {
          return { result: { value: { clicked: false, reason: "label not found" } } };
        }
        return fake.respond(method, params, socket);
      },
    });
    t.after(() => stub.restore());

    const result = await captureCli(() => run(["walk", "fixture", "--out", join(dir, "walk.jsonl"), "--screens", join(dir, "screens")]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /Шагов с ошибкой: 1/u);
  });
});

/** Проверка формы записи (компилятор ловит рассинхрон с WalkRecord). */
const _recordShape = /** @type {WalkRecord | null} */ (null);
void _recordShape;
