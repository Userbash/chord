#!/usr/bin/env node
// @ts-check
// Минимальный Chrome DevTools Protocol клиент (Node 24: fetch + WebSocket из коробки).
//
// Использование:
//   node scripts/cdp.mjs targets
//   node scripts/cdp.mjs eval "<js>"
//   node scripts/cdp.mjs list "<css>"                # список элементов: tag, role, aria, text, rect
//   node scripts/cdp.mjs click "<css>"               # клик по элементу
//   node scripts/cdp.mjs clicktext "<текст>"         # клик по тексту/aria-label/title (contains, без регистра)
//   node scripts/cdp.mjs key "<Escape|ctrl+,|Enter>" # клавиша/хоткей через Input.dispatchKeyEvent
//   node scripts/cdp.mjs shot <file.png>
//   node scripts/cdp.mjs log-click                   # включить сборщик кликов в window.__clickLog
//   node scripts/cdp.mjs dump-clicks <file.jsonl>    # слить window.__clickLog в файл (JSONL)
//
// Переменные: CDP_ENDPOINT (default http://127.0.0.1:9222),
//             CDP_TARGET (подстрока URL целевой страницы, default discord.com)

/** @typedef {import("./cdp-types.js").CdpTarget} CdpTarget */
/** @typedef {import("./cdp-types.js").CdpMessage} CdpMessage */
/** @typedef {import("./cdp-types.js").CdpClient} CdpClient */
/** @typedef {import("./cdp-types.js").ClickOutcome} ClickOutcome */
/** @typedef {CdpTarget & { webSocketDebuggerUrl: string }} PageTarget */
/**
 * @typedef {object} KeyInfo
 * @property {string} key
 * @property {string} code
 * @property {number} vk
 * @property {string} [text]
 */

const endpoint = process.env.CDP_ENDPOINT ?? "http://127.0.0.1:9222";
const targetHint = process.env.CDP_TARGET ?? "discord.com";

/** @returns {Promise<CdpTarget[]>} */
async function listTargets() {
  const res = await fetch(`${endpoint}/json/list`);
  if (!res.ok) throw new Error(`CDP list failed: ${res.status}`);
  return /** @type {CdpTarget[]} */ (await res.json());
}

/**
 * @param {CdpTarget[]} targets
 * @returns {PageTarget | undefined}
 */
function pickTarget(targets) {
  const pages = targets.filter((t) => t.type === "page" && t.webSocketDebuggerUrl);
  const page =
    pages.find((t) => (t.url ?? "").includes(targetHint)) ??
    pages.find((t) => (t.url ?? "").includes("discord")) ??
    pages[0];
  return /** @type {PageTarget | undefined} */ (page);
}

/**
 * @param {string} wsUrl
 * @returns {Promise<CdpClient>}
 */
async function connect(wsUrl) {
  const ws = new WebSocket(wsUrl);
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
      pending.set(id, (message) =>
        message.error ? reject(new Error(`${method}: ${message.error.message}`)) : resolve(message.result),
      );
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
  const result = await cdp.send("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (result.exceptionDetails) {
    throw new Error(result.exceptionDetails.exception?.description ?? result.exceptionDetails.text);
  }
  return result.result.value;
}

/** @type {Record<string, KeyInfo>} */
const KEY_INFO = {
  Escape: { key: "Escape", code: "Escape", vk: 27 },
  Enter: { key: "Enter", code: "Enter", vk: 13 },
  Tab: { key: "Tab", code: "Tab", vk: 9 },
  ArrowDown: { key: "ArrowDown", code: "ArrowDown", vk: 40 },
  ArrowUp: { key: "ArrowUp", code: "ArrowUp", vk: 38 },
  ",": { key: ",", code: "Comma", vk: 188, text: "," },
};

/**
 * @param {CdpClient} cdp
 * @param {string} spec
 */
async function pressKey(cdp, spec) {
  const parts = spec.split("+");
  const keyName = parts.pop() ?? "";
  let modifiers = 0;
  for (const part of parts) {
    if (/^ctrl$/i.test(part)) modifiers |= 2;
    if (/^shift$/i.test(part)) modifiers |= 8;
    if (/^alt$/i.test(part)) modifiers |= 1;
    if (/^(meta|cmd|super)$/i.test(part)) modifiers |= 4;
  }
  const info = KEY_INFO[keyName] ?? {
    key: keyName,
    code: keyName.length === 1 ? `Key${keyName.toUpperCase()}` : keyName,
    vk: keyName.length === 1 ? keyName.toUpperCase().charCodeAt(0) : 0,
  };
  await cdp.send("Input.dispatchKeyEvent", {
    type: "rawKeyDown",
    modifiers,
    key: info.key,
    code: info.code,
    windowsVirtualKeyCode: info.vk,
    nativeVirtualKeyCode: info.vk,
  });
  await cdp.send("Input.dispatchKeyEvent", {
    type: "keyUp",
    modifiers,
    key: info.key,
    code: info.code,
    windowsVirtualKeyCode: info.vk,
    nativeVirtualKeyCode: info.vk,
  });
}

const CLICK_LOGGER = `(() => {
  if (window.__clickLog) return 'already';
  window.__clickLog = [];
  const info = (el) => {
    const r = el.getBoundingClientRect ? el.getBoundingClientRect() : {x:0,y:0,width:0,height:0};
    return {
      tag: el.tagName || null,
      role: el.getAttribute ? el.getAttribute('role') : null,
      aria: el.getAttribute ? el.getAttribute('aria-label') : null,
      title: el.getAttribute ? el.getAttribute('title') : null,
      text: (el.innerText || el.textContent || '').replace(/\\s+/g, ' ').trim().slice(0, 120),
      x: Math.round(r.x), y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height),
    };
  };
  document.addEventListener('click', (e) => {
    window.__clickLog.push({ at: Date.now(), event: 'click', target: info(e.target) });
  }, true);
  document.addEventListener('contextmenu', (e) => {
    window.__clickLog.push({ at: Date.now(), event: 'contextmenu', target: info(e.target) });
  }, true);
  return 'installed';
})()`;

const main = async () => {
  const [command, argument = ""] = process.argv.slice(2);
  const targets = await listTargets();
  if (command === "targets") {
    targets.forEach((t, index) => {
      process.stdout.write(`${index}\t${t.type}\t${t.title}\t${t.url}\n`);
    });
    return;
  }
  const target = pickTarget(targets);
  if (!target) throw new Error("Нет подходящей страницы. Проверь CDP_ENDPOINT/CDP_TARGET.");
  const cdp = await connect(target.webSocketDebuggerUrl);
  try {
    if (command === "eval") {
      const value = await evaluate(cdp, argument);
      process.stdout.write(`${JSON.stringify(value, null, 2)}\n`);
    } else if (command === "list") {
      const selector = JSON.stringify(argument);
      const value = await evaluate(cdp, `(() => {
        const els = [...document.querySelectorAll(${selector})];
        return els.slice(0, 200).map((el) => {
          const r = el.getBoundingClientRect();
          const style = getComputedStyle(el);
          return {
            tag: el.tagName, role: el.getAttribute('role'), aria: el.getAttribute('aria-label'),
            text: (el.innerText || '').replace(/\\s+/g, ' ').trim().slice(0, 100),
            visible: r.width > 0 && r.height > 0 && style.visibility !== 'hidden',
            rect: [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)],
          };
        });
      })()`);
      process.stdout.write(`${JSON.stringify(value, null, 1)}\n`);
    } else if (command === "click") {
      const selector = JSON.stringify(argument);
      const value = await evaluate(cdp, `(() => {
        const el = document.querySelector(${selector});
        if (!el) return null;
        el.scrollIntoView({ block: 'center' });
        el.click();
        const r = el.getBoundingClientRect();
        return { clicked: true, tag: el.tagName, aria: el.getAttribute('aria-label'),
                 text: (el.innerText || '').replace(/\\s+/g, ' ').trim().slice(0, 80),
                 rect: [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)] };
      })()`);
      process.stdout.write(`${JSON.stringify(value)}\n`);
    } else if (command === "clicktext") {
      const needle = JSON.stringify(argument.toLowerCase());
      const value = await evaluate(cdp, `(() => {
        const needle = ${needle};
        const candidates = [...document.querySelectorAll('button, [role="button"], [role="tab"], [role="menuitem"], [role="menuitemradio"], a, div[tabindex]')];
        for (const el of candidates) {
          const r = el.getBoundingClientRect();
          if (r.width === 0 || r.height === 0) continue;
          const hay = ((el.getAttribute('aria-label') || '') + ' ' + (el.getAttribute('title') || '') + ' ' + (el.innerText || '')).toLowerCase();
          if (hay.includes(needle)) {
            el.scrollIntoView({ block: 'center' });
            el.click();
            return { clicked: true, aria: el.getAttribute('aria-label'), text: (el.innerText || '').replace(/\\s+/g, ' ').trim().slice(0, 80) };
          }
        }
        return { clicked: false };
      })()`);
      process.stdout.write(`${JSON.stringify(value)}\n`);
    } else if (command === "rclick") {
      const needle = JSON.stringify(argument.toLowerCase());
      const coords = await evaluate(cdp, `(() => {
        const needle = ${needle};
        const candidates = [...document.querySelectorAll('button, [role="button"], [role="treeitem"], [role="menuitem"], a, div[tabindex], [aria-label]')];
        for (const el of candidates) {
          const r = el.getBoundingClientRect();
          if (r.width === 0 || r.height === 0) continue;
          const hay = ((el.getAttribute('aria-label') || '') + ' ' + (el.getAttribute('title') || '') + ' ' + (el.innerText || '')).toLowerCase();
          if (hay.includes(needle)) {
            el.scrollIntoView({ block: 'center' });
            const r2 = el.getBoundingClientRect();
            return { x: Math.round(r2.x + r2.width / 2), y: Math.round(r2.y + r2.height / 2), aria: el.getAttribute('aria-label'), text: (el.innerText || '').replace(/\\s+/g, ' ').trim().slice(0, 80) };
          }
        }
        return null;
      })()`);
      if (!coords) {
        process.stdout.write('{"clicked":false}\n');
      } else {
        await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", x: coords.x, y: coords.y, button: "right", buttons: 2, clickCount: 1 });
        await new Promise((done) => setTimeout(done, 40));
        await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: coords.x, y: coords.y, button: "right", buttons: 0, clickCount: 1 });
        process.stdout.write(`${JSON.stringify({ clicked: true, ...coords })}\n`);
      }
    } else if (command === "lclick") {
      const needle = JSON.stringify(argument.toLowerCase());
      const coords = await evaluate(cdp, `(() => {
        const needle = ${needle};
        const candidates = [...document.querySelectorAll('button, [role="button"], [role="treeitem"], a, div[tabindex], [aria-label], div[class*=clickable]')];
        for (const el of candidates) {
          const r = el.getBoundingClientRect();
          if (r.width === 0 || r.height === 0) continue;
          const hay = ((el.getAttribute('aria-label') || '') + ' ' + (el.getAttribute('title') || '') + ' ' + (el.innerText || '')).toLowerCase();
          if (hay.includes(needle)) {
            el.scrollIntoView({ block: 'center' });
            const r2 = el.getBoundingClientRect();
            return { x: Math.round(r2.x + r2.width / 2), y: Math.round(r2.y + r2.height / 2), aria: el.getAttribute('aria-label'), text: (el.innerText || '').replace(/\\s+/g, ' ').trim().slice(0, 80) };
          }
        }
        return null;
      })()`);
      if (!coords) {
        process.stdout.write('{"clicked":false}\n');
      } else {
        await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", x: coords.x, y: coords.y, button: "left", buttons: 1, clickCount: 1 });
        await new Promise((done) => setTimeout(done, 40));
        await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: coords.x, y: coords.y, button: "left", buttons: 0, clickCount: 1 });
        process.stdout.write(`${JSON.stringify({ clicked: true, ...coords })}\n`);
      }
    } else if (command === "menu") {
      const value = await evaluate(cdp, `(() => {
        const visible = (el) => { const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0; };
        return [...document.querySelectorAll('[role^=menuitem],[role=menu] li,[role=menu] a')].filter(visible)
          .map((el) => ({ role: el.getAttribute('role'), text: (el.innerText || '').replace(/\\s+/g, ' ').trim().slice(0, 70) }))
          .filter((entry) => entry.text);
      })()`);
      process.stdout.write(`${JSON.stringify(value, null, 1)}\n`);
    } else if (command === "key") {
      await pressKey(cdp, argument);
      process.stdout.write(`${JSON.stringify({ pressed: argument })}\n`);
    } else if (command === "shot") {
      const shot = await cdp.send("Page.captureScreenshot", { format: "png" });
      const { writeFile } = await import("node:fs/promises");
      await writeFile(argument, Buffer.from(shot.data, "base64"));
      process.stdout.write(`${JSON.stringify({ saved: argument, bytes: Buffer.byteLength(shot.data, "base64") })}\n`);
    } else if (command === "log-click") {
      const value = await evaluate(cdp, CLICK_LOGGER);
      process.stdout.write(`${JSON.stringify({ logger: value })}\n`);
    } else if (command === "dump-clicks") {
      /** @type {any[]} */
      const clicks = await evaluate(cdp, "window.__clickLog ? window.__clickLog.splice(0) : []");
      const { appendFile } = await import("node:fs/promises");
      const lines = clicks.map((entry) => JSON.stringify(entry)).join("\n");
      if (lines) await appendFile(argument, `${lines}\n`);
      process.stdout.write(`${JSON.stringify({ dumped: clicks.length, file: argument })}\n`);
    } else {
      process.stderr.write(`Неизвестная команда: ${command}\n`);
      process.exitCode = 1;
    }
  } finally {
    cdp.close();
  }
};

await main();
