// @ts-check

import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { isCallPayload } from "../core/calls.mjs";
import { projectRoot } from "../core/paths.mjs";

/** @typedef {import("frida").Device} Device */
/** @typedef {Pick<import("frida").Process, "pid" | "name">} ProcessInfo */
/** @typedef {import("frida").Session} Session */
/** @typedef {import("frida").Script} Script */
/** @typedef {import("../core/types.js").CallRecord} CallRecord */
/** @typedef {{ type: string, payload?: unknown, [key: string]: unknown }} HookMessage */
/** @typedef {{ inventory(options: object): Promise<unknown>, hook(options: object): Promise<unknown>, unhook(): Promise<unknown> }} AgentExports */
/** @typedef {Script & { exports: AgentExports }} AgentScript */

/** @returns {Promise<Device>} */
export async function loadDevice() {
  const frida = await import("frida");
  return frida.getLocalDevice();
}

/** @returns {Promise<string>} */
export async function loadAgentSource() {
  return readFile(join(projectRoot(), "scripts", "frida-log-calls.js"), "utf8");
}

/** @param {Pick<Device, "enumerateProcesses">} device @returns {Promise<ProcessInfo[]>} */
export async function listProcesses(device) {
  return (await device.enumerateProcesses()).map(({ pid, name }) => ({ pid, name }));
}

/**
 * @param {Pick<Device, "attach">} device
 * @param {number} pid
 * @param {{ agentSource?: string, captureMessages?: boolean }} [options]
 * @remarks На hook-пути массив сообщений не читается, а сообщений могут приходить тысячи.
 * @returns {Promise<{ session: Session, script: Script, messages: import("frida").Message[] }>}
 */
export async function attach(device, pid, options = {}) {
  const session = await device.attach(pid);
  const source = options.agentSource ?? await loadAgentSource();
  const script = await session.createScript(source, { name: "discord-lab" });
  /** @type {import("frida").Message[]} */
  const messages = [];
  script.message.connect((message) => {
    if (options.captureMessages === true) messages.push(message);
  });
  await script.load();
  return { session, script, messages };
}

/** @param {AgentScript} script @param {object} [options] @returns {Promise<unknown>} */
export function collectInventory(script, options = {}) {
  return script.exports.inventory(options);
}

/** @param {HookMessage} message @returns {CallRecord | null} */
export function parseHookMessage(message) {
  if (message.type !== "send") return null;
  return isCallPayload(message.payload) ? message.payload : null;
}

/** @param {AgentScript} script @param {{ pattern?: RegExp | string } & Record<string, unknown>} options @returns {Promise<unknown>} */
export function startHook(script, options) {
  // frida RPC не сериализует RegExp; агент ждёт строку (scripts/frida-log-calls.js: pattern — СТРОКА)
  const pattern = options.pattern instanceof RegExp ? options.pattern.source : options.pattern;
  return script.exports.hook({ ...options, pattern });
}

/** @param {AgentScript} script @returns {Promise<void>} */
export async function stopHook(script) {
  await script.exports.unhook().catch(() => {});
  await script.unload().catch(() => {});
}
