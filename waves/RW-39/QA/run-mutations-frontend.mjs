// @ts-check
// RW-39 / QA-1: мутации frontend-инвариантов W39d.
// Каждая мутация применяется к КОПИИ `rust/gui-spike/ui` в /tmp/opencode/qa1-w39-mut
// (репозиторий read-only). Копия подключается через GUI_DOM_APP — штатный механизм
// харнесса. После прогона копия удаляется, md5 исходника сверяется с baseline.
//
//   node waves/RW-39/QA/run-mutations-frontend.mjs
//
// Ожидание для каждого мутанта: exit != 0 (тесты падают). Если exit == 0 — MISSED,
// это честно фиксируется в логе, а не «зеленится».
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../../", import.meta.url));
const uiDir = join(repoRoot, "rust/gui-spike/ui");
const tests = [
  "tests/unit/gui-spike-dom.test.mjs",
  "tests/unit/gui-spike-model.test.mjs",
  "tests/unit/gui-spike-bench.test.mjs",
  "tests/unit/gui-spike-features.test.mjs",
];

/** @param {string} path @returns {string} */
function md5(path) {
  return createHash("md5").update(readFileSync(path)).digest("hex");
}

/** @param {string} file @param {string} from @param {string} to @returns {(text: string) => string} */
function replaceOnce(file, from, to) {
  return (text) => {
    const parts = text.split(from);
    if (parts.length !== 2) {
      throw new Error(`${file}: ожидался ровно один фрагмент для замены, найдено ${parts.length - 1}`);
    }
    return `${parts[0]}${to}${parts[1]}`;
  };
}

/**
 * @typedef {{
 *   id: string,
 *   title: string,
 *   file: string,
 *   mutate: (text: string) => string,
 *   expectFeatureFail?: boolean,
 * }} Mutant
 */

/** @type {Mutant[]} */
const mutants = [
  {
    id: "M1",
    title: "высота продолжения 26→44",
    file: "model.js",
    mutate: replaceOnce("model.js", "continuation: 26,", "continuation: 44,"),
  },
  {
    id: "M2",
    title: "начало группы 48→44",
    file: "model.js",
    mutate: replaceOnce("model.js", "groupStart: 48,", "groupStart: 44,"),
  },
  {
    id: "M3",
    title: "высота участника 42→40",
    file: "model.js",
    mutate: replaceOnce("model.js", "member: 42,", "member: 40,"),
    expectFeatureFail: true,
  },
  {
    id: "M4",
    title: "#emoji-grid listbox→grid (в рантайме)",
    file: "features/emoji.js",
    mutate: replaceOnce(
      "features/emoji.js",
      "  function render() {\n",
      '  function render() {\n    grid.setAttribute("role", "grid");\n',
    ),
    expectFeatureFail: true,
  },
  {
    id: "M5a",
    title: "ячейка option→gridcell",
    file: "features/emoji.js",
    mutate: replaceOnce(
      "features/emoji.js",
      'cell.setAttribute("role", "option");',
      'cell.setAttribute("role", "gridcell");',
    ),
    expectFeatureFail: true,
  },
  {
    id: "M5b",
    title: "у ячейки убран aria-label",
    file: "features/emoji.js",
    mutate: replaceOnce(
      "features/emoji.js",
      '      cell.setAttribute("aria-label", item.name);\n',
      "",
    ),
    expectFeatureFail: true,
  },
  // M6: лимит окна 500 в продукте не литерал, а pageSize*maxPages = 100*5.
  // «501 строка» при страничной гранулярности недостижима (страница ровно 100),
  // поэтому проверяем два ближайших регресса: подъём числа страниц и размера
  // страницы (бюджет 505 > 500).
  {
    id: "M6a",
    title: "лимит окна: maxPages 5→6 (бюджет 500→600)",
    file: "features/chat.js",
    mutate: replaceOnce("features/chat.js", "CHAT_MAX_PAGES = 5;", "CHAT_MAX_PAGES = 6;"),
  },
  {
    id: "M6b",
    title: "лимит окна: pageSize 100→101 (бюджет 500→505)",
    file: "features/chat.js",
    mutate: replaceOnce("features/chat.js", "CHAT_PAGE_SIZE = 100;", "CHAT_PAGE_SIZE = 101;"),
  },
  {
    id: "M7",
    title: "убран body[data-active-screen]",
    file: "features/screens.js",
    mutate: replaceOnce(
      "features/screens.js",
      '    document.body.setAttribute("data-active-screen", name);\n',
      "",
    ),
    expectFeatureFail: true,
  },
  {
    id: "M8",
    title: "поиск эмодзи no-op (фильтр игнорируется)",
    file: "features/emoji.js",
    mutate: replaceOnce(
      "features/emoji.js",
      `    visible = query === "" || category.name.toLowerCase().includes(query)
      ? [...category.items]
      : category.items.filter(
          (item) => item.glyph.includes(query) || item.name.toLowerCase().includes(query),
        );`,
      "    visible = [...category.items];",
    ),
  },
  {
    id: "M9",
    title: "число тумблеров 38→37 (убран «Превью»)",
    file: "features/settings.js",
    mutate: replaceOnce(
      "features/settings.js",
      '"Разрешение", "FPS", "Зеркало", "Превью",',
      '"Разрешение", "FPS", "Зеркало",',
    ),
    expectFeatureFail: true,
  },
];

const logLines = [];
/** @param {string} line */
function log(line) {
  logLines.push(line);
  console.log(line);
}

/** @param {string} output @returns {string[]} */
function failedTestNames(output) {
  return output
    .split("\n")
    .filter((line) => line.includes("✖"))
    .map((line) => line.trim())
    .filter((line) => !line.startsWith("✖ failing tests"));
}

const sourceMd5 = new Map(mutants.map((mutant) => [mutant.file, md5(join(uiDir, mutant.file))]));
log("# RW-39 QA-1: мутации frontend-инвариантов");
log(`# дата: ${new Date().toISOString()}`);
log(`# команда на мутанта: GUI_DOM_APP=<копия>/app.js node --test ${tests.join(" ")}`);
log("#");
for (const [file, hash] of sourceMd5) log(`baseline md5 ${hash}  ${file}`);
log("");

const sandboxRoot = mkdtempSync(join(tmpdir(), "qa1-w39-mut-"));
/** @type {Array<{ id: string, title: string, caught: boolean, exit: number, mutantsMd5: string, restoredMd5: string, failed: string[] }>} */
const results = [];

try {
  for (const mutant of mutants) {
    const workDir = join(sandboxRoot, mutant.id);
    cpSync(uiDir, workDir, { recursive: true });
    const target = join(workDir, mutant.file);
    const original = sourceMd5.get(mutant.file);
    writeFileSync(target, mutant.mutate(readFileSync(target, "utf8")));
    const mutatedHash = md5(target);

    const run = spawnSync(
      process.execPath,
      ["--test", ...tests],
      {
        cwd: repoRoot,
        env: { ...process.env, GUI_DOM_APP: join(workDir, "app.js") },
        encoding: "utf8",
        timeout: 300_000,
      },
    );
    const output = `${run.stdout ?? ""}\n${run.stderr ?? ""}`;
    const exit = run.status ?? -1;
    const caught = exit !== 0;
    const failed = failedTestNames(output);

    // restore: удаляем песочницу и сверяем md5 исходника (репозиторий не менялся).
    rmSync(workDir, { recursive: true, force: true });
    const restoredHash = md5(join(uiDir, mutant.file));
    if (restoredHash !== original) {
      throw new Error(`${mutant.id}: исходник изменился! ${original} -> ${restoredHash}`);
    }

    log(`## ${mutant.id}: ${mutant.title}`);
    log(`файл: ${mutant.file}`);
    log(`md5 до:      ${original}`);
    log(`md5 мутанта: ${mutatedHash}`);
    log(`exit: ${exit} (${caught ? "CAUGHT" : "MISSED"})`);
    log(`упавших проверок: ${failed.length}`);
    for (const name of failed.slice(0, 12)) log(`  ${name}`);
    if (failed.length > 12) log(`  … ещё ${failed.length - 12}`);
    log(`md5 после restore: ${restoredHash} ${restoredHash === original ? "== до" : "!! РАСХОЖДЕНИЕ"}`);
    log("");

    results.push({
      id: mutant.id,
      title: mutant.title,
      caught,
      exit,
      mutantsMd5: mutatedHash,
      restoredMd5: restoredHash,
      failed,
    });
  }
} finally {
  rmSync(sandboxRoot, { recursive: true, force: true });
}

const caught = results.filter((result) => result.caught).length;
log(`## Итог: caught ${caught}/${results.length}, missed ${results.length - caught}`);
for (const result of results) {
  log(`- ${result.id} ${result.caught ? "CAUGHT" : "MISSED"} (exit ${result.exit}) — ${result.title}`);
}

const outDir = fileURLToPath(new URL(".", import.meta.url));
writeFileSync(join(outDir, "mutations-frontend.log"), `${logLines.join("\n")}\n`);
writeFileSync(
  join(outDir, "mutations-frontend.json"),
  `${JSON.stringify({ generatedAt: new Date().toISOString(), tests, caught, total: results.length, results }, null, 2)}\n`,
);

if (caught !== results.length) process.exitCode = 1;
