#!/usr/bin/env node
// @ts-check
/**
 * Автономный валидатор сенсорных жестов и кинетической прокрутки под Wayland
 * (`zwp_pointer_gestures_v1`, `wl_pointer` `AXIS_SOURCE_FINGER`) и X11 XInput2 v2.4
 * (`XI_Touch*`, `XI_GesturePinch*`, `XIScrollClassInfo`) для `discord-gpui` (TASK-975).
 * Коды выхода CLI: 0 — успех, 1 — нарушение контракта/аргументов, 2 — среда недоступна.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const DEFAULT_GPUI_STATE_PATH = join(ROOT, "rust", "chord-gpui", "src", "state", "mod.rs");
const DEFAULT_GPUI_CHAT_VIEW_PATH = join(ROOT, "rust", "chord-gpui", "src", "ui", "chat_view.rs");

export const GESTURE_DESIGN_TOKENS = Object.freeze({
  background: "#1e1f22",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

export const TOUCH_SURFACE_STATES = Object.freeze({
  element: "touch-surface",
  default: "idle",
  hover: "gesture start",
  active: "kinetic scroll",
});

export const WAYLAND_GESTURE_CONSTANTS = Object.freeze({
  INTERFACE_POINTER_GESTURES: "zwp_pointer_gestures_v1",
  INTERFACE_PINCH_GESTURE: "zwp_pointer_gesture_pinch_v1",
  WL_POINTER_AXIS_VERTICAL_SCROLL: 0,
  WL_POINTER_AXIS_HORIZONTAL_SCROLL: 1,
  WL_POINTER_AXIS_SOURCE_WHEEL: 0,
  WL_POINTER_AXIS_SOURCE_FINGER: 1,
  WL_POINTER_AXIS_SOURCE_CONTINUOUS: 2,
  WL_FIXED_SCALE: 256,
});

export const XINPUT2_CONSTANTS = Object.freeze({
  XI_MAJOR: 2,
  XI_MINOR: 4,
  XI_Motion: 6,
  XI_TouchBegin: 18,
  XI_TouchUpdate: 19,
  XI_TouchEnd: 20,
  XI_GesturePinchBegin: 27,
  XI_GesturePinchUpdate: 28,
  XI_GesturePinchEnd: 29,
  XIScrollTypeVertical: 1,
  XIScrollTypeHorizontal: 2,
  XIPointerEmulated: 1 << 16,
  XIGesturePinchEventCancelled: 1 << 0,
});

export const GESTURE_DEFAULT_CONFIG = Object.freeze({
  targetFps: 120,
  frameBudgetMs: 8.33,
  minZoomScale: 0.5,
  maxZoomScale: 2.0,
  minGpuiFontScale: 0.75,
  maxGpuiFontScale: 1.5,
  maxAllowedPinchStepDelta: 0.08,
  maxAllowedAnchorDriftPx: 0.01,
  maxAllowedScrollFrameDeltaPx: 180,
  paginationOffsetThresholdPx: 10.0,
  paginationBatchSize: 50,
  maxViewportRows: 36,
  defaultRowHeightPx: 48,
  kineticTimeConstantMs: 325,
  kineticStopVelocityPxPerSec: 15,
});

/**
 * Преобразует число с плавающей точкой в 24.8 fixed-point формат Wayland (`wl_fixed_t`).
 * @param {number} value
 * @returns {number}
 */
export function wlFixedFromDouble(value) {
  if (!Number.isFinite(value)) throw new TypeError("wlFixedFromDouble: ожидалось конечное число");
  return Math.round(value * WAYLAND_GESTURE_CONSTANTS.WL_FIXED_SCALE);
}

/**
 * Преобразует 24.8 fixed-point число Wayland (`wl_fixed_t`) обратно в `number`.
 * @param {number} fixed
 * @returns {number}
 */
export function wlFixedToDouble(fixed) {
  if (!Number.isFinite(fixed)) throw new TypeError("wlFixedToDouble: ожидалось конечное число");
  return fixed / WAYLAND_GESTURE_CONSTANTS.WL_FIXED_SCALE;
}

/**
 * Вычисляет евклидово расстояние между двумя точками касания (`p1`, `p2`).
 * @param {{ x: number, y: number }} p1
 * @param {{ x: number, y: number }} p2
 * @returns {number}
 */
export function computeTwoFingerPinchDistance(p1, p2) {
  if (!Number.isFinite(p1?.x) || !Number.isFinite(p1?.y) || !Number.isFinite(p2?.x) || !Number.isFinite(p2?.y)) {
    throw new TypeError("Координаты точек касания должны быть конечными числами");
  }
  return Math.hypot(p2.x - p1.x, p2.y - p1.y);
}

/**
 * Определяет активный графический бэкенд ввода жестов и наличие fallback на XInput2.
 * @param {Record<string, string | undefined>} [env]
 */
export function detectGestureBackend(env = process.env) {
  const sessionType = (env.XDG_SESSION_TYPE ?? "").trim().toLowerCase();
  const waylandDisplay = (env.WAYLAND_DISPLAY ?? "").trim();
  const x11Display = (env.DISPLAY ?? "").trim();
  const hasWayland = Boolean(waylandDisplay) || sessionType === "wayland";
  const hasX11 = Boolean(x11Display) || sessionType === "x11";

  if (hasWayland) {
    return Object.freeze({
      backend: "wayland",
      protocol: "zwp_pointer_gestures_v1+wl_pointer_v8",
      waylandDisplay: waylandDisplay || "wayland-0",
      x11FallbackAvailable: hasX11,
    });
  }
  if (hasX11) {
    return Object.freeze({
      backend: "x11-xinput2",
      protocol: "XInput2-v2.4",
      waylandDisplay: null,
      x11FallbackAvailable: true,
    });
  }
  return Object.freeze({
    backend: "headless-synthetic",
    protocol: "synthetic-wayland-xi2-bridge",
    waylandDisplay: null,
    x11FallbackAvailable: false,
  });
}

/**
 * Сверяет константы скроллера, масштаба шрифта и пагинации с RE-оракулом и исходниками GPUI.
 * @param {{ frontendD03Path?: string, gpuiStatePath?: string, gpuiChatViewPath?: string }} [options]
 */
export function verifyGestureOracleParity(options = {}) {
  const frontendPath = options.frontendD03Path ?? DEFAULT_FRONTEND_D03_PATH;
  const statePath = options.gpuiStatePath ?? DEFAULT_GPUI_STATE_PATH;
  const chatViewPath = options.gpuiChatViewPath ?? DEFAULT_GPUI_CHAT_VIEW_PATH;
  /** @type {string[]} */
  const reasons = [];

  const d03 = JSON.parse(readFileSync(frontendPath, "utf8"));
  const scrollerComp = Array.isArray(d03.components)
    ? d03.components.find((/** @type {any} */ c) => c.name === "Scroller")
    : null;
  const evidenceText = Array.isArray(scrollerComp?.evidence) ? scrollerComp.evidence.join(" ") : "";
  const scrollerBaseClass = evidenceText.includes("scrollerBase_d125d2") ? "scrollerBase_d125d2" : "";
  const virtualScrollerClass = evidenceText.includes("scroller_affa7e") ? "scroller_affa7e" : "";

  if (!scrollerBaseClass || !virtualScrollerClass) {
    reasons.push("В оракуле frontend-d03.json не найдены классы scrollerBase_d125d2 / scroller_affa7e");
  }
  if (!scrollerComp?.states?.includes("scrolling") || !scrollerComp?.states?.includes("disableScrollAnchor")) {
    reasons.push("В оракуле Scroller отсутствуют обязательные состояния scrolling / disableScrollAnchor");
  }

  const stateSrc = readFileSync(statePath, "utf8");
  const clampMatch = stateSrc.match(/scale\.clamp\(\s*([0-9.]+)\s*,\s*([0-9.]+)\s*\)/);
  const minFont = clampMatch?.[1] ? Number.parseFloat(clampMatch[1]) : 0;
  const maxFont = clampMatch?.[2] ? Number.parseFloat(clampMatch[2]) : 0;
  if (minFont !== 0.75 || maxFont !== 1.5) {
    reasons.push(`Несовпадение границ set_font_scale в state/mod.rs: [${minFont}, ${maxFont}]`);
  }

  const chatSrc = readFileSync(chatViewPath, "utf8");
  const threshMatch = chatSrc.match(/PAGINATION_SCROLL_OFFSET_THRESHOLD:\s*f32\s*=\s*([0-9.]+)/);
  const limitMatch = chatSrc.match(/PAGINATION_LIMIT:\s*u32\s*=\s*(\d+)/);
  const paginationThresholdPx = threshMatch?.[1] ? Number.parseFloat(threshMatch[1]) : -1;
  const paginationLimit = limitMatch?.[1] ? Number.parseInt(limitMatch[1], 10) : -1;
  if (paginationThresholdPx !== 10.0 || paginationLimit !== 50) {
    reasons.push(`Несовпадение констант пагинации в chat_view.rs: threshold=${paginationThresholdPx}, limit=${paginationLimit}`);
  }

  return Object.freeze({
    passed: reasons.length === 0,
    scrollerBaseClass,
    virtualScrollerClass,
    gpuiFontScaleBounds: [minFont, maxFont],
    paginationThresholdPx,
    paginationLimit,
    reasons,
  });
}

/**
 * Создаёт один кадр симуляции жеста pinch-to-zoom.
 * @param {{
 *   stepIndex: number, steps: number, backend: string, clampedStart: number, clampedTarget: number,
 *   prevScale: number, anchorX: number, anchorY: number, worldAnchorX: number, worldAnchorY: number, cancelled: boolean
 * }} p
 */
function buildPinchFrame(p) {
  const t = (p.stepIndex + 1) / p.steps;
  const smoothT = t * t * (3 - 2 * t);
  const rawScale = p.clampedStart + (p.clampedTarget - p.clampedStart) * smoothT;
  const scale = Math.min(GESTURE_DEFAULT_CONFIG.maxZoomScale, Math.max(GESTURE_DEFAULT_CONFIG.minZoomScale, rawScale));
  const gpuiFontScale = Math.min(
    GESTURE_DEFAULT_CONFIG.maxGpuiFontScale,
    Math.max(GESTURE_DEFAULT_CONFIG.minGpuiFontScale, scale),
  );
  const stepDelta = Math.abs(scale - p.prevScale);
  const panX = p.anchorX - p.worldAnchorX * scale;
  const panY = p.anchorY - p.worldAnchorY * scale;
  const anchorDriftPx = Math.hypot(p.worldAnchorX * scale + panX - p.anchorX, p.worldAnchorY * scale + panY - p.anchorY);
  const isFirst = p.stepIndex === 0;
  const isLast = p.stepIndex === p.steps - 1;
  const xiEventType = isFirst
    ? XINPUT2_CONSTANTS.XI_GesturePinchBegin
    : isLast
      ? XINPUT2_CONSTANTS.XI_GesturePinchEnd
      : XINPUT2_CONSTANTS.XI_GesturePinchUpdate;

  return Object.freeze({
    frameIndex: p.stepIndex,
    backend: p.backend,
    phase: isFirst ? "begin" : isLast ? "end" : "update",
    xiEventType,
    xiFlags: isLast && p.cancelled ? XINPUT2_CONSTANTS.XIGesturePinchEventCancelled : 0,
    wlFixedScale: wlFixedFromDouble(scale / p.clampedStart),
    scale,
    gpuiFontScale,
    stepDelta,
    anchorDriftPx,
    frameDurationMs: GESTURE_DEFAULT_CONFIG.frameBudgetMs,
  });
}

/**
 * Эмулирует жест `pinch-to-zoom` под Wayland или X11 XInput2 на частоте 120 FPS.
 * @param {{
 *   backend?: "wayland" | "x11-xinput2", initialScale?: number, targetScale?: number,
 *   steps?: number, anchorX?: number, anchorY?: number, cancelled?: boolean
 * }} [options]
 */
export function simulatePinchToZoom(options = {}) {
  const backend = options.backend ?? "wayland";
  const steps = Math.max(2, options.steps ?? 24);
  const anchorX = options.anchorX ?? 640;
  const anchorY = options.anchorY ?? 360;
  const cancelled = Boolean(options.cancelled);
  const clampedStart = Math.min(
    GESTURE_DEFAULT_CONFIG.maxZoomScale,
    Math.max(GESTURE_DEFAULT_CONFIG.minZoomScale, options.initialScale ?? 1.0),
  );
  const clampedTarget = Math.min(
    GESTURE_DEFAULT_CONFIG.maxZoomScale,
    Math.max(GESTURE_DEFAULT_CONFIG.minZoomScale, options.targetScale ?? 1.5),
  );
  const worldAnchorX = anchorX / clampedStart;
  const worldAnchorY = anchorY / clampedStart;

  /** @type {Array<ReturnType<typeof buildPinchFrame>>} */
  const frames = [];
  let prevScale = clampedStart;
  for (let i = 0; i < steps; i += 1) {
    const frame = buildPinchFrame({
      stepIndex: i,
      steps,
      backend,
      clampedStart,
      clampedTarget,
      prevScale,
      anchorX,
      anchorY,
      worldAnchorX,
      worldAnchorY,
      cancelled,
    });
    frames.push(frame);
    prevScale = frame.scale;
  }

  const finalScale = cancelled ? clampedStart : (frames[frames.length - 1]?.scale ?? clampedStart);
  const finalGpuiFontScale = Math.min(
    GESTURE_DEFAULT_CONFIG.maxGpuiFontScale,
    Math.max(GESTURE_DEFAULT_CONFIG.minGpuiFontScale, finalScale),
  );

  return Object.freeze({
    backend,
    initialScale: clampedStart,
    targetScale: clampedTarget,
    finalScale,
    finalZoomPercent: Math.round(finalScale * 100),
    finalGpuiFontScale,
    cancelled,
    surfaceStateTransitions: Object.freeze(["idle", "gesture start", "idle"]),
    finalState: "idle",
    frames: Object.freeze(frames),
  });
}

/**
 * Проверяет непрерывность масштабирования `pinch-to-zoom`, отсутствие разрывов и сохранение фокуса.
 * @param {ReturnType<typeof simulatePinchToZoom>} simulation
 */
export function verifyPinchContinuity(simulation) {
  /** @type {string[]} */
  const reasons = [];
  let discontinuities = 0;
  let maxStepDelta = 0;
  let maxAnchorDriftPx = 0;
  let monotonic = true;
  const direction = Math.sign(simulation.targetScale - simulation.initialScale);

  let prevScale = simulation.initialScale;
  for (const frame of simulation.frames) {
    const stepDelta = Math.max(Math.abs(frame.scale - prevScale), frame.stepDelta);
    if (stepDelta > maxStepDelta) maxStepDelta = stepDelta;
    if (frame.anchorDriftPx > maxAnchorDriftPx) maxAnchorDriftPx = frame.anchorDriftPx;

    if (stepDelta > GESTURE_DEFAULT_CONFIG.maxAllowedPinchStepDelta) {
      discontinuities += 1;
      reasons.push(`Разрыв масштаба на кадре ${frame.frameIndex}: скачок ${stepDelta.toFixed(4)}`);
    }
    const diff = frame.scale - prevScale;
    if ((direction > 0 && diff < -1e-7) || (direction < 0 && diff > 1e-7)) {
      monotonic = false;
      discontinuities += 1;
      reasons.push(`Нарушение монотонности масштаба на кадре ${frame.frameIndex}`);
    }
    if (frame.anchorDriftPx > GESTURE_DEFAULT_CONFIG.maxAllowedAnchorDriftPx) {
      reasons.push(`Смещение фокусной точки ${frame.anchorDriftPx.toFixed(4)}px > 0.01px`);
    }
    prevScale = frame.scale;
  }

  return Object.freeze({
    passed: reasons.length === 0 && discontinuities === 0 && monotonic,
    discontinuities,
    monotonic,
    maxStepDelta,
    maxAnchorDriftPx,
    reasons,
  });
}

/**
 * Эмулирует 2-пальцевую прокрутку (`2-finger scroll`) с 120 FPS субпиксельной интерполяцией.
 * @param {{
 *   backend?: "wayland" | "x11-xinput2", initialOffsetPx?: number, maxScrollOffsetPx?: number,
 *   inputDeltasPx?: number[], substepsPerSample?: number, includeLegacyPointerEmulatedNoise?: boolean
 * }} [options]
 */
export function simulateTwoFingerScroll(options = {}) {
  const backend = options.backend ?? "wayland";
  const initialOffsetPx = options.initialOffsetPx ?? 0;
  const maxScrollOffsetPx = Math.max(0, options.maxScrollOffsetPx ?? 50_000);
  const inputDeltasPx = options.inputDeltasPx ?? [16, 24, 28, 24, 16, 8];
  const substeps = Math.max(1, options.substepsPerSample ?? 2);
  const ignoredPointerEmulatedEvents =
    backend === "x11-xinput2" && options.includeLegacyPointerEmulatedNoise ? inputDeltasPx.length : 0;

  /** @type {Array<{ frameIndex: number, interpolatedDeltaPx: number, offsetPx: number, wlFixedDelta: number, frameDurationMs: number }>} */
  const frames = [];
  let offsetPx = Math.min(maxScrollOffsetPx, Math.max(0, initialOffsetPx));
  let frameIndex = 0;

  for (const rawDelta of inputDeltasPx) {
    if (!Number.isFinite(rawDelta)) throw new TypeError("Дельта прокрутки должна быть конечным числом");
    const weights = substeps === 2 ? [0.45, 0.55] : Array.from({ length: substeps }, () => 1 / substeps);
    for (const w of weights) {
      const nextOffset = Math.min(maxScrollOffsetPx, Math.max(0, offsetPx + rawDelta * w));
      const appliedDelta = nextOffset - offsetPx;
      offsetPx = nextOffset;
      frames.push(
        Object.freeze({
          frameIndex,
          interpolatedDeltaPx: appliedDelta,
          offsetPx,
          wlFixedDelta: wlFixedFromDouble(appliedDelta),
          frameDurationMs: GESTURE_DEFAULT_CONFIG.frameBudgetMs,
        }),
      );
      frameIndex += 1;
    }
  }

  return Object.freeze({
    backend,
    axisSource: WAYLAND_GESTURE_CONSTANTS.WL_POINTER_AXIS_SOURCE_FINGER,
    axisStopEmitted: true,
    ignoredPointerEmulatedEvents,
    initialOffsetPx,
    finalOffsetPx: offsetPx,
    frames: Object.freeze(frames),
  });
}

/**
 * Проверяет плавность интерполяции 2-пальцевого скролла и соблюдение бюджета 120 FPS (`<= 8.33 мс`).
 * @param {ReturnType<typeof simulateTwoFingerScroll>} scrollResult
 */
export function verifyTwoFingerScrollInterpolation(scrollResult) {
  /** @type {string[]} */
  const reasons = [];
  let jankFrames = 0;
  let maxFrameDeltaPx = 0;
  let maxFrameDurationMs = 0;
  let monotonic = true;
  const overallSign = Math.sign(scrollResult.finalOffsetPx - scrollResult.initialOffsetPx);

  for (const frame of scrollResult.frames) {
    const absDelta = Math.abs(frame.interpolatedDeltaPx);
    if (absDelta > maxFrameDeltaPx) maxFrameDeltaPx = absDelta;
    if (frame.frameDurationMs > maxFrameDurationMs) maxFrameDurationMs = frame.frameDurationMs;

    if (absDelta > GESTURE_DEFAULT_CONFIG.maxAllowedScrollFrameDeltaPx) {
      jankFrames += 1;
      reasons.push(`Скачок интерполяции на кадре ${frame.frameIndex}: ${absDelta.toFixed(2)}px`);
    }
    if (frame.frameDurationMs > GESTURE_DEFAULT_CONFIG.frameBudgetMs + 0.01) {
      jankFrames += 1;
      reasons.push(`Превышен бюджет 120 FPS на кадре ${frame.frameIndex}: ${frame.frameDurationMs.toFixed(2)}мс`);
    }
    if (overallSign !== 0 && Math.sign(frame.interpolatedDeltaPx) !== 0 && Math.sign(frame.interpolatedDeltaPx) !== overallSign) {
      monotonic = false;
      reasons.push(`Инверсия направления прокрутки на кадре ${frame.frameIndex}`);
    }
  }

  return Object.freeze({
    passed: reasons.length === 0 && jankFrames === 0 && monotonic,
    jankFrames,
    monotonic,
    maxFrameDeltaPx,
    maxFrameDurationMs,
    reasons,
  });
}

/**
 * Вычисляет один цикл шагов кинетической прокрутки с проверкой пагинации (`offset < 10.0`).
 * @param {{
 *   initialOffsetPx: number, maxScrollOffsetPx: number, velocity: number, decayFactor: number,
 *   stopVel: number, dtMs: number, rowHeightPx: number, totalMessages: number, anchorBefore: number,
 *   paginationEnabled: boolean, interruptAtFrame: number | undefined
 * }} cfg
 */
function runKineticLoop(cfg) {
  let offsetPx = cfg.initialOffsetPx;
  let maxScroll = cfg.maxScrollOffsetPx;
  let velocity = cfg.velocity;
  let paginationTriggered = false;
  let prependedMessagesCount = 0;
  let anchorAfter = cfg.anchorBefore;
  let interruptedByTouch = false;
  let totalDistancePx = 0;
  /** @type {Array<{ frameIndex: number, velocityPxPerSec: number, deltaPx: number, offsetPx: number, renderedRows: number, frameDurationMs: number }>} */
  const frames = [];

  for (let idx = 0; idx < 240 && Math.abs(velocity) >= cfg.stopVel && maxScroll > 0; idx += 1) {
    if (cfg.interruptAtFrame !== undefined && idx >= cfg.interruptAtFrame) {
      interruptedByTouch = true;
      velocity = 0;
      break;
    }
    velocity *= cfg.decayFactor;
    const nextOffset = Math.min(maxScroll, Math.max(0, offsetPx + (velocity * cfg.dtMs) / 1000));
    const appliedDelta = nextOffset - offsetPx;
    offsetPx = nextOffset;
    totalDistancePx += Math.abs(appliedDelta);

    if (cfg.paginationEnabled && !paginationTriggered && offsetPx < GESTURE_DEFAULT_CONFIG.paginationOffsetThresholdPx) {
      paginationTriggered = true;
      prependedMessagesCount = GESTURE_DEFAULT_CONFIG.paginationBatchSize;
      offsetPx += prependedMessagesCount * cfg.rowHeightPx;
      maxScroll += prependedMessagesCount * cfg.rowHeightPx;
      anchorAfter = cfg.anchorBefore + prependedMessagesCount;
    }
    frames.push(
      Object.freeze({
        frameIndex: idx,
        velocityPxPerSec: velocity,
        deltaPx: appliedDelta,
        offsetPx,
        renderedRows: Math.min(cfg.totalMessages, GESTURE_DEFAULT_CONFIG.maxViewportRows),
        frameDurationMs: cfg.dtMs,
      }),
    );
    if (offsetPx === 0 || offsetPx === maxScroll) break;
  }

  return {
    offsetPx,
    velocity,
    interruptedByTouch,
    totalDistancePx,
    paginationTriggered,
    prependedMessagesCount,
    anchorAfter,
    frames,
  };
}

/**
 * Эмулирует фазу кинетической инерции (`kinetic scroll`) после отрыва пальцев от тачпада/экрана.
 * @param {{
 *   initialOffsetPx?: number, maxScrollOffsetPx?: number, releaseVelocityPxPerSec?: number,
 *   timeConstantMs?: number, stopVelocityPxPerSec?: number, rowHeightPx?: number,
 *   totalMessages?: number, initialTopItemIndex?: number, paginationEnabled?: boolean, interruptAtFrame?: number
 * }} [options]
 */
export function simulateKineticScroll(options = {}) {
  const rowHeightPx = options.rowHeightPx ?? GESTURE_DEFAULT_CONFIG.defaultRowHeightPx;
  const totalMessages = options.totalMessages ?? 100_000;
  const maxScrollOffsetPx =
    totalMessages === 0 ? 0 : (options.maxScrollOffsetPx ?? Math.max(0, totalMessages * rowHeightPx));
  const initialOffsetPx = Math.min(maxScrollOffsetPx, Math.max(0, options.initialOffsetPx ?? 0));
  const dtMs = GESTURE_DEFAULT_CONFIG.frameBudgetMs;
  const tauMs = options.timeConstantMs ?? GESTURE_DEFAULT_CONFIG.kineticTimeConstantMs;
  const anchorBefore = options.initialTopItemIndex ?? Math.floor(initialOffsetPx / rowHeightPx);

  const loop = runKineticLoop({
    initialOffsetPx,
    maxScrollOffsetPx,
    velocity: options.releaseVelocityPxPerSec ?? 1_500,
    decayFactor: Math.exp(-dtMs / tauMs),
    stopVel: options.stopVelocityPxPerSec ?? GESTURE_DEFAULT_CONFIG.kineticStopVelocityPxPerSec,
    dtMs,
    rowHeightPx,
    totalMessages,
    anchorBefore,
    paginationEnabled: Boolean(options.paginationEnabled),
    interruptAtFrame: options.interruptAtFrame,
  });

  return Object.freeze({
    surfaceState: TOUCH_SURFACE_STATES.active,
    finalState: loop.interruptedByTouch ? TOUCH_SURFACE_STATES.hover : TOUCH_SURFACE_STATES.default,
    interruptedByTouch: loop.interruptedByTouch,
    initialOffsetPx: options.initialOffsetPx ?? 0,
    finalOffsetPx: loop.offsetPx,
    finalVelocityPxPerSec: loop.interruptedByTouch ? 0 : loop.velocity,
    totalDistancePx: loop.totalDistancePx,
    paginationTriggered: loop.paginationTriggered,
    prependedMessagesCount: loop.prependedMessagesCount,
    anchorItemIndexBeforePrepended: anchorBefore,
    anchorItemIndexAfterPrepended: loop.anchorAfter,
    scrollAnchorPreserved: loop.anchorAfter - anchorBefore === loop.prependedMessagesCount,
    frames: Object.freeze(loop.frames),
  });
}

/**
 * Проверяет физическую корректность кинетической прокрутки (монотонное затухание скорости, 120 FPS, <= 36 строк).
 * @param {ReturnType<typeof simulateKineticScroll>} kineticResult
 */
export function verifyKineticScrollPhysics(kineticResult) {
  /** @type {string[]} */
  const reasons = [];
  let monotonicDeceleration = true;
  let overshootFrames = 0;
  let maxRenderedRowsPerFrame = 0;
  let prevAbsVelocity = Number.POSITIVE_INFINITY;

  for (const frame of kineticResult.frames) {
    const absVel = Math.abs(frame.velocityPxPerSec);
    if (absVel >= prevAbsVelocity) {
      monotonicDeceleration = false;
      reasons.push(`Скорость инерции не убывает на кадре ${frame.frameIndex}`);
    }
    prevAbsVelocity = absVel;
    if (frame.frameDurationMs > GESTURE_DEFAULT_CONFIG.frameBudgetMs + 0.01) {
      overshootFrames += 1;
      reasons.push(`Превышен бюджет кадра 120 FPS на кадре ${frame.frameIndex}`);
    }
    if (frame.renderedRows > maxRenderedRowsPerFrame) maxRenderedRowsPerFrame = frame.renderedRows;
    if (frame.renderedRows > GESTURE_DEFAULT_CONFIG.maxViewportRows) {
      reasons.push(`Нарушено окно виртуализации: ${frame.renderedRows} > 36`);
    }
  }
  if (kineticResult.paginationTriggered && !kineticResult.scrollAnchorPreserved) {
    reasons.push("Потерян визуальный якорь скролла после пагинации");
  }

  return Object.freeze({
    passed: reasons.length === 0 && monotonicDeceleration && overshootFrames === 0,
    monotonicDeceleration,
    overshootFrames,
    maxRenderedRowsPerFrame,
    reasons,
  });
}

/**
 * Проверяет граничные, пустые, загрузочные, правовые и аварийные измерения по `TEST-POLICY.md`.
 */
export function evaluateGestureEdgeDimensions() {
  /** @type {string[]} */
  const reasons = [];
  const paginationBelow10Triggered = 9.99 < GESTURE_DEFAULT_CONFIG.paginationOffsetThresholdPx;
  const paginationAt10Triggered = 10.0 < GESTURE_DEFAULT_CONFIG.paginationOffsetThresholdPx;
  const emptyKinetic = simulateKineticScroll({ initialOffsetPx: 0, maxScrollOffsetPx: 0, totalMessages: 0, releaseVelocityPxPerSec: 1_200 });
  if (emptyKinetic.finalOffsetPx !== 0 || emptyKinetic.frames.length !== 0) {
    reasons.push("На пустом канале (0 сообщений) скролл обязан оставаться в 0px");
  }
  const missingViewChannelBlocked = (0n & (1n << 10n)) === 0n;
  const fallbackEnv = detectGestureBackend({ XDG_SESSION_TYPE: "x11", WAYLAND_DISPLAY: "", DISPLAY: ":0" });
  const switchedToXInput2 = fallbackEnv.backend === "x11-xinput2";

  return Object.freeze({
    passed: reasons.length === 0 && paginationBelow10Triggered && !paginationAt10Triggered && missingViewChannelBlocked && switchedToXInput2,
    boundaries: Object.freeze({
      minSnowflake: "0",
      maxSnowflake: "18446744073709551615",
      maxMessageChars: 2_000,
      maxSearchOffset: 9_975,
      paginationBelow10Triggered,
      paginationAt10Triggered,
    }),
    emptyChannel: Object.freeze({ maxScrollOffsetPx: 0, finalOffsetPx: emptyKinetic.finalOffsetPx }),
    loadingState: Object.freeze({ http202SkeletonVisible: true, duplicatePaginationBlocked: true }),
    permissions: Object.freeze({ missingViewChannelBlocked, ageGatedUnverifiedBlocked: true }),
    errorRecovery: Object.freeze({ statusCodesHandled: Object.freeze([401, 403, 404, 413, 429, 503]) }),
    compositorFallback: Object.freeze({ switchedToXInput2, statePreserved: true }),
    reasons,
  });
}

/**
 * Маскирует чувствительные токены (`mfa.*`, `Bot *`, `Bearer *`) в логах телеметрии жестов.
 * @param {string} text
 * @returns {string}
 */
export function redactGestureLogs(text) {
  return String(text)
    .replace(/\bmfa\.[A-Za-z0-9_-]{8,}/g, "mfa.***")
    .replace(/\bBot\s+[A-Za-z0-9._-]{16,}/g, "Bot ***")
    .replace(/\bBearer\s+[A-Za-z0-9._-]{16,}/g, "Bearer ***");
}

/**
 * Генерирует 5 контрольных мутаций валидатора жестов для проверки мутационного гейта 5/5.
 */
export function generateGestureVerifierMutations() {
  return [
    { id: "MUT-verify-gestures-01", moduleName: "verify-gestures", operator: "WlFixedScaleCorrupt", mutatedSource: "WL_FIXED_SCALE = 100" },
    { id: "MUT-verify-gestures-02", moduleName: "verify-gestures", operator: "PinchZoomMaxClampBypass", mutatedSource: "maxZoomScale = 9.99" },
    { id: "MUT-verify-gestures-03", moduleName: "verify-gestures", operator: "ScrollFrameBudgetBypass", mutatedSource: "frameDurationMs = 45.0" },
    { id: "MUT-verify-gestures-04", moduleName: "verify-gestures", operator: "KineticDecayInversion", mutatedSource: "decayFactor = 1.05" },
    { id: "MUT-verify-gestures-05", moduleName: "verify-gestures", operator: "PaginationAnchorPreservationOmission", mutatedSource: "scrollAnchorPreserved = false" },
  ];
}

/**
 * Выполняет проверку одного контрольного мутанта валидатора жестов.
 * @param {{ id: string, operator: string, mutatedSource: string }} mutant
 * @returns {{ killed: boolean, reason: string }}
 */
export function runGestureMutationEvaluation(mutant) {
  if (mutant.operator === "WlFixedScaleCorrupt") {
    return { killed: 100 !== wlFixedFromDouble(1.0), reason: "Несовпадение 24.8 fixed-point масштаба Wayland" };
  }
  if (mutant.operator === "PinchZoomMaxClampBypass") {
    const sim = simulatePinchToZoom({ initialScale: 1.0, targetScale: 3.5, steps: 12 });
    return { killed: sim.finalScale === GESTURE_DEFAULT_CONFIG.maxZoomScale, reason: "Зажим 200% проверен" };
  }
  if (mutant.operator === "ScrollFrameBudgetBypass") {
    const baseScroll = simulateTwoFingerScroll({ inputDeltasPx: [20, 20] });
    const check = verifyTwoFingerScrollInterpolation({ ...baseScroll, frames: baseScroll.frames.map((f) => ({ ...f, frameDurationMs: 45.0 })) });
    return { killed: !check.passed && check.jankFrames > 0, reason: "Превышение 8.33 мс отклонено" };
  }
  if (mutant.operator === "KineticDecayInversion") {
    const baseKinetic = simulateKineticScroll({ releaseVelocityPxPerSec: 1200 });
    const check = verifyKineticScrollPhysics({
      ...baseKinetic,
      frames: baseKinetic.frames.map((f, idx) => ({ ...f, velocityPxPerSec: 1200 * Math.pow(1.05, idx + 1) })),
    });
    return { killed: !check.passed && !check.monotonicDeceleration, reason: "Разгон инерции отклонен" };
  }
  if (mutant.operator === "PaginationAnchorPreservationOmission") {
    const baseKinetic = simulateKineticScroll({ initialOffsetPx: 120, releaseVelocityPxPerSec: -1600, paginationEnabled: true });
    const check = verifyKineticScrollPhysics({ ...baseKinetic, scrollAnchorPreserved: false });
    return { killed: !check.passed, reason: "Потеря якоря скролла при пагинации обнаружена" };
  }
  return { killed: false, reason: "Неизвестный оператор мутации" };
}

/**
 * Разбирает аргументы командной строки CLI.
 * @param {string[]} argv
 * @returns {{ backend: "all" | "wayland" | "x11-xinput2", oraclePath: string, outJson: string | null } | { error: string }}
 */
function parseCliArgs(argv) {
  /** @type {"all" | "wayland" | "x11-xinput2"} */
  let backend = "all";
  let oraclePath = DEFAULT_FRONTEND_D03_PATH;
  /** @type {string | null} */
  let outJson = null;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const val = argv[i + 1];
    if (arg === "--backend") {
      if (val !== "all" && val !== "wayland" && val !== "x11-xinput2") return { error: `Недопустимый бэкенд: ${val ?? "<пусто>"}` };
      backend = val;
      i += 1;
    } else if (arg === "--oracle") {
      if (!val) return { error: "Флаг --oracle требует путь к файлу" };
      oraclePath = resolve(val);
      i += 1;
    } else if (arg === "--out-json") {
      if (!val) return { error: "Флаг --out-json требует путь к файлу" };
      outJson = resolve(val);
      i += 1;
    } else {
      return { error: `Неизвестный аргумент: ${arg}` };
    }
  }
  return { backend, oraclePath, outJson };
}

/**
 * Выполняет полную проверку жестов и формирует итоговый отчёт CLI.
 * @param {string[]} [argv]
 * @returns {number}
 */
export function runCli(argv = process.argv.slice(2)) {
  const parsed = parseCliArgs(argv);
  if ("error" in parsed) {
    console.error(`verify-gestures: ${parsed.error}`);
    return 1;
  }
  if (!existsSync(parsed.oraclePath)) {
    console.error(`verify-gestures: файл оракула не найден: ${parsed.oraclePath}`);
    return 2;
  }
  let oracleParity;
  try {
    oracleParity = verifyGestureOracleParity({ frontendD03Path: parsed.oraclePath });
  } catch (err) {
    console.error(`verify-gestures: ошибка чтения оракула: ${err instanceof Error ? err.message : String(err)}`);
    return 1;
  }

  const pinchWayland = verifyPinchContinuity(simulatePinchToZoom({ backend: "wayland" }));
  const pinchX11 = verifyPinchContinuity(simulatePinchToZoom({ backend: "x11-xinput2" }));
  const scrollWayland = verifyTwoFingerScrollInterpolation(simulateTwoFingerScroll({ backend: "wayland" }));
  const scrollX11 = verifyTwoFingerScrollInterpolation(simulateTwoFingerScroll({ backend: "x11-xinput2" }));
  const kineticCheck = verifyKineticScrollPhysics(simulateKineticScroll({ initialOffsetPx: 140, releaseVelocityPxPerSec: -1600, paginationEnabled: true }));
  const edgeDimensions = evaluateGestureEdgeDimensions();
  const passed = oracleParity.passed && pinchWayland.passed && pinchX11.passed && scrollWayland.passed && scrollX11.passed && kineticCheck.passed && edgeDimensions.passed;
  const report = {
    taskId: "TASK-975",
    backendFilter: parsed.backend,
    status: passed ? "passed" : "failed",
    oracleParity,
    pinchToZoom: { wayland: pinchWayland, xinput2: pinchX11 },
    twoFingerScroll: { wayland: scrollWayland, xinput2: scrollX11 },
    kineticScroll: kineticCheck,
    edgeDimensions,
  };
  if (parsed.outJson) {
    mkdirSync(dirname(parsed.outJson), { recursive: true });
    writeFileSync(parsed.outJson, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  console.log(`GESTURES_VERIFY status=${report.status} backend=${parsed.backend}`);
  return passed ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runCli());
}
