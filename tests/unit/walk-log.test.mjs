// @ts-check
// RED (M1): реализация — src/core/ui/walk-log.mjs. Контракт: журнал UI-прогона
// (паритет ui-walk-settings2: прогресс, записи шагов, ошибки консоли).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import {
  buildWalkRecord,
  consoleIssues,
  parseVisited,
  pickNextItem,
  shouldBail,
} from "../../src/core/ui/walk-log.mjs";

/** @typedef {import("../../src/core/types.js").CdpEvent} CdpEvent */

const walkFixture = fileURLToPath(new URL("../fixtures/logs/walk.sample.jsonl", import.meta.url));
const settingsFixture = fileURLToPath(new URL("../fixtures/logs/walk-settings.jsonl", import.meta.url));
const menusFixture = fileURLToPath(new URL("../fixtures/logs/walk-menus.jsonl", import.meta.url));

describe("parseVisited", () => {
  it("собирает подписи и максимальный шаг, пропуская битые строки", async () => {
    const text = await readFile(walkFixture, "utf8");
    const state = parseVisited(text);
    assert.deepEqual([...state.labels], ["Мой аккаунт", "Профиль", "Голос и видео"]);
    assert.equal(state.maxStep, 4);
  });

  it("пустой текст → пустой набор и maxStep 0", () => {
    const state = parseVisited("");
    assert.equal(state.labels.size, 0);
    assert.equal(state.maxStep, 0);
  });

  it("записи без строкового label игнорируются", () => {
    const state = parseVisited('{"step":1}\n\n   \n{"label":"ok","step":2}');
    assert.deepEqual([...state.labels], ["ok"]);
    assert.equal(state.maxStep, 2);
  });

  // Источники logs/ui-walk-settings.jsonl (md5 bf5b40e345d8b86d71cfcbf1b50d0948)
  // и logs/ui-walk-menus.jsonl (md5 de63e05ea72c09185384426d775be99d).
  // Оракулы: jq -s 'length' = 56/52; jq -s 'map(.step)|max' = 56/52;
  // jq -r '.label' | sort -u | wc -l = 56/46; sort | uniq -d = 0/6 повторов;
  // jq -s 'map(.consoleErrors // 0)|add' = 7/0; jq -r '.error // empty' | sort -u = пусто.
  it("восстанавливает golden-прогресс из полного журнала настроек", async () => {
    const text = await readFile(settingsFixture, "utf8");
    const records = text.trim().split("\n").map((line) => JSON.parse(line));
    const state = parseVisited(text);
    assert.equal(records.length, 56);
    assert.equal(state.labels.size, 56);
    assert.equal(state.maxStep, 56);
    for (const record of records) assert.ok(state.labels.has(record.label));
    assert.equal(new Set(records.map((record) => record.label)).size, state.labels.size);
  });

  it("сохраняет множество подписей при повторах в журнале меню", async () => {
    const text = await readFile(menusFixture, "utf8");
    const records = text.trim().split("\n").map((line) => JSON.parse(line));
    const state = parseVisited(text);
    assert.equal(records.length, 52);
    assert.equal(state.labels.size, 46);
    assert.equal(state.maxStep, 52);
    for (const record of records) assert.ok(state.labels.has(record.label));
    assert.equal(new Set(records.map((record) => record.label)).size, state.labels.size);
  });

  it("пропускает повреждённую реальную строку и продолжает разбор", async () => {
    const text = await readFile(settingsFixture, "utf8");
    const lines = text.trimEnd().split("\n");
    lines.splice(1, 1, `${lines[1]} мусор`);
    const state = parseVisited(lines.join("\n"));
    assert.equal(state.maxStep, 56);
    assert.equal(state.labels.size, 55);
  });
});

describe("buildWalkRecord", () => {
  it("значения по умолчанию", () => {
    assert.deepEqual(buildWalkRecord(5, "Профиль"), {
      step: 5,
      label: "Профиль",
      clicked: false,
      screenshot: null,
      consoleErrors: 0,
      consoleErrorTexts: [],
      topFunctions: [],
      error: null,
    });
  });

  it("сохраняет кириллическую подпись из реального журнала", () => {
    assert.equal(buildWalkRecord(1, "Учётная запись").label, "Учётная запись");
    assert.deepEqual(buildWalkRecord(1, "Учётная запись"), {
      step: 1, label: "Учётная запись", clicked: false, screenshot: null,
      consoleErrors: 0, consoleErrorTexts: [], topFunctions: [], error: null,
    });
  });
});

describe("consoleIssues", () => {
  it("считает exceptionThrown и consoleAPICalled type=error; текстов не больше 3", () => {
    /** @type {CdpEvent[]} */
    const events = [
      { at: 0, method: "Runtime.consoleAPICalled", params: { type: "log" } },
      { at: 1, method: "Runtime.exceptionThrown", params: { exceptionDetails: { text: "boom" } } },
      { at: 2, method: "Runtime.consoleAPICalled", params: { type: "error", args: [{ value: "x" }] } },
      { at: 3, method: "Runtime.consoleAPICalled", params: { type: "warning" } },
      { at: 4, method: "Log.entryAdded", params: {} },
      { at: 5, method: "Runtime.exceptionThrown", params: { exceptionDetails: { text: "два" } } },
      { at: 6, method: "Runtime.consoleAPICalled", params: { type: "error" } },
      { at: 7, method: "Runtime.exceptionThrown", params: { exceptionDetails: { text: "три" } } },
      { at: 8, method: "Runtime.exceptionThrown", params: { exceptionDetails: { text: "четыре" } } },
    ];
    const issues = consoleIssues(events);
    assert.equal(issues.count, 6);
    assert.equal(issues.texts.length, 3);
    assert.ok(issues.texts[0]?.includes("boom"));
    assert.ok(issues.texts[1]?.includes('"error"'));
    assert.ok(issues.texts[2]?.includes("два"));
  });

  it("обрезает текст до 220 символов", () => {
    const events = [
      {
        at: 0,
        method: "Runtime.exceptionThrown",
        params: { exceptionDetails: { text: "x".repeat(500) } },
      },
    ];
    assert.equal(consoleIssues(events).texts[0]?.length, 220);
  });

  it("без ошибок → пусто", () => {
    assert.deepEqual(consoleIssues([]), { count: 0, texts: [] });
  });

  it("считает события с реальным именем и URL и обрезает JSON до 220", async () => {
    const text = await readFile(settingsFixture, "utf8");
    const record = text.trim().split("\n").map((line) => JSON.parse(line))
      .find((item) => item.topFunctions.some((/** @type {{ fn: string, url: string }} */ fn) => fn.fn === "Mn.b3.channelType" && fn.url));
    assert.ok(record);
    const realFunction = record.topFunctions.find((/** @type {{ fn: string, url: string }} */ fn) => fn.fn === "Mn.b3.channelType" && fn.url);
    assert.ok(realFunction);
    const event = { at: 0, method: "Runtime.exceptionThrown", params: {
      exceptionDetails: { text: `${realFunction.fn} ${realFunction.url} ${"x".repeat(300)}` },
    } };
    const issues = consoleIssues([event, { at: 1, method: "Runtime.consoleAPICalled", params: { type: "error", args: [{ value: "real console error" }] } }]);
    assert.equal(issues.count, 2);
    assert.equal(issues.texts.length, 2);
    assert.equal(issues.texts[0]?.length, 220);
    assert.match(issues.texts[0] ?? "", /Mn\.b3\.channelType/);
  });
});

describe("shouldBail", () => {
  it("прерывает прогон на таймауте и закрытом соединении", () => {
    assert.equal(shouldBail("Runtime.evaluate: таймаут 15с"), true);
    assert.equal(shouldBail("CDP соединение закрыто (страница умерла?)"), true);
  });

  it("обычная ошибка шага прогон не прерывает", () => {
    assert.equal(shouldBail("не найден элемент (no nav)"), false);
    assert.equal(shouldBail(null), false);
  });

  // Оракул: jq -c 'select(.error!=null)' <fixture> | wc -l = 0 для обоих журналов.
  it("подтверждает отсутствие ошибок в журналах и проверяет все найденные", async () => {
    for (const path of [settingsFixture, menusFixture]) {
      const records = (await readFile(path, "utf8")).trim().split("\n").map((line) => JSON.parse(line));
      const errorRecords = records.filter((record) => record.error != null);
      assert.equal(errorRecords.length, 0, path);
      for (const record of errorRecords) assert.equal(shouldBail(record.error), false, record.error);
    }
  });
});

describe("pickNextItem", () => {
  const items = [{ text: "Мой аккаунт" }, { text: "Покинуть сервер" }, { text: "Профиль" }];

  it("пропускает посещённые и запрещённые (чёрный список по умолчанию)", () => {
    assert.deepEqual(pickNextItem(items, new Set(["Мой аккаунт"])), { text: "Профиль" });
  });

  it("сохраняет порядок; когда всё пройдено → undefined", () => {
    assert.deepEqual(pickNextItem(items, new Set()), { text: "Мой аккаунт" });
    assert.equal(pickNextItem(items, new Set(["Мой аккаунт", "Профиль"])), undefined);
  });

  it("можно передать свой чёрный список", () => {
    assert.equal(pickNextItem(items, new Set(), () => false)?.text, "Мой аккаунт");
    assert.equal(pickNextItem(items, new Set(), (label) => label !== "Профиль")?.text, "Профиль");
  });

  it("выбирает следующий непосещённый пункт из реального журнала", async () => {
    const text = await readFile(settingsFixture, "utf8");
    const items = text.trim().split("\n").map((line) => ({ text: JSON.parse(line).label }));
    const visited = new Set(items.slice(0, 10).map((item) => item.text));
    assert.deepEqual(pickNextItem(items, visited, () => false), items[10]);
  });
});
