// @ts-check
/**
 * Контрактные и модульные тесты продвинутой WebRTC-диагностики и окна `RTC Internals HUD`
 * (`TASK-1011` / `TASK-1011A` / `TASK-1011B` — `RE-GAP-22`):
 * - SUB-1011-0: Статический и компиляционный контракт `rust/discord-gpui/src/voice/rtc_diagnostics.rs`
 *   и `scripts/verify-rtc-diag.mjs` (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде,
 *   лимиты `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк, чистый `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1011-1: Сверка карточек `TASK-1011`, `TASK-1011A`, `TASK-1011B` в `waves/kanban-board.json`,
 *   дизайн-токенов (`#1e1f22`, `#2b2d31`, `#ffffff`, `#23a55a`, `#3f4147`), геометрии `RTC Internals HUD`
 *   (`640px x 480px`, `padding: 16px`, `spacing: 10px`), `RTCP Collector` (`RingBuffer 120`),
 *   интерактивных состояний `rtc-graph` (`60fps graph -> data point tooltip -> zoomed metric`),
 *   `telemetry-probe` (`polling -> collect -> buffered`), `graph-canvas` (`rendering -> tooltip value -> zoom range`),
 *   оракула `waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`, `docs/re/voice-deep.json` и фикстур
 *   `waves/RW-26/research/fixtures/rtcp-voice-udp{3,4}-server.json`.
 * - SUB-1011-2: Парсинг бинарных пакетов RTCP Sender Report (`PT=200`, 28B clear / 60B SRTCP) и
 *   Receiver Report (`PT=201`, 32B clear / 52B SRTCP) в реальном времени, знаковое расширение 24-битного
 *   `cumulative_lost`, расчёт RTT по `LSR`/`DLSR` (`1/65536` с), межпакетного джиттера (`48 кГц -> мс`),
 *   дельта-потерь исходящих пакетов `outboundLossRate` и кольцевого буфера `RingBuffer 120` (60 с с шагом 500 мс).
 * - SUB-1011-3: Векторная геометрия 60 FPS графиков `RTC Internals HUD` (`640x480`, 3 кривые: RTT, джиттер,
 *   потери/битрейт, сетка шкал, зум `1x/2x/4x`, тултипы точек при наведении) и верификация DAVE MLS v1
 *   (`secureFramesProtocolVersion`, эпохи MLS, 30-значный `Voice Privacy Code` `6x5` из `epochAuthenticator`).
 * - SUB-1011-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные, граничные:
 *   Snowflake `0`/`u64::MAX`, пороги пинга `250/500` мс и потерь `5%/10%`, `RingBuffer 120`, `_pings 199`,
 *   2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`, бейдж `99+`; ошибки HTTP и `AVError`
 *   `2004..2017`; пустые; `skeleton`/`HTTP 202`; права `VIEW_CHANNEL`/`CONNECT`/`SPEAK`/`AGE_GATED_SPACES`;
 *   восстановление после 3 таймаутов пинга `4000 мс` и ротации UDP-сокета; маскирование секретов) и CLI `0 / 1 / 2`.
 * - SUB-1011-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/discord-gpui/src/voice/rtc_diagnostics.rs`
 *   и `scripts/verify-rtc-diag.mjs` по контракту `scripts/quality-gate.mjs`.
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
import {
  computeSparklinePoints as spikeComputeSparklinePoints,
  evaluatePingQuality as spikeEvaluatePingQuality,
  extractShortHostname as spikeExtractShortHostname,
  formatDaveTransitionSummary as spikeFormatDaveTransitionSummary,
  formatPopoutMetricsRu as spikeFormatPopoutMetricsRu,
  formatReadableVerificationCode as spikeFormatReadableVerificationCode,
  formatVoicePrivacyCodeChunks as spikeFormatVoicePrivacyCodeChunks,
  shouldShowVerificationCodeForChannel as spikeShouldShowVerificationCode,
} from "../../rust/gui-spike/ui/features/voice-diagnostics.js";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/discord-gpui/src/voice/rtc_diagnostics.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const SCRIPT_REL_PATH = "scripts/verify-rtc-diag.mjs";
const SCRIPT_PATH = join(ROOT, SCRIPT_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const RE_GAP_22_PATH = join(
  ROOT,
  "waves",
  "RW-40",
  "RE-AUDIT",
  "RE-GAP-22-VOICE-DIAGNOSTICS.md",
);
const VOICE_DEEP_JSON_PATH = join(ROOT, "docs", "re", "voice-deep.json");
const RTCP_UDP3_FIXTURE_PATH = join(
  ROOT,
  "waves",
  "RW-26",
  "research",
  "fixtures",
  "rtcp-voice-udp3-server.json",
);
const RTCP_UDP4_FIXTURE_PATH = join(
  ROOT,
  "waves",
  "RW-26",
  "research",
  "fixtures",
  "rtcp-voice-udp4-server.json",
);

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-gpui/src/voice/rtc_diagnostics.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1011: Продвинутая WebRTC диагностика и RTC Internals (rust/discord-gpui/src/voice/rtc_diagnostics.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "rtc-diagnostics-1011-"));
    testBinPath = join(workDir, "rtc_diagnostics_1011_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1011-0: модули rust/discord-gpui/src/voice/rtc_diagnostics.rs и scripts/verify-rtc-diag.mjs существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), запрещают unsafe и компилируются без предупреждений", () => {
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
      "В продуктовом коде rtc_diagnostics.rs запрещены .unwrap() и .expect()",
    );

    const rustSizeCheck = checkRustSourceSizeLimits(
      RUST_MODULE_REL_PATH,
      rustSource,
    );
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
    assert.equal(
      secretScanRust.length,
      0,
      "В Rust-модуле не должно быть захардкоженных секретов",
    );
    assert.equal(
      secretScanJs.length,
      0,
      "В JS-скрипте не должно быть захардкоженных секретов",
    );

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
      `Встроенные тесты rtc_diagnostics.rs должны проходить:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test result: ok\./);
  });

  it("SUB-1011-1: сверяет дизайн-токены, раскладку RTC Internals HUD (640x480), состояния rtc-graph/telemetry-probe/graph-canvas, константы RE-GAP-22 и фикстуры RTCP с независимыми оракулами", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      RTC_INTERNALS_HUD_LAYOUT,
      RTCP_COLLECTOR_LAYOUT,
      RTC_HUD_LAYOUT,
      RTC_DIAGNOSTICS_DESIGN_TOKENS,
      RTC_GRAPH_STATES,
      TELEMETRY_PROBE_STATES,
      GRAPH_CANVAS_STATES,
      RTC_PROTOCOL_CONSTANTS,
      resolveRtcGraphState,
      resolveTelemetryProbeState,
      resolveGraphCanvasState,
      verifyRtcDiagnosticsOracleParity,
    } = await import("../../scripts/verify-rtc-diag.mjs");

    // 1. Оракул: карточки TASK-1011, TASK-1011A, TASK-1011B в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1011 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1011");
    const card1011a = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1011A");
    const card1011b = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1011B");
    assert.ok(card1011, "Карточка TASK-1011 должна присутствовать в waves/kanban-board.json");
    assert.ok(card1011a, "Карточка TASK-1011A должна присутствовать в waves/kanban-board.json");
    assert.ok(card1011b, "Карточка TASK-1011B должна присутствовать в waves/kanban-board.json");

    assert.deepEqual(RTC_INTERNALS_HUD_LAYOUT, card1011.layout);
    assert.deepEqual(RTCP_COLLECTOR_LAYOUT, card1011a.layout);
    assert.deepEqual(RTC_HUD_LAYOUT, card1011b.layout);
    assert.deepEqual(RTC_DIAGNOSTICS_DESIGN_TOKENS, card1011.design_tokens);
    assert.deepEqual(RTC_GRAPH_STATES, card1011.interactive_states[0]);
    assert.deepEqual(TELEMETRY_PROBE_STATES, card1011a.interactive_states[0]);
    assert.deepEqual(GRAPH_CANVAS_STATES, card1011b.interactive_states[0]);

    // 2. Интерактивные состояния rtc-graph, telemetry-probe, graph-canvas
    assert.equal(resolveRtcGraphState({ hovered: false, zoomed: false }), "60fps graph");
    assert.equal(resolveRtcGraphState({ hovered: true, zoomed: false }), "data point tooltip");
    assert.equal(resolveRtcGraphState({ hovered: false, zoomed: true }), "zoomed metric");

    assert.equal(resolveTelemetryProbeState("default"), "polling");
    assert.equal(resolveTelemetryProbeState("hover"), "collect");
    assert.equal(resolveTelemetryProbeState("active"), "buffered");

    assert.equal(resolveGraphCanvasState("default"), "rendering");
    assert.equal(resolveGraphCanvasState("hover"), "tooltip value");
    assert.equal(resolveGraphCanvasState("active"), "zoom range");

    // 3. Проверка протокольных констант RE-GAP-22 и docs/re/voice-deep.json
    assert.equal(RTC_PROTOCOL_CONSTANTS.RTCP_SR_PT, 200);
    assert.equal(RTC_PROTOCOL_CONSTANTS.RTCP_RR_PT, 201);
    assert.equal(RTC_PROTOCOL_CONSTANTS.SRTCP_SR_WIRE_LEN, 60);
    assert.equal(RTC_PROTOCOL_CONSTANTS.SRTCP_RR_WIRE_LEN, 52);
    assert.equal(RTC_PROTOCOL_CONSTANTS.RING_BUFFER_CAPACITY, 120);
    assert.equal(RTC_PROTOCOL_CONSTANTS.RING_BUFFER_STEP_MS, 500);
    assert.equal(RTC_PROTOCOL_CONSTANTS.HUD_TARGET_FPS, 60);
    assert.equal(RTC_PROTOCOL_CONSTANTS.PING_FINE_MAX_MS, 250);
    assert.equal(RTC_PROTOCOL_CONSTANTS.PING_AVERAGE_MAX_MS, 500);
    assert.equal(RTC_PROTOCOL_CONSTANTS.LOSS_FINE_MAX_PCT, 5);
    assert.equal(RTC_PROTOCOL_CONSTANTS.LOSS_AVERAGE_MAX_PCT, 10);
    assert.equal(RTC_PROTOCOL_CONSTANTS.PRIVACY_CODE_DIGITS, 30);

    // 4. Полная сверка с независимыми оракулами
    const parity = verifyRtcDiagnosticsOracleParity({
      kanbanPath: KANBAN_BOARD_PATH,
      reGap22Path: RE_GAP_22_PATH,
      voiceDeepPath: VOICE_DEEP_JSON_PATH,
      rtcpUdp3Path: RTCP_UDP3_FIXTURE_PATH,
      rtcpUdp4Path: RTCP_UDP4_FIXTURE_PATH,
    });
    assert.equal(parity.passed, true, `Несовпадение с оракулами: ${parity.reasons.join("; ")}`);
    assert.equal(parity.udp3PacketCount, 12);
    assert.equal(parity.udp4PacketCount, 20);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1011_1_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1011-1 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1011_1_.*ok/);
  });

  it("SUB-1011-2: разбирает бинарные пакеты RTCP Sender/Receiver Report в реальном времени, считает RTT по LSR/DLSR, джиттер и дельту потерь в RingBuffer 120", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      buildRtcpSenderReportBytes,
      buildRtcpReceiverReportBytes,
      parseRtcpPacketBytes,
      computeRttFromReportBlockMs,
      computeOutboundLossRateDeltaPct,
      classifyRtcConnectionQuality,
      RtcpMetricsRingBuffer,
    } = await import("../../scripts/verify-rtc-diag.mjs");

    // 1. Сборка и разбор 28-байтного и 60-байтного SRTCP Sender Report (PT=200)
    const srBytes = buildRtcpSenderReportBytes({
      senderSsrc: 17843,
      ntpSec: 0x83aa7e80,
      ntpFrac: 0x80000000,
      rtpTimestamp: 48_000,
      packetCount: 250,
      octetCount: 30_000,
      srtcpIndex: 7,
    });
    assert.equal(srBytes.byteLength, 60);
    const parsedSr = parseRtcpPacketBytes(srBytes);
    assert.equal(parsedSr.kind, "SR");
    assert.equal(parsedSr.payloadType, 200);
    assert.equal(parsedSr.senderSsrc, 17843);
    assert.equal(parsedSr.senderInfo?.packetCount, 250);
    assert.equal(parsedSr.senderInfo?.octetCount, 30_000);
    assert.equal(parsedSr.trailerLen, 32);

    // 2. Сборка и разбор 52-байтного серверного SRTCP Receiver Report (PT=201) со знаковым cumulative_lost
    const rrBytes = buildRtcpReceiverReportBytes({
      senderSsrc: 17843,
      ssrc: 857040390,
      fractionLostRaw: 20, // 20 / 256 = 7.8125%
      cumulativeLost: -3128775,
      extendedHighestSeq: 2268601906,
      jitterTicks: 960, // 960 / 48 = 20 мс при 48 кГц Opus
      lastSr: 0x7e80_8000,
      delaySinceLastSr: 65_536, // 1.0 с = 65536 / 65536
      srtcpCounter: 1,
    });
    assert.equal(rrBytes.byteLength, 52);
    const parsedRr = parseRtcpPacketBytes(rrBytes);
    assert.equal(parsedRr.kind, "RR");
    assert.equal(parsedRr.payloadType, 201);
    assert.equal(parsedRr.senderSsrc, 17843);
    assert.equal(parsedRr.reports.length, 1);
    const block = parsedRr.reports[0];
    assert.ok(block);
    assert.equal(block.ssrc, 857040390);
    assert.equal(block.fractionLostRaw, 20);
    assert.equal(block.fractionLostPct, 7.8125);
    assert.equal(block.cumulativeLost, -3128775);
    assert.equal(block.jitterMs, 20);

    // 3. Расчёт RTT по компактному NTP: receiveNtp = 0x7e81_c000 (разница 1.25 с - DLSR 1.0 с = 0.25 с = 250 мс)
    const rttMs = computeRttFromReportBlockMs({
      receiveNtpCompact: 0x7e81_c000,
      lastSr: 0x7e80_8000,
      delaySinceLastSr: 65_536,
    });
    assert.equal(rttMs, 250);

    // 4. Расчёт дельты потерь исходящих пакетов (модуль 206959) и сверка классификации качества с эталоном
    assert.equal(
      computeOutboundLossRateDeltaPct(
        { packetsSent: 100, packetsLost: 2 },
        { packetsSent: 190, packetsLost: 12 },
      ),
      10,
    );
    for (const [ping, loss] of [
      [42, 0],
      [250, 5],
      [251, 2],
      [200, 6],
      [500, 10],
      [501, 0],
      [120, 10.5],
      [null, 0],
    ]) {
      assert.equal(
        classifyRtcConnectionQuality(true, ping, loss),
        spikeEvaluatePingQuality(ping, loss),
      );
    }

    // 5. Кольцевой буфер RingBuffer 120 (60 с с шагом 500 мс) с вытеснением FIFO
    const ring = new RtcpMetricsRingBuffer(120);
    for (let i = 0; i < 125; i += 1) {
      ring.pushSample({
        timeMs: 1_000 + i * 500,
        rttMs: 30 + (i % 20),
        jitterMs: 4 + (i % 5),
        lossPct: (i % 6) * 0.5,
        bitrateKbps: 64 + (i % 16),
      });
    }
    assert.equal(ring.size(), 120);
    assert.equal(ring.samples()[0]?.timeMs, 1_000 + 5 * 500);
    assert.equal(ring.samples()[119]?.timeMs, 1_000 + 124 * 500);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1011_2_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1011-2 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1011_2_.*ok/);
  });

  it("SUB-1011-3: строит 60 FPS кривые RTT/джиттера/битрейта для RTC Internals HUD (640x480) с тултипами/зумом и верифицирует DAVE MLS v1 (Voice Privacy Code 6x5)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      renderRtcHudFrame60Fps,
      verifyDaveMlsState,
      extractShortRtcHostname,
      formatRtcPopoutSummaryRu,
    } = await import("../../scripts/verify-rtc-diag.mjs");

    const samples = Array.from({ length: 60 }, (_, idx) => ({
      timeMs: 1_762_000_000_000 + idx * 500,
      rttMs: 40 + (idx % 15),
      jitterMs: 5 + (idx % 6),
      lossPct: idx === 30 ? 2.5 : 0.5,
      bitrateKbps: 64 + (idx % 32),
    }));

    // 1. Отрисовка кадра 60 FPS RTC Internals HUD (640x480, padding 16, spacing 10) с зумом и тултипом
    const frameDefault = renderRtcHudFrame60Fps(samples, {
      zoomFactor: 1,
      hoverIndex: null,
    });
    assert.equal(frameDefault.container, "RTC Internals HUD");
    assert.equal(frameDefault.widthPx, 640);
    assert.equal(frameDefault.heightPx, 480);
    assert.equal(frameDefault.paddingPx, 16);
    assert.equal(frameDefault.spacingPx, 10);
    assert.equal(frameDefault.targetFps, 60);
    assert.ok(frameDefault.frameBudgetMs <= 16.67);
    assert.equal(frameDefault.graphState, "60fps graph");
    assert.equal(frameDefault.canvasState, "rendering");
    assert.equal(frameDefault.rttCurve.points.length, 60);
    assert.equal(frameDefault.jitterCurve.points.length, 60);
    assert.equal(frameDefault.bitrateCurve.points.length, 60);
    assert.equal(frameDefault.gridLinesCount, 4);

    // Проверка наведения курсора (data point tooltip) и зума (zoomed metric)
    const frameHover = renderRtcHudFrame60Fps(samples, {
      zoomFactor: 1,
      hoverIndex: 10,
    });
    assert.equal(frameHover.graphState, "data point tooltip");
    assert.equal(frameHover.canvasState, "tooltip value");
    assert.ok(frameHover.tooltip);
    assert.match(frameHover.tooltip.textRu, /RTT: 50 мс/);
    assert.match(frameHover.tooltip.textRu, /Джиттер: 9 мс/);

    const frameZoomed = renderRtcHudFrame60Fps(samples, {
      zoomFactor: 2,
      hoverIndex: null,
    });
    assert.equal(frameZoomed.graphState, "zoomed metric");
    assert.equal(frameZoomed.canvasState, "zoom range");
    assert.equal(frameZoomed.rttCurve.points.length, 30);

    // Сверка спарклайна 258x80 с эталоном voice-diagnostics.js
    const pingValues = samples.slice(0, 10).map((s) => s.rttMs);
    const spikePoints = spikeComputeSparklinePoints(pingValues, 258, 80);
    assert.deepEqual(frameDefault.sparkline258x80.slice(0, 10), spikePoints);

    // 2. Верификация DAVE MLS v1 и 30-значного Voice Privacy Code (6 групп по 5 цифр)
    const authenticatorBytes = new Uint8Array(30);
    for (let i = 0; i < 30; i += 1) {
      authenticatorBytes[i] = (i * 17 + 11) & 0xff;
    }
    const spikeCode = spikeFormatReadableVerificationCode(authenticatorBytes, 30, 5);
    const daveStatus = verifyDaveMlsState({
      channelId: "120",
      activeVoiceChannelId: "120",
      isStageVoiceChannel: false,
      protocolVersion: 1,
      epoch: 4,
      transitionCount: 3,
      rosterSize: 5,
      timeToDaveGroupMs: 142,
      authenticatorBytes,
    });
    assert.equal(daveStatus.isActive, true);
    assert.equal(daveStatus.shouldShowVerificationCode, true);
    assert.equal(
      daveStatus.shouldShowVerificationCode,
      spikeShouldShowVerificationCode({
        channelId: "120",
        activeVoiceChannelId: "120",
        isStageVoiceChannel: false,
        secureFramesVersion: 1,
      }),
    );
    assert.deepEqual(daveStatus.privacyCodeChunks, spikeCode.chunks);
    assert.deepEqual(
      spikeFormatVoicePrivacyCodeChunks(daveStatus.privacyCodeFormatted),
      spikeCode.chunks,
    );
    assert.deepEqual(
      daveStatus.transitionSummary,
      spikeFormatDaveTransitionSummary({
        protocolVersion: 1,
        transitionCount: 3,
        rosterSize: 5,
        timeToDaveGroupMs: 142,
      }),
    );

    // 3. Сверка короткого имени RTC-сервера и русских строк попапа с эталоном
    assert.equal(
      extractShortRtcHostname("c-fra19-4da7fe5f.discord.media:2053"),
      spikeExtractShortHostname("c-fra19-4da7fe5f.discord.media:2053"),
    );
    const popoutInput = {
      shortHostname: "c-fra19-4da7fe5f",
      averagePingMs: 42,
      lastPingMs: 45,
      outboundLossRatePct: 0.5,
      isEndToEndEncrypted: true,
      secureFramesProtocolVersion: 1,
    };
    assert.deepEqual(
      formatRtcPopoutSummaryRu(popoutInput),
      spikeFormatPopoutMetricsRu(popoutInput),
    );

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1011_3_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1011-3 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1011_3_.*ok/);
  });

  it("SUB-1011-4: покрывает все 9 обязательных измерений TEST-POLICY.md, маскирует секреты и проверяет CLI scripts/verify-rtc-diag.mjs (коды 0 / 1 / 2)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      evaluateRtcDiagNineDimensions,
      redactRtcDiagnosticsTelemetry,
      runRtcDiagCli,
    } = await import("../../scripts/verify-rtc-diag.mjs");

    // 1. Все 9 обязательных измерений TEST-POLICY.md
    const dimReport = evaluateRtcDiagNineDimensions();
    assert.equal(
      dimReport.passed,
      true,
      `Сбой в 9 измерениях TEST-POLICY.md: ${dimReport.failures.join("; ")}`,
    );
    assert.equal(dimReport.dimensionsChecked, 9);

    // 2. Маскирование токенов и ключей шифрования в логах RTC диагностики
    const rawLog =
      'RTCDiag endpoint=c-fra19.discord.media auth="Bearer mfa.secret_rtc_token_1011" secret_key=0102030405060708090a token=MTIzNDU2Nzg5MDEyMzQ1Njc4.GhIjKl.MnOpQrStUvWxYz0123456789abcdef';
    const redacted = redactRtcDiagnosticsTelemetry(rawLog);
    assert.doesNotMatch(redacted, /mfa\.secret_rtc_token_1011/);
    assert.doesNotMatch(redacted, /0102030405060708090a/);
    assert.doesNotMatch(redacted, /GhIjKl\.MnOpQrStUvWxYz0123456789abcdef/);
    assert.match(redacted, /\[REDACTED\]/);

    // 3. Проверка CLI scripts/verify-rtc-diag.mjs (коды выхода 0 / 1 / 2)
    const outJson = join(workDir, "rtc-diag-report.json");
    const outMd = join(workDir, "RTC-DIAG-REPORT.md");
    assert.equal(runRtcDiagCli(["--help"]), 0);
    const cliOk = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--out-json", outJson, "--out-md", outMd],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliOk.status, 0, `CLI завершился с ошибкой: ${cliOk.stderr}`);
    assert.ok(existsSync(outJson));
    assert.ok(existsSync(outMd));

    const cliBadArg = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--unknown-flag"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliBadArg.status, 1);

    const cliMissingOracle = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--oracle", "/nonexistent/missing-kanban-1011.json"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliMissingOracle.status, 2);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1011_4_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1011-4 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1011_4_.*ok/);
  });

  it("SUB-1011-5: убивает 5/5 контрольных мутаций в rust/discord-gpui/src/voice/rtc_diagnostics.rs и scripts/verify-rtc-diag.mjs (100% mutation kill rate)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalRustMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutationSpecs = [
      {
        id: "MUT-1011-RS-01",
        operator: "PingFineMaxThresholdTamper",
        from: "pub const PING_FINE_MAX_MS: u64 = 250;",
        to: "pub const PING_FINE_MAX_MS: u64 = 50;",
      },
      {
        id: "MUT-1011-RS-02",
        operator: "RingBufferCapacityTamper",
        from: "pub const RTC_METRICS_RING_CAPACITY: usize = 120;",
        to: "pub const RTC_METRICS_RING_CAPACITY: usize = 10;",
      },
      {
        id: "MUT-1011-RS-03",
        operator: "RtcpRrPayloadTypeTamper",
        from: "pub const RTCP_PT_RR: u8 = 201;",
        to: "pub const RTCP_PT_RR: u8 = 202;",
      },
      {
        id: "MUT-1011-RS-04",
        operator: "DavePrivacyCodeLengthTamper",
        from: "pub const PRIVACY_CODE_TOTAL_DIGITS: usize = 30;",
        to: "pub const PRIVACY_CODE_TOTAL_DIGITS: usize = 15;",
      },
      {
        id: "MUT-1011-RS-05",
        operator: "RtcGraphHoverStateTamper",
        from: 'Self::Hover => "data point tooltip",',
        to: 'Self::Hover => "60fps graph",',
      },
    ];

    const rustMutants = rustMutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в rtc_diagnostics.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "rtc_diagnostics.rs",
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

    const afterRustMd5 = createHash("md5")
      .update(readFileSync(RUST_MODULE_PATH, "utf8"))
      .digest("hex");
    assert.equal(
      afterRustMd5,
      originalRustMd5,
      "MD5 исходного файла rtc_diagnostics.rs не должен измениться",
    );
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
        id: "MUT-1011-JS-01",
        operator: "PingFineMaxThresholdTamper",
        from: "PING_FINE_MAX_MS: 250,",
        to: "PING_FINE_MAX_MS: 50,",
      },
      {
        id: "MUT-1011-JS-02",
        operator: "RtcpRrPayloadTypeTamper",
        from: "RTCP_RR_PT: 201,",
        to: "RTCP_RR_PT: 205,",
      },
      {
        id: "MUT-1011-JS-03",
        operator: "RtcGraphHoverStateTamper",
        from: 'hover: "data point tooltip",',
        to: 'hover: "idle",',
      },
      {
        id: "MUT-1011-JS-04",
        operator: "RingBufferCapacityTamper",
        from: "RING_BUFFER_CAPACITY: 120,",
        to: "RING_BUFFER_CAPACITY: 8,",
      },
      {
        id: "MUT-1011-JS-05",
        operator: "DaveStageChannelCheckBypass",
        from: "if (params.isStageVoiceChannel === true) return false;",
        to: "if (false && params.isStageVoiceChannel === true) return false;",
      },
    ];

    const jsMutants = jsMutationSpecs.map((spec) => {
      assert.ok(
        originalJs.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в verify-rtc-diag.mjs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "verify-rtc-diag.mjs",
        operator: spec.operator,
        mutatedSource: originalJs.replace(spec.from, spec.to),
      };
    });

    const jsGate = evaluateMutationGate(jsMutants, (mutant, index) => {
      const mutantJsPath = join(workDir, `mutant_rtc_diag_${index + 1}.mjs`);
      const harnessPath = join(
        workDir,
        `mutant_rtc_diag_harness_${index + 1}.mjs`,
      );
      writeFileSync(mutantJsPath, mutant.mutatedSource, "utf8");
      writeFileSync(
        harnessPath,
        [
          'import assert from "node:assert/strict";',
          `import { RTC_GRAPH_STATES, RTC_PROTOCOL_CONSTANTS, classifyRtcConnectionQuality, buildRtcpReceiverReportBytes, parseRtcpPacketBytes, RtcpMetricsRingBuffer, verifyDaveMlsState } from ${JSON.stringify(mutantJsPath)};`,
          'assert.equal(RTC_GRAPH_STATES.hover, "data point tooltip");',
          "assert.equal(RTC_PROTOCOL_CONSTANTS.PING_FINE_MAX_MS, 250);",
          "assert.equal(RTC_PROTOCOL_CONSTANTS.RING_BUFFER_CAPACITY, 120);",
          'assert.equal(classifyRtcConnectionQuality(true, 200, 1), "fine");',
          "const rr = buildRtcpReceiverReportBytes({ senderSsrc: 17843, ssrc: 1, fractionLostRaw: 0, cumulativeLost: 0, extendedHighestSeq: 1, jitterTicks: 48, lastSr: 0, delaySinceLastSr: 0 });",
          'assert.equal(parseRtcpPacketBytes(rr).kind, "RR");',
          "assert.equal(parseRtcpPacketBytes(rr).payloadType, 201);",
          "const rb = new RtcpMetricsRingBuffer();",
          "for (let i = 0; i < 120; i += 1) rb.pushSample({ timeMs: i, rttMs: 10, jitterMs: 1, lossPct: 0, bitrateKbps: 64 });",
          "assert.equal(rb.size(), 120);",
          "assert.equal(verifyDaveMlsState({ channelId: '1', activeVoiceChannelId: '1', isStageVoiceChannel: true, protocolVersion: 1 }).shouldShowVerificationCode, false);",
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
