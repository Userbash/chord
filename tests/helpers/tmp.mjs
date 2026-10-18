// @ts-check
// Временные каталоги для тестов: только /tmp, никаких записей в рабочие каталоги проекта.

import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

/**
 * Создаёт временный каталог и удаляет его после теста.
 * @param {import("node:test").TestContext} t
 * @param {string} [prefix]
 * @returns {Promise<string>}
 */
export async function makeTmpDir(t, prefix = "discord-lab-test-") {
  const dir = await mkdtemp(join(tmpdir(), prefix));
  t.after(() => rm(dir, { recursive: true, force: true }));
  return dir;
}

/**
 * Ждёт выполнения условия (например, появления файла).
 * @param {() => Promise<boolean>} check
 * @param {{ timeoutMs?: number, stepMs?: number }} [options]
 * @returns {Promise<void>}
 */
export async function waitFor(check, options = {}) {
  const timeoutMs = options.timeoutMs ?? 10000;
  const stepMs = options.stepMs ?? 100;
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    if (await check()) return;
    if (Date.now() > deadline) {
      throw new Error(`waitFor: условие не выполнено за ${timeoutMs} мс`);
    }
    await new Promise((done) => setTimeout(done, stepMs));
  }
}
