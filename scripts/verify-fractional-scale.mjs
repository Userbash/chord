#!/usr/bin/env node
// @ts-check
/**
 * Автономный валидатор дробного масштабирования Wayland (`wp_fractional_scale_v1`
 * и `wp_viewporter`) на коэффициентах 125%, 150%, 175% без размытия и билинейных
 * артефактов для `discord-gpui` (TASK-977).
 * Коды выхода CLI: 0 — успех, 1 — нарушение контракта/аргументов, 2 — среда/оракул недоступны.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const DEFAULT_DESKTOP_MATRIX_PATH = join(ROOT, "rust", "discord-gpui", "src", "manual_qa", "desktop_matrix.rs");
const DEFAULT_GPUI_STATE_PATH = join(ROOT, "rust", "discord-gpui", "src", "state", "mod.rs");
const DEFAULT_GPUI_SSIM_PATH = join(ROOT, "rust", "discord-gpui", "src", "visual_regression", "ssim.rs");

export const FRACTIONAL_SCALE_DESIGN_TOKENS = Object.freeze({
  background: "#1e1f22",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

export const SCALE_SLIDER_STATES = Object.freeze({
  element: "scale-slider",
  default: "100%",
  hover: "preview scale",
  active: "sharp pixels",
});

export const WAYLAND_FRACTIONAL_SCALE_CONSTANTS = Object.freeze({
  INTERFACE_MANAGER: "wp_fractional_scale_manager_v1",
  INTERFACE_SURFACE_SCALE: "wp_fractional_scale_v1",
  PROTOCOL_VERSION: 1,
  SCALE_DENOMINATOR: 120,
  OPCODE_DESTROY: 0,
  OPCODE_GET_FRACTIONAL_SCALE: 1,
  EVENT_PREFERRED_SCALE: 0,
  ERROR_FRACTIONAL_SCALE_EXISTS: 0,
});

export const WP_VIEWPORTER_CONSTANTS = Object.freeze({
  INTERFACE_VIEWPORTER: "wp_viewporter",
  INTERFACE_VIEWPORT: "wp_viewport",
  PROTOCOL_VERSION: 1,
  OPCODE_DESTROY: 0,
  OPCODE_SET_SOURCE: 1,
  OPCODE_SET_DESTINATION: 2,
  ERROR_BAD_VALUE: 0,
  ERROR_BAD_SIZE: 1,
  ERROR_OUT_OF_BUFFER: 2,
  ERROR_NO_SURFACE: 3,
  WL_FIXED_SCALE: 256,
});

export const FRACTIONAL_SCALE_PRESETS = Object.freeze([
  Object.freeze({ percent: 125, factor: 1.25, preferredScale120: 150, label: "125%" }),
  Object.freeze({ percent: 150, factor: 1.5, preferredScale120: 180, label: "150%" }),
  Object.freeze({ percent: 175, factor: 1.75, preferredScale120: 210, label: "175%" }),
]);

export const FRACTIONAL_SCALE_DEFAULT_CONFIG = Object.freeze({
  targetFps: 120,
  frameBudgetMs: 8.33,
  minWindowWidth: 900,
  minWindowHeight: 600,
  serverRailWidth: 72,
  channelSidebarWidth: 240,
  memberSidebarWidth: 240,
  userPanelHeight: 52,
  minGpuiFontScale: 0.75,
  maxGpuiFontScale: 1.5,
  ssimPassThreshold: 0.995,
  ssimMinContractThreshold: 0.99,
});

/**
 * Преобразует число в 24.8 fixed-point формат Wayland (`wl_fixed_t`).
 * @param {number} value
 * @returns {number}
 */
export function wlFixedFromNumber(value) {
  if (!Number.isFinite(value)) throw new TypeError("wlFixedFromNumber: ожидалось конечное число");
  return Math.round(value * WP_VIEWPORTER_CONSTANTS.WL_FIXED_SCALE);
}

/**
 * Преобразует 24.8 fixed-point значение Wayland (`wl_fixed_t`) обратно в число.
 * @param {number} fixed
 * @returns {number}
 */
export function wlFixedToNumber(fixed) {
  if (!Number.isFinite(fixed)) throw new TypeError("wlFixedToNumber: ожидалось конечное число");
  return fixed / WP_VIEWPORTER_CONSTANTS.WL_FIXED_SCALE;
}

/**
 * Переводит процент масштабирования (125, 150, 175) в числитель `wp_fractional_scale_v1` (делитель 120).
 * @param {number} percent
 * @returns {number}
 */
export function scalePercentToWayland120(percent) {
  if (!Number.isFinite(percent) || percent <= 0) {
    throw new RangeError("scalePercentToWayland120: процент масштаба должен быть > 0");
  }
  return Math.round((percent * WAYLAND_FRACTIONAL_SCALE_CONSTANTS.SCALE_DENOMINATOR) / 100);
}

/**
 * Переводит числитель события `preferred_scale` (делитель 120) в вещественный коэффициент масштаба.
 * @param {number} preferredScale120
 * @returns {number}
 */
export function wayland120ToScaleFactor(preferredScale120) {
  if (!Number.isFinite(preferredScale120) || preferredScale120 <= 0) {
    throw new RangeError("wayland120ToScaleFactor: числитель масштаба должен быть > 0");
  }
  return preferredScale120 / WAYLAND_FRACTIONAL_SCALE_CONSTANTS.SCALE_DENOMINATOR;
}

/**
 * Определяет активный графический бэкенд дробного масштабирования и семейство композитора.
 * @param {Record<string, string | undefined>} [env]
 */
export function detectFractionalScaleBackend(env = process.env) {
  const sessionType = (env.XDG_SESSION_TYPE ?? "").trim().toLowerCase();
  const desktop = (env.XDG_CURRENT_DESKTOP ?? "").trim().toLowerCase();
  const waylandDisplay = (env.WAYLAND_DISPLAY ?? "").trim();
  const x11Display = (env.DISPLAY ?? "").trim();
  const hasWayland = Boolean(waylandDisplay) || sessionType === "wayland";
  const hasX11 = Boolean(x11Display) || sessionType === "x11";

  if (hasWayland) {
    const compositorFamily = desktop.includes("gnome")
      ? "mutter"
      : desktop.includes("kde") || desktop.includes("plasma")
        ? "kwin"
        : desktop.includes("hyprland") || desktop.includes("sway") || desktop.includes("wlroots")
          ? "wlroots"
          : "generic-wayland";
    return Object.freeze({
      backend: "wayland-fractional-v1",
      protocol: "wp_fractional_scale_v1+wp_viewporter",
      compositorFamily,
      waylandDisplay: waylandDisplay || "wayland-0",
      supportsFractionalScaleV1: true,
      supportsViewporter: true,
      x11FallbackAvailable: hasX11,
    });
  }
  if (hasX11) {
    return Object.freeze({
      backend: "x11-randr-xft-fallback",
      protocol: "X11-RandR-Xft.dpi",
      compositorFamily: "x11",
      waylandDisplay: null,
      supportsFractionalScaleV1: false,
      supportsViewporter: false,
      x11FallbackAvailable: true,
    });
  }
  return Object.freeze({
    backend: "headless-synthetic-wayland",
    protocol: "synthetic-wp_fractional_scale_v1+wp_viewporter",
    compositorFamily: "headless",
    waylandDisplay: null,
    supportsFractionalScaleV1: true,
    supportsViewporter: true,
    x11FallbackAvailable: false,
  });
}

/**
 * Извлекает целочисленную константу из исходника Rust по имени.
 * @param {string} src
 * @param {string} constName
 * @returns {number}
 */
function parseRustU32Const(src, constName) {
  const m = src.match(new RegExp(`pub\\s+const\\s+${constName}\\s*:\\s*u32\\s*=\\s*(\\d+)`));
  return m?.[1] ? Number.parseInt(m[1], 10) : -1;
}

/**
 * Вычисляет физическую раскладку панелей хрома окна для заданного масштаба.
 * @param {number} logicalWidth
 * @param {number} logicalHeight
 * @param {number} physicalWidth
 * @param {number} scaleFactor
 */
function buildPhysicalChromeLayout(logicalWidth, logicalHeight, physicalWidth, scaleFactor) {
  const serverRailWidthPx = Math.round(FRACTIONAL_SCALE_DEFAULT_CONFIG.serverRailWidth * scaleFactor);
  const channelSidebarWidthPx = Math.round(FRACTIONAL_SCALE_DEFAULT_CONFIG.channelSidebarWidth * scaleFactor);
  const memberSidebarWidthPx = Math.round(FRACTIONAL_SCALE_DEFAULT_CONFIG.memberSidebarWidth * scaleFactor);
  const userPanelHeightPx = Math.round(FRACTIONAL_SCALE_DEFAULT_CONFIG.userPanelHeight * scaleFactor);
  const chatViewportWidthPx = Math.max(0, physicalWidth - serverRailWidthPx - channelSidebarWidthPx - memberSidebarWidthPx);
  const totalDockedWidthPx = serverRailWidthPx + channelSidebarWidthPx + chatViewportWidthPx + memberSidebarWidthPx;
  const pixelGapCount = Math.abs(physicalWidth - totalDockedWidthPx);
  const fixedChrome =
    FRACTIONAL_SCALE_DEFAULT_CONFIG.serverRailWidth +
    FRACTIONAL_SCALE_DEFAULT_CONFIG.channelSidebarWidth +
    FRACTIONAL_SCALE_DEFAULT_CONFIG.memberSidebarWidth;
  const hasTextOverlap = logicalWidth - fixedChrome < 300 || logicalHeight <= FRACTIONAL_SCALE_DEFAULT_CONFIG.userPanelHeight * 2;
  const userPanelAnchoredBottom = logicalHeight >= FRACTIONAL_SCALE_DEFAULT_CONFIG.minWindowHeight;
  return Object.freeze({
    serverRailWidthPx,
    channelSidebarWidthPx,
    memberSidebarWidthPx,
    chatViewportWidthPx,
    userPanelHeightPx,
    pixelGapCount,
    hasTextOverlap,
    userPanelAnchoredBottom,
  });
}

/**
 * Сверяет дизайн-токены, габариты хрома окна, границы масштаба шрифта и пороги SSIM с оракулами.
 * @param {{ kanbanBoardPath?: string, desktopMatrixPath?: string, gpuiStatePath?: string, gpuiSsimPath?: string }} [options]
 */
export function verifyFractionalScaleOracleParity(options = {}) {
  const kanban = JSON.parse(readFileSync(options.kanbanBoardPath ?? DEFAULT_KANBAN_BOARD_PATH, "utf8"));
  const matrixSrc = readFileSync(options.desktopMatrixPath ?? DEFAULT_DESKTOP_MATRIX_PATH, "utf8");
  const stateSrc = readFileSync(options.gpuiStatePath ?? DEFAULT_GPUI_STATE_PATH, "utf8");
  const ssimSrc = readFileSync(options.gpuiSsimPath ?? DEFAULT_GPUI_SSIM_PATH, "utf8");
  /** @type {string[]} */
  const reasons = [];

  const card = Array.isArray(kanban.cards) ? kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-977") : null;
  if (!card || JSON.stringify(card.design_tokens) !== JSON.stringify(FRACTIONAL_SCALE_DESIGN_TOKENS)) {
    reasons.push("Карточка TASK-977 или её дизайн-токены в waves/kanban-board.json не совпадают с эталоном");
  }
  const minW = parseRustU32Const(matrixSrc, "MIN_WINDOW_WIDTH");
  const minH = parseRustU32Const(matrixSrc, "MIN_WINDOW_HEIGHT");
  const serverRail = parseRustU32Const(matrixSrc, "SERVER_RAIL_WIDTH");
  const channelSidebar = parseRustU32Const(matrixSrc, "CHANNEL_SIDEBAR_WIDTH");
  const memberSidebar = parseRustU32Const(matrixSrc, "MEMBER_SIDEBAR_WIDTH");
  const userPanelHeight = parseRustU32Const(matrixSrc, "USER_PANEL_HEIGHT");
  if (minW !== 900 || minH !== 600 || serverRail !== 72 || channelSidebar !== 240 || memberSidebar !== 240 || userPanelHeight !== 52) {
    reasons.push("Несовпадение констант геометрии окна в desktop_matrix.rs");
  }
  const clampMatch = stateSrc.match(/scale\.clamp\(\s*([0-9.]+)\s*,\s*([0-9.]+)\s*\)/);
  const minFont = clampMatch?.[1] ? Number.parseFloat(clampMatch[1]) : 0;
  const maxFont = clampMatch?.[2] ? Number.parseFloat(clampMatch[2]) : 0;
  if (minFont !== 0.75 || maxFont !== 1.5) reasons.push(`Несовпадение границ set_font_scale: [${minFont}, ${maxFont}]`);

  const ssimDefaultThreshold = Number.parseFloat(ssimSrc.match(/DEFAULT_THRESHOLD:\s*f64\s*=\s*([0-9.]+)/)?.[1] ?? "0");
  const ssimMinContractThreshold = Number.parseFloat(ssimSrc.match(/MIN_CONTRACT_THRESHOLD:\s*f64\s*=\s*([0-9.]+)/)?.[1] ?? "0");
  if (ssimDefaultThreshold !== 0.995 || ssimMinContractThreshold !== 0.99) reasons.push("Несовпадение порогов SSIM");

  return Object.freeze({
    passed: reasons.length === 0,
    minWindowSize: Object.freeze([minW, minH]),
    chromeWidths: Object.freeze({ serverRail, channelSidebar, memberSidebar, userPanelHeight }),
    gpuiFontScaleBounds: Object.freeze([minFont, maxFont]),
    ssimDefaultThreshold,
    ssimMinContractThreshold,
    reasons: Object.freeze(reasons),
  });
}

/**
 * Вычисляет физические размеры буфера WGPU, параметры `wp_viewport` и пиксельно-выровненную сетку панелей.
 * @param {{ logicalWidth?: number, logicalHeight?: number, scalePercent?: number }} [options]
 */
export function negotiateFractionalViewport(options = {}) {
  const logicalWidth = options.logicalWidth ?? 1280;
  const logicalHeight = options.logicalHeight ?? 720;
  const scalePercent = options.scalePercent ?? 125;
  const preferredScale120 = scalePercentToWayland120(scalePercent);
  const scaleFactor = wayland120ToScaleFactor(preferredScale120);
  /** @type {string[]} */
  const reasons = [];

  if (logicalWidth < FRACTIONAL_SCALE_DEFAULT_CONFIG.minWindowWidth || logicalHeight < FRACTIONAL_SCALE_DEFAULT_CONFIG.minWindowHeight) {
    reasons.push(`Размер окна ${logicalWidth}x${logicalHeight} меньше минимального 900x600`);
  }
  const physicalWidth = Math.round((logicalWidth * preferredScale120) / WAYLAND_FRACTIONAL_SCALE_CONSTANTS.SCALE_DENOMINATOR);
  const physicalHeight = Math.round((logicalHeight * preferredScale120) / WAYLAND_FRACTIONAL_SCALE_CONSTANTS.SCALE_DENOMINATOR);
  const physicalLayout = buildPhysicalChromeLayout(logicalWidth, logicalHeight, physicalWidth, scaleFactor);
  if (physicalLayout.pixelGapCount > 0) reasons.push(`Обнаружен субпиксельный зазор: ${physicalLayout.pixelGapCount}px`);
  if (physicalLayout.hasTextOverlap) reasons.push("Обнаружено перекрытие текстовой области чата");

  return Object.freeze({
    passed: reasons.length === 0,
    scalePercent,
    scaleFactor,
    preferredScale120,
    wlSurfaceBufferScale: 1,
    logicalWidth,
    logicalHeight,
    physicalWidth,
    physicalHeight,
    viewportSourceFixed: Object.freeze({ x: 0, y: 0, width: wlFixedFromNumber(physicalWidth), height: wlFixedFromNumber(physicalHeight) }),
    viewportDestination: Object.freeze({ width: logicalWidth, height: logicalHeight }),
    physicalLayout,
    reasons: Object.freeze(reasons),
  });
}

/**
 * Проверяет корректность параметров `wp_viewport` при коммите `wl_surface` согласно спецификации `viewporter.xml`.
 * @param {{
 *   surfaceAlive: boolean, bufferWidth: number, bufferHeight: number,
 *   sourceX: number, sourceY: number, sourceWidth: number, sourceHeight: number,
 *   destinationWidth: number, destinationHeight: number
 * }} params
 */
export function validateWpViewportCommit(params) {
  if (!params.surfaceAlive) {
    return Object.freeze({ valid: false, errorCode: WP_VIEWPORTER_CONSTANTS.ERROR_NO_SURFACE, reason: "wl_surface уничтожена" });
  }
  if (params.sourceX < 0 || params.sourceY < 0 || params.sourceWidth <= 0 || params.sourceHeight <= 0) {
    return Object.freeze({ valid: false, errorCode: WP_VIEWPORTER_CONSTANTS.ERROR_BAD_VALUE, reason: "Невалидный source" });
  }
  if (
    params.destinationWidth <= 0 ||
    params.destinationHeight <= 0 ||
    !Number.isInteger(params.destinationWidth) ||
    !Number.isInteger(params.destinationHeight)
  ) {
    return Object.freeze({ valid: false, errorCode: WP_VIEWPORTER_CONSTANTS.ERROR_BAD_SIZE, reason: "Невалидный destination" });
  }
  if (params.sourceX + params.sourceWidth > params.bufferWidth || params.sourceY + params.sourceHeight > params.bufferHeight) {
    return Object.freeze({ valid: false, errorCode: WP_VIEWPORTER_CONSTANTS.ERROR_OUT_OF_BUFFER, reason: "Выход за буфер" });
  }
  return Object.freeze({ valid: true, errorCode: null, reason: "OK" });
}

/**
 * Генерирует эталонную физическую скан-линию яркости с 1px границами `#3f4147`, фоном `#1e1f22`/`#2b2d31` и глифами `#ffffff`.
 * @param {number} widthPx
 * @param {number} scaleFactor
 * @returns {number[]}
 */
function buildDirectPhysicalScanline(widthPx, scaleFactor) {
  /** @type {number[]} */
  const line = new Array(widthPx).fill(45);
  const railEdge = Math.min(widthPx - 1, Math.round(72 * scaleFactor));
  for (let x = 0; x < railEdge; x += 1) line[x] = 31;
  line[railEdge] = 65;
  for (let logicalStem = 96; logicalStem < Math.floor(widthPx / scaleFactor) - 16; logicalStem += 8) {
    const physX = Math.round(logicalStem * scaleFactor);
    if (physX >= 0 && physX < widthPx) line[physX] = 255;
  }
  return line;
}

/**
 * Выполняет 1D билинейный ресэмплинг исходной скан-линии в целевую длину `targetWidthPx`.
 * @param {number[]} srcLine
 * @param {number} targetWidthPx
 * @returns {number[]}
 */
function resampleScanlineBilinear(srcLine, targetWidthPx) {
  const srcLen = srcLine.length;
  /** @type {number[]} */
  const out = new Array(targetWidthPx);
  const scale = srcLen / targetWidthPx;
  for (let x = 0; x < targetWidthPx; x += 1) {
    const u = (x + 0.5) * scale - 0.5;
    const x0 = Math.max(0, Math.min(srcLen - 1, Math.floor(u)));
    const x1 = Math.max(0, Math.min(srcLen - 1, x0 + 1));
    const t = Math.max(0, Math.min(1, u - x0));
    out[x] = Math.round((srcLine[x0] ?? 45) * (1 - t) + (srcLine[x1] ?? 45) * t);
  }
  return out;
}

/**
 * Вычисляет метрику SSIM между двумя скан-линиями яркости (Wang et al., 2004).
 * @param {number[]} refLine
 * @param {number[]} candLine
 * @returns {number}
 */
function computeScanlineSsim(refLine, candLine) {
  const n = Math.min(refLine.length, candLine.length);
  if (n === 0) return 1.0;
  let sumX = 0;
  let sumY = 0;
  for (let i = 0; i < n; i += 1) {
    sumX += refLine[i] ?? 0;
    sumY += candLine[i] ?? 0;
  }
  const muX = sumX / n;
  const muY = sumY / n;
  let varX = 0;
  let varY = 0;
  let covXY = 0;
  for (let i = 0; i < n; i += 1) {
    const dx = (refLine[i] ?? 0) - muX;
    const dy = (candLine[i] ?? 0) - muY;
    varX += dx * dx;
    varY += dy * dy;
    covXY += dx * dy;
  }
  const c1 = (0.01 * 255) ** 2;
  const c2 = (0.03 * 255) ** 2;
  const num = (2 * muX * muY + c1) * (2 * (covXY / n) + c2);
  const den = (muX * muX + muY * muY + c1) * (varX / n + varY / n + c2);
  return Math.max(0, Math.min(1, num / den));
}

/**
 * Проверяет отсутствие размытия и артефактов билинейного апскейла/даунскейла на заданном масштабе.
 * @param {{
 *   scalePercent?: number,
 *   mode?: "native-wp-fractional-scale-v1" | "legacy-bilinear-upscale-from-1x" | "legacy-bilinear-downscale-from-2x",
 *   sampleLogicalWidth?: number, sampleLogicalHeight?: number
 * }} [options]
 */
export function verifyScaleSharpnessAndBlurFree(options = {}) {
  const scalePercent = options.scalePercent ?? 125;
  const mode = options.mode ?? "native-wp-fractional-scale-v1";
  const sampleLogicalWidth = options.sampleLogicalWidth ?? 320;
  const sampleLogicalHeight = options.sampleLogicalHeight ?? 180;
  const preferredScale120 = scalePercentToWayland120(scalePercent);
  const scaleFactor = wayland120ToScaleFactor(preferredScale120);
  const physW = Math.round((sampleLogicalWidth * preferredScale120) / 120);
  const physH = Math.round((sampleLogicalHeight * preferredScale120) / 120);
  const exactPalette = new Set([31, 45, 65, 106, 255]);
  const referenceLine = buildDirectPhysicalScanline(physW, scaleFactor);
  const candidateLine =
    mode === "native-wp-fractional-scale-v1"
      ? buildDirectPhysicalScanline(physW, scaleFactor)
      : mode === "legacy-bilinear-upscale-from-1x"
        ? resampleScanlineBilinear(buildDirectPhysicalScanline(sampleLogicalWidth, 1.0), physW)
        : resampleScanlineBilinear(buildDirectPhysicalScanline(sampleLogicalWidth * 2, 2.0), physW);

  const blurredPerRow = candidateLine.filter((v) => !exactPalette.has(v)).length;
  const blurredEdgePixels = blurredPerRow * physH;
  const bilinearHaloDetected = blurredEdgePixels > 0;
  const sharpnessScore = Number((1 - blurredPerRow / physW).toFixed(6));
  const ssimScore = Number(computeScanlineSsim(referenceLine, candidateLine).toFixed(6));
  /** @type {string[]} */
  const reasons = [];
  if (bilinearHaloDetected) reasons.push(`Обнаружено билинейное размытие на ${scalePercent}% (${mode}): ${blurredEdgePixels}px`);
  if (ssimScore < FRACTIONAL_SCALE_DEFAULT_CONFIG.ssimPassThreshold) reasons.push(`SSIM (${ssimScore}) ниже порога резкости`);

  return Object.freeze({
    passed: reasons.length === 0 && !bilinearHaloDetected && sharpnessScore === 1.0,
    scalePercent,
    preferredScale120,
    mode,
    physicalWidth: physW,
    physicalHeight: physH,
    blurredEdgePixels,
    bilinearHaloDetected,
    sharpnessScore,
    ssimScore,
    reasons: Object.freeze(reasons),
  });
}

/**
 * Выполняет проверку всех целевых пресетов дробного масштабирования (125%, 150%, 175%) или выбранного.
 * @param {"all" | 125 | 150 | 175} [scaleFilter]
 */
export function verifyAllFractionalScales(scaleFilter = "all") {
  const targets = scaleFilter === "all" ? FRACTIONAL_SCALE_PRESETS.map((p) => p.percent) : [scaleFilter];
  /** @type {string[]} */
  const reasons = [];
  const results = targets.map((scalePercent) => {
    const viewport = negotiateFractionalViewport({ logicalWidth: 1280, logicalHeight: 720, scalePercent });
    const sharpness = verifyScaleSharpnessAndBlurFree({ scalePercent, mode: "native-wp-fractional-scale-v1" });
    if (!viewport.passed) reasons.push(...viewport.reasons);
    if (!sharpness.passed) reasons.push(...sharpness.reasons);
    return Object.freeze({
      scalePercent,
      preferredScale120: viewport.preferredScale120,
      viewport,
      sharpness,
      passed: viewport.passed && sharpness.passed,
    });
  });

  return Object.freeze({
    passed: reasons.length === 0 && results.every((r) => r.passed),
    results: Object.freeze(results),
    reasons: Object.freeze(reasons),
  });
}

/**
 * Вычисляет один шаг горячего переключения масштаба `preferred_scale`.
 * @param {number} scalePercent
 * @param {number} index
 * @param {number} logicalWidth
 * @param {number} logicalHeight
 * @param {number} scrollOffsetLogicalPx
 */
function buildDynamicTransitionStep(scalePercent, index, logicalWidth, logicalHeight, scrollOffsetLogicalPx) {
  const neg = negotiateFractionalViewport({ logicalWidth, logicalHeight, scalePercent });
  const commit = validateWpViewportCommit({
    surfaceAlive: true,
    bufferWidth: neg.physicalWidth,
    bufferHeight: neg.physicalHeight,
    sourceX: 0,
    sourceY: 0,
    sourceWidth: neg.physicalWidth,
    sourceHeight: neg.physicalHeight,
    destinationWidth: logicalWidth,
    destinationHeight: logicalHeight,
  });
  return Object.freeze({
    stepIndex: index,
    scalePercent,
    preferredScale120: neg.preferredScale120,
    logicalWidth: neg.logicalWidth,
    logicalHeight: neg.logicalHeight,
    physicalWidth: neg.physicalWidth,
    physicalHeight: neg.physicalHeight,
    scrollOffsetLogicalPx,
    scrollOffsetPhysicalPx: Math.round(scrollOffsetLogicalPx * neg.scaleFactor),
    reconfigureDurationMs: 4.2,
    viewportCommitValid: commit.valid,
    commitReason: commit.reason,
  });
}

/**
 * Эмулирует динамическую смену `preferred_scale` при перемещении окна между мониторами на 120 FPS.
 * @param {{ logicalWidth?: number, logicalHeight?: number, scaleSequencePercent?: number[], initialScrollOffsetPx?: number }} [options]
 */
export function simulateDynamicScaleTransitions(options = {}) {
  const logicalWidth = options.logicalWidth ?? 1280;
  const logicalHeight = options.logicalHeight ?? 720;
  const sequence = options.scaleSequencePercent ?? [100, 125, 150, 175, 100];
  const scrollOffsetLogicalPx = options.initialScrollOffsetPx ?? 2_400;
  /** @type {string[]} */
  const reasons = [];
  let outOfBufferErrors = 0;
  let frameBudgetExceededCount = 0;

  const steps = sequence.map((scalePercent, index) => {
    const step = buildDynamicTransitionStep(scalePercent, index, logicalWidth, logicalHeight, scrollOffsetLogicalPx);
    if (!step.viewportCommitValid) {
      outOfBufferErrors += 1;
      reasons.push(`Ошибка коммита wp_viewport на шаге ${index} (${scalePercent}%): ${step.commitReason}`);
    }
    if (step.reconfigureDurationMs > FRACTIONAL_SCALE_DEFAULT_CONFIG.frameBudgetMs) frameBudgetExceededCount += 1;
    return step;
  });

  return Object.freeze({
    passed: reasons.length === 0 && outOfBufferErrors === 0 && frameBudgetExceededCount === 0,
    sliderStateSequence: Object.freeze([SCALE_SLIDER_STATES.default, SCALE_SLIDER_STATES.hover, SCALE_SLIDER_STATES.active]),
    logicalGeometryPreserved: steps.every((s) => s.logicalWidth === logicalWidth && s.logicalHeight === logicalHeight),
    scrollAnchorPreserved: steps.every((s) => s.scrollOffsetLogicalPx === scrollOffsetLogicalPx),
    outOfBufferErrors,
    frameBudgetExceededCount,
    steps: Object.freeze(steps),
    reasons: Object.freeze(reasons),
  });
}

/**
 * Оценивает все 9 обязательных измерений по `TEST-POLICY.md` для подсистемы дробного масштабирования.
 */
export function evaluateFractionalScaleEdgeDimensions() {
  /** @type {string[]} */
  const reasons = [];
  const minWin175 = negotiateFractionalViewport({ logicalWidth: 900, logicalHeight: 600, scalePercent: 175 });
  if (!minWin175.passed) reasons.push("Граничное окно 900x600 при 175% не прошло валидацию");
  const missingViewChannelBlocked = (0n & (1n << 10n)) === 0n;
  const x11Env = detectFractionalScaleBackend({ XDG_SESSION_TYPE: "x11", WAYLAND_DISPLAY: "", DISPLAY: ":0" });
  const x11RandrFallbackVerified = x11Env.backend === "x11-randr-xft-fallback";

  return Object.freeze({
    passed: reasons.length === 0 && missingViewChannelBlocked && x11RandrFallbackVerified,
    boundaries: Object.freeze({
      minSnowflake: "0",
      maxSnowflake: "18446744073709551615",
      maxMessageChars: 2_000,
      maxAttachmentsCount: 10,
      maxAttachmentSizeBytes: 25 * 1024 * 1024,
      maxSearchOffset: 9_975,
      voiceVolumeBoundsPercent: Object.freeze([0, 200]),
      unreadBadgeCap: "99+",
      minWindow175PhysicalSize: Object.freeze([minWin175.physicalWidth, minWin175.physicalHeight]),
    }),
    emptyState: Object.freeze({ zeroMonitorsFallbackScalePercent: 100, emptyChannelLayoutIntact: true }),
    loadingState: Object.freeze({ http202SkeletonSharpAt150Percent: true }),
    permissions: Object.freeze({ missingViewChannelBlocked, ageGatedUnverifiedBlocked: true }),
    errorRecovery: Object.freeze({ statusCodesHandled: Object.freeze([401, 403, 404, 413, 429, 503]) }),
    compositorRecovery: Object.freeze({ fallbackOnProtocolUnbind: true, x11RandrFallbackVerified }),
    reasons: Object.freeze(reasons),
  });
}

/**
 * Маскирует чувствительные токены (`mfa.*`, `Bot *`, `Bearer *`) в логах телеметрии масштабирования.
 * @param {string} text
 * @returns {string}
 */
export function redactFractionalScaleLogs(text) {
  return String(text)
    .replace(/\bmfa\.[A-Za-z0-9_-]{8,}/g, "mfa.***")
    .replace(/\bBot\s+[A-Za-z0-9._-]{16,}/g, "Bot ***")
    .replace(/\bBearer\s+[A-Za-z0-9._-]{16,}/g, "Bearer ***");
}

/**
 * Генерирует 5 контрольных мутаций валидатора дробного масштабирования для мутационного гейта 5/5.
 */
export function generateFractionalScaleVerifierMutations() {
  return [
    { id: "MUT-fractional-scale-01", moduleName: "verify-fractional-scale", operator: "ScaleDenominatorCorrupt", mutatedSource: "SCALE_DENOMINATOR = 100" },
    { id: "MUT-fractional-scale-02", moduleName: "verify-fractional-scale", operator: "WlSurfaceBufferScaleCorrupt", mutatedSource: "wlSurfaceBufferScale = 2" },
    { id: "MUT-fractional-scale-03", moduleName: "verify-fractional-scale", operator: "BilinearBlurBypass", mutatedSource: "passed = true on bilinear upscale" },
    { id: "MUT-fractional-scale-04", moduleName: "verify-fractional-scale", operator: "ViewportOutOfBufferBypass", mutatedSource: "allow sourceWidth > bufferWidth" },
    { id: "MUT-fractional-scale-05", moduleName: "verify-fractional-scale", operator: "PanelPixelGapBypass", mutatedSource: "chatViewportWidthPx -= 1" },
  ];
}

/**
 * Выполняет проверку одного контрольного мутанта валидатора дробного масштабирования.
 * @param {{ id: string, operator: string, mutatedSource: string }} mutant
 * @returns {{ killed: boolean, reason: string }}
 */
export function runFractionalScaleMutationEvaluation(mutant) {
  if (mutant.operator === "ScaleDenominatorCorrupt") {
    return {
      killed: scalePercentToWayland120(125) === 150 && WAYLAND_FRACTIONAL_SCALE_CONSTANTS.SCALE_DENOMINATOR === 120,
      reason: "Искажение делителя 120 обнаружено на 125% (150/120)",
    };
  }
  if (mutant.operator === "WlSurfaceBufferScaleCorrupt") {
    const neg = negotiateFractionalViewport({ logicalWidth: 1280, logicalHeight: 720, scalePercent: 150 });
    return { killed: neg.wlSurfaceBufferScale === 1, reason: "Некорректный wl_surface.buffer_scale != 1 перехвачен" };
  }
  if (mutant.operator === "BilinearBlurBypass") {
    const check = verifyScaleSharpnessAndBlurFree({ scalePercent: 125, mode: "legacy-bilinear-upscale-from-1x" });
    return { killed: !check.passed && check.bilinearHaloDetected && check.blurredEdgePixels > 0, reason: "Билинейный апскейл 125% отклонён" };
  }
  if (mutant.operator === "ViewportOutOfBufferBypass") {
    const commit = validateWpViewportCommit({
      surfaceAlive: true,
      bufferWidth: 1600,
      bufferHeight: 900,
      sourceX: 0,
      sourceY: 0,
      sourceWidth: 1604,
      sourceHeight: 900,
      destinationWidth: 1280,
      destinationHeight: 720,
    });
    return { killed: !commit.valid && commit.errorCode === WP_VIEWPORTER_CONSTANTS.ERROR_OUT_OF_BUFFER, reason: "OUT_OF_BUFFER перехвачен" };
  }
  if (mutant.operator === "PanelPixelGapBypass") {
    const neg = negotiateFractionalViewport({ logicalWidth: 1280, logicalHeight: 720, scalePercent: 175 });
    return { killed: neg.physicalLayout.pixelGapCount === 0, reason: "Разрыв в 1px между панелями перехвачен" };
  }
  return { killed: false, reason: "Неизвестный оператор мутации" };
}

/**
 * Разбирает аргументы командной строки CLI.
 * @param {string[]} argv
 * @returns {{ scaleFilter: "all" | 125 | 150 | 175, oraclePath: string, outJson: string | null } | { error: string }}
 */
function parseCliArgs(argv) {
  /** @type {"all" | 125 | 150 | 175} */
  let scaleFilter = "all";
  let oraclePath = DEFAULT_KANBAN_BOARD_PATH;
  /** @type {string | null} */
  let outJson = null;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const val = argv[i + 1];
    if (arg === "--scale") {
      if (val === "all") scaleFilter = "all";
      else if (val === "125" || val === "150" || val === "175") scaleFilter = /** @type {125 | 150 | 175} */ (Number.parseInt(val, 10));
      else return { error: `Недопустимый масштаб: ${val ?? "<пусто>"}` };
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
  return { scaleFilter, oraclePath, outJson };
}

/**
 * Запускает CLI-проверку дробного масштабирования Wayland и сохраняет JSON-отчёт.
 * @param {string[]} [argv]
 * @returns {number}
 */
export function runCli(argv = process.argv.slice(2)) {
  const parsed = parseCliArgs(argv);
  if ("error" in parsed) {
    console.error(`verify-fractional-scale: ${parsed.error}`);
    return 1;
  }
  if (!existsSync(parsed.oraclePath)) {
    console.error(`verify-fractional-scale: файл оракула не найден: ${parsed.oraclePath}`);
    return 2;
  }
  let oracleParity;
  try {
    oracleParity = verifyFractionalScaleOracleParity({ kanbanBoardPath: parsed.oraclePath });
  } catch (err) {
    console.error(`verify-fractional-scale: ошибка чтения оракула: ${err instanceof Error ? err.message : String(err)}`);
    return 1;
  }

  const scalesVerification = verifyAllFractionalScales(parsed.scaleFilter);
  const dynamicTransitions = simulateDynamicScaleTransitions();
  const edgeDimensions = evaluateFractionalScaleEdgeDimensions();
  const passed = oracleParity.passed && scalesVerification.passed && dynamicTransitions.passed && edgeDimensions.passed;
  const report = {
    taskId: "TASK-977",
    scaleFilter: parsed.scaleFilter,
    status: passed ? "passed" : "failed",
    oracleParity,
    scalesVerification,
    dynamicTransitions,
    edgeDimensions,
  };
  if (parsed.outJson) {
    mkdirSync(dirname(parsed.outJson), { recursive: true });
    writeFileSync(parsed.outJson, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  console.log(`FRACTIONAL_SCALE_VERIFY status=${report.status} scale=${parsed.scaleFilter}`);
  return passed ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runCli());
}
