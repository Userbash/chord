// @ts-check
// Подмена адаптера Frida для unit-тестов CLI: живой Frida/Discord не используются.
// Подключение — через module.registerHooks (resolve только для import из src/cli/run.mjs);
// продукт и его адаптер не изменяются.

import { registerHooks } from "node:module";
import { parseHookMessage as realParseHookMessage } from "../../src/adapters/frida.mjs";

/**
 * @typedef {object} FakeFrida
 * @property {() => Promise<any>} loadDevice
 * @property {(device: any) => Promise<Array<{ pid: number, name: string }>>} listProcesses
 * @property {(device: any, pid: number, options?: object) => Promise<any>} attach
 * @property {(script: any, options?: object) => Promise<any>} collectInventory
 * @property {(script: any, options?: object) => Promise<any>} startHook
 * @property {(script: any) => Promise<void>} stopHook
 */

const ADAPTER_SUFFIX = "adapters/frida.mjs";
const RUN_SUFFIX = "/src/cli/run.mjs";

/** @returns {FakeFrida} */
function fake() {
  const configured = /** @type {any} */ (globalThis).__FAKE_FRIDA__;
  if (!configured) throw new Error("fake-frida: не настроен globalThis.__FAKE_FRIDA__");
  return configured;
}

/** @returns {Promise<any>} */
export async function loadDevice() {
  return fake().loadDevice();
}

/** Не используется CLI (агент загружает адаптер внутри attach). @returns {Promise<string>} */
export async function loadAgentSource() {
  return "";
}

/** @param {any} device @returns {Promise<Array<{ pid: number, name: string }>>} */
export async function listProcesses(device) {
  return fake().listProcesses(device);
}

/** @param {any} device @param {number} pid @param {object} [options] @returns {Promise<any>} */
export async function attach(device, pid, options = {}) {
  return fake().attach(device, pid, options);
}

/** @param {any} script @param {object} [options] @returns {Promise<any>} */
export function collectInventory(script, options = {}) {
  return fake().collectInventory(script, options);
}

/**
 * Разбор сообщений агента — тот же, что у продуктового адаптера (чистая функция).
 * @param {any} message
 * @returns {any}
 */
export function parseHookMessage(message) {
  return realParseHookMessage(message);
}

/** @param {any} script @param {object} [options] @returns {Promise<any>} */
export function startHook(script, options = {}) {
  return fake().startHook(script, options);
}

/** @param {any} script @returns {Promise<void>} */
export function stopHook(script) {
  return fake().stopHook(script);
}

/**
 * Регистрирует resolve-хук: `await import("../adapters/frida.mjs")` из `src/cli/run.mjs`
 * получает этот модуль. Вызывать до импорта `run.mjs`.
 * @returns {{ configure: (fakeFrida: FakeFrida) => void, restore: () => void }}
 */
export function installFakeFrida() {
  const hooks = registerHooks({
    resolve(/** @type {string} */ specifier, /** @type {import("node:module").ResolveHookContext} */ context, /** @type {any} */ nextResolve) {
      const fromRun = Boolean(context.parentURL?.endsWith(RUN_SUFFIX));
      if (fromRun && specifier.endsWith(ADAPTER_SUFFIX)) {
        return { url: import.meta.url, shortCircuit: true };
      }
      return nextResolve(specifier, context);
    },
  });
  return {
    configure(fakeFrida) {
      /** @type {any} */ (globalThis).__FAKE_FRIDA__ = fakeFrida;
    },
    restore() {
      hooks.deregister();
      delete /** @type {any} */ (globalThis).__FAKE_FRIDA__;
    },
  };
}
