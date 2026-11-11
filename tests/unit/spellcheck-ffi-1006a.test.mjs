// @ts-check
/**
 * Контрактные и модульные тесты FFI-биндинга `libenchant-2` / `hunspell`, интеграции с системным
 * словарём Linux (`/usr/share/hunspell/`), метода `check_word()` и генерации до 5 подсказок замены (TASK-1006A):
 * - SUB-1006A-0: Статический и компиляционный контракт `rust/crates/discord-platform/src/spellcheck.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, файл < 800 строк, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений и прогон встроенных Rust-тестов).
 * - SUB-1006A-1: Сверка с независимыми оракулами репозитория (`waves/kanban-board.json` карточка `TASK-1006A`,
 *   системный каталог `/usr/share/hunspell/` и эталонные словари `fixtures/hunspell/test.{aff,dic}`,
 *   `docs/re/backend-ipc.json` 6 IPC-каналов `DISCORD_SPELLCHECK_*`, ключи локализации `TKkotf`, `HJmG1G`, `xXqIX0`
 *   в `/home/sanya/re-deep/{en,ru}-catalog.json` и инициализация словаря для активной локали `ru_RU` / `en_US`).
 * - SUB-1006A-2: Метод `check_word(word) -> bool` с фильтрацией внешней пунктуации, URL (`https://...`),
 *   e-mail (`user@domain`), эмодзи (`<:name:id>`, `<a:name:id>`, `:shortcode:`), упоминаний `@user` / `<@123>`,
 *   каналов `<#456>`, inline-кода и поддержкой слов с дефисом (`well-known`, `по-русски`) и апострофом (`don't`).
 * - SUB-1006A-3: Метод `suggest(word) -> Vec<String>` с эвристикой Дамерау — Левенштейна, сохранением регистра
 *   первой буквы, ограничением до 5 лучших вариантов замены (`MAX_SUGGESTIONS = 5`), переключением локали
 *   (`switch_locale`) и контролем освобождения списков C-строк (`active_c_allocations() == 0`).
 * - SUB-1006A-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные Snowflake `0`/`u64::MAX`, `2000` символов, `10` файлов/`25 МБ`, `offset=9975`, громкость `0..200%`,
 *   бейдж `99+`, HTTP `400/401/403/404/413/429/5xx`, пустые, `skeleton`/`HTTP 202`, права `VIEW_CHANNEL` /
 *   `SEND_MESSAGES` / `AGE_GATED_SPACES`, fallback-восстановление при отсутствии системного словаря и редактирование секретов).
 * - SUB-1006A-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/crates/discord-platform/src/spellcheck.rs`
 *   (пропуск слов с дефисом, утечка памяти C-строк, сбой при смене локали, отключение фильтра `@mention`/URL,
 *   превышение лимита 5 подсказок).
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
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
const RUST_MODULE_REL_PATH = "rust/crates/discord-platform/src/spellcheck.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const BACKEND_IPC_PATH = join(ROOT, "docs", "re", "backend-ipc.json");
const FIXTURE_AFF_PATH = join(ROOT, "fixtures", "hunspell", "test.aff");
const FIXTURE_DIC_PATH = join(ROOT, "fixtures", "hunspell", "test.dic");
const SYSTEM_HUNSPELL_DIR = "/usr/share/hunspell";
const RE_EN_CATALOG_PATH = "/home/sanya/re-deep/en-catalog.json";
const RE_RU_CATALOG_PATH = "/home/sanya/re-deep/ru-catalog.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/crates/discord-platform/src/spellcheck.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1006A: Интеграция с системным словарём Linux (/usr/share/hunspell/), метод check_word() и генерация до 5 подсказок замены", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "spellcheck-ffi-1006a-"));
    testBinPath = join(workDir, "spellcheck_ffi_1006a_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1006A-0: проверяет статический контракт rust/crates/discord-platform/src/spellcheck.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024 и rustc --test -D warnings)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Отсутствует Rust-модуль ${RUST_MODULE_REL_PATH}`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле spellcheck.rs обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде spellcheck.rs запрещены .unwrap() и .expect()",
    );

    const sizeCheck = checkRustSourceSizeLimits(
      RUST_MODULE_REL_PATH,
      rustSource,
    );
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines >= 350 && sizeCheck.fileLines < 800,
      `Ожидался размер модуля 350..799 строк, получено: ${sizeCheck.fileLines}`,
    );

    const secretFindings = scanTextForSecrets(rustSource, RUST_MODULE_REL_PATH);
    assert.equal(
      secretFindings.length,
      0,
      `Обнаружены секреты в исходнике: ${JSON.stringify(secretFindings)}`,
    );

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", RUST_MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check завершился с ошибкой: ${fmtCheck.stderr || fmtCheck.stdout}`,
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
      `Компиляция не должна порождать предупреждений:\n${compiled.stderr}`,
    );

    const allTests = runCompiledRustTests(testBinPath);
    assert.equal(
      allTests.status,
      0,
      `Ошибка выполнения встроенных Rust-тестов:\n${allTests.stderr || allTests.stdout}`,
    );
    assert.match(allTests.stdout, /test result: ok\./);
  });

  it("SUB-1006A-1: сверяет карточку TASK-1006A в waves/kanban-board.json, дизайн-токены, геометрию Spellcheck FFI (C ABI), состояния word-check, оракул /usr/share/hunspell/ и инициализацию локалей ru_RU / en_US", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Независимый оракул: карточка TASK-1006A в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1006A = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1006A",
    );
    assert.ok(
      card1006A,
      "Карточка TASK-1006A должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1006A.department, "G1 Backend");
    assert.equal(card1006A.priority, "P2");
    assert.deepEqual(card1006A.layout, {
      container: "Spellcheck FFI",
      dimensions: "C ABI",
      padding: "0px",
      spacing: "0px",
    });
    assert.deepEqual(card1006A.design_tokens, {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#f23f43",
      border: "#3f4147",
    });
    assert.deepEqual(card1006A.interactive_states[0], {
      element: "word-check",
      default: "valid",
      hover: "checking",
      active: "misspelled",
    });

    // 2. Оракул: fixtures/hunspell/test.{aff,dic} и системный путь /usr/share/hunspell
    assert.ok(existsSync(FIXTURE_AFF_PATH), "Оракул test.aff должен существовать");
    assert.ok(existsSync(FIXTURE_DIC_PATH), "Оракул test.dic должен существовать");
    const affContent = readFileSync(FIXTURE_AFF_PATH, "utf8");
    const dicContent = readFileSync(FIXTURE_DIC_PATH, "utf8");
    assert.match(affContent, /^SET UTF-8/m);
    assert.match(affContent, /^WORDCHARS '-/m);
    assert.equal(dicContent.trim().split("\n")[0], "6");

    // 3. Оракул: docs/re/backend-ipc.json (6 IPC-каналов DISCORD_SPELLCHECK_*)
    const backendIpc = JSON.parse(readFileSync(BACKEND_IPC_PATH, "utf8"));
    const spellChannels = (backendIpc.channels ?? [])
      .map((/** @type {any} */ ch) => ch.name)
      .filter((/** @type {string} */ n) => n.startsWith("DISCORD_SPELLCHECK_"));
    assert.equal(spellChannels.length, 6);

    // 4. Оракул: /home/sanya/re-deep/{en,ru}-catalog.json
    if (existsSync(RE_EN_CATALOG_PATH) && existsSync(RE_RU_CATALOG_PATH)) {
      const enCat = JSON.parse(readFileSync(RE_EN_CATALOG_PATH, "utf8"));
      const ruCat = JSON.parse(readFileSync(RE_RU_CATALOG_PATH, "utf8"));
      assert.deepEqual(enCat.TKkotf, ["Spellcheck"]);
      assert.deepEqual(ruCat.TKkotf, ["Проверка правописания"]);
      assert.deepEqual(enCat.HJmG1G, ["Add to Dictionary"]);
      assert.deepEqual(ruCat.HJmG1G, ["Добавить в словарь"]);
      assert.deepEqual(enCat.xXqIX0, ["Remove from Dictionary"]);
      assert.deepEqual(ruCat.xXqIX0, ["Удалить из словаря"]);
    }

    // 5. Сверка констант и символов в исходнике spellcheck.rs
    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1006A.design_tokens)) {
      assert.ok(
        rustSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} карточки TASK-1006A должен присутствовать в spellcheck.rs`,
      );
    }
    assert.ok(rustSource.includes("Spellcheck FFI"));
    assert.ok(rustSource.includes("C ABI"));
    assert.ok(rustSource.includes("word-check"));
    assert.ok(rustSource.includes(SYSTEM_HUNSPELL_DIR));
    assert.ok(rustSource.includes("ru_RU"));
    assert.ok(rustSource.includes("en_US"));

    const run = runCompiledRustTests(testBinPath, "test_sub_1006a_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1006A-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1006a_1_.*ok/);
  });

  it("SUB-1006A-2: проверяет метод check_word(word) -> bool с фильтрацией пунктуации, URL, e-mail, эмодзи <:name:id>/:shortcode:, @меншнов и поддержкой дефисов/апострофов", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "SpellChecker",
      "SpellLocale",
      "WordCheckState",
      "check_word",
      "for_locale",
      "switch_locale",
      "resolve_system_dictionary_paths",
      "tokenize",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В spellcheck.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1006a_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1006A-2 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1006a_2_.*ok/);
  });

  it("SUB-1006A-3: проверяет метод suggest(word) -> Vec<String> с ограничением до 5 вариантов (MAX_SUGGESTIONS = 5), эвристикой Дамерау — Левенштейна, сменой локали и отсутствием утечек C-строк", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "MAX_SUGGESTIONS",
      "MAX_EDIT_DISTANCE",
      "damerau_levenshtein_distance",
      "suggest",
      "active_c_allocations",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В spellcheck.rs должен присутствовать символ ${symbol}`,
      );
    }
    assert.match(
      rustSource,
      /MAX_SUGGESTIONS:\s*usize\s*=\s*5\b/,
      "Константа MAX_SUGGESTIONS в spellcheck.rs должна быть равна 5",
    );

    const run = runCompiledRustTests(testBinPath, "test_sub_1006a_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1006A-3 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1006a_3_.*ok/);
  });

  it("SUB-1006A-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, HTTP 400..5xx, skeleton/202, права VIEW_CHANNEL/SEND_MESSAGES/AGE_GATED_SPACES, fallback-словарь и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "validate_spellcheck_policy_boundaries",
      "classify_dictionary_http_status",
      "evaluate_spellcheck_permissions",
      "redact_spellcheck_log",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В spellcheck.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1006a_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1006A-4 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1006a_4_.*ok/);
  });

  it("SUB-1006A-5: убивает 5/5 контрольных мутаций в rust/crates/discord-platform/src/spellcheck.rs (100% mutation kill rate)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      "Модуль spellcheck.rs должен существовать",
    );
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1006A-RS-01",
        moduleName: "spellcheck",
        operator: "HyphenWordAcceptBypass",
        from: "let allow_hyphen = current == '-' && next_is_letter;",
        to: "let allow_hyphen = false && current == '-' && next_is_letter;",
      },
      {
        id: "MUT-1006A-RS-02",
        moduleName: "spellcheck",
        operator: "CStringListFreeOmissionLeak",
        from: "self.c_allocations = self.c_allocations.saturating_sub(1);",
        to: "self.c_allocations = self.c_allocations.saturating_add(0);",
      },
      {
        id: "MUT-1006A-RS-03",
        moduleName: "spellcheck",
        operator: "LocaleSwitchReloadFailure",
        from: "self.locale = reloaded.locale;",
        to: "self.locale = SpellLocale::EnUs;",
      },
      {
        id: "MUT-1006A-RS-04",
        moduleName: "spellcheck",
        operator: "MentionAndUrlFilterBypass",
        from: "if is_ignored_special_token(trimmed) {",
        to: "if false && is_ignored_special_token(trimmed) {",
      },
      {
        id: "MUT-1006A-RS-05",
        moduleName: "spellcheck",
        operator: "MaxSuggestionsCapTamper",
        from: "pub const MAX_SUGGESTIONS: usize = 5;",
        to: "pub const MAX_SUGGESTIONS: usize = 12;",
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в spellcheck.rs: ${m.from}`,
      );
      return {
        id: m.id,
        moduleName: m.moduleName,
        operator: m.operator,
        mutatedSource: originalRust.replace(m.from, m.to),
      };
    });

    const gateResult = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutSrcPath = join(workDir, `mutant_${idx + 1}.rs`);
      const mutBinPath = join(workDir, `mutant_${idx + 1}_bin`);
      writeFileSync(mutSrcPath, mutant.mutatedSource, "utf8");
      const comp = compileRustModuleTests(mutSrcPath, mutBinPath);
      assert.equal(
        comp.status,
        0,
        `Мутант ${mutant.id} должен компилироваться без ошибок:\n${comp.stderr}`,
      );
      const run = runCompiledRustTests(mutBinPath);
      return {
        killed: run.status !== 0,
        reason: run.status !== 0 ? "test_failed" : "survived",
      };
    });

    const afterMd5 = createHash("md5")
      .update(readFileSync(RUST_MODULE_PATH, "utf8"))
      .digest("hex");
    assert.equal(
      afterMd5,
      originalMd5,
      "MD5 исходного файла spellcheck.rs не должен измениться после мутационного гейта",
    );
    assert.equal(
      gateResult.passed,
      true,
      `Выжившие мутанты: ${gateResult.survivors.join(", ")}`,
    );
    assert.equal(gateResult.killed, 5);
    assert.equal(gateResult.total, 5);
    assert.equal(gateResult.killRate, 1);
  });
});
