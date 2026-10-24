// @ts-check
// RW-39 / NET-D2: окно входа по LOGIN-SPEC (AUTH-CONTRACT §1/§2/§4) на настоящем
// `ui/app.js` в DOM-шиме. Проверяются: 7 состояний (`checking`, `no-session`,
// `validating`, `captcha`, `mfa`, `ready`, `error`), раскладка 784/416/240/64/32,
// QR-панель (inline-SVG + честный disabled), вход по логину/паролю и токену,
// captcha только через интерактивный adapter, MFA (totp/sms/backup/password),
// ошибки 401/429/captcha/сеть без секретов в DOM и логах.
//
// Независимые оракулы: `waves/RW-39/NET/LOGIN-SPEC.md` (числа 784/480/416/240/64/32/830,
// тексты §7/§9), `waves/RW-39/NET/AUTH-CONTRACT.md` (id, состояния, LoginOutcome),
// `rust/gui-spike/src/auth.rs` (JSON-форма `LoginView`/`AuthError`), `ui/index.html`.
// Ответы IPC задаёт тест — реального invoke/сети/окна нет.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { GuiDom, ShimElement } from "../support/gui-dom-harness.mjs";

// Типы приходят из импортированных классов (GuiDom/ShimElement).

const uiDir = fileURLToPath(new URL("../../rust/gui-spike/ui/", import.meta.url));
const appPath = `${uiDir}app.js`;
const html = readFileSync(`${uiDir}index.html`, "utf8");
const authJs = readFileSync(`${uiDir}features/auth.js`, "utf8");
const authCss = readFileSync(`${uiDir}components/auth.css`, "utf8");

/** Сырой user-токен в формате Discord (три base64-сегмента), заведомо фейковый. */
const TOKEN = "MTk4NjIyNDgzNDcxOTI1MjQ4.Cl2FMQ.fake-user-token-DO-NOT-LEAK";
/** Пароль и коды — фейковые, проверяем, что они не остаются в DOM. */
const PASSWORD = "fake-password-DO-NOT-LEAK";
const TOTP = "123456";
const SMS_CODE = "654321";
const BACKUP = "ABCD-EFGH";

/** Ответ `/users/@me` в форме `AuthUser` (netc2 REPORT §1). */
const USER = {
  id: "42",
  username: "tester",
  discriminator: "0",
  global_name: "Тестер",
  avatar: null,
  bot: false,
};

const READY = { status: "ready", stored: true, user: USER, error: null };
const IDLE_STORED = { status: "idle", stored: true, user: null, error: null };
const IDLE_EMPTY = { status: "idle", stored: false, user: null, error: null };

/** `LoginView` из `rust/gui-spike/src/auth.rs` (serde tag=kind). */
const VIEW_READY = { kind: "ready", user: USER };
const VIEW_MFA = {
  kind: "mfa",
  ticket: "ticket-777",
  methods: ["totp", "sms", "backup", "password"],
  login_instance_id: "instance-1",
};
const VIEW_MFA_ONE = {
  kind: "mfa",
  ticket: "ticket-777",
  methods: ["totp"],
  login_instance_id: null,
};
const VIEW_CAPTCHA = {
  kind: "captcha",
  sitekey: "site-key-1",
  service: "hcaptcha",
  rqtoken: "rq-1",
};

// DOM-шим не реализует focus(): подменяем один раз на весь файл и смотрим журнал.
/** @type {ShimElement[]} */
const focusLog = [];
/** @type {any} */ (ShimElement.prototype).focus = function focus() {
  focusLog.push(this);
};

/**
 * @template T
 * @returns {{ promise: Promise<T>, resolve: (value: T) => void, reject: (reason: unknown) => void }}
 */
function deferred() {
  /** @type {(value: T) => void} */
  let resolve = () => {};
  /** @type {(reason: unknown) => void} */
  let reject = () => {};
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/**
 * Ответы фейкового `__TAURI__.core.invoke` для auth-команд.
 * @typedef {{
 *   status?: any | ((args: any) => any),
 *   login?: any | ((args: any) => any),
 *   password?: any | ((args: any) => any),
 *   mfa?: any | ((args: any) => any),
 *   sms?: any | ((args: any) => any),
 *   restore?: any | ((args: any) => any),
 *   logout?: any | ((args: any) => any),
 * }} AuthScript
 * @typedef {{ cmd: string, args: any }} AuthCall
 * @typedef {(container: ShimElement, ctx: any) => any} FakeCaptcha
 */

let importCounter = 0;

/**
 * Своя загрузка app.js: auth-команды отвечают по сценарию теста, остальные —
 * штатной заглушкой харнесса. `captcha` подставляется как хост-адаптер
 * (`window.__AUTH_CAPTCHA__`), официальный виджет в тестах не грузится.
 * @param {AuthScript} [script]
 * @param {{ captcha?: FakeCaptcha }} [envOptions]
 * @returns {Promise<{ env: GuiDom, authCalls: AuthCall[] }>}
 */
async function loadAuthApp(script = {}, envOptions = {}) {
  const env = new GuiDom({ appPath });
  /** @type {AuthCall[]} */
  const authCalls = [];
  /** @type {Record<string, any>} */
  const handlers = {
    auth_status: script.status ?? IDLE_STORED,
    auth_login_token: script.login ?? READY,
    auth_login_password: script.password ?? VIEW_READY,
    auth_mfa: script.mfa ?? VIEW_READY,
    auth_sms: script.sms ?? { phone: "+7 *** ** ** 12" },
    auth_restore: script.restore ?? READY,
    auth_logout: script.logout ?? IDLE_EMPTY,
  };
  const passthrough = env.invoke.bind(env);
  /** @type {any} */ (env).invoke = async (/** @type {string} */ cmd, /** @type {any} */ args) => {
    if (Object.hasOwn(handlers, cmd)) {
      authCalls.push({ cmd, args: args ?? null });
      const response = handlers[cmd];
      return typeof response === "function" ? await response(args ?? null) : response;
    }
    return passthrough(cmd, args);
  };
  if (envOptions.captcha) {
    /** @type {any} */ (env.window).__AUTH_CAPTCHA__ = { mount: envOptions.captcha };
  }
  env.installGlobals();
  // Query-строка — новый экземпляр модуля: состояние app.js не утекает между кейсами.
  env.appModule = await import(`${pathToFileURL(appPath).href}?auth=${++importCounter}`);
  await env.settle(30);
  return { env, authCalls };
}

/**
 * @param {AuthScript} script
 * @param {(scene: { env: GuiDom, authCalls: AuthCall[] }) => void | Promise<void>} body
 * @param {{ captcha?: FakeCaptcha }} [envOptions]
 */
async function withAuth(script, body, envOptions = {}) {
  const scene = await loadAuthApp(script, envOptions);
  try {
    await body(scene);
  } finally {
    scene.env.dispose();
  }
}

/**
 * @param {GuiDom} env
 * @param {string} id
 * @returns {ShimElement}
 */
function byId(env, id) {
  const node = env.document.getElementById(id);
  assert.ok(node, `в DOM нет #${id}`);
  return node;
}

/**
 * @param {GuiDom} env
 * @param {string} selector
 * @returns {ShimElement}
 */
function one(env, selector) {
  const node = env.document.querySelector(selector);
  assert.ok(node, `в DOM нет «${selector}»`);
  return node;
}

/**
 * @param {GuiDom} env
 * @param {string} selector
 * @returns {ShimElement[]}
 */
function all(env, selector) {
  return env.document.querySelectorAll(selector);
}

/**
 * Всё, что видит пользователь/AT и что могло бы утечь: атрибуты (включая
 * data-*), тексты, значения полей. Секретов здесь быть не должно.
 * @param {GuiDom} env
 * @returns {string}
 */
function domDump(env) {
  /** @type {string[]} */
  const parts = [];
  /** @param {ShimElement} node */
  const walk = (node) => {
    parts.push(node.tagName, node.id, node.className, node.title, node.value, node.nodeText);
    for (const [name, value] of node.attributes) parts.push(name, value);
    for (const child of node.children) walk(child);
  };
  walk(env.document.documentElement);
  return parts.join("\n");
}

/**
 * @param {GuiDom} env
 * @param {string} secret
 * @param {string[]} [allowed]
 */
function assertSecretGone(env, secret, allowed = []) {
  assert.equal(domDump(env).includes(secret), false, `секрет остался в DOM: ${secret.slice(0, 6)}…`);
  const logs = [...env.consoleErrors, ...env.consoleWarns].join("\n");
  assert.equal(logs.includes(secret), false, "секрет попал в лог консоли");
  const foreign = env.invokeCalls.filter((call) => !allowed.includes(call.cmd));
  assert.equal(
    JSON.stringify(foreign).includes(secret),
    false,
    "секрет ушёл в чужую IPC-команду",
  );
}

/** @param {GuiDom} env */
function authApi(env) {
  const app = env.appModule?.currentApp?.();
  assert.ok(app?.auth, "currentApp().auth не собран");
  return app.auth;
}

/** @param {GuiDom} env @param {string} id @param {string} text */
function type(env, id, text) {
  const input = /** @type {any} */ (byId(env, id));
  input.value = text;
  input.emit("input");
}

/** @param {GuiDom} env @returns {string} */
function errorText(env) {
  return byId(env, "auth-error").textContent.trim();
}

/** @param {GuiDom} env @returns {string} */
function statusText(env) {
  return byId(env, "auth-status").textContent.trim();
}

/** @param {GuiDom} env @param {string} id */
function click(env, id) {
  byId(env, id).click();
}

/** @param {GuiDom} env @returns {string} */
function state(env) {
  return byId(env, "auth-gate").getAttribute("data-state") ?? "";
}

/** @param {GuiDom} env @param {string} text @param {string} secret */
function loginWithPassword(env, text, secret) {
  type(env, "auth-login", text);
  type(env, "auth-password", secret);
}

describe("auth-гейт: разметка и носитель (AUTH-CONTRACT §4)", () => {
  it("index.html: #auth-gate вне .screen, dialog-семантика, auth.css подключён style.css", () => {
    const tag = html.match(/<div[^>]*id="auth-gate"[^>]*>/)?.[0] ?? "";
    assert.ok(tag, "в index.html нет оверлея #auth-gate");
    assert.match(tag, /class="auth-gate"/, "у оверлея нет класса .auth-gate");
    assert.match(tag, /role="dialog"/, "#auth-gate должен быть dialog");
    assert.match(tag, /aria-modal="true"/, "#auth-gate должен быть модальным");
    assert.match(tag, /\bhidden\b/, "#auth-gate до входа должен быть скрыт через hidden");

    // screenCount остаётся 4: оверлей — не экран роутера.
    const screens = [...html.matchAll(/id="screen-([\w-]+)"/g)].map((match) => match[1] ?? "");
    assert.deepEqual(screens, ["chat", "emoji", "members", "settings"]);
    assert.equal(/<section[^>]*id="auth-gate"/.test(html), false, "#auth-gate не должен быть .screen-секцией");

    const style = readFileSync(`${uiDir}style.css`, "utf8");
    assert.match(style, /@import\s+url\("components\/auth\.css"\);/, "style.css не подключает auth.css");

    assert.match(authCss, /\.auth-gate\s*\{/, "в auth.css нет правила .auth-gate");
    assert.match(authCss, /z-index:\s*var\(--z-modal\)/, "оверлей не на слое --z-modal");
    assert.match(authCss, /\.auth-gate\[hidden\]\s*\{[^}]*display:\s*none/, "скрытый гейт должен не рисоваться");
  });

  it("контракт id: форма, логин/пароль/токен, captcha, MFA, alert, logout", () => {
    const required = [
      "auth-gate",
      "auth-form",
      "auth-login",
      "auth-password",
      "auth-token",
      "auth-captcha",
      "auth-mfa",
      "auth-mfa-code",
      "auth-error",
      "auth-qr",
      "auth-logout",
    ];
    // В HTML живёт только носитель; id есть в рантайм-DOM (шин собирает app.js).
    const missingInSource = required.filter((id) => !html.includes(`id="${id}"`) && !authJs.includes(`"${id}"`));
    assert.deepEqual(missingInSource, [], "id контракта не объявлены ни в html, ни в features/auth.js");
  });

  it("no-session + stored: гейт виден, restore доступен, фокус в логине, auth_status один раз", async () => {
    await withAuth({}, async ({ env, authCalls }) => {
      assert.equal(byId(env, "auth-gate").getAttribute("hidden"), null, "гейт скрыт в no-session");
      assert.equal(byId(env, "auth-login").getAttribute("type"), "text", "поле логина не text");
      assert.equal(byId(env, "auth-login").getAttribute("autocomplete"), "username", "autocomplete логина");
      assert.equal(byId(env, "auth-password").getAttribute("type"), "password", "пароль не password");
      assert.equal(
        byId(env, "auth-password").getAttribute("autocomplete"),
        "current-password",
        "autocomplete пароля",
      );
      assert.equal(byId(env, "auth-token").getAttribute("type"), "password", "поле токена не password");
      assert.equal(byId(env, "auth-restore").getAttribute("hidden"), null, "restore не показан при stored=true");
      assert.equal(byId(env, "auth-submit").getAttribute("aria-disabled"), null, "submit заблокирован в no-session");
      assert.equal(authApi(env).state, "no-session");
      assert.equal(errorText(env), "", "в no-session уже висит ошибка");
      assert.deepEqual(authCalls, [{ cmd: "auth_status", args: null }], "на старте нужен ровно один auth_status");
      assert.ok(
        focusLog.some((node) => node.id === "auth-login"),
        "фокус не переведён в поле логина",
      );
      assert.equal(one(env, "#auth-error").getAttribute("aria-live"), "assertive");
      assert.equal(one(env, "#auth-error").getAttribute("role"), "alert");
    });
  });

  it("no-session без ключа в keyring: restore скрыт (не мёртвая кнопка), submit работает", async () => {
    await withAuth({ status: IDLE_EMPTY }, async ({ env, authCalls }) => {
      assert.equal(byId(env, "auth-gate").getAttribute("hidden"), null);
      assert.equal(
        byId(env, "auth-restore").getAttribute("hidden"),
        "",
        "без stored=true кнопка восстановления не должна быть доступна",
      );
      assert.equal(byId(env, "auth-submit").getAttribute("aria-disabled"), null);
      assert.equal(authCalls.length, 1);
      assert.equal(authCalls[0]?.cmd, "auth_status");
    });
  });

  it("ready на старте: гейт скрыт, чат доступен, имя в user-bar, logout виден", async () => {
    await withAuth({ status: READY }, async ({ env }) => {
      const gate = byId(env, "auth-gate");
      assert.equal(gate.getAttribute("hidden"), "", "гейт не скрыт в ready");
      assert.equal(gate.style.display, "none");
      assert.equal(gate.getAttribute("aria-hidden"), "true");
      assert.equal(authApi(env).state, "ready");

      const active = all(env, ".screen.active");
      assert.equal(active.length, 1, "активных экранов не один");
      assert.equal(active[0]?.id, "screen-chat", "чат не активен после входа");
      assert.equal(one(env, "#composer-input").getAttribute("aria-disabled"), null);

      assert.equal(one(env, ".user-name").textContent, "Тестер", "имя из /users/@me не попало в user-bar");
      const logout = byId(env, "auth-logout");
      assert.equal(logout.getAttribute("hidden"), null, "logout не показан после входа");
      assert.equal(logout.getAttribute("data-action"), "auth-logout", "logout без data-action");
      assert.ok((logout.getAttribute("aria-label") ?? "").length > 0, "у logout нет доступного имени");
      assert.ok((logout.getAttribute("title") ?? "").length > 0, "у logout нет подсказки");
    });
  });
});

describe("auth-гейт: раскладка окна входа (LOGIN-SPEC §1, §4, §11)", () => {
  it("CSS: карточка 480/784 = 32+416+64+240, QR 240, скрытие QR ≤830", () => {
    assert.match(authCss, /\.auth-card\s*\{[^}]*width:\s*480px/, "карточка формы должна быть 480px");
    assert.match(
      authCss,
      /\.auth-card\[data-qr="true"\]\s*\{[^}]*width:\s*784px/,
      "карточка с QR должна быть 784px",
    );
    assert.match(authCss, /\.auth-card\s*\{[^}]*padding:\s*var\(--space-9\)/, "паддинг карточки 32 = --space-9");
    assert.match(authCss, /\.auth-form-col\s*\{[^}]*width:\s*416px/, "колонка формы должна быть 416px");
    assert.match(authCss, /\.auth-qr\s*\{[^}]*width:\s*240px/, "QR-колонка должна быть 240px");
    assert.match(
      authCss,
      /\.auth-body\s*\{[^}]*gap:\s*var\(--space-13\)/,
      "gap формы и QR должен быть 64 = --space-13",
    );
    assert.match(authCss, /@media\s*\(max-width:\s*830px\)/, "нет media ≤830");
    const media = authCss.slice(authCss.indexOf("@media (max-width: 830px)"));
    assert.match(media, /\.auth-qr\s*\{[^}]*display:\s*none/, "≤830 QR должен скрываться");
    assert.match(
      media,
      /\.auth-card\[data-qr="true"\]\s*\{[^}]*width:\s*480px/,
      "≤830 карточка возвращается к 480",
    );
    assert.match(authCss, /\.auth-qr-code\s*\{[^}]*width:\s*176px/, "контейнер QR 176×176");
    assert.match(authCss, /\.auth-input\s*\{[^}]*height:\s*var\(--input-height\)/, "поле 40 = --input-height");
    assert.match(authCss, /\.auth-submit\s*\{[^}]*height:\s*var\(--btn-height-lg\)/, "кнопка 38 = --btn-height-lg");
  });

  it("QR-панель: inline-SVG 160 без внешних ассетов, role=img, честный disabled", async () => {
    await withAuth({}, async ({ env }) => {
      const qr = byId(env, "auth-qr");
      assert.equal(qr.getAttribute("hidden"), null, "QR-панель скрыта в no-session");
      const code = byId(env, "auth-qr-code");
      assert.equal(code.getAttribute("role"), "img", "QR-код должен быть role=img");
      assert.match(code.getAttribute("aria-label") ?? "", /QR-код/i, "у QR нет доступного имени");
      const svg = one(env, "#auth-qr-code svg");
      assert.equal(svg.getAttribute("width"), "160", "QR-код должен быть 160");
      assert.equal(svg.getAttribute("height"), "160", "QR-код должен быть 160");
      assert.equal(svg.getAttribute("aria-hidden"), "true", "декоративный SVG должен быть скрыт от AT");

      for (const id of ["auth-qr-disabled", "auth-passkey-disabled"]) {
        const button = byId(env, id);
        assert.equal(button.getAttribute("aria-disabled"), "true", `#${id} должен быть честно disabled`);
        assert.ok((button.getAttribute("title") ?? "").length > 0, `#${id} disabled без подсказки`);
        assert.ok((button.textContent ?? "").trim().length > 0, `#${id} без подписи`);
      }
      assert.match(byId(env, "auth-qr-disabled").textContent, /QR-вход \(Phase 3\)/);
      // Ассеты не копируем: ни картинок, ни внешних URL.
      assert.equal(/<img\b/i.test(authJs), false, "в auth.js появился <img>");
      assert.equal(/https?:\/\//.test(authCss), false, "в auth.css внешние URL");
      assert.equal(/url\(\s*["']?https?:/.test(authCss), false, "в auth.css внешний url()");
      assert.equal(/(solver|2captcha|anti-captcha|rucaptcha)/i.test(authJs), false, "в auth.js solver/обход");
    });
  });

  it("состояния: checking на старте, затем no-session; QR скрыт вне no-session", async () => {
    const status = deferred();
    await withAuth({ status: () => status.promise }, async ({ env }) => {
      assert.equal(state(env), "checking", "стартовое состояние должно быть checking");
      assert.equal(byId(env, "auth-gate").getAttribute("aria-busy"), "true");
      assert.match(statusText(env), /проверяем сохранённую сессию/i);
      assert.equal(byId(env, "auth-form").getAttribute("hidden"), "", "в checking формы быть не должно");
      assert.equal(byId(env, "auth-qr").getAttribute("hidden"), "", "в checking QR быть не должно");

      status.resolve(IDLE_STORED);
      await env.settle(4);
      assert.equal(state(env), "no-session");
      assert.equal(byId(env, "auth-gate").getAttribute("aria-busy"), "false");
      assert.equal(byId(env, "auth-form").getAttribute("hidden"), null);
      assert.equal(byId(env, "auth-qr").getAttribute("hidden"), null);
      assert.equal(byId(env, "auth-card").getAttribute("data-qr"), "true", "в no-session карточка должна быть широкой");
    });
  });
});

describe("auth-гейт: вход по логину и паролю (AUTH-CONTRACT §2)", () => {
  it("submit: validating, пароль очищен до ответа, invoke(auth_login_password), ready", async () => {
    const reply = deferred();
    await withAuth({ password: () => reply.promise }, async ({ env, authCalls }) => {
      loginWithPassword(env, "  user@example.test  ", PASSWORD);
      click(env, "auth-submit");
      await env.settle(2);

      assert.equal(state(env), "validating");
      assert.equal(byId(env, "auth-gate").getAttribute("aria-busy"), "true");
      assert.equal(byId(env, "auth-submit").getAttribute("aria-disabled"), "true");
      assert.ok((byId(env, "auth-submit").getAttribute("title") ?? "").length > 0);
      assert.equal(byId(env, "auth-login").getAttribute("disabled"), "", "поля не заблокированы на проверке");
      assert.equal(byId(env, "auth-password").value, "", "пароль остался в DOM после submit");
      assert.equal(byId(env, "auth-qr").getAttribute("hidden"), "", "QR не скрыт во время проверки");
      assert.match(statusText(env), /проверяем/i);

      const call = authCalls.find((item) => item.cmd === "auth_login_password");
      assert.ok(call, "auth_login_password не вызван");
      assert.deepEqual(call.args, { login: "user@example.test", password: PASSWORD });
      assert.equal(authCalls.filter((item) => item.cmd === "auth_login_password").length, 1);
      assertSecretGone(env, PASSWORD, ["auth_login_password"]);
      assert.equal(domDump(env).includes(PASSWORD), false, "пароль остался в DOM");

      reply.resolve(VIEW_READY);
      await env.settle(4);
      assert.equal(state(env), "ready");
      assert.equal(byId(env, "auth-gate").getAttribute("hidden"), "");
      assert.equal(one(env, ".user-name").textContent, "Тестер");
      assertSecretGone(env, PASSWORD, ["auth_login_password"]);
    });
  });

  it("пустые поля: подсказка у поля, IPC не вызывается, фокус в проблемном поле", async () => {
    await withAuth({}, async ({ env, authCalls }) => {
      click(env, "auth-submit");
      await env.settle(2);
      assert.match(byId(env, "auth-login-error").textContent, /введите email или телефон/i);
      assert.equal(authCalls.filter((call) => call.cmd === "auth_login_password").length, 0);
      assert.equal(focusLog[focusLog.length - 1]?.id, "auth-login");

      loginWithPassword(env, "user@example.test", "");
      click(env, "auth-submit");
      await env.settle(2);
      assert.match(byId(env, "auth-password-error").textContent, /введите пароль/i);
      assert.equal(authCalls.filter((call) => call.cmd === "auth_login_password").length, 0);
      assert.equal(focusLog[focusLog.length - 1]?.id, "auth-password");
    });
  });

  it("показать/скрыть пароль: type password↔text, aria-pressed и aria-label", async () => {
    await withAuth({}, async ({ env }) => {
      const input = byId(env, "auth-password");
      const reveal = byId(env, "auth-password-reveal");
      assert.equal(input.getAttribute("type"), "password");
      assert.equal(reveal.getAttribute("aria-pressed"), "false");
      assert.match(reveal.getAttribute("aria-label") ?? "", /показать пароль/i);

      reveal.click();
      assert.equal(input.getAttribute("type"), "text", "пароль не показан");
      assert.equal(reveal.getAttribute("aria-pressed"), "true");
      assert.match(reveal.getAttribute("aria-label") ?? "", /скрыть пароль/i);

      reveal.click();
      assert.equal(input.getAttribute("type"), "password", "пароль не скрыт обратно");
      assert.equal(reveal.getAttribute("aria-pressed"), "false");
    });
  });
});

describe("auth-гейт: captcha только интерактивная (LOGIN-SPEC §5)", () => {
  it("kind=captcha: зона показана, поля disabled, adapter монтируется, «Продолжить» ждёт токен", async () => {
    /** @type {any} */
    let captured = null;
    /** @type {any} */
    let widget = null;
    const fake = (/** @type {ShimElement} */ container, /** @type {any} */ ctx) => {
      captured = ctx;
      widget = /** @type {any} */ (envDoc.createElement("button"));
      widget.id = "fake-captcha-widget";
      widget.textContent = "Я не робот";
      widget.addEventListener("click", () => {
        assert.equal(container, byId(envDoc, "auth-captcha-widget"), "виджет смонтирован не в контейнер");
        ctx.onToken("fake-captcha-token");
        widget.textContent = "Готово";
      });
      container.append(widget);
      return { destroy: () => widget.remove() };
    };
    const reply = deferred();
    /** @type {any} */
    let envDoc = null;

    await withAuth(
      {
        password: (/** @type {any} */ args) => (args.captcha_key ? reply.promise : VIEW_CAPTCHA),
      },
      async ({ env, authCalls }) => {
        envDoc = env;
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);

        assert.equal(state(env), "captcha");
        assert.equal(byId(env, "auth-captcha").getAttribute("hidden"), null, "зона captcha не показана");
        assert.equal(byId(env, "auth-captcha").getAttribute("role"), "group");
        assert.match(one(env, "#auth-captcha").getAttribute("aria-labelledby") ?? "", /auth-captcha-title/);
        assert.match(errorText(env), /проверка безопасности/i);
        assert.equal(byId(env, "auth-login").getAttribute("disabled"), "", "поля должны быть disabled в captcha");
        assert.equal(byId(env, "auth-password").value, "", "пароль остался в DOM в captcha");
        assert.ok(captured, "adapter не получил контекст");
        assert.equal(captured.sitekey, "site-key-1");
        assert.equal(byId(env, "auth-captcha-continue").getAttribute("aria-disabled"), "true");
        assert.ok((byId(env, "auth-captcha-continue").getAttribute("title") ?? "").length > 0);
        assert.equal(byId(env, "auth-qr").getAttribute("hidden"), "", "QR не скрыт в captcha");

        widget.click();
        assert.equal(byId(env, "auth-captcha-continue").getAttribute("aria-disabled"), null, "токен не разблокировал «Продолжить»");
        click(env, "auth-captcha-continue");
        await env.settle(2);

        const calls = authCalls.filter((call) => call.cmd === "auth_login_password");
        assert.equal(calls.length, 2, "нужен повторный вход с ключом captcha");
        const secondCall = calls[1];
        assert.ok(secondCall, "второй вызов auth_login_password отсутствует");
        assert.deepEqual(secondCall.args, {
          login: "user@example.test",
          password: PASSWORD,
          captcha_key: "fake-captcha-token",
          captcha_rqtoken: "rq-1",
        });
        assertSecretGone(env, PASSWORD, ["auth_login_password"]);
        assert.equal(domDump(env).includes("fake-captcha-token"), false, "ключ captcha остался в DOM");

        reply.resolve(VIEW_READY);
        await env.settle(4);
        assert.equal(state(env), "ready");
        assert.equal(byId(env, "auth-captcha").getAttribute("hidden"), "", "зона captcha не скрылась");
      },
      { captcha: fake },
    );
  });

  it("kind=captcha: session/rqdata сохраняются в adapter context и retry", async () => {
    /** @type {any} */
    let captured = null;
    const captcha = {
      kind: "captcha",
      sitekey: "site-key-session",
      service: "hcaptcha",
      rqtoken: "rq-session",
      captcha_session_id: "session-123",
      captcha_rqdata: "rqdata-456",
      should_serve_invisible: true,
      user_flow: "login",
    };

    await withAuth(
      {
        password: (/** @type {any} */ args) => args.captcha_key ? VIEW_READY : captcha,
      },
      async ({ env, authCalls }) => {
        const fake = (/** @type {ShimElement} */ _container, /** @type {any} */ ctx) => {
          captured = ctx;
        };
        /** @type {any} */ (env.window).__AUTH_CAPTCHA__ = { mount: fake };
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);

        assert.deepEqual(captured, {
          sitekey: "site-key-session",
          service: "hcaptcha",
          rqtoken: "rq-session",
          captcha_session_id: "session-123",
          captcha_rqdata: "rqdata-456",
          should_serve_invisible: true,
          user_flow: "login",
          onToken: captured.onToken,
        });
        captured.onToken("captcha-session-key");
        click(env, "auth-captcha-continue");
        await env.settle(4);

        const calls = authCalls.filter((call) => call.cmd === "auth_login_password");
        assert.deepEqual(calls[1]?.args, {
          login: "user@example.test",
          password: PASSWORD,
          captcha_key: "captcha-session-key",
          captcha_rqtoken: "rq-session",
          captcha_session_id: "session-123",
          captcha_rqdata: "rqdata-456",
        });
      },
    );
  });

  it("без adapter: placeholder и честный disabled, никаких обходов и вызовов", async () => {
    await withAuth({ password: VIEW_CAPTCHA }, async ({ env, authCalls }) => {
      loginWithPassword(env, "user@example.test", PASSWORD);
      click(env, "auth-submit");
      await env.settle(4);

      assert.equal(state(env), "captcha");
      const widget = byId(env, "auth-captcha-widget");
      assert.ok((widget.textContent ?? "").length > 0, "нет placeholder до загрузки виджета");
      assert.equal(byId(env, "auth-captcha-continue").getAttribute("aria-disabled"), "true");
      const before = authCalls.filter((call) => call.cmd === "auth_login_password").length;
      click(env, "auth-captcha-continue");
      await env.settle(2);
      assert.equal(
        authCalls.filter((call) => call.cmd === "auth_login_password").length,
        before,
        "без ключа captcha повторный вход запрещён",
      );
    });
  });
});

describe("auth-гейт: MFA (LOGIN-SPEC §6)", () => {
  it("kind=mfa: список методов в серверном порядке, QR и форма скрыты", async () => {
    await withAuth({ password: VIEW_MFA }, async ({ env }) => {
      loginWithPassword(env, "user@example.test", PASSWORD);
      click(env, "auth-submit");
      await env.settle(4);

      assert.equal(state(env), "mfa");
      assert.equal(byId(env, "auth-mfa").getAttribute("hidden"), null, "MFA-экран не показан");
      assert.equal(byId(env, "auth-form").getAttribute("hidden"), "", "форма не скрыта в MFA");
      assert.equal(byId(env, "auth-qr").getAttribute("hidden"), "", "QR не скрыт в MFA");
      assert.equal(byId(env, "auth-card").getAttribute("data-qr"), "false", "карточка должна сузиться до 480");
      assert.equal(byId(env, "auth-mfa-methods").getAttribute("hidden"), null);
      const methods = all(env, "#auth-mfa-methods button").map((node) => node.getAttribute("data-method"));
      assert.deepEqual(methods, ["totp", "sms", "backup", "password"], "порядок методов не серверный");
      assert.equal(byId(env, "auth-mfa-code").getAttribute("hidden"), "", "поле кода видно до выбора метода");
      assert.equal(byId(env, "auth-mfa-other").getAttribute("hidden"), "", "«другой способ» виден в списке");
    });
  });

  it("один метод: слайд открывается сразу, без списка", async () => {
    await withAuth({ password: VIEW_MFA_ONE }, async ({ env }) => {
      loginWithPassword(env, "user@example.test", PASSWORD);
      click(env, "auth-submit");
      await env.settle(4);

      assert.equal(state(env), "mfa");
      assert.equal(byId(env, "auth-mfa-methods").getAttribute("hidden"), "", "список методов не скрыт");
      assert.equal(byId(env, "auth-mfa-form").getAttribute("hidden"), null, "слайд кода не открыт");
      assert.equal(byId(env, "auth-mfa-code").getAttribute("maxlength"), "6");
      assert.equal(byId(env, "auth-mfa-code").getAttribute("inputmode"), "numeric");
      assert.equal(byId(env, "auth-mfa-code").getAttribute("autocomplete"), "one-time-code");
      assert.equal(byId(env, "auth-mfa-submit").getAttribute("aria-disabled"), "true", "submit без кода должен быть disabled");
    });
  });

  it("totp: submit активен при 6 цифрах, auth_mfa, код очищен до ответа, ready", async () => {
    const reply = deferred();
    await withAuth({ password: VIEW_MFA_ONE, mfa: () => reply.promise }, async ({ env, authCalls }) => {
      loginWithPassword(env, "user@example.test", PASSWORD);
      click(env, "auth-submit");
      await env.settle(4);

      type(env, "auth-mfa-code", "12345");
      assert.equal(byId(env, "auth-mfa-submit").getAttribute("aria-disabled"), "true", "5 цифр — рано");
      type(env, "auth-mfa-code", TOTP);
      assert.equal(byId(env, "auth-mfa-submit").getAttribute("aria-disabled"), null, "6 цифр — submit должен быть активен");

      click(env, "auth-mfa-submit");
      await env.settle(2);
      assert.equal(byId(env, "auth-mfa-code").value, "", "код остался в DOM после submit");
      assert.equal(state(env), "validating");
      assert.equal(byId(env, "auth-mfa").getAttribute("hidden"), null, "MFA-экран не должен подменяться формой");
      const call = authCalls.find((item) => item.cmd === "auth_mfa");
      assert.ok(call, "auth_mfa не вызван");
      assert.equal(call.args.method, "totp");
      assert.equal(call.args.code, TOTP);
      assert.equal(call.args.ticket, "ticket-777");
      assert.equal(call.args.login_instance_id, null);
      assertSecretGone(env, TOTP, ["auth_mfa"]);

      reply.resolve(VIEW_READY);
      await env.settle(4);
      assert.equal(state(env), "ready");
    });
  });

  it("после submit код очищен, кнопка disabled, второй клик не шлёт пустой auth_mfa", async () => {
    const reply = deferred();
    await withAuth({ password: VIEW_MFA_ONE, mfa: () => reply.promise }, async ({ env, authCalls }) => {
      loginWithPassword(env, "user@example.test", PASSWORD);
      click(env, "auth-submit");
      await env.settle(4);

      type(env, "auth-mfa-code", TOTP);
      assert.equal(byId(env, "auth-mfa-submit").getAttribute("aria-disabled"), null);
      click(env, "auth-mfa-submit");
      await env.settle(2);

      assert.equal(byId(env, "auth-mfa-code").value, "", "код остался в поле после submit");
      assert.equal(
        byId(env, "auth-mfa-submit").getAttribute("aria-disabled"),
        "true",
        "updateMfa не вернул disabled после очистки кода",
      );
      click(env, "auth-mfa-submit");
      await env.settle(2);
      const calls = authCalls.filter((item) => item.cmd === "auth_mfa");
      assert.equal(calls.length, 1, "второй клик отправил auth_mfa с пустым кодом");
      assert.equal(calls[0]?.args.code, TOTP);

      reply.resolve(VIEW_READY);
      await env.settle(4);
      assert.equal(state(env), "ready");
    });
  });

  it("sms: auth_sms при открытии слайда, маскированный номер, повтор по кнопке", async () => {
    await withAuth(
      { password: VIEW_MFA, sms: { phone: "+7 *** ** ** 12" } },
      async ({ env, authCalls }) => {
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);
        byId(env, "auth-mfa-method-sms").click();
        await env.settle(4);

        const smsCalls = authCalls.filter((call) => call.cmd === "auth_sms");
        assert.deepEqual(smsCalls, [{ cmd: "auth_sms", args: { ticket: "ticket-777" } }]);
        assert.match(statusOf(env, "auth-mfa-subtitle"), /\+7 \*\*\*/);
        assert.equal(byId(env, "auth-mfa-code").getAttribute("maxlength"), "10");
        assert.equal(byId(env, "auth-mfa-sms-resend").getAttribute("hidden"), null, "кнопка повтора не показана");
        byId(env, "auth-mfa-sms-resend").click();
        await env.settle(4);
        assert.equal(authCalls.filter((call) => call.cmd === "auth_sms").length, 2, "повтор не отправлен");
      },
    );
  });

  it("backup: предупреждение и счётчик 10 с, затем слайд резервного кода", async () => {
    await withAuth({ password: VIEW_MFA }, async ({ env }) => {
      loginWithPassword(env, "user@example.test", PASSWORD);
      click(env, "auth-submit");
      await env.settle(4);
      byId(env, "auth-mfa-method-backup").click();
      await env.settle(2);

      const ack = byId(env, "auth-mfa-backup-ack");
      assert.equal(ack.getAttribute("hidden"), null, "кнопка подтверждения backup не показана");
      assert.equal(ack.getAttribute("aria-disabled"), "true", "подтверждение backup без счётчика");
      assert.match(statusOf(env, "auth-mfa-subtitle"), /10 с/);
      for (let index = 0; index < 9; index++) env.flushTimers();
      assert.equal(ack.getAttribute("aria-disabled"), "true", "счётчик не идёт");
      env.flushTimers();
      assert.equal(ack.getAttribute("aria-disabled"), null, "через 10 с кнопка должна включиться");

      ack.click();
      await env.settle(2);
      assert.equal(byId(env, "auth-mfa-methods").getAttribute("hidden"), "", "список не скрыт");
      assert.equal(byId(env, "auth-mfa-form").getAttribute("hidden"), null, "слайд backup не открыт");
      assert.equal(byId(env, "auth-mfa-code").getAttribute("maxlength"), "11");
      assert.equal(byId(env, "auth-mfa-code").getAttribute("minlength"), "8");
      type(env, "auth-mfa-code", "ABCD-EFG");
      assert.equal(byId(env, "auth-mfa-submit").getAttribute("aria-disabled"), "true", "7 символов — рано");
      type(env, "auth-mfa-code", BACKUP);
      assert.equal(byId(env, "auth-mfa-submit").getAttribute("aria-disabled"), null);
    });
  });

  it("backup submit: дефисы вырезаются, метод backup", async () => {
    await withAuth({ password: VIEW_MFA, mfa: VIEW_READY }, async ({ env, authCalls }) => {
      loginWithPassword(env, "user@example.test", PASSWORD);
      click(env, "auth-submit");
      await env.settle(4);
      byId(env, "auth-mfa-method-backup").click();
      await env.settle(2);
      for (let index = 0; index < 10; index++) env.flushTimers();
      byId(env, "auth-mfa-backup-ack").click();
      await env.settle(2);
      type(env, "auth-mfa-code", BACKUP);
      click(env, "auth-mfa-submit");
      await env.settle(4);

      const call = authCalls.find((item) => item.cmd === "auth_mfa");
      assert.ok(call);
      assert.equal(call.args.method, "backup");
      assert.equal(call.args.code, "ABCDEFGH", "дефисы должны вырезаться");
      assert.equal(domDump(env).includes(BACKUP), false, "резервный код остался в DOM");
    });
  });

  it("password-метод: поле type=password, пустой submit disabled", async () => {
    await withAuth({ password: VIEW_MFA }, async ({ env }) => {
      loginWithPassword(env, "user@example.test", PASSWORD);
      click(env, "auth-submit");
      await env.settle(4);
      byId(env, "auth-mfa-method-password").click();
      await env.settle(2);
      const code = byId(env, "auth-mfa-code");
      assert.equal(code.getAttribute("type"), "password");
      assert.equal(code.getAttribute("autocomplete"), "current-password");
      assert.equal(byId(env, "auth-mfa-submit").getAttribute("aria-disabled"), "true");
      type(env, "auth-mfa-code", "secret");
      assert.equal(byId(env, "auth-mfa-submit").getAttribute("aria-disabled"), null);
    });
  });

  it("webauthn: disabled-affordance с подсказкой (нативной подписи нет)", async () => {
    await withAuth({ password: VIEW_MFA }, async ({ env, authCalls }) => {
      loginWithPassword(env, "user@example.test", PASSWORD);
      click(env, "auth-submit");
      await env.settle(4);
      const webauthn = byId(env, "auth-mfa-method-webauthn");
      assert.equal(webauthn.getAttribute("aria-disabled"), "true");
      assert.ok((webauthn.getAttribute("title") ?? "").length > 0);
      webauthn.click();
      await env.settle(2);
      assert.equal(authCalls.filter((call) => call.cmd === "auth_mfa").length, 0, "webauthn не должен слать код");
      assert.equal(state(env), "mfa");
    });
  });

  it("неверный код: «Неверный код.» в alert, MFA остаётся, код очищен", async () => {
    await withAuth(
      { password: VIEW_MFA_ONE, mfa: () => Promise.reject({ kind: "MfaRejected", message: "bad code", code: 60008 }) },
      async ({ env }) => {
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);
        type(env, "auth-mfa-code", TOTP);
        click(env, "auth-mfa-submit");
        await env.settle(4);

        assert.equal(state(env), "mfa");
        assert.equal(errorText(env), "Неверный код.");
        assert.equal(byId(env, "auth-mfa-code").value, "", "код остался в поле");
        assert.equal(byId(env, "auth-mfa").getAttribute("hidden"), null);
      },
    );
  });

  it("истёкший ticket (60006): возврат в no-session с понятным текстом", async () => {
    await withAuth(
      { password: VIEW_MFA_ONE, mfa: () => Promise.reject({ kind: "MfaRejected", message: "expired", code: 60006 }) },
      async ({ env }) => {
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);
        type(env, "auth-mfa-code", TOTP);
        click(env, "auth-mfa-submit");
        await env.settle(4);

        assert.equal(state(env), "no-session");
        assert.match(errorText(env), /сессия входа истекла/i);
        assert.equal(byId(env, "auth-mfa").getAttribute("hidden"), "", "MFA не скрыт");
        assert.equal(byId(env, "auth-qr").getAttribute("hidden"), null, "QR не вернулся");
      },
    );
  });

  it("«Отмена»: возврат к форме, QR и restore снова доступны", async () => {
    await withAuth({ password: VIEW_MFA }, async ({ env }) => {
      loginWithPassword(env, "user@example.test", PASSWORD);
      click(env, "auth-submit");
      await env.settle(4);
      click(env, "auth-mfa-close");
      await env.settle(2);

      assert.equal(state(env), "no-session");
      assert.equal(byId(env, "auth-mfa").getAttribute("hidden"), "");
      assert.equal(byId(env, "auth-form").getAttribute("hidden"), null);
      assert.equal(byId(env, "auth-qr").getAttribute("hidden"), null);
      assert.ok(focusLog.some((node) => node.id === "auth-login"));
    });
  });
});

describe("auth-гейт: ошибки 401/429/captcha/сеть без секретов (LOGIN-SPEC §7)", () => {
  it("401 InvalidCredentials: alert, пароль помечен aria-invalid, гейт открыт", async () => {
    await withAuth(
      { password: () => Promise.reject({ kind: "InvalidCredentials", message: "unauthorized" }) },
      async ({ env }) => {
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);

        assert.equal(state(env), "error");
        assert.match(errorText(env), /неверный логин или пароль/i);
        assert.equal(byId(env, "auth-gate").getAttribute("hidden"), null, "гейт закрылся после 401");
        assert.equal(byId(env, "auth-password").getAttribute("aria-invalid"), "true", "пароль не помечен ошибкой");
        assert.equal(byId(env, "auth-password").value, "", "пароль остался в DOM");
        assert.equal(byId(env, "auth-submit").getAttribute("aria-disabled"), null);
        assertSecretGone(env, PASSWORD, ["auth_login_password"]);
      },
    );
  });

  it("429 RateLimited: retry_after в тексте, submit заблокирован, автоповтора нет", async () => {
    await withAuth(
      {
        password: () =>
          Promise.reject({ kind: "RateLimited", message: "rate limited", retry_after_ms: 3000 }),
      },
      async ({ env, authCalls }) => {
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);

        assert.match(errorText(env), /слишком много попыток/i);
        assert.match(errorText(env), /3 с/);
        assert.equal(state(env), "error");
        assert.equal(byId(env, "auth-submit").getAttribute("aria-disabled"), "true", "submit не заблокирован на 429");
        assert.ok((byId(env, "auth-submit").getAttribute("title") ?? "").length > 0);
        assert.equal(
          authCalls.filter((call) => call.cmd === "auth_login_password").length,
          1,
          "429 не должен ретраиться сам",
        );

        env.flushTimers();
        env.flushTimers();
        assert.equal(byId(env, "auth-submit").getAttribute("aria-disabled"), "true", "счётчик не идёт");
        env.flushTimers();
        assert.equal(byId(env, "auth-submit").getAttribute("aria-disabled"), null, "по окончании счётчика submit должен включиться");
        assert.equal(authCalls.filter((call) => call.cmd === "auth_login_password").length, 1);
      },
    );
  });

  it("сеть (Network kind): «нет связи», состояние error, гейт открыт", async () => {
    await withAuth(
      { password: () => Promise.reject({ kind: "Network", message: "connect timeout" }) },
      async ({ env }) => {
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);

        assert.match(errorText(env), /нет связи/i, "нет сообщения о сети");
        assert.equal(state(env), "error");
        assert.equal(byId(env, "auth-password").value, "");
        assertSecretGone(env, PASSWORD, ["auth_login_password"]);
      },
    );
  });

  it("сорванный IPC (invoke rejects строкой): тоже «нет связи», без console.error", async () => {
    await withAuth(
      { password: () => Promise.reject("ipc closed") },
      async ({ env }) => {
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);

        assert.match(errorText(env), /нет связи/i);
        assert.deepEqual(env.consoleErrors, [], "сбой IPC не должен шуметь в console.error");
        assertSecretGone(env, PASSWORD, ["auth_login_password"]);
      },
    );
  });

  it("Keyring в теле AuthState: сообщение о хранилище", async () => {
    await withAuth(
      {
        password: {
          status: "error",
          stored: true,
          user: null,
          error: { kind: "Keyring", message: "no secret service" },
        },
      },
      async ({ env }) => {
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);

        assert.match(errorText(env), /хранилищ/i);
        assert.equal(state(env), "error");
        assertSecretGone(env, PASSWORD, ["auth_login_password"]);
      },
    );
  });

  it("LoginRejected 70007: подтверждение в официальном клиенте, retry скрыт", async () => {
    await withAuth(
      {
        password: () =>
          Promise.reject({ kind: "LoginRejected", message: "rejected", code: 70007 }),
      },
      async ({ env }) => {
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);
        assert.match(errorText(env), /официальном клиенте/i);
        assert.equal(byId(env, "auth-retry").getAttribute("hidden"), null, "кнопка «Повторить» должна быть видна");
      },
    );
  });

  it("«Повторить»: сбрасывает ошибку и возвращает форму ввода", async () => {
    await withAuth(
      { password: () => Promise.reject({ kind: "Network", message: "offline" }) },
      async ({ env }) => {
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);
        assert.equal(state(env), "error");
        click(env, "auth-retry");
        await env.settle(2);
        assert.equal(state(env), "no-session");
        assert.equal(errorText(env), "");
        assert.ok(focusLog.some((node) => node.id === "auth-login"));
      },
    );
  });
});

describe("auth-гейт: вход токеном, restore, logout, отсутствие мёртвых кнопок", () => {
  it("токен: validating, поле очищено, invoke(auth_login_token), ready", async () => {
    const login = deferred();
    await withAuth({ login: () => login.promise }, async ({ env, authCalls }) => {
      type(env, "auth-token", `  ${TOKEN}  `);
      click(env, "auth-token-submit");
      await env.settle(2);

      assert.equal(state(env), "validating");
      assert.equal(byId(env, "auth-token").value, "", "токен остался в поле после submit");
      const call = authCalls.find((item) => item.cmd === "auth_login_token");
      assert.ok(call, "auth_login_token не вызван");
      assert.deepEqual(call.args, { token: TOKEN }, "токен ушёл не в поле token (trim)");

      login.resolve(READY);
      await env.settle(4);
      assert.equal(state(env), "ready");
      assert.equal(one(env, ".user-name").textContent, "Тестер");
      assertSecretGone(env, TOKEN, ["auth_login_token"]);
    });
  });

  it("токен: пустой ввод — «введите токен», IPC не вызывается", async () => {
    await withAuth({}, async ({ env, authCalls }) => {
      type(env, "auth-token", "   ");
      click(env, "auth-token-submit");
      await env.settle(2);
      assert.match(errorText(env), /введите токен/i);
      assert.equal(authCalls.filter((call) => call.cmd === "auth_login_token").length, 0);
      assert.equal(state(env), "no-session");
      assert.equal(focusLog[focusLog.length - 1]?.id, "auth-token");
    });
  });

  it("restore: invoke(auth_restore) без аргументов, validating → ready", async () => {
    const restore = deferred();
    await withAuth({ restore: () => restore.promise }, async ({ env, authCalls }) => {
      click(env, "auth-restore");
      await env.settle(2);

      assert.equal(state(env), "validating");
      assert.equal(byId(env, "auth-gate").getAttribute("aria-busy"), "true");
      assert.deepEqual(
        authCalls.filter((call) => call.cmd === "auth_restore"),
        [{ cmd: "auth_restore", args: null }],
        "auth_restore должен вызываться без аргументов",
      );

      restore.resolve(READY);
      await env.settle(4);
      assert.equal(state(env), "ready");
      assert.equal(byId(env, "auth-gate").getAttribute("hidden"), "");
      assert.equal(one(env, ".user-name").textContent, "Тестер");
    });
  });

  it("токен: повторный клик до ответа не шлёт второй IPC, кнопка disabled до ответа", async () => {
    const login = deferred();
    await withAuth({ login: () => login.promise }, async ({ env, authCalls }) => {
      type(env, "auth-token", TOKEN);
      click(env, "auth-token-submit");
      await env.settle(2);

      const submit = byId(env, "auth-token-submit");
      assert.equal(submit.getAttribute("disabled"), "", "кнопка токена не disabled во время IPC");
      assert.equal(submit.getAttribute("aria-disabled"), "true");
      assert.ok((submit.getAttribute("title") ?? "").length > 0, "disabled без подсказки");

      click(env, "auth-token-submit");
      click(env, "auth-token-submit");
      await env.settle(2);
      assert.equal(
        authCalls.filter((item) => item.cmd === "auth_login_token").length,
        1,
        "повторный клик отправил второй auth_login_token",
      );
      assert.equal(state(env), "validating");
      assert.equal(errorText(env), "", "повторный клик показал ложную ошибку");

      login.resolve(READY);
      await env.settle(4);
      assert.equal(state(env), "ready");
      assert.equal(submit.getAttribute("disabled"), null, "disabled не снят после ответа");
      assert.equal(submit.getAttribute("aria-disabled"), null, "aria-disabled не снят после ответа");
    });
  });

  it("restore: повторный клик до ответа не шлёт второй auth_restore", async () => {
    const restore = deferred();
    await withAuth({ restore: () => restore.promise }, async ({ env, authCalls }) => {
      click(env, "auth-restore");
      await env.settle(2);
      const button = byId(env, "auth-restore");
      assert.equal(button.getAttribute("disabled"), "", "restore не disabled во время IPC");
      assert.equal(button.getAttribute("aria-disabled"), "true");

      button.click();
      button.click();
      await env.settle(2);
      assert.equal(
        authCalls.filter((item) => item.cmd === "auth_restore").length,
        1,
        "повторный клик отправил второй auth_restore",
      );

      restore.resolve(READY);
      await env.settle(4);
      assert.equal(state(env), "ready");
      assert.equal(button.getAttribute("disabled"), null, "disabled не снят после ответа");
      assert.equal(button.getAttribute("aria-disabled"), null, "aria-disabled не снят после ответа");
    });
  });

  it("restore с отозванным токеном: ошибка в гейте, сессия не открыта", async () => {
    await withAuth(
      { restore: () => Promise.reject({ kind: "InvalidToken", message: "unauthorized" }) },
      async ({ env }) => {
        click(env, "auth-restore");
        await env.settle(4);

        assert.match(errorText(env), /токен недействителен/i);
        assert.equal(state(env), "error");
        assert.equal(byId(env, "auth-gate").getAttribute("hidden"), null);
        assert.equal(byId(env, "auth-logout").getAttribute("hidden"), "", "logout не должен быть доступен без входа");
      },
    );
  });

  it("logout: invoke(auth_logout), гейт снова открыт, имя сброшено, фокус в логине", async () => {
    await withAuth({ status: READY }, async ({ env, authCalls }) => {
      click(env, "auth-logout");
      await env.settle(4);

      assert.deepEqual(
        authCalls.filter((call) => call.cmd === "auth_logout"),
        [{ cmd: "auth_logout", args: null }],
        "auth_logout не вызван (или с аргументами)",
      );
      assert.equal(state(env), "no-session");
      assert.equal(byId(env, "auth-gate").getAttribute("hidden"), null, "гейт не вернулся после logout");
      assert.equal(byId(env, "auth-logout").getAttribute("hidden"), "", "logout остался в user-bar");
      assert.equal(one(env, ".user-name").textContent, "ShadowByte", "имя не сброшено к заглушке");
      assert.equal(byId(env, "auth-password").value, "");
      assert.equal(byId(env, "auth-token").value, "");
      assert.equal(focusLog[focusLog.length - 1]?.id, "auth-login", "после logout фокус не в логине");
    });
  });

  it("logout отклонён: поля и captcha/MFA-контекст сброшены, гейт открыт, без console.error", async () => {
    /** @type {any} */
    let captured = null;
    await withAuth(
      {
        password: (/** @type {any} */ args) =>
          args.captcha_key
            ? VIEW_MFA_ONE
            : {
                ...VIEW_CAPTCHA,
                captcha_session_id: "session-123",
                captcha_rqdata: "rqdata-456",
              },
        mfa: VIEW_READY,
        logout: () => Promise.reject(new Error("offline")),
      },
      async ({ env, authCalls }) => {
        // Контекст капчи доходит до монтажа адаптера; ключ и session/rqdata
        // запоминаются гейтом и обязаны исчезнуть при logout.
        /** @type {any} */ (env.window).__AUTH_CAPTCHA__ = {
          mount: (/** @type {ShimElement} */ _container, /** @type {any} */ ctx) => {
            captured = ctx;
          },
        };

        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);
        assert.equal(state(env), "captcha");
        assert.ok(captured, "adapter капчи не смонтирован");
        captured.onToken("captcha-token-1");
        click(env, "auth-captcha-continue");
        await env.settle(4);

        assert.equal(state(env), "mfa");
        type(env, "auth-mfa-code", TOTP);
        click(env, "auth-mfa-submit");
        await env.settle(4);
        assert.equal(state(env), "ready");

        click(env, "auth-logout");
        await env.settle(4);

        assert.deepEqual(
          authCalls.filter((call) => call.cmd === "auth_logout"),
          [{ cmd: "auth_logout", args: null }],
          "auth_logout не вызван (или с аргументами)",
        );
        assert.ok(
          ["no-session", "error"].includes(state(env)),
          `после отклонённого logout гейт в состоянии «${state(env)}»`,
        );
        assert.equal(byId(env, "auth-gate").getAttribute("hidden"), null, "гейт не вернулся после logout");
        assert.equal(byId(env, "auth-logout").getAttribute("hidden"), "", "logout остался в user-bar");
        for (const id of ["auth-login", "auth-password", "auth-token", "auth-mfa-code"]) {
          assert.equal(byId(env, id).value, "", `#${id} не очищен после logout`);
        }
        assert.equal(domDump(env).includes(PASSWORD), false, "пароль остался в DOM после logout");
        assert.equal(domDump(env).includes("captcha-token-1"), false, "ключ капчи остался в DOM");
        assert.deepEqual(env.consoleErrors, [], "сбой logout не должен шуметь в console.error");

        const app = env.appModule?.currentApp?.();
        assert.ok(app, "приложение не собралось после сбоя logout");
        assert.equal(all(env, ".screen").length, 4, "screenCount изменился");
        assert.ok(all(env, "#chat-list .row").length > 0, "чат не отрисован");

        // Повторный вход: старые captcha_key/rqtoken/session/rqdata и MFA-ticket
        // не должны попасть в auth_login_password.
        loginWithPassword(env, "user2@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);

        const passwordCalls = authCalls.filter((call) => call.cmd === "auth_login_password");
        assert.equal(passwordCalls.length, 3, "повторный вход после logout не отправлен");
        assert.deepEqual(
          passwordCalls[2]?.args,
          { login: "user2@example.test", password: PASSWORD },
          "старый captcha/MFA-контекст утёк в новый вход",
        );
        assert.deepEqual(env.consoleErrors, [], "повторный вход шумит в console.error");
      },
    );
  });

  it("показать/скрыть токен: type password↔text и aria-pressed", async () => {
    await withAuth({}, async ({ env }) => {
      const input = byId(env, "auth-token");
      const reveal = byId(env, "auth-reveal");
      assert.equal(input.getAttribute("type"), "password");
      assert.equal(reveal.getAttribute("aria-pressed"), "false");

      reveal.click();
      assert.equal(input.getAttribute("type"), "text", "токен не показан");
      assert.equal(reveal.getAttribute("aria-pressed"), "true");
      assert.ok((reveal.getAttribute("aria-label") ?? "").length > 0, "у переключателя нет доступного имени");

      reveal.click();
      assert.equal(input.getAttribute("type"), "password", "токен не скрыт обратно");
      assert.equal(reveal.getAttribute("aria-pressed"), "false");
    });
  });

  it("нет мёртвых кнопок: подпись/имя у каждой, disabled — с подсказкой", async () => {
    await withAuth({}, async ({ env }) => {
      const buttons = all(env, "#auth-gate button");
      const ids = buttons.map((node) => node.id).filter(Boolean).sort();
      for (const required of [
        "auth-captcha-continue",
        "auth-forgot",
        "auth-password-reveal",
        "auth-passkey-disabled",
        "auth-qr-disabled",
        "auth-register",
        "auth-restore",
        "auth-retry",
        "auth-reveal",
        "auth-submit",
        "auth-token-submit",
      ]) {
        assert.ok(ids.includes(required), `в гейте нет кнопки #${required}`);
      }
      for (const node of buttons) {
        const disabled = node.getAttribute("aria-disabled") === "true";
        const hidden = node.getAttribute("hidden") !== null;
        if (disabled && !hidden) {
          assert.ok((node.getAttribute("title") ?? "").length > 0, `#${node.id || node.className} disabled без подсказки`);
        }
        assert.ok(
          (node.textContent ?? "").trim().length > 0 || (node.getAttribute("aria-label") ?? "").length > 0,
          `#${node.id || node.className} без видимой подписи и доступного имени`,
        );
      }
    });
  });
});

describe("auth-гейт: среда без IPC и отсутствие секретов", () => {
  it("auth_status падает: гейт с «нет связи», приложение живо, без console.error", async () => {
    await withAuth(
      { status: () => Promise.reject(new Error("нет window.__TAURI__")) },
      async ({ env }) => {
        assert.match(errorText(env), /нет связи/i, "нет видимого сообщения о недоступном ядре");
        assert.equal(state(env), "error");
        assert.equal(byId(env, "auth-gate").getAttribute("hidden"), null, "гейт не показан без сессии");

        const app = env.appModule?.currentApp?.();
        assert.ok(app, "приложение не собралось");
        assert.equal(all(env, ".screen").length, 4, "screenCount изменился");
        assert.ok(all(env, "#chat-list .row").length > 0, "чат не отрисован");
        assert.deepEqual(env.consoleErrors, [], "недоступный IPC не должен шуметь в console.error");
      },
    );
  });

  it("секреты не попадают в DOM/логи/чужие IPC: пароль и MFA-код", async () => {
    await withAuth(
      {
        password: VIEW_MFA_ONE,
        mfa: () => Promise.reject({ kind: "MfaRejected", message: "bad", code: 60008 }),
      },
      async ({ env }) => {
        loginWithPassword(env, "user@example.test", PASSWORD);
        click(env, "auth-submit");
        await env.settle(4);
        type(env, "auth-mfa-code", TOTP);
        click(env, "auth-mfa-submit");
        await env.settle(4);

        assert.equal(byId(env, "auth-password").value, "", "пароль не очищен");
        assert.equal(byId(env, "auth-mfa-code").value, "", "код не очищен");
        assert.equal(
          one(env, ".user-name").textContent.includes(PASSWORD),
          false,
          "пароль попал в user-bar",
        );
        const snapshot = env.callsOf("report_ui_snapshot");
        assert.ok(snapshot.length > 0, "UI_SNAPSHOT не отправлен");
        const dumped = JSON.stringify(snapshot);
        assert.equal(dumped.includes(PASSWORD), false, "пароль попал в UI_SNAPSHOT");
        assert.equal(dumped.includes(TOTP), false, "код попал в UI_SNAPSHOT");
        assertSecretGone(env, PASSWORD, ["auth_login_password"]);
        assertSecretGone(env, TOTP, ["auth_mfa"]);
      },
    );
  });

  it("токен не появляется ни в гейте, ни в user-bar, ни в снапшоте UI", async () => {
    await withAuth({ login: () => Promise.reject({ kind: "InvalidToken", message: "unauthorized" }) }, async ({ env }) => {
      type(env, "auth-token", TOKEN);
      click(env, "auth-token-submit");
      await env.settle(4);

      assert.equal(byId(env, "auth-token").value, "", "поле токена не очищено");
      assert.equal(one(env, ".user-name").textContent.includes(TOKEN), false, "токен попал в user-bar");
      const snapshot = env.callsOf("report_ui_snapshot");
      assert.ok(snapshot.length > 0, "UI_SNAPSHOT не отправлен");
      assert.equal(JSON.stringify(snapshot).includes(TOKEN), false, "токен попал в UI_SNAPSHOT");
      assertSecretGone(env, TOKEN, ["auth_login_token"]);
    });
  });
});

/**
 * Текст узла по id (короче, чем byId(...).textContent в отдельных кейсах).
 * @param {GuiDom} env
 * @param {string} id
 * @returns {string}
 */
function statusOf(env, id) {
  return byId(env, id).textContent.trim();
}
