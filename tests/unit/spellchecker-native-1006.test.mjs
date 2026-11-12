// @ts-check
/**
 * Контрактные и модульные тесты нативной проверки орфографии в поле ввода чата (TASK-1006 / TASK-1006A / TASK-1006B):
 * - SUB-1006-0: Статический контракт нативного модуля `rust/discord-gpui/src/chat/spellchecker.rs`
 *   и валидатора `scripts/verify-spellchecker.mjs` (`#![forbid(unsafe_code)]`, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, модуль < 800 строк, 0 `.unwrap()`/`.expect()` в продуктовом коде,
 *   чистый `rustfmt --check --edition 2024`, компиляция `rustc --test --edition 2024 -D warnings`
 *   без предупреждений и `secret-scan.mjs`).
 * - SUB-1006-1: Сверка с независимыми оракулами репозитория (`waves/kanban-board.json` карточки
 *   `TASK-1006`, `TASK-1006A`, `TASK-1006B`, `docs/re/backend-ipc.json` 6 IPC-каналов `DISCORD_SPELLCHECK_*`
 *   в `core/#3132` ↔ `preload#9121`, `fixtures/hunspell/test.{aff,dic}`, `/home/sanya/re-deep/modules-index.json`
 *   `638128 SpellcheckStore` / `900582 discord_spellcheck`, и ключи локализации `TKkotf`, `HJmG1G`, `xXqIX0`
 *   в `/home/sanya/re-deep/{en,ru}-catalog.json`).
 * - SUB-1006-2: Валидация словарей `.aff`/`.dic` (`SET UTF-8`, заголовок количества слов, защита от NUL-байтов),
 *   токенизация текста с байтовыми смещениями и фильтрацией URL (`https://...`), e-mail (`@`),
 *   inline-кода (`` `...` ``), упоминаний (`<@123>`, `<#456>`) и эмодзи (`<:name:id>`, `<a:name:id>`, `:shortcode:`),
 *   и генерация до 5 вариантов исправления (`suggest`) по расстоянию Дамерау — Левенштейна.
 * - SUB-1006-3: Дебаунс проверки орфографии `150 мс` при наборе текста (коалесценция нажатий `< 150 мс`
 *   без блокировки ввода и выполнение на `>= 150 мс` в бюджете `<= 8.33 мс`), интерактивные состояния
 *   `misspelled-word` (`red-wavy-underline` `#f23f43` -> `pointer` -> `open suggestions`),
 *   `word-check` (`valid` -> `checking` -> `misspelled`), `wavy-underline` (`red-wave` -> `pointer` -> `context menu`),
 *   контекстное меню при клике ПКМ со списком до 5 подсказок, замена слова в 1 клик (`DISCORD_SPELLCHECK_REPLACE_MISSPELLING`)
 *   и управление пользовательским словарём (`Добавить в словарь` / `Удалить из словаря`).
 * - SUB-1006-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные `Snowflake 0 / u64::MAX`, `149 мс vs 150 мс`, `2000 символов`, `10 файлов / 25 МБ`,
 *   `offset=9975`, громкость `0..200%`, бейдж `99+`, коды `400/401/403/404/413/429/5xx`, пустые,
 *   `skeleton / HTTP 202`, права `PERM_SEND_MESSAGES` / `AGE_GATED_SPACES`, fallback-восстановление словаря,
 *   маскирование секретов и коды выхода CLI `scripts/verify-spellchecker.mjs` `0 / 1 / 2`).
 * - SUB-1006-5: Мутационный гейт `5/5` (`100%` kill rate) для Rust и JS модулей по контракту `scripts/quality-gate.mjs`.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  checkRustSourceSizeLimits,
  evaluateMutationGate,
} from "../../scripts/quality-gate.mjs";
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/discord-gpui/src/chat/spellchecker.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const SCRIPT_REL_PATH = "scripts/verify-spellchecker.mjs";
const SCRIPT_PATH = join(ROOT, SCRIPT_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const BACKEND_IPC_PATH = join(ROOT, "docs", "re", "backend-ipc.json");
const HUNSPELL_AFF_PATH = join(ROOT, "fixtures", "hunspell", "test.aff");
const HUNSPELL_DIC_PATH = join(ROOT, "fixtures", "hunspell", "test.dic");
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";
const RE_EN_CATALOG_PATH = "/home/sanya/re-deep/en-catalog.json";
const RE_RU_CATALOG_PATH = "/home/sanya/re-deep/ru-catalog.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `spellchecker.rs` через `rustc --test --edition 2024 -D warnings`.
 * @param {string} sourcePath
 * @param {string} outputBin
 * @returns {{ status: number | null, stdout: string, stderr: string }}
 */
function compileRustModuleTests(sourcePath, outputBin) {
  const res = spawnSync(
    "rustc",
    [
      "--test",
      "--edition",
      "2024",
      "-D",
      "warnings",
      sourcePath,
      "-o",
      outputBin,
    ],
    {
      cwd: ROOT,
      encoding: "utf8",
      timeout: 30_000,
    },
  );
  return {
    status: res.status,
    stdout: res.stdout ?? "",
    stderr: res.stderr ?? "",
  };
}

/**
 * Запускает скомпилированный тестовый бинарник с опциональным фильтром теста.
 * @param {string} binPath
 * @param {string} [filter]
 * @returns {{ status: number | null, stdout: string, stderr: string }}
 */
function runCompiledRustTests(binPath, filter) {
  const args = filter ? [filter, "--nocapture"] : ["--nocapture"];
  const res = spawnSync(binPath, args, {
    cwd: ROOT,
    encoding: "utf8",
    timeout: 15_000,
  });
  return {
    status: res.status,
    stdout: res.stdout ?? "",
    stderr: res.stderr ?? "",
  };
}

describe("TASK-1006: Нативная проверка орфографии в поле ввода (hunspell / libenchant), дебаунс 150 мс, волнистое подчёркивание #f23f43 и контекстное меню ПКМ", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "spellchecker-1006-"));
    testBinPath = join(workDir, "spellchecker_1006_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1006-0: модули rust/discord-gpui/src/chat/spellchecker.rs и scripts/verify-spellchecker.mjs существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), запрещают unsafe и компилируются без предупреждений", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Модуль ${RUST_MODULE_REL_PATH} должен существовать`,
    );
    assert.ok(
      existsSync(SCRIPT_PATH),
      `Скрипт ${SCRIPT_REL_PATH} должен существовать`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "Rust-модуль обязан содержать #![forbid(unsafe_code)]",
    );

    const prodCode = rustSource.split("#[cfg(test)]")[0] ?? rustSource;
    assert.doesNotMatch(
      prodCode,
      /\.unwrap\(\)|\.expect\(/,
      "В продуктовом коде spellchecker.rs запрещены .unwrap() и .expect()",
    );

    const rustSizeCheck = checkRustSourceSizeLimits(RUST_MODULE_REL_PATH, rustSource);
    assert.equal(
      rustSizeCheck.passed,
      true,
      `Нарушение лимитов размера Rust-кода: ${rustSizeCheck.reason}`,
    );
    assert.ok(
      rustSizeCheck.fileLines > 200 && rustSizeCheck.fileLines < 800,
      `Ожидался размер файла 200..800 строк, получено: ${rustSizeCheck.fileLines}`,
    );

    const jsSource = readFileSync(SCRIPT_PATH, "utf8");
    const jsLines = jsSource.split("\n").length;
    assert.ok(
      jsLines > 150 && jsLines < 800,
      `Ожидался размер JS-скрипта 150..800 строк, получено: ${jsLines}`,
    );

    const secretScanRust = scanTextForSecrets(rustSource, RUST_MODULE_REL_PATH);
    const secretScanJs = scanTextForSecrets(jsSource, SCRIPT_REL_PATH);
    assert.equal(secretScanRust.length, 0, "В Rust-модуле не должно быть захардкоженных секретов");
    assert.equal(secretScanJs.length, 0, "В JS-скрипте не должно быть захардкоженных секретов");

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", RUST_MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check не прошёл:\n${fmtCheck.stderr || fmtCheck.stdout}`,
    );

    const compiled = compileRustModuleTests(RUST_MODULE_PATH, testBinPath);
    assert.equal(
      compiled.status,
      0,
      `Ошибка компиляции rustc --test -D warnings:\n${compiled.stderr}`,
    );
    assert.equal(
      compiled.stderr.includes("warning:"),
      false,
      `Не должно быть предупреждений компилятора:\n${compiled.stderr}`,
    );

    const rustRun = runCompiledRustTests(testBinPath);
    assert.equal(
      rustRun.status,
      0,
      `Встроенные тесты spellchecker.rs должны проходить:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test result: ok\./);
  });

  it("SUB-1006-1: сверяет дизайн-токены, макет Text Input Context, интерактивные состояния misspelled-word, 6 IPC-каналов DISCORD_SPELLCHECK_* и словари hunspell с независимыми оракулами", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      SPELLCHECK_LAYOUT,
      SPELLCHECK_DESIGN_TOKENS,
      MISSPELLED_WORD_STATES,
      WORD_CHECK_STATES,
      WAVY_UNDERLINE_STATES,
      SPELLCHECK_DEBOUNCE_MS,
      MAX_SUGGESTIONS,
      SPELLCHECK_IPC_CHANNELS,
      SPELLCHECK_I18N_KEYS,
      resolveMisspelledWordState,
      resolveWordCheckState,
      resolveWavyUnderlineState,
      verifySpellcheckerOracleParity,
    } = await import("../../scripts/verify-spellchecker.mjs");

    // 1. Независимый оракул: карточки TASK-1006, TASK-1006A, TASK-1006B в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1006 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1006");
    const card1006a = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1006A");
    const card1006b = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1006B");
    assert.ok(card1006, "Карточка TASK-1006 должна присутствовать в waves/kanban-board.json");
    assert.ok(card1006a, "Карточка TASK-1006A должна присутствовать в waves/kanban-board.json");
    assert.ok(card1006b, "Карточка TASK-1006B должна присутствовать в waves/kanban-board.json");

    assert.deepEqual(SPELLCHECK_LAYOUT, card1006.layout);
    assert.deepEqual(SPELLCHECK_DESIGN_TOKENS, card1006.design_tokens);
    assert.deepEqual(MISSPELLED_WORD_STATES, card1006.interactive_states[0]);
    assert.deepEqual(WORD_CHECK_STATES, card1006a.interactive_states[0]);
    assert.deepEqual(WAVY_UNDERLINE_STATES, card1006b.interactive_states[0]);
    assert.equal(SPELLCHECK_DEBOUNCE_MS, 150);
    assert.equal(MAX_SUGGESTIONS, 5);

    // 2. Интерактивные состояния misspelled-word, word-check и wavy-underline
    assert.equal(resolveMisspelledWordState("default"), "red-wavy-underline");
    assert.equal(resolveMisspelledWordState("hover"), "pointer");
    assert.equal(resolveMisspelledWordState("active"), "open suggestions");

    assert.equal(resolveWordCheckState("default"), "valid");
    assert.equal(resolveWordCheckState("hover"), "checking");
    assert.equal(resolveWordCheckState("active"), "misspelled");

    assert.equal(resolveWavyUnderlineState("default"), "red-wave");
    assert.equal(resolveWavyUnderlineState("hover"), "pointer");
    assert.equal(resolveWavyUnderlineState("active"), "context menu");

    // 3. Независимый оракул docs/re/backend-ipc.json (6 каналов DISCORD_SPELLCHECK_*)
    const ipcRaw = JSON.parse(readFileSync(BACKEND_IPC_PATH, "utf8"));
    const ipcNames = new Set(
      (ipcRaw.channels ?? []).map((/** @type {any} */ ch) => ch.name),
    );
    assert.equal(SPELLCHECK_IPC_CHANNELS.length, 6);
    for (const channelName of SPELLCHECK_IPC_CHANNELS) {
      assert.ok(
        ipcNames.has(channelName),
        `IPC-канал ${channelName} должен присутствовать в docs/re/backend-ipc.json`,
      );
    }

    // 4. Независимый оракул /home/sanya/re-deep/{en,ru}-catalog.json и modules-index.json
    if (existsSync(RE_EN_CATALOG_PATH) && existsSync(RE_RU_CATALOG_PATH)) {
      const enCatalog = JSON.parse(readFileSync(RE_EN_CATALOG_PATH, "utf8"));
      const ruCatalog = JSON.parse(readFileSync(RE_RU_CATALOG_PATH, "utf8"));
      assert.deepEqual(enCatalog[SPELLCHECK_I18N_KEYS.SPELLCHECK], ["Spellcheck"]);
      assert.deepEqual(ruCatalog[SPELLCHECK_I18N_KEYS.SPELLCHECK], ["Проверка правописания"]);
      assert.deepEqual(enCatalog[SPELLCHECK_I18N_KEYS.ADD_TO_DICTIONARY], ["Add to Dictionary"]);
      assert.deepEqual(ruCatalog[SPELLCHECK_I18N_KEYS.ADD_TO_DICTIONARY], ["Добавить в словарь"]);
      assert.deepEqual(enCatalog[SPELLCHECK_I18N_KEYS.REMOVE_FROM_DICTIONARY], ["Remove from Dictionary"]);
      assert.deepEqual(ruCatalog[SPELLCHECK_I18N_KEYS.REMOVE_FROM_DICTIONARY], ["Удалить из словаря"]);
    }
    if (existsSync(RE_MODULES_INDEX_PATH)) {
      const modulesIdx = JSON.parse(readFileSync(RE_MODULES_INDEX_PATH, "utf8"));
      assert.ok(modulesIdx["638128"].names.includes("SpellcheckStore"));
      assert.ok(modulesIdx["900582"].strings.includes("2026-05-electron-multilang-spellcheck"));
    }

    // 5. Полная сверка всех оракулов через verifySpellcheckerOracleParity
    const parity = verifySpellcheckerOracleParity({
      kanbanBoardPath: KANBAN_BOARD_PATH,
      backendIpcPath: BACKEND_IPC_PATH,
      hunspellAffPath: HUNSPELL_AFF_PATH,
      hunspellDicPath: HUNSPELL_DIC_PATH,
    });
    assert.equal(parity.passed, true, `Несовпадение с оракулами: ${parity.reasons.join("; ")}`);
    assert.equal(parity.fixtureWordsCount, 6);
    assert.equal(parity.ipcChannelsVerified, 6);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1006_1_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1006-1 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1006_1_.*ok/);
  });

  it("SUB-1006-2: валидирует словари .aff/.dic, токенизирует текст с фильтрацией URL, e-mail, inline-кода, упоминаний и эмодзи, проверяет слова и генерирует до 5 подсказок замены", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      parseHunspellDictionaryFiles,
      tokenizeSpellcheckInput,
      checkWordSpelling,
      suggestWordCorrections,
    } = await import("../../scripts/verify-spellchecker.mjs");

    // 1. Загрузка и валидация реальных фикстур fixtures/hunspell/test.{aff,dic}
    const affText = readFileSync(HUNSPELL_AFF_PATH, "utf8");
    const dicText = readFileSync(HUNSPELL_DIC_PATH, "utf8");
    const parsedDict = parseHunspellDictionaryFiles(affText, dicText);
    assert.equal(parsedDict.valid, true);
    assert.equal(parsedDict.encoding, "UTF-8");
    assert.equal(parsedDict.declaredCount, 6);
    assert.deepEqual(parsedDict.words, [
      "hello",
      "world",
      "привет",
      "мир",
      "don't",
      "well-known",
    ]);

    // Негативная валидация битых словарей (.aff без SET, .dic с неверным заголовком и NUL-байтом)
    assert.equal(parseHunspellDictionaryFiles("FLAG UTF-8\n", dicText).valid, false);
    assert.equal(parseHunspellDictionaryFiles(affText, "99\nhello\n").valid, false);
    assert.equal(parseHunspellDictionaryFiles(affText, "1\nhel\0lo\n").valid, false);

    // 2. Токенизация с фильтрацией URL, e-mail, `code`, <@mentions>, <#channels>, <:emoji:id> и :shortcode:
    const inputSample =
      "Привет, wrold! Check https://discord.com/channels/1/2 mail@test.org `const x_y = 42;` <@123456789> <#987654321> <:CoP:3527262836281581568> :thumbsup: don't well-known";
    const tokens = tokenizeSpellcheckInput(inputSample);
    assert.deepEqual(
      tokens.map((/** @type {any} */ t) => t.text),
      ["Привет", "wrold", "Check", "don't", "well-known"],
    );
    for (const tok of tokens) {
      assert.equal(
        Buffer.from(inputSample, "utf8").subarray(tok.startByte, tok.endByte).toString("utf8"),
        tok.text,
      );
    }

    // 3. Проверка орфографии и генерация до 5 подсказок (Дамерау — Левенштейн)
    assert.equal(checkWordSpelling("hello", parsedDict.words), true);
    assert.equal(checkWordSpelling("Привет", parsedDict.words), true);
    assert.equal(checkWordSpelling("don't", parsedDict.words), true);
    assert.equal(checkWordSpelling("well-known", parsedDict.words), true);
    assert.equal(checkWordSpelling("wrold", parsedDict.words), false);

    const suggestionsEn = suggestWordCorrections("wrold", [
      ...parsedDict.words,
      "would",
      "word",
      "worlds",
      "whorl",
      "weld",
      "wild",
    ]);
    assert.ok(suggestionsEn.length >= 1 && suggestionsEn.length <= 5);
    assert.equal(suggestionsEn[0], "world");

    const suggestionsRu = suggestWordCorrections("привте", parsedDict.words);
    assert.equal(suggestionsRu[0], "привет");

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1006_2_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1006-2 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1006_2_.*ok/);
  });

  it("SUB-1006-3: реализует дебаунс 150 мс при наборе текста, состояния red-wavy-underline (#f23f43), контекстное меню ПКМ и замену слова в 1 клик", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      SpellcheckerInputSession,
      SPELLCHECK_DEBOUNCE_MS,
      SPELLCHECK_DESIGN_TOKENS,
    } = await import("../../scripts/verify-spellchecker.mjs");

    const session = new SpellcheckerInputSession({
      languages: ["en-US", "ru-RU"],
    });

    // 1. Ввод текста в t=1000 мс: до истечения 150 мс проверка находится в ожидании дебаунса и не тормозит ввод
    session.onTextInput("Привет wrold и helo!", 1_000);
    assert.equal(session.isDebouncePending(), true);
    assert.equal(session.flushAt(1_100).executed, false, "На 100 мс (< 150 мс) дебаунс ещё не должен срабатывать");

    // 2. Повторное нажатие клавиши в t=1120 мс сдвигает дедлайн на 1120 + 150 = 1270 мс
    session.onTextInput("Привет wrold и helo!", 1_120);
    assert.equal(session.flushAt(1_269).executed, false, "На 149 мс после последнего ввода дебаунс ещё не срабатывает");

    // 3. На t=1270 мс (ровно 150 мс после последнего нажатия) выполняется расчёт опечаток
    const flushed = session.flushAt(1_270);
    assert.equal(flushed.executed, true);
    assert.equal(flushed.debounceMs, SPELLCHECK_DEBOUNCE_MS);
    assert.ok(flushed.computeTimeMs <= 8.33);
    assert.equal(flushed.misspelledSpans.length, 2);

    const firstSpan = flushed.misspelledSpans[0];
    assert.ok(firstSpan);
    assert.equal(firstSpan.word, "wrold");
    assert.equal(firstSpan.underlineColor, SPELLCHECK_DESIGN_TOKENS.accent); // #f23f43
    assert.equal(firstSpan.interactiveState, "red-wavy-underline");
    assert.equal(firstSpan.wavyState, "red-wave");
    assert.equal(firstSpan.wordCheckState, "misspelled");
    assert.equal(firstSpan.suggestions[0], "world");
    assert.ok(firstSpan.suggestions.length <= 5);

    // 4. Наведение курсора переводит состояние в pointer
    const hovered = session.hoverSpan(0);
    assert.equal(hovered.interactiveState, "pointer");
    assert.equal(hovered.wavyState, "pointer");

    // 5. Клик ПКМ по подчёркнутому слову открывает контекстное меню с подсказками и действием "Добавить в словарь"
    const menu = session.openContextMenuOnRightClick(firstSpan.startByte + 1, { x: 240, y: 520 });
    assert.equal(menu.opened, true);
    assert.equal(menu.targetWord, "wrold");
    assert.equal(menu.interactiveState, "open suggestions");
    assert.equal(menu.wavyState, "context menu");
    assert.ok(menu.suggestions.includes("world"));
    assert.ok(menu.suggestions.length <= 5);
    assert.equal(menu.addToDictionaryLabelRu, "Добавить в словарь");
    assert.equal(menu.addToDictionaryKey, "HJmG1G");

    // 6. Замена опечатки в 1 клик по варианту из контекстного меню ("wrold" -> "world")
    const replaced = session.selectContextMenuSuggestion(0);
    assert.equal(replaced.replaced, true);
    assert.equal(replaced.text, "Привет world и helo!");
    assert.equal(replaced.ipcChannel, "DISCORD_SPELLCHECK_REPLACE_MISSPELLING");
    const remainingSpans = session.getMisspelledSpans();
    assert.equal(remainingSpans.length, 1);
    assert.ok(remainingSpans[0]);
    assert.equal(remainingSpans[0].word, "helo");

    // 7. Добавление оставшегося слова "helo" в пользовательский словарь через ПКМ
    const secondSpan = remainingSpans[0];
    session.openContextMenuOnRightClick(secondSpan.startByte, { x: 300, y: 520 });
    const learned = session.addTargetWordToDictionary();
    assert.equal(learned.added, true);
    assert.equal(learned.ipcChannel, "DISCORD_SPELLCHECK_SET_LEARNED_WORDS");
    assert.equal(session.getMisspelledSpans().length, 0);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1006_3_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1006-3 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1006_3_.*ok/);
  });

  it("SUB-1006-4: покрывает все 9 обязательных измерений TEST-POLICY.md, маскирует секреты и проверяет CLI scripts/verify-spellchecker.mjs (коды выхода 0 / 1 / 2)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      evaluateSpellcheckerPolicyDimensions,
      redactSpellcheckTelemetry,
    } = await import("../../scripts/verify-spellchecker.mjs");

    // 1. Проверка всех 9 обязательных измерений TEST-POLICY.md
    const policy = evaluateSpellcheckerPolicyDimensions();
    assert.equal(policy.allPassed, true, `Сбой измерений TEST-POLICY: ${policy.failures.join("; ")}`);
    assert.equal(policy.dimensionsChecked, 9);

    // 2. Маскирование секретов в телеметрии проверки орфографии
    const sampleLog =
      'Spellcheck telemetry auth="Bearer mfa.abCDef1234567890_secretTokenValue" token=dGVzdG1vY2t0b2tlbnZhbHVl.GhIjKl.MnOpQrStUvWxYz0123456789abcdef';
    const redacted = redactSpellcheckTelemetry(sampleLog);
    assert.doesNotMatch(redacted, /mfa\.abCDef1234567890_secretTokenValue/);
    assert.doesNotMatch(redacted, /GhIjKl\.MnOpQrStUvWxYz0123456789abcdef/);
    assert.match(redacted, /\[REDACTED\]/);

    // 3. Проверка CLI scripts/verify-spellchecker.mjs (коды выхода 0 / 1 / 2)
    const outJson = join(workDir, "verify-spellchecker-report.json");
    const cliOk = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--out-json", outJson],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliOk.status, 0, `CLI завершился с ошибкой: ${cliOk.stderr}`);
    assert.ok(existsSync(outJson));

    const cliBadArg = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--unknown-flag"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliBadArg.status, 1);

    const cliMissingOracle = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--oracle", "/nonexistent/missing-kanban-1006.json"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliMissingOracle.status, 2);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1006_4_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1006-4 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1006_4_.*ok/);
  });

  it("SUB-1006-5: проходит мутационный гейт 5/5 (100% kill rate) для Rust и JS реализаций по контракту scripts/quality-gate.mjs", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const rustMutationSpecs = [
      {
        id: "M1-RUST-DEBOUNCE-MS-CORRUPT",
        operator: "DebounceThresholdCorrupted",
        from: "pub const SPELLCHECK_DEBOUNCE_MS: u64 = 150;",
        to: "pub const SPELLCHECK_DEBOUNCE_MS: u64 = 10;",
      },
      {
        id: "M2-RUST-WAVY-STATE-CORRUPT",
        operator: "MisspelledDefaultStateBroken",
        from: 'Self::Default => "red-wavy-underline",',
        to: 'Self::Default => "none",',
      },
      {
        id: "M3-RUST-MAX-SUGGESTIONS-CORRUPT",
        operator: "MaxSuggestionsCapBroken",
        from: "pub const MAX_SUGGESTIONS: usize = 5;",
        to: "pub const MAX_SUGGESTIONS: usize = 0;",
      },
      {
        id: "M4-RUST-URL-SKIP-BYPASS",
        operator: "UrlFilterBypassed",
        from: 'if chunk.contains("://") || chunk.contains(\'@\') {',
        to: "if false {",
      },
      {
        id: "M5-RUST-REPLACE-MISSPELLING-NOOP",
        operator: "ContextMenuReplaceSkipped",
        from: "self.text.replace_range(span.start_byte..span.end_byte, &replacement);",
        to: "let _ = &replacement;",
      },
    ];

    const rustMutants = rustMutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна существовать в spellchecker.rs`,
      );
      return {
        id: spec.id,
        moduleName: "spellchecker.rs",
        operator: spec.operator,
        mutatedSource: originalRust.replace(spec.from, spec.to),
      };
    });

    const rustGate = evaluateMutationGate(rustMutants, (mutant, index) => {
      const mutantSrcPath = join(workDir, `mutant_${index + 1}.rs`);
      const mutantBinPath = join(workDir, `mutant_${index + 1}_bin`);
      writeFileSync(mutantSrcPath, mutant.mutatedSource, "utf8");
      const compiled = compileRustModuleTests(mutantSrcPath, mutantBinPath);
      if (compiled.status !== 0) {
        return { killed: true, reason: "compile_error" };
      }
      const run = runCompiledRustTests(mutantBinPath);
      return {
        killed: run.status !== 0,
        reason: run.status !== 0 ? "test_failed" : "survived",
      };
    });

    assert.equal(
      rustGate.passed,
      true,
      `Мутационный гейт Rust не пройден: ${rustGate.verdict} (выжили: ${rustGate.survivors.join(", ")})`,
    );
    assert.equal(rustGate.killed, 5);
    assert.equal(rustGate.total, 5);
    assert.equal(rustGate.killRate, 1);

    const originalJs = readFileSync(SCRIPT_PATH, "utf8");
    const jsMutationSpecs = [
      {
        id: "M1-JS-DEBOUNCE-MS-CORRUPT",
        operator: "SpellcheckDebounceCorrupted",
        from: "export const SPELLCHECK_DEBOUNCE_MS = 150;",
        to: "export const SPELLCHECK_DEBOUNCE_MS = 0;",
      },
      {
        id: "M2-JS-UNDERLINE-STATE-CORRUPT",
        operator: "MisspelledWordDefaultStateBroken",
        from: 'default: "red-wavy-underline",',
        to: 'default: "plain-text",',
      },
      {
        id: "M3-JS-URL-FILTER-BYPASS",
        operator: "UrlAndEmailFilterDisabled",
        from: 'if (rawChunk.includes("://") || rawChunk.includes("@")) {',
        to: "if (false) {",
      },
      {
        id: "M4-JS-MAX-SUGGESTIONS-CORRUPT",
        operator: "SuggestionsSliceDisabled",
        from: "export const MAX_SUGGESTIONS = 5;",
        to: "export const MAX_SUGGESTIONS = 0;",
      },
      {
        id: "M5-JS-CONTEXT-REPLACE-NOOP",
        operator: "ContextMenuWordReplacementBroken",
        from: "this.text = prefix + replacement + suffix;",
        to: "this.text = this.text;",
      },
    ];

    const jsMutants = jsMutationSpecs.map((spec) => {
      assert.ok(
        originalJs.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна существовать в verify-spellchecker.mjs`,
      );
      return {
        id: spec.id,
        moduleName: "verify-spellchecker.mjs",
        operator: spec.operator,
        mutatedSource: originalJs.replace(spec.from, spec.to),
      };
    });

    const jsGate = evaluateMutationGate(jsMutants, (mutant, index) => {
      const mutantJsPath = join(workDir, `mutant_spell_${index + 1}.mjs`);
      const harnessPath = join(workDir, `mutant_harness_${index + 1}.mjs`);
      writeFileSync(mutantJsPath, mutant.mutatedSource, "utf8");
      writeFileSync(
        harnessPath,
        [
          'import assert from "node:assert/strict";',
          `import { SPELLCHECK_DEBOUNCE_MS, MISSPELLED_WORD_STATES, MAX_SUGGESTIONS, tokenizeSpellcheckInput, suggestWordCorrections, SpellcheckerInputSession } from ${JSON.stringify(mutantJsPath)};`,
          "assert.equal(SPELLCHECK_DEBOUNCE_MS, 150);",
          'assert.equal(MISSPELLED_WORD_STATES.default, "red-wavy-underline");',
          "assert.equal(MAX_SUGGESTIONS, 5);",
          'const toks = tokenizeSpellcheckInput("hello https://discord.com user@host.org");',
          'assert.deepEqual(toks.map((t) => t.text), ["hello"]);',
          'const sugg = suggestWordCorrections("wrold", ["world", "hello"]);',
          'assert.equal(sugg[0], "world");',
          "const sess = new SpellcheckerInputSession();",
          'sess.onTextInput("helo world", 100);',
          "assert.equal(sess.flushAt(200).executed, false);",
          "const fl = sess.flushAt(250);",
          "assert.equal(fl.executed, true);",
          "sess.openContextMenuOnRightClick(0, { x: 10, y: 10 });",
          "const rep = sess.selectContextMenuSuggestion(0);",
          'assert.equal(rep.text, "hello world");',
        ].join("\n"),
        "utf8",
      );
      const res = spawnSync(process.execPath, [harnessPath], {
        cwd: ROOT,
        encoding: "utf8",
        timeout: 10_000,
      });
      return {
        killed: res.status !== 0,
        reason: res.status !== 0 ? "test_failed" : "survived",
      };
    });

    assert.equal(
      jsGate.passed,
      true,
      `Мутационный гейт JS не пройден: ${jsGate.verdict} (выжили: ${jsGate.survivors.join(", ")})`,
    );
    assert.equal(jsGate.killed, 5);
    assert.equal(jsGate.total, 5);
    assert.equal(jsGate.killRate, 1);
  });
});
