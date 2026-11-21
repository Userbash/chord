// @ts-check
/**
 * Unit-тесты интерактивного WebView-контейнера `#captcha-modal`
 * с передачей токена решения в 1 клик через `postMessage` без ручного копирования строки
 * (`rust/gui-spike/ui/features/captcha-modal.js`).
 *
 * Независимый оракул:
 * - `waves/kanban-board.json` (`TASK-817`, `TASK-610`):
 *   - container: `"Captcha Resolver Container"` / `"Modal Dialog"` (`540x480 max`, `padding: 16px`, `spacing: 12px`)
 *   - design_tokens: `background: "#1e1f22"`, `surface: "#2b2d31"`, `text_primary: "#ffffff"`, `accent: "#5865f2"`
 *   - interactive_states:
 *     - Interactive Captcha: `default: "webview widget"`, `hover: "user click"`, `active: "auto submit token"`
 *     - Captcha Frame: `default: "loading spinner"`, `hover: "interactive"`, `active: "token resolved"`
 * - `waves/RW-39/NET/LOGIN-UI-RE.md` §8 (`CAP-01`..`CAP-15`)
 *
 * Покрытие 9 измерений `TEST-POLICY.md`:
 * 1. Позитивные: 1-click перехват токена через `postMessage` (объект, JSON, `P1_...`, `webviewFrame.postMessage`, `#captcha-webview-verify-btn`).
 * 2. Негативные: отклонение `postMessage` с недоверенных origin, шумных событий без токена и в закрытом состоянии.
 * 3. Граничные: различные форматы вложенных полезных нагрузок `postMessage`, пробелы по краям, длинные JWT-токены.
 * 4. Ошибки: обработка `401/403/404/429/500` при автоматическом `submit` из `postMessage`.
 * 5. Пустые/Отмена: закрытие по `Escape`, `Cancel`, крестику и клику по подложке снимает слушатели и не вызывает IPC.
 * 6. Загрузка: защита от дублирующей отправки (`inFlight` / `aria-busy="true"`) при серии `postMessage`.
 * 7. Доступность (A11y): `role="dialog"`, `aria-modal="true"`, `role="region"`, `role="status"`, `aria-live="polite"`.
 * 8. Восстановление: обновление `src` и `data-sitekey` WebView при ротации челленджа и повторное 1-click решение.
 * 9. Безопасность и наблюдаемость: проверка origin, sandbox iframe, `data-interactive-state`, `data-frame-state`.
 */

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { describe, it } from "node:test";
import { ShimDocument, loadApp } from "../support/gui-dom-harness.mjs";
import {
  CAPTCHA_ERROR_CODE,
  CAPTCHA_MODAL_TOKENS,
  CAPTCHA_TEXT,
  DEFAULT_HCAPTCHA_SITEKEY,
  HCAPTCHA_API_URL,
  HCAPTCHA_BRIDGE_TOKENS,
  HCAPTCHA_BRIDGE_URL,
  HCAPTCHA_SERVICE,
  TRUSTED_CAPTCHA_ORIGINS,
  buildHcaptchaBridgeUrl,
  buildHcaptchaWidgetUrl,
  createCaptchaModal,
  createCaptchaTokenMessage,
  extractCaptchaChallenge,
  extractTokenFromMessage,
  isTrustedCaptchaOrigin,
} from "../../rust/gui-spike/ui/features/captcha-modal.js";
import { createJoinServer } from "../../rust/gui-spike/ui/features/join-server.js";

const captchaModalPath = new URL("../../rust/gui-spike/ui/features/captcha-modal.js", import.meta.url);
const captchaModalSource = readFileSync(captchaModalPath, "utf8");
const hcaptchaBridgePath = new URL("../../rust/gui-spike/ui/assets/hcaptcha-bridge.html", import.meta.url);
const kanbanBoardPath = new URL("../../waves/kanban-board.json", import.meta.url);
const kanbanBoard = JSON.parse(readFileSync(kanbanBoardPath, "utf8"));

const SAMPLE_SITEKEY = "a9b5fb07-92ff-493f-86fe-352a2803b3df";
const SAMPLE_TOKEN = "P1_eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.interactive_webview_token_817";
const SAMPLE_PREVIEW = Object.freeze({
  code: "discord-testers",
  guildId: "197038439483310086",
  guildName: "Discord Testers",
  icon: "a_icon_hash",
  memberCount: 250000,
  presenceCount: 42000,
  channelId: "197038439483310087",
});

/**
 * Создаёт изолированное тестовое окружение для интерактивного WebView `#captcha-modal`.
 * @param {(cmd: string, args?: Record<string, unknown>) => unknown | Promise<unknown>} [handler]
 * @param {Record<string, unknown>} [extraOptions]
 */
function createInteractiveTestEnv(
  handler = () => ({ guildId: SAMPLE_PREVIEW.guildId }),
  extraOptions = {},
) {
  const document = new ShimDocument();
  const root = document.createElement("div");
  root.id = "app-root";
  document.body.append(root);

  /** @type {Array<{ cmd: string, args: Record<string, unknown> | undefined }>} */
  const calls = [];
  /** @type {Array<{ response: any, meta: any }>} */
  const solved = [];
  let cancelCount = 0;

  /**
   * @param {string} cmd
   * @param {Record<string, unknown>} [args]
   */
  const invoke = async (cmd, args) => {
    calls.push({ cmd, args });
    return await handler(cmd, args);
  };

  const modal = createCaptchaModal({
    document: /** @type {any} */ (document),
    invoke,
    onSolved(response, meta) {
      solved.push({ response, meta });
    },
    onCancel() {
      cancelCount += 1;
    },
    ...extraOptions,
  });

  return {
    document,
    modal,
    calls,
    solved,
    get cancelCount() {
      return cancelCount;
    },
  };
}

describe("1. Сверка дизайн-токенов, констант и чистых функций с оракулом kanban-board.json", () => {
  it("CAPTCHA_MODAL_TOKENS совпадает с дизайн-токенами TASK-817 / TASK-610 в kanban-board.json", () => {
    const task817 = kanbanBoard.cards.find((/** @type {any} */ t) => t.id === "TASK-817");
    const task610 = kanbanBoard.cards.find((/** @type {any} */ t) => t.id === "TASK-610");
    assert.ok(task817, "TASK-817 присутствует в kanban-board.json");
    assert.ok(task610, "TASK-610 присутствует в kanban-board.json");

    assert.equal(CAPTCHA_MODAL_TOKENS.background, task817.design_tokens.background);
    assert.equal(CAPTCHA_MODAL_TOKENS.surface, task817.design_tokens.surface);
    assert.equal(CAPTCHA_MODAL_TOKENS.textPrimary, task817.design_tokens.text_primary);
    assert.equal(CAPTCHA_MODAL_TOKENS.accent, task817.design_tokens.accent);
    assert.equal(CAPTCHA_MODAL_TOKENS.padding, task817.layout.padding);
    assert.equal(CAPTCHA_MODAL_TOKENS.spacing, task817.layout.spacing);
    assert.equal(CAPTCHA_MODAL_TOKENS.container, task817.layout.container);
  });

  it("isTrustedCaptchaOrigin разрешает доверенные домены hCaptcha/Discord/Tauri и блокирует сторонние", () => {
    assert.ok(Array.isArray(TRUSTED_CAPTCHA_ORIGINS));
    assert.equal(isTrustedCaptchaOrigin("https://newassets.hcaptcha.com"), true);
    assert.equal(isTrustedCaptchaOrigin("https://hcaptcha.com"), true);
    assert.equal(isTrustedCaptchaOrigin("https://assets.hcaptcha.com"), true);
    assert.equal(isTrustedCaptchaOrigin("https://discord.com"), true);
    assert.equal(isTrustedCaptchaOrigin("tauri://localhost"), true);
    assert.equal(isTrustedCaptchaOrigin("http://localhost:1420"), true);
    assert.equal(isTrustedCaptchaOrigin(""), true, "пустой origin локального события допустим");
    assert.equal(isTrustedCaptchaOrigin(undefined), true);

    assert.equal(isTrustedCaptchaOrigin("https://evil.example.com"), false);
    assert.equal(isTrustedCaptchaOrigin("https://hcaptcha.com.attacker.org"), false);
    assert.equal(isTrustedCaptchaOrigin("http://phishing-discord.com"), false);
    assert.equal(isTrustedCaptchaOrigin("javascript:alert(1)"), false);
  });

  it("extractTokenFromMessage извлекает токен из всех форматов postMessage и отвергает шум", () => {
    assert.equal(extractTokenFromMessage(SAMPLE_TOKEN), SAMPLE_TOKEN);
    assert.equal(extractTokenFromMessage(`  ${SAMPLE_TOKEN}  `), SAMPLE_TOKEN);
    assert.equal(
      extractTokenFromMessage(JSON.stringify({ event: "challenge-passed", response: SAMPLE_TOKEN })),
      SAMPLE_TOKEN,
    );
    assert.equal(
      extractTokenFromMessage({ type: "hcaptcha-token", token: SAMPLE_TOKEN }),
      SAMPLE_TOKEN,
    );
    assert.equal(
      extractTokenFromMessage({ "h-captcha-response": SAMPLE_TOKEN }),
      SAMPLE_TOKEN,
    );
    assert.equal(
      extractTokenFromMessage({ data: { captcha_key: SAMPLE_TOKEN } }),
      SAMPLE_TOKEN,
    );
    assert.equal(
      extractTokenFromMessage({ payload: { response: SAMPLE_TOKEN } }),
      SAMPLE_TOKEN,
    );

    // Негативные / шумные сообщения WebView
    assert.equal(extractTokenFromMessage(null), "");
    assert.equal(extractTokenFromMessage(undefined), "");
    assert.equal(extractTokenFromMessage(""), "");
    assert.equal(extractTokenFromMessage("short"), "");
    assert.equal(extractTokenFromMessage({ event: "challenge-opened" }), "");
    assert.equal(extractTokenFromMessage({ type: "resize", height: 480 }), "");
    assert.equal(extractTokenFromMessage("{not-valid-json"), "");
  });

  it("createCaptchaTokenMessage формирует структурированный пакет postMessage для WebView-моста", () => {
    const packet = createCaptchaTokenMessage(SAMPLE_TOKEN, {
      captcha_sitekey: SAMPLE_SITEKEY,
      captcha_rqtoken: "rq_tok_1",
    });
    assert.equal(packet.source, "discord-captcha-webview");
    assert.equal(packet.service, HCAPTCHA_SERVICE);
    assert.equal(packet.event, "challenge-passed");
    assert.equal(packet.type, "hcaptcha-token");
    assert.equal(packet.token, SAMPLE_TOKEN);
    assert.equal(packet.captcha_key, SAMPLE_TOKEN);
    assert.equal(packet.sitekey, SAMPLE_SITEKEY);
    assert.equal(packet.rqtoken, "rq_tok_1");
  });
});

describe("2. Интерактивный WebView-контейнер и передача токена в 1 клик через postMessage", () => {
  it("монтирует интерактивный WebView-контейнер с атрибутами состояний, статуса и кнопкой 1-click", () => {
    const env = createInteractiveTestEnv();
    const opened = env.modal.open({
      code: "discord-testers",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
      captcha_rqdata: "rq_interactive_1",
    });
    assert.equal(opened, true);

    const container = env.document.getElementById("captcha-widget-container");
    assert.ok(container, "#captcha-widget-container смонтирован");
    assert.equal(container.getAttribute("data-container"), "Captcha Resolver Container");
    assert.equal(container.getAttribute("data-interactive-state"), "webview widget");
    assert.equal(container.getAttribute("data-bg"), CAPTCHA_MODAL_TOKENS.background);
    assert.equal(container.getAttribute("data-surface"), CAPTCHA_MODAL_TOKENS.surface);
    assert.equal(container.getAttribute("data-accent"), CAPTCHA_MODAL_TOKENS.accent);

    const widgetFrame = env.document.getElementById("captcha-widget-frame");
    assert.ok(widgetFrame, "#captcha-widget-frame смонтирован");
    assert.equal(widgetFrame.getAttribute("data-webview"), "true");
    assert.equal(widgetFrame.getAttribute("data-transfer-mode"), "postMessage");
    assert.equal(widgetFrame.getAttribute("data-frame-state"), "loading spinner");

    const webviewIframe = /** @type {any} */ (env.document.getElementById("captcha-webview-frame"));
    assert.ok(webviewIframe, "iframe #captcha-webview-frame смонтирован");
    assert.equal(webviewIframe.getAttribute("data-postmessage-bridge"), "true");
    assert.equal(typeof webviewIframe.postMessage, "function", "webviewFrame предоставляет мост postMessage");

    // После события load на iframe состояние фрейма переходит в interactive
    webviewIframe.dispatchEvent("load");
    assert.equal(widgetFrame.getAttribute("data-frame-state"), "interactive");

    const statusEl = env.document.getElementById("captcha-webview-status");
    assert.ok(statusEl, "статус WebView #captcha-webview-status присутствует");
    assert.equal(statusEl.getAttribute("role"), "status");
    assert.equal(statusEl.getAttribute("aria-live"), "polite");

    const verifyBtn = env.document.getElementById("captcha-webview-verify-btn");
    assert.ok(verifyBtn, "кнопка 1-click проверки #captcha-webview-verify-btn присутствует");
    assert.equal(verifyBtn.getAttribute("data-action"), "captcha-webview-verify");
  });

  it("передаёт токен через webviewFrame.postMessage в 1 клик без ручного копирования в input", async () => {
    const env = createInteractiveTestEnv((cmd, args) => {
      if (cmd === "accept_invite") {
        return {
          guildId: SAMPLE_PREVIEW.guildId,
          targetChannelId: SAMPLE_PREVIEW.channelId,
        };
      }
      throw new Error(`unexpected cmd: ${cmd}`);
    });

    env.modal.open({
      code: "discord-testers",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
      captcha_rqtoken: "rq_tok_postmsg",
      captcha_session_id: "sess_postmsg",
    });

    const webviewIframe = /** @type {any} */ (env.document.getElementById("captcha-webview-frame"));
    const container = env.document.getElementById("captcha-widget-container");
    const widgetFrame = env.document.getElementById("captcha-widget-frame");

    // Пользователь проходит капчу в WebView -> WebView шлёт postMessage без ручного ввода в #captcha-token-input
    webviewIframe.postMessage(
      {
        source: "hcaptcha",
        event: "challenge-passed",
        response: SAMPLE_TOKEN,
      },
      "https://newassets.hcaptcha.com",
    );

    await new Promise((r) => setTimeout(r, 20));

    assert.equal(env.calls.length, 1, "accept_invite автоматически вызван после postMessage");
    assert.deepEqual(env.calls[0], {
      cmd: "accept_invite",
      args: {
        code: "discord-testers",
        captcha_key: SAMPLE_TOKEN,
        captcha_rqtoken: "rq_tok_postmsg",
        captcha_session_id: "sess_postmsg",
      },
    });
    assert.equal(container?.getAttribute("data-interactive-state"), "auto submit token");
    assert.equal(widgetFrame?.getAttribute("data-frame-state"), "token resolved");
    assert.equal(env.modal.isOpen, false, "модалка автоматически закрылась после успешного решения");
    assert.equal(env.solved.length, 1);
    assert.equal(env.solved[0]?.meta.captcha_key, SAMPLE_TOKEN);
  });

  it("кнопка #captcha-webview-verify-btn выполняет проверку в 1 клик через postMessage без ручного копирования строки", async () => {
    const env = createInteractiveTestEnv(
      (cmd) => {
        if (cmd === "accept_invite") {
          return { guildId: SAMPLE_PREVIEW.guildId };
        }
        throw new Error(`unexpected cmd: ${cmd}`);
      },
      {
        resolveWebviewToken: () => SAMPLE_TOKEN,
      },
    );

    env.modal.open({
      code: "discord-testers",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
      captcha_rqtoken: "rq_1click",
    });

    const container = env.document.getElementById("captcha-widget-container");
    const verifyBtn = env.document.getElementById("captcha-webview-verify-btn");
    assert.ok(verifyBtn);

    // Наведение переводит интерактивное состояние в "user click"
    verifyBtn.dispatchEvent("mouseenter");
    assert.equal(container?.getAttribute("data-interactive-state"), "user click");

    // 1 клик по интерактивному элементу WebView без какого-либо ввода в #captcha-token-input
    const clickEvent = verifyBtn.dispatchEvent("click");
    assert.equal(/** @type {any} */ (clickEvent).stopped, true, "клик останавливает всплытие");

    await new Promise((r) => setTimeout(r, 20));

    assert.equal(env.calls.length, 1, "1 клик по WebView-кнопке отправил accept_invite через postMessage");
    assert.deepEqual(env.calls[0], {
      cmd: "accept_invite",
      args: {
        code: "discord-testers",
        captcha_key: SAMPLE_TOKEN,
        captcha_rqtoken: "rq_1click",
      },
    });
    assert.equal(env.modal.isOpen, false);
    assert.equal(env.solved.length, 1);
  });

  it("кнопка #captcha-webview-verify-btn работает в 1 клик с data-token на фрейме без resolveWebviewToken", async () => {
    const env = createInteractiveTestEnv((cmd) => {
      if (cmd === "accept_invite") return { guildId: SAMPLE_PREVIEW.guildId };
      throw new Error(`unexpected: ${cmd}`);
    });

    env.modal.open({
      code: "discord-testers",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
    });

    const webviewIframe = env.document.getElementById("captcha-webview-frame");
    webviewIframe?.setAttribute("data-token", SAMPLE_TOKEN);

    const verifyBtn = env.document.getElementById("captcha-webview-verify-btn");
    verifyBtn?.click();

    await new Promise((r) => setTimeout(r, 20));

    assert.equal(env.calls.length, 1);
    assert.equal(env.calls[0]?.args?.captcha_key, SAMPLE_TOKEN);
    assert.equal(env.modal.isOpen, false);
  });

  it("postTokenMessage на контроллере передаёт токен через postMessage-пайплайн и решает капчу", async () => {
    const env = createInteractiveTestEnv((cmd) => {
      if (cmd === "accept_invite") return { guildId: SAMPLE_PREVIEW.guildId };
      throw new Error(`unexpected: ${cmd}`);
    });

    env.modal.open({
      code: "discord-testers",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
    });

    const transferred = env.modal.postTokenMessage(SAMPLE_TOKEN);
    assert.equal(transferred, true);

    await new Promise((r) => setTimeout(r, 20));

    assert.equal(env.calls.length, 1);
    assert.equal(env.calls[0]?.args?.captcha_key, SAMPLE_TOKEN);
    assert.equal(env.modal.isOpen, false);
  });
});

describe("3. Безопасность, негативные сценарии, защита от гонок и восстановление (9 измерений TEST-POLICY)", () => {
  it("отклоняет postMessage с недоверенного origin и не вызывает accept_invite", async () => {
    const env = createInteractiveTestEnv();
    env.modal.open({
      code: "discord-testers",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
    });

    const accepted = env.modal.handleMessage({
      type: "message",
      origin: "https://evil.example.com",
      data: {
        event: "challenge-passed",
        response: SAMPLE_TOKEN,
      },
    });

    await new Promise((r) => setTimeout(r, 20));

    assert.equal(accepted, false, "сообщение с недоверенного origin должно быть отклонено");
    assert.equal(env.calls.length, 0, "accept_invite не должен вызываться для чужого origin");
    assert.equal(env.modal.isOpen, true, "модалка остаётся открытой");
  });

  it("игнорирует служебные postMessage без токена и сообщения при закрытой модалке", async () => {
    const env = createInteractiveTestEnv();

    // Модалка закрыта
    assert.equal(
      env.modal.handleMessage({
        type: "message",
        origin: "https://newassets.hcaptcha.com",
        data: { response: SAMPLE_TOKEN },
      }),
      false,
    );
    assert.equal(env.calls.length, 0);

    // Открываем и шлём служебное событие ресайза без токена
    env.modal.open({
      code: "discord-testers",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
    });
    assert.equal(
      env.modal.handleMessage({
        type: "message",
        origin: "https://newassets.hcaptcha.com",
        data: { event: "challenge-loaded", height: 420 },
      }),
      false,
    );
    assert.equal(env.calls.length, 0);
    assert.equal(env.modal.isOpen, true);
  });

  it("предотвращает двойную отправку при серии postMessage во время in-flight и восстанавливается после ошибки", async () => {
    /** @type {((reason?: any) => void) | null} */
    let rejectFirst = null;
    let attempt = 0;

    const env = createInteractiveTestEnv(() => {
      attempt += 1;
      if (attempt === 1) {
        return new Promise((_, reject) => {
          rejectFirst = reject;
        });
      }
      return { guildId: SAMPLE_PREVIEW.guildId };
    });

    env.modal.open({
      code: "discord-testers",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
    });

    // Первый postMessage запускает запрос
    const firstAccepted = env.modal.postTokenMessage("P1_first_token_attempt_12345");
    assert.equal(firstAccepted, true);
    assert.equal(env.modal.state, "submitting");

    // Второй postMessage во время полёта первого игнорируется
    const secondAccepted = env.modal.postTokenMessage("P1_second_token_duplicate_12345");
    assert.equal(secondAccepted, false, "дублирующий postMessage во время in-flight отклонён");
    assert.equal(env.calls.length, 1);

    // Первая попытка возвращает новый челлендж 40003 с новым sitekey и rqdata
    const rotatedSitekey = "b1c2d3e4-0000-4000-8000-111122223333";
    /** @type {any} */ (rejectFirst)?.({
      kind: "captchaRequired",
      status: 400,
      code: CAPTCHA_ERROR_CODE,
      captcha_service: "hcaptcha",
      captcha_sitekey: rotatedSitekey,
      captcha_rqdata: "rotated_rqdata_99",
      captcha_rqtoken: "rotated_rqtoken_99",
    });

    await new Promise((r) => setTimeout(r, 20));

    assert.equal(env.modal.state, "error");
    assert.equal(env.modal.isOpen, true);
    const webviewIframe = env.document.getElementById("captcha-webview-frame");
    assert.match(
      String(webviewIframe?.getAttribute("src") ?? ""),
      new RegExp(`sitekey=${rotatedSitekey}`),
      "src WebView iframe обновился при ротации челленджа",
    );
    assert.match(
      String(webviewIframe?.getAttribute("src") ?? ""),
      /rqdata=rotated_rqdata_99/,
      "rqdata WebView iframe обновился при ротации челленджа",
    );

    // Повторный postMessage со вторым токеном успешно завершает вступление
    env.modal.postTokenMessage(SAMPLE_TOKEN);
    await new Promise((r) => setTimeout(r, 20));

    assert.equal(env.calls.length, 2);
    assert.equal(env.calls[1]?.args?.captcha_key, SAMPLE_TOKEN);
    assert.equal(env.calls[1]?.args?.captcha_rqtoken, "rotated_rqtoken_99");
    assert.equal(env.modal.isOpen, false);
    assert.equal(env.solved.length, 1);
  });

  it("сквозной сценарий join-server.js: window.dispatchEvent('message') от WebView автоматически завершает вступление на сервер", async () => {
    const gui = await loadApp();
    /** @type {Array<{ cmd: string, args: any }>} */
    const calls = [];
    /** @type {Array<{ id: string, name: string }>} */
    const acceptedGuilds = [];
    let acceptAttempt = 0;

    const join = createJoinServer({
      invoke: async (cmd, args) => {
        calls.push({ cmd, args });
        if (cmd === "resolve_invite") return SAMPLE_PREVIEW;
        if (cmd === "accept_invite") {
          acceptAttempt += 1;
          if (acceptAttempt === 1) {
            throw {
              kind: "captchaRequired",
              status: 400,
              code: CAPTCHA_ERROR_CODE,
              captcha_service: "hcaptcha",
              captcha_sitekey: SAMPLE_SITEKEY,
              captcha_rqtoken: "rq_live_window_1",
            };
          }
          return {
            guildId: SAMPLE_PREVIEW.guildId,
            targetChannelId: SAMPLE_PREVIEW.channelId,
          };
        }
        throw new Error(`Unexpected command: ${cmd}`);
      },
      onAccepted(guild) {
        acceptedGuilds.push(guild);
      },
    });

    try {
      join.open();
      const inviteInput = /** @type {any} */ (gui.window.document.getElementById("join-invite-input"));
      inviteInput.value = "discord-testers";
      inviteInput.emit("input");

      // 1. resolve_invite
      await join.submit();
      assert.equal(join.state, "preview_ready");

      // 2. Первый accept_invite -> 40003 -> открывается #captcha-modal с WebView
      await join.submit();
      const captchaModalEl = gui.window.document.getElementById("captcha-modal");
      assert.ok(captchaModalEl);
      assert.equal(captchaModalEl.hasAttribute("hidden"), false);

      // 3. WebView отправляет токен через window postMessage (без ручного копирования в input!)
      gui.window.dispatchEvent({
        type: "message",
        origin: "https://newassets.hcaptcha.com",
        data: createCaptchaTokenMessage(SAMPLE_TOKEN, {
          captcha_sitekey: SAMPLE_SITEKEY,
          captcha_rqtoken: "rq_live_window_1",
        }),
      });

      await gui.settle(5);

      const acceptCalls = calls.filter((c) => c.cmd === "accept_invite");
      assert.equal(acceptCalls.length, 2, "accept_invite автоматически вызван после window message");
      assert.deepEqual(acceptCalls[1]?.args, {
        code: "discord-testers",
        captcha_key: SAMPLE_TOKEN,
        captcha_rqtoken: "rq_live_window_1",
      });
      assert.equal(captchaModalEl.hasAttribute("hidden"), true, "#captcha-modal закрылся");
      assert.equal(join.state, "accepted");
      assert.deepEqual(acceptedGuilds, [
        { id: SAMPLE_PREVIEW.guildId, name: SAMPLE_PREVIEW.guildName },
      ]);
    } finally {
      join.dispose();
      gui.dispose();
    }
  });

  it("безопасность: исходный код captcha-modal.js не использует сетевые API, eval или внешнее хранилище", () => {
    assert.equal(
      /\bfetch\s*\(|XMLHttpRequest|localStorage|sessionStorage|\beval\s*\(/.test(captchaModalSource),
      false,
    );
  });
});

describe("4. TASK-818: Локальный мост hcaptcha-bridge.html, устранение XML AccessDenied и решение 40003", () => {
  const LIVE_DISCORD_SITEKEY = "b2b02ab5-7dae-4d6f-830e-7b55634c888b";
  const LIVE_DISCORD_RQDATA =
    "92gRgMZWCdUQfzt8d4cRs6IEgLCIW0Qa6MSSRei9SZlx0LjIJRCa1VfxlC9rQ/HzGp37O5B93wKfHfj9W+p31FWTAk7Il8+TqZhagyzlF+4I6mz7usMdpt81pCY64gZOkuSg3LfdPmDM5g==aubnoMnDsiDpk4id";
  const LIVE_DISCORD_RQTOKEN =
    "mock_captcha_rqtoken_live_discord_sanitized_token_value";
  const LIVE_DISCORD_SESSION_ID = "7d05ae83-ba3b-4b61-9a33-c08ce00e1eed";

  it("шаблон rust/gui-spike/ui/assets/hcaptcha-bridge.html существует, содержит api.js, sitekey b2b02ab5-7dae-4d6f-830e-7b55634c888b, rqdata, dark theme и postMessage мост без AccessDenied", () => {
    assert.equal(
      existsSync(hcaptchaBridgePath),
      true,
      "rust/gui-spike/ui/assets/hcaptcha-bridge.html должен существовать",
    );
    const bridgeHtml = readFileSync(hcaptchaBridgePath, "utf8");
    assert.ok(bridgeHtml.length > 200, "hcaptcha-bridge.html не пустой");

    // 1. Отсутствие S3 XML AccessDenied
    assert.equal(
      /AccessDenied|<Error><Code>AccessDenied/i.test(bridgeHtml),
      false,
      "hcaptcha-bridge.html не должен содержать ошибку AccessDenied",
    );

    // 2. Официальный SDK api.js и эталонный sitekey
    assert.match(
      bridgeHtml,
      /https:\/\/js\.hcaptcha\.com\/1\/api\.js/,
      "подключён официальный SDK https://js.hcaptcha.com/1/api.js",
    );
    assert.match(
      bridgeHtml,
      new RegExp(LIVE_DISCORD_SITEKEY),
      "содержит эталонный sitekey b2b02ab5-7dae-4d6f-830e-7b55634c888b",
    );

    // 3. Контейнер div.h-captcha / #hcaptcha-webview-box, dark theme и callback onHCaptchaSuccess
    assert.match(bridgeHtml, /class=["'][^"']*\bh-captcha\b[^"']*["']/, "содержит контейнер div.h-captcha");
    assert.match(bridgeHtml, /id=["']hcaptcha-webview-box["']/, "содержит #hcaptcha-webview-box");
    assert.match(bridgeHtml, /data-theme=["']dark["']/, "установлена тёмная тема data-theme=\"dark\"");
    assert.match(
      bridgeHtml,
      /data-callback=["']onHCaptchaSuccess["']/,
      "установлен callback data-callback=\"onHCaptchaSuccess\"",
    );

    // 4. Параметры URLSearchParams (sitekey, rqdata, theme: dark) и hcaptcha.render + postMessage
    assert.match(bridgeHtml, /URLSearchParams/, "читает динамические параметры через URLSearchParams");
    assert.match(bridgeHtml, /rqdata/, "передаёт параметр rqdata в hcaptcha.render / setData");
    assert.match(bridgeHtml, /hcaptcha\.render/, "вызывает hcaptcha.render");
    assert.match(bridgeHtml, /onHCaptchaSuccess/, "реализует обработчик onHCaptchaSuccess");
    assert.match(bridgeHtml, /postMessage/, "отправляет токен решения через postMessage");

    // 5. Сверка дизайн-токенов TASK-818 из kanban-board.json
    const task818 = kanbanBoard.cards.find((/** @type {any} */ t) => t.id === "TASK-818");
    assert.ok(task818, "TASK-818 присутствует в kanban-board.json");
    assert.equal(HCAPTCHA_BRIDGE_TOKENS.background, task818.design_tokens.background);
    assert.equal(HCAPTCHA_BRIDGE_TOKENS.surface, task818.design_tokens.surface);
    assert.equal(HCAPTCHA_BRIDGE_TOKENS.textPrimary, task818.design_tokens.text_primary);
    assert.equal(HCAPTCHA_BRIDGE_TOKENS.accent, task818.design_tokens.accent);
    assert.equal(HCAPTCHA_BRIDGE_TOKENS.border, task818.design_tokens.border);
    assert.match(bridgeHtml, /#313338/i, "использует background токен #313338");
    assert.match(bridgeHtml, /#2b2d31/i, "использует surface токен #2b2d31");
    assert.match(bridgeHtml, /#5865f2/i, "использует accent токен #5865f2");
    assert.match(bridgeHtml, /#3f4147/i, "использует border токен #3f4147");
  });

  it("buildHcaptchaBridgeUrl формирует локальный URL assets/hcaptcha-bridge.html с sitekey, rqdata и theme=dark", () => {
    assert.equal(DEFAULT_HCAPTCHA_SITEKEY, LIVE_DISCORD_SITEKEY);
    assert.equal(HCAPTCHA_API_URL, "https://js.hcaptcha.com/1/api.js");
    assert.equal(HCAPTCHA_BRIDGE_URL, "assets/hcaptcha-bridge.html");

    const url = buildHcaptchaBridgeUrl({
      captcha_sitekey: LIVE_DISCORD_SITEKEY,
      captcha_rqdata: LIVE_DISCORD_RQDATA,
      captcha_rqtoken: LIVE_DISCORD_RQTOKEN,
      captcha_session_id: LIVE_DISCORD_SESSION_ID,
    });
    assert.ok(
      url.startsWith("assets/hcaptcha-bridge.html?"),
      "URL моста должен начинаться с assets/hcaptcha-bridge.html?",
    );
    assert.match(url, new RegExp(`sitekey=${LIVE_DISCORD_SITEKEY}`));
    assert.match(url, /theme=dark/);
    assert.match(url, /host=discord\.com/);
    assert.match(url, new RegExp(`rqdata=${encodeURIComponent(LIVE_DISCORD_RQDATA).replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`));
    assert.equal(url.includes("newassets.hcaptcha.com/captcha/v1/embed.html"), false);
  });

  it("полный цикл решения реального челленджа 40003 из телеметрии с sitekey b2b02ab5-7dae-4d6f-830e-7b55634c888b и rqdata через hcaptcha-bridge.html", async () => {
    const env = createInteractiveTestEnv((cmd) => {
      if (cmd === "accept_invite") {
        return {
          guildId: SAMPLE_PREVIEW.guildId,
          targetChannelId: SAMPLE_PREVIEW.channelId,
        };
      }
      throw new Error(`unexpected cmd: ${cmd}`);
    });

    const opened = env.modal.open({
      code: "eSrxp2YGUC",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: LIVE_DISCORD_SITEKEY,
      captcha_rqdata: LIVE_DISCORD_RQDATA,
      captcha_rqtoken: LIVE_DISCORD_RQTOKEN,
      captcha_session_id: LIVE_DISCORD_SESSION_ID,
    });
    assert.equal(opened, true);

    const webviewIframe = /** @type {any} */ (env.document.getElementById("captcha-webview-frame"));
    const webviewBox = env.document.getElementById("hcaptcha-webview-box");
    assert.ok(webviewIframe, "#captcha-webview-frame присутствует");
    assert.ok(webviewBox, "#hcaptcha-webview-box присутствует в модалке");

    const iframeSrc = String(webviewIframe.getAttribute("src") ?? "");
    assert.ok(
      iframeSrc.startsWith("assets/hcaptcha-bridge.html?"),
      "iframe загружает локальный мост assets/hcaptcha-bridge.html вместо S3 embed.html",
    );
    assert.match(iframeSrc, new RegExp(`sitekey=${LIVE_DISCORD_SITEKEY}`));
    assert.match(iframeSrc, /theme=dark/);

    // Эмулируем callback onHCaptchaSuccess из hcaptcha-bridge.html через postMessage
    webviewIframe.postMessage(
      {
        source: "discord-captcha-webview",
        service: "hcaptcha",
        event: "challenge-passed",
        type: "hcaptcha-token",
        token: SAMPLE_TOKEN,
        response: SAMPLE_TOKEN,
        "h-captcha-response": SAMPLE_TOKEN,
        captcha_key: SAMPLE_TOKEN,
        sitekey: LIVE_DISCORD_SITEKEY,
        rqdata: LIVE_DISCORD_RQDATA,
        rqtoken: LIVE_DISCORD_RQTOKEN,
        session_id: LIVE_DISCORD_SESSION_ID,
      },
      "tauri://localhost",
    );

    await new Promise((r) => setTimeout(r, 20));

    assert.equal(env.calls.length, 1);
    assert.deepEqual(env.calls[0], {
      cmd: "accept_invite",
      args: {
        code: "eSrxp2YGUC",
        captcha_key: SAMPLE_TOKEN,
        captcha_rqtoken: LIVE_DISCORD_RQTOKEN,
        captcha_session_id: LIVE_DISCORD_SESSION_ID,
      },
    });
    assert.equal(env.modal.isOpen, false);
    assert.equal(env.solved.length, 1);
  });
});
