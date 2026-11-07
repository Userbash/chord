// @ts-check
/**
 * TASK-634 (G2 Frontend): модульные тесты модального окна подтверждения входа
 * по мобильному QR-коду (`#qr-auth-modal`, `rust/gui-spike/ui/features/qr-auth-modal.js`,
 * `rust/gui-spike/ui/styles/qr-auth-modal.css`).
 *
 * RE-оракулы:
 * - `/home/sanya/re-deep/frontend-d03.md` (§5 п.1 Button, п.5 Modal `data-mana-component="layer-modal"`,
 *   `role="dialog"`, `aria-modal="true"`, фокус-ловушка `Tab`/`Shift+Tab`, `Escape`);
 * - `/home/sanya/re-deep/decoded/f0cac74b6ccdca5e_0` (`QR_CHUNK`, модуль `52423`):
 *   - `RemoteAuthStep` (`INITIALIZING=0..FINISH=4`), `https://discord.com/ra/${fingerprint}`,
 *   - `publicKeyFingerprint` (`base64url_nopad(SHA-256(spki_der))` = 43 символа),
 *   - разбор `pending_ticket` (`^(\d+):(\d{1,4}):([a-zA-Z0-9_]+):(.*)$`, `"0"` -> `null`);
 * - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0` (`@275654..@275802`, `@11800708`):
 *   - `REMOTE_AUTH_INITIALIZE = "/users/@me/remote-auth"`,
 *   - `REMOTE_AUTH_FINISH = "/users/@me/remote-auth/finish"`,
 *   - `REMOTE_AUTH_CANCEL = "/users/@me/remote-auth/cancel"`,
 *   - `REMOTE_AUTH_LOGIN = "/users/@me/remote-auth/login"`,
 *   - `REMOTE_AUTH = "/ra/:remoteAuthFingerprint"`;
 * - `/home/sanya/re-deep/decoded/d8b0ffc3255e8113_0` (`@500000..@504200`, `AuthSessionsStore`):
 *   - отображение устройства (`client_info.os`, `client_info.platform`, `client_info.location`, `client_info.ip`,
 *     ключ `cDHCNY` «Неизвестно», иконки `ios`/`android`/`horizon os`/`desktop`);
 * - `rust/gui-spike/src/remote_auth.rs` (`TASK-629`) и `rust/gui-spike/ui/tokens.css`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { ShimDocument } from "../support/gui-dom-harness.mjs";

import {
  DEFAULT_QR_SESSION_TIMEOUT_MS,
  DEFAULT_QR_SESSION_TIMEOUT_SECONDS,
  QR_AUTH_IPC_COMMANDS,
  QR_AUTH_MODAL_STATES,
  QR_AUTH_SECURITY_LEVELS,
  QR_AUTH_TEXT,
  REMOTE_AUTH_BACKOFF_MAX_MS,
  REMOTE_AUTH_BACKOFF_MIN_MS,
  REMOTE_AUTH_FINGERPRINT_LEN,
  REMOTE_AUTH_GATEWAY_URL,
  REMOTE_AUTH_PROTOCOL_VERSION,
  REMOTE_AUTH_QR_URL_PREFIX,
  REMOTE_AUTH_REST_ENDPOINTS,
  REMOTE_AUTH_ROUTE_PATTERN,
  REMOTE_AUTH_STEPS,
  REMOTE_AUTH_WS_URL,
   clampSessionTimeoutMs,
  classifyQrAuthError,
  createQrAuthModal,
  evaluateQrSecurityIndicator,
  formatDeviceClientInfo,
  formatFingerprintDigest,
  isValidSnowflake,
  parseQrAuthUrl,
  parseRemoteAuthUserPayload,
  validateDeviceIpAddress,
  verifyFingerprintMatch,
} from "../../rust/gui-spike/ui/features/qr-auth-modal.js";

const QR_AUTH_MODAL_JS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/features/qr-auth-modal.js", import.meta.url),
);
const QR_AUTH_MODAL_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/styles/qr-auth-modal.css", import.meta.url),
);
const TOKENS_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/tokens.css", import.meta.url),
);

const SAMPLE_FINGERPRINT = "unglv48Bz-pBQUDeXa4iI7ADYaOWF3qctBD_YfIAFa0";
const OTHER_FINGERPRINT = "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789-_AbCdE";

/**
 * Создаёт изолированное DOM-окружение с кнопкой запуска подтверждения QR-входа.
 * @returns {{ doc: any, container: any, triggerBtn: any }}
 */
function createTestHarness() {
  const doc = /** @type {any} */ (new ShimDocument());
  const container = /** @type {any} */ (doc.createElement("div"));
  container.id = "app-root";
  const triggerBtn = /** @type {any} */ (doc.createElement("button"));
  triggerBtn.setAttribute("type", "button");
  triggerBtn.setAttribute("data-action", "open-qr-auth-modal");
  triggerBtn.textContent = "Сканировать QR-код";
  container.appendChild(triggerBtn);
  doc.body.appendChild(container);
  return { doc, container, triggerBtn };
}

describe("TASK-634: Константы RE-оракула Remote Auth v2 и чистые функции валидации", () => {
  it("экспортирует канонические константы модулей 52423 (QR_CHUNK), 4d2a64cb5df5fc4e_0 и TASK-629", () => {
    assert.equal(REMOTE_AUTH_QR_URL_PREFIX, "https://discord.com/ra/");
    assert.equal(REMOTE_AUTH_ROUTE_PATTERN, "/ra/:remoteAuthFingerprint");
    assert.equal(REMOTE_AUTH_WS_URL, "wss://remote-auth.discord.gg/?v=2");
    assert.equal(REMOTE_AUTH_GATEWAY_URL, "wss://remote-auth-gateway.discord.gg/?v=2");
    assert.equal(REMOTE_AUTH_PROTOCOL_VERSION, 2);
    assert.equal(REMOTE_AUTH_FINGERPRINT_LEN, 43);
    assert.equal(DEFAULT_QR_SESSION_TIMEOUT_MS, 120000);
    assert.equal(DEFAULT_QR_SESSION_TIMEOUT_SECONDS, 120);
    assert.equal(REMOTE_AUTH_BACKOFF_MIN_MS, 1500);
    assert.equal(REMOTE_AUTH_BACKOFF_MAX_MS, 30000);

    assert.equal(Object.isFrozen(REMOTE_AUTH_REST_ENDPOINTS), true);
    assert.equal(REMOTE_AUTH_REST_ENDPOINTS.INITIALIZE, "/users/@me/remote-auth");
    assert.equal(REMOTE_AUTH_REST_ENDPOINTS.FINISH, "/users/@me/remote-auth/finish");
    assert.equal(REMOTE_AUTH_REST_ENDPOINTS.CANCEL, "/users/@me/remote-auth/cancel");
    assert.equal(REMOTE_AUTH_REST_ENDPOINTS.LOGIN, "/users/@me/remote-auth/login");

    assert.equal(Object.isFrozen(REMOTE_AUTH_STEPS), true);
    assert.equal(REMOTE_AUTH_STEPS.INITIALIZING, 0);
    assert.equal(REMOTE_AUTH_STEPS.PENDING_REMOTE_INIT, 1);
    assert.equal(REMOTE_AUTH_STEPS.PENDING_TICKET, 2);
    assert.equal(REMOTE_AUTH_STEPS.PENDING_LOGIN, 3);
    assert.equal(REMOTE_AUTH_STEPS.FINISH, 4);

    assert.deepEqual(QR_AUTH_IPC_COMMANDS, [
      "remote_auth_initialize",
      "remote_auth_finish",
      "remote_auth_cancel",
      "remote_auth_start_session",
      "remote_auth_process_frame",
      "remote_auth_heartbeat_tick",
      "remote_auth_cancel_session",
    ]);

    assert.equal(Object.isFrozen(QR_AUTH_MODAL_STATES), true);
    for (const st of ["closed", "pending", "confirming", "submitting", "approved", "rejected", "expired"]) {
      assert.equal(QR_AUTH_MODAL_STATES.includes(st), true, `ожидалось состояние ${st}`);
    }

    assert.equal(Object.isFrozen(QR_AUTH_SECURITY_LEVELS), true);
    assert.equal(Object.isFrozen(QR_AUTH_TEXT), true);
  });

  it("parseQrAuthUrl, verifyFingerprintMatch и formatFingerprintDigest разбирают QR-ссылки и сверяют отпечаток SPKI", () => {
    const parsedUrl = parseQrAuthUrl(`https://discord.com/ra/${SAMPLE_FINGERPRINT}`);
    assert.equal(parsedUrl.valid, true);
    assert.equal(parsedUrl.fingerprint, SAMPLE_FINGERPRINT);
    assert.equal(parsedUrl.qrUrl, `https://discord.com/ra/${SAMPLE_FINGERPRINT}`);
    assert.equal(parsedUrl.canonicalLength, true);

    const parsedDeepLink = parseQrAuthUrl(`discord://ra/${SAMPLE_FINGERPRINT}`);
    assert.equal(parsedDeepLink.valid, true);
    assert.equal(parsedDeepLink.fingerprint, SAMPLE_FINGERPRINT);

    const parsedRaw = parseQrAuthUrl(SAMPLE_FINGERPRINT);
    assert.equal(parsedRaw.valid, true);
    assert.equal(parsedRaw.fingerprint, SAMPLE_FINGERPRINT);

    assert.equal(parseQrAuthUrl("").valid, false);
    assert.equal(parseQrAuthUrl(null).valid, false);
    assert.equal(parseQrAuthUrl("https://evil.example.com/ra/" + SAMPLE_FINGERPRINT).valid, false);
    assert.equal(parseQrAuthUrl("short_fp").valid, false);
    assert.equal(parseQrAuthUrl("bad$chars!in!fingerprint!12345678901234567890").valid, false);

    const matchOk = verifyFingerprintMatch(SAMPLE_FINGERPRINT, `https://discord.com/ra/${SAMPLE_FINGERPRINT}`);
    assert.equal(matchOk.matched, true);
    assert.equal(matchOk.normalizedExpected, SAMPLE_FINGERPRINT);

    const matchFail = verifyFingerprintMatch(SAMPLE_FINGERPRINT, OTHER_FINGERPRINT);
    assert.equal(matchFail.matched, false);

    const matchEmpty = verifyFingerprintMatch("", SAMPLE_FINGERPRINT);
    assert.equal(matchEmpty.matched, false);

    const digest = formatFingerprintDigest(SAMPLE_FINGERPRINT);
    assert.equal(typeof digest.short, "string");
    assert.equal(typeof digest.grouped, "string");
    assert.ok(digest.short.includes("unglv48B"));
    assert.ok(digest.short.includes("AFa0"));
    assert.ok(digest.grouped.length > SAMPLE_FINGERPRINT.length);
  });

  it("validateDeviceIpAddress и formatDeviceClientInfo валидируют IPv4/IPv6 и форматируют ОС, браузер и локацию", () => {
    const pubIp4 = validateDeviceIpAddress("89.208.102.45");
    assert.equal(pubIp4.valid, true);
    assert.equal(pubIp4.version, 4);
    assert.equal(pubIp4.isPrivateOrLoopback, false);

    const privIp4 = validateDeviceIpAddress("192.168.1.10");
    assert.equal(privIp4.valid, true);
    assert.equal(privIp4.version, 4);
    assert.equal(privIp4.isPrivateOrLoopback, true);

    const loopIp4 = validateDeviceIpAddress("127.0.0.1");
    assert.equal(loopIp4.valid, true);
    assert.equal(loopIp4.isPrivateOrLoopback, true);

    const pubIp6 = validateDeviceIpAddress("2a00:1450:4001:82f::200e");
    assert.equal(pubIp6.valid, true);
    assert.equal(pubIp6.version, 6);
    assert.equal(pubIp6.isPrivateOrLoopback, false);

    const loopIp6 = validateDeviceIpAddress("::1");
    assert.equal(loopIp6.valid, true);
    assert.equal(loopIp6.version, 6);
    assert.equal(loopIp6.isPrivateOrLoopback, true);

    assert.equal(validateDeviceIpAddress("999.1.1.1").valid, false);
    assert.equal(validateDeviceIpAddress("1.2.3.256").valid, false);
    assert.equal(validateDeviceIpAddress("01.2.3.4").valid, false);
    assert.equal(validateDeviceIpAddress("").valid, false);
    assert.equal(validateDeviceIpAddress("not-an-ip").valid, false);

    const desktopInfo = formatDeviceClientInfo({
      os: "Linux",
      platform: "Discord Desktop",
      ip: "89.208.102.45",
      location: "Москва, Россия",
    });
    assert.equal(desktopInfo.os, "Linux");
    assert.equal(desktopInfo.platform, "Discord Desktop");
    assert.equal(desktopInfo.deviceIconType, "desktop");
    assert.equal(desktopInfo.headline, "Linux • Discord Desktop");
    assert.equal(desktopInfo.ip, "89.208.102.45");
    assert.equal(desktopInfo.location, "Москва, Россия");
    assert.match(desktopInfo.locationLine, /Москва, Россия/);
    assert.match(desktopInfo.locationLine, /89\.208\.102\.45/);

    const mobileInfo = formatDeviceClientInfo({
      os: "iOS",
      platform: "Discord iOS",
      ip: "89.208.102.45",
      location: "Санкт-Петербург, Россия",
    });
    assert.equal(mobileInfo.deviceIconType, "mobile");

    const vrInfo = formatDeviceClientInfo({
      os: "Horizon OS",
      platform: "Discord VR",
    });
    assert.equal(vrInfo.deviceIconType, "vr");

    const unknownInfo = formatDeviceClientInfo({});
    assert.equal(unknownInfo.os, "Неизвестно");
    assert.equal(unknownInfo.deviceIconType, "desktop");
  });

  it("evaluateQrSecurityIndicator вычисляет уровень безопасности verified / warning / danger", () => {
    const verified = evaluateQrSecurityIndicator({
      expectedFingerprint: SAMPLE_FINGERPRINT,
      actualFingerprint: SAMPLE_FINGERPRINT,
      ip: "89.208.102.45",
      os: "Windows",
      platform: "Firefox",
      location: "Москва, Россия",
    });
    assert.equal(verified.level, "verified");
    assert.equal(verified.canApprove, true);

    const warningPrivateIp = evaluateQrSecurityIndicator({
      expectedFingerprint: SAMPLE_FINGERPRINT,
      actualFingerprint: SAMPLE_FINGERPRINT,
      ip: "10.0.0.15",
      os: "Linux",
      platform: "Discord Desktop",
      location: "Локальная сеть",
    });
    assert.equal(warningPrivateIp.level, "warning");
    assert.equal(warningPrivateIp.canApprove, true);

    const dangerMitm = evaluateQrSecurityIndicator({
      expectedFingerprint: SAMPLE_FINGERPRINT,
      actualFingerprint: OTHER_FINGERPRINT,
      ip: "89.208.102.45",
      os: "Windows",
      platform: "Chrome",
      location: "Москва, Россия",
    });
    assert.equal(dangerMitm.level, "danger");
    assert.equal(dangerMitm.canApprove, false);

    const dangerInvalidIp = evaluateQrSecurityIndicator({
      expectedFingerprint: SAMPLE_FINGERPRINT,
      actualFingerprint: SAMPLE_FINGERPRINT,
      ip: "999.999.999.999",
      os: "Windows",
      platform: "Chrome",
      location: "Москва, Россия",
    });
    assert.equal(dangerInvalidIp.level, "danger");
    assert.equal(dangerInvalidIp.canApprove, false);
  });

  it("parseRemoteAuthUserPayload, isValidSnowflake, clampSessionTimeoutMs и classifyQrAuthError обрабатывают граничные случаи", () => {
    assert.equal(isValidSnowflake("1"), true);
    assert.equal(isValidSnowflake("18446744073709551615"), true);
    assert.equal(isValidSnowflake("0"), false);
    assert.equal(isValidSnowflake("18446744073709551616"), false);

    const userWithAvatar = parseRemoteAuthUserPayload(
      "1301880740245405707:0:a_1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d:sanya_discord",
    );
    assert.ok(userWithAvatar);
    assert.equal(userWithAvatar.id, "1301880740245405707");
    assert.equal(userWithAvatar.discriminator, "0");
    assert.equal(userWithAvatar.avatar, "a_1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d");
    assert.equal(userWithAvatar.username, "sanya_discord");
    assert.equal(userWithAvatar.tag, "sanya_discord");

    const userZeroAvatar = parseRemoteAuthUserPayload("1301880740245405707:0421:0:legacy_user");
    assert.ok(userZeroAvatar);
    assert.equal(userZeroAvatar.avatar, null);
    assert.equal(userZeroAvatar.tag, "legacy_user#0421");

    assert.equal(parseRemoteAuthUserPayload("0:0:0:invalid_snowflake"), null);
    assert.equal(parseRemoteAuthUserPayload("malformed"), null);

    assert.equal(clampSessionTimeoutMs(undefined), 120000);
    assert.equal(clampSessionTimeoutMs(1000), 5000);
    assert.equal(clampSessionTimeoutMs(900000), 300000);
    assert.equal(clampSessionTimeoutMs(90000), 90000);

    const err429 = classifyQrAuthError({ kind: "rateLimited", retry_after_ms: 4100 });
    assert.equal(err429.state, "error-429");
    assert.equal(err429.retryAfterSeconds, 5);

    const err403 = classifyQrAuthError({ status: 403 });
    assert.equal(err403.state, "error-403");

    const err400 = classifyQrAuthError({ status: 400 });
    assert.equal(err400.state, "error-400");

    const errNet = classifyQrAuthError({ kind: "network" });
    assert.equal(errNet.state, "error-network");
  });
});

describe("TASK-634: Интерактивный контроллер модалки #qr-auth-modal, машина состояний и A11y", () => {
  it("проходит цикл pending -> confirming -> approved, отображает fingerprint/ОС/браузер/IP/город и требует подтверждения IP", async () => {
    const { doc, triggerBtn } = createTestHarness();
    /** @type {Array<{ cmd: string, args: any }>} */
    const ipcCalls = [];
    /** @type {Array<any>} */
    const approvedEvents = [];

    /** @type {(v: any) => void} */
    let resolveFinish = () => {};

    const modal = createQrAuthModal({
      document: /** @type {any} */ (doc),
      triggerButton: triggerBtn,
      onApproved: (info) => {
        approvedEvents.push(info);
      },
      invoke: (cmd, args) => {
        ipcCalls.push({ cmd, args });
        if (cmd === "remote_auth_initialize") {
          return Promise.resolve({
            handshake_token: "hs_token_live_9001",
            fingerprint: SAMPLE_FINGERPRINT,
            client_info: {
              os: "Linux",
              platform: "Discord Desktop",
              ip: "89.208.102.45",
              location: "Москва, Россия",
            },
            user: {
              id: "1301880740245405707",
              username: "sanya_discord",
              discriminator: "0",
            },
            timeout_ms: 120000,
          });
        }
        if (cmd === "remote_auth_finish") {
          return new Promise((resolve) => {
            resolveFinish = resolve;
          });
        }
        return Promise.resolve({});
      },
    });

    assert.equal(triggerBtn.getAttribute("aria-haspopup"), "dialog");
    assert.equal(triggerBtn.getAttribute("aria-controls"), "qr-auth-modal");
    assert.equal(triggerBtn.getAttribute("aria-expanded"), "false");

    const host = doc.getElementById("qr-auth-modal");
    assert.ok(host, "#qr-auth-modal должен быть смонтирован в DOM");
    assert.equal(host.hidden, true);

    // Открываем модалку со сканированной QR-ссылкой
    const openPromise = modal.open({
      qrUrl: `https://discord.com/ra/${SAMPLE_FINGERPRINT}`,
    });
    // Сразу после вызова open() до резолва initialize состояние "pending"
    const card = host.querySelector(".qr-auth-modal-card");
    assert.equal(card.getAttribute("role"), "dialog");
    assert.equal(card.getAttribute("aria-modal"), "true");

    await openPromise;
    assert.equal(host.hidden, false);
    assert.equal(triggerBtn.getAttribute("aria-expanded"), "true");
    assert.equal(card.getAttribute("data-state"), "confirming");
    assert.equal(ipcCalls[0]?.cmd, "remote_auth_initialize");
    assert.equal(ipcCalls[0]?.args.fingerprint, SAMPLE_FINGERPRINT);

    // Проверяем отображение браузера/клиента, ОС, IP, города и отпечатка
    const osBrowserEl = host.querySelector("#qr-auth-device-os-browser");
    const ipEl = host.querySelector("#qr-auth-ip");
    const locationEl = host.querySelector("#qr-auth-location");
    const fpEl = host.querySelector("#qr-auth-fingerprint");
    const secBanner = host.querySelector("#qr-auth-security-banner");
    const timerEl = host.querySelector("#qr-auth-timer");

    assert.equal(osBrowserEl?.textContent, "Linux • Discord Desktop");
    assert.equal(ipEl?.textContent, "89.208.102.45");
    assert.equal(locationEl?.textContent, "Москва, Россия");
    assert.equal(fpEl?.getAttribute("data-fingerprint"), SAMPLE_FINGERPRINT);
    assert.equal(secBanner?.getAttribute("data-security-level"), "verified");
    assert.match(timerEl?.textContent || "", /120/);

    // Попытка нажать «Войти» без отметки подтверждения IP блокируется
    const approveBtn = host.querySelector("#qr-auth-approve");
    const errorEl = host.querySelector("#qr-auth-error");
    await modal.approve();
    assert.equal(card.getAttribute("data-state"), "confirming");
    assert.equal(errorEl.hidden, false);
    assert.equal(errorEl.textContent, QR_AUTH_TEXT.ipConfirmRequired);
    assert.equal(ipcCalls.filter((c) => c.cmd === "remote_auth_finish").length, 0);

    // Отмечаем чекбокс подтверждения IP и чекбокс временной сессии
    const ipCheckbox = host.querySelector("#qr-auth-confirm-ip");
    const tempCheckbox = host.querySelector("#qr-auth-temporary-token");
    ipCheckbox.checked = true;
    ipCheckbox.dispatchEvent("change");
    tempCheckbox.checked = true;
    tempCheckbox.dispatchEvent("change");

    assert.equal(modal.getState().ipConfirmed, true);
    assert.equal(modal.getState().temporaryToken, true);

    // Нажимаем «Войти» дважды (защита от двойного клика в состоянии submitting)
    approveBtn.dispatchEvent("click");
    approveBtn.dispatchEvent("click");

    assert.equal(card.getAttribute("data-state"), "submitting");
    assert.equal(approveBtn.disabled, true);
    const finishCalls = ipcCalls.filter((c) => c.cmd === "remote_auth_finish");
    assert.equal(finishCalls.length, 1);
    assert.equal(finishCalls[0]?.args.handshake_token, "hs_token_live_9001");
    assert.equal(finishCalls[0]?.args.temporary_token, true);

    // Во время submitting Escape и клик по крестику не закрывают окно
    host.dispatchEvent("keydown", { key: "Escape" });
    host.querySelector('[data-action="qr-auth-close"]').dispatchEvent("click");
    assert.equal(host.hidden, false);
    assert.equal(card.getAttribute("data-state"), "submitting");

    // Завершаем подтверждение
    resolveFinish({});
    await modal.whenIdle();

    assert.equal(card.getAttribute("data-state"), "approved");
    const statusEl = host.querySelector("#qr-auth-status");
    assert.equal(statusEl.hidden, false);
    assert.equal(statusEl.textContent, QR_AUTH_TEXT.approvedMessage);
    assert.equal(approvedEvents.length, 1);
    assert.equal(approvedEvents[0]?.handshakeToken, "hs_token_live_9001");

    modal.destroy();
  });

  it("выполняет отмену входа («Отклонить» -> rejected), истечение таймера сессии (expired) и блокировку MITM", async () => {
    const { doc, triggerBtn } = createTestHarness();
    /** @type {Array<{ cmd: string, args: any }>} */
    const ipcCalls = [];
    /** @type {Array<any>} */
    const rejectedEvents = [];

    const modal = createQrAuthModal({
      document: /** @type {any} */ (doc),
      triggerButton: triggerBtn,
      onRejected: (info) => {
        rejectedEvents.push(info);
      },
      invoke: async (cmd, args) => {
        ipcCalls.push({ cmd, args });
        if (cmd === "remote_auth_initialize") {
          return {
            handshake_token: "hs_token_cancel_77",
            fingerprint: SAMPLE_FINGERPRINT,
            client_info: {
              os: "Windows",
              platform: "Chrome",
              ip: "93.184.216.34",
              location: "Санкт-Петербург, Россия",
            },
            timeout_ms: 10000,
          };
        }
        return {};
      },
    });

    await modal.open({ fingerprint: SAMPLE_FINGERPRINT });
    const host = doc.getElementById("qr-auth-modal");
    const card = host.querySelector(".qr-auth-modal-card");
    assert.equal(card.getAttribute("data-state"), "confirming");
    assert.equal(modal.getState().remainingSeconds, 10);

    // Нажимаем кнопку «Отклонить» (#qr-auth-reject)
    const rejectBtn = host.querySelector("#qr-auth-reject");
    rejectBtn.dispatchEvent("click");
    await modal.whenIdle();

    assert.equal(card.getAttribute("data-state"), "rejected");
    const cancelCalls = ipcCalls.filter((c) => c.cmd === "remote_auth_cancel");
    assert.equal(cancelCalls.length, 1);
    assert.equal(cancelCalls[0]?.args.handshake_token, "hs_token_cancel_77");
    assert.equal(rejectedEvents.length, 1);
    assert.equal(host.querySelector("#qr-auth-status").textContent, QR_AUTH_TEXT.rejectedMessage);

    // Закрываем и проверяем истечение времени сессии (expired)
    modal.close();
    await modal.open({ fingerprint: SAMPLE_FINGERPRINT });
    assert.equal(card.getAttribute("data-state"), "confirming");
    assert.equal(modal.getState().remainingSeconds, 10);

    modal.tickSessionSecond(5);
    assert.equal(modal.getState().remainingSeconds, 5);
    assert.equal(card.getAttribute("data-state"), "confirming");

    modal.tickSessionSecond(5);
    assert.equal(modal.getState().remainingSeconds, 0);
    assert.equal(card.getAttribute("data-state"), "expired");
    assert.equal(host.querySelector("#qr-auth-approve").disabled, true);
    assert.equal(host.querySelector("#qr-auth-status").textContent, QR_AUTH_TEXT.expiredMessage);

    modal.destroy();
  });

  it("блокирует подтверждение при MITM-несовпадении отпечатка и поддерживает фокус-ловушку Tab/Enter/Escape и ошибки 429/403/400/network", async () => {
    const { doc, triggerBtn } = createTestHarness();
    /** @type {any} */
    let finishError = null;
    /** @type {string} */
    let serverReturnedFingerprint = OTHER_FINGERPRINT; // MITM mismatch сначала

    const modal = createQrAuthModal({
      document: /** @type {any} */ (doc),
      triggerButton: triggerBtn,
      invoke: async (cmd) => {
        if (cmd === "remote_auth_initialize") {
          return {
            handshake_token: "hs_token_sec_55",
            fingerprint: serverReturnedFingerprint,
            client_info: {
              os: "Windows",
              platform: "Firefox",
              ip: "89.208.102.45",
              location: "Екатеринбург, Россия",
            },
          };
        }
        if (cmd === "remote_auth_finish") {
          if (finishError) throw finishError;
          return {};
        }
        return {};
      },
    });

    // 1. MITM-несовпадение отпечатка
    await modal.open({ fingerprint: SAMPLE_FINGERPRINT });
    const host = doc.getElementById("qr-auth-modal");
    const card = host.querySelector(".qr-auth-modal-card");
    const secBanner = host.querySelector("#qr-auth-security-banner");
    const errorEl = host.querySelector("#qr-auth-error");

    assert.equal(secBanner.getAttribute("data-security-level"), "danger");
    modal.setIpConfirmed(true);
    await modal.approve();
    assert.equal(card.getAttribute("data-state"), "confirming");
    assert.equal(errorEl.hidden, false);
    assert.equal(errorEl.textContent, QR_AUTH_TEXT.mitmError);

    // 2. Корректный отпечаток: проверяем ошибки 429, 403, 400, network и клавиши Tab / Enter / Escape
    modal.close();
    serverReturnedFingerprint = SAMPLE_FINGERPRINT;
    await modal.open({ fingerprint: SAMPLE_FINGERPRINT });
    assert.equal(secBanner.getAttribute("data-security-level"), "verified");
    modal.setIpConfirmed(true);

    // 429 Rate Limit
    finishError = { kind: "rateLimited", retry_after_ms: 2000 };
    await modal.approve();
    assert.equal(card.getAttribute("data-state"), "error-429");
    assert.equal(errorEl.textContent, QR_AUTH_TEXT.rateLimited(2));
    modal.tickRateLimitSecond();
    assert.equal(errorEl.textContent, QR_AUTH_TEXT.rateLimited(1));
    modal.tickRateLimitSecond();
    assert.equal(card.getAttribute("data-state"), "confirming");

    // 403 Forbidden
    finishError = { status: 403 };
    await modal.approve();
    assert.equal(card.getAttribute("data-state"), "error-403");
    assert.equal(errorEl.textContent, QR_AUTH_TEXT.forbidden);

    // 400 Bad Request
    finishError = { status: 400 };
    await modal.approve();
    assert.equal(card.getAttribute("data-state"), "error-400");
    assert.equal(errorEl.textContent, QR_AUTH_TEXT.badRequest);

    // Network Error -> затем успешное подтверждение по клавише Enter
    finishError = { kind: "network" };
    await modal.approve();
    assert.equal(card.getAttribute("data-state"), "error-network");
    assert.equal(errorEl.textContent, QR_AUTH_TEXT.network);

    // Фокус-ловушка Tab / Shift+Tab
    const closeBtn = host.querySelector('[data-action="qr-auth-close"]');
    const approveBtn = host.querySelector("#qr-auth-approve");
    closeBtn.focus();
    card.dispatchEvent("keydown", {
      key: "Tab",
      shiftKey: true,
      preventDefault: () => {},
    });
    assert.equal(doc.activeElement, approveBtn);

    approveBtn.focus();
    card.dispatchEvent("keydown", {
      key: "Tab",
      shiftKey: false,
      preventDefault: () => {},
    });
    assert.equal(doc.activeElement, closeBtn);

    // Нажатие Enter на карточке подтверждает вход
    finishError = null;
    card.dispatchEvent("keydown", {
      key: "Enter",
      preventDefault: () => {},
    });
    await modal.whenIdle();
    assert.equal(card.getAttribute("data-state"), "approved");

    // Закрытие по Escape возвращает фокус на триггер
    host.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(host.hidden, true);
    assert.equal(triggerBtn.getAttribute("aria-expanded"), "false");
    assert.equal(doc.activeElement, triggerBtn);

    modal.destroy();
  });
});

describe("TASK-634: Контракт стилей qr-auth-modal.css и безопасность кода", () => {
  it("CSS-файл styles/qr-auth-modal.css использует только токены из tokens.css без сырых цветов, :has() и !important", () => {
    assert.equal(
      existsSync(QR_AUTH_MODAL_CSS_PATH),
      true,
      "файл rust/gui-spike/ui/styles/qr-auth-modal.css должен существовать",
    );
    const css = readFileSync(QR_AUTH_MODAL_CSS_PATH, "utf8");
    const tokensCss = readFileSync(TOKENS_CSS_PATH, "utf8");

    for (const selector of [
      "#qr-auth-modal",
      ".qr-auth-modal-card",
      ".qr-auth-head",
      ".qr-auth-close",
      ".qr-auth-security-banner",
      ".qr-auth-device-card",
      ".qr-auth-fingerprint",
      ".qr-auth-meta-grid",
      ".qr-auth-checkbox-row",
      ".qr-auth-foot",
      "#qr-auth-approve",
      "#qr-auth-reject",
    ]) {
      assert.equal(
        css.includes(selector),
        true,
        `ожидался селектор ${selector} в qr-auth-modal.css`,
      );
    }

    assert.equal(css.includes("!important"), false, "в qr-auth-modal.css запрещён !important");
    assert.equal(/:has\(/i.test(css), false, "в qr-auth-modal.css запрещён :has()");
    assert.equal(
      /#[0-9a-fA-F]{3,8}\b/.test(css),
      false,
      "в qr-auth-modal.css запрещены сырые hex-цвета",
    );
    assert.equal(
      /\b(?:rgb|rgba|hsl|hsla)\s*\(/.test(css),
      false,
      "в qr-auth-modal.css запрещены сырые rgb/hsl-цвета",
    );
    assert.equal(
      /url\(|@import|https?:\/\//.test(css),
      false,
      "в qr-auth-modal.css запрещены внешние ассеты",
    );

    const declaredTokens = new Set(
      Array.from(tokensCss.matchAll(/(--[a-zA-Z0-9_-]+)\s*:/g)).map((m) => m[1]),
    );
    const usedTokens = Array.from(css.matchAll(/var\(\s*(--[a-zA-Z0-9_-]+)/g)).map((m) => m[1]);
    assert.ok(usedTokens.length >= 12, "qr-auth-modal.css должен использовать токены из tokens.css");
    for (const token of usedTokens) {
      assert.equal(
        declaredTokens.has(token),
        true,
        `токен ${token} не объявлен в rust/gui-spike/ui/tokens.css`,
      );
    }
  });

  it("код qr-auth-modal.js не вызывает запрещённые нативные/медиа API и не пишет в console", () => {
    const js = readFileSync(QR_AUTH_MODAL_JS_PATH, "utf8");
    const forbidden = [
      "navigator.mediaDevices",
      "getUserMedia",
      "getDisplayMedia",
      "startRecordingRawSamples",
      "startLocalAudioRecording",
      "setAecDump",
      "getScreenPreviews",
      "getWindowPreviews",
    ];
    for (const needle of forbidden) {
      assert.equal(js.includes(needle), false, `qr-auth-modal.js содержит запрещённое «${needle}»`);
    }
    assert.equal(/console\.(log|warn|error)/.test(js), false, "qr-auth-modal.js пишет в console");
  });
});
