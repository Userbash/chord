// @ts-check
/**
 * V-08 (RW-40): голосовая панель на фейковом store — red-first тесты.
 *
 * Оракул — не текущая реализация, а принятый дизайн-контракт:
 * - `waves/RW-40/DESIGN/voice-spec.json` (schema `rw40/voice-spec-v1`):
 *   `panel.states` (12 состояний и их тексты), `copy` (строки i18n),
 *   `dom.ids` / `dom.states` / `dom.data_attrs`, `panel.participants`
 *   (строка 32, лимит 6, speaking-кольцо 2px `--positive`),
 *   `panel.ping` (формат `{ping} ms`, пороги 120/250), `deviations` D-01/D-04;
 * - `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.7: в текущей сборке реальны
 *   только `disabled` (voice-сессии нет) и `idle`, остальные — fake store;
 *   без голосовой сессии вход заблокирован, «подключено» не имитируется.
 *
 * Среда: DOM-шим RW-38 (`tests/support/gui-dom-harness.mjs`), без `fetch`,
 * `WebSocket`, `navigator.mediaDevices`, `AudioContext` и без IPC голоса
 * (новых endpoint'ов карточка не добавляет).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { setImmediate as tick } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { ShimDocument, loadApp } from "../support/gui-dom-harness.mjs";
import {
  PARTICIPANT_LIMIT,
  VOICE_STATES,
  VOICE_TEXT,
  createVoicePanel,
  createVoiceStore,
  createVoiceStageGrid,
  createGoLiveModal,
  createVoiceStreamController,
  pingQuality,
  speakingFlag,
} from "../../rust/gui-spike/ui/features/voice.js";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const root = new URL("../../", import.meta.url);
const uiDir = fileURLToPath(new URL("rust/gui-spike/ui/", root));
const spec = JSON.parse(
  readFileSync(fileURLToPath(new URL("waves/RW-40/DESIGN/voice-spec.json", root)), "utf8"),
);
const html = readFileSync(`${uiDir}index.html`, "utf8");
const voiceCss = readFileSync(`${uiDir}components/voice.css`, "utf8");
const voiceJs = readFileSync(`${uiDir}features/voice.js`, "utf8");
const tokensCss = readFileSync(`${uiDir}tokens.css`, "utf8");

/** Тексты, подтверждённые `[D]`, — берём из спеки, а не из модуля. */
/** @param {string} key */
function copyText(key) {
  const row = spec.copy.find((/** @type {any} */ entry) => entry.key === key);
  assert.ok(row, `в voice-spec.json нет строки ${key}`);
  return row.text;
}

/** @param {string} needle @returns {string} */
function authored(needle) {
  const text = spec.authored_copy.find((/** @type {string} */ entry) => entry.includes(needle));
  assert.ok(text, `в authored_copy нет строки «${needle}»`);
  return text;
}

/**
 * Фейковый voice-сервис: только запись вызовов, никакой сети/медиа.
 * @param {{ failJoin?: boolean, pendingJoin?: boolean }} [options]
 */
function makeService(options = {}) {
  /** @type {Array<{ op: string, args: any[] }>} */
  const calls = [];
  /** @param {string} op @returns {(...args: any[]) => Promise<unknown>} */
  const method = (op) => (...args) => {
    calls.push({ op, args });
    if (op === "join" && options.failJoin) return Promise.reject(new Error("join rejected"));
    if (op === "join" && options.pendingJoin) return new Promise(() => {});
    return Promise.resolve();
  };
  return {
    calls,
    join: method("join"),
    leave: method("leave"),
    reconnect: method("reconnect"),
    setSelfMute: method("setSelfMute"),
    setSelfDeaf: method("setSelfDeaf"),
    enablePtt: method("enablePtt"),
    voice_stream_create: method("voice_stream_create"),
    voice_stream_delete: method("voice_stream_delete"),
    voice_stream_watch: method("voice_stream_watch"),
    voice_stream_set_paused: method("voice_stream_set_paused"),
  };
}

/**
 * Панель в чистом шим-документе. `service` не передан → disabled (сборка без
 * голосового backend'а), как в `app.js`.
 * @param {{ service?: ReturnType<typeof makeService> | null, store?: any }} [options]
 */
function makePanel(options = {}) {
  const doc = new ShimDocument();
  const host = doc.createElement("section");
  host.id = "voice-panel";
  host.className = "voice-panel";
  doc.body.append(host);
  const store = options.store ?? createVoiceStore();
  const service = options.service === undefined ? makeService() : options.service;
  const panel = createVoicePanel({ host: /** @type {any} */ (host), store, service });
  return { doc, host, store, service, panel };
}

/** @param {ShimElement} panel @param {string} id */
function byId(panel, id) {
  const node = /** @type {ShimElement | null} */ (panel.querySelector(`#${id}`));
  assert.ok(node, `в панели нет #${id}`);
  return node;
}

/** @param {ShimElement} node @param {string} attr */
function attr(node, attr) {
  return node.getAttribute(attr);
}

/** @param {ShimElement} node @returns {boolean} */
function hidden(node) {
  return node.getAttribute("hidden") !== null;
}

/** @param {string} selector @param {string} css */
function cssBlock(selector, css) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  // Правило может быть составным (`.voice-user[data-speaking="true"] .avatar`);
  // берём первое, у которого селектор начинается с нужного фрагмента.
  const match = css.match(new RegExp(`${escaped}[^{}]*\\{([^}]*)\\}`));
  assert.ok(match, `в voice.css нет правила ${selector}`);
  return match[1] ?? "";
}

/** Пользователь события: только поля, которые разрешены §5.1 (id/name/mute/speaking). */
let participantSeq = 0;
/** @param {Record<string, any>} [overrides] */
function participant(overrides = {}) {
  participantSeq += 1;
  return {
    id: `u${participantSeq}`,
    name: `user_${participantSeq}`,
    self: false,
    speaking: false,
    mute: "none",
    ...overrides,
  };
}

const CHANNEL = /** @type {const} */ ({
  guildId: "1014537851418980372",
  channelId: "1176785373989445703",
  channelName: "тестируем твики / ITSKIDON PC BOOST",
});

/* -------------------------------------------------------------------------- */
/* 1. Статический контракт и текущая сборка                                    */
/* -------------------------------------------------------------------------- */

describe("V-08 voice: контракт разметки и сборки", () => {
  it("12 состояний и их порядок совпадают со спекой; тексты — из copy", () => {
    assert.equal(spec.panel.states.length, 12, "в спеке должно быть 12 состояний");
    assert.deepEqual([...VOICE_STATES], spec.dom.states, "VOICE_STATES разошлись с dom.states");
    const states = new Set(VOICE_STATES);
    for (const row of spec.panel.states) {
      assert.ok(states.has(row.state), `нет состояния ${row.state}`);
      if (typeof row.text === "string") {
        const fromCopy = spec.copy.find((/** @type {any} */ entry) => entry.text === row.text);
        assert.ok(fromCopy, `текст «${row.text}» не подтверждён copy-таблицей`);
        assert.equal(
          /** @type {Record<string, string>} */ (VOICE_TEXT)[row.state],
          fromCopy.text,
          `текст состояния ${row.state} разошёлся с i18n-оракулом`,
        );
      }
    }
  });

  it("index.html содержит host панели (id/класс/disabled) и подключает voice.css", () => {
    const tag = html.match(/<section[^>]*id="voice-panel"[^>]*>/)?.[0] ?? "";
    assert.ok(tag, "в index.html нет #voice-panel");
    assert.match(tag, /class="voice-panel"/, "#voice-panel без класса voice-panel");
    assert.match(tag, /data-state="disabled"/, "host должен стартовать в disabled (нет voice-сессии)");
    assert.match(html, /<link[^>]*href="components\/voice\.css"/, "voice.css не подключён");
  });

  it("в сборке без voice-сервиса панель disabled, а не «подключено»", async () => {
    const app = await loadApp();
    try {
      const panel = app.document.getElementById("voice-panel");
      assert.ok(panel, "в шим-DOM нет #voice-panel");
      assert.equal(attr(panel, "data-state"), "disabled", "без сессии состояние должно быть disabled");
      assert.equal(attr(panel, "aria-disabled"), "true", "disabled-панель не помечена aria-disabled");
      const label = /** @type {ShimElement} */ (app.document.getElementById("voice-status-label"));
      assert.equal(
        label.textContent,
        authored("Голосовой канал недоступен"),
        "disabled-статус не объясняет причину",
      );
      assert.equal(
        /** @type {ShimElement} */ (app.document.getElementById("voice-ping")).getAttribute("aria-disabled"),
        "true",
        "ping без голосовой сессии не должен быть активен",
      );
      assert.equal(app.callsOf("report_ui_snapshot").length > 0, true, "снапшот не снят");
      assert.equal(
        app.invokeCalls.some((call) => /voice/i.test(call.cmd)),
        false,
        "панель добавила IPC-команду голоса (новых endpoint'ов быть не должно)",
      );
    } finally {
      app.dispose();
    }
  });

  it("клик join без backend не даёт фейкового успеха и пишет причину в live-регион", async () => {
    const app = await loadApp();
    try {
      const join = /** @type {ShimElement} */ (app.document.getElementById("voice-join"));
      assert.ok(join, "нет кнопки входа");
      assert.equal(join.getAttribute("aria-disabled"), "true", "join без сессии должен быть disabled");
      const before = app.invokeCalls.length;
      join.click();
      await tick();
      const panel = /** @type {ShimElement} */ (app.document.getElementById("voice-panel"));
      assert.equal(attr(panel, "data-state"), "disabled", "клик join сымитировал подключение");
      assert.equal(app.invokeCalls.length, before, "клик join вызвал IPC");
      const a11y = /** @type {ShimElement} */ (app.document.getElementById("voice-a11y"));
      assert.match(a11y.textContent, /недоступен в этой сборке/, "причина отказа не объявлена");
    } finally {
      app.dispose();
    }
  });

  it("user-bar mute/deafen сохраняют прежний aria-pressed, панель их не подменяет", async () => {
    const app = await loadApp();
    try {
      const barMute = /** @type {ShimElement} */ (app.document.querySelector('[data-action="mute"]'));
      const barDeafen = /** @type {ShimElement} */ (app.document.querySelector('[data-action="deafen"]'));
      assert.ok(barMute && barDeafen, "нет кнопок user-bar");
      barMute.click();
      assert.equal(barMute.getAttribute("aria-pressed"), "true", "user-bar mute сломан");
      barDeafen.click();
      assert.equal(barDeafen.getAttribute("aria-pressed"), "true", "user-bar deafen сломан");

      const panelMute = /** @type {ShimElement} */ (app.document.getElementById("voice-mute"));
      const panelDeafen = /** @type {ShimElement} */ (app.document.getElementById("voice-deafen"));
      assert.equal(panelMute.getAttribute("aria-checked"), "false", "панель выдаёт mute за включённый");
      panelMute.click();
      panelDeafen.click();
      await tick();
      assert.equal(panelMute.getAttribute("aria-checked"), "false", "клик без сервиса переключил mute");
      assert.equal(panelDeafen.getAttribute("aria-checked"), "false", "клик без сервиса переключил deafen");
      assert.equal(panelMute.getAttribute("aria-disabled"), "true", "mute без voice-state должен быть disabled");
    } finally {
      app.dispose();
    }
  });
});

/* -------------------------------------------------------------------------- */
/* 2. Все 12 состояний на фейковом store                                       */
/* -------------------------------------------------------------------------- */

describe("V-08 voice: 12 состояний на fake store", () => {
  for (const row of spec.panel.states) {
    it(`${row.state}: data-state, статус, aria-busy/alert`, () => {
      const { host, store } = makePanel();
      store.set({ ...CHANNEL, state: row.state });
      assert.equal(attr(host, "data-state"), row.state, `data-state не ${row.state}`);
      const status = byId(host, "voice-status");
      const label = byId(host, "voice-status-label");
      if (row.status_block === "hidden") {
        assert.equal(hidden(status), true, `${row.state}: блок связи должен быть скрыт`);
        return;
      }
      assert.equal(hidden(status), false, `${row.state}: блок связи скрыт`);
      if (row.state === "loading") {
        assert.equal(status.getAttribute("aria-busy"), "true", "loading без aria-busy");
        assert.equal(label.textContent, "", "loading не должен показывать текст статуса");
        assert.ok(
          host.querySelectorAll(".voice-skeleton").length > 0,
          "loading без skeleton-плашек",
        );
        assert.equal(byId(host, "voice-leave").getAttribute("aria-disabled"), null, "leave заблокирован в loading");
      } else if (row.state === "error") {
        const error = byId(host, "voice-error");
        assert.equal(error.getAttribute("role"), "alert", "error без role=alert");
        assert.equal(error.textContent, copyText("S69lJR"), "текст error не S69lJR");
        assert.equal(hidden(error), false, "error скрыт");
        assert.equal(label.textContent, copyText("juL9/L"), "ошибка продублирована в polite-регионе");
      } else if (row.state === "disabled") {
        assert.equal(label.textContent, authored("Голосовой канал недоступен"), "disabled без причины");
        assert.equal(host.getAttribute("aria-disabled"), "true", "disabled без aria-disabled");
      } else {
        assert.equal(label.textContent, row.text, `текст ${row.state} разошёлся со спекой`);
      }
      if (row.state !== "disabled") {
        assert.equal(host.getAttribute("aria-disabled"), null, `${row.state}: панель помечена aria-disabled`);
      }
    });
  }

  it("состояние вне контракта не применяется", () => {
    const { host, store } = makePanel();
    store.set({ ...CHANNEL, state: "connected" });
    store.set({ state: "banana" });
    assert.equal(attr(host, "data-state"), "connected", "стор принял состояние вне контракта");
  });

  it("idle: блок связи скрыт, строка пользователя видна, join активен", () => {
    const { host, store } = makePanel();
    store.set({ ...CHANNEL, state: "idle" });
    assert.equal(hidden(byId(host, "voice-status")), true, "idle: статус виден");
    assert.equal(hidden(byId(host, "voice-participants")), true, "idle: список участников виден");
    assert.equal(hidden(byId(host, "voice-user-row")), false, "idle: строка пользователя скрыта");
    const join = byId(host, "voice-join");
    assert.equal(hidden(join), false, "idle: нет кнопки входа");
    assert.equal(join.getAttribute("aria-disabled"), null, "idle + сервис: вход должен быть активен");
  });

  it("connected: канал, ping 93 ms, участники, leave", () => {
    const { host, store } = makePanel();
    store.set({
      ...CHANNEL,
      state: "connected",
      ping: 93,
      participants: [participant({ self: true, name: "ShadowByte" })],
    });
    const link = byId(host, "voice-channel-link");
    assert.match(link.textContent, /PC BOOST/, "ссылка на канал без имени");
    assert.equal(link.getAttribute("title"), CHANNEL.channelName, "title не полное имя канала");
    assert.equal(
      link.getAttribute("href"),
      `#/channels/${CHANNEL.guildId}/${CHANNEL.channelId}`,
      "href канала не по контракту",
    );
    assert.equal(byId(host, "voice-ping-value").textContent, "93 ms", "ping не «93 ms»");
    assert.equal(
      byId(host, "voice-ping").getAttribute("aria-describedby"),
      "voice-ping-value",
      "ping без aria-describedby",
    );
    assert.equal(host.querySelectorAll(".voice-user").length, 1, "участники не отрисованы");
    assert.equal(hidden(byId(host, "voice-leave")), false, "connected без кнопки выхода");
  });

  it("ping: quality по порогам спеки, без значения — aria-disabled", () => {
    assert.equal(pingQuality(93), "fine");
    assert.equal(pingQuality(120), "fine");
    assert.equal(pingQuality(121), "average");
    assert.equal(pingQuality(250), "average");
    assert.equal(pingQuality(251), "bad");
    const { host, store } = makePanel();
    store.set({ ...CHANNEL, state: "connected", ping: 200 });
    assert.equal(byId(host, "voice-ping").getAttribute("data-quality"), "average", "quality не average");
    store.set({ ping: 400 });
    assert.equal(byId(host, "voice-ping").getAttribute("data-quality"), "bad", "quality не bad");
    store.set({ ping: null });
    assert.equal(byId(host, "voice-ping").getAttribute("aria-disabled"), "true", "нет ping, но кнопка активна");
    assert.equal(byId(host, "voice-ping-value").textContent, "", "нет данных, но значение показано");
  });

  it("empty: строка wM2WTM с role=status, participants пуст", () => {
    const { host, store } = makePanel();
    store.set({ ...CHANNEL, state: "empty", participants: [] });
    const empty = byId(host, "voice-empty");
    assert.equal(hidden(empty), false, "empty скрыт");
    assert.equal(empty.getAttribute("role"), "status", "empty без role=status");
    assert.equal(empty.textContent, copyText("wM2WTM"), "текст empty не wM2WTM");
    assert.equal(host.querySelectorAll(".voice-user").length, 0, "empty с участниками");
  });

  it("error: ровно один join на клик, фокус уходит на «Переподключиться»", async () => {
    const service = makeService({ failJoin: true });
    const { doc, host, store } = makePanel({ service });
    store.set({ ...CHANNEL, state: "idle" });
    const join = byId(host, "voice-join");
    join.focus();
    join.click();
    await tick();
    assert.equal(service.calls.filter((call) => call.op === "join").length, 1, "не 1 join на клик");
    assert.equal(attr(host, "data-state"), "error", "reject не перевёл панель в error");
    const reconnect = byId(host, "voice-reconnect");
    assert.equal(hidden(reconnect), false, "error без «Переподключиться»");
    assert.equal(doc.activeElement, reconnect, "фокус не переведён на «Переподключиться»");
  });

  it("повтор после ошибки: второй клик в pending не шлёт второй join", async () => {
    const service = makeService({ pendingJoin: true });
    const { host, store } = makePanel({ service });
    store.set({ ...CHANNEL, state: "error" });
    const reconnect = byId(host, "voice-reconnect");
    reconnect.click();
    reconnect.click();
    await tick();
    assert.equal(service.calls.filter((call) => call.op === "join").length, 1, "pending не защищён от повтора");
  });

  it("leave: 1 вызов, состояние меняется только по событию store", async () => {
    const service = makeService();
    const { host, store } = makePanel({ service });
    store.set({ ...CHANNEL, state: "connected", participants: [participant({ self: true })] });
    byId(host, "voice-leave").click();
    await tick();
    assert.equal(service.calls.filter((call) => call.op === "leave").length, 1, "не 1 leave");
    assert.equal(attr(host, "data-state"), "connected", "leave сработал оптимистично");
    store.set({ state: "idle", channelId: "", channelName: "", participants: [] });
    assert.equal(attr(host, "data-state"), "idle", "store-событие не применилось");
    assert.equal(hidden(byId(host, "voice-status")), true, "idle после выхода: статус виден");
    assert.equal(host.querySelectorAll(".voice-user").length, 0, "idle после выхода: участники остались");
  });

  it("no-permission: 0 вызовов сервиса, reconnect отсутствует", async () => {
    const service = makeService();
    const { host, store } = makePanel({ service });
    store.set({ ...CHANNEL, state: "no-permission" });
    await tick();
    assert.equal(service.calls.length, 0, "no-permission дёрнул сервис");
    assert.equal(hidden(byId(host, "voice-reconnect")), true, "no-permission показывает reconnect");
    assert.equal(
      byId(host, "voice-status-label").textContent,
      copyText("7/2/3M"),
      "текст no-permission не 7/2/3M",
    );
  });

  it("ptt-required: NYklhr + «Перейти на режим рации»", async () => {
    const service = makeService();
    const { host, store } = makePanel({ service });
    store.set({ ...CHANNEL, state: "ptt-required" });
    assert.equal(byId(host, "voice-status-label").textContent, copyText("NYklhr"), "нет NYklhr");
    const enable = byId(host, "voice-enable-ptt");
    assert.equal(hidden(enable), false, "нет кнопки перехода на рацию");
    enable.click();
    await tick();
    assert.equal(service.calls.filter((call) => call.op === "enablePtt").length, 1, "enablePtt не вызван");
  });

  it("device-missing: bgn75v + MA+OZh, разговор не рвётся", () => {
    const { host, store } = makePanel();
    store.set({ ...CHANNEL, state: "device-missing" });
    assert.equal(byId(host, "voice-status-label").textContent, copyText("bgn75v"), "нет bgn75v");
    assert.equal(byId(host, "voice-status-hint").textContent, copyText("MA+OZh"), "нет MA+OZh");
    assert.equal(hidden(byId(host, "voice-leave")), false, "device-missing не должен рвать сессию");
  });

  it("server-mute: mute aria-disabled, клик не меняет aria-checked и не зовёт сервис", async () => {
    const service = makeService();
    const { host, store } = makePanel({ service });
    store.set({ ...CHANNEL, state: "server-mute", selfMute: false });
    const mute = byId(host, "voice-mute");
    assert.equal(mute.getAttribute("aria-disabled"), "true", "server-mute: mute не disabled");
    assert.equal(byId(host, "voice-status-label").textContent, copyText("+JQCa/"), "нет +JQCa/");
    mute.click();
    await tick();
    assert.equal(mute.getAttribute("aria-checked"), "false", "клик изменил aria-checked");
    assert.equal(service.calls.filter((call) => call.op === "setSelfMute").length, 0, "сервис вызван при server-mute");
  });
});

/* -------------------------------------------------------------------------- */
/* 3. Участники, speaking, mute/deafen                                         */
/* -------------------------------------------------------------------------- */

describe("V-08 voice: участники и speaking", () => {
  it("своя строка первая, data-self/data-mute и tjtv3P в aria-label", () => {
    const { host, store } = makePanel();
    store.set({
      ...CHANNEL,
      state: "connected",
      selfMute: true,
      participants: [
        participant({ name: "Аня" }),
        participant({ self: true, name: "ShadowByte", mute: "self" }),
      ],
    });
    const rows = host.querySelectorAll(".voice-user");
    assert.equal(rows.length, 2, "не 2 строки");
    const first = /** @type {ShimElement} */ (rows[0]);
    assert.equal(first.getAttribute("data-self"), "true", "своя строка не первая");
    assert.equal(first.getAttribute("data-mute"), "self", "data-mute не self");
    assert.match(first.getAttribute("aria-label") ?? "", /ShadowByte/, "aria-label без имени");
    assert.match(first.getAttribute("aria-label") ?? "", /Этот микрофон отключён/, "нет tjtv3P в aria-label");
    assert.equal(first.getAttribute("role"), "listitem", "строка без role=listitem");
  });

  it("speaking: bool и число > 0, только у своего пользователя", () => {
    assert.equal(speakingFlag(0.4), true);
    assert.equal(speakingFlag(0), false);
    assert.equal(speakingFlag(true), true);
    assert.equal(speakingFlag(undefined), false);
    const { host, store } = makePanel();
    store.set({
      ...CHANNEL,
      state: "connected",
      participants: [
        participant({ name: "A", speaking: 0.4 }),
        participant({ name: "B", speaking: 0 }),
        participant({ name: "C" }),
      ],
    });
    const rows = host.querySelectorAll(".voice-user");
    assert.deepEqual(
      rows.map((row) => row.getAttribute("data-speaking")),
      ["true", "false", "false"],
      "data-speaking не по op5-правилу",
    );
    assert.match(rows[0]?.getAttribute("aria-label") ?? "", /говорит/, "речь не объявлена текстом");
  });

  it("без op5 ни у кого нет data-speaking=true", () => {
    const { host, store } = makePanel();
    store.set({
      ...CHANNEL,
      state: "connected",
      participants: [participant({ name: "A" }), participant({ name: "B" })],
    });
    assert.equal(
      host.querySelectorAll('.voice-user[data-speaking="true"]').length,
      0,
      "панель выдумала речь без op5",
    );
  });

  it("7 участников → 6 строк + «Ещё 1»", () => {
    const { host, store } = makePanel();
    store.set({
      ...CHANNEL,
      state: "connected",
      participants: Array.from({ length: 7 }, () => participant()),
    });
    assert.equal(host.querySelectorAll(".voice-user").length, PARTICIPANT_LIMIT, "лимит строк не 6");
    const overflow = byId(host, "voice-overflow");
    assert.equal(hidden(overflow), false, "нет строки «Ещё n»");
    assert.match(overflow.textContent, /^Ещё 1/, "счётчик не «Ещё 1»");
  });

  it("чужой mute: data-mute=server и текст в aria-label", () => {
    const { host, store } = makePanel();
    store.set({
      ...CHANNEL,
      state: "connected",
      participants: [participant({ name: "Аня", mute: "server" })],
    });
    const row = /** @type {ShimElement} */ (host.querySelector(".voice-user"));
    assert.equal(row.getAttribute("data-mute"), "server", "data-mute не server");
    assert.match(row.getAttribute("aria-label") ?? "", /микрофон отключён/, "нет текста server-mute");
  });

  it("mute/deafen: сервис вызывается, aria-checked только из store", async () => {
    const service = makeService();
    const { host, store } = makePanel({ service });
    store.set({ ...CHANNEL, state: "connected", selfMute: false, selfDeaf: false });
    const mute = byId(host, "voice-mute");
    const deafen = byId(host, "voice-deafen");
    assert.equal(mute.getAttribute("role"), "switch", "mute без role=switch");
    assert.equal(mute.getAttribute("aria-label"), "Заглушить", "aria-label mute не sWmtI6");
    assert.equal(deafen.getAttribute("aria-label"), "Откл. звук", "aria-label deafen не hK51Yg");
    mute.click();
    await tick();
    assert.equal(service.calls.filter((call) => call.op === "setSelfMute").length, 1, "нет вызова setSelfMute");
    assert.equal(mute.getAttribute("aria-checked"), "false", "mute переключился оптимистично");
    store.set({ selfMute: true, selfDeaf: true });
    assert.equal(mute.getAttribute("aria-checked"), "true", "aria-checked не синхронен с store");
    assert.equal(attr(host, "data-self-mute"), "true", "data-self-mute не синхронен");
    assert.equal(deafen.getAttribute("aria-checked"), "true", "deafen aria-checked не синхронен");
    assert.equal(attr(host, "data-self-deaf"), "true", "data-self-deaf не синхронен");
  });
});

/* -------------------------------------------------------------------------- */
/* 4. PTT-индикация                                                           */
/* -------------------------------------------------------------------------- */

describe("V-08 voice: индикация PTT", () => {
  it("data-ptt idle|held|blocked и текстовый индикатор (не только цветом)", () => {
    const { host, store } = makePanel();
    store.set({
      ...CHANNEL,
      state: "connected",
      participants: [participant({ self: true, name: "ShadowByte" })],
    });
    assert.equal(attr(host, "data-ptt"), "idle", "по умолчанию не idle");
    store.set({ ptt: "held" });
    assert.equal(attr(host, "data-ptt"), "held", "data-ptt не held");
    assert.match(byId(host, "voice-a11y").textContent, /микрофон активен/i, "held не объявлен текстом");
    store.set({ ptt: "blocked" });
    assert.equal(attr(host, "data-ptt"), "blocked", "data-ptt не blocked");
    assert.match(
      byId(host, "voice-a11y").textContent,
      /системных настроек доступа/,
      "blocked без текста vxfv7v",
    );
    store.set({ ptt: "nonsense" });
    assert.equal(attr(host, "data-ptt"), "blocked", "стор принял неизвестный ptt");
  });
});

/* -------------------------------------------------------------------------- */
/* 5. Доступность                                                             */
/* -------------------------------------------------------------------------- */

describe("V-08 voice: доступность", () => {
  it("у каждой иконочной кнопки русский aria-label, svg aria-hidden", () => {
    const { host, store } = makePanel();
    store.set({ ...CHANNEL, state: "connected", participants: [participant()] });
    for (const button of host.querySelectorAll("button")) {
      const label = button.getAttribute("aria-label") ?? "";
      assert.ok(label.length > 0, `кнопка #${button.id} без aria-label`);
      assert.match(label, /[А-Яа-яЁё]/, `aria-label «${label}» не русский`);
    }
    const svgs = host.querySelectorAll("svg");
    assert.ok(svgs.length > 0, "в панели нет inline-SVG");
    for (const svg of svgs) {
      assert.equal(svg.getAttribute("aria-hidden"), "true", "svg без aria-hidden");
    }
  });

  it("один live-регион статуса + отдельный на действия; ошибка assertive", () => {
    const { host } = makePanel();
    const live = host.querySelectorAll('[aria-live="polite"]');
    assert.equal(live.length, 2, "ожидались два polite-региона: статус и действия");
    assert.equal(byId(host, "voice-status").getAttribute("aria-live"), "polite", "статус без polite");
    assert.equal(byId(host, "voice-a11y").getAttribute("aria-live"), "polite", "действия без polite");
    assert.equal(byId(host, "voice-error").getAttribute("role"), "alert", "ошибка без role=alert");
  });

  it("ping доступен с клавиатуры, значение — через aria-describedby", () => {
    const { host } = makePanel();
    const ping = byId(host, "voice-ping");
    assert.equal(ping.tagName, "BUTTON", "ping не button");
    assert.notEqual(ping.getAttribute("tabindex"), "-1", "ping выведен из tab-порядка");
    const value = byId(host, "voice-ping-value");
    assert.equal(value.getAttribute("class"), "visually-hidden", "значение ping видимое");
    assert.equal(ping.getAttribute("aria-describedby"), value.id, "aria-describedby не на значение");
  });

  it("Tab-порядок панели: ping → leave → mute → deafen", () => {
    const { host, store } = makePanel();
    store.set({ ...CHANNEL, state: "connected", participants: [participant()] });
    const ids = host.querySelectorAll("button")
      .filter((button) => button.getAttribute("hidden") === null)
      .map((button) => button.id);
    assert.deepEqual(
      ids,
      ["voice-ping", "voice-leave", "voice-mute", "voice-deafen"],
      "порядок кнопок разошёлся с §4.6/VK-05",
    );
  });
});

/* -------------------------------------------------------------------------- */
/* 6. CSS-контракт и приватность                                              */
/* -------------------------------------------------------------------------- */

describe("V-08 voice: CSS-контракт и приватность", () => {
  it("строка участника 32px, speaking-кольцо 2px --positive (оракул — спека)", () => {
    const rowH = spec.panel.participants.row_h;
    const ring = spec.panel.participants.speaking.visual;
    assert.equal(rowH, 32, "спека изменилась: row_h");
    assert.match(ring, /2px/, "спека изменилась: кольцо");
    const row = cssBlock(".voice-user", voiceCss);
    assert.match(row, new RegExp(`height:\\s*${rowH}px`), ".voice-user не 32px");
    const speaking = cssBlock('.voice-user[data-speaking="true"]', voiceCss);
    assert.match(speaking, /2px/, "speaking-кольцо не 2px");
    assert.match(speaking, /var\(--positive\)/, "speaking-кольцо не --positive");
    assert.match(voiceCss, /prefers-reduced-motion/, "нет reduced-motion");
  });

  it("токены voice.css существуют в tokens.css; нет внешних ассетов", () => {
    const declared = new Set(
      [...tokensCss.matchAll(/(--[a-z0-9-]+)\s*:/g)].map((match) => match[1] ?? ""),
    );
    const used = [...voiceCss.matchAll(/var\((--[a-z0-9-]+)\)/g)].map((match) => match[1] ?? "");
    assert.ok(used.length > 0, "voice.css не использует токены");
    for (const name of used) {
      assert.ok(declared.has(name), `voice.css использует несуществующий токен ${name}`);
    }
    assert.equal(/url\(|@import|https?:\/\//.test(voiceCss), false, "внешние ассеты в voice.css");
  });

  it("в коде панели нет media/record/screen API и новых каналов", () => {
    const forbidden = [
      "navigator.mediaDevices",
      "getUserMedia",
      "getDisplayMedia",
      "startRecordingRawSamples",
      "startLocalAudioRecording",
      "setAecDump",
      "setLoopback",
      "getScreenPreviews",
      "getWindowPreviews",
      "fetch(",
      "WebSocket",
      "__TAURI__",
      "invoke(",
    ];
    for (const needle of forbidden) {
      assert.equal(voiceJs.includes(needle), false, `voice.js содержит запрещённое «${needle}»`);
    }
  });

  it("панель не логирует имена участников/устройств", () => {
    assert.equal(/console\.(log|warn|error)/.test(voiceJs), false, "voice.js пишет в console");
  });
});

/* -------------------------------------------------------------------------- */
/* 7. FE-P1-02 / task-154: Go Live & Stream Tiles UI                          */
/* -------------------------------------------------------------------------- */

describe("FE-P1-02 / task-154: Go Live & Stream Tiles UI в voice.js", () => {
  /** @param {Record<string, any>} [options] */
  function makeStage(options = {}) {
    const doc = new ShimDocument();
    const host = doc.createElement("section");
    host.id = "voice-stage";
    doc.body.append(host);
    const store = options.store ?? createVoiceStore();
    const service = options.service === undefined ? makeService() : options.service;
    const stage = createVoiceStageGrid({ host: /** @type {any} */ (host), store, service, ...options });
    return { doc, host, store, service, stage };
  }

  /** @param {Record<string, any>} [options] */
  function makeModal(options = {}) {
    const doc = new ShimDocument();
    const host = doc.createElement("div");
    host.id = "modal-root";
    doc.body.append(host);
    const service = options.service === undefined ? makeService() : options.service;
    const modal = createGoLiveModal({ host: /** @type {any} */ (host), service, ...options });
    return { doc, host, service, modal };
  }

  /** @param {Record<string, any>} [options] */
  function makePanelWithStage(options = {}) {
    const doc = new ShimDocument();
    const host = doc.createElement("section");
    host.id = "voice-panel";
    host.className = "voice-panel";
    const stageHost = doc.createElement("section");
    stageHost.id = "voice-stage-host";
    doc.body.append(host, stageHost);
    const store = options.store ?? createVoiceStore();
    const service = options.service === undefined ? makeService() : options.service;
    const panel = createVoicePanel({
      host: /** @type {any} */ (host),
      stageHost: /** @type {any} */ (stageHost),
      store,
      service,
      ...options,
    });
    return { doc, host, stageHost, store, service, panel };
  }

  // 1. Сетка видеоплиток (#voice-stage-grid / .voice-tile)
  it("сетка видеоплиток #voice-stage-grid отображает плитки участников .voice-tile", () => {
    const { host, store } = makeStage();
    store.set({
      ...CHANNEL,
      state: "connected",
      participants: [
        participant({ id: "u1", name: "Alice" }),
        participant({ id: "u2", name: "Bob" }),
      ],
    });
    const grid = byId(host, "voice-stage-grid");
    assert.ok(grid, "в stage нет #voice-stage-grid");
    const tiles = host.querySelectorAll(".voice-tile");
    assert.equal(tiles.length, 2, "число .voice-tile не равно числу участников");
    assert.equal(tiles[0]?.getAttribute("data-user-id"), "u1");
    assert.equal(tiles[1]?.getAttribute("data-user-id"), "u2");
    assert.match(tiles[0]?.textContent ?? "", /Alice/);
    assert.match(tiles[1]?.textContent ?? "", /Bob/);
    assert.equal(tiles[0]?.classList.contains("is-speaking"), false, "тихий участник получил is-speaking");
  });

  it("активная говорящая рамка (.is-speaking) выставляется при speaking=true и снимается при false", () => {
    const { host, store } = makeStage();
    store.set({
      ...CHANNEL,
      state: "connected",
      participants: [
        participant({ id: "u1", name: "Alice", speaking: true }),
        participant({ id: "u2", name: "Bob", speaking: false }),
      ],
    });
    const tileAlice = host.querySelector('.voice-tile[data-user-id="u1"]');
    const tileBob = host.querySelector('.voice-tile[data-user-id="u2"]');
    assert.ok(tileAlice, "плитка Alice не найдена");
    assert.ok(tileBob, "плитка Bob не найдена");
    assert.equal(tileAlice.classList.contains("is-speaking"), true, "Alice без .is-speaking");
    assert.equal(tileAlice.getAttribute("data-speaking"), "true", "Alice data-speaking не true");
    assert.equal(tileBob.classList.contains("is-speaking"), false, "Bob получил .is-speaking");

    store.set({
      participants: [
        participant({ id: "u1", name: "Alice", speaking: false }),
        participant({ id: "u2", name: "Bob", speaking: true }),
      ],
    });
    const nextAlice = host.querySelector('.voice-tile[data-user-id="u1"]');
    const nextBob = host.querySelector('.voice-tile[data-user-id="u2"]');
    assert.ok(nextAlice);
    assert.ok(nextBob);
    assert.equal(nextAlice.classList.contains("is-speaking"), false, "Alice не сбросила .is-speaking");
    assert.equal(nextBob.classList.contains("is-speaking"), true, "Bob не получил .is-speaking");
  });

  it("плитка стрима Go Live имеет бейдж 'В ЭФИРЕ' (#stream-live-badge / .badge-live)", () => {
    const { host, store } = makeStage();
    store.set({
      ...CHANNEL,
      state: "connected",
      participants: [
        participant({ id: "u1", name: "Alice", isStream: true }),
        participant({ id: "u2", name: "Bob" }),
      ],
    });
    const streamTile = host.querySelector('.voice-tile[data-user-id="u1"]');
    assert.ok(streamTile, "плитка стрима не найдена");
    assert.equal(streamTile.getAttribute("data-is-stream"), "true", "нет data-is-stream");
    const badge = streamTile.querySelector("#stream-live-badge, .badge-live");
    assert.ok(badge, "нет бейджа стрима #stream-live-badge / .badge-live");
    assert.match(badge.textContent ?? "", /В ЭФИРЕ/, "текст бейджа не содержит 'В ЭФИРЕ'");
  });

  it("переключение фокуса (#voice-stage-focused) активирует фокусный режим выбранной плитки и переключает его", () => {
    const { host, store } = makeStage();
    store.set({
      ...CHANNEL,
      state: "connected",
      participants: [
        participant({ id: "u1", name: "Alice", isStream: true }),
        participant({ id: "u2", name: "Bob" }),
      ],
    });
    const focusedArea = byId(host, "voice-stage-focused");
    assert.ok(focusedArea, "нет контейнера #voice-stage-focused");
    assert.equal(focusedArea.getAttribute("hidden") !== null, true, "по умолчанию #voice-stage-focused должен быть скрыт");

    const tileAlice = /** @type {ShimElement} */ (host.querySelector('.voice-tile[data-user-id="u1"]'));
    assert.ok(tileAlice);
    tileAlice.click();
    assert.equal(focusedArea.getAttribute("hidden"), null, "#voice-stage-focused не открылся после клика");
    assert.equal(focusedArea.getAttribute("data-focused-user-id"), "u1", "data-focused-user-id не u1");
    assert.equal(tileAlice.classList.contains("is-focused"), true, "плитка Alice не получила .is-focused");

    // Переключение на другую плитку (Bob)
    const tileBob = /** @type {ShimElement} */ (host.querySelector('.voice-tile[data-user-id="u2"]'));
    assert.ok(tileBob);
    tileBob.click();
    assert.equal(focusedArea.getAttribute("data-focused-user-id"), "u2", "data-focused-user-id не переключился на Bob");
    assert.equal(tileAlice.classList.contains("is-focused"), false, "плитка Alice сохранила .is-focused");
    assert.equal(tileBob.classList.contains("is-focused"), true, "плитка Bob не получила .is-focused");

    // Повторный клик по активной плитке снимает фокус
    tileBob.click();
    assert.equal(focusedArea.getAttribute("hidden") !== null, true, "повторный клик не снял фокус");
    assert.equal(tileBob.classList.contains("is-focused"), false, "плитка Bob сохранила .is-focused");
  });

  // 2. Модалка запуска стрима Go Live (#go-live-modal, role='dialog')
  it("модалка #go-live-modal имеет role='dialog', вкладки 'Приложения' / 'Экраны' и переключает их", () => {
    const { host } = makeModal();
    const modal = byId(host, "go-live-modal");
    assert.equal(modal.getAttribute("role"), "dialog", "модалка без role=dialog");
    assert.equal(modal.getAttribute("aria-modal"), "true", "модалка без aria-modal=true");

    const tabApps = /** @type {ShimElement} */ (host.querySelector('#go-live-tab-apps, [data-tab="applications"]'));
    const tabScreens = /** @type {ShimElement} */ (host.querySelector('#go-live-tab-screens, [data-tab="screens"]'));
    assert.ok(tabApps, "нет вкладки Приложения");
    assert.ok(tabScreens, "нет вкладки Экраны");
    assert.match(tabApps.textContent ?? "", /Приложения/);
    assert.match(tabScreens.textContent ?? "", /Экраны/);
    assert.equal(tabApps.getAttribute("aria-selected"), "true", "Приложения должны быть выбраны по умолчанию");
    assert.equal(tabScreens.getAttribute("aria-selected"), "false", "Экраны не должны быть выбраны по умолчанию");

    tabScreens.click();
    assert.equal(tabScreens.getAttribute("aria-selected"), "true", "клик по Экраны не переключил вкладку");
    assert.equal(tabApps.getAttribute("aria-selected"), "false", "Приложения остались активными");
  });

  it("выбор разрешения (720p / 1080p / 1440p) и FPS (30 / 60) в модалке", () => {
    const { host, modal } = makeModal();
    const btn720 = /** @type {ShimElement} */ (host.querySelector('[data-resolution="720p"]'));
    const btn1080 = /** @type {ShimElement} */ (host.querySelector('[data-resolution="1080p"]'));
    const btn1440 = /** @type {ShimElement} */ (host.querySelector('[data-resolution="1440p"]'));
    assert.ok(btn720, "нет опции 720p");
    assert.ok(btn1080, "нет опции 1080p");
    assert.ok(btn1440, "нет опции 1440p");

    const btnFps30 = /** @type {ShimElement} */ (host.querySelector('[data-fps="30"]'));
    const btnFps60 = /** @type {ShimElement} */ (host.querySelector('[data-fps="60"]'));
    assert.ok(btnFps30, "нет опции 30 fps");
    assert.ok(btnFps60, "нет опции 60 fps");

    btn1080.click();
    assert.equal(modal.getResolution(), "1080p", "getResolution() не вернул 1080p");
    assert.equal(btn1080.getAttribute("aria-checked"), "true");
    assert.equal(btn720.getAttribute("aria-checked"), "false");

    btnFps60.click();
    assert.equal(modal.getFps(), 60, "getFps() не вернул 60");
    assert.equal(btnFps60.getAttribute("aria-checked"), "true");
    assert.equal(btnFps30.getAttribute("aria-checked"), "false");
  });

  it("кнопка 'Прямой эфир' (#go-live-submit-btn) вызывает voice_stream_create / onStartStream и закрывает модалку", async () => {
    const service = makeService();
    let started = null;
    const { host } = makeModal({
      service,
      onStartStream: (/** @type {any} */ params) => { started = params; },
    });
    const submitBtn = byId(host, "go-live-submit-btn");
    assert.match(submitBtn.textContent ?? "", /Прямой эфир/, "кнопка не содержит 'Прямой эфир'");

    submitBtn.click();
    await tick();

    const createCalls = service.calls.filter((c) => c.op === "voice_stream_create");
    assert.equal(createCalls.length, 1, "voice_stream_create не был вызван в сервисе");
    assert.ok(started, "onStartStream колбэк не был вызван");
    assert.equal(byId(host, "go-live-modal").getAttribute("hidden") !== null, true, "модалка не закрылась после отправки");
  });

  // 3. Элементы управления активным стримом (просмотр, пауза, остановка)
  it("просмотр чужого стрима: клик вызывает voice_stream_watch / onWatchStream", async () => {
    const service = makeService();
    let watchedId = null;
    const { host, store } = makeStage({
      service,
      onWatchStream: (/** @type {any} */ id) => { watchedId = id; },
    });
    store.set({
      ...CHANNEL,
      state: "connected",
      participants: [
        participant({ id: "u42", name: "StreamerUser", isStream: true }),
      ],
    });
    const watchBtn = /** @type {ShimElement} */ (host.querySelector('.stream-watch-btn, [data-stream-action="watch"]'));
    assert.ok(watchBtn, "нет кнопки просмотра стрима .stream-watch-btn");

    watchBtn.click();
    await tick();

    const watchCalls = service.calls.filter((c) => c.op === "voice_stream_watch");
    assert.equal(watchCalls.length, 1, "voice_stream_watch не был вызван в сервисе");
    assert.equal(watchCalls[0]?.args[0], "u42", "аргумент watchCalls не u42");
    assert.equal(watchedId, "u42", "onWatchStream не получил u42");
  });

  it("пауза стрима: контрол паузы вызывает voice_stream_set_paused / onPauseStream и меняет состояние", async () => {
    const service = makeService();
    let pausedState = null;
    const { host } = makeStage({
      service,
      onPauseStream: (/** @type {any} */ val) => { pausedState = val; },
    });
    const pauseBtn = byId(host, "stream-pause-btn");
    assert.ok(pauseBtn, "нет кнопки паузы #stream-pause-btn");
    assert.equal(pauseBtn.getAttribute("aria-pressed"), "false");
    assert.match(pauseBtn.textContent ?? "", /Приостановить стрим/);

    pauseBtn.click();
    await tick();

    const pauseCalls = service.calls.filter((c) => c.op === "voice_stream_set_paused");
    assert.equal(pauseCalls.length, 1, "voice_stream_set_paused не был вызван");
    assert.equal(pauseCalls[0]?.args[0], true, "voice_stream_set_paused аргумент не true");
    assert.equal(pausedState, true, "onPauseStream не получил true");
    assert.equal(pauseBtn.getAttribute("aria-pressed"), "true", "aria-pressed не стал true");
    assert.match(pauseBtn.textContent ?? "", /Возобновить стрим/, "текст не сменился на Возобновить");

    pauseBtn.click();
    await tick();

    assert.equal(service.calls.filter((c) => c.op === "voice_stream_set_paused").length, 2);
    assert.equal(pausedState, false, "onPauseStream не получил false");
    assert.equal(pauseBtn.getAttribute("aria-pressed"), "false", "aria-pressed не вернулся в false");
  });

  it("остановка стрима: контрол остановки вызывает voice_stream_delete / onStopStream", async () => {
    const service = makeService();
    let stopped = false;
    const { host } = makeStage({
      service,
      onStopStream: () => { stopped = true; },
    });
    const stopBtn = byId(host, "stream-stop-btn");
    assert.ok(stopBtn, "нет кнопки остановки #stream-stop-btn");
    assert.match(stopBtn.textContent ?? "", /Прекратить стрим/);

    stopBtn.click();
    await tick();

    const deleteCalls = service.calls.filter((c) => c.op === "voice_stream_delete");
    assert.equal(deleteCalls.length, 1, "voice_stream_delete не был вызван в сервисе");
    assert.equal(stopped, true, "onStopStream не был вызван");
  });

  // 4. Оверлей возрастного ограничения AGE_GATED_SPACES (#voice-age-gated-overlay)
  it("оверлей #voice-age-gated-overlay блокирует предпросмотр до подтверждения", async () => {
    let confirmed = false;
    const { host, store } = makeStage({
      onConfirmAge: () => { confirmed = true; },
    });
    store.set({
      ...CHANNEL,
      state: "connected",
      ageGated: true,
      participants: [
        participant({ id: "u1", name: "MatureStreamer", isStream: true }),
      ],
    });

    const overlay = byId(host, "voice-age-gated-overlay");
    assert.ok(overlay, "нет #voice-age-gated-overlay");
    assert.equal(overlay.getAttribute("hidden"), null, "overlay должен быть видим при ageGated: true");
    const grid = byId(host, "voice-stage-grid");
    assert.equal(grid.getAttribute("data-blocked"), "true", "grid не заблокирован оверлеем");

    const confirmBtn = /** @type {ShimElement} */ (host.querySelector('#voice-age-gated-confirm-btn, [data-action="confirm-age"]'));
    assert.ok(confirmBtn, "нет кнопки подтверждения возраста");
    assert.match(confirmBtn.textContent ?? "", /18|Подтвердить|Продолжить/i);

    confirmBtn.click();
    await tick();

    assert.equal(overlay.getAttribute("hidden") !== null, true, "overlay не скрылся после подтверждения");
    assert.equal(overlay.getAttribute("data-confirmed"), "true", "overlay не получил data-confirmed='true'");
    assert.equal(grid.getAttribute("data-blocked"), "false", "grid не разблокирован");
    assert.equal(confirmed, true, "onConfirmAge не был вызван");
  });

  it("интеграция в createVoicePanel: панель связывается со сценой и модалкой запуска стрима", async () => {
    const service = makeService();
    const { host, stageHost, store, panel } = makePanelWithStage({ service });
    store.set({
      ...CHANNEL,
      state: "connected",
      participants: [
        participant({ id: "u1", name: "Alice", isStream: true }),
      ],
    });

    assert.ok(panel.stageGrid, "panel.stageGrid не экспортирован");
    assert.ok(panel.goLiveModal, "panel.goLiveModal не экспортирован");
    assert.ok(panel.ageGatedOverlay, "panel.ageGatedOverlay не экспортирован");

    // Открытие модалки стрима через метод панели
    panel.openGoLiveModal();
    assert.equal(panel.goLiveModal.getAttribute("hidden"), null, "openGoLiveModal не открыл модалку");

    panel.closeGoLiveModal();
    assert.equal(panel.goLiveModal.getAttribute("hidden") !== null, true, "closeGoLiveModal не закрыл модалку");
  });
});
