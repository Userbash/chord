// @ts-check
// Поиск и запуск headless-браузера для e2e-тестов.
// Нет браузера — тесты сами пропускаются (skip с подсказкой про CHROME_BIN).

import { spawn } from "node:child_process";
import { accessSync, constants, statSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { delimiter, join } from "node:path";
import { waitFor } from "./tmp.mjs";

/** @type {string[]} */
const BROWSER_NAMES = [
  "chromium",
  "chromium-browser",
  "google-chrome",
  "google-chrome-stable",
  "brave-browser",
  "brave",
  "microsoft-edge",
  "microsoft-edge-stable",
];

/**
 * Ищет исполняемый файл по имени в PATH.
 * @param {string} name
 * @returns {string | null}
 */
export function findInPath(name) {
  for (const dir of (process.env.PATH ?? "").split(delimiter)) {
    if (!dir) continue;
    const candidate = join(dir, name);
    try {
      accessSync(candidate, constants.X_OK);
      if (statSync(candidate).isFile()) return candidate;
    } catch {
      // файла нет — пробуем следующий каталог
    }
  }
  return null;
}

/**
 * Браузер для e2e: `CHROME_BIN` / `CHROME_PATH` или первое известное имя в PATH.
 * @param {Record<string, string | undefined>} [env]
 * @returns {string | null}
 */
export function findBrowser(env = process.env) {
  const fromEnv = env.CHROME_BIN ?? env.CHROME_PATH;
  if (fromEnv) return fromEnv;
  for (const name of BROWSER_NAMES) {
    const found = findInPath(name);
    if (found) return found;
  }
  return null;
}

/**
 * @typedef {object} Browser
 * @property {import("node:child_process").ChildProcess} child
 * @property {string} endpoint CDP endpoint вида `http://127.0.0.1:<port>`
 */

/**
 * Запускает headless-браузер с remote-debugging и открытым `url`.
 * Порт выбирает сам браузер (`--remote-debugging-port=0`): фактический порт
 * берётся из файла `DevToolsActivePort` в каталоге профиля.
 * @param {{ bin: string, url: string, userDataDir: string }} options
 * @returns {Promise<Browser>}
 */
export async function launchBrowser(options) {
  const args = [
    "--headless=new",
    "--disable-gpu",
    "--disable-software-rasterizer",
    "--use-gl=swiftshader",
    "--disable-dev-shm-usage",
    "--no-first-run",
    "--no-default-browser-check",
    "--no-sandbox",
    `--user-data-dir=${options.userDataDir}`,
    "--remote-debugging-port=0",
    options.url,
  ];
  const child = spawn(options.bin, args, { stdio: "ignore" });
  const portFile = join(options.userDataDir, "DevToolsActivePort");
  /** @type {string} */
  let endpoint = "";
  try {
    await waitFor(
      async () => {
        try {
          const text = await readFile(portFile, "utf8");
          const port = text.split("\n")[0]?.trim();
          if (!port) return false;
          endpoint = `http://127.0.0.1:${port}`;
          return true;
        } catch {
          return false;
        }
      },
      { timeoutMs: 15000 },
    );
  } catch (error) {
    child.kill("SIGKILL");
    throw new Error(`Браузер не отдал DevToolsActivePort: ${options.bin}`, { cause: error });
  }
  return { child, endpoint };
}

/**
 * Останавливает браузер.
 * @param {Browser} browser
 */
export function stopBrowser(browser) {
  browser.child.kill("SIGKILL");
}
