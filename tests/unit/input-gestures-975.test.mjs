// @ts-check
/**
 * Контрактные и модульные тесты валидатора сенсорных жестов и кинетической прокрутки
 * под Wayland и X11 XInput2 (TASK-975):
 * - SUB-975-1: Протоколы ввода Wayland (`zwp_pointer_gestures_v1`, `wl_pointer` `AXIS_SOURCE_FINGER`,
 *   24.8 fixed-point `wl_fixed_t`) и X11 XInput2 v2.4 (`XI_TouchBegin=18`, `XI_TouchUpdate=19`,
 *   `XI_TouchEnd=20`, `XI_GesturePinchBegin=27`, `XI_GesturePinchUpdate=28`, `XI_GesturePinchEnd=29`,
 *   `XIScrollClassInfo`), автоопределение бэкенда и сверка с независимыми оракулами
 *   (`docs/re/frontend-d03.json`, `rust/discord-gpui/src/state/mod.rs`, `rust/discord-gpui/src/ui/chat_view.rs`,
 *   `waves/kanban-board.json`).
 * - SUB-975-2: Эмуляция и верификация `pinch-to-zoom` без разрывов масштаба (непрерывность кривой,
 *   сохранение фокусной точки под пальцами `anchorDriftPx < 0.01`, зажим `zoomScale` в `0.50..2.00`
 *   по `DiscordNative.window.setZoomFactor` `#5639` и `gpuiFontScale` в `0.75..1.50`, откат при `cancelled=1`).
 * - SUB-975-3: Эмуляция и проверка плавной интерполяции 2-пальцевой прокрутки (`2-finger scroll`)
 *   под Wayland и X11 XInput2 (центроид двух касаний, фильтрация `XIPointerEmulated`, субпиксельная
 *   интерполяция 120 FPS без ступенчатых скачков, бюджет кадра `<= 8.33 мс`).
 * - SUB-975-4: Кинетическая инерция (`kinetic scroll`) в виртуальном списке на 100 000 сообщений
 *   (экспоненциальное затухание скорости после `axis_stop`/`XI_TouchEnd`, строго монотонное торможение,
 *   мгновенная остановка при новом касании, триггер пагинации при `offset < 10.0` с сохранением якоря `+50`).
 * - SUB-975-5: Покрытие всех 9 измерений `TEST-POLICY.md` (позитивные, негативные, граничные Snowflake
 *   `0`/`u64::MAX`, 2000 символов, `offset=9975`, ошибки REST `401/403/404/413/429/5xx`, пустые каналы,
 *   `skeleton`/`HTTP 202`, права `VIEW_CHANNEL`/`AGE_GATED_SPACES`, fallback `Wayland -> XInput2`,
 *   маскирование токенов), CLI `scripts/verify-gestures.mjs` (коды `0/1/2`) и мутационный гейт `5/5`.
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
const SCRIPT_PATH = join(ROOT, "scripts", "verify-gestures.mjs");
const FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const GPUI_STATE_PATH = join(ROOT, "rust", "discord-gpui", "src", "state", "mod.rs");
const GPUI_CHAT_VIEW_PATH = join(ROOT, "rust", "discord-gpui", "src", "ui", "chat_view.rs");
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");

describe("TASK-975: Верификация сенсорных жестов и кинетической прокрутки Wayland/XInput2 (scripts/verify-gestures.mjs)", () => {
  it("SUB-975-1: проверяет константы протоколов Wayland / X11 XInput2, 24.8 wl_fixed_t и сверку с RE-оракулами", async () => {
    const {
      GESTURE_DEFAULT_CONFIG,
      GESTURE_DESIGN_TOKENS,
      TOUCH_SURFACE_STATES,
      WAYLAND_GESTURE_CONSTANTS,
      XINPUT2_CONSTANTS,
      wlFixedFromDouble,
      wlFixedToDouble,
      detectGestureBackend,
      verifyGestureOracleParity,
    } = await import("../../scripts/verify-gestures.mjs");

    // 1. Независимый оракул: сверка дизайн-токенов и состояний с карточкой TASK-975 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card975 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-975");
    assert.ok(card975, "Карточка TASK-975 должна присутствовать в waves/kanban-board.json");
    assert.deepEqual(GESTURE_DESIGN_TOKENS, card975.design_tokens);
    assert.deepEqual(TOUCH_SURFACE_STATES, card975.interactive_states[0]);

    // 2. Проверка констант протокола Wayland (pointer-gestures-unstable-v1 & wl_pointer)
    assert.equal(WAYLAND_GESTURE_CONSTANTS.INTERFACE_POINTER_GESTURES, "zwp_pointer_gestures_v1");
    assert.equal(WAYLAND_GESTURE_CONSTANTS.INTERFACE_PINCH_GESTURE, "zwp_pointer_gesture_pinch_v1");
    assert.equal(WAYLAND_GESTURE_CONSTANTS.WL_POINTER_AXIS_VERTICAL_SCROLL, 0);
    assert.equal(WAYLAND_GESTURE_CONSTANTS.WL_POINTER_AXIS_HORIZONTAL_SCROLL, 1);
    assert.equal(WAYLAND_GESTURE_CONSTANTS.WL_POINTER_AXIS_SOURCE_WHEEL, 0);
    assert.equal(WAYLAND_GESTURE_CONSTANTS.WL_POINTER_AXIS_SOURCE_FINGER, 1);
    assert.equal(WAYLAND_GESTURE_CONSTANTS.WL_POINTER_AXIS_SOURCE_CONTINUOUS, 2);
    assert.equal(WAYLAND_GESTURE_CONSTANTS.WL_FIXED_SCALE, 256);

    // 3. Проверка 24.8 fixed-point преобразований Wayland (wl_fixed_t)
    assert.equal(wlFixedFromDouble(1.0), 256);
    assert.equal(wlFixedFromDouble(1.5), 384);
    assert.equal(wlFixedFromDouble(0.5), 128);
    assert.equal(wlFixedFromDouble(-2.25), -576);
    assert.equal(wlFixedToDouble(256), 1.0);
    assert.equal(wlFixedToDouble(384), 1.5);
    assert.equal(wlFixedToDouble(128), 0.5);
    assert.equal(wlFixedToDouble(-576), -2.25);

    // 4. Проверка констант X11 XInput2 v2.4 (XI2.h)
    assert.equal(XINPUT2_CONSTANTS.XI_MAJOR, 2);
    assert.equal(XINPUT2_CONSTANTS.XI_MINOR, 4);
    assert.equal(XINPUT2_CONSTANTS.XI_Motion, 6);
    assert.equal(XINPUT2_CONSTANTS.XI_TouchBegin, 18);
    assert.equal(XINPUT2_CONSTANTS.XI_TouchUpdate, 19);
    assert.equal(XINPUT2_CONSTANTS.XI_TouchEnd, 20);
    assert.equal(XINPUT2_CONSTANTS.XI_GesturePinchBegin, 27);
    assert.equal(XINPUT2_CONSTANTS.XI_GesturePinchUpdate, 28);
    assert.equal(XINPUT2_CONSTANTS.XI_GesturePinchEnd, 29);
    assert.equal(XINPUT2_CONSTANTS.XIScrollTypeVertical, 1);
    assert.equal(XINPUT2_CONSTANTS.XIScrollTypeHorizontal, 2);
    assert.equal(XINPUT2_CONSTANTS.XIPointerEmulated, 1 << 16);
    assert.equal(XINPUT2_CONSTANTS.XIGesturePinchEventCancelled, 1 << 0);

    // 5. Проверка базовых параметров движка жестов
    assert.equal(GESTURE_DEFAULT_CONFIG.targetFps, 120);
    assert.ok(Math.abs(GESTURE_DEFAULT_CONFIG.frameBudgetMs - 8.333) < 0.01);
    assert.equal(GESTURE_DEFAULT_CONFIG.minZoomScale, 0.5);
    assert.equal(GESTURE_DEFAULT_CONFIG.maxZoomScale, 2.0);
    assert.equal(GESTURE_DEFAULT_CONFIG.minGpuiFontScale, 0.75);
    assert.equal(GESTURE_DEFAULT_CONFIG.maxGpuiFontScale, 1.5);
    assert.equal(GESTURE_DEFAULT_CONFIG.paginationOffsetThresholdPx, 10.0);
    assert.equal(GESTURE_DEFAULT_CONFIG.paginationBatchSize, 50);
    assert.equal(GESTURE_DEFAULT_CONFIG.maxViewportRows, 36);

    // 6. Автоопределение графического бэкенда и fallback цепочка Wayland -> X11 XInput2 -> Headless
    const waylandEnv = detectGestureBackend({
      XDG_SESSION_TYPE: "wayland",
      WAYLAND_DISPLAY: "wayland-0",
      DISPLAY: ":0",
    });
    assert.equal(waylandEnv.backend, "wayland");
    assert.equal(waylandEnv.protocol, "zwp_pointer_gestures_v1+wl_pointer_v8");
    assert.equal(waylandEnv.x11FallbackAvailable, true);

    const x11Env = detectGestureBackend({
      XDG_SESSION_TYPE: "x11",
      WAYLAND_DISPLAY: "",
      DISPLAY: ":0",
    });
    assert.equal(x11Env.backend, "x11-xinput2");
    assert.equal(x11Env.protocol, "XInput2-v2.4");

    const headlessEnv = detectGestureBackend({
      XDG_SESSION_TYPE: "",
      WAYLAND_DISPLAY: "",
      DISPLAY: "",
    });
    assert.equal(headlessEnv.backend, "headless-synthetic");

    // 7. Сверка с реальными файлами оракулов репозитория
    const oracleParity = verifyGestureOracleParity({
      frontendD03Path: FRONTEND_D03_PATH,
      gpuiStatePath: GPUI_STATE_PATH,
      gpuiChatViewPath: GPUI_CHAT_VIEW_PATH,
    });
    assert.equal(oracleParity.passed, true, `Несовпадение с оракулами: ${oracleParity.reasons.join("; ")}`);
    assert.equal(oracleParity.scrollerBaseClass, "scrollerBase_d125d2");
    assert.equal(oracleParity.virtualScrollerClass, "scroller_affa7e");
    assert.deepEqual(oracleParity.gpuiFontScaleBounds, [0.75, 1.5]);
    assert.equal(oracleParity.paginationThresholdPx, 10.0);
    assert.equal(oracleParity.paginationLimit, 50);
  });

  it("SUB-975-2: эмулирует pinch-to-zoom под Wayland и X11 XInput2 и подтверждает изменение масштаба без разрывов и без смещения якоря", async () => {
    const {
      simulatePinchToZoom,
      verifyPinchContinuity,
      computeTwoFingerPinchDistance,
    } = await import("../../scripts/verify-gestures.mjs");

    // 1. Евклидово расстояние между двумя пальцами на сенсорном экране
    const d0 = computeTwoFingerPinchDistance({ x: 400, y: 300 }, { x: 500, y: 300 });
    const d1 = computeTwoFingerPinchDistance({ x: 350, y: 300 }, { x: 550, y: 300 });
    assert.equal(d0, 100);
    assert.equal(d1, 200);
    assert.equal(d1 / d0, 2.0);

    // 2. Позитивный сценарий: плавный zoom-in 1.00 -> 1.50 за 24 кадра (120 FPS) в Wayland
    const waylandZoomIn = simulatePinchToZoom({
      backend: "wayland",
      initialScale: 1.0,
      targetScale: 1.5,
      steps: 24,
      anchorX: 640,
      anchorY: 360,
    });
    assert.equal(waylandZoomIn.backend, "wayland");
    assert.equal(waylandZoomIn.surfaceStateTransitions[0], "idle");
    assert.equal(waylandZoomIn.surfaceStateTransitions[1], "gesture start");
    assert.equal(waylandZoomIn.frames.length, 24);
    assert.ok(Math.abs(waylandZoomIn.finalScale - 1.5) < 1e-6);
    assert.ok(Math.abs(waylandZoomIn.finalGpuiFontScale - 1.5) < 1e-6);
    assert.equal(waylandZoomIn.finalZoomPercent, 150);

    const waylandContinuity = verifyPinchContinuity(waylandZoomIn);
    assert.equal(waylandContinuity.passed, true, waylandContinuity.reasons.join("; "));
    assert.equal(waylandContinuity.discontinuities, 0);
    assert.equal(waylandContinuity.monotonic, true);
    assert.ok(waylandContinuity.maxStepDelta <= 0.05);
    assert.ok(waylandContinuity.maxAnchorDriftPx < 0.01, "Фокусная точка под пальцами не должна смещаться");

    // 3. Позитивный сценарий: плавный zoom-out 1.25 -> 0.85 под X11 XInput2 (XI_GesturePinchBegin/Update/End)
    const x11ZoomOut = simulatePinchToZoom({
      backend: "x11-xinput2",
      initialScale: 1.25,
      targetScale: 0.85,
      steps: 20,
      anchorX: 500,
      anchorY: 400,
    });
    assert.equal(x11ZoomOut.backend, "x11-xinput2");
    assert.equal(x11ZoomOut.frames[0]?.xiEventType, 27); // XI_GesturePinchBegin
    assert.equal(x11ZoomOut.frames[1]?.xiEventType, 28); // XI_GesturePinchUpdate
    assert.equal(x11ZoomOut.frames[x11ZoomOut.frames.length - 1]?.xiEventType, 29); // XI_GesturePinchEnd
    assert.ok(Math.abs(x11ZoomOut.finalScale - 0.85) < 1e-6);
    assert.ok(Math.abs(x11ZoomOut.finalGpuiFontScale - 0.85) < 1e-6);

    const x11Continuity = verifyPinchContinuity(x11ZoomOut);
    assert.equal(x11Continuity.passed, true);
    assert.equal(x11Continuity.discontinuities, 0);
    assert.equal(x11Continuity.monotonic, true);

    // 4. Граничные условия: зажим масштаба на границах [0.50, 2.00] и gpuiFontScale на [0.75, 1.50]
    const clampedHigh = simulatePinchToZoom({
      backend: "wayland",
      initialScale: 1.5,
      targetScale: 3.5,
      steps: 24,
    });
    assert.equal(clampedHigh.finalScale, 2.0);
    assert.equal(clampedHigh.finalZoomPercent, 200);
    assert.equal(clampedHigh.finalGpuiFontScale, 1.5);

    const clampedLow = simulatePinchToZoom({
      backend: "x11-xinput2",
      initialScale: 0.9,
      targetScale: 0.2,
      steps: 24,
    });
    assert.equal(clampedLow.finalScale, 0.5);
    assert.equal(clampedLow.finalZoomPercent, 50);
    assert.equal(clampedLow.finalGpuiFontScale, 0.75);

    // 5. Негативный сценарий: искусственный разрыв масштаба (скачок на +0.45 за один кадр) обнаруживается валидатором
    const brokenFrames = waylandZoomIn.frames.map((frame, idx) =>
      idx === 10 ? { ...frame, scale: frame.scale + 0.45, stepDelta: 0.45 } : frame,
    );
    const brokenVerdict = verifyPinchContinuity({
      ...waylandZoomIn,
      frames: brokenFrames,
    });
    assert.equal(brokenVerdict.passed, false);
    assert.ok(brokenVerdict.discontinuities > 0);
    assert.match(brokenVerdict.reasons.join("; "), /разрыв|скачок|монотонн/i);

    // 6. Сценарий восстановления: отмена жеста пинча (cancelled = true) возвращает масштаб к initialScale
    const cancelledPinch = simulatePinchToZoom({
      backend: "wayland",
      initialScale: 1.15,
      targetScale: 1.8,
      steps: 16,
      cancelled: true,
    });
    assert.equal(cancelledPinch.cancelled, true);
    assert.equal(cancelledPinch.finalScale, 1.15);
    assert.equal(cancelledPinch.finalState, "idle");
  });

  it("SUB-975-3: эмулирует 2-пальцевую прокрутку (2-finger scroll) под Wayland и X11 XInput2 с плавной 120 FPS интерполяцией", async () => {
    const {
      simulateTwoFingerScroll,
      verifyTwoFingerScrollInterpolation,
    } = await import("../../scripts/verify-gestures.mjs");

    // 1. Позитивный сценарий: 2-пальцевый скролл в Wayland (WL_POINTER_AXIS_SOURCE_FINGER)
    const waylandScroll = simulateTwoFingerScroll({
      backend: "wayland",
      initialOffsetPx: 1_200,
      maxScrollOffsetPx: 50_000,
      inputDeltasPx: [18, 24, 30, 32, 28, 22, 16, 10],
      substepsPerSample: 2,
    });
    assert.equal(waylandScroll.backend, "wayland");
    assert.equal(waylandScroll.axisSource, 1); // WL_POINTER_AXIS_SOURCE_FINGER
    assert.equal(waylandScroll.axisStopEmitted, true);
    assert.equal(waylandScroll.frames.length, 16);
    assert.ok(Math.abs(waylandScroll.finalOffsetPx - (1_200 + 180)) < 1e-4);

    const waylandScrollCheck = verifyTwoFingerScrollInterpolation(waylandScroll);
    assert.equal(waylandScrollCheck.passed, true, waylandScrollCheck.reasons.join("; "));
    assert.equal(waylandScrollCheck.jankFrames, 0);
    assert.equal(waylandScrollCheck.monotonic, true);
    assert.ok(waylandScrollCheck.maxFrameDurationMs <= 8.333);

    // 2. Позитивный сценарий: 2-пальцевый скролл в X11 XInput2 с отсевом эмулированных кнопок XIPointerEmulated
    const x11Scroll = simulateTwoFingerScroll({
      backend: "x11-xinput2",
      initialOffsetPx: 500,
      maxScrollOffsetPx: 50_000,
      inputDeltasPx: [-15, -20, -25, -25, -20, -15],
      substepsPerSample: 2,
      includeLegacyPointerEmulatedNoise: true,
    });
    assert.equal(x11Scroll.backend, "x11-xinput2");
    assert.ok(x11Scroll.ignoredPointerEmulatedEvents > 0, "Шум XIPointerEmulated (кнопки 4/5) должен отфильтровываться");
    assert.ok(Math.abs(x11Scroll.finalOffsetPx - (500 - 120)) < 1e-4);

    const x11ScrollCheck = verifyTwoFingerScrollInterpolation(x11Scroll);
    assert.equal(x11ScrollCheck.passed, true, x11ScrollCheck.reasons.join("; "));
    assert.equal(x11ScrollCheck.jankFrames, 0);

    // 3. Негативный сценарий: резкий скачок кадра (jank / пропуск интерполяции > лимита) отклоняется
    const jankyScroll = {
      ...waylandScroll,
      frames: waylandScroll.frames.map((f, idx) =>
        idx === 4 ? { ...f, interpolatedDeltaPx: 320, frameDurationMs: 24.5 } : f,
      ),
    };
    const jankCheck = verifyTwoFingerScrollInterpolation(jankyScroll);
    assert.equal(jankCheck.passed, false);
    assert.ok(jankCheck.jankFrames > 0);
  });

  it("SUB-975-4: проверяет кинетическую инерцию (kinetic scroll) в ленте на 100 000 сообщений, торможение и сохранение якоря пагинации (< 10px -> +50 сообщений)", async () => {
    const {
      simulateKineticScroll,
      verifyKineticScrollPhysics,
    } = await import("../../scripts/verify-gestures.mjs");

    // 1. Кинетическая прокрутка вниз после отрыва пальцев (axis_stop / XI_TouchEnd)
    const kineticDown = simulateKineticScroll({
      initialOffsetPx: 2_000,
      maxScrollOffsetPx: 4_800_000, // 100 000 сообщений * 48px
      releaseVelocityPxPerSec: 1_800,
      timeConstantMs: 325,
      stopVelocityPxPerSec: 15,
      rowHeightPx: 48,
      totalMessages: 100_000,
    });
    assert.equal(kineticDown.surfaceState, "kinetic scroll");
    assert.equal(kineticDown.finalState, "idle");
    assert.ok(kineticDown.frames.length >= 20, "Инерция должна плавно затухать в течение нескольких десятков кадров 120 FPS");
    assert.ok(kineticDown.totalDistancePx > 400);
    assert.ok(kineticDown.finalOffsetPx > 2_400);

    const physicsCheck = verifyKineticScrollPhysics(kineticDown);
    assert.equal(physicsCheck.passed, true, physicsCheck.reasons.join("; "));
    assert.equal(physicsCheck.monotonicDeceleration, true);
    assert.equal(physicsCheck.overshootFrames, 0);
    assert.ok(physicsCheck.maxRenderedRowsPerFrame <= 36);

    // 2. Мгновенная остановка кинетической прокрутки при новом касании экрана (interruptAtFrame)
    const interruptedKinetic = simulateKineticScroll({
      initialOffsetPx: 2_000,
      maxScrollOffsetPx: 4_800_000,
      releaseVelocityPxPerSec: 2_400,
      interruptAtFrame: 8,
    });
    assert.equal(interruptedKinetic.interruptedByTouch, true);
    assert.equal(interruptedKinetic.frames.length, 8);
    assert.equal(interruptedKinetic.finalVelocityPxPerSec, 0);
    assert.equal(interruptedKinetic.finalState, "gesture start");

    // 3. Кинетический скролл вверх к верхнему краю (offset < 10.0) -> пагинация +50 сообщений и сохранение якоря
    const kineticPagination = simulateKineticScroll({
      initialOffsetPx: 140,
      maxScrollOffsetPx: 240_000,
      releaseVelocityPxPerSec: -1_600,
      rowHeightPx: 48,
      totalMessages: 5_000,
      initialTopItemIndex: 3,
      paginationEnabled: true,
    });
    assert.equal(kineticPagination.paginationTriggered, true, "При достижении offset < 10px должна сработать пагинация");
    assert.equal(kineticPagination.prependedMessagesCount, 50);
    assert.equal(kineticPagination.scrollAnchorPreserved, true, "Визуальный якорь скролла должен сохраняться после вставки 50 сообщений");
    assert.equal(
      kineticPagination.anchorItemIndexAfterPrepended,
      kineticPagination.anchorItemIndexBeforePrepended + 50,
    );
  });

  it("SUB-975-5: покрывает все 9 измерений TEST-POLICY.md, CLI scripts/verify-gestures.mjs (коды 0/1/2) и мутационный гейт 5/5", async () => {
    const {
      evaluateGestureEdgeDimensions,
      redactGestureLogs,
      generateGestureVerifierMutations,
      runGestureMutationEvaluation,
    } = await import("../../scripts/verify-gestures.mjs");

    // 1. Проверка 9 измерений (граничные Snowflake 0/u64::MAX, 2000 символов, offset=9975,
    //    ошибки REST 401/403/404/413/429/500, пустой канал, HTTP 202 skeleton,
    //    права VIEW_CHANNEL / AGE_GATED_SPACES, fallback Wayland -> XInput2)
    const edgeReport = evaluateGestureEdgeDimensions();
    assert.equal(edgeReport.passed, true, `Ошибки 9 измерений: ${edgeReport.reasons.join("; ")}`);
    assert.equal(edgeReport.boundaries.minSnowflake, "0");
    assert.equal(edgeReport.boundaries.maxSnowflake, "18446744073709551615");
    assert.equal(edgeReport.boundaries.maxMessageChars, 2_000);
    assert.equal(edgeReport.boundaries.maxSearchOffset, 9_975);
    assert.equal(edgeReport.boundaries.paginationBelow10Triggered, true);
    assert.equal(edgeReport.boundaries.paginationAt10Triggered, false);
    assert.equal(edgeReport.emptyChannel.maxScrollOffsetPx, 0);
    assert.equal(edgeReport.emptyChannel.finalOffsetPx, 0);
    assert.equal(edgeReport.loadingState.http202SkeletonVisible, true);
    assert.equal(edgeReport.loadingState.duplicatePaginationBlocked, true);
    assert.equal(edgeReport.permissions.missingViewChannelBlocked, true);
    assert.equal(edgeReport.permissions.ageGatedUnverifiedBlocked, true);
    assert.equal(edgeReport.errorRecovery.statusCodesHandled.length, 6); // 401, 403, 404, 413, 429, 503
    assert.equal(edgeReport.compositorFallback.switchedToXInput2, true);
    assert.equal(edgeReport.compositorFallback.statePreserved, true);

    // 2. Маскирование секретных токенов в логах наблюдаемости
    const rawLog =
      'Gesture telemetry user token="mfa.abCDef1234567890_secretToken" auth="Bot dGVzdG1vY2t0b2tlbnZhbHVl.GhIjKl.MnOpQrStUvWxYz123456"';
    const sanitized = redactGestureLogs(rawLog);
    assert.ok(!sanitized.includes("abCDef1234567890_secretToken"));
    assert.ok(!sanitized.includes("MnOpQrStUvWxYz123456"));
    assert.match(sanitized, /mfa\.\*\*\*/);
    assert.match(sanitized, /Bot \*\*\*/);

    // 3. Проверка CLI scripts/verify-gestures.mjs (коды выхода 0, 1, 2 и генерация JSON-отчёта)
    const tmp = mkdtempSync(join(tmpdir(), "verify-gestures-975-"));
    try {
      const outJson = join(tmp, "gestures-report.json");
      const okRun = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--out-json", outJson],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(okRun.status, 0, `CLI завершился с ошибкой: ${okRun.stderr}`);
      assert.ok(existsSync(outJson), "JSON-отчёт верификации жестов должен быть создан");

      const report = JSON.parse(readFileSync(outJson, "utf8"));
      assert.equal(report.taskId, "TASK-975");
      assert.equal(report.status, "passed");
      assert.equal(report.oracleParity.passed, true);
      assert.equal(report.pinchToZoom.wayland.passed, true);
      assert.equal(report.pinchToZoom.xinput2.passed, true);
      assert.equal(report.twoFingerScroll.wayland.passed, true);
      assert.equal(report.twoFingerScroll.xinput2.passed, true);
      assert.equal(report.kineticScroll.passed, true);
      assert.equal(report.edgeDimensions.passed, true);

      // Фильтрация по конкретному бэкенду (--backend wayland и --backend x11-xinput2)
      for (const backend of ["wayland", "x11-xinput2"]) {
        const backendRun = spawnSync(
          process.execPath,
          [SCRIPT_PATH, "--backend", backend],
          { cwd: ROOT, encoding: "utf8" },
        );
        assert.equal(backendRun.status, 0, `stderr (${backend}): ${backendRun.stderr}`);
      }

      // Код выхода 1 при неизвестном флаге или невалидном бэкенде
      const badFlag = spawnSync(process.execPath, [SCRIPT_PATH, "--unknown-flag"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badFlag.status, 1);

      const badBackend = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--backend", "invalid-display-server"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(badBackend.status, 1);

      // Код выхода 2 при отсутствии явно запрошенного файла оракула
      const missingOracle = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--oracle", join(tmp, "nonexistent-frontend-d03.json")],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(missingOracle.status, 2);

      // Код выхода 1 при нарушении контракта в переданном файле оракула
      const brokenOracle = join(tmp, "broken-frontend-d03.json");
      writeFileSync(brokenOracle, JSON.stringify({ components: [] }), "utf8");
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
    const sizeCheck = checkRustSourceSizeLimits("scripts/verify-gestures.mjs", scriptSource);
    assert.equal(sizeCheck.passed, true, sizeCheck.reason);
    assert.ok(sizeCheck.fileLines < 800, `scripts/verify-gestures.mjs должен быть < 800 строк (сейчас ${sizeCheck.fileLines})`);

    // 5. Мутационный гейт 5/5 (100% kill rate)
    const mutants = generateGestureVerifierMutations();
    assert.equal(mutants.length, 5);
    const mutationGate = evaluateMutationGate(mutants, (mutant) =>
      runGestureMutationEvaluation(mutant),
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
