// @ts-check
/**
 * Контрактные и модульные тесты валидатора дробного масштабирования Wayland
 * (`wp_fractional_scale_v1` и `wp_viewporter` на 125%, 150%, 175% без размытия, TASK-977):
 * - SUB-977-1: Протоколы `wp_fractional_scale_v1` (делитель `120`, событие `preferred_scale` для
 *   100%=120, 125%=150, 150%=180, 175%=210, 200%=240) и `wp_viewporter` (`wp_viewport.set_source`
 *   в 24.8 `wl_fixed_t`, `wp_viewport.set_destination`, ошибки `BAD_VALUE=0`, `BAD_SIZE=1`,
 *   `OUT_OF_BUFFER=2`, `NO_SURFACE=3`), автоопределение композитора и сверка с независимыми оракулами
 *   (`waves/kanban-board.json`, `rust/chord-gpui/src/manual_qa/desktop_matrix.rs`,
 *   `rust/chord-gpui/src/state/mod.rs`, `rust/chord-gpui/src/visual_regression/ssim.rs`).
 * - SUB-977-2: Расчёт геометрии `wp_viewporter` + `wp_fractional_scale_v1` для 125%, 150%, 175%,
 *   выравнивание физических пикселей панелей (`serverRail=72`, `channelSidebar=240`, `memberSidebar=240`,
 *   `userPanel=52`) без 1px-щелей и проверка ошибок протокола `wp_viewport`.
 * - SUB-977-3: Валидация отсутствия размытия и артефактов билинейного апскейла/даунскейла на 125%,
 *   150% и 175% (Direct Physical Render 1:1 против билинейной фильтрации `wl_output.scale=1` и `buffer_scale=2`,
 *   `blurredEdgePixels === 0`, `sharpnessScore === 1.0`, `ssimScore >= 0.995`).
 * - SUB-977-4: Горячее переключение `preferred_scale` при переносе окна между мониторами
 *   (`100% -> 125% -> 150% -> 175% -> 100%`) в рамках бюджета 120 FPS (`<= 8.33 мс`) и состояния `scale-slider`.
 * - SUB-977-5: Покрытие всех 9 измерений `TEST-POLICY.md`, CLI `scripts/verify-fractional-scale.mjs`
 *   (коды выхода `0/1/2`), проверка лимитов размера кода и мутационный гейт `5/5` (`100%` kill rate).
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
const SCRIPT_PATH = join(ROOT, "scripts", "verify-fractional-scale.mjs");
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const DESKTOP_MATRIX_PATH = join(ROOT, "rust", "chord-gpui", "src", "manual_qa", "desktop_matrix.rs");
const GPUI_STATE_PATH = join(ROOT, "rust", "chord-gpui", "src", "state", "mod.rs");
const GPUI_SSIM_PATH = join(ROOT, "rust", "chord-gpui", "src", "visual_regression", "ssim.rs");

describe("TASK-977: Верификация дробного масштабирования Wayland wp_fractional_scale_v1 и wp_viewporter (scripts/verify-fractional-scale.mjs)", () => {
  it("SUB-977-1: проверяет константы протоколов wp_fractional_scale_v1 (делитель 120) и wp_viewporter, 24.8 wl_fixed_t и сверку с независимыми оракулами", async () => {
    const {
      FRACTIONAL_SCALE_DESIGN_TOKENS,
      SCALE_SLIDER_STATES,
      WAYLAND_FRACTIONAL_SCALE_CONSTANTS,
      WP_VIEWPORTER_CONSTANTS,
      FRACTIONAL_SCALE_PRESETS,
      FRACTIONAL_SCALE_DEFAULT_CONFIG,
      wlFixedFromNumber,
      wlFixedToNumber,
      scalePercentToWayland120,
      wayland120ToScaleFactor,
      detectFractionalScaleBackend,
      verifyFractionalScaleOracleParity,
    } = await import("../../scripts/verify-fractional-scale.mjs");

    // 1. Независимый оракул: сверка дизайн-токенов, состояний и критериев с карточкой TASK-977 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card977 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-977");
    assert.ok(card977, "Карточка TASK-977 должна присутствовать в waves/kanban-board.json");
    assert.deepEqual(FRACTIONAL_SCALE_DESIGN_TOKENS, card977.design_tokens);
    assert.deepEqual(SCALE_SLIDER_STATES, card977.interactive_states[0]);

    // 2. Константы протокола wp_fractional_scale_v1 (fractional-scale-v1.xml)
    assert.equal(
      WAYLAND_FRACTIONAL_SCALE_CONSTANTS.INTERFACE_MANAGER,
      "wp_fractional_scale_manager_v1",
    );
    assert.equal(
      WAYLAND_FRACTIONAL_SCALE_CONSTANTS.INTERFACE_SURFACE_SCALE,
      "wp_fractional_scale_v1",
    );
    assert.equal(WAYLAND_FRACTIONAL_SCALE_CONSTANTS.PROTOCOL_VERSION, 1);
    assert.equal(WAYLAND_FRACTIONAL_SCALE_CONSTANTS.SCALE_DENOMINATOR, 120);
    assert.equal(WAYLAND_FRACTIONAL_SCALE_CONSTANTS.OPCODE_DESTROY, 0);
    assert.equal(WAYLAND_FRACTIONAL_SCALE_CONSTANTS.OPCODE_GET_FRACTIONAL_SCALE, 1);
    assert.equal(WAYLAND_FRACTIONAL_SCALE_CONSTANTS.EVENT_PREFERRED_SCALE, 0);
    assert.equal(WAYLAND_FRACTIONAL_SCALE_CONSTANTS.ERROR_FRACTIONAL_SCALE_EXISTS, 0);

    // 3. Константы протокола wp_viewporter (viewporter.xml)
    assert.equal(WP_VIEWPORTER_CONSTANTS.INTERFACE_VIEWPORTER, "wp_viewporter");
    assert.equal(WP_VIEWPORTER_CONSTANTS.INTERFACE_VIEWPORT, "wp_viewport");
    assert.equal(WP_VIEWPORTER_CONSTANTS.PROTOCOL_VERSION, 1);
    assert.equal(WP_VIEWPORTER_CONSTANTS.OPCODE_DESTROY, 0);
    assert.equal(WP_VIEWPORTER_CONSTANTS.OPCODE_SET_SOURCE, 1);
    assert.equal(WP_VIEWPORTER_CONSTANTS.OPCODE_SET_DESTINATION, 2);
    assert.equal(WP_VIEWPORTER_CONSTANTS.ERROR_BAD_VALUE, 0);
    assert.equal(WP_VIEWPORTER_CONSTANTS.ERROR_BAD_SIZE, 1);
    assert.equal(WP_VIEWPORTER_CONSTANTS.ERROR_OUT_OF_BUFFER, 2);
    assert.equal(WP_VIEWPORTER_CONSTANTS.ERROR_NO_SURFACE, 3);
    assert.equal(WP_VIEWPORTER_CONSTANTS.WL_FIXED_SCALE, 256);

    // 4. Преобразования коэффициентов масштаба с делителем 120 и 24.8 wl_fixed_t
    assert.equal(scalePercentToWayland120(100), 120);
    assert.equal(scalePercentToWayland120(125), 150);
    assert.equal(scalePercentToWayland120(150), 180);
    assert.equal(scalePercentToWayland120(175), 210);
    assert.equal(scalePercentToWayland120(200), 240);

    assert.equal(wayland120ToScaleFactor(120), 1.0);
    assert.equal(wayland120ToScaleFactor(150), 1.25);
    assert.equal(wayland120ToScaleFactor(180), 1.5);
    assert.equal(wayland120ToScaleFactor(210), 1.75);
    assert.equal(wayland120ToScaleFactor(240), 2.0);

    assert.equal(wlFixedFromNumber(1600), 409_600);
    assert.equal(wlFixedFromNumber(1.25), 320);
    assert.equal(wlFixedToNumber(409_600), 1600);
    assert.equal(wlFixedToNumber(320), 1.25);

    // 5. Целевые пресеты дробного масштабирования (125%, 150%, 175%)
    assert.deepEqual(
      FRACTIONAL_SCALE_PRESETS.map((p) => p.percent),
      [125, 150, 175],
    );
    assert.deepEqual(
      FRACTIONAL_SCALE_PRESETS.map((p) => p.preferredScale120),
      [150, 180, 210],
    );
    assert.equal(FRACTIONAL_SCALE_DEFAULT_CONFIG.minWindowWidth, 900);
    assert.equal(FRACTIONAL_SCALE_DEFAULT_CONFIG.minWindowHeight, 600);
    assert.equal(FRACTIONAL_SCALE_DEFAULT_CONFIG.ssimPassThreshold, 0.995);
    assert.equal(FRACTIONAL_SCALE_DEFAULT_CONFIG.ssimMinContractThreshold, 0.99);

    // 6. Автоопределение поддержки wp_fractional_scale_v1 + wp_viewporter по окружению
    const gnomeWayland = detectFractionalScaleBackend({
      XDG_SESSION_TYPE: "wayland",
      XDG_CURRENT_Desktop: "GNOME",
      XDG_CURRENT_DESKTOP: "GNOME",
      WAYLAND_DISPLAY: "wayland-0",
      DISPLAY: ":0",
    });
    assert.equal(gnomeWayland.backend, "wayland-fractional-v1");
    assert.equal(gnomeWayland.supportsFractionalScaleV1, true);
    assert.equal(gnomeWayland.supportsViewporter, true);
    assert.equal(gnomeWayland.compositorFamily, "mutter");

    const kdeWayland = detectFractionalScaleBackend({
      XDG_SESSION_TYPE: "wayland",
      XDG_CURRENT_DESKTOP: "KDE",
      WAYLAND_DISPLAY: "wayland-0",
    });
    assert.equal(kdeWayland.backend, "wayland-fractional-v1");
    assert.equal(kdeWayland.compositorFamily, "kwin");

    const hyprlandWayland = detectFractionalScaleBackend({
      XDG_SESSION_TYPE: "wayland",
      XDG_CURRENT_DESKTOP: "Hyprland:sway",
      WAYLAND_DISPLAY: "wayland-1",
    });
    assert.equal(hyprlandWayland.backend, "wayland-fractional-v1");
    assert.equal(hyprlandWayland.compositorFamily, "wlroots");

    const x11Fallback = detectFractionalScaleBackend({
      XDG_SESSION_TYPE: "x11",
      XDG_CURRENT_DESKTOP: "XFCE:i3",
      WAYLAND_DISPLAY: "",
      DISPLAY: ":0",
    });
    assert.equal(x11Fallback.backend, "x11-randr-xft-fallback");
    assert.equal(x11Fallback.supportsFractionalScaleV1, false);

    const headlessSynthetic = detectFractionalScaleBackend({
      XDG_SESSION_TYPE: "",
      WAYLAND_DISPLAY: "",
      DISPLAY: "",
    });
    assert.equal(headlessSynthetic.backend, "headless-synthetic-wayland");

    // 7. Сверка с реальными файлами оракулов репозитория
    const oracleParity = verifyFractionalScaleOracleParity({
      kanbanBoardPath: KANBAN_BOARD_PATH,
      desktopMatrixPath: DESKTOP_MATRIX_PATH,
      gpuiStatePath: GPUI_STATE_PATH,
      gpuiSsimPath: GPUI_SSIM_PATH,
    });
    assert.equal(oracleParity.passed, true, `Несовпадение с оракулами: ${oracleParity.reasons.join("; ")}`);
    assert.deepEqual(oracleParity.minWindowSize, [900, 600]);
    assert.deepEqual(oracleParity.chromeWidths, {
      serverRail: 72,
      channelSidebar: 240,
      memberSidebar: 240,
      userPanelHeight: 52,
    });
    assert.deepEqual(oracleParity.gpuiFontScaleBounds, [0.75, 1.5]);
    assert.equal(oracleParity.ssimDefaultThreshold, 0.995);
    assert.equal(oracleParity.ssimMinContractThreshold, 0.99);
  });

  it("SUB-977-2: проверяет протокольную связку wp_fractional_scale_v1 + wp_viewporter и выравнивание пиксельной сетки на 125%, 150%, 175%", async () => {
    const {
      negotiateFractionalViewport,
      validateWpViewportCommit,
      WP_VIEWPORTER_CONSTANTS,
    } = await import("../../scripts/verify-fractional-scale.mjs");

    // 1. Проверка расчёта физических буферов и wp_viewport для 1280x720 на 125% (150/120), 150% (180/120), 175% (210/120)
    const expected1280 = [
      { percent: 125, preferredScale120: 150, physW: 1600, physH: 900, railPx: 90, chanPx: 300, memberPx: 300, chatPx: 910, userPanelPx: 65 },
      { percent: 150, preferredScale120: 180, physW: 1920, physH: 1080, railPx: 108, chanPx: 360, memberPx: 360, chatPx: 1092, userPanelPx: 78 },
      { percent: 175, preferredScale120: 210, physW: 2240, physH: 1260, railPx: 126, chanPx: 420, memberPx: 420, chatPx: 1274, userPanelPx: 91 },
    ];

    for (const item of expected1280) {
      const neg = negotiateFractionalViewport({
        logicalWidth: 1280,
        logicalHeight: 720,
        scalePercent: item.percent,
      });
      assert.equal(neg.passed, true, neg.reasons.join("; "));
      assert.equal(neg.preferredScale120, item.preferredScale120);
      assert.equal(neg.wlSurfaceBufferScale, 1, "При wp_viewporter буферный масштаб wl_surface обязан оставаться 1");
      assert.equal(neg.physicalWidth, item.physW);
      assert.equal(neg.physicalHeight, item.physH);
      assert.deepEqual(neg.viewportDestination, { width: 1280, height: 720 });
      assert.deepEqual(neg.viewportSourceFixed, {
        x: 0,
        y: 0,
        width: item.physW * 256,
        height: item.physH * 256,
      });
      assert.equal(neg.physicalLayout.serverRailWidthPx, item.railPx);
      assert.equal(neg.physicalLayout.channelSidebarWidthPx, item.chanPx);
      assert.equal(neg.physicalLayout.memberSidebarWidthPx, item.memberPx);
      assert.equal(neg.physicalLayout.chatViewportWidthPx, item.chatPx);
      assert.equal(neg.physicalLayout.userPanelHeightPx, item.userPanelPx);
      assert.equal(
        neg.physicalLayout.serverRailWidthPx +
          neg.physicalLayout.channelSidebarWidthPx +
          neg.physicalLayout.chatViewportWidthPx +
          neg.physicalLayout.memberSidebarWidthPx,
        neg.physicalWidth,
        `Сумма ширин панелей при ${item.percent}% должна без 1px-щелей равняться физической ширине буфера`,
      );
      assert.equal(neg.physicalLayout.pixelGapCount, 0);
      assert.equal(neg.physicalLayout.hasTextOverlap, false);
      assert.equal(neg.physicalLayout.userPanelAnchoredBottom, true);
    }

    // 2. Проверка FHD 1920x1080 и минимального окна 900x600 на 125%, 150%, 175%
    for (const scalePercent of [125, 150, 175]) {
      const fhd = negotiateFractionalViewport({
        logicalWidth: 1920,
        logicalHeight: 1080,
        scalePercent,
      });
      assert.equal(fhd.passed, true);
      assert.equal(fhd.physicalLayout.pixelGapCount, 0);

      const minWin = negotiateFractionalViewport({
        logicalWidth: 900,
        logicalHeight: 600,
        scalePercent,
      });
      assert.equal(minWin.passed, true);
      assert.equal(minWin.physicalLayout.pixelGapCount, 0);
      assert.equal(minWin.physicalLayout.userPanelAnchoredBottom, true);
    }

    // 3. Негативный сценарий: окно меньше минимального размера 900x600 отклоняется
    const belowMin = negotiateFractionalViewport({
      logicalWidth: 800,
      logicalHeight: 500,
      scalePercent: 125,
    });
    assert.equal(belowMin.passed, false);
    assert.match(belowMin.reasons.join("; "), /900x600|минимальн/i);

    // 4. Проверка протокольных ошибок wp_viewport при коммите wl_surface
    const validCommit = validateWpViewportCommit({
      surfaceAlive: true,
      bufferWidth: 1600,
      bufferHeight: 900,
      sourceX: 0,
      sourceY: 0,
      sourceWidth: 1600,
      sourceHeight: 900,
      destinationWidth: 1280,
      destinationHeight: 720,
    });
    assert.equal(validCommit.valid, true);
    assert.equal(validCommit.errorCode, null);

    const noSurface = validateWpViewportCommit({
      surfaceAlive: false,
      bufferWidth: 1600,
      bufferHeight: 900,
      sourceX: 0,
      sourceY: 0,
      sourceWidth: 1600,
      sourceHeight: 900,
      destinationWidth: 1280,
      destinationHeight: 720,
    });
    assert.equal(noSurface.valid, false);
    assert.equal(noSurface.errorCode, WP_VIEWPORTER_CONSTANTS.ERROR_NO_SURFACE);

    const badValue = validateWpViewportCommit({
      surfaceAlive: true,
      bufferWidth: 1600,
      bufferHeight: 900,
      sourceX: -4,
      sourceY: 0,
      sourceWidth: 1600,
      sourceHeight: 900,
      destinationWidth: 1280,
      destinationHeight: 720,
    });
    assert.equal(badValue.valid, false);
    assert.equal(badValue.errorCode, WP_VIEWPORTER_CONSTANTS.ERROR_BAD_VALUE);

    const badSize = validateWpViewportCommit({
      surfaceAlive: true,
      bufferWidth: 1600,
      bufferHeight: 900,
      sourceX: 0,
      sourceY: 0,
      sourceWidth: 1600,
      sourceHeight: 900,
      destinationWidth: 0,
      destinationHeight: 720,
    });
    assert.equal(badSize.valid, false);
    assert.equal(badSize.errorCode, WP_VIEWPORTER_CONSTANTS.ERROR_BAD_SIZE);

    const outOfBuffer = validateWpViewportCommit({
      surfaceAlive: true,
      bufferWidth: 1600,
      bufferHeight: 900,
      sourceX: 0,
      sourceY: 0,
      sourceWidth: 1620,
      sourceHeight: 900,
      destinationWidth: 1280,
      destinationHeight: 720,
    });
    assert.equal(outOfBuffer.valid, false);
    assert.equal(outOfBuffer.errorCode, WP_VIEWPORTER_CONSTANTS.ERROR_OUT_OF_BUFFER);
  });

  it("SUB-977-3: подтверждает рендеринг 125%, 150%, 175% без размытия и детектирует артефакты билинейного апскейла/даунскейла", async () => {
    const {
      verifyScaleSharpnessAndBlurFree,
      verifyAllFractionalScales,
    } = await import("../../scripts/verify-fractional-scale.mjs");

    // 1. Позитивный сценарий: нативный рендеринг через wp_fractional_scale_v1 + wp_viewporter на 125%, 150%, 175%
    for (const scalePercent of [125, 150, 175]) {
      const sharpReport = verifyScaleSharpnessAndBlurFree({
        scalePercent,
        mode: "native-wp-fractional-scale-v1",
        sampleLogicalWidth: 320,
        sampleLogicalHeight: 180,
      });
      assert.equal(
        sharpReport.passed,
        true,
        `Масштаб ${scalePercent}% должен рендериться без размытия: ${sharpReport.reasons.join("; ")}`,
      );
      assert.equal(sharpReport.scalePercent, scalePercent);
      assert.equal(sharpReport.blurredEdgePixels, 0, `На ${scalePercent}% не должно быть размытых граничных пикселей`);
      assert.equal(sharpReport.bilinearHaloDetected, false);
      assert.equal(sharpReport.sharpnessScore, 1.0);
      assert.ok(
        sharpReport.ssimScore >= 0.995,
        `SSIM на ${scalePercent}% должен быть >= 0.995 (получено ${sharpReport.ssimScore})`,
      );
    }

    // 2. Сводная верификация всех трёх целевых масштабов (125%, 150%, 175%)
    const allScales = verifyAllFractionalScales();
    assert.equal(allScales.passed, true, allScales.reasons.join("; "));
    assert.equal(allScales.results.length, 3);
    assert.deepEqual(
      allScales.results.map((r) => r.scalePercent),
      [125, 150, 175],
    );

    // 3. Негативный сценарий А: композиторный билинейный апскейл от 100% (wl_output.scale=1 без wp_fractional_scale_v1)
    for (const scalePercent of [125, 150, 175]) {
      const blurryUpscale = verifyScaleSharpnessAndBlurFree({
        scalePercent,
        mode: "legacy-bilinear-upscale-from-1x",
        sampleLogicalWidth: 320,
        sampleLogicalHeight: 180,
      });
      assert.equal(
        blurryUpscale.passed,
        false,
        `Билинейный апскейл на ${scalePercent}% обязан отклоняться валидатором резкости`,
      );
      assert.ok(blurryUpscale.blurredEdgePixels > 0, "Должны быть обнаружены промежуточные полутона размытия");
      assert.equal(blurryUpscale.bilinearHaloDetected, true);
      assert.ok(blurryUpscale.sharpnessScore < 1.0);
      assert.ok(blurryUpscale.ssimScore < 0.99, `SSIM при билинейном апскейле должен падать < 0.99 (получено ${blurryUpscale.ssimScore})`);
      assert.match(blurryUpscale.reasons.join("; "), /размыт|билинейн|SSIM/i);
    }

    // 4. Негативный сценарий Б: композиторный даунскейл от 200% (wl_surface.set_buffer_scale(2))
    const blurryDownscale = verifyScaleSharpnessAndBlurFree({
      scalePercent: 150,
      mode: "legacy-bilinear-downscale-from-2x",
      sampleLogicalWidth: 320,
      sampleLogicalHeight: 180,
    });
    assert.equal(blurryDownscale.passed, false);
    assert.ok(blurryDownscale.blurredEdgePixels > 0);
    assert.equal(blurryDownscale.bilinearHaloDetected, true);
  });

  it("SUB-977-4: эмулирует динамическое переключение preferred_scale между мониторами (100% -> 125% -> 150% -> 175% -> 100%) на 120 FPS", async () => {
    const {
      simulateDynamicScaleTransitions,
      SCALE_SLIDER_STATES,
    } = await import("../../scripts/verify-fractional-scale.mjs");

    const transitions = simulateDynamicScaleTransitions({
      logicalWidth: 1280,
      logicalHeight: 720,
      scaleSequencePercent: [100, 125, 150, 175, 100],
      initialScrollOffsetPx: 2_400,
    });

    assert.equal(transitions.passed, true, transitions.reasons.join("; "));
    assert.equal(transitions.steps.length, 5);
    assert.deepEqual(transitions.sliderStateSequence, [
      SCALE_SLIDER_STATES.default,
      SCALE_SLIDER_STATES.hover,
      SCALE_SLIDER_STATES.active,
    ]);
    assert.equal(transitions.logicalGeometryPreserved, true);
    assert.equal(transitions.scrollAnchorPreserved, true);
    assert.equal(transitions.outOfBufferErrors, 0);
    assert.equal(transitions.frameBudgetExceededCount, 0);

    for (const step of transitions.steps) {
      assert.equal(step.logicalWidth, 1280);
      assert.equal(step.logicalHeight, 720);
      assert.equal(step.scrollOffsetLogicalPx, 2_400);
      assert.ok(step.reconfigureDurationMs <= 8.333, "Перестроение swapchain + wp_viewport обязано укладываться в 8.33 мс (120 FPS)");
      assert.equal(step.viewportCommitValid, true);
    }
  });

  it("SUB-977-5: покрывает все 9 измерений TEST-POLICY.md, CLI scripts/verify-fractional-scale.mjs (коды 0/1/2), лимиты размера и мутационный гейт 5/5", async () => {
    const {
      evaluateFractionalScaleEdgeDimensions,
      redactFractionalScaleLogs,
      generateFractionalScaleVerifierMutations,
      runFractionalScaleMutationEvaluation,
    } = await import("../../scripts/verify-fractional-scale.mjs");

    // 1. Проверка всех 9 измерений TEST-POLICY.md
    const edgeReport = evaluateFractionalScaleEdgeDimensions();
    assert.equal(edgeReport.passed, true, `Ошибки 9 измерений: ${edgeReport.reasons.join("; ")}`);
    assert.equal(edgeReport.boundaries.minSnowflake, "0");
    assert.equal(edgeReport.boundaries.maxSnowflake, "18446744073709551615");
    assert.equal(edgeReport.boundaries.maxMessageChars, 2_000);
    assert.equal(edgeReport.boundaries.maxAttachmentsCount, 10);
    assert.equal(edgeReport.boundaries.maxAttachmentSizeBytes, 25 * 1024 * 1024);
    assert.equal(edgeReport.boundaries.maxSearchOffset, 9_975);
    assert.deepEqual(edgeReport.boundaries.voiceVolumeBoundsPercent, [0, 200]);
    assert.equal(edgeReport.boundaries.unreadBadgeCap, "99+");
    assert.equal(edgeReport.emptyState.zeroMonitorsFallbackScalePercent, 100);
    assert.equal(edgeReport.loadingState.http202SkeletonSharpAt150Percent, true);
    assert.equal(edgeReport.permissions.missingViewChannelBlocked, true);
    assert.equal(edgeReport.permissions.ageGatedUnverifiedBlocked, true);
    assert.deepEqual(edgeReport.errorRecovery.statusCodesHandled, [401, 403, 404, 413, 429, 503]);
    assert.equal(edgeReport.compositorRecovery.fallbackOnProtocolUnbind, true);
    assert.equal(edgeReport.compositorRecovery.x11RandrFallbackVerified, true);

    // 2. Маскирование чувствительных токенов в логах наблюдаемости
    const rawLog =
      'Wayland fractional_scale telemetry token="mfa.abCDef1234567890_secretToken" auth="Bot dGVzdG1vY2t0b2tlbnZhbHVl.GhIjKl.MnOpQrStUvWxYz123456" bearer="Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"';
    const sanitized = redactFractionalScaleLogs(rawLog);
    assert.ok(!sanitized.includes("abCDef1234567890_secretToken"));
    assert.ok(!sanitized.includes("MnOpQrStUvWxYz123456"));
    assert.ok(!sanitized.includes("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    assert.match(sanitized, /mfa\.\*\*\*/);
    assert.match(sanitized, /Bot \*\*\*/);
    assert.match(sanitized, /Bearer \*\*\*/);

    // 3. Проверка CLI scripts/verify-fractional-scale.mjs (коды выхода 0, 1, 2 и JSON-отчёт)
    const tmp = mkdtempSync(join(tmpdir(), "verify-fractional-scale-977-"));
    try {
      const outJson = join(tmp, "fractional-scale-report.json");
      const okRun = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--out-json", outJson],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(okRun.status, 0, `CLI завершился с ошибкой: ${okRun.stderr}`);
      assert.ok(existsSync(outJson), "JSON-отчёт верификации дробного масштабирования должен быть создан");

      const report = JSON.parse(readFileSync(outJson, "utf8"));
      assert.equal(report.taskId, "TASK-977");
      assert.equal(report.status, "passed");
      assert.equal(report.oracleParity.passed, true);
      assert.equal(report.scalesVerification.passed, true);
      assert.equal(report.dynamicTransitions.passed, true);
      assert.equal(report.edgeDimensions.passed, true);

      // Фильтрация по конкретному масштабу (--scale 125, 150, 175, all)
      for (const scaleArg of ["125", "150", "175", "all"]) {
        const scaleRun = spawnSync(
          process.execPath,
          [SCRIPT_PATH, "--scale", scaleArg],
          { cwd: ROOT, encoding: "utf8" },
        );
        assert.equal(scaleRun.status, 0, `stderr (--scale ${scaleArg}): ${scaleRun.stderr}`);
      }

      // Код выхода 1 при неизвестном флаге или недопустимом масштабе
      const badFlag = spawnSync(process.execPath, [SCRIPT_PATH, "--unknown-flag"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badFlag.status, 1);

      const badScale = spawnSync(process.execPath, [SCRIPT_PATH, "--scale", "133"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badScale.status, 1);

      // Код выхода 2 при отсутствии явно указанного файла оракула
      const missingOracle = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--oracle", join(tmp, "nonexistent-kanban-board.json")],
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

    // 4. Проверка соблюдения лимитов размера кода (< 800 строк на файл, < 50 строк на функцию)
    const scriptSource = readFileSync(SCRIPT_PATH, "utf8");
    const sizeCheck = checkRustSourceSizeLimits("scripts/verify-fractional-scale.mjs", scriptSource);
    assert.equal(sizeCheck.passed, true, sizeCheck.reason);
    assert.ok(
      sizeCheck.fileLines < 800,
      `scripts/verify-fractional-scale.mjs должен быть < 800 строк (сейчас ${sizeCheck.fileLines})`,
    );

    // 5. Мутационный гейт 5/5 (100% kill rate)
    const mutants = generateFractionalScaleVerifierMutations();
    assert.equal(mutants.length, 5);
    const mutationGate = evaluateMutationGate(mutants, (mutant) =>
      runFractionalScaleMutationEvaluation(mutant),
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
