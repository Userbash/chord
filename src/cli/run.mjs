// @ts-check
// Точка входа CLI discord-lab: маршрутизация команд.
// Контракт: tests/smoke/cli.smoke.test.mjs (+ команды из TDD-PLAN.md §6).

import { appendFile, mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { parseArgs } from "../core/args.mjs";
import { callLine, parseCallLog, summarizeCallLines } from "../core/calls.mjs";
import { listTargets, resolveEndpoint } from "../core/cdp/client.mjs";
import { csv, safeName } from "../core/csv.mjs";
import { inventoryCounts, inventoryCsvRows, inventoryJson } from "../core/inventory.mjs";
import { filterDiscordProcesses } from "../core/processes.mjs";
import { functionsDir, logsDir } from "../core/paths.mjs";

const HELP = `discord-lab — инструменты анализа Discord

Использование: discord-lab <команда> [аргументы]

Команды:
  pids [--all]             список процессов Discord
  inventory                инвентарь модулей и символов
  hook                     запись вызовов функций
  cdp targets              список CDP-целей
  walk <app>               обход интерфейса
  summarize [лог]          сводка вызовов из JSONL
  domains                  отчёт по доменам функций
  static                   таблицы статических символов
  --help                   эта справка
  --version                версия discord-lab`;

/**
 * @returns {Promise<string>}
 */
async function packageVersion() {
  const packageJson = JSON.parse(await readFile(new URL("../../package.json", import.meta.url), "utf8"));
  return String(packageJson.version);
}

/** @param {string} message */
function usageError(message) {
  console.error(message);
  return 1;
}

/** @param {string} message */
function environmentError(message) {
  console.error(message);
  return 2;
}

/** @param {string} message */
function note(message) {
  console.error(message);
}

/**
 * Выполняет команду и возвращает код выхода процесса.
 *
 * M1 (волна W3e): help (по умолчанию), --version, summarize, cdp targets;
 * pids/inventory/hook/walk/domains/static — честные заглушки волн M2–M4.
 * Коды выхода: 0 — успех; 1 — неизвестная команда/ошибка аргументов;
 * 2 — недоступная внешняя среда (CDP/Frida).
 * @param {readonly string[]} argv
 * @returns {Promise<number>}
 */
export async function run(argv) {
  const parsed = parseArgs(argv);
  const [command, ...positionals] = parsed._;

  if (argv.length === 0 || parsed.help === true || argv[0] === "--help" || argv[0] === "-h") {
    console.log(HELP);
    return 0;
  }
  if (parsed.version === true || argv[0] === "--version") {
    console.log(await packageVersion());
    return 0;
  }
  if (!command) {
    console.log(HELP);
    return 0;
  }

  switch (command) {
    case "summarize": {
      let logPath = positionals[0];
      if (!logPath) {
        let candidates;
        try {
          candidates = (await readdir(process.cwd()))
            .filter((name) => /^calls-.*\.jsonl$/.test(name))
            .sort();
        } catch (error) {
          return environmentError(`Не удалось найти логи в ${process.cwd()}: ${errorMessage(error)}`);
        }
        logPath = candidates[0] ? join(process.cwd(), candidates[0]) : undefined;
        if (!logPath) return usageError("Логов не найдено: передайте путь к calls-*.jsonl или запустите команду в каталоге с логами.");
      }

      const absolutePath = resolve(logPath);
      try {
        const records = parseCallLog(await readFile(absolutePath, "utf8"));
        const summary = summarizeCallLines(records);
        const outputPath = `${absolutePath}.by-function.csv`;
        await writeFile(outputPath, `${summary.csv}\n`);
        console.log(`${basename(absolutePath)}: вызовов ${summary.total}, уникальных функций ${summary.unique} -> ${basename(outputPath)}`);
        return 0;
      } catch (error) {
        return environmentError(`Не удалось обработать лог ${absolutePath}: ${errorMessage(error)}`);
      }
    }
    case "cdp": {
      const subcommand = positionals[0];
      if (subcommand !== "targets") return usageError(`Неизвестная команда cdp: ${subcommand ?? "(не указана)"}`);
      const endpoint = resolveEndpoint();
      try {
        const targets = await listTargets(endpoint);
        console.log(JSON.stringify(targets, null, 2));
        return 0;
      } catch (error) {
        return environmentError(`Ошибка CDP endpoint ${endpoint}: ${errorMessage(error)}`);
      }
    }
    case "pids": {
      /** @type {typeof import("../adapters/frida.mjs")} */
      let frida;
      try {
        frida = await import("../adapters/frida.mjs");
      } catch (error) {
        return environmentError(`Frida недоступна: ${errorMessage(error)}`);
      }
      try {
        const device = await frida.loadDevice();
        const processes = await frida.listProcesses(device);
        const targets = parsed.all === true ? processes : filterDiscordProcesses(processes);
        if (targets.length === 0 && parsed.all !== true) {
          note("Процессы Discord не найдены. Запустите Discord.");
          return 0;
        }
        for (const process of targets) console.log(`${process.pid}\t${process.name}`);
        return 0;
      } catch (error) {
        return environmentError(`Frida недоступна: ${errorMessage(error)}`);
      }
    }
    case "inventory": {
      /** @type {typeof import("../adapters/frida.mjs")} */
      let frida;
      try {
        frida = await import("../adapters/frida.mjs");
      } catch (error) {
        return environmentError(`Frida недоступна: ${errorMessage(error)}`);
      }
      /** @type {import("frida").Device} */
      let device;
      /** @type {{ pid: number, name: string }[]} */
      let targets;
      try {
        device = await frida.loadDevice();
        targets = parsed.pid === undefined
          ? filterDiscordProcesses(await frida.listProcesses(device))
          : [{ pid: Number(parsed.pid), name: `pid-${parsed.pid}` }];
      } catch (error) {
        return environmentError(`Frida недоступна: ${errorMessage(error)}`);
      }
      if (targets.length === 0) {
        note("Процессы Discord не найдены. Запустите Discord.");
        return 0;
      }

      let failed = 0;
      for (const target of targets) {
        note(`[inventory] pid=${target.pid} ${target.name}`);
        /** @type {import("frida").Session | undefined} */
        let session;
        /** @type {import("frida").Script | undefined} */
        let script;
        try {
          const attached = await frida.attach(device, target.pid);
          session = attached.session;
          script = attached.script;
          const modules = /** @type {import("../core/types.js").ModuleInfo[]} */ (await frida.collectInventory(
            /** @type {import("../adapters/frida.mjs").AgentScript} */ (script), {
            modules: parsed.modules,
            maxSymbols: parsed["max-symbols"] === undefined ? 50000 : Number(parsed["max-symbols"]),
          }));
          const at = new Date().toISOString();
          const jsonPath = join(functionsDir(), `pid-${target.pid}-${safeName(target.name)}.json`);
          const csvPath = join(functionsDir(), `pid-${target.pid}-${safeName(target.name)}.csv`);
          await mkdir(functionsDir(), { recursive: true });
          await writeFile(jsonPath, `${JSON.stringify(inventoryJson({ pid: target.pid, processName: target.name, at, modules }), null, 2)}\n`);
          await writeFile(csvPath, `${csv(inventoryCsvRows(target.name, target.pid, modules))}\n`);
          const counts = inventoryCounts(modules);
          note(`[inventory] модулей: ${modules.length}, символов: ${counts.symbols}, экспортов: ${counts.exports} -> ${jsonPath}`);
        } catch (error) {
          failed += 1;
          note(`[inventory] пропущено (${errorMessage(error)})`);
        } finally {
          await script?.unload().catch(() => {});
          await session?.detach().catch(() => {});
        }
      }
      if (failed > 0) {
        note(`[inventory] процессов пропущено: ${failed}`);
        return 2;
      }
      return 0;
    }
    case "hook": {
      /** @type {typeof import("../adapters/frida.mjs")} */
      let frida;
      try {
        frida = await import("../adapters/frida.mjs");
      } catch (error) {
        return environmentError(`Frida недоступна: ${errorMessage(error)}`);
      }
      /** @type {import("frida").Device} */
      let device;
      /** @type {number} */
      let pid;
      try {
        device = await frida.loadDevice();
        if (parsed.spawn === true) {
          pid = await device.spawn(["discord"]);
          await device.resume(pid);
          await new Promise((done) => setTimeout(done, 4000));
        } else if (parsed.pid !== undefined) {
          pid = Number(parsed.pid);
        } else {
          const targets = filterDiscordProcesses(await frida.listProcesses(device));
          const target = targets[0];
          if (!target) {
            note("Процессы Discord не найдены. Запустите Discord или используйте --spawn.");
            return 0;
          }
          pid = target.pid;
        }
      } catch (error) {
        return environmentError(`Frida недоступна: ${errorMessage(error)}`);
      }

      const out = String(parsed.out ?? join(logsDir(), `calls-${pid}.jsonl`));
      await mkdir(dirname(out), { recursive: true });
      let attached;
      try {
        attached = await frida.attach(device, pid);
      } catch (error) {
        return environmentError(`Не удалось подключиться (pid ${pid}): ${errorMessage(error)}`);
      }
      let calls = 0;
      attached.script.message.connect((message) => {
        const record = frida.parseHookMessage(/** @type {import("../adapters/frida.mjs").HookMessage} */ (message));
        if (record === null) return;
        calls += 1;
        const line = callLine(record);
        if (line !== null) appendFile(out, line).catch(() => {});
      });
      try {
        const kind = parsed.kind === "exports" ? "exports" : "symbols";
        const result = /** @type {{ hooked: number, targets: number, kind: string }} */ (await frida.startHook(
          /** @type {import("../adapters/frida.mjs").AgentScript} */ (attached.script), {
          modules: parsed.modules,
          kind,
          pattern: parsed.pattern === undefined ? "." : String(parsed.pattern),
          limit: Number(parsed.limit ?? 64),
          argCount: Number(parsed["arg-count"] ?? 2),
        }));
        note(`[hook] pid=${pid} источник=${result.kind} целей=${result.targets} установлено=${result.hooked}`);
        note(`[hook] лог: ${out}`);
        if (result.hooked === 0) note("[hook] Под фильтр не попали функции: проверьте --module/--pattern/--kind.");

        const duration = Number(parsed["duration-ms"] ?? 0);
        await new Promise((done) => {
          if (duration > 0) {
            setTimeout(done, duration);
            return;
          }
          process.once("SIGINT", done);
          process.once("SIGTERM", done);
          note("[hook] Ctrl-C — остановить логирование.");
        });
        return 0;
      } catch (error) {
        return environmentError(`Ошибка hook (pid ${pid}): ${errorMessage(error)}`);
      } finally {
        await frida.stopHook(/** @type {import("../adapters/frida.mjs").AgentScript} */ (attached.script));
        await attached.session.detach().catch(() => {});
        note(`[hook] завершено: записано вызовов: ${calls}`);
      }
    }
    case "walk":
      return environmentError("Команда walk появится в M3.");
    case "domains":
      return environmentError("Команда domains появится в M4.");
    case "static":
      return environmentError("Команда static появится в M4.");
    default:
      return usageError(`Неизвестная команда: ${command}`);
  }
}

/** @param {unknown} error */
function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}
