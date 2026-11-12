// @ts-check
/**
 * Контрактные и модульные тесты стенда аудита системных шрифтов, цепочки fallback
 * (Noto Color Emoji, CJK, RTL Arabic/Hebrew, Tofurion) и кэша атласа WGPU (TASK-981):
 * - SUB-981-1: Дизайн-токены, интерактивные состояния `emoji-picker` (`rendered -> lookup glyph -> atlas cached`),
 *   классификация Unicode-скриптов (`latin`, `cyrillic`, `emoji`, `cjk`, `rtl_arabic`, `rtl_hebrew`,
 *   `symbols_math`, `private_or_unassigned`), парсинг `fc-list` и сверка с независимыми оракулами
 *   (`waves/kanban-board.json`, `rust/discord-gpui/src/theme/discord_dark.rs`, `rust/gui-spike/ui/tokens.css`,
 *   `rust/discord-gpui/src/state/mod.rs`, `docs/re/frontend-d03.json`).
 * - SUB-981-2: Сегментация графемных кластеров (ZWJ-последовательности `👨‍👩‍👧‍👦`, модификаторы тона кожи `👋🏽`,
 *   региональные флаги `🇯🇵`, вариационные селекторы `❤️`, арабские огласовки `مَرْحَبًا`) и разрешение
 *   цепочки шрифтов для Noto Color Emoji (`rgba8_unorm_srgb`), CJK (`SC`/`TC`/`JP`/`KR`, индексы TTC `0..3`)
 *   и RTL (`Arabic`/`Hebrew`, `direction: "rtl"`, `harfBuzzShaped: true`).
 * - SUB-981-3: Гарантированная подстановка `tofurion` (`glyphId: 0`, `replacementChar: "\uFFFD"`,
 *   `resolvedFont: "Tofurion Fallback"`, `isTofurion: true`) для отсутствующих глифов, области PUA
 *   (`U+E000..U+F8FF`, `U+F0000..U+10FFFD`), неназначенных скаляров (`U+0378`) и при отказе всех
 *   шрифтов цепочки — строго без паник (`panicked: false`).
 * - SUB-981-4: Симулятор текстурного атласа WGPU (`WgpuGlyphAtlasSimulator`) — раздельные страницы
 *   `r8_unorm` (маски текста/CJK/RTL) и `rgba8_unorm_srgb` (цветные эмодзи), бин-пакинг Shelf с `1px`
 *   паддингом против билинейного кровотечения, LRU-вытеснение при заполнении лимита страниц и защита
 *   от паник на нулевых/отрицательных/сверхбольших (`> 8192px`) размерах глифа.
 * - SUB-981-5: Сквозной матричный аудит (`runFontFallbackMatrixAudit`), покрытие всех 9 измерений
 *   `TEST-POLICY.md` (`evaluateFontAuditEdgeDimensions`), маскирование токенов (`redactFontAuditLogs`),
 *   CLI `scripts/font-audit.mjs` (коды `0/1/2`, `--out-json`, `--out-md`, `--script`, `--oracle`),
 *   лимиты размера кода (`< 800` строк на файл, `< 50` строк на функцию) и мутационный гейт `5/5`.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
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
const SCRIPT_PATH = join(ROOT, "scripts", "font-audit.mjs");
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const GPUI_THEME_PATH = join(ROOT, "rust", "discord-gpui", "src", "theme", "discord_dark.rs");
const GPUI_STATE_PATH = join(ROOT, "rust", "discord-gpui", "src", "state", "mod.rs");
const UI_TOKENS_CSS_PATH = join(ROOT, "rust", "gui-spike", "ui", "tokens.css");
const FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");

describe("TASK-981: Комплексный чекер системных шрифтов и кэша атласа WGPU (scripts/font-audit.mjs)", () => {
  it("SUB-981-1: сверяет дизайн-токены, интерактивные состояния emoji-picker, классификацию скриптов Unicode, fc-list и независимые оракулы", async () => {
    const {
      FONT_AUDIT_DESIGN_TOKENS,
      FONT_MANAGER_LAYOUT,
      EMOJI_PICKER_STATES,
      FONT_FALLBACK_CHAINS,
      WGPU_ATLAS_DEFAULT_CONFIG,
      CJK_TTC_FACE_INDICES,
      classifyCodePointScript,
      parseFcListFamilies,
      probeSystemFonts,
      verifyFontAuditOracleParity,
    } = await import("../../scripts/font-audit.mjs");

    // 1. Независимый оракул: карточка TASK-981 в waves/kanban-board.json (проверка md5 и полей)
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card981 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-981");
    assert.ok(card981, "Карточка TASK-981 должна присутствовать в waves/kanban-board.json");
    assert.deepEqual(FONT_AUDIT_DESIGN_TOKENS, card981.design_tokens);
    assert.deepEqual(FONT_MANAGER_LAYOUT, card981.layout);
    assert.deepEqual(EMOJI_PICKER_STATES, card981.interactive_states[0]);

    // 2. Проверка цепочек резервных шрифтов (Emoji, CJK, RTL, Tofurion Fallback)
    assert.ok(FONT_FALLBACK_CHAINS.primary.includes("Noto Sans"));
    assert.ok(FONT_FALLBACK_CHAINS.primary.includes("Noto Color Emoji"));
    assert.equal(FONT_FALLBACK_CHAINS.primary.at(-1), "Tofurion Fallback");
    assert.equal(FONT_FALLBACK_CHAINS.emoji[0], "Noto Color Emoji");
    assert.ok(FONT_FALLBACK_CHAINS.emoji.includes("Twemoji"));
    assert.equal(FONT_FALLBACK_CHAINS.emoji.at(-1), "Tofurion Fallback");
    assert.ok(FONT_FALLBACK_CHAINS.cjk.includes("Noto Sans CJK SC"));
    assert.ok(FONT_FALLBACK_CHAINS.cjk.includes("Noto Sans CJK JP"));
    assert.ok(FONT_FALLBACK_CHAINS.cjk.includes("Noto Sans CJK KR"));
    assert.equal(FONT_FALLBACK_CHAINS.cjk.at(-1), "Tofurion Fallback");
    assert.ok(FONT_FALLBACK_CHAINS.rtl.includes("Noto Sans Arabic"));
    assert.ok(FONT_FALLBACK_CHAINS.rtl.includes("Noto Sans Hebrew"));
    assert.equal(FONT_FALLBACK_CHAINS.rtl.at(-1), "Tofurion Fallback");

    // 3. Индексы региональных лиц в коллекции NotoSansCJK.ttc (JP=0, KR=1, SC=2, TC=3)
    assert.equal(CJK_TTC_FACE_INDICES.ja, 0);
    assert.equal(CJK_TTC_FACE_INDICES.ko, 1);
    assert.equal(CJK_TTC_FACE_INDICES["zh-CN"], 2);
    assert.equal(CJK_TTC_FACE_INDICES["zh-TW"], 3);

    // 4. Параметры текстурного атласа глифов WGPU по умолчанию
    assert.equal(WGPU_ATLAS_DEFAULT_CONFIG.pageWidthPx, 2048);
    assert.equal(WGPU_ATLAS_DEFAULT_CONFIG.pageHeightPx, 2048);
    assert.equal(WGPU_ATLAS_DEFAULT_CONFIG.maxTextureDimension2D, 8192);
    assert.equal(WGPU_ATLAS_DEFAULT_CONFIG.glyphPaddingPx, 1);
    assert.equal(WGPU_ATLAS_DEFAULT_CONFIG.maxPagesPerFormat, 4);
    assert.equal(WGPU_ATLAS_DEFAULT_CONFIG.maskFormat, "r8_unorm");
    assert.equal(WGPU_ATLAS_DEFAULT_CONFIG.colorFormat, "rgba8_unorm_srgb");

    // 5. Классификация кодовых точек Unicode по скриптам
    assert.equal(classifyCodePointScript("A".codePointAt(0) ?? 0), "latin");
    assert.equal(classifyCodePointScript("Я".codePointAt(0) ?? 0), "cyrillic");
    assert.equal(classifyCodePointScript("🔥".codePointAt(0) ?? 0), "emoji");
    assert.equal(classifyCodePointScript("🦀".codePointAt(0) ?? 0), "emoji");
    assert.equal(classifyCodePointScript("骨".codePointAt(0) ?? 0), "cjk");
    assert.equal(classifyCodePointScript("あ".codePointAt(0) ?? 0), "cjk");
    assert.equal(classifyCodePointScript("ア".codePointAt(0) ?? 0), "cjk");
    assert.equal(classifyCodePointScript("한".codePointAt(0) ?? 0), "cjk");
    assert.equal(classifyCodePointScript("ع".codePointAt(0) ?? 0), "rtl_arabic");
    assert.equal(classifyCodePointScript("ש".codePointAt(0) ?? 0), "rtl_hebrew");
    assert.equal(classifyCodePointScript("✦".codePointAt(0) ?? 0), "symbols_math");
    assert.equal(classifyCodePointScript(0xe000), "private_or_unassigned");
    assert.equal(classifyCodePointScript(0xf0000), "private_or_unassigned");
    assert.equal(classifyCodePointScript(0x0378), "private_or_unassigned");
    assert.equal(classifyCodePointScript(0xfffd), "private_or_unassigned");

    // 6. Разбор вывода fc-list и зондирование системных шрифтов
    const parsedFamilies = parseFcListFamilies(
      "Noto Color Emoji\nNoto Sans,Noto Sans Regular\nNoto Sans Arabic,Noto Sans Arabic Med\n.HiddenSystemUI\nLiberation Mono\n",
    );
    assert.deepEqual(parsedFamilies, [
      "Liberation Mono",
      "Noto Color Emoji",
      "Noto Sans",
      "Noto Sans Arabic",
      "Noto Sans Arabic Med",
      "Noto Sans Regular",
    ]);

    const sysProbe = probeSystemFonts();
    assert.equal(sysProbe.panicked, false);
    assert.ok(sysProbe.effectiveFamilies.includes("Noto Color Emoji"));
    assert.ok(sysProbe.effectiveFamilies.includes("Noto Sans"));
    assert.ok(sysProbe.effectiveFamilies.includes("Noto Sans Arabic"));
    assert.ok(sysProbe.effectiveFamilies.includes("Noto Sans Hebrew"));
    assert.ok(sysProbe.effectiveFamilies.includes("Noto Sans CJK SC"));

    // 7. Сверка с реальными файлами оракулов репозитория
    const oracleParity = verifyFontAuditOracleParity({
      kanbanBoardPath: KANBAN_BOARD_PATH,
      gpuiThemePath: GPUI_THEME_PATH,
      gpuiStatePath: GPUI_STATE_PATH,
      uiTokensCssPath: UI_TOKENS_CSS_PATH,
      frontendD03Path: FRONTEND_D03_PATH,
    });
    assert.equal(oracleParity.passed, true, `Несовпадение с оракулами: ${oracleParity.reasons.join("; ")}`);
    assert.equal(oracleParity.emojiPickerClass, "emojiPicker_c0e32c");
    assert.equal(oracleParity.emojiItemClass, "emojiItem_fc7141");
    assert.equal(oracleParity.lockedEmojiClass, "lockedEmoji_d982c2");
    assert.deepEqual(oracleParity.gpuiFontScaleBounds, [0.75, 1.5]);
    assert.equal(oracleParity.typographyScaleOrdered, true);
  });

  it("SUB-981-2: сегментирует графемные кластеры (ZWJ, тон кожи, флаги, харакат) и разрешает fallback для Noto Color Emoji, CJK и RTL", async () => {
    const {
      segmentGraphemeClusters,
      resolveGlyphFallback,
    } = await import("../../scripts/font-audit.mjs");

    // 1. Сегментация сложных графемных кластеров без разрыва суррогатных пар и ZWJ
    const clusters = segmentGraphemeClusters("A🔥👨‍👩‍👧‍👦👋🏽🇯🇵骨مَرْ");
    assert.equal(clusters[0], "A");
    assert.equal(clusters[1], "🔥");
    assert.equal(clusters[2], "👨‍👩‍👧‍👦");
    assert.equal(clusters[3], "👋🏽");
    assert.equal(clusters[4], "🇯🇵");
    assert.equal(clusters[5], "骨");
    assert.equal(clusters[6], "مَ");
    assert.equal(clusters[7], "رْ");

    // 2. Разрешение одиночных и составных эмодзи через Noto Color Emoji (цветной атлас RGBA8)
    for (const emojiCluster of ["🔥", "🦀", "🚀", "🎉", "👨‍👩‍👧‍👦", "👩‍💻", "👋🏽", "🇯🇵", "❤️"]) {
      const resolved = resolveGlyphFallback(emojiCluster);
      assert.equal(resolved.panicked, false);
      assert.equal(resolved.isTofurion, false, `Эмодзи ${emojiCluster} не должен падать в tofurion`);
      assert.equal(resolved.script, "emoji");
      assert.equal(resolved.resolvedFont, "Noto Color Emoji");
      assert.equal(resolved.isColorBitmap, true);
      assert.equal(resolved.atlasFormat, "rgba8_unorm_srgb");
      assert.equal(resolved.direction, "ltr");
      assert.ok(resolved.glyphId > 0);
    }

    // Fallback с Noto Color Emoji на Twemoji, если системный Noto Color Emoji отключён
    const twemojiFallback = resolveGlyphFallback("🦀", {
      availableFonts: ["Noto Sans", "Twemoji", "Tofurion Fallback"],
    });
    assert.equal(twemojiFallback.resolvedFont, "Twemoji");
    assert.equal(twemojiFallback.isTofurion, false);
    assert.equal(twemojiFallback.atlasFormat, "rgba8_unorm_srgb");

    // 3. Разрешение CJK (китайский упрощённый/традиционный, японская кана/кандзи, корейский хангыль)
    const cjkCases = [
      { char: "骨", locale: "ja", expectedFont: "Noto Sans CJK JP", expectedTtcIndex: 0 },
      { char: "あ", locale: "ja", expectedFont: "Noto Sans CJK JP", expectedTtcIndex: 0 },
      { char: "ア", locale: "ja", expectedFont: "Noto Sans CJK JP", expectedTtcIndex: 0 },
      { char: "한", locale: "ko", expectedFont: "Noto Sans CJK KR", expectedTtcIndex: 1 },
      { char: "汉", locale: "zh-CN", expectedFont: "Noto Sans CJK SC", expectedTtcIndex: 2 },
      { char: "繁", locale: "zh-TW", expectedFont: "Noto Sans CJK TC", expectedTtcIndex: 3 },
    ];
    for (const item of cjkCases) {
      const res = resolveGlyphFallback(item.char, { locale: item.locale });
      assert.equal(res.panicked, false);
      assert.equal(res.isTofurion, false);
      assert.equal(res.script, "cjk");
      assert.equal(res.resolvedFont, item.expectedFont);
      assert.equal(res.ttcFaceIndex, item.expectedTtcIndex);
      assert.equal(res.isColorBitmap, false);
      assert.equal(res.atlasFormat, "r8_unorm");
      assert.ok(res.glyphId > 0);
    }

    // 4. Разрешение RTL глифов (арабское письмо с харакатом и иврит с никудом)
    const arabicRes = resolveGlyphFallback("مَ", { locale: "ar" });
    assert.equal(arabicRes.panicked, false);
    assert.equal(arabicRes.isTofurion, false);
    assert.equal(arabicRes.script, "rtl_arabic");
    assert.equal(arabicRes.resolvedFont, "Noto Sans Arabic");
    assert.equal(arabicRes.direction, "rtl");
    assert.equal(arabicRes.harfBuzzShaped, true);
    assert.equal(arabicRes.atlasFormat, "r8_unorm");

    const hebrewRes = resolveGlyphFallback("שָׁ", { locale: "he" });
    assert.equal(hebrewRes.panicked, false);
    assert.equal(hebrewRes.isTofurion, false);
    assert.equal(hebrewRes.script, "rtl_hebrew");
    assert.equal(hebrewRes.resolvedFont, "Noto Sans Hebrew");
    assert.equal(hebrewRes.direction, "rtl");
    assert.equal(hebrewRes.harfBuzzShaped, true);
    assert.equal(hebrewRes.atlasFormat, "r8_unorm");
  });

  it("SUB-981-3: заменяет отсутствующие, PUA и невалидные глифы на tofurion (.notdef / U+FFFD) без паники", async () => {
    const {
      resolveGlyphFallback,
      validateCustomFontBytes,
    } = await import("../../scripts/font-audit.mjs");

    // 1. Приватные области Unicode (PUA BMP U+E000, Supplementary PUA-A U+F0001, PUA-B U+10FFFD) и неназначенные кодовые точки
    const missingSamples = [
      "\uE000",
      "\uF8FF",
      "\u{F0001}",
      "\u{10FFFD}",
      "\u0378",
      "\uFFFD",
      "\uFFFF",
    ];
    for (const sample of missingSamples) {
      const res = resolveGlyphFallback(sample);
      assert.equal(res.panicked, false, `Глиф ${sample.codePointAt(0)?.toString(16)} не должен вызывать панику`);
      assert.equal(res.isTofurion, true, `Глиф ${sample.codePointAt(0)?.toString(16)} должен заменяться на tofurion`);
      assert.equal(res.resolvedFont, "Tofurion Fallback");
      assert.equal(res.glyphId, 0);
      assert.equal(res.replacementChar, "\uFFFD");
      assert.equal(res.atlasFormat, "r8_unorm");
      assert.ok(res.tofurionBox.widthPx >= 8 && res.tofurionBox.heightPx >= 10);
      assert.equal(res.tofurionBox.strokePx, 1);
    }

    // 2. Отказ всех шрифтов в цепочке (например, пустой список доступных шрифтов) -> безопасный откат на tofurion без паники
    const starvedFallback = resolveGlyphFallback("Ж", { availableFonts: [] });
    assert.equal(starvedFallback.panicked, false);
    assert.equal(starvedFallback.isTofurion, true);
    assert.equal(starvedFallback.resolvedFont, "Tofurion Fallback");
    assert.equal(starvedFallback.glyphId, 0);

    // 3. Валидация пользовательских/системных TTF/OTF/TTC блобов перед передачей растеризатору (защита от паник парсера)
    const emptyCheck = validateCustomFontBytes("EmptyFont", new Uint8Array(0));
    assert.equal(emptyCheck.valid, false);
    assert.match(emptyCheck.reason, /size|empty|32 MiB/i);

    const corruptMagic = validateCustomFontBytes("CorruptFont", new TextEncoder().encode("not a font file"));
    assert.equal(corruptMagic.valid, false);
    assert.match(corruptMagic.reason, /TTF|OTF|TTC|sfnt/i);

    const controlCharName = validateCustomFontBytes("Bad\u0007Name", new Uint8Array([0x00, 0x01, 0x00, 0x00, 0, 1, 0, 16, 0, 0, 0, 0]));
    assert.equal(controlCharName.valid, false);
    assert.match(controlCharName.reason, /name/i);

    const validSfntHeader = new Uint8Array(32);
    // TrueType sfnt version 0x00010000, numTables = 4, unitsPerEm non-zero marker
    validSfntHeader.set([0x00, 0x01, 0x00, 0x00, 0x00, 0x04, 0x00, 0x40, 0x00, 0x02, 0x00, 0x00], 0);
    const validCheck = validateCustomFontBytes("Inter Regular", validSfntHeader);
    assert.equal(validCheck.valid, true);
  });

  it("SUB-981-4: симулирует кэш текстурного атласа WGPU (Shelf bin-packing, переходы состояний emoji-picker и LRU-вытеснение без паники)", async () => {
    const {
      WgpuGlyphAtlasSimulator,
      resolveGlyphFallback,
    } = await import("../../scripts/font-audit.mjs");

    const atlas = new WgpuGlyphAtlasSimulator({
      pageWidthPx: 256,
      pageHeightPx: 256,
      maxPagesPerFormat: 2,
      glyphPaddingPx: 1,
    });

    assert.equal(atlas.getInteractiveState(), "rendered");

    // 1. Вставка цветного эмодзи, CJK, RTL и tofurion-глифа в атлас
    const samples = ["🔥", "🦀", "骨", "مَ", "\uE000"];
    for (const cluster of samples) {
      const resolved = resolveGlyphFallback(cluster, { fontSizePx: 16, fontScale: 1.0 });
      const allocFirst = atlas.cacheGlyph(resolved);
      assert.equal(allocFirst.panicked, false);
      assert.equal(allocFirst.fromCache, false);
      assert.equal(allocFirst.stateTransition, "lookup glyph -> atlas cached");
      assert.ok(allocFirst.uvRect.widthPx > 0 && allocFirst.uvRect.heightPx > 0);

      // Повторное обращение обязано попадать в кэш атласа без новой аллокации
      const allocSecond = atlas.cacheGlyph(resolved);
      assert.equal(allocSecond.panicked, false);
      assert.equal(allocSecond.fromCache, true);
      assert.equal(allocSecond.stateTransition, "atlas cached");
      assert.deepEqual(allocSecond.uvRect, allocFirst.uvRect);
    }

    assert.equal(atlas.getInteractiveState(), "atlas cached");
    const midStats = atlas.getStats();
    assert.equal(midStats.panicked, false);
    assert.equal(midStats.cacheMisses, 5);
    assert.equal(midStats.cacheHits, 5);
    assert.equal(midStats.hitRate, 0.5);

    // 2. Защита от паник WGPU при экстремальных/некорректных размерах растра (0x0, отрицательные, NaN, > 8192px)
    const baseGlyph = resolveGlyphFallback("A");
    const zeroDimAlloc = atlas.cacheGlyph({ ...baseGlyph, cluster: "zero", rasterWidthPx: 0, rasterHeightPx: -5 });
    assert.equal(zeroDimAlloc.panicked, false);
    assert.equal(zeroDimAlloc.clampedToTofurion, true);
    assert.ok(zeroDimAlloc.uvRect.widthPx >= 8 && zeroDimAlloc.uvRect.heightPx >= 8);

    const hugeDimAlloc = atlas.cacheGlyph({ ...baseGlyph, cluster: "huge", rasterWidthPx: 16384, rasterHeightPx: 16384 });
    assert.equal(hugeDimAlloc.panicked, false);
    assert.equal(hugeDimAlloc.clampedToSafeBounds, true);
    assert.ok(hugeDimAlloc.uvRect.widthPx <= 254 && hugeDimAlloc.uvRect.heightPx <= 254);

    // 3. Стресс-тест переполнения атласа (вставка 450 уникальных CJK-иероглифов в компактный атлас 256x256x2 страницы)
    //    Обязан сработать детерминированный LRU-eviction вместо паники wgpu::Error::OutOfMemory
    for (let cp = 0x4e00; cp < 0x4e00 + 450; cp += 1) {
      const char = String.fromCodePoint(cp);
      const cjkGlyph = resolveGlyphFallback(char, { fontSizePx: 24, fontScale: 1.25 });
      const res = atlas.cacheGlyph(cjkGlyph);
      assert.equal(res.panicked, false, `Атлас WGPU не должен паниковать на иероглифе U+${cp.toString(16)}`);
    }

    const stressStats = atlas.getStats();
    assert.equal(stressStats.panicked, false);
    assert.equal(stressStats.atlasPanicCount, 0);
    assert.ok(stressStats.lruEvictions > 0, "При заполнении всех страниц атласа должно сработать LRU-вытеснение");
    assert.ok(stressStats.maskPagesUsed <= 2);
    assert.ok(stressStats.colorPagesUsed <= 2);
  });

  it("SUB-981-5: проверяет матричный аудит, 9 измерений TEST-POLICY.md, CLI scripts/font-audit.mjs (коды 0/1/2), лимиты кода и 5/5 мутаций", async () => {
    const {
      runFontFallbackMatrixAudit,
      evaluateFontAuditEdgeDimensions,
      redactFontAuditLogs,
      generateFontAuditMutations,
      runFontAuditMutationEvaluation,
    } = await import("../../scripts/font-audit.mjs");

    // 1. Полный прогон матрицы подстановки шрифтов (Latin, Cyrillic, Emoji, CJK, RTL Arabic/Hebrew, Tofurion)
    const audit = runFontFallbackMatrixAudit();
    assert.equal(audit.passed, true, `Ошибки матричного аудита шрифтов: ${audit.reasons.join("; ")}`);
    assert.equal(audit.panicked, false);
    assert.equal(audit.atlasStats.atlasPanicCount, 0);
    assert.ok(audit.summary.emojiResolvedCount >= 8);
    assert.ok(audit.summary.cjkResolvedCount >= 12);
    assert.ok(audit.summary.rtlResolvedCount >= 8);
    assert.ok(audit.summary.tofurionSubstitutedCount >= 4);
    assert.equal(audit.summary.unhandledMissingCount, 0);

    // 2. Проверка 9 измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 2000 символов, offset=9975,
    //    fontScale 0.75..1.5, бейдж 99+, ошибки 401/403/404/413/429/503, пустые строки, HTTP 202 async CJK,
    //    права USE_EXTERNAL_EMOJIS / AGE_GATED_SPACES, восстановление после WGPU DeviceLost)
    const edgeReport = evaluateFontAuditEdgeDimensions();
    assert.equal(edgeReport.passed, true, `Ошибки 9 измерений: ${edgeReport.reasons.join("; ")}`);
    assert.equal(edgeReport.boundaries.minSnowflake, "0");
    assert.equal(edgeReport.boundaries.maxSnowflake, "18446744073709551615");
    assert.equal(edgeReport.boundaries.maxMessageChars, 2_000);
    assert.equal(edgeReport.boundaries.maxSearchOffset, 9_975);
    assert.deepEqual(edgeReport.boundaries.fontScaleBounds, [0.75, 1.5]);
    assert.equal(edgeReport.boundaries.badgeCapFormatted, "99+");
    assert.equal(edgeReport.emptyInput.clustersCount, 0);
    assert.equal(edgeReport.emptyInput.panicked, false);
    assert.equal(edgeReport.loadingState.http202AsyncCjkDecodeOffThread, true);
    assert.equal(edgeReport.loadingState.frameBudget120FpsPreserved, true);
    assert.equal(edgeReport.permissions.externalEmojiWithoutPermBlocked, true);
    assert.equal(edgeReport.permissions.nitroLockedEmojiClass, "lockedEmoji_d982c2");
    assert.equal(edgeReport.permissions.ageGatedUnverifiedBlocked, true);
    assert.equal(edgeReport.errorRecovery.statusCodesHandled.length, 6);
    assert.equal(edgeReport.deviceLostRecovery.rebuiltAtlasWithoutPanic, true);

    // 3. Маскирование секретных токенов в логах наблюдаемости
    const rawLog =
      'Font audit telemetry token="mfa.abCDef1234567890_secretToken" auth="Bot dGVzdG1vY2t0b2tlbnZhbHVl.GhIjKl.MnOpQrStUvWxYz123456"';
    const sanitized = redactFontAuditLogs(rawLog);
    assert.ok(!sanitized.includes("abCDef1234567890_secretToken"));
    assert.ok(!sanitized.includes("MnOpQrStUvWxYz123456"));
    assert.match(sanitized, /mfa\.\*\*\*/);
    assert.match(sanitized, /Bot \*\*\*/);

    // 4. Интеграционный запуск CLI scripts/font-audit.mjs (коды выхода 0, 1, 2 и генерация JSON/MD отчётов)
    const tmp = mkdtempSync(join(tmpdir(), "font-audit-981-"));
    try {
      const outJson = join(tmp, "font-audit-981.json");
      const outMd = join(tmp, "FONT-AUDIT-REPORT.md");
      const okRun = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--out-json", outJson, "--out-md", outMd],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(okRun.status, 0, `CLI завершился с ошибкой: ${okRun.stderr}`);
      assert.ok(existsSync(outJson), "JSON-отчёт должен быть создан");
      assert.ok(existsSync(outMd), "Markdown-отчёт должен быть создан");

      const report = JSON.parse(readFileSync(outJson, "utf8"));
      assert.equal(report.taskId, "TASK-981");
      assert.equal(report.status, "passed");
      assert.equal(report.oracleParity.passed, true);
      assert.equal(report.matrixAudit.passed, true);
      assert.equal(report.matrixAudit.panicked, false);
      assert.equal(report.edgeDimensions.passed, true);

      const mdText = readFileSync(outMd, "utf8");
      assert.match(mdText, /TASK-981/);
      assert.match(mdText, /Noto Color Emoji/);
      assert.match(mdText, /Tofurion Fallback/);

      // Фильтрация по категории скрипта (--script emoji, --script cjk, --script rtl)
      for (const scriptFilter of ["emoji", "cjk", "rtl"]) {
        const scriptRun = spawnSync(
          process.execPath,
          [SCRIPT_PATH, "--script", scriptFilter],
          { cwd: ROOT, encoding: "utf8" },
        );
        assert.equal(scriptRun.status, 0, `stderr (${scriptFilter}): ${scriptRun.stderr}`);
      }

      // Код выхода 1 при неизвестном флаге или невалидном скрипте
      const badFlag = spawnSync(process.execPath, [SCRIPT_PATH, "--unknown-flag"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badFlag.status, 1);

      const badScript = spawnSync(process.execPath, [SCRIPT_PATH, "--script", "klingon-invalid"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badScript.status, 1);

      // Код выхода 2 при отсутствии явно указанного файла оракула
      const missingOracle = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--oracle", join(tmp, "missing-kanban-board.json")],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(missingOracle.status, 2);

      // Код выхода 1 при нарушении контракта в переданном файле оракула
      const brokenOracle = join(tmp, "broken-kanban-board.json");
      writeFileSync(brokenOracle, JSON.stringify({ cards: [] }), "utf8");
      const brokenRun = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--oracle", brokenOracle],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(brokenRun.status, 1);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }

    // 5. Проверка соблюдения лимитов размера кода (< 800 строк на файл, < 50 строк на функцию)
    const scriptSource = readFileSync(SCRIPT_PATH, "utf8");
    const sizeCheck = checkRustSourceSizeLimits("scripts/font-audit.mjs", scriptSource);
    assert.equal(sizeCheck.passed, true, sizeCheck.reason);
    assert.ok(sizeCheck.fileLines < 800, `scripts/font-audit.mjs должен быть < 800 строк (сейчас ${sizeCheck.fileLines})`);

    // 6. Мутационный гейт 5/5 (100% kill rate)
    const mutants = generateFontAuditMutations();
    assert.equal(mutants.length, 5);
    const mutationGate = evaluateMutationGate(mutants, (mutant) =>
      runFontAuditMutationEvaluation(mutant),
    );
    assert.equal(
      mutationGate.passed,
      true,
      `Выжившие мутанты: ${mutationGate.survivors.join(", ")}`,
    );
    assert.equal(mutationGate.killed, 5);
    assert.equal(mutationGate.survived, 0);
    assert.equal(mutationGate.killRate, 1);
  });
});
