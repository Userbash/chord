// @ts-check
// Запуск реального CLI bin/discord-lab.mjs в smoke/e2e-тестах.

import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

/** Путь к точке входа CLI. @returns {string} */
export function cliBin() {
  return fileURLToPath(new URL("../../bin/discord-lab.mjs", import.meta.url));
}

/** Корень проекта (tests/helpers → ../..). @returns {string} */
export function projectDir() {
  return fileURLToPath(new URL("../../", import.meta.url));
}

/**
 * @typedef {object} CliResult
 * @property {number | null} code
 * @property {string | null} signal
 * @property {string} stdout
 * @property {string} stderr
 */

/**
 * Запускает `node bin/discord-lab.mjs <args>` и собирает stdout/stderr/код выхода.
 * @param {readonly string[]} args
 * @param {{ env?: Record<string, string>, cwd?: string, timeoutMs?: number }} [options]
 * @returns {Promise<CliResult>}
 */
export function runCli(args, options = {}) {
  const timeoutMs = options.timeoutMs ?? 20000;
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [cliBin(), ...args], {
      cwd: options.cwd ?? projectDir(),
      env: { ...process.env, ...options.env },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    const out = child.stdout;
    const err = child.stderr;
    if (!out || !err) {
      child.kill("SIGKILL");
      reject(new Error("CLI: не удалось захватить stdout/stderr"));
      return;
    }
    out.setEncoding("utf8");
    err.setEncoding("utf8");
    out.on("data", (chunk) => {
      stdout += chunk;
    });
    err.on("data", (chunk) => {
      stderr += chunk;
    });
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      reject(new Error(`CLI не завершился за ${timeoutMs} мс: discord-lab ${args.join(" ")}`));
    }, timeoutMs);
    child.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once("close", (code, signal) => {
      clearTimeout(timer);
      resolve({ code, signal, stdout, stderr });
    });
  });
}

/**
 * Парсит stdout CLI как JSON; при ошибке — понятное исключение с превью вывода.
 * @param {string} stdout
 * @returns {any}
 */
export function parseJson(stdout) {
  try {
    return JSON.parse(stdout.trim());
  } catch (error) {
    throw new Error(`stdout не является JSON: ${stdout.slice(0, 200)}`, { cause: error });
  }
}
