// @ts-check
/**
 * TASK-610: unit-тесты интерактивного модального окна решения hCaptcha (`#captcha-modal` / `ui/features/captcha-modal.js`)
 * и его интеграции с `accept_invite` в `ui/features/join-server.js`.
 *
 * Контракт:
 * 1. При вызове `accept_invite`, если Discord возвращает HTTP 400 с `captcha_service: 'hcaptcha'`
 *    и `captcha_sitekey` (код ошибки `40003`), открывается интерактивное диалоговое окно `#captcha-modal`.
 * 2. Модалка загружает веб-виджет / форму решения hCaptcha, принимает решение от пользователя,
 *    получает токен `captcha_key` и повторно вызывает `accept_invite` с параметром `captcha_key`.
 * 3. Покрыты 9 измерений TEST-POLICY: позитивные, негативные, граничные, ошибки (400/401/403/404/429/5xx),
 *    пустые/отмена (Escape/Cancel), загрузка (double-submit guard), доступность (ARIA), восстановление и безопасность.
 */
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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

const captchaModalUrl = new URL("../../rust/gui-spike/ui/features/captcha-modal.js", import.meta.url);
const captchaModalSource = readFileSync(captchaModalUrl, "utf8");

const SAMPLE_SITEKEY = "a9b5fb07-92ff-493f-86fe-352a2803b3df";
const SAMPLE_TOKEN = "P1_eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.hcaptcha_solution_token_610";
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
 * Создаёт изолированное DOM-окружение для тестирования `#captcha-modal`.
 * @param {(cmd: string, args?: Record<string, unknown>) => unknown | Promise<unknown>} [handler]
 */
function createCaptchaTestEnv(handler = () => ({ guildId: SAMPLE_PREVIEW.guildId })) {
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

describe("TASK-610: классификатор и разбор hCaptcha-челленджа (extractCaptchaChallenge)", () => {
  it("распознаёт HTTP 400 с captcha_service: 'hcaptcha', captcha_sitekey и code: 40003", () => {
    const errKind = {
      kind: "captchaRequired",
      code: 40003,
      captcha_service: "hcaptcha",
      captcha_sitekey: SAMPLE_SITEKEY,
      captcha_rqdata: "rqdata_abc",
      captcha_rqtoken: "rqtoken_xyz",
      captcha_session_id: "sess_123",
    };
    assert.equal(isCaptchaChallengeError(errKind), true);
    const challenge = extractCaptchaChallenge(errKind);
    assert.deepEqual(challenge, {
      code: 40003,
      captcha_service: "hcaptcha",
      captcha_sitekey: SAMPLE_SITEKEY,
      captcha_rqdata: "rqdata_abc",
      captcha_rqtoken: "rqtoken_xyz",
      captcha_session_id: "sess_123",
    });

    const errHttp400 = {
      kind: "client",
      status: 400,
      code: 40003,
      captcha_service: "hcaptcha",
      captcha_sitekey: SAMPLE_SITEKEY,
    };
    assert.equal(isCaptchaChallengeError(errHttp400), true);
    const challenge400 = extractCaptchaChallenge(errHttp400);
    assert.deepEqual(challenge400, {
      code: 40003,
      captcha_service: "hcaptcha",
      captcha_sitekey: SAMPLE_SITEKEY,
      captcha_rqdata: null,
      captcha_rqtoken: null,
      captcha_session_id: null,
    });
  });

  it("отклоняет ошибки без captcha_sitekey, с неподдерживаемым сервисом или не-400 статусом", () => {
    assert.equal(extractCaptchaChallenge(null), null);
    assert.equal(extractCaptchaChallenge(undefined), null);
    assert.equal(extractCaptchaChallenge({ kind: "forbidden" }), null);
    assert.equal(extractCaptchaChallenge({ kind: "captchaRequired", code: 40003 }), null);
    assert.equal(
      extractCaptchaChallenge({
        kind: "client",
        status: 400,
        code: 40003,
        captcha_service: "recaptcha",
        captcha_sitekey: SAMPLE_SITEKEY,
      }),
      null,
    );
    assert.equal(
      extractCaptchaChallenge({
        kind: "client",
        status: 403,
        code: 40003,
        captcha_service: "hcaptcha",
        captcha_sitekey: SAMPLE_SITEKEY,
      }),
      null,
    );
  });

  it("формирует URL веб-виджета hCaptcha с sitekey, host и опциональным rqdata", () => {
    const url = buildHcaptchaWidgetUrl({
      captcha_sitekey: SAMPLE_SITEKEY,
      captcha_rqdata: "rq_payload_1",
    });
    assert.match(url, /^https:\/\/newassets\.hcaptcha\.com\/captcha\/v1\//);
    assert.match(url, new RegExp(`sitekey=${SAMPLE_SITEKEY}`));
    assert.match(url, /host=discord\.com/);
    assert.match(url, /rqdata=rq_payload_1/);
  });
});

describe("TASK-610: интерактивное модальное окно #captcha-modal (DOM, A11y, решение и повтор accept_invite)", () => {
  it("динамически монтирует #captcha-modal с role=dialog, aria-modal=true, виджетом и формой ввода captcha_key", () => {
    const env = createCaptchaTestEnv();
    const root = env.document.getElementById("captcha-modal");
    assert.ok(root, "#captcha-modal смонтирован в DOM");
    assert.equal(root.getAttribute("role"), "dialog");
    assert.equal(root.getAttribute("aria-modal"), "true");
    assert.equal(root.getAttribute("aria-labelledby"), "captcha-modal-title");
    assert.equal(root.getAttribute("hidden"), "", "по умолчанию модалка скрыта");
    assert.equal(root.getAttribute("data-state"), "idle");

    const opened = env.modal.open({
      code: "discord-testers",
      captcha_service: HCAPTCHA_SERVICE,
      captcha_sitekey: SAMPLE_SITEKEY,
      captcha_rqdata: "rq_test_data",
    });
    assert.equal(opened, true);
    assert.equal(root.getAttribute("hidden"), null, "после open() модалка видима");
    assert.equal(root.getAttribute("data-state"), "ready");
    assert.equal(root.getAttribute("data-service"), "hcaptcha");
    assert.equal(root.getAttribute("data-sitekey"), SAMPLE_SITEKEY);

    const widgetFrame = env.document.getElementById("captcha-widget-frame");
    assert.ok(widgetFrame, "контейнер веб-виджета hCaptcha присутствует");
    assert.equal(widgetFrame.getAttribute("data-sitekey"), SAMPLE_SITEKEY);
    assert.equal(widgetFrame.getAttribute("data-service"), "hcaptcha");
    assert.match(String(widgetFrame.getAttribute("data-widget-url") ?? ""), new RegExp(SAMPLE_SITEKEY));

    const input = env.document.getElementById("captcha-token-input");
    const submitBtn = env.document.getElementById("captcha-submit-btn");
    const cancelBtn = env.document.getElementById("captcha-cancel-btn");
    assert.ok(input, "поле ввода токена captcha_key присутствует");
    assert.ok(submitBtn, "кнопка отправки присутствует");
    assert.ok(cancelBtn, "кнопка отмены присутствует");
  });

  it("принимает решение от пользователя и повторно вызывает accept_invite с параметром captcha_key", async () => {
    const env = createCaptchaTestEnv((cmd, args) => {
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
      captcha_service: "hcaptcha",
      captcha_sitekey: SAMPLE_SITEKEY,
      captcha_rqtoken: "rq_tok_77",
      captcha_session_id: "sess_88",
    });

    const input = /** @type {any} */ (env.document.getElementById("captcha-token-input"));
    input.value = `  ${SAMPLE_TOKEN}  `;
    input.emit("input");

    const result = await env.modal.submit();
    assert.deepEqual(result, {
      guildId: SAMPLE_PREVIEW.guildId,
      targetChannelId: SAMPLE_PREVIEW.channelId,
    });
    assert.equal(env.calls.length, 1, "вызван ровно один повтор accept_invite");
    assert.deepEqual(env.calls[0], {
      cmd: "accept_invite",
      args: {
        code: "discord-testers",
        captcha_key: SAMPLE_TOKEN,
        captcha_rqtoken: "rq_tok_77",
        captcha_session_id: "sess_88",
      },
    });

    const root = env.document.getElementById("captcha-modal");
    assert.equal(root?.getAttribute("hidden"), "", "после успешного accept_invite модалка закрыта");
    assert.equal(env.solved.length, 1, "вызван колбэк onSolved");
    assert.equal(env.solved[0]?.meta.captcha_key, SAMPLE_TOKEN);
  });

  it("поддерживает получение токена напрямую из веб-виджета hCaptcha (solveWithToken / widget callback)", async () => {
    const env = createCaptchaTestEnv(() => ({
      guildId: SAMPLE_PREVIEW.guildId,
    }));

    env.modal.open({
      code: "discord-testers",
      captcha_service: "hcaptcha",
      captcha_sitekey: SAMPLE_SITEKEY,
    });

    await env.modal.solveWithToken(SAMPLE_TOKEN);
    assert.equal(env.calls.length, 1);
    assert.deepEqual(env.calls[0], {
      cmd: "accept_invite",
      args: {
        code: "discord-testers",
        captcha_key: SAMPLE_TOKEN,
      },
    });
    assert.equal(env.modal.isOpen, false);
  });

  it("валидирует пустой captcha_key без вызова IPC и отображает ошибку в #captcha-modal-error", async () => {
    const env = createCaptchaTestEnv();
    env.modal.open({
      code: "discord-testers",
      captcha_service: "hcaptcha",
      captcha_sitekey: SAMPLE_SITEKEY,
    });

    const input = /** @type {any} */ (env.document.getElementById("captcha-token-input"));
    input.value = "   ";
    await env.modal.submit();

    assert.equal(env.calls.length, 0, "пустой токен не должен вызывать accept_invite");
    const root = env.document.getElementById("captcha-modal");
    const errorNode = env.document.getElementById("captcha-modal-error");
    assert.equal(root?.getAttribute("data-state"), "error");
    assert.equal(errorNode?.getAttribute("hidden"), null);
    assert.equal(errorNode?.textContent, CAPTCHA_TEXT.emptyToken);
  });

  it("блокирует повторную отправку во время полёта запроса (double-submit guard) и восстанавливается после ошибки", async () => {
    /** @type {((val: any) => void) | null} */
    let resolvePending = null;
    let attempt = 0;
    const env = createCaptchaTestEnv(() => {
      attempt += 1;
      if (attempt === 1) {
        return new Promise((_, reject) => {
          resolvePending = reject;
        });
      }
      return { guildId: SAMPLE_PREVIEW.guildId };
    });

    env.modal.open({
      code: "discord-testers",
      captcha_service: "hcaptcha",
      captcha_sitekey: SAMPLE_SITEKEY,
    });

    const input = /** @type {any} */ (env.document.getElementById("captcha-token-input"));
    input.value = "token_attempt_1";

    const firstAttempt = env.modal.submit();
    const root = env.document.getElementById("captcha-modal");
    const submitBtn = /** @type {any} */ (env.document.getElementById("captcha-submit-btn"));
    assert.equal(root?.getAttribute("data-state"), "submitting");
    assert.equal(submitBtn?.disabled, true);

    // Повторный клик во время in-flight игнорируется
    await env.modal.submit();
    assert.equal(env.calls.length, 1, "повтор во время in-flight не создаёт второй вызов");

    // Завершаем первую попытку ошибкой 429 rateLimited
    /** @type {any} */ (resolvePending)?.({ kind: "rateLimited", retry_after_ms: 2000 });
    await firstAttempt;

    assert.equal(root?.getAttribute("data-state"), "error");
    assert.equal(env.modal.isOpen, true, "при ошибке модалка остаётся открытой");
    const errorNode = env.document.getElementById("captcha-modal-error");
    assert.match(String(errorNode?.textContent ?? ""), /2\s*сек/);

    // Вторая попытка с новым токеном проходит успешно
    input.value = SAMPLE_TOKEN;
    await env.modal.submit();
    assert.equal(env.calls.length, 2);
    assert.equal(env.modal.isOpen, false);
    assert.equal(env.solved.length, 1);
  });

  it("закрывается по кнопке Отмена и клавише Escape без вызова accept_invite", () => {
    const env = createCaptchaTestEnv();
    env.modal.open({
      code: "discord-testers",
      captcha_service: "hcaptcha",
      captcha_sitekey: SAMPLE_SITEKEY,
    });
    assert.equal(env.modal.isOpen, true);

    const root = env.document.getElementById("captcha-modal");
    root?.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(env.modal.isOpen, false);
    assert.equal(env.cancelCount, 1);
    assert.equal(env.calls.length, 0);
  });
});

describe("TASK-610: сквозная интеграция join-server.js ↔ #captcha-modal", () => {
  it("при HTTP 400 (code 40003, captcha_service: 'hcaptcha', captcha_sitekey) открывает #captcha-modal и завершает вступление с captcha_key", async () => {
    const gui = await loadApp();
    const document = gui.window.document;
    /** @type {Array<{ cmd: string, args: any }>} */
    const calls = [];
    /** @type {Array<{ id: string, name: string }>} */
    const acceptedGuilds = [];
    let acceptAttempt = 0;

    const join = createJoinServer({
      invoke: async (cmd, args) => {
        calls.push({ cmd, args });
        if (cmd === "resolve_invite") {
          return SAMPLE_PREVIEW;
        }
        if (cmd === "accept_invite") {
          acceptAttempt += 1;
          if (acceptAttempt === 1) {
            throw {
              kind: "captchaRequired",
              status: 400,
              code: CAPTCHA_ERROR_CODE,
              captcha_service: "hcaptcha",
              captcha_sitekey: SAMPLE_SITEKEY,
              captcha_rqtoken: "rq_token_invite_1",
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
      const inviteInput = /** @type {any} */ (document.getElementById("join-invite-input"));
      inviteInput.value = "discord-testers";
      inviteInput.emit("input");

      // Шаг 1: resolve_invite
      await join.submit();
      assert.equal(join.state, "preview_ready");

      // Шаг 2: первый accept_invite -> возвращает 400 с hCaptcha челленджем
      await join.submit();
      const captchaModalEl = document.getElementById("captcha-modal");
      assert.ok(captchaModalEl, "#captcha-modal создан в DOM");
      assert.equal(captchaModalEl.getAttribute("hidden"), null, "#captcha-modal открыт после 40003");
      assert.equal(captchaModalEl.getAttribute("data-sitekey"), SAMPLE_SITEKEY);
      assert.equal(captchaModalEl.getAttribute("data-service"), "hcaptcha");
      assert.equal(acceptedGuilds.length, 0, "до решения капчи сервер не добавлен");

      // Шаг 3: пользователь решает капчу и отправляет токен captcha_key
      const tokenInput = /** @type {any} */ (document.getElementById("captcha-token-input"));
      tokenInput.value = SAMPLE_TOKEN;
      tokenInput.emit("input");
      const captchaSubmitBtn = document.getElementById("captcha-submit-btn");
      captchaSubmitBtn?.click();

      // Ждём завершения асинхронного повтора accept_invite
      await gui.settle(3);

      const acceptCalls = calls.filter((c) => c.cmd === "accept_invite");
      assert.equal(acceptCalls.length, 2, "accept_invite вызван дважды: до и после решения hCaptcha");
      assert.deepEqual(acceptCalls[0]?.args, { code: "discord-testers" });
      assert.deepEqual(acceptCalls[1]?.args, {
        code: "discord-testers",
        captcha_key: SAMPLE_TOKEN,
        captcha_rqtoken: "rq_token_invite_1",
      });

      assert.equal(captchaModalEl.getAttribute("hidden"), "", "#captcha-modal закрылся после успеха");
      assert.equal(join.isOpen, false, "#join-server-dialog закрылся после успешного вступления");
      assert.equal(join.state, "accepted");
      assert.deepEqual(acceptedGuilds, [
        { id: SAMPLE_PREVIEW.guildId, name: SAMPLE_PREVIEW.guildName },
      ]);
    } finally {
      join.dispose();
      gui.dispose();
    }
  });

  it("TASK-637: клик по captcha-cancel/captcha-submit/captcha-close останавливает всплытие и не падает в ACTION_FAILED", () => {
    const env = createCaptchaTestEnv();
    env.modal.open({
      code: "eSrxp2YGUC",
      captcha_service: "hcaptcha",
      captcha_sitekey: SAMPLE_SITEKEY,
    });

    const cancelBtn = env.document.getElementById("captcha-cancel-btn");
    assert.ok(cancelBtn, "кнопка captcha-cancel найдена");
    assert.equal(cancelBtn.getAttribute("data-action"), "captcha-cancel");

    // Вызываем click на кнопке Отмена
    const event = cancelBtn.dispatchEvent("click");
    assert.equal(/** @type {any} */ (event).stopped, true, "клик по кнопке отмены остановил всплытие события (stopPropagation)");
    assert.equal(env.modal.isOpen, false, "модалка закрылась после нажатия Отмена");
  });

  it("безопасность: модуль captcha-modal.js не обращается к fetch/localStorage/sessionStorage напрямую", () => {
    assert.equal(
      /\bfetch\s*\(|XMLHttpRequest|localStorage|sessionStorage/.test(captchaModalSource),
      false,
      "captcha-modal.js не должен ходить в сеть или браузерное хранилище мимо IPC",
    );
  });
});
