#!/usr/bin/env node
// @ts-check
// Прогон настроек Discord по всем разделам меню с логированием:
//   - клик по каждому пункту (лог window.__clickLog),
//   - скриншот каждого экрана,
//   - профиль JS-функций, выполненных во время клика (CDP Profiler),
//   - ошибки консоли за шаг.
//
// Запуск: node scripts/ui-walk-settings.mjs
// Вход: Discord должен быть запущен с --remote-debugging-port=9222 и открытыми
// настройками (scripts/cdp.mjs clicktext "Настройки пользователя").
//
// Результаты: logs/ui-walk-settings.jsonl, evidence/screens/ui-settings-*.png

/** @typedef {import("./cdp-types.js").CdpEvent} CdpEvent */
/** @typedef {import("./cdp-types.js").CdpEventClient} CdpEventClient */
/** @typedef {import("./cdp-types.js").CdpMessage} CdpMessage */
/** @typedef {import("./cdp-types.js").CpuProfile} CpuProfile */
/** @typedef {import("./cdp-types.js").TopFunction} TopFunction */
/** @typedef {import("./cdp-types.js").WalkRecord} WalkRecord */

import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const endpoint = process.env.CDP_ENDPOINT ?? "http://127.0.0.1:9222";
const BLACKLIST = /^(Выйти|Log ?Out|Sign ?Out|Отключиться|Удалить)/i;

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
    for (const [id, settle] of pending) settle({ error: { message: "CDP соединение закрыто (страница умерла?)" } });
    pending.clear();
  });
  /** @type {CdpEventClient["send"]} */
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = nextId++;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`${method}: таймаут 15с (страница отвечает?)`));
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

const NAV_QUERY = `(() => {
  const nav = document.querySelector('.navScroller__409aa');
  if (!nav) return [];
  return [...nav.querySelectorAll('div')]
    .filter((el) => el.textContent.trim() && [...el.children].every((child) => !child.textContent.trim()))
    .map((el) => ({ text: el.textContent.replace(/\\s+/g, ' ').trim(), cls: (el.className || '').toString().slice(0, 40) }));
})()`;

/** @param {string} label */
const CLICK_BY_TEXT = (label) => `(() => {
  const label = ${JSON.stringify(label)};
  const nav = document.querySelector('.navScroller__409aa');
  if (!nav) return { clicked: false };
  const candidates = [...nav.querySelectorAll('div')].filter((el) => el.textContent.replace(/\\s+/g, ' ').trim() === label);
  const el = candidates[candidates.length - 1];
  if (!el) return { clicked: false };
  el.scrollIntoView({ block: 'center' });
  el.click();
  return { clicked: true, text: el.textContent.trim() };
})()`;

/** @param {CpuProfile} profile @returns {TopFunction[]} */
const topFunctions = (profile) =>
  profile.nodes
    .filter((node) => node.hitCount)
    .sort((left, right) => (right.hitCount ?? 0) - (left.hitCount ?? 0))
    .slice(0, 8)
    .map((node) => ({
      fn: node.callFrame.functionName || "(anonymous)",
      url: (node.callFrame.url || "").split("/").pop()?.slice(0, 60) ?? "",
      hits: node.hitCount ?? 0,
    }));

const main = async () => {
  await mkdir(join(root, "evidence", "screens"), { recursive: true });
  const cdp = await connect();
  const logPath = join(root, "logs", "ui-walk-settings.jsonl");
  /** @type {WalkRecord[]} */
  const records = [];
  try {
    await cdp.send("Runtime.enable");
    await cdp.send("Profiler.enable");
    await cdp.send("Page.enable");
    // логгер кликов в странице
    await evaluate(cdp, `(() => {
      if (window.__clickLog) return 'exists';
      window.__clickLog = [];
      document.addEventListener('click', (event) => {
        const el = event.target;
        const rect = el.getBoundingClientRect ? el.getBoundingClientRect() : { x: 0, y: 0 };
        window.__clickLog.push({
          at: Date.now(),
          aria: el.getAttribute ? el.getAttribute('aria-label') : null,
          text: (el.innerText || el.textContent || '').replace(/\\s+/g, ' ').trim().slice(0, 100),
          x: Math.round(rect.x), y: Math.round(rect.y),
        });
      }, true);
      return 'installed';
    })()`);

    let nav = await evaluate(cdp, NAV_QUERY);
    if (!nav.length) {
      process.stderr.write("Меню настроек не найдено: открой настройки (Настройки пользователя) и повтори.\n");
      return;
    }
    process.stdout.write(`Пунктов меню: ${nav.length}\n`);

    let step = 0;
    let index = 0;
    const { appendFile } = await import("node:fs/promises");
    while (index < nav.length) {
      const label = nav[index].text;
      if (BLACKLIST.test(label)) {
        process.stdout.write(`[${index}] ПРОПУСК (запрещено): ${label}\n`);
        index += 1;
        continue;
      }
      step += 1;
      const consoleBefore = cdp.events.length;
      /** @type {WalkRecord} */
      const record = { step, label, clicked: false, screenshot: null, consoleErrors: 0, topFunctions: [], error: null };
      try {
        await cdp.send("Profiler.start");
        const clicked = await evaluate(cdp, CLICK_BY_TEXT(label));
        record.clicked = clicked.clicked;
        await sleep(1000);
        const { profile } = await cdp.send("Profiler.stop");
        record.topFunctions = topFunctions(profile);
        const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
        const slug = label.replace(/[^\p{L}\p{N}]+/gu, "-").slice(0, 40) || `item-${index}`;
        const shotPath = join(root, "evidence", "screens", `ui-setting-${String(step).padStart(2, "0")}-${slug}.png`);
        await writeFile(shotPath, Buffer.from(shot.data, "base64"));
        record.screenshot = shotPath.replace(`${root}/`, "");
      } catch (error) {
        record.error = error instanceof Error ? error.message : String(error);
        process.stderr.write(`[${step}] ОШИБКА на "${label}": ${record.error}\n`);
      }
      record.consoleErrors = cdp.events
        .slice(consoleBefore)
        .filter((event) => event.method === "Runtime.exceptionThrown" || (event.method === "Runtime.consoleAPICalled" && event.params?.type === "error"))
        .length;
      records.push(record);
      await appendFile(logPath, `${JSON.stringify(record)}\n`);
      process.stdout.write(
        `[${String(step).padStart(2, "0")}] ${label} | клик=${record.clicked} | ошибок: ${record.consoleErrors} | топ: ${record.topFunctions.slice(0, 3).map((f) => f.fn).join(", ")}${record.error ? ` | ОШИБКА: ${record.error}` : ""}\n`,
      );
      if (record.error && /таймаут|закрыто/.test(record.error)) break;
      index += 1;
      try {
        nav = await evaluate(cdp, NAV_QUERY);
      } catch (error) {
        process.stderr.write(`Меню недоступно: ${error instanceof Error ? error.message : String(error)}\n`);
        break;
      }
    }

    /** @type {any[]} */
    const clicks = await evaluate(cdp, "window.__clickLog ? window.__clickLog.splice(0) : []");
    await writeFile(logPath, `${records.map((r) => JSON.stringify(r)).join("\n")}\n`);
    const clicksPath = join(root, "logs", "ui-clicks-settings.jsonl");
    await writeFile(clicksPath, `${clicks.map((c) => JSON.stringify(c)).join("\n")}\n`);
    process.stdout.write(`\nГотово: ${records.length} шагов -> ${logPath}\nКлики: ${clicksPath}\nСкриншоты: evidence/screens/\n`);
  } finally {
    cdp.close();
  }
};

await main();
