#!/usr/bin/env node
// @ts-check
// Host-side runner for the Discord function lab.
//
// Commands:
//   pids                                            list Discord processes visible to Frida
//   inventory [--pid N] [--max-symbols N]           symbol/export inventory -> functions/
//   hook [--pid N | --spawn] [options]              log calls of matching functions -> logs/
//
// hook options:
//   --module NAME      restrict to a module (repeatable); default: all discord*.node + updater.node
//   --kind symbols|exports   hook source (default: symbols; Discord hides exports)
//   --pattern REGEX    restrict hooked names (default: '.', everything)
//   --limit N          maximum hooks (default 64)
//   --arg-count N      0..6 pointer arguments recorded per call (default 2)
//   --duration-ms N    stop after N ms (default 0: run until Ctrl-C)
//   --out FILE         JSONL target (default: logs/calls-<pid>.jsonl)
//
// Attaching to a non-child process needs Yama to allow it:
//   sudo sysctl kernel.yama.ptrace_scope=0   (restore to 1 afterwards)
// Use --spawn to start Discord under Frida instead, which works with ptrace_scope=1.

import frida from "frida";
import { appendFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** @typedef {import("frida").Device} Device */
/** @typedef {import("frida").Message} FridaMessage */
/** @typedef {import("frida").Session} Session */
/** @typedef {import("frida").Script} Script */
/** @typedef {{ pid: number, name: string }} TargetProcess */
/** @typedef {(...args: any[]) => Promise<any>} AgentMethod */
/**
 * @typedef {object} SymbolEntry
 * @property {string} name
 * @property {string} address
 * @property {string} offset
 */
/**
 * @typedef {object} ExportEntry
 * @property {string} name
 * @property {string} type
 * @property {string} address
 * @property {string} offset
 */
/**
 * @typedef {object} ModuleInfo
 * @property {string} name
 * @property {string} path
 * @property {string} base
 * @property {number} size
 * @property {SymbolEntry[]} symbols
 * @property {ExportEntry[]} exports
 * @property {number} [symbolsTruncated]
 * @property {string | null} error
 */
/**
 * @typedef {Record<string, unknown> & {
 *   _: string[],
 *   modules: string[],
 *   pid?: string,
 *   spawn?: boolean | string,
 *   kind?: string,
 *   pattern?: string,
 *   limit?: string,
 *   out?: string,
 *   "max-symbols"?: string,
 *   "arg-count"?: string,
 *   "duration-ms"?: string,
 * }} ParsedArgs
 */

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const functionsDir = join(root, "functions");
const logsDir = join(root, "logs");
const agentSource = await readFile(join(root, "scripts", "frida-log-calls.js"), "utf8");

/** @param {string} message */
const note = (message) => process.stderr.write(`${message}\n`);
/** @param {TargetProcess} process_ */
const isDiscord = (process_) =>
  /discord/i.test(process_.name) && !/vesktop/i.test(process_.name);

/** @param {string[]} argv @returns {ParsedArgs} */
const parseArgs = (argv) => {
  /** @type {ParsedArgs} */
  const parsed = { _: [], modules: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index] ?? "";
    if (!token.startsWith("--")) {
      parsed._.push(token);
      continue;
    }
    const [key = "", inline] = token.slice(2).split("=");
    /** @type {string | boolean | undefined} */
    let value = inline;
    const next = argv[index + 1];
    if (value === undefined && next !== undefined && !next.startsWith("--")) {
      value = next;
      index += 1;
    }
    if (value === undefined) value = true;
    if (key === "module") parsed.modules.push(String(value));
    else parsed[key] = value;
  }
  return parsed;
};

/** @param {Device} device @returns {Promise<TargetProcess[]>} */
const discordProcesses = async (device) =>
  (await device.enumerateProcesses()).filter(isDiscord);

/**
 * @param {Device} device
 * @param {number} pid
 * @returns {Promise<{ session: Session, script: Script, messages: FridaMessage[] }>}
 */
const attach = async (device, pid) => {
  const session = await device.attach(pid);
  const script = await session.createScript(agentSource, { name: "rea-discord-lab" });
  /** @type {FridaMessage[]} */
  const messages = [];
  script.message.connect((message) => messages.push(message));
  await script.load();
  return { session, script, messages };
};

/** @param {string} value @returns {string} */
const safeName = (value) => value.replaceAll(/[^A-Za-z0-9._-]/g, "_");

/** @param {(string | number)[][]} rows @returns {string} */
const csv = (rows) =>
  rows
    .map((row) =>
      row.map((value) => `"${String(value).replaceAll('"', '""')}"`).join(","),
    )
    .join("\n");

/**
 * @param {number} pid
 * @param {string} processName
 * @param {ModuleInfo[]} modules
 * @returns {Promise<{ target: string, csvPath: string, symbols: number, exports: number }>}
 */
const writeInventory = async (pid, processName, modules) => {
  await mkdir(functionsDir, { recursive: true });
  const name = `${safeName(processName)}`;
  const target = join(functionsDir, `pid-${pid}-${name}.json`);
  await writeFile(
    target,
    `${JSON.stringify(
      { pid, process: processName, at: new Date().toISOString(), modules },
      null,
      2,
    )}\n`,
  );

  /** @type {(string | number)[][]} */
  const rows = [["process", "pid", "module", "kind", "type", "name", "address", "offset"]];
  for (const module_ of modules) {
    for (const symbol of module_.symbols) {
      rows.push([name, pid, module_.name, "symbol", "function", symbol.name, symbol.address, symbol.offset]);
    }
    for (const entry of module_.exports) {
      rows.push([name, pid, module_.name, "export", entry.type, entry.name, entry.address, entry.offset]);
    }
  }
  const csvPath = join(functionsDir, `pid-${pid}-${name}.csv`);
  await writeFile(csvPath, `${csv(rows)}\n`);
  const counts = modules.reduce(
    (sum, m) => ({
      symbols: sum.symbols + m.symbols.length,
      exports: sum.exports + m.exports.length,
    }),
    { symbols: 0, exports: 0 },
  );
  return { target, csvPath, ...counts };
};

const commandPids = async () => {
  const device = await frida.getLocalDevice();
  const processes = await discordProcesses(device);
  if (processes.length === 0) {
    note("Процессы Discord не найдены. Запустите Discord.");
    return;
  }
  for (const process_ of processes) process.stdout.write(`${process_.pid}\t${process_.name}\n`);
};

/** @param {ParsedArgs} args */
const commandInventory = async (args) => {
  const device = await frida.getLocalDevice();
  const targets =
    args.pid === undefined
      ? await discordProcesses(device)
      : [{ pid: Number(args.pid), name: `pid-${args.pid}` }];
  if (targets.length === 0) {
    note("Процессы Discord не найдены. Запустите Discord.");
    return;
  }
  const options = {
    modules: args.modules,
    maxSymbols: args["max-symbols"] === undefined ? 50000 : Number(args["max-symbols"]),
  };
  let failed = 0;
  for (const process_ of targets) {
    note(`[inventory] pid=${process_.pid} ${process_.name}`);
    /** @type {Session | undefined} */
    let session;
    try {
      const attached = await attach(device, process_.pid);
      session = attached.session;
      const agentInventory = /** @type {AgentMethod} */ (attached.script.exports.inventory);
      /** @type {ModuleInfo[]} */
      const modules = await agentInventory(options);
      const summary = await writeInventory(process_.pid, process_.name, modules);
      note(
        `[inventory] модулей: ${modules.length}, символов: ${summary.symbols}, экспортов: ${summary.exports} -> ${summary.target}`,
      );
      await attached.script.unload().catch(() => {});
    } catch (error) {
      failed += 1;
      note(`[inventory] пропущено (${error instanceof Error ? error.message : String(error)})`);
    } finally {
      await session?.detach().catch(() => {});
    }
  }
  if (failed > 0) note(`[inventory] процессов пропущено: ${failed}`);
};

/** @param {ParsedArgs} args */
const commandHook = async (args) => {
  const device = await frida.getLocalDevice();
  /** @type {number} */
  let pid;
  if (args.spawn !== undefined) {
    note("[hook] запускаю Discord под Frida (spawn)…");
    pid = await device.spawn(["discord"]);
    await device.resume(pid);
    await new Promise((done) => setTimeout(done, 4000));
  } else if (args.pid !== undefined) {
    pid = Number(args.pid);
  } else {
    const processes = await discordProcesses(device);
    const firstProcess = processes[0];
    if (firstProcess === undefined) {
      note("Процессы Discord не найдены. Запустите Discord или используйте --spawn.");
      return;
    }
    pid = firstProcess.pid;
  }

  const out = String(args.out ?? join(logsDir, `calls-${pid}.jsonl`));
  await mkdir(dirname(out), { recursive: true });

  const { session, script } = await attach(device, pid);
  let calls = 0;
  script.message.connect((message) => {
    if (message.type !== frida.MessageType.Send) return;
    if (message.payload?.type !== "call") return;
    calls += 1;
    appendFile(out, `${JSON.stringify(message.payload)}\n`).catch(() => {});
  });

  const options = {
    modules: args.modules,
    kind: args.kind === "exports" ? "exports" : "symbols",
    pattern: args.pattern === undefined ? "." : String(args.pattern),
    limit: args.limit === undefined ? 64 : Number(args.limit),
    argCount: args["arg-count"] === undefined ? 2 : Number(args["arg-count"]),
  };
  const agentHook = /** @type {AgentMethod} */ (script.exports.hook);
  /** @type {{ hooked: number, targets: number, kind: string }} */
  const result = await agentHook(options);
  note(
    `[hook] pid=${pid} источник=${result.kind} целей=${result.targets} установлено=${result.hooked}`,
  );
  note(`[hook] лог: ${out}`);
  if (result.hooked === 0) {
    note("[hook] Под фильтр не попали функции: проверьте --module/--pattern/--kind.");
  }

  const duration = args["duration-ms"] === undefined ? 0 : Number(args["duration-ms"]);
  await new Promise((done) => {
    if (duration > 0) {
      setTimeout(done, duration);
      return;
    }
    process.once("SIGINT", done);
    process.once("SIGTERM", done);
    note("[hook] Ctrl-C — остановить логирование.");
  });

  const agentUnhook = /** @type {AgentMethod} */ (script.exports.unhook);
  await agentUnhook().catch(() => {});
  await script.unload().catch(() => {});
  await session.detach().catch(() => {});
  note(`[hook] завершено: записано вызовов: ${calls}`);
};

const main = async () => {
  const args = parseArgs(process.argv.slice(2));
  const command = args._[0] ?? "pids";
  if (command === "pids") await commandPids();
  else if (command === "inventory") await commandInventory(args);
  else if (command === "hook") await commandHook(args);
  else {
    note(`Неизвестная команда: ${command}`);
    process.exitCode = 1;
  }
};

await main();
