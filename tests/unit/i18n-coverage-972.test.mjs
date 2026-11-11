// @ts-check
/**
 * Контрактные и модульные тесты стенда верификации локализации `discord-i18n`
 * под 36 языков, хешей `xxh64`, RTL-текста и plural-форм (TASK-972):
 * - SUB-972-1: Сверка реестра 36 языковых локалей (`31` включённых + `5` отключённых),
 *   канонического разрешения локалей, цепочки отката (`fallbackChain`) и 24 ключей `A11yI18nKey`
 *   против `rust/crates/discord-i18n/src/locale.rs`, `docs/re/i18n-deep.json` и `cldr-oracle.json`.
 * - SUB-972-2: Побитовая верификация `xxh64(input, seed)` и `messageKey(id)` (`xxh64 -> LE -> base64[0..6]`)
 *   по 14 эталонным векторам RE-оракула `docs/re/i18n-deep.json`, граничным буферам (`0`, `31`, `32`, `64` байт)
 *   и реальным каталогам `/home/sanya/re-deep/ru-catalog.json` (23 836 ключей) и `en-catalog.json` (23 903 ключа).
 * - SUB-972-3: Валидация RTL-локалей (`ar`, `he`), выравнивания (`direction: "rtl"`, `textAlign: "right"`),
 *   обнаружения арабского/ивритского письма и BiDi-управляющих символов, нейтрализации `U+202E` (RLO)
 *   и изоляции через `U+2068` (FSI) ... `U+2069` (PDI) на границе 2000 символов.
 * - SUB-972-4: Сверка CLDR `cardinal` и `ordinal` plural-форм для всех 36 локалей против независимого
 *   ICU-оракула `rust/crates/discord-i18n/tests/data/cldr-oracle.json`, проверка больших целых (`u64::MAX`),
 *   парсинг и рендеринг keyless JSON-AST (`Literal`, `Argument`, `Number`, `Date`, `Time`, `Select`,
 *   `Plural`, `Pound`, `Tag`) и проверка обязательной ветви `other` в реальных каталогах.
 * - SUB-972-5: CLI-контракт `scripts/verify-i18n.mjs` (коды выхода `0 / 1 / 2`, `--out-json`, `--locale`),
 *   статические лимиты размера кода и мутационный гейт `5/5` (`evaluateMutationGate`).
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  checkRustSourceSizeLimits,
  evaluateMutationGate,
} from "../../scripts/quality-gate.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SCRIPT_PATH = join(ROOT, "scripts", "verify-i18n.mjs");
const CLDR_ORACLE_PATH = join(ROOT, "rust", "crates", "discord-i18n", "tests", "data", "cldr-oracle.json");
const I18N_DEEP_PATH = join(ROOT, "docs", "re", "i18n-deep.json");
const RUST_LOCALE_PATH = join(ROOT, "rust", "crates", "discord-i18n", "src", "locale.rs");

describe("TASK-972: Стенд верификации локализации i18n под 36 языков, xxh64, RTL и plural-форм", () => {
  it("SUB-972-1: сверяет реестр 36 локалей (31 enabled + 5 disabled), цепочки fallback и A11y-ключи с оракулами", async () => {
    const {
      TOTAL_LOCALES,
      ENABLED_LOCALES_COUNT,
      DEFAULT_LOCALE,
      SUPPORTED_LOCALES,
      DISABLED_LOCALES,
      isSupportedLocale,
      isEnabledLocale,
      canonicalLocale,
      localeLanguage,
      resolveLocale,
      fallbackChain,
      verifyLocalesRegistry,
      verifyA11yCatalogKeys,
    } = await import("../../scripts/verify-i18n.mjs");

    // 1. Константы реестра 36 локалей
    assert.equal(TOTAL_LOCALES, 36);
    assert.equal(ENABLED_LOCALES_COUNT, 31);
    assert.equal(DEFAULT_LOCALE, "en-US");
    assert.equal(SUPPORTED_LOCALES.length, 36);
    assert.deepEqual(DISABLED_LOCALES, ["pt-PT", "sk", "he", "ar", "id"]);
    assert.equal(ENABLED_LOCALES_COUNT + DISABLED_LOCALES.length, TOTAL_LOCALES);

    // 2. Позитивные и регистронезависимые проверки всех 36 кодов
    for (const code of SUPPORTED_LOCALES) {
      assert.equal(isSupportedLocale(code), true, `Локаль ${code} должна поддерживаться`);
      assert.equal(canonicalLocale(code.toLowerCase()), code);
      assert.equal(canonicalLocale(code.toUpperCase()), code);
      assert.equal(isEnabledLocale(code), !DISABLED_LOCALES.includes(code));
    }

    // 3. Негативные и граничные сценарии разрешения локалей
    assert.equal(isSupportedLocale("xx-YY"), false);
    assert.equal(isSupportedLocale(""), false);
    assert.equal(canonicalLocale("unknown"), null);
    assert.equal(localeLanguage("pt-BR"), "pt");
    assert.equal(localeLanguage("es-419"), "es");
    assert.equal(localeLanguage("ru"), "ru");

    const available = ["en-US", "en-GB", "ru", "fr", "pt-PT", "sk"];
    assert.equal(resolveLocale("ru", available), "ru");
    assert.equal(resolveLocale("RU-ru", available), "ru");
    assert.equal(resolveLocale("FR", available), "fr");
    assert.equal(resolveLocale("fr-CA", available), "fr");
    assert.equal(resolveLocale("en-AU", available), "en-US");
    // Отключённые локали (pt-PT, sk, he, ar, id) обязаны откатываться к DEFAULT_LOCALE (en-US)
    assert.equal(resolveLocale("pt-PT", available), "en-US");
    assert.equal(resolveLocale("sk", available), "en-US");
    assert.equal(resolveLocale("ar", available), "en-US");
    assert.equal(resolveLocale("he", available), "en-US");
    assert.equal(resolveLocale("", available), "en-US");
    assert.equal(resolveLocale("ru", ["fr"]), "en-US");

    assert.deepEqual(fallbackChain("ru", available), ["ru", "en-US"]);
    assert.deepEqual(fallbackChain("en-US", available), ["en-US"]);
    assert.deepEqual(fallbackChain("pt-PT", available), ["en-US"]);

    // 4. Полная сверка с rust/crates/discord-i18n/src/locale.rs, docs/re/i18n-deep.json и cldr-oracle.json
    const regReport = verifyLocalesRegistry({
      rustLocalePath: RUST_LOCALE_PATH,
      i18nDeepPath: I18N_DEEP_PATH,
      cldrOraclePath: CLDR_ORACLE_PATH,
    });
    assert.equal(regReport.passed, true, `Ошибки реестра локалей: ${regReport.reasons.join("; ")}`);
    assert.equal(regReport.totalLocales, 36);
    assert.equal(regReport.enabledCount, 31);
    assert.equal(regReport.disabledCount, 5);
    assert.equal(regReport.enabledHashesVerifiedInRuCatalog, 31);

    // 5. Сверка 24 ключей доступности A11yI18nKey из rust/discord-gpui/src/a11y/mod.rs
    const a11yReport = verifyA11yCatalogKeys();
    assert.equal(a11yReport.passed, true, `Ошибки A11y ключей: ${a11yReport.reasons.join("; ")}`);
    assert.equal(a11yReport.keyCount, 24);
  });

  it("SUB-972-2: валидирует xxh64 и 6-символьные base64 хеши ключей по оракулу i18n-deep.json и реальным каталогам", async () => {
    const {
      MESSAGE_KEY_LEN,
      xxh64,
      messageKey,
      isValidCatalogKeyHash,
      verifyXxh64Oracle,
      verifyRealCatalogsInventory,
    } = await import("../../scripts/verify-i18n.mjs");

    assert.equal(MESSAGE_KEY_LEN, 6);

    // 1. Проверка эталонного digest пустой строки и граничных длин буфера (0, 4, 8, 31, 32, 64 байт)
    assert.equal(xxh64("", 0n), 0xef46db3751d8e999n);
    assert.equal(xxh64(new Uint8Array(0), 0n), 0xef46db3751d8e999n);

    const len31 = "a".repeat(31);
    const len32 = "a".repeat(32);
    const len64 = "b".repeat(64);
    assert.notEqual(xxh64(len31, 0n), xxh64(len32, 0n));
    assert.equal(typeof xxh64(len32, 0n), "bigint");
    assert.equal(typeof xxh64(len64, 0n), "bigint");
    assert.equal(messageKey(len32).length, 6);
    assert.equal(messageKey(len64).length, 6);

    // 2. Сверка всех 14 векторов из docs/re/i18n-deep.json + векторов из rust/crates/discord-i18n/src/lib.rs
    const expectedVectors = [
      ["ru", "+irF5O"],
      ["uk", "6Et1LI"],
      ["en-US", "+dXJII"],
      ["de", "njcCQ9"],
      ["ja", "fHulc+"],
      ["AKA", "l1QVfj"],
      ["GIF", "I5gL2H"],
      ["OK", "TyCVIq"],
      ["PS4", "fJyyQn"],
      ["PS5", "oElqN5"],
      ["Dismiss popup window", "T/IUYx"],
      ["Spotify", "djS8Tc"],
      ["Channel {name}, created", "t3+xw3"],
      ["add", "EEAQE1"],
      ["ADD", "OYkgVk"],
    ];
    for (const [input, expected] of expectedVectors) {
      assert.equal(
        messageKey(input),
        expected,
        `Несовпадение messageKey("${input}"): ожидалось ${expected}`,
      );
      assert.equal(isValidCatalogKeyHash(expected), true);
    }

    // Негативные проверки формата 6-символьного base64 ключа
    assert.equal(isValidCatalogKeyHash("short"), false);
    assert.equal(isValidCatalogKeyHash("toolong7"), false);
    assert.equal(isValidCatalogKeyHash("bad!@#"), false);
    assert.equal(isValidCatalogKeyHash(""), false);

    // 3. Проверка по оракулу docs/re/i18n-deep.json
    const oracleReport = verifyXxh64Oracle(I18N_DEEP_PATH);
    assert.equal(oracleReport.passed, true, `Сбой xxh64 оракула: ${oracleReport.reasons.join("; ")}`);
    assert.ok(oracleReport.checkedCases >= 15);
    assert.equal(oracleReport.mismatches.length, 0);

    // 4. Сверка с реальными каталогами /home/sanya/re-deep/ru-catalog.json и en-catalog.json
    const catReport = verifyRealCatalogsInventory();
    assert.equal(catReport.passed, true, `Сбой сверки каталогов: ${catReport.reasons.join("; ")}`);
    if (!catReport.skipped) {
      assert.equal(catReport.ruKeyCount, 23_836);
      assert.equal(catReport.enKeyCount, 23_903);
      assert.equal(catReport.enabledLocalesFoundInRu, 31);
      assert.equal(catReport.pluralMissingOtherCount, 0);
    }
  });

  it("SUB-972-3: проверяет RTL-локали (ar, he), выравнивание, детекцию BiDi и изоляцию FSI/PDI (U+2068..U+2069)", async () => {
    const {
      RTL_LOCALES,
      MAX_MESSAGE_CHARS,
      isRtlLocale,
      getLocaleDirection,
      getLocaleAlignment,
      detectRtlAndBidi,
      isolateRtlText,
      verifyRtlAlignmentAndIsolation,
    } = await import("../../scripts/verify-i18n.mjs");

    // 1. RTL локали среди 36 поддерживаемых — строго ar (арабский) и he (иврит)
    assert.deepEqual(RTL_LOCALES, ["ar", "he"]);
    assert.equal(MAX_MESSAGE_CHARS, 2000);
    assert.equal(isRtlLocale("ar"), true);
    assert.equal(isRtlLocale("AR"), true);
    assert.equal(isRtlLocale("ar-SA"), true);
    assert.equal(isRtlLocale("he"), true);
    assert.equal(isRtlLocale("he-IL"), true);
    assert.equal(isRtlLocale("ru"), false);
    assert.equal(isRtlLocale("en-US"), false);

    assert.equal(getLocaleDirection("ar"), "rtl");
    assert.equal(getLocaleDirection("he"), "rtl");
    assert.equal(getLocaleDirection("ru"), "ltr");

    const arAlign = getLocaleAlignment("ar");
    assert.deepEqual(arAlign, {
      locale: "ar",
      direction: "rtl",
      textAlign: "right",
      flexDirection: "row-reverse",
      unicodeBidi: "isolate",
    });
    const ruAlign = getLocaleAlignment("ru");
    assert.deepEqual(ruAlign, {
      locale: "ru",
      direction: "ltr",
      textAlign: "left",
      flexDirection: "row",
      unicodeBidi: "normal",
    });

    // 2. Детекция арабского и ивритского письма и опасного BiDi override (U+202E)
    const arabicSample = "مرحبا بك في ديسكورد @alex 123";
    const hebrewSample = "שלום עולם ברוכים הבאים #general";
    const bidiOverrideSample = " normal \u202Eevil.exe.png";

    const arDet = detectRtlAndBidi(arabicSample);
    assert.equal(arDet.containsRtlScript, true);
    assert.equal(arDet.containsArabic, true);
    assert.equal(arDet.containsHebrew, false);
    assert.equal(arDet.hasBidiOverride, false);

    const heDet = detectRtlAndBidi(hebrewSample);
    assert.equal(heDet.containsRtlScript, true);
    assert.equal(heDet.containsHebrew, true);
    assert.equal(heDet.containsArabic, false);

    const spoofingDet = detectRtlAndBidi(bidiOverrideSample);
    assert.equal(spoofingDet.containsRtlOrBidi, true);
    assert.equal(spoofingDet.hasBidiOverride, true);

    // 3. Безопасная BiDi-изоляция (FSI \u2068 ... PDI \u2069, замена \u202E -> \u2067, баланс маркеров)
    const isolatedAr = isolateRtlText(arabicSample);
    assert.equal(isolatedAr.containsRtlText, true);
    assert.equal(isolatedAr.rtlIsolatedSafely, true);
    assert.equal(isolatedAr.isolatesBalanced, true);
    assert.ok(isolatedAr.sanitizedOutput.startsWith("\u2068"));
    assert.ok(isolatedAr.sanitizedOutput.endsWith("\u2069"));

    const isolatedSpoof = isolateRtlText(bidiOverrideSample);
    assert.equal(isolatedSpoof.containsRtlText, true);
    assert.equal(isolatedSpoof.rtlIsolatedSafely, true);
    assert.equal(isolatedSpoof.sanitizedOutput.includes("\u202E"), false, "U+202E должен быть нейтрализован");
    assert.equal(isolatedSpoof.sanitizedOutput.includes("\u2067"), true, "U+202E заменяется на U+2067 (RLI)");
    assert.equal(isolatedSpoof.isolatesBalanced, true);

    // 4. Граничные условия: 2000 символов (OK), 0 символов (ошибка), 2001 символ (ошибка), незакрытый Markdown
    const boundary2000 = "ش".repeat(2000);
    const res2000 = isolateRtlText(boundary2000);
    assert.equal(res2000.charCount, 2000);
    assert.equal(res2000.rtlIsolatedSafely, true);

    assert.throws(() => isolateRtlText(""), /EmptyMessagePayload/);
    assert.throws(() => isolateRtlText("م".repeat(2001)), /MessageTooLong/);

    const brokenRtlMarkdown = "مرحبا **نص عريض `كود";
    const fixedMd = isolateRtlText(brokenRtlMarkdown);
    assert.equal(fixedMd.hasUnclosedMarkdown, true);
    assert.equal(fixedMd.markdownAstBalanced, true);
    assert.equal(fixedMd.rtlIsolatedSafely, true);

    // 5. Комплексная проверка RTL-стенда
    const rtlReport = verifyRtlAlignmentAndIsolation();
    assert.equal(rtlReport.passed, true, `Сбой RTL-стенда: ${rtlReport.reasons.join("; ")}`);
  });

  it("SUB-972-4: валидирует CLDR plural-формы всех 36 локалей по cldr-oracle.json, u64::MAX и рендеринг AST каталога", async () => {
    const {
      pluralCategory,
      pluralCategoryInteger,
      verifyCldrPluralsAgainstOracle,
      parseCatalogAst,
      I18nCatalogStore,
    } = await import("../../scripts/verify-i18n.mjs");

    // 1. Полная сверка всех 36 локалей (cardinal + ordinal) с rust/crates/discord-i18n/tests/data/cldr-oracle.json
    const cldrReport = verifyCldrPluralsAgainstOracle(CLDR_ORACLE_PATH);
    assert.equal(
      cldrReport.passed,
      true,
      `Несовпадения с CLDR оракулом: ${cldrReport.mismatches.slice(0, 5).join("; ")}`,
    );
    assert.equal(cldrReport.localesChecked, 36);
    assert.ok(cldrReport.samplesChecked >= 4000, `Проверено сэмплов: ${cldrReport.samplesChecked}`);
    assert.equal(cldrReport.mismatches.length, 0);

    // 2. Граничные проверки правил плурализации (ru, ar, he, da, uk, hi, pl, cs, fr, pt-PT vs pt-BR)
    for (const [n, expected] of /** @type {const} */ ([
      [1, "one"],
      [2, "few"],
      [5, "many"],
      [11, "many"],
      [21, "one"],
      [22, "few"],
      [101, "one"],
      [-1, "one"],
      [1.5, "other"],
    ])) {
      assert.equal(pluralCategory("ru", n, "cardinal"), expected, `ru cardinal ${n}`);
    }

    for (const [n, expected] of /** @type {const} */ ([
      [0, "zero"],
      [1, "one"],
      [2, "two"],
      [3, "few"],
      [10, "few"],
      [11, "many"],
      [99, "many"],
      [100, "other"],
      [3.5, "other"],
    ])) {
      assert.equal(pluralCategory("ar", n, "cardinal"), expected, `ar cardinal ${n}`);
    }

    assert.equal(pluralCategory("he", 1, "cardinal"), "one");
    assert.equal(pluralCategory("he", 2, "cardinal"), "two");
    assert.equal(pluralCategory("he", 0.5, "cardinal"), "one");
    assert.equal(pluralCategory("he", 5, "cardinal"), "other");
    assert.equal(pluralCategory("da", 1.5, "cardinal"), "one");
    assert.equal(pluralCategory("fr", 1_000_000, "cardinal"), "many");
    assert.equal(pluralCategory("es-ES", 1_000_000, "cardinal"), "many");
    assert.equal(pluralCategory("pt-PT", 0, "cardinal"), "other");
    assert.equal(pluralCategory("pt-BR", 0, "cardinal"), "one");
    assert.equal(pluralCategory("uk", 3, "ordinal"), "few");
    assert.equal(pluralCategory("uk", 13, "ordinal"), "other");
    assert.equal(pluralCategory("uk", 3.5, "ordinal"), "other");

    // 3. Точная целочисленная плурализация выше 2^53 (Snowflake и u64::MAX = 18446744073709551615n)
    // 18446744073709551615 заканчивается на 15 -> в русском языке это "many", хотя в Float64 округляется до ...1616!
    const u64Max = 18_446_744_073_709_551_615n;
    const u64One = 18_446_744_073_709_551_621n;
    const u64Few = 18_446_744_073_709_551_622n;
    assert.equal(pluralCategoryInteger("ru", u64Max, "cardinal"), "many");
    assert.equal(pluralCategoryInteger("ru", u64One, "cardinal"), "one");
    assert.equal(pluralCategoryInteger("ru", u64Few, "cardinal"), "few");
    assert.equal(pluralCategoryInteger("ru", 0n, "cardinal"), "many");

    // 4. Парсинг и рендеринг keyless JSON-AST (Literal, Argument, Number, Date, Time, Select, Plural, Pound, Tag)
    const store = new I18nCatalogStore("en-US");
    store.insertCatalog("en-US", {
      hello: [[0, "Hello "], [1, "name"]],
      only_en: [[0, "English fallback"]],
      count: [
        6,
        "n",
        {
          one: [[7], [0, " game"]],
          other: [[7], [0, " games"]],
        },
        0,
        "cardinal",
      ],
    });
    store.insertCatalog("ru", {
      hello: [[0, "Привет "], [1, "name"]],
      count: [
        6,
        "n",
        {
          one: [[7], [0, " игра"]],
          few: [[7], [0, " игры"]],
          many: [[7], [0, " игр"]],
          other: [[7], [0, " игр"]],
        },
        0,
        "cardinal",
      ],
      exact0: [6, "n", { "=0": [[0, "ноль"]], other: [[0, "другое"]] }, 0, "cardinal"],
      offset2: [
        6,
        "n",
        {
          "=3": [[0, "ровно три"]],
          one: [[7], [0, " ещё один"]],
          other: [[7], [0, " других"]],
        },
        2,
        "cardinal",
      ],
      select_role: [5, "role", { admin: [[0, "Админ"]], other: [[0, "Участник"]] }],
      rich: [8, "$b", [[0, "важно"]]],
      num: [2, "x"],
    });

    store.setLocale("ru");
    assert.equal(store.translate("hello", { name: "Мир" }), "Привет Мир");
    assert.equal(store.translate("only_en", {}), "English fallback");
    assert.equal(store.translate("count", { n: 1 }), "1 игра");
    assert.equal(store.translate("count", { n: 2 }), "2 игры");
    assert.equal(store.translate("count", { n: 5 }), "5 игр");
    assert.equal(store.translate("count", { n: 11 }), "11 игр");
    assert.equal(store.translate("count", { n: 21 }), "21 игра");
    assert.equal(store.translate("count", { n: 22 }), "22 игры");
    assert.equal(store.translate("count", { n: 101 }), "101 игра");
    assert.equal(store.translate("exact0", { n: 0 }), "ноль");
    assert.equal(store.translate("exact0", { n: 1 }), "другое");
    assert.equal(store.translate("offset2", { n: 3 }), "ровно три");
    assert.equal(store.translate("offset2", { n: 4 }), "2 других");
    assert.equal(store.translate("select_role", { role: "admin" }), "Админ");
    assert.equal(store.translate("select_role", { role: "guest" }), "Участник");
    assert.equal(store.translate("rich", {}), "**важно**");
    assert.equal(store.translate("num", { x: 1_234_567 }), "1\u00a0234\u00a0567");

    // 5. Негативные сценарии AST и ошибок перевода
    assert.throws(() => store.insertCatalog("pt-PT", {}), /UnsupportedLocale/);
    assert.throws(() => store.setLocale("ar"), /UnsupportedLocale/);
    assert.throws(() => store.translate("absent_key", {}), /MissingMessage/);
    assert.throws(() => store.translate("hello", {}), /MissingArgument/);
    assert.throws(() => store.translate("num", { x: "not-a-number" }), /InvalidArgument/);
    assert.throws(() => parseCatalogAst([99, "bad"]), /unknown node type/);

    // Глубина вложенности > 128 должна отклоняться с depth limit
    /** @type {unknown} */
    let deepNode = "leaf";
    for (let i = 0; i < 132; i += 1) {
      deepNode = [deepNode];
    }
    assert.throws(() => parseCatalogAst(deepNode), /depth limit/);
  });

  it("SUB-972-5: проверяет CLI scripts/verify-i18n.mjs (коды 0/1/2, --out-json), лимиты размера кода и 5/5 мутаций", async () => {
    const {
      generateI18nVerifierMutations,
      runI18nMutationEvaluation,
    } = await import("../../scripts/verify-i18n.mjs");

    // 1. Запуск CLI с сохранением JSON-отчёта
    const tmp = mkdtempSync(join(tmpdir(), "verify-i18n-972-"));
    try {
      const outJson = join(tmp, "i18n-report.json");
      const okRun = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--out-json", outJson],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(okRun.status, 0, `stderr: ${okRun.stderr}`);
      assert.ok(existsSync(outJson), "Файл JSON-отчёта должен быть создан");

      const report = JSON.parse(readFileSync(outJson, "utf8"));
      assert.equal(report.taskId, "TASK-972");
      assert.equal(report.status, "passed");
      assert.equal(report.locales.totalLocales, 36);
      assert.equal(report.locales.enabledCount, 31);
      assert.equal(report.locales.passed, true);
      assert.equal(report.xxh64.passed, true);
      assert.equal(report.rtl.passed, true);
      assert.equal(report.cldrPlurals.passed, true);
      assert.equal(report.cldrPlurals.localesChecked, 36);

      // Фильтрация по конкретной локали (--locale ru и --locale ar)
      const localeRun = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--locale", "ar"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(localeRun.status, 0, `stderr: ${localeRun.stderr}`);

      // Код выхода 1 при неизвестном флаге или неподдерживаемой локали
      const badFlag = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--unknown-flag"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(badFlag.status, 1);

      const badLocale = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--locale", "xx-INVALID"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(badLocale.status, 1);

      // Код выхода 2 при отсутствии явно указанного файла оракула
      const missingOracle = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--oracle", join(tmp, "missing-oracle.json")],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(missingOracle.status, 2);

      // Код выхода 1 при нарушении контракта в переданном оракуле
      const brokenOraclePath = join(tmp, "broken-oracle.json");
      writeFileSync(
        brokenOraclePath,
        JSON.stringify({
          locales: {
            "en-US": {
              plural: {
                cardinal: { "1": "many" },
                ordinal: { "1": "one" },
              },
            },
          },
        }),
        "utf8",
      );
      const brokenRun = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--oracle", brokenOraclePath],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(brokenRun.status, 1);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }

    // 2. Проверка лимитов размера файлов крейта discord-i18n и самого скрипта
    const rustLocaleSrc = readFileSync(RUST_LOCALE_PATH, "utf8");
    const sizeCheck = checkRustSourceSizeLimits("rust/crates/discord-i18n/src/locale.rs", rustLocaleSrc);
    assert.equal(sizeCheck.passed, true, sizeCheck.reason);

    const scriptLines = readFileSync(SCRIPT_PATH, "utf8").split("\n").length;
    assert.ok(scriptLines < 800, `scripts/verify-i18n.mjs должен быть < 800 строк (сейчас ${scriptLines})`);

    // 3. Мутационный гейт 5/5 (100% kill rate)
    const mutants = generateI18nVerifierMutations();
    assert.equal(mutants.length, 5);
    const mutationGate = evaluateMutationGate(mutants, (mutant) => runI18nMutationEvaluation(mutant));
    assert.equal(mutationGate.passed, true, `Выжившие мутанты: ${mutationGate.survivors.join(", ")}`);
    assert.equal(mutationGate.killed, 5);
    assert.equal(mutationGate.survived, 0);
    assert.equal(mutationGate.killRate, 1);
  });
});
