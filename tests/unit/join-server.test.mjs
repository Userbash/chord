// @ts-check
/**
 * G-03-UI: модалка «Присоединиться к серверу» на backend `resolve_invite` /
 * `accept_invite` (D-116), red-first, без сети и окна.
 *
 * Оракул — не текущая реализация, а документы:
 *   - `waves/RW-40/DESIGN/SERVER-MENU.md` §2.3 (два шага: resolve → accept),
 *     §3.1 (тексты ошибок), §5.1 (копирайт), §6 (DOM-контракт `#join-server-dialog`),
 *     §7.3 (SM-20…SM-39);
 *   - `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.3: `resolve_invite(code)` →
 *     `{code, guildId, guildName, icon?, memberCount?, presenceCount?, expiresAt?,
 *     channelId?}`, `accept_invite(code)` → `{guildId, targetChannelId?, targetMessageId?}`;
 *     accept — только 2xx, тело — три nullable поля, токен не приходит из фронтенда;
 *   - `waves/RW-40/PM/g03/REPORT.md` — принятый backend (типизированные `kind`:
 *     unauthorized/forbidden/notFound/rateLimited/…).
 *
 * Шим (`tests/support/gui-dom-harness.mjs`) не всплывает события и не считает
 * layout: клики/клавиши адресуются самому узлу, проверяются атрибуты/текст и
 * вызовы IPC-заглушки. `fetch`, окно и реальный Discord не используются.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadApp } from "../support/gui-dom-harness.mjs";

/** @typedef {import("../support/gui-dom-harness.mjs").GuiDom} GuiDom */
/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

const uiDir = fileURLToPath(new URL("../../rust/gui-spike/ui/", import.meta.url));
const html = readFileSync(`${uiDir}index.html`, "utf8");
const source = readFileSync(`${uiDir}features/join-server.js`, "utf8");

/** `ReadyPayload` в форме `data.rs` (`ready()`). */
const READY = Object.freeze({
  user: { id: "42", username: "tester", globalName: "Тестер", avatar: null },
  guilds: [
    { id: "111", name: "Первый сервер", icon: null, memberCount: 3, channelCount: 2 },
  ],
  guildCount: 1,
  privateChannelCount: 0,
  relationshipCount: 0,
  readStateCount: 0,
  session: null,
});

/** Ответ `resolve_invite` (InvitePreview, IPC-camelCase). */
const PREVIEW = Object.freeze({
  code: "abc123",
  guildId: "999",
  guildName: "Новый сервер",
  icon: "a_1b2c3d",
  memberCount: 12345,
  presenceCount: 678,
  expiresAt: "2026-11-01T00:00:00+00:00",
  channelId: "777",
});

/**
 * @param {GuiDom} app
 * @returns {import("../../rust/gui-spike/ui/app.js").App}
 */
function api(app) {
  const instance = /** @type {any} */ (app.appModule)?.currentApp?.();
  assert.ok(instance, "app.js не экспортирует currentApp()");
  return instance;
}

/** @param {GuiDom} app @param {string} selector @returns {ShimElement} */
function firstNode(app, selector) {
  const node = app.document.querySelector(selector);
  assert.ok(node, `нет ${selector}`);
  return node;
}

/** @param {GuiDom} app @param {string} selector @returns {ShimElement[]} */
function all(app, selector) {
  return app.document.querySelectorAll(selector);
}

/**
 * app.js с пройденным входом; `ready()` отдаёт одну гильдию, `channels()` — пусто,
 * остальные команды — сценарий теста.
 * @param {(cmd: string, args: any) => any} handler
 * @returns {Promise<GuiDom>}
 */
function loadJoinApp(handler) {
  return loadApp({
    invokeHandler: (cmd, args) => {
      if (cmd === "auth_status") {
        return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
      }
      if (cmd === "ready") return READY;
      if (cmd === "channels") return [];
      return handler(cmd, args);
    },
  });
}

/**
 * @param {GuiDom} app
 * @returns {ShimElement} плитка «+» в рейле
 */
function addTile(app) {
  return firstNode(app, '#guild-rail [data-action="add-server"]');
}

/** @param {GuiDom} app */
function openDialog(app) {
  addTile(app).click();
}

/** @param {GuiDom} app @param {string} value */
function typeCode(app, value) {
  const input = /** @type {any} */ (firstNode(app, "#join-invite-input"));
  input.value = value;
  input.emit("input");
}

/** @param {GuiDom} app @returns {ShimElement} */
function dialog(app) {
  return firstNode(app, "#join-server-dialog");
}

/** @param {GuiDom} app @returns {boolean} */
function dialogOpen(app) {
  return dialog(app).getAttribute("hidden") === null;
}

/** @param {GuiDom} app @returns {ShimElement} */
function enterButton(app) {
  return firstNode(app, '#join-preview [data-action="enter-guild"]');
}

/** @param {GuiDom} app */
async function reachPreview(app) {
  openDialog(app);
  typeCode(app, "abc123");
  firstNode(app, "#join-submit").click();
  await app.settle(3);
}

/** @param {GuiDom} app @param {string} code @param {any} [preview] */
function resolveOk(app, code = "abc123", preview = PREVIEW) {
  app.setInvokeHandler((cmd) => {
    if (cmd === "resolve_invite") return { ...preview, code };
    if (cmd === "accept_invite") return { guildId: "999" };
    if (cmd === "ready") return READY;
    if (cmd === "channels") return [];
    return undefined;
  });
}

describe("G-03-UI: нормализация кода приглашения (SM-23)", () => {
  /**
   * Чистая функция без DOM; правило совпадает с backend `normalize_code`
   * (`rust/gui-spike/src/invites.rs`, принято D-116): trim, снять `?`/`#`,
   * снять хвостовой `/`, последний сегмент, `[A-Za-z0-9-]`, 2…32.
   */
  it("ссылки discord.gg/invite, хвостовой слэш, пробелы и query — один и тот же код", async () => {
    const { normalizeInvite } = await import("../../rust/gui-spike/ui/features/join-server.js");
    const cases = [
      ["abc123", "abc123"],
      ["  abc123  ", "abc123"],
      ["https://discord.gg/abc123/", "abc123"],
      ["https://discord.com/invite/abc123", "abc123"],
      ["discord.com/invite/abc123?event=1", "abc123"],
      ["discord.gg/abc123#frag", "abc123"],
      ["AB-12", "AB-12"],
    ];
    for (const [input, expected] of cases) {
      assert.equal(normalizeInvite(input), expected, `normalizeInvite(${JSON.stringify(input)})`);
    }
  });

  it("пустое, короткое, длинное и недопустимые символы → \"\" (запрос не уходит)", async () => {
    const { normalizeInvite } = await import("../../rust/gui-spike/ui/features/join-server.js");
    for (const input of ["", "   ", "a", "abc 123", "код", "abc_123", "a".repeat(33), "discord.com/invite/"]) {
      assert.equal(normalizeInvite(input), "", `normalizeInvite(${JSON.stringify(input)}) должен быть пустым`);
    }
  });
});

describe("G-03-UI: диалог join (#join-server-dialog)", () => {
  it("в index.html нет фальшивой разметки: только host, содержимое собирает JS", () => {
    assert.match(html, /id="join-server-dialog"/, "в index.html нет host #join-server-dialog");
    // Данные preview не могут быть захардкожены: в разметке нет узлов preview.
    assert.equal(/join-preview-name|join-preview-meta/.test(html), false, "в index.html захардкожен preview");
  });

  it("«+» в рейле: aria-label/haspopup, открывает диалог, фокус в поле (SM-20, SM-21)", async () => {
    const app = await loadJoinApp(() => undefined);
    try {
      const plus = addTile(app);
      assert.equal(plus.getAttribute("aria-label"), "Добавить сервер", "aria-label не «Добавить сервер»");
      assert.equal(plus.getAttribute("aria-haspopup"), "dialog", "у «+» нет aria-haspopup=dialog");
      assert.equal(plus.getAttribute("aria-expanded"), "false", "aria-expanded не false до открытия");

      plus.click();
      assert.equal(dialogOpen(app), true, "клик по «+» не открыл #join-server-dialog");
      assert.equal(plus.getAttribute("aria-expanded"), "true", "aria-expanded не true после открытия");

      const card = firstNode(app, "#join-server-dialog .dialog");
      assert.equal(card.getAttribute("role"), "dialog", "карточка не role=dialog");
      assert.equal(card.getAttribute("aria-modal"), "true", "карточка не aria-modal");
      assert.equal(card.getAttribute("data-state"), "idle", "начальное состояние не idle");
      assert.equal(firstNode(app, "#join-server-title").textContent, "Присоединиться к серверу");
      assert.equal(firstNode(app, "#join-invite-input").getAttribute("data-autofocus"), "true", "нет data-autofocus");
      assert.equal(app.document.activeElement, firstNode(app, "#join-invite-input"), "фокус не в поле ввода");
      assert.equal(all(app, "#join-preview").length, 1, "нет контейнера preview");
      assert.equal(firstNode(app, "#join-preview").getAttribute("hidden") !== null, true, "preview показан до ответа");
      assert.equal(api(app).join?.state, "idle", "join.state не idle");
    } finally {
      app.dispose();
    }
  });

  it("пустое поле: submit disabled; ввод включает; Enter = один resolve (SM-22, SM-39)", async () => {
    const app = await loadJoinApp((cmd) => (cmd === "resolve_invite" ? PREVIEW : undefined));
    try {
      openDialog(app);
      const submit = firstNode(app, "#join-submit");
      assert.notEqual(submit.getAttribute("disabled"), null, "submit активен при пустом поле");

      typeCode(app, "abc123");
      assert.equal(submit.getAttribute("disabled"), null, "submit не включился после ввода");

      const input = firstNode(app, "#join-invite-input");
      input.dispatchEvent("keydown", { key: "Enter" });
      await app.settle(3);
      assert.equal(app.callsOf("resolve_invite").length, 1, "Enter не отправил ровно один resolve_invite");
      assert.deepEqual(app.callsOf("resolve_invite")[0]?.args, { code: "abc123" }, "resolve_invite вызван не с {code}");
    } finally {
      app.dispose();
    }
  });

  it("неверный формат: aria-invalid, resolve не вызван (SM-24)", async () => {
    const app = await loadJoinApp(() => undefined);
    try {
      openDialog(app);
      typeCode(app, "abc 123");
      firstNode(app, "#join-submit").click();
      await app.settle(3);
      const input = firstNode(app, "#join-invite-input");
      assert.equal(input.getAttribute("aria-invalid"), "true", "поле не помечено aria-invalid");
      assert.equal(app.callsOf("resolve_invite").length, 0, "resolve_invite ушёл с мусорным кодом");
      const error = firstNode(app, "#join-error");
      assert.equal(error.getAttribute("hidden"), null, "ошибка формата не показана");
      assert.match(error.textContent, /Ссылка-приглашение выглядит неверно/);
    } finally {
      app.dispose();
    }
  });

  it("resolve 2xx → preview с name/счётчиками из ответа, accept ещё не вызван (SM-27)", async () => {
    const app = await loadJoinApp((cmd) => (cmd === "resolve_invite" ? PREVIEW : undefined));
    try {
      openDialog(app);
      typeCode(app, "https://discord.gg/abc123/");
      firstNode(app, "#join-submit").click();
      await app.settle(3);

      assert.equal(app.callsOf("resolve_invite").length, 1, "resolve_invite не один");
      assert.equal(app.callsOf("accept_invite").length, 0, "accept_invite вызван до подтверждения");
      const card = firstNode(app, "#join-server-dialog .dialog");
      assert.equal(card.getAttribute("data-state"), "preview_ready", "состояние не preview_ready");
      const preview = firstNode(app, "#join-preview");
      assert.equal(preview.getAttribute("hidden"), null, "preview скрыт после 2xx");
      assert.match(preview.textContent, /Новый сервер/, "в preview нет имени из ответа");
      assert.match(preview.textContent, /12[\s\u00a0]?345/, "в preview нет memberCount из ответа");
      assert.match(preview.textContent, /678/, "в preview нет presenceCount из ответа");
      // Иконка — хеш, не URL: CDN-адрес не склеиваем, картинок нет.
      assert.equal(preview.querySelectorAll("img").length, 0, "preview склеил URL иконки");
      assert.equal(preview.querySelectorAll("[src]").length, 0, "в preview появился src");
    } finally {
      app.dispose();
    }
  });

  it("один IPC на submit: двойной клик и 3 клика = 1 resolve; pending не закрывается Esc (SM-25, SM-26, SM-36)", async () => {
    /** @type {(value: any) => void} */
    let release = () => {};
    const pending = new Promise((resolve) => { release = resolve; });
    const app = await loadJoinApp((cmd) => (cmd === "resolve_invite" ? pending : undefined));
    try {
      openDialog(app);
      typeCode(app, "abc123");
      const submit = firstNode(app, "#join-submit");
      submit.click();
      submit.click();
      submit.click();
      await app.settle(2);

      assert.equal(app.callsOf("resolve_invite").length, 1, "повторные клики отправили второй IPC");
      const card = firstNode(app, "#join-server-dialog .dialog");
      assert.equal(card.getAttribute("data-state"), "resolving", "нет состояния resolving");
      assert.equal(firstNode(app, "#join-server-form").getAttribute("aria-busy"), "true", "форма не aria-busy");
      assert.match(firstNode(app, "#join-status").textContent, /Приглашение обрабатывается/);
      assert.notEqual(submit.getAttribute("disabled"), null, "submit не disabled в pending");

      app.document.body.dispatchEvent("keydown", { key: "Escape" });
      assert.equal(dialogOpen(app), true, "Esc закрыл диалог в pending");

      release(PREVIEW);
      await app.settle(3);
      assert.equal(dialogOpen(app), true, "диалог закрылся после preview");
      assert.equal(firstNode(app, "#join-server-form").getAttribute("aria-busy"), null, "aria-busy не снят");
    } finally {
      app.dispose();
    }
  });

  it("accept: только известный code, 2xx → рейл и канал обновлены, статус успеха (SM-29)", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") return PREVIEW;
      if (cmd === "accept_invite") return { guildId: "999", targetChannelId: "777" };
      return undefined;
    });
    try {
      openDialog(app);
      typeCode(app, "abc123");
      firstNode(app, "#join-submit").click();
      await app.settle(3);
      firstNode(app, "#join-submit").click();
      await app.settle(3);

      assert.equal(app.callsOf("accept_invite").length, 1, "accept_invite не один");
      assert.deepEqual(
        app.callsOf("accept_invite")[0]?.args,
        { code: "abc123" },
        "accept_invite получил не только известный code (nullable-поля не подставляем)",
      );
      // Bearer живёт только в backend: в аргументах IPC нет токена.
      const wire = JSON.stringify(app.invokeCalls);
      assert.equal(/bearer|access_token|"token"|session_id|invitation_id/i.test(wire), false, "в IPC утёк токен/неизвестные поля");

      assert.equal(dialogOpen(app), false, "диалог не закрылся после 2xx");
      assert.match(firstNode(app, "#server-status").textContent, /Вы присоединились к серверу «Новый сервер»!/);

      const tile = all(app, ".guild-item").find((node) => node.getAttribute("data-guild-id") === "999");
      assert.ok(tile, "после accept в рейле нет плитки новой гильдии");
      assert.equal(tile.getAttribute("aria-current"), "true", "новая плитка не выбрана");
      assert.equal(
        all(app, ".guild-item").filter((node) => node.getAttribute("aria-current") === "true").length,
        1,
        "выбранных плиток не одна",
      );
      const channels = app.callsOf("channels").filter((call) => call.args?.guildId === "999");
      assert.ok(channels.length >= 1, "канал не обновлён: channels({guildId:999}) не вызван");
      assert.equal(app.callsOf("resolve_invite").length, 1, "accept повторил resolve");
    } finally {
      app.dispose();
    }
  });

  it("resolve 404: видимая ошибка, preview скрыт, код сохранён, accept не вызван (SM-28)", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") throw { kind: "notFound" };
      return undefined;
    });
    try {
      openDialog(app);
      typeCode(app, "badinvite");
      firstNode(app, "#join-submit").click();
      await app.settle(3);

      const card = firstNode(app, "#join-server-dialog .dialog");
      assert.equal(card.getAttribute("data-state"), "error", "состояние не error");
      const error = firstNode(app, "#join-error");
      assert.equal(error.getAttribute("hidden"), null, "ошибка 404 не показана");
      assert.match(error.textContent, /Приглашение недействительно/);
      assert.match(error.textContent, /Приглашение содержит ошибку, или истёк срок его действия\./);
      assert.equal(firstNode(app, "#join-preview").getAttribute("hidden") !== null, true, "preview показан при ошибке (фальшивые данные)");
      assert.equal(/** @type {any} */ (firstNode(app, "#join-invite-input")).value, "badinvite", "код не сохранён");
      assert.equal(app.callsOf("accept_invite").length, 0, "accept вызван после ошибки resolve");
      assert.equal(firstNode(app, "#join-submit").getAttribute("disabled"), null, "нет возможности повторить resolve");
    } finally {
      app.dispose();
    }
  });

  it("accept 403: «Недостаточно прав», без success-текста и без новой плитки (SM-30/31)", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") return PREVIEW;
      if (cmd === "accept_invite") throw { kind: "forbidden" };
      return undefined;
    });
    try {
      openDialog(app);
      typeCode(app, "abc123");
      firstNode(app, "#join-submit").click();
      await app.settle(3);
      firstNode(app, "#join-submit").click();
      await app.settle(3);
      // Третий клик — повтор только упавшего шага (accept), не resolve.
      firstNode(app, "#join-submit").click();
      await app.settle(3);

      const error = firstNode(app, "#join-error");
      assert.match(error.textContent, /Недостаточно прав, чтобы присоединиться к этому серверу\./);
      assert.equal(dialogOpen(app), true, "диалог закрылся при 403");
      assert.equal(firstNode(app, "#server-status").textContent, "", "успех объявлен при 403");
      assert.equal(
        all(app, ".guild-item").some((node) => node.getAttribute("data-guild-id") === "999"),
        false,
        "при 403 добавлена плитка гильдии",
      );

      // Повтор повторяет только упавший шаг: 1 resolve + 2 accept.
      assert.equal(app.callsOf("resolve_invite").length, 1, "повтор повторил resolve");
      assert.equal(app.callsOf("accept_invite").length, 2, "повтор не отправил accept");
    } finally {
      app.dispose();
    }
  });

  it("401: видимая ошибка сессии, диалог открыт, успеха нет", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") return PREVIEW;
      if (cmd === "accept_invite") throw { kind: "unauthorized" };
      return undefined;
    });
    try {
      openDialog(app);
      typeCode(app, "abc123");
      firstNode(app, "#join-submit").click();
      await app.settle(3);
      firstNode(app, "#join-submit").click();
      await app.settle(3);

      const error = firstNode(app, "#join-error");
      assert.equal(error.getAttribute("hidden"), null, "401 не показан");
      assert.match(error.textContent, /Сессия истекла\. Войдите снова\./);
      assert.equal(firstNode(app, "#server-status").textContent, "", "успех объявлен при 401");
      assert.equal(
        all(app, ".guild-item").some((node) => node.getAttribute("data-guild-id") === "999"),
        false,
        "при 401 добавлена плитка гильдии",
      );
    } finally {
      app.dispose();
    }
  });

  it("429: data-retry-after, submit disabled, затем снова enabled (SM-32)", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") throw { kind: "rateLimited", retry_after_ms: 5000 };
      return undefined;
    });
    try {
      openDialog(app);
      typeCode(app, "abc123");
      firstNode(app, "#join-submit").click();
      await app.settle(3);

      const error = firstNode(app, "#join-error");
      assert.match(error.textContent, /Слишком много попыток\. Повторите через 5 с\./);
      const submit = firstNode(app, "#join-submit");
      assert.notEqual(submit.getAttribute("disabled"), null, "submit не disabled во время cooldown 429");
      assert.equal(submit.getAttribute("data-retry-after"), "5", "нет data-retry-after=5");

      app.flushTimers();
      await app.settle(2);
      assert.equal(submit.getAttribute("disabled"), null, "submit не включился после cooldown");
      assert.equal(submit.getAttribute("data-retry-after"), null, "data-retry-after не снят");
    } finally {
      app.dispose();
    }
  });

  it("accept 2xx без guildId: ошибка, рейл не мутируется (нет фальшивого успеха)", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") return PREVIEW;
      if (cmd === "accept_invite") return { targetChannelId: "777" };
      return undefined;
    });
    try {
      openDialog(app);
      typeCode(app, "abc123");
      firstNode(app, "#join-submit").click();
      await app.settle(3);
      firstNode(app, "#join-submit").click();
      await app.settle(3);

      assert.equal(firstNode(app, "#join-server-dialog .dialog").getAttribute("data-state"), "error", "нет error при 2xx без guildId");
      assert.match(firstNode(app, "#join-error").textContent, /Невозможно присоединиться к серверу/);
      assert.equal(firstNode(app, "#server-status").textContent, "", "успех объявлен без guildId");
      assert.equal(
        all(app, ".guild-item").some((node) => node.getAttribute("data-guild-id") === "999"),
        false,
        "плитка добавлена без guildId",
      );
    } finally {
      app.dispose();
    }
  });

  it("CARD-TEL-006: Captcha Required (код 40003) отображает понятное объяснение вместо неизвестной ошибки", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") return PREVIEW;
      if (cmd === "accept_invite") throw { kind: "captchaRequired", code: 40003 };
      return undefined;
    });
    try {
      openDialog(app);
      typeCode(app, "abc123");
      firstNode(app, "#join-submit").click();
      await app.settle(3);
      firstNode(app, "#join-submit").click();
      await app.settle(3);

      assert.equal(firstNode(app, "#join-server-dialog .dialog").getAttribute("data-state"), "error");
      assert.match(
        firstNode(app, "#join-error").textContent,
        /Discord запросил проверку Captcha при вступлении на сервер \(код 40003\)/,
      );
    } finally {
      app.dispose();
    }
  });

  it("CARD-TEL-006: Client 400 с code 40003 распознаётся как Captcha Required", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") return PREVIEW;
      if (cmd === "accept_invite") throw { kind: "client", status: 400, code: 40003 };
      return undefined;
    });
    try {
      openDialog(app);
      typeCode(app, "abc123");
      firstNode(app, "#join-submit").click();
      await app.settle(3);
      firstNode(app, "#join-submit").click();
      await app.settle(3);

      assert.equal(firstNode(app, "#join-server-dialog .dialog").getAttribute("data-state"), "error");
      assert.match(
        firstNode(app, "#join-error").textContent,
        /Discord запросил проверку Captcha при вступлении на сервер \(код 40003\)/,
      );
    } finally {
      app.dispose();
    }
  });

  it("«Отмена»/Esc: закрытие, фокус на «+», форма сброшена (SM-38)", async () => {
    const app = await loadJoinApp(() => undefined);
    try {
      const plus = addTile(app);
      plus.click();
      typeCode(app, "abc123");
      openDialog(app); // повторное открытие не должно ломать состояние

      firstNode(app, '[data-action="join-cancel"]').click();
      assert.equal(dialogOpen(app), false, "«Отмена» не закрыла диалог");
      assert.equal(plus.getAttribute("aria-expanded"), "false", "aria-expanded не false после закрытия");
      assert.equal(app.document.activeElement, plus, "фокус не вернулся на «+»");
      assert.equal(/** @type {any} */ (firstNode(app, "#join-invite-input")).value, "", "поле не сброшено");

      plus.click();
      assert.equal(dialogOpen(app), true, "повторное открытие не работает");
      app.document.body.dispatchEvent("keydown", { key: "Escape" });
      assert.equal(dialogOpen(app), false, "Esc не закрыл диалог");
      assert.equal(app.document.activeElement, plus, "после Esc фокус не на «+»");
    } finally {
      app.dispose();
    }
  });

  it("STUB-03 / STUB-05: ссылка 'Создать сервер' переключает на создание сервера", async () => {
    const app = await loadJoinApp(() => undefined);
    try {
      openDialog(app);
      const switchBtn = firstNode(app, ".switch-to-create-btn");
      assert.ok(switchBtn, "кнопка переключения на создание сервера найдена");
      assert.equal(dialogOpen(app), true, "диалог инвайтов открыт");

      switchBtn.click();
      await app.settle(2);

      assert.equal(dialogOpen(app), false, "модалка инвайтов закрыта после перехода");
      const createDialog = firstNode(app, "#create-guild-dialog");
      assert.ok(createDialog, "модалка создания сервера смонтирована");
      assert.equal(createDialog.getAttribute("hidden"), null, "модалка создания сервера открыта");
    } finally {
      app.dispose();
    }
  });

  it("нет новых команд и токена в коде: только resolve_invite/accept_invite", () => {
    assert.equal(/\bfetch\s*\(|localStorage|sessionStorage/.test(source), false, "UI ходит в сеть/хранилище мимо IPC");
    assert.match(source, /resolve_invite/, "нет команды resolve_invite");
    assert.match(source, /accept_invite/, "нет команды accept_invite");
    // Модуль не знает про тела/поля accept (`session_id` и т.п.) — их не подставляет.
    assert.equal(/session_id\s*:|invite_instance_id\s*:|received_installation_id\s*:/.test(source), false, "UI подставляет nullable-поля accept");
  });
});

describe("G-03a-UI: «Открыть сервер» в preview (lurker, D-76)", () => {
  it("кнопка появляется только с preview и шлёт ровно {guildId}", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") return PREVIEW;
      if (cmd === "join_guild") return null;
      return undefined;
    });
    try {
      openDialog(app);
      typeCode(app, "abc123");
      // До ответа resolve данных нет — кнопки тоже нет (guildId неизвестен).
      assert.equal(all(app, '[data-action="enter-guild"]').length, 0, "кнопка enter-guild есть до preview");
      firstNode(app, "#join-submit").click();
      await app.settle(3);

      const button = enterButton(app);
      assert.equal(button.textContent, "Открыть сервер", "текст кнопки не «Открыть сервер»");
      assert.equal(app.callsOf("join_guild").length, 0, "join_guild вызван без клика");
      assert.equal(app.callsOf("accept_invite").length, 0, "accept_invite вызван на этапе preview");

      button.click();
      await app.settle(3);
      assert.equal(app.callsOf("join_guild").length, 1, "клик не отправил ровно один join_guild");
      assert.deepEqual(
        app.callsOf("join_guild")[0]?.args,
        { guildId: "999" },
        "join_guild получил не только guildId из preview",
      );
      // Один клик — один IPC: в аргументах нет токена/полей accept.
      const wire = JSON.stringify(app.callsOf("join_guild"));
      assert.equal(/bearer|access_token|"token"|session_id|invite_instance_id|received_installation_id/i.test(wire), false, "в join_guild утёк токен/поля accept");
      assert.equal(app.callsOf("accept_invite").length, 0, "«Открыть сервер» смешан с accept_invite");
    } finally {
      app.dispose();
    }
  });

  it("204 → entered и статус «Сервер открыт для просмотра», без плитки и членства", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") return PREVIEW;
      if (cmd === "join_guild") return null;
      return undefined;
    });
    try {
      await reachPreview(app);
      enterButton(app).click();
      await app.settle(3);

      assert.equal(app.callsOf("join_guild").length, 1, "join_guild не один");
      assert.equal(firstNode(app, "#join-server-dialog .dialog").getAttribute("data-state"), "entered", "состояние не entered");
      assert.equal(firstNode(app, "#server-status").textContent, "Сервер открыт для просмотра", "нет статуса lurker-входа");
      assert.equal(firstNode(app, "#server-status").getAttribute("role"), "status", "#server-status не role=status");

      // Lurker ≠ membership: ни плитки, ни обновления каналов, ни accept.
      assert.equal(
        all(app, ".guild-item").some((node) => node.getAttribute("data-guild-id") === "999"),
        false,
        "после 204 в рейле появилась плитка гильдии",
      );
      assert.equal(
        app.callsOf("channels").some((call) => call.args?.guildId === "999"),
        false,
        "после 204 UI запросил каналы как при членстве",
      );
      assert.equal(app.callsOf("accept_invite").length, 0, "204 смешан с accept_invite");
      assert.notEqual(enterButton(app).getAttribute("disabled"), null, "кнопка enter-guild не заблокирована после 204");
    } finally {
      app.dispose();
    }
  });

  it("один IPC на клик: тройной клик в entering = 1 join_guild, после entered повтор игнорируется", async () => {
    /** @type {(value: any) => void} */
    let release = () => {};
    const flight = new Promise((resolve) => { release = resolve; });
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") return PREVIEW;
      if (cmd === "join_guild") return flight;
      return undefined;
    });
    try {
      await reachPreview(app);
      const button = enterButton(app);
      button.click();
      button.click();
      button.click();
      await app.settle(2);

      assert.equal(app.callsOf("join_guild").length, 1, "повторные клики отправили второй join_guild");
      assert.equal(firstNode(app, "#join-server-dialog .dialog").getAttribute("data-state"), "entering", "нет состояния entering");
      assert.equal(button.getAttribute("aria-busy"), "true", "кнопка не aria-busy в entering");
      assert.notEqual(button.getAttribute("disabled"), null, "кнопка не disabled в entering");
      assert.equal(firstNode(app, "#join-server-form").getAttribute("aria-busy"), "true", "форма не aria-busy в entering");
      assert.equal(app.callsOf("accept_invite").length, 0, "accept_invite отправлен во время entering");

      release(null);
      await app.settle(3);
      assert.equal(firstNode(app, "#join-server-dialog .dialog").getAttribute("data-state"), "entered", "нет entered после 204");
      assert.equal(button.getAttribute("aria-busy"), null, "aria-busy не снят после 204");

      button.click();
      await app.settle(2);
      assert.equal(app.callsOf("join_guild").length, 1, "повторный клик после entered отправил ещё один join_guild");
    } finally {
      app.dispose();
    }
  });

  it("403: видимая ошибка «Нет доступа», повтор — только упавший join_guild", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") return PREVIEW;
      if (cmd === "join_guild") throw { kind: "forbidden" };
      return undefined;
    });
    try {
      await reachPreview(app);
      enterButton(app).click();
      await app.settle(3);

      const error = firstNode(app, "#join-error");
      assert.equal(error.getAttribute("hidden"), null, "403 не показан");
      assert.match(error.textContent, /Нет доступа/, "текст 403 не про доступ");
      assert.equal(firstNode(app, "#join-server-dialog .dialog").getAttribute("data-state"), "error", "состояние не error");
      assert.equal(firstNode(app, "#join-preview").getAttribute("hidden"), null, "preview скрыт после ошибки enter");
      assert.equal(firstNode(app, "#server-status").textContent, "", "успех объявлен при 403");
      assert.equal(app.callsOf("accept_invite").length, 0, "403 смешан с accept_invite");
      assert.equal(
        all(app, ".guild-item").some((node) => node.getAttribute("data-guild-id") === "999"),
        false,
        "при 403 добавлена плитка гильдии",
      );

      // Повтор — только упавший шаг: 1 resolve + 2 join_guild, accept не тронут.
      enterButton(app).click();
      await app.settle(3);
      assert.equal(app.callsOf("join_guild").length, 2, "повтор не отправил join_guild");
      assert.equal(app.callsOf("resolve_invite").length, 1, "повтор повторил resolve_invite");
      assert.equal(app.callsOf("accept_invite").length, 0, "повтор отправил accept_invite");
    } finally {
      app.dispose();
    }
  });

  it("429: data-retry-after на кнопке, затем снова один клик = один join_guild", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") return PREVIEW;
      if (cmd === "join_guild") throw { kind: "rateLimited", retry_after_ms: 5000 };
      return undefined;
    });
    try {
      await reachPreview(app);
      const button = enterButton(app);
      button.click();
      await app.settle(3);

      assert.match(firstNode(app, "#join-error").textContent, /Слишком много попыток\. Повторите через 5 с\./);
      assert.notEqual(button.getAttribute("disabled"), null, "кнопка не disabled во время cooldown 429");
      assert.equal(button.getAttribute("data-retry-after"), "5", "нет data-retry-after=5");

      app.flushTimers();
      await app.settle(2);
      assert.equal(button.getAttribute("disabled"), null, "кнопка не включилась после cooldown");
      assert.equal(button.getAttribute("data-retry-after"), null, "data-retry-after не снят");

      button.click();
      await app.settle(3);
      assert.equal(app.callsOf("join_guild").length, 2, "после cooldown клик не отправил ровно один join_guild");
      assert.equal(app.callsOf("accept_invite").length, 0, "429 смешан с accept_invite");
    } finally {
      app.dispose();
    }
  });

  it("401 и сеть: видимая ошибка, без успеха и без плитки", async () => {
    for (const [kind, pattern] of /** @type {const} */ ([
      ["unauthorized", /Сессия истекла\. Войдите снова\./],
      ["network", /Сетевое подключение ограничено или недоступно\./],
    ])) {
      const app = await loadJoinApp((cmd) => {
        if (cmd === "resolve_invite") return PREVIEW;
        if (cmd === "join_guild") throw { kind };
        return undefined;
      });
      try {
        await reachPreview(app);
        enterButton(app).click();
        await app.settle(3);

        const error = firstNode(app, "#join-error");
        assert.equal(error.getAttribute("hidden"), null, `${kind}: ошибка не показана`);
        assert.match(error.textContent, pattern, `${kind}: неверный текст ошибки`);
        assert.equal(firstNode(app, "#server-status").textContent, "", `${kind}: успех объявлен при ошибке`);
        assert.equal(
          all(app, ".guild-item").some((node) => node.getAttribute("data-guild-id") === "999"),
          false,
          `${kind}: добавлена плитка гильдии`,
        );
        assert.equal(app.callsOf("accept_invite").length, 0, `${kind}: ошибка смешана с accept_invite`);
      } finally {
        app.dispose();
      }
    }
  });

  it("потоки разделены: после 204 accept работает отдельно и только он добавляет плитку", async () => {
    const app = await loadJoinApp((cmd) => {
      if (cmd === "resolve_invite") return PREVIEW;
      if (cmd === "join_guild") return null;
      if (cmd === "accept_invite") return { guildId: "999" };
      return undefined;
    });
    try {
      await reachPreview(app);
      enterButton(app).click();
      await app.settle(3);
      assert.equal(
        all(app, ".guild-item").some((node) => node.getAttribute("data-guild-id") === "999"),
        false,
        "204 добавил плитку (lurker ≠ membership)",
      );

      firstNode(app, "#join-submit").click();
      await app.settle(3);
      assert.equal(app.callsOf("accept_invite").length, 1, "accept после enter не отправлен");
      assert.equal(app.callsOf("join_guild").length, 1, "accept повторил join_guild");
      assert.equal(dialogOpen(app), false, "accept не закрыл диалог");
      assert.ok(
        all(app, ".guild-item").some((node) => node.getAttribute("data-guild-id") === "999"),
        "accept не добавил плитку гильдии",
      );
    } finally {
      app.dispose();
    }
  });

  it("в исходнике нет сети/токена: команда join_guild, аргумент — только guildId", async () => {
    const { JOIN_COMMANDS, JOIN_TEXT } = await import("../../rust/gui-spike/ui/features/join-server.js");
    assert.equal(JOIN_COMMANDS.enter, "join_guild", "нет команды join_guild");
    assert.equal(JOIN_TEXT.open, "Открыть сервер", "нет текста «Открыть сервер»");
    assert.match(source, /join_guild/, "исходник не вызывает join_guild");
    assert.equal(/\bfetch\s*\(/.test(source), false, "UI ходит в сеть мимо IPC");
    // Никаких выдуманных полей enter: тело запроса — только guildId.
    assert.equal(/lurker\s*:|body\s*:/.test(source), false, "UI подставляет тело/поля lurker-запроса");
  });
});
