// @ts-check
// Граница с c++filt. Контракт: tests/smoke/cxxfilt.smoke.test.mjs.

import { spawn } from "node:child_process";

/**
 * Деманглит имена через `c++filt`: по имени на строку stdin, по строке на
 * имя stdout (порядок сохраняется; неманагленные имена возвращаются как есть).
 * Пустой список → `[]` без запуска процесса.
 * @param {readonly string[]} names
 * @param {{ timeoutMs?: number }} [options]
 * @returns {Promise<string[]>}
 */
export async function demangle(names, options = {}) {
  if (names.length === 0) return [];
  const timeoutMs = options.timeoutMs ?? 30000;
  return new Promise((resolve, reject) => {
    const child = spawn("c++filt", [], { stdio: ["pipe", "pipe", "pipe"] });
    let settled = false;
    /** @param {(value: any) => void} callback @param {any} value */
    const finish = (callback, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      callback(value);
    };
    /** @type {Buffer[]} */
    const stdout = [];
    /** @type {Buffer[]} */
    const stderr = [];
    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      finish(reject, new Error(`c++filt: таймаут ${timeoutMs} мс`));
    }, timeoutMs);
    child.stdout.on("data", (chunk) => stdout.push(Buffer.from(chunk)));
    child.stderr.on("data", (chunk) => stderr.push(Buffer.from(chunk)));
    child.stdin.on("error", (error) => {
      finish(reject, new Error(`c++filt: stdin: ${error.message}`));
    });
    child.once("error", (error) => finish(reject, error));
    child.once("close", (code) => {
      if (code !== 0) {
        finish(reject, new Error(Buffer.concat(stderr).toString("utf8") || `c++filt завершился с кодом ${code}`));
        return;
      }
      const output = Buffer.concat(stdout).toString("utf8").replace(/\r?\n$/, "");
      finish(resolve, output.split(/\r?\n/));
    });
    try {
      child.stdin.end(`${names.join("\n")}\n`);
    } catch (error) {
      finish(reject, new Error(`c++filt: stdin: ${error instanceof Error ? error.message : String(error)}`));
    }
  });
}
