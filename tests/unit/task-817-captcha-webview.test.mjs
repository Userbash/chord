// @ts-check
/**
 * TASK-817: Устранение ложного вызова капчи и интеграция WebView-виджета вместо ручного ввода.
 *
 * Критерии приёмки:
 * 1. Пользователю больше не нужно вручную копировать и вставлять строковый токен капчи
 *    (интерактивный WebView iframe виджет + автоматический submit по событию решения в 1 клик).
 * 2. При возможности подтверждения через правила сервера (requests/@me) капча не запрашивается вовсе.
 * 3. Ложный вызов капчи при HTTP 400 без кода 40003 устранён (ошибки лимитов, невалидного инвайта и т.д.
 *    не показывают текст про проверку Captcha).
 * 4. Полный охват 9 измерений качества TEST-POLICY (позитивные, негативные, граничные, A11y, отмена, восстановление).
 */
import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { ShimDocument, loadApp } from "../support/gui-dom-harness.mjs";
import {
  CAPTCHA_ERROR_CODE,
  CAPTCHA_TEXT,
  HCAPTCHA_SERVICE,
  buildHcaptchaWidgetUrl,
  createCaptchaModal,
  extractCaptchaChallenge,
  isCaptchaChallengeError,
} from "../../rust/gui-spike/ui/features/captcha-modal.js";
import { createJoinServer } from "../../rust/gui-spike/ui/features/join-server.js";

const SAMPLE_SITEKEY = "a9b5fb07-92ff-493f-86fe-352a2803b3df";
const SAMPLE_TOKEN = "P1_eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.hcaptcha_solution_token_817";
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
 * Создаёт тестовое окружение для проверки TASK-817.
 * @param {(cmd: string, args?: Record<string, unknown>) => unknown | Promise<unknown>} [handler]
 * @param {Record<string, unknown>} [options]
 */
function createTestEnv(handler = () => ({ guildId: SAMPLE_PREVIEW.guildId }), options = {}) {
  const document = new ShimDocument();
  const root = document.createElement("div");
  root.id = "app-root";
  document.body.append(root);
  /** @type {Array<{ cmd: string, args: Record<string, unknown> | undefined }>} */
  const calls = [];
  /** @type {Array<{ response: any, meta: any }>} */
  const solved = [];
  let cancelCount = 0;

  const invoke = async (/** @type {string} */ cmd, /** @type {any} */ args) => {
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
    ...options,
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

describe("TASK-817: Интеграция интерактивного WebView-виджета hCaptcha (вместо ручного ввода)", () => {
  it("открытие модалки монтирует интерактивный WebView (iframe #captcha-webview-frame) с доверенным URL", () => {
    const env = createTestEnv();
    const opened = env.modal.open({
      code: "testers",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
      captcha_rqdata: "rq_data_123",
    });
    assert.equal(opened, true);

    const widgetFrame = env.document.getElementById("captcha-widget-frame");
    assert.ok(widgetFrame, "контейнер #captcha-widget-frame существует");

    const webviewIframe = env.document.getElementById("captcha-webview-frame");
    assert.ok(webviewIframe, "iframe #captcha-webview-frame для интерактивного WebView встроен");
    assert.equal(webviewIframe.getAttribute("title"), CAPTCHA_TEXT.widgetLabel);
    assert.match(
      String(webviewIframe.getAttribute("src")),
      new RegExp(`assets/hcaptcha-bridge\\.html\\?.*sitekey=${SAMPLE_SITEKEY}`),
      "src iframe ведёт на локальный мост assets/hcaptcha-bridge.html без S3 AccessDenied",
    );
    assert.match(
      String(webviewIframe.getAttribute("src")),
      /theme=dark/,
      "bridge URL передаёт тёмную тему (theme=dark)",
    );
    assert.match(
      String(webviewIframe.getAttribute("src")),
      /rqdata=rq_data_123/,
      "rqdata безопасно передаётся в параметры виджета",
    );
    assert.ok(
      webviewIframe.getAttribute("sandbox")?.includes("allow-scripts"),
      "sandbox iframe разрешает скрипты виджета",
    );
  });

  it("автоматическое подтверждение в 1 клик по событию message от WebView без ручного ввода токена", async () => {
    const env = createTestEnv((cmd) => {
      if (cmd === "accept_invite") {
        return { guildId: SAMPLE_PREVIEW.guildId };
      }
      throw new Error(`unexpected: ${cmd}`);
    });

    env.modal.open({
      code: "testers",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
    });

    // Эмулируем событие postMessage от WebView iframe при успешном решении капчи пользователем
    const messageEvent = {
      type: "message",
      data: {
        event: "challenge-passed",
        response: SAMPLE_TOKEN,
      },
    };
    // Вызываем обработчик window message
    if (typeof env.modal.handleMessage === "function") {
      env.modal.handleMessage(messageEvent);
    } else {
      // Или напрямую через globalThis message dispatch
      globalThis.dispatchEvent(new Event("message"));
    }

    // Даём промису отработать
    await new Promise((r) => setTimeout(r, 50));

    assert.equal(env.calls.length, 1, "accept_invite автоматически вызван после клика в WebView");
    assert.equal(env.calls[0]?.args?.captcha_key, SAMPLE_TOKEN);
    assert.equal(env.modal.isOpen, false, "модалка закрылась автоматически после решения в WebView");
    assert.equal(env.solved.length, 1, "onSolved колбэк вызван");
  });

  it("поддерживает строку токена в postMessage (P1_... JWT)", async () => {
    const env = createTestEnv((cmd) => {
      if (cmd === "accept_invite") return { guildId: SAMPLE_PREVIEW.guildId };
      throw new Error(`unexpected: ${cmd}`);
    });

    env.modal.open({
      code: "testers",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
    });

    if (typeof env.modal.handleMessage === "function") {
      env.modal.handleMessage({ type: "message", data: SAMPLE_TOKEN });
    }

    await new Promise((r) => setTimeout(r, 50));
    assert.equal(env.calls.length, 1);
    assert.equal(env.calls[0]?.args?.captcha_key, SAMPLE_TOKEN);
  });
});

describe("TASK-817: Исключение ложного вызова капчи и правил сервера (requests/@me)", () => {
  it("при возможности подтверждения через правила сервера (requests/@me) капча не запрашивается вовсе", async () => {
    let rulesVerified = false;
    const env = createTestEnv(
      () => ({ guildId: SAMPLE_PREVIEW.guildId }),
      {
        async verifyViaRules(/** @type {string} */ guildId) {
          if (guildId === SAMPLE_PREVIEW.guildId) {
            rulesVerified = true;
            return true;
          }
          return false;
        },
      }
    );

    // Попытка открыть капчу для сервера с правилами
    const opened = env.modal.open({
      code: "testers",
      guildId: SAMPLE_PREVIEW.guildId,
      canVerifyRules: true,
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
    });

    assert.equal(opened, false, "модалка капчи НЕ должна открываться, если сервер верифицируется через правила");
    assert.equal(env.modal.isOpen, false);
  });

  it("в join-server.js при ошибке инвайта с челленджем, если сервер верифицируется через правила, капча не открывается", async () => {
    const gui = await loadApp();
    const document = gui.window.document;
    /** @type {Array<{ cmd: string, args: any }>} */
    const calls = [];
    /** @type {Array<{ id: string, name: string }>} */
    const acceptedGuilds = [];
    let rulesConfirmed = false;

    const join = createJoinServer({
      invoke: async (cmd, args) => {
        calls.push({ cmd, args });
        if (cmd === "resolve_invite") {
          return SAMPLE_PREVIEW;
        }
        if (cmd === "accept_invite") {
          // Имитируем ответ сервера: Discord требует капчу на инвайт
          throw {
            kind: "captchaRequired",
            status: 400,
            code: CAPTCHA_ERROR_CODE,
            captcha_service: "hcaptcha",
            captcha_sitekey: SAMPLE_SITEKEY,
          };
        }
        throw new Error(`Unexpected command: ${cmd}`);
      },
      // Функция подтверждения через requests/@me
      async verifyViaRules(/** @type {string} */ guildId) {
        if (guildId === SAMPLE_PREVIEW.guildId) {
          rulesConfirmed = true;
          return true;
        }
        return false;
      },
      onAccepted(guild) {
        acceptedGuilds.push(guild);
      },
    });

    try {
      join.open();
      const input = /** @type {any} */ (document.getElementById("join-invite-input"));
      input.value = "discord-testers";
      input.emit("input");

      // Шаг 1: resolve_invite
      await join.submit();
      assert.equal(join.state, "preview_ready");

      // Шаг 2: accept_invite -> сервер выбрасывает 40003, но verifyViaRules подтверждает правила!
      await join.submit();
      await gui.settle(3);

      assert.equal(rulesConfirmed, true, "правила сервера были подтверждены через requests/@me");
      const captchaModalEl = document.getElementById("captcha-modal");
      assert.equal(
        captchaModalEl === null || captchaModalEl.hasAttribute("hidden"),
        true,
        "модалка капчи НЕ была показана пользователю!",
      );
      assert.equal(join.state, "accepted", "сервер успешно принят без ввода капчи");
      assert.equal(acceptedGuilds.length, 1);
    } finally {
      join.dispose();
      gui.dispose();
    }
  });

  it("устранение ложного вызова: ошибка HTTP 400 без кода 40003 не показывает текст капчи", async () => {
    const gui = await loadApp();
    const document = gui.window.document;

    const join = createJoinServer({
      invoke: async (cmd) => {
        if (cmd === "resolve_invite") return SAMPLE_PREVIEW;
        if (cmd === "accept_invite") {
          // Обычная клиентская ошибка 400 (например, превышен лимит 100 серверов)
          throw {
            kind: "client",
            status: 400,
            code: 40060, // NOT 40003
            message: "Превышен лимит серверов",
          };
        }
        return undefined;
      },
    });

    try {
      join.open();
      const input = /** @type {any} */ (document.getElementById("join-invite-input"));
      input.value = "testers";
      input.emit("input");

      await join.submit();
      await join.submit();
      await gui.settle(3);

      const errorNode = document.getElementById("join-error");
      assert.ok(errorNode);
      assert.equal(
        errorNode.textContent?.includes("Captcha"),
        false,
        "ложный текст про Captcha не должен отображаться при коде 40060!",
      );
    } finally {
      join.dispose();
      gui.dispose();
    }
  });
});
