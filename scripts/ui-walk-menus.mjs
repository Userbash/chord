#!/usr/bin/env node
// @ts-check
// Тест меню и панелей Discord: меню сервера, контекстное меню канала, справка,
// почта (inbox), кнопки шапки канала, попут-панель пользователя.
//
// Запуск: node scripts/ui-walk-menus.mjs
// Результаты: logs/ui-walk-menus.jsonl, logs/ui-clicks-menus.jsonl, evidence/screens/ui-menu-*.png

/** @typedef {import("./cdp-types.js").CdpEvent} CdpEvent */
/** @typedef {import("./cdp-types.js").CdpEventClient} CdpEventClient */
/** @typedef {import("./cdp-types.js").CdpMessage} CdpMessage */
/** @typedef {import("./cdp-types.js").CpuProfile} CpuProfile */
/** @typedef {import("./cdp-types.js").TopFunction} TopFunction */
/** @typedef {import("./cdp-types.js").WalkRecord} WalkRecord */

import { appendFile, mkdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const endpoint = process.env.CDP_ENDPOINT ?? "http://127.0.0.1:9222";
// запрещённые пункты: необратимые/опасные действия и переключение чужих настроек
const BLACKLIST =
  /покинуть|leave|удал|delete|заглуш|mute|deafen|скрыть|скрывать|показать скрытые|выйти|log ?out|sign ?out|заблокир|block|статус|status|отключ|reset|сбросить|очистить|очистка|забыть|forget|выйти из/i;
/** @param {number} ms @returns {Promise<void>} */
const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

/** @returns {Promise<CdpEventClient>} */
async function connect() {
  const targets = /** @type {import("./cdp-types.js").CdpTarget[]} */ (
    await (await fetch(`${endpoint}/json/list`)).json()
  );
  const page = targets.find((t) => t.type === "page" && (t.url ?? "").includes("discord.com"));
  if (!page || !page.webSocketDebuggerUrl) throw new Error("Страница Discord не найдена в CDP");
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    ws.addEventListener("open", resolve, { once: true });
    ws.addEventListener("error", () => reject(new Error("WebSocket error")), { once: true });
  });
  let nextId = 1;
  /** @type {Map<number, (message: CdpMessage) => void>} */
  const pending = new Map();
  /** @type {CdpEvent[]} */
  const events = [];
  ws.addEventListener("message", (event) => {
    /** @type {CdpMessage} */
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) {
      const settle = pending.get(message.id);
      if (settle) {
        settle(message);
        pending.delete(message.id);
      }
    } else if (message.method) {
      events.push({ at: Date.now(), method: message.method, params: message.params });
    }
  });
  ws.addEventListener("close", () => {
    for (const settle of pending.values()) settle({ error: { message: "CDP соединение закрыто" } });
    pending.clear();
  });
  /** @type {CdpEventClient["send"]} */
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = nextId++;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`${method}: таймаут 15с`));
      }, 15000);
      pending.set(id, (message) => {
        clearTimeout(timer);
        message.error ? reject(new Error(`${method}: ${message.error.message}`)) : resolve(message.result);
      });
      ws.send(JSON.stringify({ id, method, params }));
    });
  return { send, events, close: () => ws.close() };
}

/**
 * @param {CdpEventClient} cdp
 * @param {string} expression
 * @returns {Promise<any>}
 */
async function evaluate(cdp, expression) {
  const result = await cdp.send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  return result.result.value;
}

const NOISE = new Set(["(idle)", "(program)", "(garbage collector)", "(root)", "(anonymous)"]);
/** @param {CpuProfile} profile @returns {TopFunction[]} */
const topFunctions = (profile) =>
  profile.nodes
    .filter((node) => node.hitCount && !NOISE.has(node.callFrame.functionName || "(anonymous)"))
    .sort((left, right) => (right.hitCount ?? 0) - (left.hitCount ?? 0))
    .slice(0, 8)
    .map((node) => ({
      fn: node.callFrame.functionName || "(anonymous)",
      url: (node.callFrame.url || "").split("/").pop()?.slice(0, 60) ?? "",
      hits: node.hitCount ?? 0,
    }));

const VISIBLE = `(el) => { const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0; }`;

const MENU_OPEN = `(() => {
  const visible = ${VISIBLE};
  return [...document.querySelectorAll('[role=menu]')].some(visible);
})()`;

const LIST_MENU_ITEMS = `(() => {
  const visible = ${VISIBLE};
  return [...document.querySelectorAll('[role=menuitem]')].filter(visible)
    .map((el) => (el.innerText || '').replace(/\\s+/g, ' ').trim().slice(0, 70))
    .filter(Boolean);
})()`;

/** @param {string} label */
const CLICK_MENU_ITEM = (label) => `(() => {
  const visible = ${VISIBLE};
  const items = [...document.querySelectorAll('[role=menuitem]')].filter(visible);
  const el = items.find((item) => (item.innerText || '').replace(/\\s+/g, ' ').trim().startsWith(${JSON.stringify(label)}));
  if (!el) return { clicked: false };
  el.click();
  return { clicked: true };
})()`;

/** @param {string} part */
const OPEN_BY_ARIA = (part) => `(() => {
  const visible = ${VISIBLE};
  const el = [...document.querySelectorAll('button,[role=button]')].filter(visible)
    .find((node) => (node.getAttribute('aria-label') || '').includes(${JSON.stringify(part)}));
  if (!el) return { clicked: false };
  el.click();
  return { clicked: true, aria: el.getAttribute('aria-label') };
})()`;

const OPEN_CHANNEL_MENU = `(() => {
  const visible = ${VISIBLE};
  const el = [...document.querySelectorAll('[aria-label]')].filter(visible)
    .find((node) => /(текстовой канал|text channel)/i.test(node.getAttribute('aria-label') || ''));
  if (!el) return { opened: false };
  const r = el.getBoundingClientRect();
  el.dispatchEvent(new MouseEvent('contextmenu', {
    bubbles: true, cancelable: true, view: window,
    clientX: r.left + r.width / 2, clientY: r.top + r.height / 2, button: 2,
  }));
  return { opened: true };
})()`;

const TOOLBAR_BUTTONS = `(() => {
  const visible = ${VISIBLE};
  const header = document.querySelector('[class*=toolbar]') || document.querySelector('[class*=channelHeader]');
  if (!header) return [];
  return [...header.querySelectorAll('button,[role=button]')].filter(visible)
    .map((el) => (el.getAttribute('aria-label') || (el.innerText || '').trim()).slice(0, 50));
})()`;

const main = async () => {
  await mkdir(join(root, "evidence", "screens"), { recursive: true });
  const logPath = join(root, "logs", "ui-walk-menus.jsonl");
  const cdp = await connect();
  let step = 0;
  /** @type {WalkRecord[]} */
  const records = [];

  const pressEscape = async () => {
    for (const type of ["rawKeyDown", "keyUp"]) {
      await cdp.send("Input.dispatchKeyEvent", { type, key: "Escape", code: "Escape", windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 });
    }
  };

  const overlaysOpen = () =>
    evaluate(
      cdp,
      `(() => {
        const visible = ${VISIBLE};
        const menus = [...document.querySelectorAll('[role=menu]')].filter(visible).length;
        const dialogs = [...document.querySelectorAll('[role=dialog]')].filter(visible).length;
        const settings = !!document.querySelector('.navScroller__409aa');
        return { menus, dialogs, settings };
      })()`,
    );

  const escapeToBase = async () => {
    for (let attempt = 0; attempt < 5; attempt += 1) {
      const state = await overlaysOpen().catch(() => ({ menus: 0, dialogs: 0, settings: false }));
      if (!state.menus && !state.dialogs && !state.settings) return state;
      await pressEscape();
      await sleep(450);
    }
    return overlaysOpen().catch(() => ({}));
  };

  /** @param {string} label @param {() => Promise<any>} action @returns {Promise<WalkRecord>} */
  const stepRecord = async (label, action) => {
    step += 1;
    /** @type {WalkRecord} */
    const record = { step, label, clicked: false, screenshot: null, consoleErrors: 0, topFunctions: [], error: null };
    try {
      await cdp.send("Profiler.start");
      const outcome = await action();
      record.clicked = Boolean(outcome?.clicked ?? outcome?.opened ?? true);
      await sleep(900);
      const { profile } = await cdp.send("Profiler.stop");
      record.topFunctions = topFunctions(profile);
      const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
      const slug = label.replace(/[^\p{L}\p{N}]+/gu, "-").slice(0, 48) || `step-${step}`;
      const shotPath = join(root, "evidence", "screens", `ui-menu-${String(step).padStart(2, "0")}-${slug}.png`);
      await writeFile(shotPath, Buffer.from(shot.data, "base64"));
      record.screenshot = shotPath.replace(`${root}/`, "");
    } catch (error) {
      record.error = error instanceof Error ? error.message : String(error);
      process.stderr.write(`[${step}] ОШИБКА "${label}": ${record.error}\n`);
    }
    records.push(record);
    await appendFile(logPath, `${JSON.stringify(record)}\n`);
    process.stdout.write(`[${String(step).padStart(2, "0")}] ${label} | ок=${record.clicked}${record.error ? ` | ОШИБКА: ${record.error}` : ""} | топ: ${record.topFunctions.slice(0, 3).map((f) => f.fn).join(", ")}\n`);
    if (record.error && /таймаут|закрыто/.test(record.error)) throw new Error("CDP отвалился");
    return record;
  };

  try {
    await cdp.send("Runtime.enable");
    await cdp.send("Profiler.enable");
    await cdp.send("Page.enable");
    await evaluate(cdp, `(() => {
      if (window.__clickLog) return 'exists';
      window.__clickLog = [];
      for (const kind of ['click', 'contextmenu']) {
        document.addEventListener(kind, (event) => {
          const el = event.target;
          const rect = el.getBoundingClientRect ? el.getBoundingClientRect() : { x: 0, y: 0 };
          window.__clickLog.push({
            kind, at: Date.now(),
            aria: el.getAttribute ? el.getAttribute('aria-label') : null,
            text: (el.innerText || el.textContent || '').replace(/\\s+/g, ' ').trim().slice(0, 100),
            x: Math.round(rect.x), y: Math.round(rect.y),
          });
        }, true);
      }
      return 'installed';
    })()`);

    await escapeToBase();
    await sleep(500);

    // 1. Меню сервера
    const serverMenuItems = [];
    {
      const opened = await evaluate(cdp, OPEN_BY_ARIA("действия с сервером"));
      await sleep(800);
      if (opened.clicked) {
        serverMenuItems.push(...(await evaluate(cdp, LIST_MENU_ITEMS)));
        await stepRecord("Меню сервера: открыто", async () => opened);
        await escapeToBase();
      } else {
        process.stdout.write("Меню сервера не найдено\n");
      }
    }
    for (const label of serverMenuItems) {
      if (BLACKLIST.test(label)) {
        process.stdout.write(`ПРОПУСК (запрещено): Меню сервера → ${label}\n`);
        continue;
      }
      await stepRecord(`Меню сервера → ${label}`, async () => {
        const opened = await evaluate(cdp, OPEN_BY_ARIA("действия с сервером"));
        if (!opened.clicked) return { clicked: false };
        await sleep(700);
        return evaluate(cdp, CLICK_MENU_ITEM(label));
      });
      await escapeToBase();
    }

    // 2. Кнопки в шапке канала (поиск, закреплённые, участники и т.п.)
    const toolbarButtons = await evaluate(cdp, TOOLBAR_BUTTONS).catch(() => []);
    for (const aria of toolbarButtons.filter(Boolean)) {
      if (BLACKLIST.test(aria) || /Помощь|Почта|Назад|Дальше/i.test(aria)) continue;
      await stepRecord(`Шапка канала → ${aria}`, async () => evaluate(cdp, OPEN_BY_ARIA(aria)));
      await escapeToBase();
    }

    // 3. Контекстное меню текстового канала
    const channelItems = [];
    {
      const opened = await evaluate(cdp, OPEN_CHANNEL_MENU);
      await sleep(800);
      if (opened.opened) {
        channelItems.push(...(await evaluate(cdp, LIST_MENU_ITEMS)));
        await stepRecord("Меню канала: открыто", async () => opened);
        await escapeToBase();
      } else {
        process.stdout.write("Контекстное меню канала не открылось\n");
      }
    }
    for (const label of channelItems) {
      if (BLACKLIST.test(label)) {
        process.stdout.write(`ПРОПУСК (запрещено): Меню канала → ${label}\n`);
        continue;
      }
      await stepRecord(`Меню канала → ${label}`, async () => {
        const opened = await evaluate(cdp, OPEN_CHANNEL_MENU);
        if (!opened.opened) return { clicked: false };
        await sleep(700);
        return evaluate(cdp, CLICK_MENU_ITEM(label));
      });
      await escapeToBase();
    }

    // 4. Меню «Помощь»
    const helpItems = [];
    {
      const opened = await evaluate(cdp, OPEN_BY_ARIA("Помощь"));
      await sleep(800);
      if (opened.clicked) {
        helpItems.push(...(await evaluate(cdp, LIST_MENU_ITEMS)));
        await stepRecord("Меню «Помощь»: открыто", async () => opened);
        await escapeToBase();
      }
    }
    for (const label of helpItems) {
      if (BLACKLIST.test(label)) {
        process.stdout.write(`ПРОПУСК (запрещено): Помощь → ${label}\n`);
        continue;
      }
      await stepRecord(`Помощь → ${label}`, async () => {
        const opened = await evaluate(cdp, OPEN_BY_ARIA("Помощь"));
        if (!opened.clicked) return { clicked: false };
        await sleep(700);
        return evaluate(cdp, CLICK_MENU_ITEM(label));
      });
      await escapeToBase();
    }

    // 5. Почта (inbox): вкладки
    {
      const opened = await evaluate(cdp, OPEN_BY_ARIA("Почта"));
      await sleep(900);
      if (opened.clicked) {
        await stepRecord("Почта: открыто", async () => opened);
        const tabs = await evaluate(
          cdp,
          `(() => { const visible = ${VISIBLE}; return [...document.querySelectorAll('[role=tab]')].filter(visible).map((el) => (el.innerText || el.getAttribute('aria-label') || '').replace(/\\s+/g, ' ').trim().slice(0, 50)); })()`,
        ).catch(() => []);
        for (const tab of tabs.filter(Boolean)) {
          await stepRecord(`Почта → вкладка ${tab}`, async () =>
            evaluate(
              cdp,
              `(() => { const visible = ${VISIBLE}; const el = [...document.querySelectorAll('[role=tab]')].filter(visible).find((node) => ((node.innerText || node.getAttribute('aria-label') || '').replace(/\\s+/g, ' ').trim()).startsWith(${JSON.stringify(tab)})); if (!el) return { clicked: false }; el.click(); return { clicked: true }; })()`,
            ),
          );
        }
        await escapeToBase();
      }
    }

    // 6. Панель пользователя (аватар): открыть и зафиксировать состав
    {
      const opened = await evaluate(
        cdp,
        `(() => {
          const visible = ${VISIBLE};
          const panels = document.querySelector('[class*=panels]');
          if (!panels) return { clicked: false };
          const avatar = [...panels.querySelectorAll('[class*=avatar],[class*=wrapper]')].filter(visible)
            .find((el) => el.getAttribute('aria-label') || el.getAttribute('role') === 'button' || el.onclick);
          if (!avatar) return { clicked: false };
          const clickable = avatar.closest('[role=button],button') || avatar;
          clickable.click();
          return { clicked: true, aria: clickable.getAttribute('aria-label') || (clickable.className || '').toString().slice(0, 60) };
        })()`,
      );
      await sleep(900);
      if (opened.clicked) {
        await stepRecord("Панель пользователя: открыто", async () => opened);
        const popoutButtons = await evaluate(
          cdp,
          `(() => {
            const visible = ${VISIBLE};
            const popout = [...document.querySelectorAll('[class*=userPopout],[class*=outer_c]')].find(visible) || document.body;
            return [...popout.querySelectorAll('button,[role=button]')].filter(visible)
              .map((el) => (el.getAttribute('aria-label') || (el.innerText || '').replace(/\\s+/g, ' ').trim()).slice(0, 45)).filter(Boolean).slice(0, 20);
          })()`,
        ).catch(() => []);
        process.stdout.write(`Панель пользователя, элементы: ${popoutButtons.join(" | ")}\n`);
        await escapeToBase();
      }
    }

    /** @type {any[]} */
    const clicks = await evaluate(cdp, "window.__clickLog ? window.__clickLog.splice(0) : []");
    await writeFile(join(root, "logs", "ui-clicks-menus.jsonl"), `${clicks.map((c) => JSON.stringify(c)).join("\n")}\n`);
    process.stdout.write(`\nГотово: ${records.length} шагов -> ${logPath}\n`);
  } finally {
    cdp.close();
  }
};

await main();
