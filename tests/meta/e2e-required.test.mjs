// @ts-check
/**
 * Мета-инвариант QA-M2-02: семь browser-сценариев M2 обязаны существовать,
 * не иметь skip-пути и попадать в fixture-набор гейта.
 *
 * Почему именно так:
 * - «файла нет» и `skip` — это не pass (`QA-STRATEGY` §8, `QA-E2E-PLAN` §3.4);
 * - браузер в этом файле НЕ нужен: проверяются статические инварианты,
 *   контракт fake-транспорта (vm, без сети) и политика гейта (`failOnSkips`);
 * - отсутствие браузера не может дать зелёный pass: `requireBrowser()` бросает
 *   `InfraError` (не `t.skip`), а гейт переводит `skipped > 0` в failed.
 *
 * Oracle — `tests/helpers/m2-scenarios.json` (реестр сценариев), `scripts/gate.mjs`
 * (политика skip ≠ pass), `fixtures/gateway/by-type/*` (данные, не проверяются здесь).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import { bootstrapSource, installFakeTauri } from "../helpers/fake-tauri.mjs";
import {
  M2_SCENARIOS,
  detectBrowser,
  m2UiPresent,
  requireBrowser,
  startUiServer,
} from "../helpers/ui-fixture.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));

/** @param {string} path @returns {string} */
const read = (path) => readFileSync(join(root, path), "utf8");

/** Тело функции из `scripts/gate.mjs` (тот же приём, что в `test-policy.test.mjs`). */
const gateSource = read("scripts/gate.mjs");

/**
 * @param {string} source
 * @param {string} name
 * @returns {string}
 */
function extractFunction(source, name) {
  const start = source.indexOf(`function ${name}(`);
  assert.notEqual(start, -1, `в scripts/gate.mjs нет функции ${name}`);
  let depth = 0;
  for (let index = source.indexOf("{", start); index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    else if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(start, index + 1);
    }
  }
  return assert.fail(`${name}: непарные скобки`);
}

/**
 * Исполняет чистую функцию из gate.mjs в изоляции.
 * @param {string} name
 * @returns {(...args: any[]) => any}
 */
function loadGateFunction(name) {
  return new Function(`${extractFunction(gateSource, name)}; return ${name};`)();
}

/** Токены, которыми тест можно «позеленить» без выполнения. */
const FORBIDDEN = [
  { pattern: /(?:^|[^.\w])t\.skip\s*\(/m, label: "t.skip(" },
  { pattern: /\b(?:it|test|describe)\.skip\s*\(/m, label: "*.skip(" },
  { pattern: /\bskip\s*:\s*(?:true|"|'|`)/m, label: "skip: <причина>" },
  { pattern: /\b(?:it|test|describe)\.only\s*\(/m, label: ".only(" },
  { pattern: /\b(?:it|test)\.todo\s*\(/m, label: ".todo(" },
  { pattern: /assert\.ok\(\s*true\s*\)/m, label: "assert.ok с литералом true" },
];

describe("M2 e2e: обязательный browser-набор", () => {
  it("реестр сценариев объявляет ровно 7 уникальных M2-сценариев", () => {
    assert.equal(M2_SCENARIOS.length, 7, "в реестре не 7 сценариев");
    const ids = M2_SCENARIOS.map((scenario) => scenario.id);
    assert.deepEqual(ids, ["M2-01", "M2-02", "M2-03", "M2-04", "M2-05", "M2-06", "M2-07"]);
    const files = M2_SCENARIOS.map((scenario) => scenario.file);
    assert.equal(new Set(files).size, 7, "файлы сценариев повторяются");
    for (const file of files) assert.match(file, /^tests\/e2e\/m2-[a-z-]+\.e2e\.test\.mjs$/);
  });

  for (const scenario of M2_SCENARIOS) {
    it(`${scenario.id}: ${scenario.title} — файл есть, сценарий помечен, skip-пути нет`, () => {
      const source = read(scenario.file);
      assert.match(source, new RegExp(`\\b${scenario.id}\\b`), `${scenario.file}: нет метки ${scenario.id}`);
      assert.match(source, /describe\(/, `${scenario.file}: нет describe`);
      assert.match(source, /\bit\(/, `${scenario.file}: нет it`);
      assert.match(source, /assert\./, `${scenario.file}: нет ни одного assert`);
      assert.match(
        source,
        /open(?:UiFixture|App)\(/,
        `${scenario.file}: сценарий не поднимает общий harness (openUiFixture/openApp)`,
      );
      assert.doesNotMatch(
        source,
        /launchBrowser\(|findBrowser\(|makeTmpDir\(/,
        `${scenario.file}: сценарий обходит harness (собственный запуск браузера)`,
      );
      for (const rule of FORBIDDEN) {
        assert.doesNotMatch(source, rule.pattern, `${scenario.file}: запрещённый токен ${rule.label}`);
      }
    });
  }

  it("harness и fake-транспорт существуют, без skip-пути", () => {
    for (const path of ["tests/helpers/ui-fixture.mjs", "tests/helpers/fake-tauri.mjs"]) {
      const source = read(path);
      assert.match(source, /^\/\/ @ts-check/m, `${path}: нет // @ts-check`);
      assert.doesNotMatch(source, /(?:^|[^.\w])t\.skip\s*\(/m, `${path}: skip-путь в harness`);
    }
    // Сценарии не должны сами решать «нет браузера → пропустить».
    assert.match(read("tests/helpers/ui-fixture.mjs"), /class InfraError|InfraError\s*=/);
  });

  it("нет браузера ⇒ InfraError (fail), а не skip", () => {
    // Пустое окружение: и CHROME_BIN, и PATH. Сканирование настоящего PATH не должно подмешиваться.
    assert.equal(detectBrowser({ PATH: "", CHROME_BIN: "", CHROME_PATH: "" }), null);
    assert.throws(
      () => requireBrowser({ PATH: "", CHROME_BIN: "", CHROME_PATH: "" }),
      (error) => {
        assert.equal(/** @type {any} */ (error).code, "INFRA_NO_BROWSER");
        assert.equal(/** @type {any} */ (error).infra, true);
        assert.match(/** @type {Error} */ (error).message, /^INFRA:/);
        return true;
      },
    );
    // А с валидным (но несуществующим) путём — тоже fail, а не «повезёт».
    assert.throws(
      () => requireBrowser({ PATH: "", CHROME_BIN: "/nonexistent/chromium" }),
      (error) => /** @type {any} */ (error).code === "INFRA_NO_BROWSER",
    );
  });

  it("fake transport: ok/err/seq/byArg, журнал вызовов, persist настроек, события, audio", async () => {
    /** @type {Map<string, string>} */
    const storage = new Map();
    /** @type {any} */
    const sandbox = {
      console: { error() {}, warn() {}, log() {} },
      navigator: { mediaDevices: { getUserMedia: async () => ({}) } },
      document: {},
      localStorage: {
        /** @param {string} key @returns {string | null} */
        getItem: (key) => storage.get(key) ?? null,
        /** @param {string} key @param {string} value */
        setItem: (key, value) => void storage.set(key, String(value)),
        /** @param {string} key */
        removeItem: (key) => void storage.delete(key),
      },
      JSON,
      Promise,
      Object,
      Array,
      String,
      Number,
      Date,
      Error,
      RegExp,
      Math,
    };
    const context = vm.createContext(sandbox);
    sandbox.window = sandbox;
    vm.runInContext(bootstrapSource({
      echo: { ok: { hello: "world" } },
      boom: { err: { kind: "forbidden", status: 403 } },
      tick: { seq: [{ ok: 1 }, { ok: 2 }] },
      byArg: {
        byArg: {
          key: "guildId",
          map: { "111": { ok: "A" }, "222": { ok: "B" } },
          fallback: { ok: "none" },
        },
      },
    }), context);

    const qa = sandbox.__QA__;
    assert.ok(qa, "нет window.__QA__");
    const invoke = sandbox.__TAURI__.core.invoke;

    assert.equal(JSON.stringify(await invoke("echo", {})), JSON.stringify({ hello: "world" }));
    assert.equal(
      JSON.stringify(qa.callsOf("echo").map((/** @type {any} */ call) => call.args)),
      JSON.stringify([{}]),
    );
    await assert.rejects(() => invoke("boom"), (error) => /** @type {any} */ (error).kind === "forbidden");
    assert.equal(await invoke("tick"), 1);
    assert.equal(await invoke("tick"), 2);
    assert.equal(await invoke("tick"), 2, "последний элемент seq должен повторяться");
    assert.equal(await invoke("byArg", { guildId: "111" }), "A");
    assert.equal(await invoke("byArg", { guildId: "222" }), "B");
    assert.equal(await invoke("byArg", { guildId: "333" }), "none");
    await assert.rejects(
      () => invoke("nope"),
      (error) => /** @type {any} */ (error).kind === "unknownCommand",
      "неизвестная команда должна падать видимо, а не отдавать undefined",
    );

    // Настройки переживают «reload»: состояние в localStorage, не в памяти страницы.
    await invoke("settings_save", { settings: { version: 1, toggles: {}, density: "compact", theme: "light", locale: "ru" } });
    const loaded = await invoke("settings_load");
    assert.equal(loaded.density, "compact");
    assert.equal(loaded.theme, "light");

    // Журнал: boot-команды с дефолтами и счётчики, а не «что-нибудь».
    assert.equal(qa.count("report_ui_snapshot"), 0);
    await invoke("report_ui_snapshot", { payload: "{}" });
    assert.equal(qa.count("report_ui_snapshot"), 1);

    // События Tauri v2: listen → emit с payload.
    /** @type {any[]} */
    const received = [];
    const unlisten = await sandbox.__TAURI__.event.listen("gateway://dispatch", (/** @type {any} */ event) => {
      received.push(event.payload);
    });
    qa.emit("gateway://dispatch", { t: "READY" });
    assert.equal(JSON.stringify(received), JSON.stringify([{ t: "READY" }]));
    unlisten();
    qa.emit("gateway://dispatch", { t: "READY" });
    assert.equal(received.length, 1, "unlisten не снял обработчик");

    // Аудио: инструментация обязана считать вызовы и блокировать захват устройства.
    assert.equal(qa.audio.getUserMedia, 0);
    await assert.rejects(() => sandbox.navigator.mediaDevices.getUserMedia({ audio: true }));
    assert.equal(qa.audio.getUserMedia, 1, "вызов getUserMedia не посчитан");
    qa.reset();
    assert.equal(qa.calls.length, 0);
  });

  it("fake transport не содержит утечек секретов по умолчанию", () => {
    const source = bootstrapSource({});
    assert.doesNotMatch(source, /MTk4NjIyNDgzNDcxOTI1MjQ4\./, "в дефолтном транспорте настоящий токен");
    assert.doesNotMatch(source, /password\s*[:=]\s*["'][^"']+["']/i, "в дефолтном транспорте пароль");
  });

  it("гейт: skipped>0 в e2e — failed, а не зелёный pass (failOnSkips + minPass)", () => {
    const phaseOutcome = loadGateFunction("phaseOutcome");
    const skippedRun = phaseOutcome({ failOnSkips: true, minPass: 1 }, { pass: 0, fail: 0, skipped: 7 }, true);
    assert.equal(skippedRun.ok, false, "гейт принял прогон из одних skip");
    assert.equal(skippedRun.skipFailure, true);
    const emptyRun = phaseOutcome({ failOnSkips: true, minPass: 7 }, { pass: 0, fail: 0, skipped: 0 }, true);
    assert.equal(emptyRun.ok, false, "гейт принял прогон без выполненных тестов (minPass)");
    assert.equal(emptyRun.unmet, true);
    const goodRun = phaseOutcome({ failOnSkips: true, minPass: 7 }, { pass: 7, fail: 0, skipped: 0 }, true);
    assert.equal(goodRun.ok, true);

    const fixturePhase = extractFunction(gateSource, "e2eFixturePhase");
    assert.match(fixturePhase, /failOnSkips: true/, "у fixture-фазы e2e нет failOnSkips");
    assert.match(fixturePhase, /minPass: \d+/, "у fixture-фазы e2e нет minPass");
    assert.match(fixturePhase, /--test-concurrency=1/, "e2e гоняется параллельно (Chrome нельзя параллелить)");
    assert.match(fixturePhase, /browserPreflight\(\)/, "нет preflight-диагностики браузера");
    assert.match(extractFunction(gateSource, "browserPreflight"), /e2e_unmet/, "preflight не помечает отсутствие браузера");
  });

  it("ui-сервер: настоящий index.html + инъекция fake-транспорта, app.js не переписан", async () => {
    assert.equal(m2UiPresent(), true, "rust/gui-spike/ui/index.html или app.js отсутствует");
    const server = await startUiServer({ bootstrap: "globalThis.__M2_BOOT__ = 1;" });
    try {
      const response = await fetch(server.url);
      assert.equal(response.status, 200);
      assert.match(response.headers.get("content-type") ?? "", /text\/html/);
      const html = await response.text();
      const injectedAt = html.indexOf('<script data-qa="fake-tauri">');
      const moduleAt = html.indexOf('<script type="module" src="app.js">');
      assert.ok(injectedAt >= 0, "в HTML нет инъекции fake __TAURI__");
      assert.ok(moduleAt > injectedAt, "инъекция стоит после module-скрипта (транспорт не успеет)");
      assert.equal(
        html.split('<script type="module" src="app.js">').length - 1,
        1,
        "module-скрипт app.js продублирован инъекцией",
      );

      // app.js и его статические импорты отдаются байт-в-байт: harness не подменяет продукт.
      const app = await fetch(new URL("app.js", server.url));
      assert.equal(app.status, 200);
      assert.equal(await app.text(), read("rust/gui-spike/ui/app.js"), "app.js изменён сервером");

      const appSource = read("rust/gui-spike/ui/app.js");
      /** @type {Set<string>} */
      const specifiers = new Set();
      for (const match of appSource.matchAll(/(?:from|import)\s+"([^"]+)"/g)) {
        const specifier = match[1] ?? "";
        if (specifier.startsWith(".")) specifiers.add(specifier);
      }
      assert.ok(specifiers.size >= 8, `в app.js найдено ${specifiers.size} относительных импортов`);
      for (const specifier of specifiers) {
        const url = new URL(specifier, new URL("app.js", server.url));
        const module = await fetch(url);
        assert.equal(module.status, 200, `модуль ${specifier} не отдаётся сервером`);
        assert.match(module.headers.get("content-type") ?? "", /javascript/, `${specifier}: неверный MIME`);
      }
      for (const match of html.matchAll(/href="([^"]+\.css)"/g)) {
        const css = await fetch(new URL(match[1] ?? "", server.url));
        assert.equal(css.status, 200, `css ${match[1]} не отдаётся сервером`);
      }

      // Выход за корень запрещён: иначе сервер отдал бы продуктовый репозиторий целиком.
      const escape = await fetch(server.url.replace("/index.html", "/%2e%2e%2fpackage.json"));
      assert.equal(escape.status, 403, "path traversal не закрыт");
    } finally {
      await server.close();
    }
  });

  it("все 7 файлов попадают в fixture-набор гейта (и live-* исключён)", () => {
    const dir = join(root, "tests", "e2e");
    const fixtureFiles = readdirSync(dir, { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.endsWith(".e2e.test.mjs") && !entry.name.startsWith("live-"))
      .map((entry) => `tests/e2e/${entry.name}`)
      .sort();
    const expected = M2_SCENARIOS.map((scenario) => scenario.file).sort();
    for (const file of expected) {
      assert.ok(fixtureFiles.includes(file), `${file} не попадёт в fixture-фазу e2e`);
    }
    const gateFiles = extractFunction(gateSource, "fixtureE2eFiles");
    assert.match(gateFiles, /startsWith\("live-"\)/, "гейт не исключает live-файл из fixture-фазы");
  });
});
