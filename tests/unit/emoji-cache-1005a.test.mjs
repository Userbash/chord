// @ts-check
/**
 * Контрактные и модульные тесты парсера статических/GIF эмодзи `<:name:id>` / `<a:name:id>`,
 * CDN-загрузчика и двухуровневого LRU-кэша (ОЗУ 256 МБ + диск `~/.cache/discord-gpui/emojis/` с ETag) (TASK-1005A):
 * - SUB-1005A-0: Статический и компиляционный контракт `rust/crates/discord-media/src/emoji_cache.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, файл < 800 строк, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений и прогон встроенных Rust-тестов).
 * - SUB-1005A-1: Сверка с независимыми оракулами репозитория (`waves/kanban-board.json` карточка `TASK-1005A`,
 *   `/home/sanya/re-deep/frontend-d03.json` и `docs/re/frontend-d03.json`, `docs/re/i18n-deep.json`,
 *   `docs/re/gateway-schema.json` и `/home/sanya/re-deep/cache-urls.json`).
 * - SUB-1005A-2: Регекс-парсер токенов `<:name:snowflake>` (статический) и `<a:name:snowflake>` (анимированный GIF),
 *   шорткодов `:name::skin-tone-3:`, Jumbo-режима (1..=30 эмодзи -> 48px, смешанный текст -> 22px) и
 *   генератор CDN URL `https://cdn.discordapp.com/emojis/{id}.{webp|gif}?size=48&quality=lossless`.
 * - SUB-1005A-3: Двухуровневый кэш `TwoTierEmojiCache` (L1 ОЗУ LRU 256 МБ + L2 диск `~/.cache/discord-gpui/emojis/`
 *   с валидацией `ETag` / `If-None-Match` / `304 Not Modified`, контрольной суммой FNV-1a64 и защитой от Path Traversal).
 * - SUB-1005A-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные Snowflake `0`/`u64::MAX`, `2000` символов, `10` файлов/`25 МБ`, `offset=9975`, громкость `0..200%`,
 *   бейдж `99+`, HTTP `400/401/403/404/413/429/5xx`, пустые, `skeleton`/`HTTP 202`, права `USE_EXTERNAL_EMOJIS`
 *   и `AGE_GATED_SPACES`, самовосстановление кэша при повреждении файла и маскирование секретов).
 * - SUB-1005A-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/crates/discord-media/src/emoji_cache.rs`.
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
const RUST_MODULE_REL_PATH = "rust/crates/discord-media/src/emoji_cache.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const DOCS_FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const RE_DEEP_FRONTEND_D03_PATH = "/home/sanya/re-deep/frontend-d03.json";
const DOCS_I18N_DEEP_PATH = join(ROOT, "docs", "re", "i18n-deep.json");
const DOCS_GATEWAY_SCHEMA_PATH = join(ROOT, "docs", "re", "gateway-schema.json");
const RE_DEEP_CACHE_URLS_PATH = "/home/sanya/re-deep/cache-urls.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/crates/discord-media/src/emoji_cache.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1005A: Парсер <:name:id> и двухуровневый кэш эмодзи (ОЗУ LRU 256 МБ + диск ~/.cache/discord-gpui/emojis/ с ETag)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "emoji-cache-1005a-"));
    testBinPath = join(workDir, "emoji_cache_1005a_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1005A-0: проверяет статический контракт rust/crates/discord-media/src/emoji_cache.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024 и rustc --test -D warnings)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Отсутствует Rust-модуль ${RUST_MODULE_REL_PATH}`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде emoji_cache.rs запрещены .unwrap() и .expect()",
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
      sizeCheck.fileLines >= 250 && sizeCheck.fileLines < 800,
      `Ожидался размер модуля 250..799 строк, получено: ${sizeCheck.fileLines}`,
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

  it("SUB-1005A-1: сверяет карточку TASK-1005A в waves/kanban-board.json, дизайн-токены, геометрию Media Cache (LRU 256MB), состояния cache-hit и RE-оракул frontend-d03.json", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Оракул: карточка TASK-1005A в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1005A = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1005A",
    );
    assert.ok(
      card1005A,
      "Карточка TASK-1005A должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1005A.department, "G1 Backend");
    assert.deepEqual(card1005A.layout, {
      container: "Media Cache",
      dimensions: "LRU 256MB",
      padding: "0px",
      spacing: "0px",
    });
    assert.deepEqual(card1005A.design_tokens, {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#5865f2",
      border: "#3f4147",
    });
    assert.deepEqual(card1005A.interactive_states[0], {
      element: "cache-hit",
      default: "cached",
      hover: "revalidate",
      active: "fetched",
    });

    // 2. Оракул: /home/sanya/re-deep/frontend-d03.json и docs/re/frontend-d03.json
    const frontendOraclePath = existsSync(RE_DEEP_FRONTEND_D03_PATH)
      ? RE_DEEP_FRONTEND_D03_PATH
      : DOCS_FRONTEND_D03_PATH;
    const d03 = JSON.parse(readFileSync(frontendOraclePath, "utf8"));
    const emojiScreen = d03.screens.find(
      (/** @type {any} */ s) => s.name === "emoji",
    );
    assert.ok(
      emojiScreen,
      "Экран emoji должен присутствовать в frontend-d03.json",
    );
    assert.equal(emojiScreen.counts.tabCount, 3);
    assert.equal(emojiScreen.counts.navCategories, 10);
    assert.equal(emojiScreen.counts.visibleGridChannel, 50);
    assert.equal(emojiScreen.counts.lockedChannel, 17);
    assert.equal(emojiScreen.counts.visibleGridDM, 42);
    assert.equal(emojiScreen.counts.reactionPickerItems, 60);
    assert.equal(emojiScreen.counts.reactionPickerLocked, 43);
    assert.match(emojiScreen.structure.grid, /emojiItem_fc7141/);
    assert.match(emojiScreen.structure.grid, /lockedEmoji_d982c2/);

    // 3. Оракул: docs/re/i18n-deep.json (<a:name:123> / <:name:123> и EMOJI_NAME_RE)
    const i18nRaw = readFileSync(DOCS_I18N_DEEP_PATH, "utf8");
    assert.ok(i18nRaw.includes("<a:name:123> / <:name:123>"));
    assert.ok(i18nRaw.includes("EMOJI_NAME_RE"));

    // 4. Оракул: docs/re/gateway-schema.json (363 эмодзи в READY.guilds[])
    const gwSchema = JSON.parse(readFileSync(DOCS_GATEWAY_SCHEMA_PATH, "utf8"));
    const emojisPerGuild =
      gwSchema?.stats?.ready?.emojisPerGuild ??
      gwSchema?.ready?.emojisPerGuild ??
      [];
    const totalReadyEmojis = emojisPerGuild.reduce(
      (/** @type {number} */ acc, /** @type {number | null} */ v) =>
        acc + (typeof v === "number" ? v : 0),
      0,
    );
    assert.equal(totalReadyEmojis, 363);

    // 5. Оракул: /home/sanya/re-deep/cache-urls.json (живые CDN URL эмодзи)
    if (existsSync(RE_DEEP_CACHE_URLS_PATH)) {
      const cacheUrlsRaw = readFileSync(RE_DEEP_CACHE_URLS_PATH, "utf8");
      assert.ok(
        cacheUrlsRaw.includes(
          "https://cdn.discordapp.com/emojis/1056141433670742056.webp?size=60",
        ),
      );
    }

    // 6. Сверка констант в исходнике emoji_cache.rs
    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1005A.design_tokens)) {
      assert.ok(
        rustSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} карточки TASK-1005A должен присутствовать в emoji_cache.rs`,
      );
    }
    assert.ok(rustSource.includes("Media Cache"));
    assert.ok(rustSource.includes("LRU 256MB"));
    assert.ok(rustSource.includes("cache-hit"));
    assert.ok(rustSource.includes("~/.cache/discord-gpui/emojis/"));

    const run = runCompiledRustTests(testBinPath, "test_sub_1005a_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1005A-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1005a_1_.*ok/);
  });

  it("SUB-1005A-2: проверяет парсер <:name:id> и <a:name:id>, шорткодов :name::skin-tone-N:, Jumbo-режима и генератор CDN URL https://cdn.discordapp.com/emojis/{id}.{webp|gif}?size=48&quality=lossless", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "ParsedEmojiToken",
      "ShortcodeToken",
      "JumboEmojiLayout",
      "validate_emoji_name",
      "validate_snowflake_id",
      "parse_emoji_tokens",
      "parse_shortcode_token",
      "compute_jumbo_layout",
      "build_emoji_cdn_url",
      "build_emoji_cdn_url_lossless",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В emoji_cache.rs должен присутствовать символ ${symbol}`,
      );
    }
    assert.ok(
      rustSource.includes("https://cdn.discordapp.com/emojis/"),
      "Базовый префикс CDN эмодзи должен присутствовать в emoji_cache.rs",
    );
    assert.ok(
      rustSource.includes("quality=lossless"),
      "Параметр quality=lossless должен присутствовать в emoji_cache.rs",
    );

    const run = runCompiledRustTests(testBinPath, "test_sub_1005a_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1005A-2 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1005a_2_.*ok/);
  });

  it("SUB-1005A-3: проверяет двухуровневый кэш TwoTierEmojiCache (L1 ОЗУ LRU 256 МБ + L2 диск ~/.cache/discord-gpui/emojis/ с ETag, If-None-Match, 304 Not Modified и защитой от Path Traversal)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "LRU_RAM_BUDGET_BYTES",
      "DEFAULT_EMOJI_CACHE_DIR",
      "CacheHitState",
      "EmojiCacheEntry",
      "CdnFetchOutcome",
      "TwoTierEmojiCache",
      "validate_cache_key",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В emoji_cache.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1005a_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1005A-3 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1005a_3_.*ok/);
  });

  it("SUB-1005A-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, HTTP 400..5xx, skeleton/202, права USE_EXTERNAL_EMOJIS/AGE_GATED_SPACES, самовосстановление при порче диска и редактирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "validate_emoji_policy_boundaries",
      "classify_cdn_http_status",
      "evaluate_emoji_permissions",
      "format_badge_count",
      "redact_emoji_cache_log",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В emoji_cache.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1005a_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1005A-4 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1005a_4_.*ok/);
  });

  it("SUB-1005A-5: убивает 5/5 контрольных мутаций в rust/crates/discord-media/src/emoji_cache.rs (100% mutation kill rate)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      "Модуль emoji_cache.rs должен существовать",
    );
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1005A-RS-01",
        moduleName: "emoji_cache",
        operator: "PathTraversalGuardBypass",
        from: "if !valid_key {",
        to: "if false && !valid_key {",
      },
      {
        id: "MUT-1005A-RS-02",
        moduleName: "emoji_cache",
        operator: "AnimatedPrefixTamper",
        from: "let animated = prefix == \"<a:\";",
        to: "let animated = false;",
      },
      {
        id: "MUT-1005A-RS-03",
        moduleName: "emoji_cache",
        operator: "SnowflakeZeroGuardBypass",
        from: "if id == 0 {",
        to: "if false && id == 0 {",
      },
      {
        id: "MUT-1005A-RS-04",
        moduleName: "emoji_cache",
        operator: "LruRamEvictionBypass",
        from: "while self.ram_used_bytes.saturating_add(incoming_bytes) > self.max_ram_bytes",
        to: "while false && self.ram_used_bytes.saturating_add(incoming_bytes) > self.max_ram_bytes",
      },
      {
        id: "MUT-1005A-RS-05",
        moduleName: "emoji_cache",
        operator: "EtagRevalidationStateTamper",
        from: "state: CacheHitState::Revalidate,",
        to: "state: CacheHitState::Fetched,",
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в emoji_cache.rs: ${m.from}`,
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
      if (comp.status !== 0) {
        return { killed: true, reason: "compile_error" };
      }
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
      "MD5 исходного файла emoji_cache.rs не должен измениться после мутационного гейта",
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
