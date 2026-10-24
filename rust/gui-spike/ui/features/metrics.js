// @ts-check
/**
 * Метрики спайка: FPS/p95 кадра, RAM/потоки, ручной замер прокрутки.
 *
 * `aria-live` — только у счётчиков данных (`#member-count`, `#member-online`,
 * `.screen-title`). Панель `.metrics` (FPS/p95) в разметке помечена `aria-live` —
 * это снимается здесь: скринридер не должен слышать 60 обновлений в секунду.
 */
import { byId, fmt } from "./dom.js";
import { UIError, reportError } from "./errors.js";

/** Момент, когда скрипт начал исполняться (DOM уже разобран): готовность фронта. */
export const frontendReadyMs = Math.round(performance.now());

/** @typedef {{ frames: number, avgMs: number, p95Ms: number, fps: number, stalled?: boolean, lowSample?: boolean }} BenchResult */

/** @type {number[]} */
let frameTimes = [];
let lastFpsText = "";
let lastFpsPaint = 0;

/** @param {number} now */
function frameLoop(now) {
  frameTimes.push(now);
  if (frameTimes.length > 120) frameTimes.shift();
  const first = frameTimes[0] ?? now;
  const fps = frameTimes.length > 1 ? (1000 * (frameTimes.length - 1)) / (now - first) : 0;
  // Не чаще 2 раз в секунду и только при изменении: DOM-запись в каждом кадре
  // сама по себе стоит кадров.
  if (now - lastFpsPaint >= 500) {
    const text = fmt(fps);
    if (text !== lastFpsText) {
      lastFpsText = text;
      const node = byId("m-fps");
      if (node) node.textContent = text;
    }
    lastFpsPaint = now;
  }
  requestAnimationFrame(frameLoop);
}

async function pollProcessMetrics() {
  const tauri = /** @type {any} */ (window).__TAURI__;
  const rss = byId("m-rss");
  const threads = byId("m-threads");
  if (!tauri?.core?.invoke) {
    if (rss) rss.textContent = "n/a";
    if (threads) threads.textContent = "n/a";
    return;
  }
  try {
    const metrics = await tauri.core.invoke("process_metrics");
    if (rss) rss.textContent = `${Math.round(metrics.rss_kib / 1024)} МБ`;
    if (threads) threads.textContent = String(metrics.threads);
  } catch (error) {
    if (rss) rss.textContent = "err";
    reportError("IPC_FAILED", error);
  }
}

let benchRunning = false;

/**
 * Замер прокрутки: rAF-дельта кадров, p95 nearest-rank (как
 * `scripts/gui-spike-bench.mjs`), guard на occluded/software-рендер.
 * @param {HTMLElement} element
 * @param {number} [durationMs]
 * @returns {Promise<BenchResult | null>}
 */
export async function benchScroll(element, durationMs = 5000) {
  if (benchRunning) return null;
  benchRunning = true;
  try {
    const start = performance.now();
    /** @type {number[]} */
    const deltas = [];
    let last = start;
    let stalled = false;

    await new Promise((resolve) => {
      let settled = false;
      /** @param {unknown} _value */
      const finish = (_value) => {
        if (settled) return;
        settled = true;
        clearTimeout(guard);
        resolve(undefined);
      };
      // rAF не тикает, если окно occluded или рендер software: не зависаем навсегда.
      const guard = setTimeout(() => {
        stalled = true;
        finish(undefined);
      }, durationMs + 2000);
      /** @param {number} now */
      function step(now) {
        deltas.push(now - last);
        last = now;
        const progress = (now - start) / durationMs;
        const max = element.scrollHeight - element.clientHeight;
        element.scrollTop = Math.floor(max * Math.min(1, progress));
        if (progress < 1) requestAnimationFrame(step);
        else finish(undefined);
      }
      requestAnimationFrame(step);
    });

    deltas.shift();
    const sorted = [...deltas].sort((a, b) => a - b);
    const p95 = sorted.length === 0 ? 0 : sorted[Math.max(0, Math.ceil(0.95 * sorted.length) - 1)] ?? 0;
    const avg = deltas.length === 0 ? 0 : deltas.reduce((a, b) => a + b, 0) / deltas.length;
    /** @type {BenchResult} */
    const result = {
      frames: deltas.length,
      avgMs: +avg.toFixed(2),
      p95Ms: +p95.toFixed(2),
      fps: avg > 0 ? +(1000 / avg).toFixed(1) : 0,
      ...(stalled ? { stalled: true } : {}),
      // Слишком короткая выборка не доказывает FPS — помечаем явно, а не выдаём за факт.
      ...(!stalled && deltas.length < 30 ? { lowSample: true } : {}),
    };
    const p95Node = byId("m-p95");
    if (p95Node) p95Node.textContent = `${p95.toFixed(1)} мс`;
    const fpsNode = byId("m-fps");
    if (fpsNode) fpsNode.textContent = `${result.fps}`;
    console.log("[bench]", result);
    return result;
  } finally {
    benchRunning = false;
  }
}

/**
 * @param {{ onError?: (error: UIError) => void }} [options]
 */
export function startMetrics(options = {}) {
  const onError = options.onError ?? (() => {});
  const metricsPanel = document.querySelector(".metrics");
  // FPS/p95 — не live-region: снимаем aria-live с панели метрик (счётчики
  // данных живут отдельно и остаются polite).
  if (metricsPanel) metricsPanel.removeAttribute("aria-live");
  requestAnimationFrame(frameLoop);
  setInterval(pollProcessMetrics, 2000);
  pollProcessMetrics().catch((/** @type {unknown} */ error) => {
    onError(reportError("IPC_FAILED", error));
  });
  const benchButton = byId("bench");
  if (benchButton) {
    benchButton.addEventListener("click", () => {
      const active = document.querySelector(".screen.active");
      const scroller = active ? active.querySelector(".list, .emoji-grid, .settings") : null;
      if (!scroller) {
        onError(reportError("ELEMENT_MISSING", new UIError("ELEMENT_MISSING", "нет контейнера для замера")));
        return;
      }
      benchScroll(/** @type {HTMLElement} */ (scroller)).catch((/** @type {unknown} */ error) => {
        onError(reportError("ACTION_FAILED", error));
      });
    });
  }
}
