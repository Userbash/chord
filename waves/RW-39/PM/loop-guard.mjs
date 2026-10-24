// @ts-check
/**
 * loop-guard — защита PM-цикла от повторения одной и той же попытки.
 *
 * Правило (waves/RW-39/PM/PLAYBOOK.md §0, LOOP-LEDGER.md):
 *   0–1 попытка  — можно;
 *   2-я          — можно с предупреждением;
 *   3-я          — только со сменой подхода (--new-approach --note "...");
 *   4-я          — только с решением владельца (--owner-approved --note "...");
 *   5-я и далее  — запрещено.
 *
 * Использование:
 *   node waves/RW-39/PM/loop-guard.mjs check --sig "RW-38|probe|node --test|AssertionError"
 *   node waves/RW-39/PM/loop-guard.mjs run   --sig "..." -- node --test tests/unit/foo.test.mjs
 *   node waves/RW-39/PM/loop-guard.mjs run   --sig "..." --new-approach --note "grid→list" -- make
 *
 * Сигнатура — стабильная строка «находка|зона|команда|первая строка ошибки».
 * Журнал append-only: waves/RW-39/PM/loop-state.jsonl (можно переопределить --state).
 */
import { appendFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const PM_DIR = dirname(fileURLToPath(import.meta.url));
const DEFAULT_STATE = resolve(PM_DIR, "loop-state.jsonl");

/** @typedef {{ts:string, sig:string, exit:number|null, errorLine:string|null, note:string|null}} Entry */

/** @param {string} sig @returns {string} */
function keyOf(sig) {
  return createHash("sha1").update(sig).digest("hex");
}

/** @param {string} file @returns {Entry[]} */
function readEntries(file) {
  if (!existsSync(file)) return [];
  return readFileSync(file, "utf8")
    .split("\n")
    .filter(Boolean)
    .flatMap((line) => {
      try { return [/** @type {Entry} */ (JSON.parse(line))]; } catch { return []; }
    });
}

/** @param {string[]} argv */
function parseArgs(argv) {
  /** @type {{mode:string, sig:string|null, state:string, note:string|null, approach:boolean, owner:boolean, error:string|null, cmd:string[]}} */
  const out = { mode: "check", sig: null, state: DEFAULT_STATE, note: null, approach: false, owner: false, error: null, cmd: [] };
  const rest = [...argv];
  out.mode = rest.shift() ?? "check";
  while (rest.length) {
    const arg = rest.shift();
    if (arg === "--") { out.cmd = rest; break; }
    if (arg === "--sig") out.sig = rest.shift() ?? null;
    else if (arg === "--state") out.state = resolve(rest.shift() ?? DEFAULT_STATE);
    else if (arg === "--note") out.note = rest.shift() ?? null;
    else if (arg === "--new-approach") out.approach = true;
    else if (arg === "--owner-approved") out.owner = true;
    else if (arg === "--error") out.error = rest.shift() ?? null;
    else throw new Error(`неизвестный аргумент: ${arg}`);
  }
  return out;
}

/**
 * @param {import("node:stream").Readable} stream
 * @param {NodeJS.WriteStream} out
 * @param {string[]} lines
 */
function tee(stream, out, lines) {
  stream.setEncoding("utf8");
  stream.on("data", (/** @type {string} */ chunk) => {
    out.write(chunk);
    for (const line of chunk.split("\n")) if (line.trim()) lines.push(line.trim());
  });
}

/** @param {string} command @param {string[]} args */
async function runAndTee(command, args) {
  return await new Promise((resolvePromise) => {
    const child = spawn(command, args, { stdio: ["inherit", "pipe", "pipe"] });
    /** @type {string[]} */
    const lines = [];
    tee(child.stdout, process.stdout, lines);
    tee(child.stderr, process.stderr, lines);
    child.on("error", (error) => resolvePromise({ code: 127, lines: [String(error)] }));
    child.on("close", (code) => resolvePromise({ code: code ?? 1, lines }));
  });
}

/** @param {string[]} lines @returns {string} */
function firstErrorLine(lines) {
  const bad = lines.find((line) => /(error|fail|panic|not ok|assert)/i.test(line));
  return bad ?? lines.at(-1) ?? "(нет вывода)";
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args.sig) throw new Error("нужен --sig: «находка|зона|команда|первая строка ошибки»");
  const key = keyOf(args.sig);
  const entries = readEntries(args.state).filter((entry) => keyOf(entry.sig) === key);
  const attempt = entries.length + 1;

  if (attempt >= 5) {
    console.error(`LOOP-BLOCKED: попытка #${attempt} по сигнатуре ${key.slice(0, 8)}. Лимит исчерпан — эскалация владельцу.`);
    process.exit(2);
  }
  if (attempt === 4 && !args.owner) {
    console.error(`LOOP-DETECTED: попытка #4. Нужно решение владельца: --owner-approved --note "..."`);
    process.exit(2);
  }
  if (attempt === 3 && !args.approach) {
    console.error(`LOOP-DETECTED: попытка #3 с той же сигнатурой. Смени подход: --new-approach --note "что меняем"`);
    process.exit(2);
  }
  if ((args.approach || args.owner) && !args.note) {
    console.error("для смены подхода/решения владельца нужен --note с описанием");
    process.exit(2);
  }
  if (args.mode === "check") {
    console.log(`attempt=${attempt} key=${key.slice(0, 8)} entries=${entries.length}`);
    if (attempt >= 3) console.log("требуется смена подхода");
    return;
  }
  if (attempt >= 2) console.log(`WARN: попытка #${attempt} по сигнатуре ${key.slice(0, 8)} — не повторяй ту же команду без новых данных`);

  const [command, ...rest] = args.cmd;
  if (!command) throw new Error("нет команды после --");
  const result = await runAndTee(command, rest);
  const errorLine = result.code === 0 ? null : firstErrorLine(result.lines);

  mkdirSync(dirname(args.state), { recursive: true });
  appendFileSync(args.state, JSON.stringify({
    ts: new Date().toISOString(),
    sig: args.sig,
    exit: result.code,
    errorLine: args.error ?? errorLine,
    note: args.note,
  }) + "\n");
  console.log(`loop-guard: attempt=${attempt} exit=${result.code}${errorLine ? ` error="${errorLine}"` : ""}`);
  process.exit(result.code);
}

main().catch((error) => {
  console.error(`loop-guard: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
});
