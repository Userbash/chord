// @ts-check
// Мета-тест инварианта PLAT-11: «IPC не принимает путь».
//
// Оракул — исходник Tauri-команд (`#[tauri::command]`): сигнатура функции, а не её
// реализация. Тест краснеет от одной строки `fn upload(path: String)` — то есть от
// кода, который ещё напишут (attachments/capture), а не от текущего поведения.
//
// Уровни проверки:
//  1) `scan()` разбирает сигнатуры после `#[tauri::command]`; комментарии и строковые
//     литералы не считаются (иначе доки/тесты про «path» давали бы ложный красный);
//  2) текущий `main.rs` — 0 нарушений, набор найденных команд == `generate_handler!`;
//  3) red-proof — синтетическая команда с `path: String`/`PathBuf` даёт violation;
//  4) в конфигурации нет `tauri-plugin-fs`/`tauri-plugin-dialog` и лишних capability.
//
// Без окна, без live, без сети: читаются только файлы репозитория.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const spikeDir = join(root, "rust", "gui-spike");

/** Имена аргументов, похожие на путь (PLAT-11 §2 п.1). */
const FORBIDDEN_NAME_SEGMENTS = new Set([
  "path",
  "file",
  "filepath",
  "filename",
  "dir",
  "dirname",
  "folder",
  "root",
  "location",
]);

/** Типы, означающие путь/дескриптор файла (PLAT-11 §2 п.1). */
const FORBIDDEN_TYPE = /\b(PathBuf|Path|File|OsString|OsStr)\b/;

/**
 * @typedef {object} IpcArg
 * @property {string} name
 * @property {string} type
 */

/**
 * @typedef {object} IpcCommand
 * @property {string} name
 * @property {IpcArg[]} args
 * @property {number} line 1-based строка `fn`
 */

/**
 * @typedef {object} Violation
 * @property {string} command
 * @property {string} arg
 * @property {string} type
 * @property {"name" | "type"} rule
 */

/**
 * @typedef {object} ScanResult
 * @property {IpcCommand[]} commands
 * @property {Violation[]} violations
 */

/**
 * Убирает комментарии и содержимое строковых литералов, сохраняя переводы строк
 * (номера строк остаются валидными). Одинарные кавычки не трогаем: это лайфтаймы.
 * @param {string} source
 * @returns {string}
 */
function stripCommentsAndStrings(source) {
  let out = "";
  let i = 0;
  while (i < source.length) {
    const c = source[i] ?? "";
    const n = source[i + 1] ?? "";
    if (c === "/" && n === "/") {
      while (i < source.length && source[i] !== "\n") {
        out += " ";
        i += 1;
      }
      continue;
    }
    if (c === "/" && n === "*") {
      let depth = 1;
      out += "  ";
      i += 2;
      while (i < source.length && depth > 0) {
        const a = source[i] ?? "";
        const b = source[i + 1] ?? "";
        if (a === "/" && b === "*") {
          depth += 1;
          out += "  ";
          i += 2;
          continue;
        }
        if (a === "*" && b === "/") {
          depth -= 1;
          out += "  ";
          i += 2;
          continue;
        }
        out += a === "\n" ? "\n" : " ";
        i += 1;
      }
      continue;
    }
    if (c === '"') {
      // Строковый литерал: кавычки сохраняем, содержимое гасим.
      out += '"';
      i += 1;
      while (i < source.length) {
        const a = source[i] ?? "";
        if (a === "\\") {
          out += "  ";
          i += 2;
          continue;
        }
        if (a === "\n") {
          out += "\n";
          i += 1;
          break;
        }
        out += " ";
        i += 1;
        if (a === '"') break;
      }
      continue;
    }
    out += c;
    i += 1;
  }
  return out;
}

/**
 * @param {string} source
 * @param {number} index
 * @returns {number} 1-based номер строки
 */
const lineAt = (source, index) => source.slice(0, index).split("\n").length;

/**
 * Делит список аргументов по запятым верхнего уровня (учитывает `<>`, `()`, `[]`).
 * @param {string} text
 * @returns {string[]}
 */
function splitTopLevel(text) {
  /** @type {string[]} */
  const parts = [];
  let depth = 0;
  let start = 0;
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i] ?? "";
    if (c === "(" || c === "[" || c === "{" || c === "<") depth += 1;
    else if (c === ")" || c === "]" || c === "}" || c === ">") depth = Math.max(0, depth - 1);
    else if (c === "," && depth === 0) {
      parts.push(text.slice(start, i));
      start = i + 1;
    }
  }
  parts.push(text.slice(start));
  return parts.map((part) => part.trim()).filter((part) => part.length > 0);
}

/**
 * Индекс первого одиночного `:` (не `::`) на верхнем уровне, либо -1.
 * @param {string} text
 * @returns {number}
 */
function findArgColon(text) {
  let depth = 0;
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i] ?? "";
    if (c === "(" || c === "[" || c === "{" || c === "<") depth += 1;
    else if (c === ")" || c === "]" || c === "}" || c === ">") depth = Math.max(0, depth - 1);
    else if (c === ":" && depth === 0 && text[i - 1] !== ":" && text[i + 1] !== ":") return i;
  }
  return -1;
}

/**
 * Находит парную закрывающую скобку для `open` (индекс `(`).
 * @param {string} text
 * @param {number} open
 * @returns {number} индекс `)` или -1
 */
function matchingParen(text, open) {
  let depth = 0;
  for (let i = open; i < text.length; i += 1) {
    const c = text[i] ?? "";
    if (c === "(") depth += 1;
    else if (c === ")") {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  return -1;
}

/**
 * Разбирает сигнатуры всех `#[tauri::command]` и возвращает нарушения инварианта.
 * @param {string} source исходник Rust-файла
 * @returns {ScanResult}
 */
export function scan(source) {
  const clean = stripCommentsAndStrings(source);
  const attribute = /#\[tauri::command\b[^\]]*\]/g;
  /** @type {IpcCommand[]} */
  const commands = [];
  /** @type {Violation[]} */
  const violations = [];

  for (const match of clean.matchAll(attribute)) {
    const from = (match.index ?? 0) + match[0].length;
    const rest = clean.slice(from);
    const fn = /\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>{}()]*>)?\s*\(/.exec(rest);
    if (!fn) continue;
    const name = fn[1] ?? "";
    const open = from + (fn.index ?? 0) + fn[0].length - 1;
    const close = matchingParen(clean, open);
    if (close < 0) continue;
    /** @type {IpcArg[]} */
    const args = [];
    for (const raw of splitTopLevel(clean.slice(open + 1, close))) {
      // Атрибуты аргумента (`#[serde(...)]`) не участвуют в имени/типе.
      const text = raw.replace(/#\[[^\]]*\]/g, "").trim();
      const colon = findArgColon(text);
      if (colon < 0) continue;
      const namePart = text
        .slice(0, colon)
        .trim()
        .replace(/^mut\s+/, "");
      const typePart = text.slice(colon + 1).trim();
      args.push({ name: namePart, type: typePart });
    }
    commands.push({ name, args, line: lineAt(source, open) });

    for (const arg of args) {
      const segments = arg.name.toLowerCase().split(/[_-]/);
      if (segments.some((segment) => FORBIDDEN_NAME_SEGMENTS.has(segment))) {
        violations.push({ command: name, arg: arg.name, type: arg.type, rule: "name" });
      }
      if (FORBIDDEN_TYPE.test(arg.type)) {
        violations.push({ command: name, arg: arg.name, type: arg.type, rule: "type" });
      }
    }
  }

  return { commands, violations };
}

/**
 * Рекурсивный список `.rs` файлов.
 * @param {string} dir
 * @returns {Promise<string[]>}
 */
async function listRustFiles(dir) {
  /** @type {string[]} */
  const out = [];
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...(await listRustFiles(full)));
    else if (entry.isFile() && entry.name.endsWith(".rs")) out.push(full);
  }
  return out;
}

/** @param {string} p @returns {string} */
const rel = (p) => relative(root, p).split(sep).join("/");

/**
 * Имена команд из `tauri::generate_handler![...]` (оракул регистрации).
 * @param {string} source
 * @returns {string[]}
 */
function registeredCommands(source) {
  const block = /generate_handler!\[([\s\S]*?)\]/.exec(source);
  assert.ok(block, "main.rs: не найден `generate_handler![...]`");
  return (block[1] ?? "")
    .split(",")
    .map((name) => name.trim())
    .filter((name) => name.length > 0);
}

/** @param {Violation[]} violations @returns {string} */
const formatViolations = (violations) =>
  violations
    .map((v) => `${v.command}(${v.arg}: ${v.type}) — правило ${v.rule}`)
    .join("; ");

const mainSource = await readFile(join(spikeDir, "src", "main.rs"), "utf8");

describe("ipc-path: синтетика (red-proof сканера)", () => {
  it("команда с `path: String` — нарушение (иначе инвариант не защищён)", () => {
    const { violations } = scan(`
#[tauri::command]
fn attachment_upload(path: String) -> Result<(), String> { Ok(()) }
`);
    assert.deepEqual(
      violations.map((v) => `${v.command}.${v.arg}:${v.rule}`),
      ["attachment_upload.path:name"],
      "новый arg `path` не поймал ни один тест — инвариант PLAT-11 не работает",
    );
  });

  it("тип пути ловится и при нейтральном имени (`source: PathBuf`, `&Path`, `File`)", () => {
    const { violations } = scan(`
#[tauri::command]
fn open_source(source: std::path::PathBuf) {}

#[tauri::command]
fn open_target(target: &Path) {}

#[tauri::command]
fn open_handle(handle: File) {}
`);
    assert.deepEqual(
      violations.map((v) => `${v.arg}:${v.rule}`).sort(),
      ["handle:type", "source:type", "target:type"],
    );
  });

  it("имена `root`/`dir`/`file_name` ловятся, `profile`/`cursor` — нет", () => {
    const { violations } = scan(`
#[tauri::command]
fn a(root: String) {}
#[tauri::command]
fn b(dir: Option<String>) {}
#[tauri::command]
fn c(file_name: String) {}
#[tauri::command]
fn d(profile: String, cursor: Option<String>, limit: Option<usize>) {}
`);
    assert.deepEqual(
      violations.map((v) => `${v.command}.${v.arg}`),
      ["a.root", "b.dir", "c.file_name"],
      "поиск по подстроке дал бы ложные срабатывания на profile/cursor",
    );
  });

  it("комментарии и строковые литералы не считаются сигнатурой", () => {
    const { commands, violations } = scan(`
//! Команды (#[tauri::command]) не принимают path: String.
// fn legacy(path: String) {}
/* #[tauri::command]
   fn commented(dir: String) {} */
#[tauri::command]
fn ok(channel_id: String) -> Result<(), String> {
    let note = "path: String, file: PathBuf";
    // file: PathBuf внутри тела — не аргумент
    let _ = note;
    Ok(())
}
`);
    assert.deepEqual(violations, [], "комментарий/строка дали ложное нарушение");
    assert.deepEqual(
      commands.map((c) => c.name),
      ["ok"],
    );
  });

  it("атрибут с параметрами и generic-тип в аргументах разбираются", () => {
    const { commands, violations } = scan(`
#[tauri::command(rename_all = "snake_case")]
fn pick(handle: Option<Vec<String>>, filter: tauri::State<'_, Arc<Backend>>) {}
`);
    assert.deepEqual(violations, []);
    assert.deepEqual(
      commands[0]?.args.map((a) => a.name),
      ["handle", "filter"],
    );
  });
});

describe("ipc-path: текущий код спайка", () => {
  it("ни одна IPC-команда не принимает путь", () => {
    const { commands, violations } = scan(mainSource);
    assert.ok(
      commands.length >= 29,
      `найдено ${commands.length} команд — парсер сломан (аудит PLAT-11: 29 на HEAD 0239543)`,
    );
    assert.deepEqual(violations, [], `путь в IPC-команде запрещён: ${formatViolations(violations)}`);
  });

  it("мутация реального main.rs ловится (добавили `path: String`)", () => {
    const marker = "fn process_metrics()";
    assert.ok(mainSource.includes(marker), "main.rs: не найдена опорная команда process_metrics");
    const mutated = mainSource.replace(
      marker,
      "fn probe_path(path: String) -> Result<(), String> { Ok(()) }\n\n" + marker,
    );
    const { violations } = scan(mutated);
    assert.deepEqual(
      violations.map((v) => `${v.command}.${v.arg}:${v.rule}`),
      ["probe_path.path:name"],
      "мутант `path: String` не пойман — тест бесполезен",
    );
  });

  it("найденные команды совпадают с зарегистрированными в generate_handler!", () => {
    const { commands } = scan(mainSource);
    const registered = registeredCommands(mainSource);
    const scanned = commands.map((c) => c.name);
    const unregistered = scanned.filter((name) => !registered.includes(name));
    const missing = registered.filter((name) => !scanned.includes(name));
    assert.deepEqual(unregistered, [], "команда объявлена, но не зарегистрирована в invoke_handler");
    assert.deepEqual(missing, [], "команда зарегистрирована, но её `#[tauri::command]` нет в исходнике");
  });

  it("остальные Rust-файлы спайка не объявляют path-команд", async () => {
    /** @type {string[]} */
    const problems = [];
    let total = 0;
    for (const file of await listRustFiles(join(spikeDir, "src"))) {
      const { commands, violations } = scan(await readFile(file, "utf8"));
      total += commands.length;
      for (const v of violations) {
        problems.push(`${rel(file)}: ${v.command}(${v.arg}: ${v.type}) — ${v.rule}`);
      }
    }
    assert.deepEqual(problems, [], "path-аргумент найден вне main.rs");
    assert.ok(total >= 29, `всего найдено ${total} команд — парсер не видит файлы`);
  });
});

describe("ipc-path: файловые плагины не подключены", () => {
  const forbidden = ["tauri-plugin-fs", "tauri-plugin-dialog"];

  it("Cargo.toml / Cargo.lock / tauri.conf.json не ссылаются на fs/dialog-плагины", async () => {
    /** @type {string[]} */
    const hits = [];
    for (const name of ["Cargo.toml", "Cargo.lock", "tauri.conf.json"]) {
      const text = await readFile(join(spikeDir, name), "utf8");
      for (const plugin of forbidden) {
        if (text.includes(plugin)) hits.push(`${name}: ${plugin}`);
      }
    }
    assert.deepEqual(hits, [], "файловый плагин открывает путь в webview — инвариант PLAT-11 нарушен");
  });

  it("исходники не вызывают tauri_plugin_fs / tauri_plugin_dialog", async () => {
    /** @type {string[]} */
    const hits = [];
    for (const file of await listRustFiles(join(spikeDir, "src"))) {
      const text = await readFile(file, "utf8");
      if (/tauri_plugin_(fs|dialog)\b/.test(text)) hits.push(rel(file));
    }
    assert.deepEqual(hits, [], "инициализация файлового плагина в коде спайка");
  });

  it("capability окна ограничена core:default", async () => {
    const config = JSON.parse(await readFile(join(spikeDir, "capabilities", "default.json"), "utf8"));
    assert.deepEqual(
      config.permissions,
      ["core:default"],
      "capability расширена: fs/dialog-разрешения вернут путь в webview",
    );
  });
});
