// @ts-check
/**
 * window-budget — кэш живых запусков GUI по ключу UI+бинаря.
 *
 * Ключ = sha256(sorted "path:md5" всех файлов rust/gui-spike/ui/** + "bin:mtime:size").
 * Пока ключ не менялся, повторный запуск окна запрещён: берём запись из журнала.
 *
 *   node waves/RW-39/PM/window-budget.mjs --key
 *   node waves/RW-39/PM/window-budget.mjs --check
 *   node waves/RW-39/PM/window-budget.mjs --record --reason "acceptance E5"
 *   node waves/RW-39/PM/window-budget.mjs --list
 */
import { createHash } from "node:crypto";
import { appendFileSync, existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const PM_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(PM_DIR, "../../..");
const UI_DIR = resolve(ROOT, "rust/gui-spike/ui");
const BIN = resolve(ROOT, "rust/gui-spike/target/debug/discord-gui-spike");
const LOG = resolve(PM_DIR, "window-runs.jsonl");

/** @param {string} dir @returns {string[]} */
function walk(dir) {
  return readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) => entry.isDirectory() ? walk(join(dir, entry.name)) : [join(dir, entry.name)])
    .sort();
}

/** @returns {string} */
function uiKey() {
  const hash = createHash("sha256");
  for (const file of walk(UI_DIR)) {
    const rel = relative(UI_DIR, file).split("\\").join("/");
    hash.update(`${rel}:${createHash("md5").update(readFileSync(file)).digest("hex")}\n`);
  }
  if (existsSync(BIN)) {
    const stat = statSync(BIN);
    hash.update(`bin:${Math.trunc(stat.mtimeMs)}:${stat.size}\n`);
  } else {
    hash.update("bin:missing\n");
  }
  return hash.digest("hex");
}

/** @typedef {{ts:string,key:string,reason:string}} Run */

/** @returns {Run[]} */
function readRuns() {
  if (!existsSync(LOG)) return [];
  return readFileSync(LOG, "utf8").split("\n").filter(Boolean).flatMap((line) => {
    try { return [/** @type {Run} */ (JSON.parse(line))]; } catch { return []; }
  });
}

const args = process.argv.slice(2);
const key = uiKey();
const runs = readRuns().filter((run) => run.key === key);

if (args.includes("--key")) {
  console.log(key);
} else if (args.includes("--list")) {
  for (const run of readRuns()) console.log(`${run.ts} ${run.key.slice(0, 12)} ${run.reason}`);
  console.log(`итого запусков: ${readRuns().length}`);
} else if (args.includes("--check")) {
  console.log(`key=${key.slice(0, 12)} runs=${runs.length}`);
  if (runs.length) console.log(`последний: ${runs.at(-1)?.ts} — ${runs.at(-1)?.reason}`);
  process.exit(runs.length ? 0 : 1);
} else if (args.includes("--record")) {
  const index = args.indexOf("--reason");
  const reason = index >= 0 ? args[index + 1] : null;
  if (!reason) {
    console.error("нужен --reason: почему открываем окно (приёмка/диагностика)");
    process.exit(1);
  }
  if (runs.length && !args.includes("--force")) {
    console.error(`окно по этому ключу уже запускалось (${runs.length}): ${runs.at(-1)?.reason}. ` +
      "Повтор запрещён WINDOW-BUDGET; для диагностики — --force --reason ...");
    process.exit(2);
  }
  appendFileSync(LOG, JSON.stringify({ ts: new Date().toISOString(), key, reason }) + "\n");
  console.log(`recorded key=${key.slice(0, 12)} reason="${reason}"`);
} else {
  console.error("используй --key | --check | --record --reason ... | --list");
  process.exit(1);
}
