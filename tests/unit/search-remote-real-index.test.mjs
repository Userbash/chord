// @ts-check
/**
 * QA-M2-03 / P1A: regression удалённого поиска на реальном `ui/index.html`.
 *
 * Старый false-green: `app.js` снимал `dataset.guild` на boot, а рейл G-01
 * строит плитки с `data-guild-id`; в реальном продукте рейл до `ready()` пуст,
 * поэтому `guildId === ""` и `search_remote` не вызывался. DOM-шим это
 * маскировал: в нём были статичные плитки с `data-guild`.
 *
 * Здесь шим-заглушки удаляются до импорта `app.js` — boot видит пустой рейл,
 * как настоящий `index.html` (в нём нет ни одного `data-guild`). Гильдии и их
 * snowflake берутся из независимого оракула `fixtures/gateway/by-type/002-READY.json`.
 * На старом коде тест обязан падать: `search_remote` не будет вызван вовсе.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { GuiDom } from "../support/gui-dom-harness.mjs";
import * as searchMod from "../../rust/gui-spike/ui/features/search.js";
import { buildSearchQuery, searchDateBounds } from "../../rust/gui-spike/ui/features/search.js";

/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const APP_PATH = fileURLToPath(new URL("../../rust/gui-spike/ui/app.js", import.meta.url));
const HTML = readFileSync(
  fileURLToPath(new URL("../../rust/gui-spike/ui/index.html", import.meta.url)),
  "utf8",
);
const FIXTURE = JSON.parse(
  readFileSync(
    fileURLToPath(new URL("../../fixtures/gateway/by-type/002-READY.json", import.meta.url)),
    "utf8",
  ),
);

describe("M-13 review regressions", () => {
  it("CSS tokens used by the M-13 block are declared in tokens.css", () => {
    const css = readFileSync(fileURLToPath(new URL("../../rust/gui-spike/ui/components/chat.css", import.meta.url)), "utf8");
    const tokens = readFileSync(fileURLToPath(new URL("../../rust/gui-spike/ui/tokens.css", import.meta.url)), "utf8");
    const block = css.slice(css.indexOf("M-13 (RW-40): HTTP 202 Indexing Progress"));
    const used = new Set([...block.matchAll(/var\(--([\w-]+)/g)].map((match) => match[1]));
    const declared = new Set([...tokens.matchAll(/--([\w-]+)\s*:/g)].map((match) => match[1]));
    assert.deepEqual([...used].filter((token) => !declared.has(token)), []);
  });

  it("styles M-13 with the normative chip classes and §6 banner colors", () => {
    const css = readFileSync(fileURLToPath(new URL("../../rust/gui-spike/ui/components/chat.css", import.meta.url)), "utf8");
    const start = css.indexOf("M-13 (RW-40): HTTP 202 Indexing Progress");
    assert.notEqual(start, -1, "нет M-13 блока в chat.css");
    const block = css.slice(start);
    // §2.1/§6: нормативные span-классы чипов, а не локальные key/val.
    assert.match(block, /\.search-chip-prefix\s*\{/, "нет правила .search-chip-prefix");
    assert.match(block, /\.search-chip-value\s*\{/, "нет правила .search-chip-value");
    assert.doesNotMatch(block, /\.search-chip-key\s*\{/, "локальный .search-chip-key не по спецификации");
    assert.doesNotMatch(block, /\.search-chip-val\s*\{/, "локальный .search-chip-val не по спецификации");
    const ruleBody = (/** @type {string} */ selector) => {
      const index = block.indexOf(`${selector} {`);
      assert.notEqual(index, -1, `нет правила ${selector}`);
      return block.slice(index, block.indexOf("}", index));
    };
    // P1-2/§2.2: бейдж 18+ и кнопка «Продолжить» — --danger-fill/--text-on-danger.
    assert.doesNotMatch(block, /background:\s*var\(--danger\)\s*;/, "фон на --danger (4.42:1 < AA)");
    const continueRule = ruleBody(".search-nsfw-btn-continue");
    assert.match(continueRule, /background:\s*var\(--danger-fill\)/);
    assert.match(continueRule, /color:\s*var\(--text-on-danger\)/);
    // P2-2/§6: бейдж индексации — --warning/--text-on-warning.
    const badge = ruleBody(".search-indexing-badge");
    assert.match(badge, /background:\s*var\(--warning\)/);
    assert.match(badge, /color:\s*var\(--text-on-warning\)/);
    // §0.2.1: сырых цветов в блоке нет.
    assert.doesNotMatch(block, /#[0-9a-fA-F]{3,8}\b|rgba?\(|hsla?\(/);
  });

  it("uses the IPC retry delay after an invalid Retry-After and skips invalid date while applying before", () => {
    assert.equal(searchMod.computeIndexingRetryDelayMs({ headers: { "retry-after": "0" }, retry_after_ms: 2000 }), 2000);
    assert.equal(searchMod.computeIndexingRetryDelayMs({ headers: { "retry-after": "не-число" }, retry_after_ms: 2500 }), 2500);
    assert.deepEqual(searchDateBounds({ date: "2026-02-31", before: "2026-03-01" }), {
      max_id: searchMod.snowflakeFromTimestamp(Date.parse("2026-03-01T00:00:00.000Z")),
    });
    assert.equal(searchMod.isAgeGatedSpacesRestricted({ age_verification_status: 2 }, undefined), false);
  });

  it("persists NSFW agreement through accessor localStorage across overlay instances", async () => {
    const values = new Map();
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      get() {
        return {
          /** @param {string} key @returns {string | null} */
          getItem: (key) => values.get(key) ?? null,
          /** @param {string} key @param {string} value */
          setItem: (key, value) => values.set(key, String(value)),
        };
      },
    });
    const guild = { id: "guild-storage", nsfw_level: 0, age_gated_features: 1 };
    const user = { age_verification_status: 3 };
    const create = async () => {
      const env = await bootWithRealRail((cmd) => cmd === "search_remote"
        ? { messages: [], total_results: 0, doing_deep_historical_index: false }
        : undefined);
      const search = /** @type {any} */ (api(env).search);
      search.setNsfwContext({ guild, user, regionalFeatureConfig: { age_gated_features: 1 }, channels: [{ id: "adult", nsfw: true }] });
      return env;
    };
    const first = await create();
    try {
      const input = first.document.querySelector(".search-overlay-input");
      assert.ok(input);
      input.value = "поиск";
      input.emit("input");
      filterControl(first, "in").value = "adult";
      filterControl(first, "in").emit("input");
      const panel = /** @type {ShimElement} */ (first.document.querySelector(".search-overlay"));
      panel.querySelector(".search-nsfw-btn-continue")?.click();
      await api(first).search.whenIdle();
      // Ключ — id выбранной в рейле гильдии (`selectedGuildId()`), а не контекстный.
      assert.deepEqual(JSON.parse(values.get("GuildNSFWAgreeStore")), { [selectedGuild(first)]: true });
    } finally {
      first.dispose();
    }
    const second = await create();
    try {
      filterControl(second, "in").value = "adult";
      filterControl(second, "in").emit("input");
      const panel = /** @type {ShimElement} */ (second.document.querySelector(".search-overlay"));
      assert.equal(panel.querySelector(".search-nsfw-gate")?.hasAttribute("hidden"), true);
    } finally {
      second.dispose();
      delete /** @type {any} */ (globalThis).localStorage;
    }
  });

  it("persists NSFW agreement through a plain data-property localStorage", async () => {
    const values = new Map();
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      writable: true,
      value: {
        /** @param {string} key @returns {string | null} */
        getItem: (key) => values.get(key) ?? null,
        /** @param {string} key @param {string} value */
        setItem: (key, value) => values.set(key, String(value)),
      },
    });
    const guild = { id: "guild-storage", nsfw_level: 0 };
    const user = { age_verification_status: 3 };
    const env = await bootWithRealRail((cmd) => cmd === "search_remote"
      ? { messages: [], total_results: 0, doing_deep_historical_index: false }
      : undefined);
    try {
      const search = /** @type {any} */ (api(env).search);
      search.setNsfwContext({
        guild,
        user,
        regionalFeatureConfig: { age_gated_features: 0 },
        channels: [{ id: "adult", nsfw: true }],
      });
      filterControl(env, "in").value = "adult";
      filterControl(env, "in").emit("input");
      const panel = /** @type {ShimElement} */ (env.document.querySelector(".search-overlay"));
      panel.querySelector(".search-nsfw-btn-continue")?.click();
      await api(env).search.whenIdle();
      assert.equal(
        values.get("GuildNSFWAgreeStore"),
        JSON.stringify({ [selectedGuild(env)]: true }),
        "согласие не записано через data-property localStorage",
      );
    } finally {
      env.dispose();
      delete /** @type {any} */ (globalThis).localStorage;
    }
  });

  it("renders normative chip spans, data-chip-remove and removes quoted tokens", async () => {
    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return { messages: [], total_results: 0, doing_deep_historical_index: false };
    });
    try {
      const panel = await searchRemote(env, 'from:"Alex Mercer" has:image релиз');
      const chip = /** @type {ShimElement | null} */ (
        panel.querySelector('.search-chip[data-chip-key="from"]')
      );
      assert.ok(chip, "нет чипа from:");
      assert.ok(chip.querySelector("span.search-chip-prefix"), "нет span.search-chip-prefix");
      assert.ok(chip.querySelector("span.search-chip-value"), "нет span.search-chip-value");
      const remove = /** @type {ShimElement | null} */ (chip.querySelector(".search-chip-remove"));
      assert.ok(remove, "нет .search-chip-remove");
      assert.equal(remove.getAttribute("data-chip-remove"), "from:Alex Mercer");

      remove.click();
      await api(env).search.whenIdle();
      const input = /** @type {ShimElement} */ (panel.querySelector(".search-overlay-input"));
      assert.doesNotMatch(input.value, /from:/, "кавычный токен не удалён из строки запроса");
      const lastQuery = calls[calls.length - 1]?.params?.query ?? {};
      assert.equal("author_id" in lastQuery, false, "author_id остался после удаления чипа");
      assert.deepEqual(lastQuery.has, ["image"], "удаление чипа снесло соседний фильтр");
    } finally {
      env.dispose();
    }
  });

  it("synchronises .search-sections controls from inline tokens (two-way)", async () => {
    const env = await bootWithRealRail((cmd) => cmd === "search_remote"
      ? { messages: [], total_results: 0, doing_deep_historical_index: false }
      : undefined);
    try {
      await searchRemote(env, "from:777001 in:1001 has:image день:2026-03-01 pinned:true релиз");
      assert.equal(filterControl(env, "from").value, "777001", "контрол «От» не заполнен из from:");
      assert.equal(filterControl(env, "in").value, "1001", "контрол «В» не заполнен из in:");
      assert.equal(filterControl(env, "has").value, "image", "контрол «Содержит» не заполнен из has:");
      assert.equal(filterControl(env, "date").value, "2026-03-01", "контрол «Дата» не заполнен из день:");
      assert.equal(
        filterControl(env, "pinned").getAttribute("aria-checked"),
        "true",
        "переключатель «Закреплено» не синхронизирован из pinned:true",
      );
    } finally {
      env.dispose();
    }
  });

  it("moves focus between chips and back to the input (§7.2)", async () => {
    const env = await bootWithRealRail((cmd) => cmd === "search_remote"
      ? { messages: [], total_results: 0, doing_deep_historical_index: false }
      : undefined);
    try {
      const panel = await searchRemote(env, "from:777001 in:1001");
      const input = /** @type {ShimElement} */ (panel.querySelector(".search-overlay-input"));
      const removeButtons = /** @type {ShimElement[]} */ (panel.querySelectorAll(".search-chip-remove"));
      assert.equal(removeButtons.length, 2);

      input.focus();
      input.setSelectionRange(0, 0);
      input.dispatchEvent("keydown", { key: "Backspace" });
      assert.ok(
        env.document.activeElement === removeButtons[1],
        "Backspace в позиции 0 не перевёл фокус на последний .search-chip-remove",
      );

      removeButtons[1].dispatchEvent("keydown", { key: "ArrowLeft" });
      assert.ok(env.document.activeElement === removeButtons[0], "ArrowLeft не перевёл фокус на предыдущий чип");
      removeButtons[0].dispatchEvent("keydown", { key: "ArrowRight" });
      assert.ok(env.document.activeElement === removeButtons[1], "ArrowRight не вернул фокус на следующий чип");

      removeButtons[1].dispatchEvent("keydown", { key: "Backspace" });
      await api(env).search.whenIdle();
      assert.equal(panel.querySelectorAll(".search-chip").length, 1, "чип не удалён по Backspace");
      assert.ok(
        env.document.activeElement === panel.querySelectorAll(".search-chip-remove")[0],
        "после удаления фокус не перешёл на соседний чип",
      );

      removeButtons[0].click();
      await api(env).search.whenIdle();
      assert.equal(panel.querySelectorAll(".search-chip").length, 0);
      assert.ok(
        env.document.activeElement === input,
        "после удаления последнего чипа фокус не вернулся в .search-overlay-input",
      );
    } finally {
      env.dispose();
    }
  });

  it("returns focus to the input after .search-chips-clear", async () => {
    const env = await bootWithRealRail((cmd) => cmd === "search_remote"
      ? { messages: [], total_results: 0, doing_deep_historical_index: false }
      : undefined);
    try {
      const panel = await searchRemote(env, "from:777001 релиз");
      const clearBtn = /** @type {ShimElement} */ (panel.querySelector(".search-chips-clear"));
      clearBtn.click();
      assert.ok(
        env.document.activeElement === panel.querySelector(".search-overlay-input"),
        "фокус не вернулся в .search-overlay-input после сброса чипов",
      );
      assert.equal(clearBtn.getAttribute("aria-label"), "Сбросить все фильтры поиска");
    } finally {
      env.dispose();
    }
  });

  it("never steals focus on repeated refreshes while the NSFW gate is open", async () => {
    const env = await bootWithRealRail((cmd) => cmd === "search_remote"
      ? { messages: [], total_results: 0, doing_deep_historical_index: false }
      : undefined);
    try {
      const search = /** @type {any} */ (api(env).search);
      search.setNsfwContext({
        guild: { id: "guild-nsfw", nsfw_level: 1 },
        user: { age_verification_status: 3 },
        regionalFeatureConfig: { age_gated_features: 0 },
      });
      const panel = await searchRemote(env, "поиск");
      const gate = /** @type {ShimElement} */ (panel.querySelector(".search-nsfw-gate"));
      assert.equal(gate.hasAttribute("hidden"), false, "NSFW-гильдия не открыла гейт");
      assert.equal(
        gate.querySelector(".search-nsfw-title")?.textContent,
        "Сервер с возрастным ограничением",
      );
      const input = /** @type {ShimElement} */ (panel.querySelector(".search-overlay-input"));
      input.focus();
      input.value = "поиск2";
      input.emit("input");
      assert.ok(
        env.document.activeElement === input,
        "фокус украден с поля ввода на повторном refresh при открытом гейте",
      );
    } finally {
      env.dispose();
    }
  });

  it("gates guild-wide search over NSFW channels (scenario B) and clears results on gate", async () => {
    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return {
        messages: [{ ...serverMessage("9710", "18+ хит"), channel_id: "777999", nsfw: true }],
        total_results: 1,
        doing_deep_historical_index: false,
      };
    });
    try {
      const search = /** @type {any} */ (api(env).search);
      search.setNsfwContext({
        guild: { id: "guild-mixed", nsfw_level: 0 },
        user: { age_verification_status: 3 },
        regionalFeatureConfig: { age_gated_features: 0 },
        channels: [{ id: "777999", nsfw: true }],
      });
      const panel = await searchRemote(env, "общий");
      assert.equal(calls.length, 1, "общегильдийский запрос должен уйти");
      assert.equal("include_nsfw" in (calls[0]?.params?.query ?? {}), false);
      const gate = /** @type {ShimElement} */ (panel.querySelector(".search-nsfw-gate"));
      assert.equal(gate.hasAttribute("hidden"), false, "баннер над NSFW-хитами не показан");
      assert.equal(
        gate.querySelector(".search-nsfw-desc")?.textContent,
        "В этом канале содержится контент для взрослых с возрастными ограничениями. Хотите продолжить?",
      );
      assert.equal(
        panel.querySelectorAll('.search-result[data-id="9710"]').length,
        0,
        "NSFW-хит показан без согласия",
      );
    } finally {
      env.dispose();
    }
  });

  it("clears status and results after «Вернуться» from the gate", async () => {
    const env = await bootWithRealRail((cmd) => cmd === "search_remote"
      ? { messages: [], total_results: 0, doing_deep_historical_index: false }
      : undefined);
    try {
      const search = /** @type {any} */ (api(env).search);
      search.setNsfwContext({
        guild: { id: "guild-mixed", nsfw_level: 0 },
        user: { age_verification_status: 3 },
        channels: [{ id: "adult", nsfw: true }],
      });
      const panel = await searchRemote(env, "поиск");
      filterControl(env, "in").value = "adult";
      filterControl(env, "in").emit("input");
      const gate = /** @type {ShimElement} */ (panel.querySelector(".search-nsfw-gate"));
      assert.equal(gate.hasAttribute("hidden"), false);
      gate.querySelector(".search-nsfw-btn-back")?.click();
      assert.equal(gate.hasAttribute("hidden"), true, "гейт не скрылся");
      assert.equal(panel.querySelector(".search-status")?.textContent, "", "статус остался от прежнего запроса");
      assert.equal(panel.querySelectorAll(".search-result").length, 0, "выдача осталась под баннером");
      assert.ok(
        env.document.activeElement === panel.querySelector(".search-overlay-input"),
        "фокус не вернулся в поле ввода",
      );
    } finally {
      env.dispose();
    }
  });

  it("marks aside.search-overlay aria-busy during a request", async () => {
    /** @type {(value: any) => void} */
    let release = () => {};
    const env = await bootWithRealRail((cmd) => {
      if (cmd !== "search_remote") return undefined;
      return new Promise((resolve) => {
        release = resolve;
      });
    });
    try {
      const header = /** @type {ShimElement} */ (env.document.getElementById("chat-search"));
      header.value = "busy";
      header.emit("input");
      await env.settle(5);
      const panel = /** @type {ShimElement} */ (env.document.querySelector(".search-overlay"));
      assert.equal(panel.getAttribute("aria-busy"), "true", "aria-busy не выставлен во время запроса");
      release({ messages: [], total_results: 0, doing_deep_historical_index: false });
      await api(env).search.whenIdle();
      assert.equal(panel.getAttribute("aria-busy"), "false", "aria-busy не снят после ответа");
    } finally {
      env.dispose();
    }
  });

  it("uses catalog strings for exhausted, historical and prompt states", async () => {
    let attempt = 0;
    const env = await bootWithRealRail((cmd) => {
      if (cmd !== "search_remote") return undefined;
      attempt += 1;
      if (attempt === 1) return { status: 202, headers: { "Retry-After": "1" } };
      if (attempt <= 6) return { indexing: true, retry_after_ms: 1000 };
      return { messages: [], total_results: 0, doing_deep_historical_index: false };
    });
    try {
      const panel = await searchRemote(env, "строки");
      const banner = /** @type {ShimElement} */ (panel.querySelector(".search-indexing-banner"));
      assert.equal(banner.querySelector(".search-indexing-title")?.textContent, "Индексация");
      assert.match(
        banner.querySelector(".search-indexing-detail")?.textContent ?? "",
        /^Нам нужно проиндексировать этот сервер перед поиском\. Дайте нам минуточку\./,
      );
      for (let i = 0; i < 5; i += 1) {
        env.flushTimers();
        await api(env).search.whenIdle();
      }
      assert.equal(banner.getAttribute("data-indexing-state"), "exhausted");
      assert.equal(
        banner.querySelector(".search-indexing-title")?.textContent,
        "Индексация занимает больше времени",
      );
      assert.equal(
        banner.querySelector(".search-indexing-detail")?.textContent,
        "Нам нужно проиндексировать этот сервер перед поиском. Дайте нам минуточку.",
      );
      assert.equal(banner.querySelector(".search-indexing-retry-btn")?.textContent, "Повторить запрос");
    } finally {
      env.dispose();
    }
  });

  it("shows the «Архив» badge on deep historical results", async () => {
    const env = await bootWithRealRail((cmd) => cmd === "search_remote"
      ? {
        messages: [serverMessage("9711", "архивный хит")],
        total_results: 1,
        doing_deep_historical_index: true,
        documents_indexed: 8450,
      }
      : undefined);
    try {
      const panel = await searchRemote(env, "архив");
      const banner = /** @type {ShimElement} */ (panel.querySelector(".search-indexing-banner"));
      assert.equal(banner.getAttribute("data-indexing-state"), "historical");
      assert.equal(banner.querySelector(".search-indexing-title")?.textContent, "Индексация");
      assert.equal(banner.querySelector(".search-indexing-badge")?.textContent, "Архив");
      assert.equal(
        banner.querySelector(".search-indexing-detail")?.textContent,
        "Найдено 8450 результатов. Мы всё ещё индексируем старые сообщения.",
      );
    } finally {
      env.dispose();
    }
  });
});

/** READY fixture → гильдии в форме `data.rs` (id — snowflake, имя — properties.name). */
const GUILDS = (FIXTURE?.d?.guilds ?? []).map((/** @type {any} */ guild) => ({
  id: String(guild.id),
  name: String(guild?.properties?.name ?? ""),
}));

/** @returns {any} */
function readyPayload() {
  return {
    user: { id: "42", username: "tester" },
    guilds: GUILDS.map((/** @type {{ id: string, name: string }} */ guild) => ({
      id: guild.id,
      name: guild.name,
      icon: null,
      memberCount: 3,
      channelCount: 2,
    })),
    guildCount: GUILDS.length,
    privateChannelCount: 0,
    relationshipCount: 0,
    readStateCount: 0,
    session: null,
  };
}

/** @param {string} id @param {string} content @returns {any} */
function serverMessage(id, content) {
  return {
    id,
    content,
    timestamp: "2026-10-20T10:05:00Z",
    channel_id: "1163408591315812392",
    author: { id: "42", username: "network_alpha", bot: false },
    pinned: false,
  };
}

let importSeq = 0;

/**
 * Boot настоящего `app.js` без статичных плиток шима: вход уже пройден
 * (`auth_status` → ready), рейл наполняется только `ready()`.
 * @param {(cmd: string, args: any) => any} handler
 * @returns {Promise<GuiDom>}
 */
async function bootWithRealRail(handler) {
  const env = new GuiDom({
    appPath: APP_PATH,
    invokeHandler: (cmd, args) => {
      if (cmd === "auth_status") {
        return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
      }
      if (cmd === "ready") return readyPayload();
      if (cmd === "channels") return [];
      return handler(cmd, args);
    },
  });
  // В `ui/index.html` нет статичных плиток; убираем и заглушки шима, чтобы
  // boot видел ровно то, что видит продукт — пустой рейл до `ready()`.
  for (const stub of env.document.querySelectorAll(".guild-item")) stub.remove();
  env.installGlobals();
  env.appModule = await import(`${pathToFileURL(APP_PATH).href}?real-rail=${++importSeq}`);
  await env.settle(30);
  return env;
}

/**
 * @param {GuiDom} env
 * @returns {import("../../rust/gui-spike/ui/app.js").App}
 */
function api(env) {
  const instance = /** @type {any} */ (env.appModule)?.currentApp?.();
  assert.ok(instance, "app.js не экспортирует currentApp()");
  return instance;
}

/**
 * @param {GuiDom} env
 * @param {string} query
 * @returns {Promise<ShimElement>}
 */
async function searchRemote(env, query) {
  const header = /** @type {ShimElement | null} */ (env.document.getElementById("chat-search"));
  assert.ok(header, "нет #chat-search");
  header.value = query;
  header.emit("input");
  await api(env).search.whenIdle();
  const panel = /** @type {ShimElement | null} */ (env.document.querySelector(".search-overlay"));
  assert.ok(panel, "нет .search-overlay");
  return panel;
}

/**
 * Контрол секции фильтров оверлея (R-10): input/select либо switch-кнопка.
 * @param {GuiDom} env
 * @param {string} key
 * @returns {ShimElement}
 */
function filterControl(env, key) {
  const found = /** @type {ShimElement | null} */ (
    env.document.querySelector(`.search-overlay .search-section[data-key="${key}"] .search-control`)
  );
  assert.ok(found, `нет контрола фильтра «${key}»`);
  return found;
}

/**
 * id гильдии, выбранной в рейле (источник `GuildNSFWAgreeStore` в оверлее).
 * @param {GuiDom} env
 * @returns {string}
 */
function selectedGuild(env) {
  const item = /** @type {ShimElement | null} */ (
    env.document.querySelector(".guild-item.is-selected")
  );
  const id = item?.getAttribute("data-guild-id") ?? "";
  assert.notEqual(id, "", "рейл не выбрал ни одной гильдии");
  return id;
}

/**
 * STUB-14: независимый оракул snowflake по timestamp (Discord epoch 2015-01-01).
 * @param {string} iso
 * @returns {string}
 */
function snowflakeAt(iso) {
  return ((BigInt(Date.parse(iso)) - 1420070400000n) << 22n).toString();
}

describe("QA-M2-03: search_remote на реальном index.html (data-guild-id)", () => {
  it("после ready шлёт ровно один search_remote с guildId из data-guild-id", async () => {
    assert.doesNotMatch(HTML, /data-guild="\d+"/, "в index.html осталась статичная плитка data-guild");
    assert.equal(GUILDS.length, 7, "READY fixture изменилась: не 7 гильдий");
    const first = GUILDS[0];
    assert.ok(first && first.id !== "" && first.name !== "", "в READY нет первой гильдии");

    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return {
        messages: [serverMessage("9001", "привет из сети")],
        total_results: 1,
        doing_deep_historical_index: false,
      };
    });
    try {
      const tiles = env.document.querySelectorAll(".guild-item");
      assert.equal(tiles.length, GUILDS.length, "рейл после ready не из payload");
      assert.equal(tiles[0]?.getAttribute("data-guild-id"), first.id, "плитка не из data-guild-id");
      assert.equal(tiles[0]?.getAttribute("data-guild"), null, "плитка несёт legacy data-guild");
      assert.equal(
        env.document.querySelectorAll("[data-guild]").length,
        0,
        "в DOM есть legacy data-guild (оракул подменён шимом)",
      );

      const panel = await searchRemote(env, "привет");
      await env.settle(2);

      assert.equal(calls.length, 1, `search_remote вызван ${calls.length} раз(а), нужен ровно один`);
      assert.equal(calls[0]?.guildId, first.id, "guildId не из выбранной плитки data-guild-id");
      assert.equal(calls[0]?.params?.content, "привет", "content не из запроса");
      assert.equal(panel.querySelectorAll(".search-result").length, 1, "серверная выдача не отрисована");
      assert.equal(panel.querySelector(".search-status")?.getAttribute("data-state"), "ok");
      assert.equal(api(env).search.mode, "remote", "режим не переключился на сервер");
    } finally {
      env.dispose();
    }
  });

  it("STUB-14: передаёт все подтверждённые фильтры поиска в query (§2)", async () => {
    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return {
        messages: [serverMessage("9401", "нашлось по фильтрам")],
        total_results: 1,
        doing_deep_historical_index: false,
      };
    });
    try {
      filterControl(env, "from").value = "42";
      filterControl(env, "mentions").value = "77";
      // «файл» — подпись UI (rows.js HAS_OPTIONS), на сервер идёт токен §2.3 `file`.
      filterControl(env, "has").value = "файл";
      filterControl(env, "in").value = "1163408591315812392";
      const author = filterControl(env, "author");
      author.value = "Бот";
      author.emit("change");
      filterControl(env, "pinned").click();

      await searchRemote(env, "привет");
      assert.equal(calls.length, 1, `search_remote вызван ${calls.length} раз(а)`);
      const params = calls[0]?.params;
      assert.ok(params, "search_remote вызван без params");

      assert.deepEqual(
        params.query,
        {
          content: "привет",
          author_id: ["42"],
          mentions: ["77"],
          has: ["file"],
          channel_id: ["1163408591315812392"],
          pinned: true,
          author_type: "bot",
          limit: 25,
          sort_by: "timestamp",
          sort_order: "desc",
          offset: 0,
        },
        "query не совпал с подтверждёнными queryKey из RE-DEEP-REA §2.3–2.4",
      );
      // Принятый backend B7a (search.rs) читает camelCase-поля — их не ломаем.
      assert.equal(params.content, "привет", "content не из запроса");
      assert.equal(params.channelId, "1163408591315812392", "channelId не из фильтра «В»");
      assert.equal(params.sortBy, "timestamp");
      assert.equal(params.sortOrder, "desc");
      assert.equal(params.offset, 0);
      assert.equal(api(env).search.mode, "remote");
    } finally {
      env.dispose();
    }
  });

  it("STUB-14: дата → min_id/max_id, has — массив токенов, сортировка asc", async () => {
    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return { messages: [], total_results: 0, doing_deep_historical_index: false };
    });
    try {
      filterControl(env, "date").value = "2026-10-20";
      filterControl(env, "has").value = "картинка, видео";
      const panel = /** @type {ShimElement} */ (env.document.querySelector(".search-overlay"));
      /** @type {ShimElement} */ (panel.querySelector(".search-sort-trigger")).click();
      /** @type {ShimElement} */ (
        panel.querySelectorAll('.search-sort-menu [role="menuitemradio"]')[1]
      ).click();

      await searchRemote(env, "дата");
      assert.equal(calls.length, 1, `search_remote вызван ${calls.length} раз(а)`);
      const query = calls[0]?.params?.query;
      assert.ok(query, "query не передан");
      assert.deepEqual(query.has, ["image", "video"], "подписи «Содержит» не нормализованы в токены §2.3");
      // Независимый оракул snowflake: (timestamp_ms − DISCORD_EPOCH) << 22.
      assert.equal(query.min_id, snowflakeAt("2026-10-20T00:00:00.000Z"), "min_id не из начала дня");
      assert.equal(query.max_id, snowflakeAt("2026-10-20T23:59:59.999Z"), "max_id не из конца дня");
      assert.equal(query.sort_by, "timestamp");
      assert.equal(query.sort_order, "asc", "«Самое старое» не ушло как sort_order=asc");
      assert.equal(query.limit, 25, "limit не равен странице поиска (25)");
    } finally {
      env.dispose();
    }
  });

  it("STUB-14: пустые фильтры не уходят в query (только content/limit/sort/offset)", async () => {
    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return { messages: [], total_results: 0, doing_deep_historical_index: false };
    });
    try {
      await searchRemote(env, "просто текст");
      assert.equal(calls.length, 1, `search_remote вызван ${calls.length} раз(а)`);
      assert.deepEqual(
        calls[0]?.params?.query,
        {
          content: "просто текст",
          limit: 25,
          sort_by: "timestamp",
          sort_order: "desc",
          offset: 0,
        },
        "пустые фильтры попали в query",
      );
    } finally {
      env.dispose();
    }
  });

  it("unauthorized/forbidden/notFound — видимая ошибка без локального fallback", async () => {
    for (const kind of ["unauthorized", "forbidden", "notFound"]) {
      /** @type {any[]} */
      const calls = [];
      const env = await bootWithRealRail((cmd, args) => {
        if (cmd !== "search_remote") return undefined;
        calls.push(args);
        throw { kind };
      });
      try {
        const panel = await searchRemote(env, "привет");
        const status = /** @type {ShimElement | null} */ (panel.querySelector(".search-status"));
        assert.ok(status, `${kind}: нет .search-status`);
        assert.equal(calls.length, 1, `${kind}: search_remote вызван ${calls.length} раз(а)`);
        assert.equal(calls[0]?.guildId, GUILDS[0]?.id, `${kind}: guildId не из data-guild-id`);
        assert.equal(status.getAttribute("data-state"), "error", `${kind}: ошибка не видна`);
        assert.equal(panel.querySelectorAll(".search-result").length, 0, `${kind}: локальный fallback`);
        assert.equal(api(env).search.matchesCount, 0, `${kind}: сработал тихий локальный fallback`);
        assert.equal(api(env).search.mode, "remote", `${kind}: режим откатился в локальный`);
        assert.doesNotMatch(status.textContent ?? "", /загруженных/, `${kind}: остался оконный счётчик`);
      } finally {
        env.dispose();
      }
    }
  });
});

// RW-40 / STUB-14-UI-SEARCH-FIX: закрытие P1 и P2 из REVIEW.md.
// P1 — числовой snowflake в фильтрах «От»/«Упоминания» не равен отображаемому
// имени, поэтому локальный фильтр после remote-ответа не должен отбрасывать
// серверное сообщение, у которого совпал `author.id`/`mentions[].id`.
// P2 — `Date.parse("2026-02-31T…")` нормализует дату в март; валидность
// проверяется обратной сверкой UTC-компонентов с исходными `ГГГГ-ММ-ДД`.
describe("STUB-14-UI-SEARCH-FIX: числовые id и календарные даты (REVIEW P1/P2)", () => {
  it("P1: числовые author_id/mentions не отбрасывают совпавшее серверное сообщение", async () => {
    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return {
        messages: [
          {
            id: "9501",
            content: "совпало по id",
            timestamp: "2026-10-20T10:05:00Z",
            channel_id: "1163408591315812392",
            author: { id: "42", username: "network_alpha", bot: false },
            mentions: [{ id: "77", username: "mentioned_user" }],
            pinned: false,
          },
          {
            id: "9502",
            content: "другой автор и другое упоминание",
            timestamp: "2026-10-20T10:06:00Z",
            channel_id: "1163408591315812392",
            author: { id: "43", username: "network_beta", bot: false },
            mentions: [{ id: "78", username: "other_user" }],
            pinned: false,
          },
        ],
        total_results: 2,
        doing_deep_historical_index: false,
      };
    });
    try {
      filterControl(env, "from").value = "42";
      filterControl(env, "mentions").value = "77";

      const panel = await searchRemote(env, "привет");
      assert.equal(calls.length, 1, `search_remote вызван ${calls.length} раз(а)`);
      assert.deepEqual(calls[0]?.params?.query?.author_id, ["42"], "author_id не ушёл в query");
      assert.deepEqual(calls[0]?.params?.query?.mentions, ["77"], "mentions не ушёл в query");
      const ids = panel
        .querySelectorAll(".search-result")
        .map((node) => node.getAttribute("data-id"));
      assert.deepEqual(
        ids,
        ["9501"],
        "локальный фильтр по числовому id отбросил совпавшее серверное сообщение",
      );
      assert.equal(
        panel.querySelector(".search-status")?.getAttribute("data-state"),
        "ok",
        "выдача ушла в ошибку",
      );
    } finally {
      env.dispose();
    }
  });

  it("P2: невалидные календарные даты отклоняются, валидные считаются по UTC", () => {
    assert.equal(searchDateBounds("2026-02-31"), null, "2026-02-31 нормализована в март");
    assert.equal(searchDateBounds("2026-04-31"), null, "2026-04-31 принята как дата");
    assert.equal(searchDateBounds("2023-02-29"), null, "2023-02-29 принята вне високосного года");
    const leap = searchDateBounds("2024-02-29");
    assert.ok(leap, "2024-02-29 отклонена");
    assert.equal(leap.min_id, snowflakeAt("2024-02-29T00:00:00.000Z"));
    assert.equal(leap.max_id, snowflakeAt("2024-02-29T23:59:59.999Z"));
    const valid = searchDateBounds("2026-10-20");
    assert.ok(valid, "валидная дата отклонена");
    assert.equal(valid.min_id, snowflakeAt("2026-10-20T00:00:00.000Z"));
    assert.equal(valid.max_id, snowflakeAt("2026-10-20T23:59:59.999Z"));
  });

  it("P2: оверлей не шлёт запрос с min_id/max_id для невалидной даты", async () => {
    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return { messages: [], total_results: 0, doing_deep_historical_index: false };
    });
    try {
      filterControl(env, "date").value = "2026-02-31";
      const panel = await searchRemote(env, "привет");
      assert.equal(calls.length, 0, "для даты 2026-02-31 ушёл серверный запрос");
      const status = /** @type {ShimElement | null} */ (panel.querySelector(".search-status"));
      assert.ok(status, "нет .search-status");
      assert.equal(status.getAttribute("data-state"), "error", "ошибка даты не видна");
      assert.equal(panel.querySelectorAll(".search-result").length, 0, "показана выдача для невалидной даты");
      assert.throws(
        () => api(env).search.applyFilter({ date: "2026-02-31" }),
        (error) => /** @type {any} */ (error)?.code === "ACTION_FAILED",
        "applyFilter принял 2026-02-31",
      );
    } finally {
      env.dispose();
    }
  });
});

/* =============================================================================
 * M-13 (RW-40): HTTP 202 Indexing Retry/Progress, NSFW Gate, Filter Chips &
 * Pagination Ceiling (SEARCH-202-NSFW-SPEC.md §8, SEARCH-202-NSFW-RE.md).
 * ========================================================================== */
describe("M-13: HTTP 202 Indexing, NSFW Gating, Filter Chips & MAX_SEARCH_OFFSET=9975", () => {
  it("T-M13-202-01: HTTP 202 игнорирует тело, берёт задержку из Retry-After / retry_after_ms и рендерит прогресс попытки 1 из 5", async () => {
    /** @type {any[]} */
    const calls = [];
    let attempt = 0;
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      attempt += 1;
      if (attempt === 1) {
        return {
          status: 202,
          headers: { "Retry-After": "2" },
          body: { documents_indexed: 1420, retry_after: 99 },
          documents_indexed: 1420,
          retry_after: 99,
        };
      }
      return {
        messages: [serverMessage("9601", "найдено после индексации")],
        total_results: 1,
        doing_deep_historical_index: false,
      };
    });
    try {
      const panel = await searchRemote(env, "индекс");
      const banner = /** @type {ShimElement | null} */ (
        panel.querySelector(".search-indexing-banner")
      );
      assert.ok(banner, "нет .search-indexing-banner");
      assert.equal(banner.hasAttribute("hidden"), false, "баннер индексации скрыт при 202");
      assert.equal(banner.getAttribute("data-indexing-state"), "indexing");
      assert.equal(
        banner.querySelector(".search-indexing-badge")?.textContent,
        "Попытка 1 из 5",
      );
      const detail = banner.querySelector(".search-indexing-detail")?.textContent ?? "";
      assert.match(
        detail,
        /Нам нужно проиндексировать этот сервер перед поиском\. Дайте нам минуточку\./,
        "нет строки AXPbZr в .search-indexing-detail",
      );
      assert.match(detail, /2 с/, "задержка из заголовка Retry-After: 2 не отражена");
      assert.doesNotMatch(detail, /1420|99/, "тело 202 не было проигнорировано");

      const progress = /** @type {ShimElement | null} */ (
        banner.querySelector('.search-indexing-progress[role="progressbar"]')
      );
      assert.ok(progress, "нет progressbar в .search-indexing-banner");
      assert.equal(progress.getAttribute("aria-valuenow"), "1");
      assert.equal(progress.getAttribute("aria-valuetext"), "Попытка 1 из 5");

      const pollTimers = [...env.timers.values()].filter((t) => t.kind === "timeout" && t.delay === 2000);
      assert.equal(pollTimers.length, 1, "не запланирован таймер ретрая ровно на 2000 мс");

      env.flushTimers();
      await api(env).search.whenIdle();

      assert.equal(calls.length, 2, "после срабатывания таймера не ушёл повторный запрос");
      assert.equal(calls[1]?.params?.query?.attempts, 1, "повторный запрос не передал attempts=1");
      assert.equal(banner.hasAttribute("hidden"), true, "после 200 OK без deep_index баннер не скрылся");
      assert.equal(panel.querySelectorAll(".search-result").length, 1);
    } finally {
      env.dispose();
    }
  });

  it("T-M13-202-02: HTTP 202 при Retry-After '0', '0.25' или без заголовка (и при IPC retry_after_ms) использует контрактную задержку / fallback 5000 мс", async () => {
    const computeDelay = /** @type {any} */ (searchMod).computeIndexingRetryDelayMs;
    assert.equal(typeof computeDelay, "function", "computeIndexingRetryDelayMs не экспортирована из search.js");
    assert.equal(
      computeDelay({ status: 202, headers: { "Retry-After": "0" }, body: { retry_after: 2 } }),
      5000,
      "Retry-After: 0 не ушёл в дефолт 5000 мс",
    );
    assert.equal(
      computeDelay({ status: 202, headers: { "retry-after": "0.25" }, retry_after: 2 }),
      5000,
      "дробный Retry-After: 0.25 через parseInt должен давать 0 -> 5000 мс",
    );
    assert.equal(
      computeDelay({ status: 202, headers: {}, body: { retry_after: 2 }, retry_after: 2 }),
      5000,
      "при отсутствии заголовка Retry-After тело 202 не должно менять дефолт 5000 мс",
    );
    assert.equal(
      computeDelay({ indexing: true, retry_after_ms: 3000 }),
      3000,
      "Tauri IPC конверт retry_after_ms не прочитан",
    );

    const env = await bootWithRealRail((cmd) => {
      if (cmd !== "search_remote") return undefined;
      return {
        status: 202,
        headers: { "Retry-After": "0.25" },
        body: { retry_after: 2 },
        retry_after: 2,
      };
    });
    try {
      const panel = await searchRemote(env, "дефолт");
      const pollTimers = [...env.timers.values()].filter((t) => t.kind === "timeout" && t.delay === 5000);
      assert.equal(pollTimers.length, 1, "таймер ретрая не выставлен в дефолтные 5000 мс");
      const detail = panel.querySelector(".search-indexing-detail")?.textContent ?? "";
      assert.match(detail, /5 с/, "в деталях баннера не показан повтор через 5 с");
    } finally {
      env.dispose();
    }
  });

  it("T-M13-202-03: лимит 5 автоматических повторов (6 запросов 202 подряд) переводит баннер в exhausted с кнопкой повтора", async () => {
    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return { indexing: true, retry_after_ms: 1000 };
    });
    try {
      const panel = await searchRemote(env, "долгий индекс");
      for (let i = 1; i <= 5; i += 1) {
        assert.equal(calls.length, i, `шаг ${i}: неверное число вызовов`);
        const timeoutEntries = [...env.timers.values()].filter((t) => t.kind === "timeout" && t.delay === 1000);
        assert.equal(timeoutEntries.length, 1, `шаг ${i}: нет таймера ретрая`);
        env.flushTimers();
        await api(env).search.whenIdle();
      }
      // Всего выполнено 6 запросов (1 начальный + 5 автоматических повторов).
      assert.equal(calls.length, 6, "должно быть ровно 6 запросов (1 + 5 повторов)");
      const remainingPoll = [...env.timers.values()].filter((t) => t.kind === "timeout" && t.delay === 1000);
      assert.equal(remainingPoll.length, 0, "после 5 повторов не должен создаваться новый таймер");

      const banner = /** @type {ShimElement | null} */ (panel.querySelector(".search-indexing-banner"));
      assert.ok(banner, "нет .search-indexing-banner");
      assert.equal(banner.getAttribute("data-indexing-state"), "exhausted");
      const progress = banner.querySelector('.search-indexing-progress[role="progressbar"]');
      assert.equal(progress?.getAttribute("aria-valuetext"), "5 из 5 попыток");
      const actions = banner.querySelector(".search-indexing-actions");
      assert.ok(actions && !actions.hasAttribute("hidden"), "блок .search-indexing-actions должен быть видим");
      const retryBtn = /** @type {ShimElement | null} */ (banner.querySelector(".search-indexing-retry-btn"));
      assert.ok(retryBtn, "нет кнопки .search-indexing-retry-btn");
    } finally {
      env.dispose();
    }
  });

  it("T-M13-202-04: смена запроса и закрытие панели немедленно отменяют таймер поллинга 202", async () => {
    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      if (args.params?.content === "старый") {
        return { indexing: true, retry_after_ms: 4000 };
      }
      return {
        messages: [serverMessage("9604", "новый результат")],
        total_results: 1,
        doing_deep_historical_index: false,
      };
    });
    try {
      await searchRemote(env, "старый");
      assert.equal(
        [...env.timers.values()].filter((t) => t.kind === "timeout" && t.delay === 4000).length,
        1,
        "таймер для старого запроса не создан",
      );

      const panel = await searchRemote(env, "новый");
      assert.equal(
        [...env.timers.values()].filter((t) => t.kind === "timeout" && t.delay === 4000).length,
        0,
        "старый таймер поллинга 202 не очищен при смене запроса",
      );
      assert.equal(panel.querySelectorAll(".search-result").length, 1);
      assert.equal(panel.querySelector(".search-result")?.getAttribute("data-id"), "9604");
    } finally {
      env.dispose();
    }
  });

  it("T-M13-202-05: ответ 200 OK с doing_deep_historical_index=true и documents_indexed=8450 показывает результаты и баннер 4Y3O+O", async () => {
    const env = await bootWithRealRail((cmd) => {
      if (cmd !== "search_remote") return undefined;
      return {
        messages: [serverMessage("9605", "частичный архив")],
        total_results: 12,
        doing_deep_historical_index: true,
        documents_indexed: 8450,
      };
    });
    try {
      const panel = await searchRemote(env, "архив");
      assert.equal(panel.querySelectorAll(".search-result").length, 1, "карточки результатов не отрендерены");
      const banner = /** @type {ShimElement | null} */ (panel.querySelector(".search-indexing-banner"));
      assert.ok(banner, "нет .search-indexing-banner");
      assert.equal(banner.hasAttribute("hidden"), false, "баннер скрыт при doing_deep_historical_index=true");
      assert.equal(banner.getAttribute("data-indexing-state"), "historical");
      assert.equal(
        banner.querySelector(".search-indexing-detail")?.textContent,
        "Найдено 8450 результатов. Мы всё ещё индексируем старые сообщения.",
      );
      const progress = banner.querySelector('.search-indexing-progress[role="progressbar"]');
      assert.equal(progress?.getAttribute("aria-valuetext"), "Архивная индексация: 8450");
    } finally {
      env.dispose();
    }
  });

  it("T-M13-NSFW-01..03: didAgreeNsfwGuild, пропуск include_nsfw при !didAgree и баннер .search-nsfw-gate (prompt, Continue, Go Back)", async () => {
    const didAgreeNsfwGuild = /** @type {any} */ (searchMod).didAgreeNsfwGuild;
    const AGE_GATED_SPACES = /** @type {any} */ (searchMod).AGE_GATED_SPACES;
    assert.equal(AGE_GATED_SPACES, 1, "AGE_GATED_SPACES должна быть равна 1");
    assert.equal(typeof didAgreeNsfwGuild, "function", "didAgreeNsfwGuild не экспортирована");

    const guild = { id: GUILDS[0].id, nsfw_level: 0, age_gated_features: 1 };
    const adultUser = { id: "42", age_verification_status: 3, nsfwAllowed: true };
    /** @type {Record<string, boolean>} */
    const agreedStore = {};

    assert.equal(
      didAgreeNsfwGuild(guild, adultUser, agreedStore),
      false,
      "без согласия в сторе didAgreeNsfwGuild обязан возвращать false",
    );

    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return {
        messages: [
          {
            ...serverMessage("9701", "контент 18+"),
            channel_id: "777999",
            nsfw: true,
          },
        ],
        total_results: 1,
        doing_deep_historical_index: false,
      };
    });
    try {
      const search = /** @type {any} */ (api(env).search);
      assert.equal(typeof search.setNsfwContext, "function", "search.setNsfwContext не реализован");
      search.setNsfwContext({
        guild,
        user: adultUser,
        regionalFeatureConfig: { age_gated_features: 687 },
        channels: [{ id: "777999", name: "nsfw-lounge", nsfw: true }],
        agreedStore,
      });

      // 1. Общегильдийский поиск при !didAgree: ключ include_nsfw опускается полностью.
      await searchRemote(env, "общий");
      assert.equal(calls.length, 1);
      assert.equal(
        "include_nsfw" in (calls[0]?.params?.query ?? {}),
        false,
        "при !didAgree ключ include_nsfw не должен присутствовать в query",
      );

      // 2. Поиск в NSFW-канале 777999: открывается баннер .search-nsfw-gate в режиме prompt.
      filterControl(env, "in").value = "777999";
      filterControl(env, "in").emit("input");
      await api(env).search.whenIdle();

      const panel = /** @type {ShimElement} */ (env.document.querySelector(".search-overlay"));
      const gate = /** @type {ShimElement | null} */ (panel.querySelector(".search-nsfw-gate"));
      assert.ok(gate, "нет баннера .search-nsfw-gate");
      assert.equal(gate.hasAttribute("hidden"), false, "баннер .search-nsfw-gate скрыт для NSFW-канала");
      assert.equal(gate.getAttribute("data-nsfw-mode"), "prompt");
      assert.equal(
        gate.querySelector(".search-nsfw-title")?.textContent,
        "Канал с возрастным ограничением",
      );
      const btnContinue = /** @type {ShimElement | null} */ (
        gate.querySelector(".search-nsfw-btn-continue")
      );
      const btnBack = /** @type {ShimElement | null} */ (
        gate.querySelector(".search-nsfw-btn-back")
      );
      assert.ok(btnContinue && !btnContinue.hasAttribute("hidden"), "нет кнопки Продолжить");
      assert.ok(btnBack && !btnBack.hasAttribute("hidden"), "нет кнопки Вернуться");
      assert.equal(
        env.document.activeElement,
        btnContinue,
        "фокус не переведён на .search-nsfw-btn-continue",
      );
      assert.equal(calls.length, 1, "запрос в NSFW-канал ушёл до согласия пользователя");

      // 3. T-M13-NSFW-03: клик по «Вернуться» очищает фильтр in, скрывает баннер и возвращает фокус в input.
      btnBack.click();
      assert.equal(gate.hasAttribute("hidden"), true, "баннер не скрылся после «Вернуться»");
      assert.equal(filterControl(env, "in").value, "", "фильтр «В» не очищен после «Вернуться»");
      assert.equal(
        env.document.activeElement,
        panel.querySelector(".search-overlay-input"),
        "фокус не вернулся в .search-overlay-input после «Вернуться»",
      );

      // 4. T-M13-NSFW-02: снова выбираем NSFW-канал и нажимаем «Продолжить».
      filterControl(env, "in").value = "777999";
      filterControl(env, "in").emit("input");
      assert.equal(gate.hasAttribute("hidden"), false);
      btnContinue.click();
      await api(env).search.whenIdle();

      assert.equal(agreedStore[GUILDS[0].id], true, "согласие не сохранено в GuildNSFWAgreeStore");
      assert.equal(didAgreeNsfwGuild(guild, adultUser, agreedStore), true);
      assert.equal(gate.hasAttribute("hidden"), true, "баннер не скрылся после согласия");
      const lastCall = calls[calls.length - 1];
      assert.equal(
        lastCall?.params?.query?.include_nsfw,
        true,
        "после согласия в query должен передаваться include_nsfw: true",
      );
      const nsfwBadge = panel.querySelector(".search-result .search-result-nsfw-badge");
      assert.ok(nsfwBadge, "на результате из NSFW-канала нет бейджа .search-result-nsfw-badge");
      assert.equal(nsfwBadge.textContent, "18+");
    } finally {
      env.dispose();
    }
  });

  it("T-M13-NSFW-04: при AGE_GATED_SPACES=1 и age_verification_status=2 (teen) didAgree=false даже при сохранённом согласии, режим underage", async () => {
    const didAgreeNsfwGuild = /** @type {any} */ (searchMod).didAgreeNsfwGuild;
    const guild = { id: GUILDS[0].id, nsfw_level: 0, age_gated_features: 687 };
    const teenUser = { id: "42", age_verification_status: 2, nsfwAllowed: true };
    const agreedStore = { [GUILDS[0].id]: true };

    assert.equal(
      didAgreeNsfwGuild(guild, teenUser, agreedStore, { age_gated_features: 687 }),
      false,
      "не-взрослый при AGE_GATED_SPACES=1 не может пройти гейт даже с записью в сторе",
    );
    // §4.1: маска берётся только из regional_feature_config; без конфига — 0.
    assert.equal(
      didAgreeNsfwGuild(guild, teenUser, agreedStore),
      true,
      "без regional_feature_config маска age_gated_features = 0",
    );
    assert.equal(
      didAgreeNsfwGuild(guild, { id: "42", age_verification_status: 4 }, agreedStore),
      true,
      "INFERRED_ADULT (4) должен проходить гейт при наличии согласия",
    );

    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return { messages: [], total_results: 0, doing_deep_historical_index: false };
    });
    try {
      const search = /** @type {any} */ (api(env).search);
      search.setNsfwContext({
        guild,
        user: teenUser,
        regionalFeatureConfig: { age_gated_features: 687 },
        channels: [{ id: "777999", name: "nsfw-lounge", nsfw: true }],
        agreedStore,
      });

      await searchRemote(env, "проверка");
      assert.equal(calls.length, 1);
      assert.equal(
        "include_nsfw" in (calls[0]?.params?.query ?? {}),
        false,
        "при underage include_nsfw не должен отправляться вообще",
      );

      filterControl(env, "in").value = "777999";
      filterControl(env, "in").emit("input");
      const panel = /** @type {ShimElement} */ (env.document.querySelector(".search-overlay"));
      const gate = /** @type {ShimElement | null} */ (panel.querySelector(".search-nsfw-gate"));
      assert.ok(gate && !gate.hasAttribute("hidden"), "баннер .search-nsfw-gate не открылся");
      assert.equal(gate.getAttribute("data-nsfw-mode"), "underage");
      assert.equal(
        gate.querySelector(".search-nsfw-title")?.textContent,
        "К сожалению, вы ещё слишком молоды для просмотра этого канала с возрастным ограничением",
      );
      assert.equal(
        gate.querySelector(".search-nsfw-desc")?.textContent,
        "Для просмотра каналов с возрастными ограничениями в Discord вам должно быть не менее 18 лет. Ознакомьтесь с нашей полезной статьёй о подтверждении возраста.",
      );
      const btnContinue = gate.querySelector(".search-nsfw-btn-continue");
      const btnBack = gate.querySelector(".search-nsfw-btn-back");
      assert.equal(btnContinue?.hasAttribute("hidden"), true, "в режиме underage кнопка Продолжить должна быть скрыта");
      assert.equal(btnBack?.hasAttribute("hidden"), false, "кнопка Вернуться должна быть видна");
      assert.equal(env.document.activeElement, btnBack, "в режиме underage фокус должен быть на Вернуться");
    } finally {
      env.dispose();
    }
  });

  it("T-M13-CHIPS-01..03: объектная форма searchDateBounds({ date, before, after }) и синхронизация чипов фильтров", async () => {
    // 1. Обратная совместимость со строкой + объектная форма { date, before, after }.
    const dayBounds = searchDateBounds("2026-10-20");
    assert.ok(dayBounds, "строковый вызов searchDateBounds('2026-10-20') сломан");
    assert.equal(dayBounds.min_id, snowflakeAt("2026-10-20T00:00:00.000Z"));
    assert.equal(dayBounds.max_id, snowflakeAt("2026-10-20T23:59:59.999Z"));

    const rangeBounds = searchDateBounds(/** @type {any} */ ({
      before: "2026-03-01",
      after: "2026-01-01",
    }));
    assert.deepEqual(
      rangeBounds,
      {
        max_id: "1477455328051200000",
        min_id: "1456436831842205696",
      },
      "searchDateBounds({ before, after }) вернул неверные границы snowflake",
    );

    const built = buildSearchQuery(
      /** @type {any} */ ({
        from: "777001",
        in: "1001",
        has: "image",
        before: "2026-03-01",
        after: "2026-01-01",
        pinned: true,
      }),
      { content: "релиз" },
    );
    assert.equal(built.max_id, "1477455328051200000");
    assert.equal(built.min_id, "1456436831842205696");

    // 2. Разбор инлайн-токенов в .search-overlay-input, рендер 6 чипов и удаление/сброс.
    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return { messages: [], total_results: 0, doing_deep_historical_index: false };
    });
    try {
      const panel = await searchRemote(
        env,
        "from:777001 in:1001 has:image before:2026-03-01 after:2026-01-01 pinned:true релиз",
      );
      const chipsBar = /** @type {ShimElement | null} */ (panel.querySelector(".search-filter-chips"));
      assert.ok(chipsBar && !chipsBar.hasAttribute("hidden"), "полоса .search-filter-chips скрыта");
      const chips = panel.querySelectorAll(".search-chip");
      assert.equal(chips.length, 6, "должно отрендериться 6 чипов");

      const lastQuery = calls[calls.length - 1]?.params?.query;
      assert.equal(lastQuery?.content, "релиз");
      assert.deepEqual(lastQuery?.author_id, ["777001"]);
      assert.deepEqual(lastQuery?.channel_id, ["1001"]);
      assert.deepEqual(lastQuery?.has, ["image"]);
      assert.equal(lastQuery?.max_id, "1477455328051200000");
      assert.equal(lastQuery?.min_id, "1456436831842205696");
      assert.equal(lastQuery?.pinned, true);

      // Удаление одного чипа (has:image) и полный сброс (.search-chips-clear).
      const removeHas = /** @type {ShimElement | null} */ (
        panel.querySelector('.search-chip[data-chip-key="has"] .search-chip-remove')
      );
      assert.ok(removeHas, "нет кнопки удаления чипа has");
      removeHas.click();
      await api(env).search.whenIdle();
      assert.equal(
        "has" in (calls[calls.length - 1]?.params?.query ?? {}),
        false,
        "после удаления чипа has фильтр остался в query",
      );

      const clearBtn = /** @type {ShimElement | null} */ (panel.querySelector(".search-chips-clear"));
      assert.ok(clearBtn, "нет кнопки .search-chips-clear");
      clearBtn.click();
      await api(env).search.whenIdle();
      assert.equal(chipsBar.hasAttribute("hidden"), true, "после сброса полоса чипов не скрылась");
      assert.equal(calls[calls.length - 1]?.params?.query?.content, "релиз", "свободный текст потерян при сбросе чипов");
    } finally {
      env.dispose();
    }
  });

  it("T-M13-CHIPS-02: отрицающие токены (-has:file -authorType:bot) и русские алиасы (от:777001 имеет:ссылку)", async () => {
    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      return { messages: [], total_results: 0, doing_deep_historical_index: false };
    });
    try {
      const panel = await searchRemote(env, "-has:file -authorType:bot от:777001 имеет:ссылку");
      const negatedChips = panel.querySelectorAll('.search-chip[data-chip-negated="true"]');
      assert.equal(negatedChips.length, 2, "должно быть 2 чипа с отрицанием data-chip-negated=true");

      const lastQuery = calls[calls.length - 1]?.params?.query;
      assert.deepEqual(lastQuery?.author_id, ["777001"]);
      assert.deepEqual(lastQuery?.has, ["-file", "link"]);
      assert.deepEqual(lastQuery?.author_type, ["-bot"]);
    } finally {
      env.dispose();
    }
  });

  it("T-M13-PAGE-01: потолок пагинации MAX_SEARCH_OFFSET = 9975 / MAX_SEARCH_PAGES = 400 блокирует переход далее 400 страницы с предупреждением E+2azY", async () => {
    assert.equal(/** @type {any} */ (searchMod).MAX_SEARCH_OFFSET, 9975, "MAX_SEARCH_OFFSET != 9975");
    assert.equal(/** @type {any} */ (searchMod).MAX_SEARCH_PAGES, 400, "MAX_SEARCH_PAGES != 400");

    /** @type {any[]} */
    const calls = [];
    const env = await bootWithRealRail((cmd, args) => {
      if (cmd !== "search_remote") return undefined;
      calls.push(args);
      const pageMsgs = Array.from({ length: 25 }, (_, idx) =>
        serverMessage(String(98000 + (args.params?.offset ?? 0) + idx), `msg ${idx}`),
      );
      return {
        messages: pageMsgs,
        total_results: 15000,
        doing_deep_historical_index: false,
      };
    });
    try {
      const panel = await searchRemote(env, "пагинация");
      const search = /** @type {any} */ (api(env).search);
      assert.equal(typeof search.loadAtOffset, "function", "search.loadAtOffset не реализован");

      await search.loadAtOffset(9975);
      await api(env).search.whenIdle();

      const status = panel.querySelector(".search-status")?.textContent ?? "";
      assert.match(
        status,
        /Мы можем предоставить не более 400 стр\. результатов\. Попробуйте сузить поиск\./,
        "нет предупреждения E+2azY о потолке в 400 страниц",
      );
      assert.equal(
        panel.querySelector(".search-more"),
        null,
        "кнопка .search-more не должна рендериться после достижения MAX_SEARCH_OFFSET=9975",
      );

      const callsBefore = calls.length;
      await search.loadAtOffset(10000);
      assert.equal(calls.length, callsBefore, "запрос с offset > 9975 не был заблокирован");
    } finally {
      env.dispose();
    }
  });
});
