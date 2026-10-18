#!/usr/bin/env node
// @ts-check
// Дополнительный прогон меню Discord:
//   A. оставшиеся пункты меню сервера (те, что не нашлись в ui-walk-menus),
//   B. меню «Помощь» (перечисление и клики),
//   C. контекстное меню текстового канала (доверенный правый клик через Input),
//   D. панель голоса: «Настройки ввода» / «Настройки вывода» (только открыть меню).
//
// Запуск: node scripts/ui-menu-extra.mjs
// Результаты: logs/ui-walk-menus.jsonl (дописывается), evidence/screens/ui-menu2-*.png

/** @typedef {import("./cdp-types.js").CdpClient} CdpClient */
/** @typedef {import("./cdp-types.js").CdpMessage} CdpMessage */
/** @typedef {import("./cdp-types.js").CpuProfile} CpuProfile */
/** @typedef {import("./cdp-types.js").TopFunction} TopFunction */
/** @typedef {import("./cdp-types.js").WalkRecord} WalkRecord */

import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const endpoint = process.env.CDP_ENDPOINT ?? "http://127.0.0.1:9222";
const BLACKLIST =
  /покинуть|leave|удал|delete|заглушить|mute|deafen|отписаться|выйти|log ?out|sign ?out|заблокир|block|статус|сброс|reset/i;
/** @param {number} ms @returns {Promise<void>} */
const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

/** @returns {Promise<CdpClient>} */
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
  ws.addEventListener("message", (event) => {
    /** @type {CdpMessage} */
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) {
      const settle = pending.get(message.id);
      if (settle) {
        settle(message);
        pending.delete(message.id);
      }
    }
  });
  /** @type {CdpClient["send"]} */
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
  return { send, close: () => ws.close() };
}

/**
 * @param {CdpClient} cdp
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
    .map((node) => ({ fn: node.callFrame.functionName || "(anonymous)", url: (node.callFrame.url || "").split("/").pop()?.slice(0, 60) ?? "", hits: node.hitCount ?? 0 }));

const VISIBLE = `(el) => { const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0; }`;

const LIST_MENU_ITEMS = `(() => {
  const visible = ${VISIBLE};
  return [...document.querySelectorAll('[role^=menuitem],[role=menu] a,[role=menu] li')].filter(visible)
    .map((el) => (el.innerText || '').replace(/\\s+/g, ' ').trim().slice(0, 70)).filter(Boolean);
})()`;

/** @param {string} label */
const CLICK_MENU_ITEM = (label) => `(() => {
  const visible = ${VISIBLE};
  const items = [...document.querySelectorAll('[role^=menuitem],[role=menu] a,[role=menu] li')].filter(visible);
  const el = items.find((item) => (item.innerText || '').replace(/\\s+/g, ' ').trim().startsWith(${JSON.stringify(label)}));
  if (!el) return { clicked: false };
  el.click();
  return { clicked: true, text: (el.innerText || '').trim().slice(0, 60) };
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

const main = async () => {
  await mkdir(join(root, "evidence", "screens"), { recursive: true });
  const logPath = join(root, "logs", "ui-walk-menus.jsonl");
  const cdp = await connect();
  let step = 0;
  try {
    for (const line of (await readFile(logPath, "utf8")).split("\n")) {
      if (!line.trim()) continue;
      step = Math.max(step, JSON.parse(line).step ?? 0);
    }
  } catch {
    // журнала нет
  }
  process.stdout.write(`Продолжаю с шага ${step + 1}\n`);

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
        return {
          menus: [...document.querySelectorAll('[role=menu]')].filter(visible).length,
          dialogs: [...document.querySelectorAll('[role=dialog]')].filter(visible).length,
          settings: !!document.querySelector('.navScroller__409aa'),
        };
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
    const record = { step, label, clicked: false, screenshot: null, topFunctions: [], error: null };
    try {
      await cdp.send("Profiler.start");
      const outcome = await action();
      record.clicked = Boolean(outcome?.clicked ?? outcome?.opened ?? true);
      await sleep(900);
      const { profile } = await cdp.send("Profiler.stop");
      record.topFunctions = topFunctions(profile);
      const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
      const slug = label.replace(/[^\p{L}\p{N}]+/gu, "-").slice(0, 48) || `step-${step}`;
      const shotPath = join(root, "evidence", "screens", `ui-menu2-${String(step).padStart(2, "0")}-${slug}.png`);
      await writeFile(shotPath, Buffer.from(shot.data, "base64"));
      record.screenshot = shotPath.replace(`${root}/`, "");
    } catch (error) {
      record.error = error instanceof Error ? error.message : String(error);
      process.stderr.write(`[${step}] ОШИБКА "${label}": ${record.error}\n`);
    }
    await appendFile(logPath, `${JSON.stringify(record)}\n`);
    process.stdout.write(`[${String(step).padStart(2, "0")}] ${label} | ок=${record.clicked}${record.error ? ` | ${record.error}` : ""}\n`);
    if (record.error && /таймаут|закрыто/.test(record.error)) throw new Error("CDP отвалился");
    return record;
  };

  try {
    await cdp.send("Runtime.enable");
    await cdp.send("Profiler.enable");
    await cdp.send("Page.enable");

    // A. оставшиеся пункты меню сервера
    const leftovers = [
      "Показать все каналы",
      "Параметры уведомлений",
      "Настройки конфиденциальности",
      "Скрыть заглушенные каналы",
      "Редактировать личный профиль сервера",
      "Изоляция сервера",
      "Пожаловаться на рейд",
      "Копировать ID сервера",
    ];
    for (const label of leftovers) {
      if (BLACKLIST.test(label)) {
        process.stdout.write(`ПРОПУСК (запрещено): Меню сервера → ${label}\n`);
        continue;
      }
      await stepRecord(`Меню сервера2 → ${label}`, async () => {
        const opened = await evaluate(cdp, OPEN_BY_ARIA("действия с сервером"));
        if (!opened.clicked) return { clicked: false };
        await sleep(700);
        return evaluate(cdp, CLICK_MENU_ITEM(label));
      });
      await escapeToBase();
    }

    // B. меню «Помощь»
    const helpItems = [];
    {
      const opened = await evaluate(cdp, OPEN_BY_ARIA("Помощь"));
      await sleep(800);
      if (opened.clicked) {
        helpItems.push(...(await evaluate(cdp, LIST_MENU_ITEMS)));
        process.stdout.write(`Помощь: пункты: ${helpItems.join(" | ")}\n`);
        await stepRecord("Помощь2: открыто", async () => opened);
        await escapeToBase();
      }
    }
    for (const label of helpItems) {
      if (BLACKLIST.test(label)) {
        process.stdout.write(`ПРОПУСК (запрещено): Помощь → ${label}\n`);
        continue;
      }
      await stepRecord(`Помощь2 → ${label}`, async () => {
        const opened = await evaluate(cdp, OPEN_BY_ARIA("Помощь"));
        if (!opened.clicked) return { clicked: false };
        await sleep(700);
        return evaluate(cdp, CLICK_MENU_ITEM(label));
      });
      await escapeToBase();
    }

    // C. контекстное меню канала (доверенный правый клик)
    const channelItems = [];
    {
      const coords = await evaluate(
        cdp,
        `(() => {
          const visible = ${VISIBLE};
          const el = [...document.querySelectorAll('[aria-label]')].filter(visible)
            .find((node) => /первый тестовый|новости \\(текстовый канал\\)/i.test(node.getAttribute('aria-label') || ''));
          if (!el) return null;
          el.scrollIntoView({ block: 'center' });
          const r = el.getBoundingClientRect();
          return { x: Math.round(r.x + r.width / 2), y: Math.round(r.y + r.height / 2), aria: el.getAttribute('aria-label') };
        })()`,
      );
      if (coords) {
        await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", x: coords.x, y: coords.y, button: "right", buttons: 2, clickCount: 1 });
        await sleep(60);
        await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: coords.x, y: coords.y, button: "right", buttons: 0, clickCount: 1 });
        await sleep(800);
        channelItems.push(...(await evaluate(cdp, LIST_MENU_ITEMS)));
        process.stdout.write(`Меню канала: пункты: ${channelItems.join(" | ")}\n`);
        await stepRecord("Меню канала2: открыто", async () => ({ clicked: true, aria: coords.aria }));
        await escapeToBase();
      } else {
        process.stdout.write("Текстовый канал не найден для меню\n");
      }
    }
    for (const label of channelItems) {
      if (BLACKLIST.test(label)) {
        process.stdout.write(`ПРОПУСК (запрещено): Меню канала → ${label}\n`);
        continue;
      }
      await stepRecord(`Меню канала2 → ${label}`, async () => {
        const coords = await evaluate(
          cdp,
          `(() => {
            const visible = ${VISIBLE};
            const el = [...document.querySelectorAll('[aria-label]')].filter(visible)
              .find((node) => /новости \\(текстовый канал\\)/i.test(node.getAttribute('aria-label') || ''));
            if (!el) return null;
            el.scrollIntoView({ block: 'center' });
            const r = el.getBoundingClientRect();
            return { x: Math.round(r.x + r.width / 2), y: Math.round(r.y + r.height / 2) };
          })()`,
        );
        if (!coords) return { clicked: false };
        await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", x: coords.x, y: coords.y, button: "right", buttons: 2, clickCount: 1 });
        await sleep(60);
        await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: coords.x, y: coords.y, button: "right", buttons: 0, clickCount: 1 });
        await sleep(700);
        return evaluate(cdp, CLICK_MENU_ITEM(label));
      });
      await escapeToBase();
    }

    // D. панель голоса: меню устройств ввода/вывода (только открыть и зафиксировать)
    for (const aria of ["Настройки ввода", "Настройки вывода"]) {
      await stepRecord(`Голосовая панель → ${aria}`, async () => {
        const opened = await evaluate(cdp, OPEN_BY_ARIA(aria));
        if (!opened.clicked) return { clicked: false };
        await sleep(800);
        const items = await evaluate(cdp, LIST_MENU_ITEMS);
        process.stdout.write(`${aria}: ${items.join(" | ")}\n`);
        return opened;
      });
      await escapeToBase();
    }

    process.stdout.write(`\nГотово: шагов теперь ${step} в ${logPath}\n`);
  } finally {
    cdp.close();
  }
};

await main();
