// @ts-check
/**
 * Browser-fixture harness для семи сценариев M2 (QA-M2-02).
 *
 * Что поднимается: настоящий `rust/gui-spike/ui/index.html` из локального
 * HTTP-сервера (ES-модули не грузятся с `file://`), headless Chromium с
 * `--remote-debugging-port=0` и CDP-сессия из `src/core/cdp/client.mjs`.
 * В HTML перед `app.js` инжектится fake `__TAURI__` (`tests/helpers/fake-tauri.mjs`);
 * сети, live Discord и Tauri-процесса нет.
 *
 * Инварианты (см. `QUALITY/QA-GATE-SKIPS.md`, `QA-E2E-PLAN` §3.4):
 * - нет браузера → `InfraError` (`code: INFRA_NO_BROWSER`), **не** `t.skip`:
 *   инфраструктурная дыра обязана красить прогон, а не давать зелёный pass;
 * - ожидания — только предикаты на DOM/журнале IPC, без `sleep`-магии;
 * - никаких скриншотов вместо оракула: проверяются значения из `fixtures/gateway/*`;
 * - после `Page.reload` fake-транспорт ставится заново (он в HTML), а настройки
 *   переживают reload в `localStorage`.
 *
 * Запуск: `CHROME_BIN=/usr/bin/chromium node --test --test-concurrency=1 tests/e2e/m2-*.e2e.test.mjs`.
 */
import { createServer } from "node:http";
import { accessSync, constants, existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { delimiter, extname, join, normalize, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { evaluate, listTargets, openPage } from "../../src/core/cdp/client.mjs";
import { launchBrowser } from "./browser.mjs";
import { bootstrapSource } from "./fake-tauri.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const UI_ROOT = join(ROOT, "rust", "gui-spike", "ui");
const GATEWAY_FIXTURES = join(ROOT, "fixtures", "gateway", "by-type");
const M2_DATA_PATH = join(ROOT, "tests", "helpers", "m2-fixtures.json");
const SCENARIOS_PATH = join(ROOT, "tests", "helpers", "m2-scenarios.json");

/** Имена, под которыми ищется браузер в `PATH` (как в `tests/helpers/browser.mjs`). */
const BROWSER_NAMES = [
  "chromium",
  "chromium-browser",
  "google-chrome",
  "google-chrome-stable",
  "brave-browser",
  "brave",
  "microsoft-edge",
  "microsoft-edge-stable",
];

/** Таймаут ожидания предиката по умолчанию: CI-браузер стартует медленно. */
const DEFAULT_TIMEOUT_MS = 20000;

/** Сколько ждём появления CDP-target после запуска браузера. */
const TARGET_TIMEOUT_MS = 25000;

/**
 * Инфраструктурная ошибка fixture-набора: браузера нет/он не запускается.
 * Это fail, а не skip: `t.skip` спрятал бы непроверенный сценарий.
 */
export class InfraError extends Error {
  /** @param {string} message */
  constructor(message) {
    super(message);
    this.name = "InfraError";
    this.code = "INFRA_NO_BROWSER";
    this.infra = true;
  }
}

/* -------------------------------------------------------------------------- */
/*  Оракулы и данные сценариев (без сети и без чтения продуктового кода)       */
/* -------------------------------------------------------------------------- */

/**
 * @typedef {object} M2Scenario
 * @property {string} id
 * @property {string} file
 * @property {string} title
 * @property {string} oracle
 * @property {string[]} ipc
 */

/**
 * Реестр семи сценариев M2 — единственный источник для тестов и мета-инварианта.
 * @type {M2Scenario[]}
 */
export const M2_SCENARIOS = /** @type {M2Scenario[]} */ (
  JSON.parse(readFileSync(SCENARIOS_PATH, "utf8")).scenarios
);

/**
 * Детерминированные данные транспорта (ошибки, invite, settings, net, voice).
 * Секретов нет: токен в сценарии — заведомо фейковая строка с пометкой.
 * @returns {any}
 */
export function m2FixtureData() {
  return structuredClone(M2_DATA);
}

/** READY fixture — независимый оракул гильдий/каналов (`fixtures/gateway/by-type`). */
const READY = JSON.parse(readFileSync(join(GATEWAY_FIXTURES, "002-READY.json"), "utf8"));

/** Данные сценариев читаются один раз: они неизменяемы по контракту. */
const M2_DATA = JSON.parse(readFileSync(M2_DATA_PATH, "utf8"));

/**
 * Гильдии READY в порядке рейла: id — snowflake, имя — `properties.name`.
 * @returns {Array<{ id: string, name: string }>}
 */
export function m2Guilds() {
  return /** @type {any[]} */ (READY?.d?.guilds ?? []).map((guild) => ({
    id: String(guild.id),
    name: String(guild?.properties?.name ?? ""),
  }));
}

/** Реальные идентификаторы из READY/MESSAGE_CREATE (не «news»/индекс). */
export const M2_IDS = Object.freeze({
  /** Гильдия, у которой в фикстурах есть сообщения. */
  guildWithMessages: String(M2_DATA.ids.guildWithMessages),
  /** Канал с тремя MESSAGE_CREATE (023/024/025). */
  channelWithMessages: String(M2_DATA.ids.channelWithMessages),
  /** Гильдия из fake-ответа `accept_invite` (приходит только из preview). */
  invitedGuild: String(M2_DATA.ids.invitedGuild),
});

/**
 * `channels({guildId})` в форме `TreeChannel`: порядок — `position`, затем `id`
 * (как в backend-проекции `data.rs`), независимо от порядка в READY.
 * @param {string} guildId
 * @returns {Array<{ id: string, guildId: string, name: string, type: number, parentId: string | null, position: number }>}
 */
export function m2Channels(guildId) {
  const guild = /** @type {any[]} */ (READY?.d?.guilds ?? []).find(
    (item) => String(item.id) === guildId,
  );
  if (!guild) return [];
  return /** @type {any[]} */ (guild.channels ?? [])
    .map((channel) => ({
      id: String(channel.id),
      guildId,
      name: typeof channel.name === "string" ? channel.name : "",
      type: Number(channel.type),
      parentId: channel.parent_id === "nil" || channel.parent_id === undefined
        ? null
        : String(channel.parent_id),
      position: Number(channel.position ?? 0),
    }))
    .sort((left, right) => {
      if (left.position !== right.position) return left.position - right.position;
      return left.id < right.id ? -1 : left.id > right.id ? 1 : 0;
    });
}

/** @typedef {{ id: string, channelId: string, author: string, content: string, timestamp: string }} M2Message */

/**
 * Сообщения канала из кадров `*-MESSAGE_CREATE.json` — независимый оракул
 * `messages_page`: порядок `timestamp`, затем `id`, без дублей.
 * @param {string} channelId
 * @returns {M2Message[]}
 */
export function m2Messages(channelId) {
  /** @type {Map<string, M2Message>} */
  const byId = new Map();
  for (const name of readdirSync(GATEWAY_FIXTURES)) {
    if (!name.endsWith(".json")) continue;
    let frame;
    try {
      frame = JSON.parse(readFileSync(join(GATEWAY_FIXTURES, name), "utf8"));
    } catch {
      continue;
    }
    if (frame?.t !== "MESSAGE_CREATE") continue;
    const data = frame?.d ?? {};
    if (String(data.channel_id ?? "") !== channelId) continue;
    const id = String(data.id ?? "");
    if (id === "") continue;
    byId.set(id, {
      id,
      channelId,
      author: String(data.author?.username ?? ""),
      content: String(data.content ?? ""),
      timestamp: String(data.timestamp ?? ""),
    });
  }
  return [...byId.values()].sort((left, right) => {
    if (left.timestamp !== right.timestamp) return left.timestamp < right.timestamp ? -1 : 1;
    return left.id < right.id ? -1 : left.id > right.id ? 1 : 0;
  });
}

/**
 * `messages_page` в форме `MessagesPage` (camelCase, как отдаёт `data.rs`).
 * @param {string} channelId
 * @returns {any}
 */
export function m2MessagesPage(channelId) {
  const rows = m2Messages(channelId);
  /** @type {string | null} */
  let previousAuthor = null;
  /** @type {string | null} */
  let previousDay = null;
  const items = rows.map((row) => {
    const day = row.timestamp.slice(0, 10);
    const group = previousAuthor === row.author && previousDay === day ? "continuation" : "start";
    previousAuthor = row.author;
    previousDay = day;
    return {
      id: row.id,
      channelId,
      guildId: null,
      author: {
        id: row.author,
        username: row.author,
        globalName: null,
        avatar: null,
        bot: /bot|monitorss|probot/i.test(row.author),
      },
      content: row.content,
      timestamp: row.timestamp,
      edited: false,
      editedAt: null,
      day,
      group,
      pinned: false,
      messageType: 0,
      embeds: [],
      attachments: [],
      reactions: [],
      replyTo: null,
      mentions: [],
      mentionEveryone: false,
    };
  });
  return {
    channelId,
    items,
    nextCursor: null,
    hasMore: false,
    total: items.length,
    limit: 100,
    loadedRowsLimit: 500,
  };
}

/**
 * `ready()` в форме `ReadyPayload` (`data.rs`): гильдии/каналы — из READY.
 * @returns {any}
 */
export function m2ReadyPayload() {
  const data = m2FixtureData();
  const guilds = m2Guilds();
  return {
    user: {
      id: String(data.user.id),
      username: String(data.user.username),
      globalName: data.user.global_name ?? data.user.username,
      avatar: null,
      email: null,
      bio: null,
      premium: false,
      mfaEnabled: false,
    },
    guilds: guilds.map((guild) => ({
      id: guild.id,
      name: guild.name,
      icon: null,
      memberCount: 3,
      channelCount: m2Channels(guild.id).length,
    })),
    guildCount: guilds.length,
    privateChannelCount: 0,
    relationshipCount: 0,
    readStateCount: 0,
    session: null,
  };
}

/**
 * Базовая спека транспорта: вход пройден, READY/каналы/история — из оракула,
 * настройки — дефолты `UiSettings`, сеть — ready. Сценарии переопределяют
 * одну-две команды, а не собирают всё заново.
 * @param {Record<string, any>} [overrides]
 * @returns {Record<string, any>}
 */
export function m2Commands(overrides = {}) {
  const data = m2FixtureData();
  /** @type {Record<string, any>} */
  const channelsByGuild = {};
  for (const guild of m2Guilds()) channelsByGuild[guild.id] = { ok: m2Channels(guild.id) };
  return {
    auth_status: { ok: data.auth.ready },
    ready: { ok: m2ReadyPayload() },
    channels: {
      byArg: { key: "guildId", map: channelsByGuild, fallback: { ok: [] } },
    },
    messages_page: {
      byArg: {
        key: "channelId",
        map: { [M2_IDS.channelWithMessages]: { ok: m2MessagesPage(M2_IDS.channelWithMessages) } },
        fallback: {
          ok: {
            channelId: "",
            items: [],
            nextCursor: null,
            hasMore: false,
            total: 0,
            limit: 100,
            loadedRowsLimit: 500,
          },
        },
      },
    },
    // settings_load/settings_save НЕ переопределяются: дефолт fake-транспорта
    // читает/пишет localStorage, чтобы сценарий 5 проверял persist после reload.
    net_status: { ok: "ready" },
    ...overrides,
  };
}
/* -------------------------------------------------------------------------- */
/*  Браузер: детекция (env-aware) и infra-fail                                  */
/* -------------------------------------------------------------------------- */

/** @param {string} path @returns {boolean} */
function isExecutable(path) {
  try {
    accessSync(path, constants.X_OK);
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

/**
 * Браузер по `CHROME_BIN`/`CHROME_PATH` или первое известное имя в `PATH`.
 * Учитывает переданное окружение (для мета-теста с пустым `PATH`), не подмешивая
 * `process.env`. Заданный, но неисполняемый `CHROME_BIN` — это `null` (infra fail),
 * а не повод искать другой браузер: окружение CI должно падать явно.
 * @param {Record<string, string | undefined>} [env]
 * @returns {string | null}
 */
export function detectBrowser(env = process.env) {
  const explicit = String(env.CHROME_BIN ?? env.CHROME_PATH ?? "").trim();
  if (explicit !== "") return isExecutable(explicit) ? explicit : null;
  const pathValue = String(env.PATH ?? "");
  for (const name of BROWSER_NAMES) {
    for (const dir of pathValue.split(delimiter)) {
      if (!dir) continue;
      const candidate = join(dir, name);
      if (isExecutable(candidate)) return candidate;
    }
  }
  return null;
}

/**
 * Браузер или `InfraError`. Сценарии вызывают только это: skip-пути нет.
 * @param {Record<string, string | undefined>} [env]
 * @returns {string}
 */
export function requireBrowser(env = process.env) {
  const bin = detectBrowser(env);
  if (bin !== null) return bin;
  const explicit = String(env.CHROME_BIN ?? env.CHROME_PATH ?? "").trim();
  throw new InfraError(
    explicit === ""
      ? "INFRA: браузер не найден (CHROME_BIN/CHROME_PATH или chromium/chrome/brave в PATH). "
        + "M2 fixture-сценарий не выполнен — это infra fail, а не pass; поставьте Chromium "
        + "или задайте CHROME_BIN."
      : `INFRA: ${explicit} не исполняется (CHROME_BIN/CHROME_PATH). M2 fixture-сценарий не выполнен — infra fail.`,
  );
}

/* -------------------------------------------------------------------------- */
/*  Локальный статический сервер ui/ с инъекцией fake __TAURI__                 */
/* -------------------------------------------------------------------------- */

/** @type {Record<string, string>} */
const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".woff2": "font/woff2",
  ".ico": "image/x-icon",
};

/** Маркер module-скрипта в `ui/index.html`: перед ним ставится fake-транспорт. */
const APP_SCRIPT_MARK = '<script type="module" src="app.js"></script>';

/**
 * Вставляет bootstrap в HTML **до** module-скрипта. Если маркер пропал из
 * разметки — это ошибка harness, а не «молча без fake»: продукт тогда пошёл бы
 * в настоящий Tauri и сценарий проверял бы не то.
 * @param {string} html
 * @param {string} script
 * @returns {string}
 */
export function injectBootstrap(html, script) {
  if (!html.includes(APP_SCRIPT_MARK)) {
    throw new Error(
      `ui/index.html: нет ${APP_SCRIPT_MARK} — инъекция fake __TAURI__ невозможна`,
    );
  }
  const tag = `<script data-qa="fake-tauri">${script}</script>\n    `;
  // Функция-заменитель: `$` в спеке не должен трактоваться как группа замены.
  return html.replace(APP_SCRIPT_MARK, () => `${tag}${APP_SCRIPT_MARK}`);
}

/**
 * Статический сервер каталога `ui/` (только чтение, только внутри root).
 * @param {{ root?: string, bootstrap: string | null }} options null serves static HTML unchanged
 * @returns {Promise<{ url: string, port: number, close: () => Promise<void> }>}
 */
export async function startUiServer(options) {
  const root = resolve(options.root ?? UI_ROOT);
  const bootstrap = options.bootstrap;
  const server = createServer((request, response) => {
    void (async () => {
      try {
        const url = new URL(request.url ?? "/", "http://127.0.0.1");
        const pathname = decodeURIComponent(url.pathname);
        const relativePath = pathname === "/" ? "index.html" : pathname.replace(/^\/+/, "");
        const target = resolve(join(root, normalize(relativePath)));
        if (target !== root && !target.startsWith(`${root}${sep}`)) {
          response.writeHead(403, { "content-type": "text/plain; charset=utf-8" });
          response.end("forbidden");
          return;
        }
        const bytes = await readFile(target);
        const type = MIME[extname(target).toLowerCase()] ?? "application/octet-stream";
        const body = target.endsWith(".html") && bootstrap !== null
          ? Buffer.from(injectBootstrap(bytes.toString("utf8"), bootstrap), "utf8")
          : bytes;
        response.writeHead(200, {
          "content-type": type,
          "cache-control": "no-store",
          "content-length": String(body.byteLength),
        });
        response.end(body);
      } catch {
        response.writeHead(404, { "content-type": "text/plain; charset=utf-8" });
        response.end("not found");
      }
    })();
  });
  await new Promise((done, fail) => {
    server.once("error", fail);
    server.listen(0, "127.0.0.1", () => done(undefined));
  });
  const address = server.address();
  if (address === null || typeof address === "string") {
    server.close();
    throw new Error("ui-fixture: сервер не отдал TCP-адрес");
  }
  return {
    url: `http://127.0.0.1:${address.port}/index.html`,
    port: address.port,
    close: () => new Promise((done) => {
      server.close(() => done(undefined));
      // Keep-alive сокеты Chromium не должны держать teardown.
      if (typeof server.closeAllConnections === "function") server.closeAllConnections();
    }),
  };
}

/* -------------------------------------------------------------------------- */
/*  CDP-хэндл страницы                                                         */
/* -------------------------------------------------------------------------- */

/** @typedef {import("../../src/core/types.js").CdpEventClient} CdpClient */

/** @param {number} ms @returns {Promise<void>} */
const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

/**
 * Ждёт появления page-target, чей URL содержит `hint`.
 * @param {string} endpoint
 * @param {string} hint
 * @returns {Promise<void>}
 */
async function waitForTarget(endpoint, hint) {
  const deadline = Date.now() + TARGET_TIMEOUT_MS;
  for (;;) {
    try {
      const targets = await listTargets(endpoint, undefined, { timeoutMs: 2000 });
      if (targets.some((target) => target.type === "page" && String(target.url ?? "").includes(hint))) {
        return;
      }
    } catch {
      // браузер ещё не поднял HTTP-endpoint — ждём
    }
    if (Date.now() > deadline) {
      throw new Error(`CDP: страница с «${hint}» не появилась за ${TARGET_TIMEOUT_MS} мс`);
    }
    await sleep(100);
  }
}

/** Хэндл одной открытой fixture-страницы. */
export class UiFixture {
  /**
   * @param {{ cdp: CdpClient, url: string, server: { close: () => Promise<void> }, browser: { child: import("node:child_process").ChildProcess } }} options
   */
  constructor(options) {
    this.cdp = options.cdp;
    this.url = options.url;
    this.server = options.server;
    this.browser = options.browser;
    this.closed = false;
    /** С какого события собираются необработанные исключения страницы. */
    this.bootMark = this.mark();
    this.expectedInfra = false;
  }

  /**
   * `Runtime.evaluate` с `awaitPromise` (см. `src/core/cdp/client.mjs`).
   * @param {string} expression
   * @returns {Promise<any>}
   */
  eval(expression) {
    return evaluate(this.cdp, expression);
  }

  /**
   * Предикат в странице: возвращает значение, когда оно truthy.
   * @param {string} expression
   * @param {{ timeoutMs?: number, label?: string }} [options]
   * @returns {Promise<any>}
   */
  async waitFor(expression, options = {}) {
    const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    const label = options.label ?? expression;
    const deadline = Date.now() + timeoutMs;
    /** @type {any} */
    let last = null;
    /** Последняя ошибка вычисления (например, execution context destroyed при reload). */
    let lastError = "";
    for (;;) {
      try {
        last = await this.eval(expression);
        lastError = "";
      } catch (error) {
        // Ошибка вычисления — не «условие выполнено»: контекст может быть уничтожен
        // навигацией, поэтому продолжаем ждать, а не возвращаем truthy-объект.
        last = null;
        lastError = error instanceof Error ? error.message : String(error);
      }
      if (last) return last;
      if (Date.now() > deadline) {
        throw new Error(
          `waitFor(${label}): не выполнено за ${timeoutMs} мс; последнее значение: `
            + `${JSON.stringify(last)}; ошибка: ${lastError || "нет"}; ${await this.diagnostics()}`,
        );
      }
      await sleep(50);
    }
  }

  /**
   * Состояние для сообщения об ошибке: страница, журнал IPC, console.error.
   * @returns {Promise<string>}
   */
  async diagnostics() {
    try {
      const state = await this.eval(
        "JSON.stringify({ ready: document.readyState, url: location.href,"
          + " title: document.title,"
          + " calls: (window.__QA__ ? window.__QA__.calls.slice(-12) : []),"
          + " consoleErrors: (window.__QA__ ? window.__QA__.consoleErrors.slice(-5) : []),"
          + " gate: (document.querySelector('#auth-gate') || {}).getAttribute"
          + "   ? document.querySelector('#auth-gate').getAttribute('data-state') : null,"
          + " bodyText: (document.body ? document.body.innerText.slice(0, 400) : ''),"
          + " })",
      );
      return `диагностика: ${state}`;
    } catch (error) {
      return `диагностика недоступна: ${error instanceof Error ? error.message : String(error)}`;
    }
  }

  /** @param {string} selector @returns {Promise<string | null>} */
  async text(selector) {
    return this.eval(
      `(() => { const node = document.querySelector(${JSON.stringify(selector)});`
        + " return node ? node.textContent : null; })()",
    );
  }

  /**
   * @param {string} selector
   * @param {string} name
   * @returns {Promise<string | null>}
   */
  async attr(selector, name) {
    return this.eval(
      `(() => { const node = document.querySelector(${JSON.stringify(selector)});`
        + ` return node ? node.getAttribute(${JSON.stringify(name)}) : null; })()`,
    );
  }

  /** @param {string} selector @returns {Promise<boolean>} */
  async has(selector) {
    return Boolean(await this.eval(
      `(() => { return document.querySelector(${JSON.stringify(selector)}) !== null; })()`,
    ));
  }

  /** @param {string} selector @returns {Promise<number>} */
  async count(selector) {
    return Number(await this.eval(
      `(() => { return document.querySelectorAll(${JSON.stringify(selector)}).length; })()`,
    ));
  }

  /**
   * Видимость «как в браузере»: не `[hidden]`/`display:none` и есть layout-бокс.
   * @param {string} selector
   * @returns {Promise<boolean>}
   */
  async visible(selector) {
    return Boolean(await this.eval(
      `(() => { const node = document.querySelector(${JSON.stringify(selector)});`
        + " if (!node) return false;"
        + " const style = getComputedStyle(node);"
        + ' return style.display !== "none" && style.visibility !== "hidden"'
        + " && node.getClientRects().length > 0; })()",
    ));
  }

  /**
   * Программный клик (без hit-testing): проверяется DOM-контракт и обработчики,
   * а не пиксели. Нет элемента — ошибка, чтобы опечатка в селекторе не «прошла».
   * @param {string} selector
   * @returns {Promise<void>}
   */
  async click(selector) {
    const clicked = await this.eval(
      `(() => { const node = document.querySelector(${JSON.stringify(selector)});`
        + " if (!node) return false; node.click(); return true; })()",
    );
    if (clicked !== true) throw new Error(`click(${selector}): элемент не найден`);
  }

  /**
   * Клик по N-му элементу селектора (индекс 0-based): нужно там, где в разметке
   * несколько однотипных кнопок (плотность/тема, сортировки).
   * @param {string} selector
   * @param {number} index
   * @returns {Promise<void>}
   */
  async clickNth(selector, index) {
    const clicked = await this.eval(
      `(() => { const node = document.querySelectorAll(${JSON.stringify(selector)})[${Number(index)}];`
        + " if (!node) return false; node.click(); return true; })()",
    );
    if (clicked !== true) throw new Error(`clickNth(${selector}, ${index}): элемент не найден`);
  }

  /**
   * Ввод: `value` + `input` (как пользовательский набор для обработчиков продукта).
   * @param {string} selector
   * @param {string} value
   * @returns {Promise<void>}
   */
  async type(selector, value) {
    const done = await this.eval(
      `(() => { const node = document.querySelector(${JSON.stringify(selector)});`
        + " if (!node) return false;"
        + " if (typeof node.focus === 'function') node.focus();"
        + ` node.value = ${JSON.stringify(value)};`
        + ' node.dispatchEvent(new Event("input", { bubbles: true }));'
        + " return true; })()",
    );
    if (done !== true) throw new Error(`type(${selector}): элемент не найден`);
  }

  /**
   * Клавиша на элементе (`KeyboardEvent` не всплывает автоматически — слушатели
   * продукта прямые, поэтому этого достаточно).
   * @param {string} selector
   * @param {string} key
   * @returns {Promise<void>}
   */
  async press(selector, key = "Enter") {
    const done = await this.eval(
      `(() => { const node = document.querySelector(${JSON.stringify(selector)});`
        + " if (!node) return false;"
        + ` node.dispatchEvent(new KeyboardEvent("keydown", { key: ${JSON.stringify(key)},`
        + ' bubbles: true, cancelable: true }));'
        + " return true; })()",
    );
    if (done !== true) throw new Error(`press(${selector}): элемент не найден`);
  }

  /**
   * Наведение (hover-панель сообщения слушает `mouseover` на списке).
   * @param {string} selector
   * @returns {Promise<void>}
   */
  async hover(selector) {
    const done = await this.eval(
      `(() => { const node = document.querySelector(${JSON.stringify(selector)});`
        + " if (!node) return false;"
        + ' node.dispatchEvent(new MouseEvent("mouseover", { bubbles: true }));'
        + " return true; })()",
    );
    if (done !== true) throw new Error(`hover(${selector}): элемент не найден`);
  }

  /** @param {string} cmd @returns {Promise<Array<{ cmd: string, args: any, at: number }>>} */
  async calls(cmd) {
    return this.eval(`window.__QA__.callsOf(${JSON.stringify(cmd)})`);
  }

  /** @param {string} cmd @returns {Promise<number>} */
  async callCount(cmd) {
    return Number(await this.eval(`window.__QA__.count(${JSON.stringify(cmd)})`));
  }

  /**
   * Подменяет ответ команды «на ходу» (сценарии с последовательностью состояний).
   * @param {string} cmd
   * @param {any} entry
   * @returns {Promise<void>}
   */
  async setCommand(cmd, entry) {
    await this.eval(`window.__QA__.set(${JSON.stringify(cmd)}, ${JSON.stringify(entry)})`);
  }

  /**
   * Ждёт вызов команды (N-й и/или первый, удовлетворяющий `where`).
   * @param {string} cmd
   * @param {{
   *   min?: number,
   *   timeoutMs?: number,
   *   where?: (args: any, call: { cmd: string, args: any, at: number }) => boolean,
   *   check?: (args: any, call: { cmd: string, args: any, at: number }) => void,
   * }} [options]
   * @returns {Promise<{ cmd: string, args: any, at: number }>}
   */
  async waitForCall(cmd, options = {}) {
    const min = options.min ?? 1;
    const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const calls = await this.calls(cmd);
      const pool = calls.slice(Math.max(0, min - 1));
      const call = options.where ? pool.find((item) => options.where?.(item.args, item)) : pool[0];
      if (call) {
        options.check?.(call.args, call);
        return call;
      }
      if (Date.now() > deadline) {
        throw new Error(
          `waitForCall(${cmd}): подходящего вызова нет за ${timeoutMs} мс; `
            + `всего ${calls.length}; ${await this.diagnostics()}`,
        );
      }
      await sleep(50);
    }
  }

  /**
   * Позиция в журнале CDP-событий: клиент возвращает `mark()`, но в публичном
   * typedef он не описан, поэтому доступ идёт через `any` (контракт клиента —
   * `src/core/cdp/client.mjs`).
   * @returns {number}
   */
  mark() {
    return Number(/** @type {any} */ (this.cdp).mark());
  }

  /** Необработанные исключения страницы после старта (boot / reload). */
  pageExceptions() {
    /** @type {any[]} */
    const events = /** @type {any} */ (this.cdp).eventsSince(this.bootMark);
    return events.filter((event) => event.method === "Runtime.exceptionThrown");
  }

  /** `console.error` страницы, собранный fake-транспортом. */
  async consoleErrors() {
    return /** @type {string[]} */ (await this.eval("window.__QA__.consoleErrors"));
  }

  /** Ждёт завершения автозамера/снапшота: последний boot-вызов `report_ui_snapshot`. */
  async waitForBoot() {
    await this.waitFor(
      "Boolean(window.__QA__ && window.__QA__.count('report_ui_snapshot') > 0)",
      { label: "boot: report_ui_snapshot" },
    );
  }

  /**
   * Перезагрузка страницы (проверка persist настроек). Fake-транспорт и
   * `localStorage` живут в документе/оригине, поэтому ставятся заново.
   * @returns {Promise<void>}
   */
  async reload() {
    this.bootMark = this.mark();
    await this.cdp.send("Page.reload", { ignoreCache: true });
    await sleep(150);
    await this.waitForBoot();
    this.assertBootClean();
  }

  /** Падение страницы при boot — ошибка сценария, а не «нет элемента». */
  assertBootClean() {
    const exceptions = this.pageExceptions();
    if (exceptions.length === 0) return;
    const details = exceptions
      .map((event) => {
        const params = /** @type {any} */ (event.params);
        return params?.exceptionDetails?.exception?.description
          ?? params?.exceptionDetails?.text
          ?? "unknown exception";
      })
      .join(" | ");
    throw new Error(`страница упала при загрузке (Runtime.exceptionThrown): ${details}`);
  }

  /** Закрывает CDP, браузер и сервер; идемпотентно. */
  async close() {
    if (this.closed) return;
    this.closed = true;
    try {
      this.cdp.close();
    } catch {
      // соединение уже закрыто
    }
    try {
      await stopFixtureBrowser(this.browser);
    } finally {
      await this.server.close();
    }
  }
}

/** @param {{ child: import("node:child_process").ChildProcess }} browser */
async function stopFixtureBrowser(browser) {
  const child = browser.child;
  if (child.exitCode !== null || child.signalCode !== null) return;
  await new Promise((done, fail) => {
    const exited = () => { clearTimeout(timer); done(undefined); };
    const stop = (/** @type {NodeJS.Signals} */ signal) => {
      try { child.kill(signal); } catch (error) {
        clearTimeout(timer);
        child.removeListener("exit", exited);
        fail(error);
      }
    };
    const timer = setTimeout(() => stop("SIGKILL"), 3000);
    child.once("exit", exited);
    stop("SIGTERM");
  });
}

/**
 * Keep temporary-profile removal last, including partial-startup/error paths.
 * @param {{ opened: { close: () => Promise<void> } | null,
 *   browser: { child: import("node:child_process").ChildProcess } | null,
 *   server: { close: () => Promise<void> }, dir: string | null }} resources
 */
export async function cleanupUiFixture({ opened, browser, server, dir }) {
  try {
    if (opened) await opened.close();
  } finally {
    try {
      if (browser) await stopFixtureBrowser(browser);
    } finally {
      try { await server.close(); } finally {
        // Chromium can finish a queued profile write just after its main process exits.
        if (dir) await rm(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
      }
    }
  }
}

/**
 * Поднимает fixture-страницу: сервер `ui/` + fake `__TAURI__`, headless Chromium,
 * CDP, ожидание boot. Без браузера — `InfraError` (не skip).
 * @param {import("node:test").TestContext} t
 * @param {{ commands?: Record<string, any>, env?: Record<string, string | undefined>, timeoutMs?: number }} [options]
 * @returns {Promise<UiFixture>}
 */
export async function openUiFixture(t, options = {}) {
  const env = options.env ?? process.env;
  const bin = requireBrowser(env);
  const server = await startUiServer({ bootstrap: bootstrapSource(options.commands ?? {}) });
  /** @type {string | null} */
  let dir = null;
  /** @type {{ child: import("node:child_process").ChildProcess, endpoint: string } | null} */
  let browser = null;
  /** @type {UiFixture | null} */
  let opened = null;
  t.after(() => cleanupUiFixture({ opened, browser, server, dir }));
  dir = await mkdtemp(join(tmpdir(), "discord-lab-m2-"));

  browser = await launchBrowser({
    bin,
    url: "about:blank",
    userDataDir: join(dir, "profile"),
  });
  const endpoint = browser.endpoint;
  await waitForTarget(endpoint, "about:blank");
  const page = await openPage({ endpoint, hint: "about:blank" });
  // Runtime.enable — чтобы поймать необработанные исключения boot, а не только
  // «элемент не найден» далеко после старта.
  await page.cdp.send("Runtime.enable");
  await page.cdp.send("Page.enable");

  const fixture = new UiFixture({ cdp: page.cdp, url: server.url, server, browser });
  opened = fixture;
  fixture.bootMark = fixture.mark();
  await page.cdp.send("Page.navigate", { url: server.url });
  await sleep(100);
  await fixture.waitForBoot();
  fixture.assertBootClean();
  return fixture;
}

/**
 * Хэндл с уже пройденным входом (`auth_status → ready`) — базовый вход для
 * M2-02…M2-07. M2-01 (сам вход) вызывает `openUiFixture` напрямую.
 * @param {import("node:test").TestContext} t
 * @param {{ overrides?: Record<string, any>, timeoutMs?: number }} [options]
 * @returns {Promise<UiFixture>}
 */
export async function openApp(t, options = {}) {
  const data = m2FixtureData();
  const fixture = await openUiFixture(t, {
    commands: m2Commands({
      auth_status: { ok: data.auth.ready },
      ...(options.overrides ?? {}),
    }),
    timeoutMs: options.timeoutMs,
  });
  await fixture.waitFor(
    `(() => { const gate = document.querySelector('#auth-gate');`
      + ' return gate !== null && gate.getAttribute("data-state") === "ready"; })()',
    { label: "auth: data-state=ready" },
  );
  await fixture.waitFor(
    `(() => { const rail = document.querySelector('#guild-rail');`
      + ' return rail !== null && rail.getAttribute("data-state") === "ready"; })()',
    { label: "rail: data-state=ready" },
  );
  return fixture;
}

/** Экспорт корней: нужен тестам, которые сверяют пути оракулов. */
export const M2_PATHS = Object.freeze({
  uiRoot: UI_ROOT,
  gatewayFixtures: GATEWAY_FIXTURES,
  data: M2_DATA_PATH,
  scenarios: SCENARIOS_PATH,
  indexHtml: join(UI_ROOT, "index.html"),
  appJs: join(UI_ROOT, "app.js"),
});

/** Проверка, что `ui/index.html` на месте: без него harness бессмысленен. */
export function m2UiPresent() {
  return existsSync(join(UI_ROOT, "index.html")) && existsSync(join(UI_ROOT, "app.js"));
}
