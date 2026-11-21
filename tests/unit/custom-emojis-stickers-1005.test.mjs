// @ts-check
/**
 * Контрактные и модульные тесты серверных эмодзи и анимированных стикеров Lottie (TASK-1005):
 * - SUB-1005-0: Статический контракт нативного модуля `rust/chord-gpui/src/chat/custom_emojis.rs`
 *   и валидатора `scripts/verify-emojis.mjs` (`#![forbid(unsafe_code)]`, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, модуль < 800 строк, 0 `.unwrap()`/`.expect()` в продуктовом коде,
 *   чистый `rustfmt --check --edition 2024`, компиляция `rustc --test --edition 2024 -D warnings`
 *   без предупреждений и `secret-scan.mjs`).
 * - SUB-1005-1: Сверка с независимыми оракулами репозитория (`waves/kanban-board.json` карточка `TASK-1005`,
 *   `docs/re/frontend-d03.json` экран `emoji`, `docs/re/i18n-deep.json` синтаксис `<a:name:123> / <:name:123>`
 *   и `EMOJI_NAME_RE`, `docs/re/gateway-schema.json` `emojisPerGuild` = 363 эмодзи).
 * - SUB-1005-2: Парсинг кастомных эмодзи гильдий `<:name:id>` и анимированных `<a:name:id>`,
 *   определение Jumbo-режима (1..30 только эмодзи -> 48px, смешанный текст -> 22px), генерация CDN URL,
 *   машина состояний `emoji-item` (`normal -> scale-110 -> inserted`) и блокировка чужих/анимированных эмодзи без Nitro.
 * - SUB-1005-3: Декодирование и покадровый рендеринг анимированных GIF (квантование задержки кадра >= 20 мс,
 *   пауза при `reducedMotion` / потере фокуса окна, бюджет кадра <= 8.33 мс для 120 FPS) и векторных стикеров
 *   формата Lottie JSON (`format_type = 3`, валидация схемы Bodymovin `v/fr/ip/op/w/h/layers`, лимит 512 KiB,
 *   растеризация превью 160x160 RGBA8 с де-премультипликацией альфа-канала).
 * - SUB-1005-4: Дисковый кэш в `~/.cache/discord-gpui/emojis/` (реальная запись/чтение на диск,
 *   проверка контрольной суммы SHA-256, LRU-вытеснение по числу файлов и байтовому бюджету, самовосстановление
 *   при повреждении файла, защита от path traversal `..`), покрытие всех 9 измерений `TEST-POLICY.md`
 *   и коды выхода CLI `scripts/verify-emojis.mjs` (`0 / 1 / 2`).
 * - SUB-1005-5: Мутационный гейт `5/5` (`100%` kill rate) для Rust и JS модулей по контракту `scripts/quality-gate.mjs`.
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
const RUST_MODULE_REL_PATH = "rust/chord-gpui/src/chat/custom_emojis.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const SCRIPT_REL_PATH = "scripts/verify-emojis.mjs";
const SCRIPT_PATH = join(ROOT, SCRIPT_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const I18N_DEEP_PATH = join(ROOT, "docs", "re", "i18n-deep.json");
const GATEWAY_SCHEMA_PATH = join(ROOT, "docs", "re", "gateway-schema.json");

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `custom_emojis.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1005: Серверные эмодзи <:name:id>, анимированные GIF, стикеры Lottie и кэш ~/.cache/discord-gpui/emojis/", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "custom-emojis-1005-"));
    testBinPath = join(workDir, "custom_emojis_1005_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1005-0: модули rust/chord-gpui/src/chat/custom_emojis.rs и scripts/verify-emojis.mjs существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), запрещают unsafe и компилируются без предупреждений", () => {
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
      "В продуктовом коде custom_emojis.rs запрещены .unwrap() и .expect()",
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
      `Встроенные тесты custom_emojis.rs должны проходить:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test result: ok\./);
  });

  it("SUB-1005-1: сверяет дизайн-токены, макет Emoji & Sticker Grid, состояния emoji-item и контракты пикера с независимыми оракулами репозитория", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      EMOJI_GRID_LAYOUT,
      EMOJI_DESIGN_TOKENS,
      EMOJI_ITEM_STATES,
      DEFAULT_EMOJI_CACHE_DIR,
      STICKER_FORMATS,
      FRONTEND_D03_EMOJI_CONTRACT,
      resolveEmojiItemState,
      verifyEmojisOracleParity,
    } = await import("../../scripts/verify-emojis.mjs");

    // 1. Независимый оракул: карточка TASK-1005 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1005 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1005");
    assert.ok(card1005, "Карточка TASK-1005 должна присутствовать в waves/kanban-board.json");
    assert.deepEqual(EMOJI_GRID_LAYOUT, card1005.layout);
    assert.deepEqual(EMOJI_DESIGN_TOKENS, card1005.design_tokens);
    assert.deepEqual(EMOJI_ITEM_STATES, card1005.interactive_states[0]);
    assert.equal(DEFAULT_EMOJI_CACHE_DIR, "~/.cache/discord-gpui/emojis/");

    // 2. Интерактивные состояния emoji-item: normal -> scale-110 -> inserted
    assert.equal(resolveEmojiItemState("default"), "normal");
    assert.equal(resolveEmojiItemState("hover"), "scale-110");
    assert.equal(resolveEmojiItemState("active"), "inserted");

    // 3. Форматы стикеров Discord API v9 (1=PNG, 2=APNG, 3=LOTTIE, 4=GIF)
    assert.equal(STICKER_FORMATS.PNG, 1);
    assert.equal(STICKER_FORMATS.APNG, 2);
    assert.equal(STICKER_FORMATS.LOTTIE, 3);
    assert.equal(STICKER_FORMATS.GIF, 4);

    // 4. Независимый оракул docs/re/frontend-d03.json (экран emoji)
    const d03 = JSON.parse(readFileSync(FRONTEND_D03_PATH, "utf8"));
    const emojiScreen = d03.screens.find((/** @type {any} */ s) => s.name === "emoji");
    assert.ok(emojiScreen, "Экран emoji должен присутствовать в docs/re/frontend-d03.json");
    assert.equal(emojiScreen.counts.tabCount, FRONTEND_D03_EMOJI_CONTRACT.tabCount);
    assert.equal(emojiScreen.counts.navCategories, FRONTEND_D03_EMOJI_CONTRACT.navCategories);
    assert.equal(emojiScreen.counts.visibleGridChannel, FRONTEND_D03_EMOJI_CONTRACT.visibleGridChannel);
    assert.equal(emojiScreen.counts.lockedChannel, FRONTEND_D03_EMOJI_CONTRACT.lockedChannel);
    assert.equal(emojiScreen.counts.visibleGridDM, FRONTEND_D03_EMOJI_CONTRACT.visibleGridDM);
    assert.equal(emojiScreen.counts.reactionPickerItems, FRONTEND_D03_EMOJI_CONTRACT.reactionPickerItems);
    assert.equal(emojiScreen.counts.reactionPickerLocked, FRONTEND_D03_EMOJI_CONTRACT.reactionPickerLocked);
    assert.deepEqual(emojiScreen.structure.navRail.categories, FRONTEND_D03_EMOJI_CONTRACT.categories);

    // 5. Полная сверка всех оракулов через verifyEmojisOracleParity
    const parity = verifyEmojisOracleParity({
      kanbanBoardPath: KANBAN_BOARD_PATH,
      frontendD03Path: FRONTEND_D03_PATH,
      i18nDeepPath: I18N_DEEP_PATH,
      gatewaySchemaPath: GATEWAY_SCHEMA_PATH,
    });
    assert.equal(parity.passed, true, `Несовпадение с оракулами: ${parity.reasons.join("; ")}`);
    assert.equal(parity.totalReadyEmojis, 363);
    assert.deepEqual(parity.sampleGuildEmojis, ["CoP", "ClearSky"]);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1005_1_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1005-1 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1005_1_.*ok/);
  });

  it("SUB-1005-2: разбирает кастомные эмодзи <:name:id> и анимированные <a:name:id>, определяет Jumbo-режим (1..30 эмодзи), строит CDN URL и проверяет Nitro/права EXTERNAL_EMOJIS", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      parseCustomEmojiTokens,
      computeMessageEmojiPresentation,
      buildEmojiCdnUrl,
      buildStickerCdnUrl,
      canUseCustomEmoji,
    } = await import("../../scripts/verify-emojis.mjs");

    // 1. Разбор статических <:name:id> и анимированных <a:name:id> токенов в тексте сообщения
    const text = "Привет <:CoP:3527262836281581568> и анимированный <a:party_blob:1056141433670742056>!";
    const tokens = parseCustomEmojiTokens(text);
    assert.equal(tokens.length, 2);
    assert.deepEqual(tokens[0], {
      raw: "<:CoP:3527262836281581568>",
      name: "CoP",
      id: "3527262836281581568",
      animated: false,
      index: 7,
    });
    assert.deepEqual(tokens[1], {
      raw: "<a:party_blob:1056141433670742056>",
      name: "party_blob",
      id: "1056141433670742056",
      animated: true,
      index: 50,
    });

    // 2. Экранированные \<:name:id>, некорректные имена (<2 или >32 символов) и Snowflake=0 игнорируются
    const invalidText =
      "\\<:escaped:123456789012345678> <:a:123456789012345678> <:bad_id:0> <:too_long_name_12345678901234567890123:123456789012345678>";
    assert.equal(parseCustomEmojiTokens(invalidText).length, 0);

    // 3. Jumbo-режим: сообщение из 1..30 одних только эмодзи рендерится в 48x48 px, иначе 22x22 px
    const jumboPres = computeMessageEmojiPresentation(
      "<:CoP:3527262836281581568> <a:party_blob:1056141433670742056>",
    );
    assert.equal(jumboPres.jumboable, true);
    assert.equal(jumboPres.renderSizePx, 48);
    assert.equal(jumboPres.emojiCount, 2);

    const inlinePres = computeMessageEmojiPresentation(
      "Текст с эмодзи <:CoP:3527262836281581568>",
    );
    assert.equal(inlinePres.jumboable, false);
    assert.equal(inlinePres.renderSizePx, 22);

    const thirtyOneEmojis = Array.from(
      { length: 31 },
      (_, i) => `<:em_${i + 10}:1000000000000000${String(i + 10).padStart(2, "0")}>`,
    ).join(" ");
    const overLimitPres = computeMessageEmojiPresentation(thirtyOneEmojis);
    assert.equal(overLimitPres.emojiCount, 31);
    assert.equal(overLimitPres.jumboable, false);
    assert.equal(overLimitPres.renderSizePx, 22);

    // 4. Формирование безопасных CDN URL для эмодзи и стикеров
    assert.equal(
      buildEmojiCdnUrl({ id: "1056141433670742056", animated: false, size: 60 }),
      "https://cdn.discordapp.com/emojis/1056141433670742056.webp?size=60",
    );
    assert.equal(
      buildEmojiCdnUrl({ id: "1056141433670742056", animated: true, size: 64 }),
      "https://cdn.discordapp.com/emojis/1056141433670742056.gif?size=64",
    );
    assert.equal(
      buildStickerCdnUrl({ id: "875058193758289990", formatType: 1 }),
      "https://cdn.discordapp.com/stickers/875058193758289990.png",
    );
    assert.equal(
      buildStickerCdnUrl({ id: "875058193758289990", formatType: 3 }),
      "https://cdn.discordapp.com/stickers/875058193758289990.json",
    );
    assert.equal(
      buildStickerCdnUrl({ id: "875058193758289990", formatType: 4 }),
      "https://media.discordapp.net/stickers/875058193758289990.gif",
    );

    // 5. Проверка доступности эмодзи (Nitro / локальная гильдия / право USE_EXTERNAL_EMOJIS 1 << 18)
    assert.equal(
      canUseCustomEmoji({
        emojiGuildId: "101",
        currentGuildId: "101",
        animated: false,
        available: true,
        hasNitro: false,
        hasUseExternalEmojisPerm: true,
      }).allowed,
      true,
    );
    // Анимированный эмодзи без подписки Nitro заблокирован (lockedEmoji_d982c2)
    const lockedAnim = canUseCustomEmoji({
      emojiGuildId: "101",
      currentGuildId: "101",
      animated: true,
      available: true,
      hasNitro: false,
      hasUseExternalEmojisPerm: true,
    });
    assert.equal(lockedAnim.allowed, false);
    assert.equal(lockedAnim.cssClass, "lockedEmoji_d982c2");

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1005_2_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1005-2 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1005_2_.*ok/);
  });

  it("SUB-1005-3: выполняет покадровый рендеринг анимированных GIF и векторных стикеров Lottie JSON (160x160 RGBA8 с де-премультипликацией альфы)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      stepAnimatedGifPlayback,
      validateAndRenderLottieSticker,
    } = await import("../../scripts/verify-emojis.mjs");

    // 1. Анимированный GIF: квантование задержек кадров (< 20 мс -> 100 мс по спецификации браузеров/GIF89a) и выбор кадра
    const gifPlay = stepAnimatedGifPlayback({
      frameDelaysMs: [40, 40, 10, 80], // 10 мс нормализуется в 100 мс -> сумма = 260 мс
      elapsedMs: 95,
      windowFocused: true,
      reducedMotion: false,
      hovering: false,
    });
    assert.equal(gifPlay.playing, true);
    assert.equal(gifPlay.totalDurationMs, 260);
    assert.equal(gifPlay.frameIndex, 2);
    assert.ok(gifPlay.frameDecodeBudgetMs <= 8.33);

    // При reducedMotion=true без наведения курсора анимация стоит на 0-м кадре
    const gifPaused = stepAnimatedGifPlayback({
      frameDelaysMs: [40, 40, 80],
      elapsedMs: 95,
      windowFocused: true,
      reducedMotion: true,
      hovering: false,
    });
    assert.equal(gifPaused.playing, false);
    assert.equal(gifPaused.frameIndex, 0);

    // 2. Валидный стикер Lottie JSON (Bodymovin формат: v, fr=60, ip=0, op=120, w=512, h=512, layers)
    const sampleLottieJson = JSON.stringify({
      v: "5.7.4",
      fr: 60,
      ip: 0,
      op: 120,
      w: 512,
      h: 512,
      nm: "wumpus_wave",
      layers: [
        {
          ty: 4,
          nm: "hand_shape",
          ip: 0,
          op: 120,
          ks: { o: { a: 0, k: 100 } },
          rgbaPremul: [64, 32, 96, 128], // полупрозрачный премультиплицированный пиксель
        },
      ],
    });

    const rendered = validateAndRenderLottieSticker(sampleLottieJson, { elapsedMs: 500 });
    assert.equal(rendered.valid, true);
    assert.equal(rendered.targetWidth, 160);
    assert.equal(rendered.targetHeight, 160);
    assert.equal(rendered.scaleFactor, Number((160 / 512).toFixed(6)));
    assert.equal(rendered.fps, 60);
    assert.equal(rendered.currentFrame, 30); // 0.5 сек * 60 fps = 30-й кадр
    assert.ok(rendered.frameRenderTimeMs <= 8.33);
    // Де-премультипликация: R = min(255, floor((64 * 255 + 64) / 128)) = 128, B = floor((96 * 255 + 64) / 128) = 191
    assert.deepEqual(rendered.sampleUnpremultipliedRgba, [128, 64, 191, 128]);

    // 3. Негативные сценарии Lottie: холст > 1024x1024, пустые слои, превышение 512 KiB
    const oversizedCanvas = JSON.stringify({
      v: "5.7.4",
      fr: 60,
      ip: 0,
      op: 60,
      w: 2048,
      h: 2048,
      layers: [{ ty: 4, nm: "layer" }],
    });
    const badCanvas = validateAndRenderLottieSticker(oversizedCanvas);
    assert.equal(badCanvas.valid, false);
    assert.match(badCanvas.reason, /canvas|размер/i);

    const emptyLayers = JSON.stringify({
      v: "5.7.4",
      fr: 60,
      ip: 0,
      op: 60,
      w: 512,
      h: 512,
      layers: [],
    });
    assert.equal(validateAndRenderLottieSticker(emptyLayers).valid, false);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1005_3_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1005-3 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1005_3_.*ok/);
  });

  it("SUB-1005-4: кэширует эмодзи и стикеры на диске в ~/.cache/discord-gpui/emojis/ с SHA-256 и LRU, покрывает 9 измерений TEST-POLICY.md и проверяет CLI scripts/verify-emojis.mjs", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      EmojiDiskCache,
      resolveEmojiCacheDirPath,
      evaluateEmojiPolicyDimensions,
      redactEmojiTelemetry,
    } = await import("../../scripts/verify-emojis.mjs");

    // 1. Проверка канонического пути ~/.cache/discord-gpui/emojis/
    assert.equal(
      resolveEmojiCacheDirPath({ HOME: "/home/tester" }),
      "/home/tester/.cache/discord-gpui/emojis/",
    );

    // 2. Реальный дисковый кэш во временной директории с проверкой SHA-256 и LRU-вытеснением
    const cacheRoot = join(workDir, ".cache", "discord-gpui", "emojis");
    const cache = new EmojiDiskCache({
      cacheDir: cacheRoot,
      maxEntries: 3,
      maxBytes: 1_024,
    });

    const payload1 = Buffer.from("RIFF_WEBP_EMOJI_1001_PAYLOAD");
    const payload2 = Buffer.from("GIF89a_ANIM_EMOJI_1002_PAYLOAD");
    const payload3 = Buffer.from("PNG_LOTTIE_STICKER_1003_PAYLOAD");
    const payload4 = Buffer.from("WEBP_EMOJI_1004_EVICTS_OLDEST");

    const put1 = cache.put("emoji_1001.webp", payload1);
    assert.equal(put1.stored, true);
    assert.ok(existsSync(put1.filePath));

    const hit1 = cache.get("emoji_1001.webp");
    assert.equal(hit1.hit, true);
    assert.deepEqual(hit1.bytes, payload1);

    cache.put("emoji_1002.gif", payload2);
    cache.put("sticker_1003.png", payload3);
    // Доступ к emoji_1001 делает emoji_1002 самым старым по LRU
    cache.get("emoji_1001.webp");
    cache.put("emoji_1004.webp", payload4);

    assert.equal(cache.get("emoji_1002.gif").hit, false, "Самый старый элемент emoji_1002.gif должен быть вытеснен по LRU");
    assert.equal(cache.get("emoji_1001.webp").hit, true);
    assert.equal(cache.get("sticker_1003.png").hit, true);
    assert.equal(cache.get("emoji_1004.webp").hit, true);

    // 3. Самовосстановление при повреждении файла на диске (несовпадение SHA-256)
    writeFileSync(put1.filePath, "CORRUPTED_BYTES_ON_DISK", "utf8");
    const corruptedRead = cache.get("emoji_1001.webp");
    assert.equal(corruptedRead.hit, false);
    assert.equal(corruptedRead.reason, "sha256_mismatch_evicted");
    assert.equal(existsSync(put1.filePath), false, "Повреждённый кэш-файл должен автоматически удаляться");

    // 4. Защита от Path Traversal в ключе кэша
    assert.throws(() => cache.put("../passwd", payload1), /traversal|недопустим/i);

    // 5. Проверка всех 9 обязательных измерений TEST-POLICY.md
    const policy = evaluateEmojiPolicyDimensions(join(workDir, "policy-cache"));
    assert.equal(policy.allPassed, true, `Сбой измерений TEST-POLICY: ${policy.failures.join("; ")}`);
    assert.equal(policy.dimensionsChecked, 9);

    // 6. Маскирование секретов в логах
    const sampleLog =
      'Emoji CDN fetch auth="Bearer mfa.abCDef1234567890_secretTokenValue" token=dGVzdG1vY2t0b2tlbnZhbHVl.GhIjKl.MnOpQrStUvWxYz0123456789abcdef';
    const redacted = redactEmojiTelemetry(sampleLog);
    assert.doesNotMatch(redacted, /mfa\.abCDef1234567890_secretTokenValue/);
    assert.doesNotMatch(redacted, /GhIjKl\.MnOpQrStUvWxYz0123456789abcdef/);
    assert.match(redacted, /\[REDACTED\]/);

    // 7. Проверка CLI scripts/verify-emojis.mjs (коды выхода 0 / 1 / 2)
    const outJson = join(workDir, "verify-emojis-report.json");
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
      [SCRIPT_PATH, "--oracle", "/nonexistent/missing-kanban-1005.json"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliMissingOracle.status, 2);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1005_4_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1005-4 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1005_4_.*ok/);
  });

  it("SUB-1005-5: проходит мутационный гейт 5/5 (100% kill rate) для Rust и JS реализаций по контракту scripts/quality-gate.mjs", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const rustMutationSpecs = [
      {
        id: "M1-RUST-ANIMATED-FLAG-INVERT",
        operator: "AnimatedEmojiPrefixIgnored",
        from: 'let animated = prefix == "<a:";',
        to: "let animated = false;",
      },
      {
        id: "M2-RUST-JUMBO-LIMIT-CORRUPT",
        operator: "JumboMaxEmojiCountCorrupted",
        from: "pub const MAX_JUMBO_EMOJI_COUNT: usize = 30;",
        to: "pub const MAX_JUMBO_EMOJI_COUNT: usize = 1;",
      },
      {
        id: "M3-RUST-HOVER-STATE-CORRUPT",
        operator: "EmojiItemHoverStateBroken",
        from: 'Self::Hover => "scale-110",',
        to: 'Self::Hover => "normal",',
      },
      {
        id: "M4-RUST-LOTTIE-CANVAS-BYPASS",
        operator: "LottieMaxCanvasDimensionBypassed",
        from: "if !is_valid_lottie_canvas(spec.width, spec.height) {",
        to: "if false {",
      },
      {
        id: "M5-RUST-CACHE-LRU-EVICTION-SKIP",
        operator: "DiskCacheLruEvictionDisabled",
        from: "self.evict_until_within_budget(bytes.len());",
        to: "let _ = bytes.len();",
      },
    ];

    const rustMutants = rustMutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна существовать в custom_emojis.rs`,
      );
      return {
        id: spec.id,
        moduleName: "custom_emojis.rs",
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
        id: "M1-JS-ANIMATED-PREFIX-INVERT",
        operator: "AnimatedEmojiFlagDisabled",
        from: 'const animated = match[1] === "a";',
        to: "const animated = false;",
      },
      {
        id: "M2-JS-HOVER-STATE-CORRUPT",
        operator: "EmojiItemHoverScaleBroken",
        from: 'hover: "scale-110",',
        to: 'hover: "normal",',
      },
      {
        id: "M3-JS-LOTTIE-CANVAS-BYPASS",
        operator: "LottieCanvasLimitBypassed",
        from: "if (w <= 0 || h <= 0 || w > 1024 || h > 1024) {",
        to: "if (false) {",
      },
      {
        id: "M4-JS-CACHE-DIR-CORRUPT",
        operator: "DefaultEmojiCacheDirCorrupted",
        from: 'export const DEFAULT_EMOJI_CACHE_DIR = "~/.cache/discord-gpui/emojis/";',
        to: 'export const DEFAULT_EMOJI_CACHE_DIR = "/tmp/wrong-emoji-cache/";',
      },
      {
        id: "M5-JS-SHA256-VERIFY-BYPASS",
        operator: "DiskCacheChecksumVerificationSkipped",
        from: "if (actualSha256 !== entry.sha256) {",
        to: "if (false) {",
      },
    ];

    const jsMutants = jsMutationSpecs.map((spec) => {
      assert.ok(
        originalJs.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна существовать в verify-emojis.mjs`,
      );
      return {
        id: spec.id,
        moduleName: "verify-emojis.mjs",
        operator: spec.operator,
        mutatedSource: originalJs.replace(spec.from, spec.to),
      };
    });

    const jsGate = evaluateMutationGate(jsMutants, (mutant, index) => {
      const mutantJsPath = join(workDir, `mutant_emojis_${index + 1}.mjs`);
      const harnessPath = join(workDir, `mutant_harness_${index + 1}.mjs`);
      const tempCacheDir = join(workDir, `mutant_cache_${index + 1}`);
      writeFileSync(mutantJsPath, mutant.mutatedSource, "utf8");
      writeFileSync(
        harnessPath,
        [
          'import assert from "node:assert/strict";',
          'import { writeFileSync } from "node:fs";',
          `import { parseCustomEmojiTokens, EMOJI_ITEM_STATES, DEFAULT_EMOJI_CACHE_DIR, validateAndRenderLottieSticker, EmojiDiskCache } from ${JSON.stringify(mutantJsPath)};`,
          'const parsed = parseCustomEmojiTokens("<a:wave:1056141433670742056>");',
          "assert.equal(parsed[0]?.animated, true);",
          'assert.equal(EMOJI_ITEM_STATES.hover, "scale-110");',
          'assert.equal(DEFAULT_EMOJI_CACHE_DIR, "~/.cache/discord-gpui/emojis/");',
          'const badLottie = validateAndRenderLottieSticker(JSON.stringify({ v: "5.7.4", fr: 60, ip: 0, op: 60, w: 2048, h: 2048, layers: [{ ty: 4 }] }));',
          "assert.equal(badLottie.valid, false);",
          `const cache = new EmojiDiskCache({ cacheDir: ${JSON.stringify(tempCacheDir)}, maxEntries: 4, maxBytes: 4096 });`,
          'const saved = cache.put("e1.webp", Buffer.from("VALID_WEBP"));',
          'writeFileSync(saved.filePath, "TAMPERED");',
          'assert.equal(cache.get("e1.webp").hit, false);',
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
