// @ts-check
/**
 * Контрактные и модульные тесты контроля перегрузки сети WebRTC BWE (GCC, адаптация
 * видеобитрейта 500..8000 kbps по RTCP REMB и TWCC) — TASK-982:
 * - SUB-982-0: Статический и структурный контракт `rust/chord-voice/src/congestion.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, лимиты `quality-gate.mjs`
 *   < 50 строк на функцию и < 800 строк на модуль, `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, `secret-scan.mjs`).
 * - SUB-982-1: Сверка констант BWE, дизайн-токенов и состояний `bwe-graph` (`probing`,
 *   `limit bandwidth`, `adapted bitrate`, диапазон `500-8000 kbps`) с независимыми оракулами
 *   `waves/kanban-board.json`, `docs/re/native-re.md`, `docs/re/voice-deep.md` и
 *   `fixtures/voice/rtc-frames.jsonl`.
 * - SUB-982-2: Формирование и разбор пакетов RTCP REMB (`PT=206, FMT=15, "REMB"`, 6-бит экспонента,
 *   18-бит мантисса, список SSRC), граничные и негативные сценарии.
 * - SUB-982-3: Формирование и разбор пакетов RTCP TWCC (`PT=205, FMT=15`,
 *   `draft-holmer-rmcat-transport-wide-cc-extensions-01`, Run-Length и Status-Vector чанки,
 *   дельты по 250 мкс, переход sequence `65535 -> 0`, расчёт градиента задержки).
 * - SUB-982-4: Плавная адаптация видеобитрейта `500..8000 kbps` (GCC Trendline + AIMD, шаг
 *   снижения <= 15% за тик) без сброса соединения (`connectionAlive = true`, `reconnectCount = 0`),
 *   покрытие 9 измерений `TEST-POLICY.md` и проверка CLI `scripts/verify-bwe.mjs` (коды `0 / 1 / 2`).
 * - SUB-982-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
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
const MODULE_REL_PATH = "rust/chord-voice/src/congestion.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const SCRIPT_PATH = join(ROOT, "scripts", "verify-bwe.mjs");
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const NATIVE_RE_PATH = join(ROOT, "docs", "re", "native-re.md");
const VOICE_DEEP_PATH = join(ROOT, "docs", "re", "voice-deep.md");
const RTC_FRAMES_PATH = join(ROOT, "fixtures", "voice", "rtc-frames.jsonl");

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/chord-voice/src/congestion.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-982: Контроль перегрузки WebRTC BWE (GCC, REMB/TWCC адаптация 500..8000 kbps)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "webrtc-bwe-982-"));
    testBinPath = join(workDir, "congestion_982_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-982-0: модуль rust/chord-voice/src/congestion.rs существует, проходит rustfmt 2024, лимиты размера (<50 строк на fn, <800 на файл) и компилируется без предупреждений", () => {
    assert.ok(
      existsSync(MODULE_PATH),
      `Модуль ${MODULE_REL_PATH} должен существовать`,
    );

    const source = readFileSync(MODULE_PATH, "utf8");
    assert.match(
      source,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "Модуль обязан содержать #![forbid(unsafe_code)]",
    );

    const prodSection = source.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продакшен-коде модуля запрещены .unwrap() и .expect()",
    );

    const sizeCheck = checkRustSourceSizeLimits(MODULE_REL_PATH, source);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(sizeCheck.fileLines > 150 && sizeCheck.fileLines < 800);

    const secretFindings = scanTextForSecrets(source, MODULE_REL_PATH);
    assert.equal(secretFindings.length, 0, "В исходнике не должно быть незамаскированных секретов");

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check не прошёл: ${fmtCheck.stderr || fmtCheck.stdout}`,
    );

    const compiled = compileRustModuleTests(MODULE_PATH, testBinPath);
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
  });

  it("SUB-982-1: сверяет константы BWE (500..8000 kbps), дизайн-токены и интерактивные состояния bwe-graph с RE-оракулами и waves/kanban-board.json", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");
    const {
      BWE_DESIGN_TOKENS,
      BWE_GRAPH_STATES,
      BWE_DEFAULT_CONFIG,
      resolveBweGraphState,
      verifyBweOracleParity,
    } = await import("../../scripts/verify-bwe.mjs");

    // 1. Сверка с карточкой TASK-982 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card982 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-982");
    assert.ok(card982, "Карточка TASK-982 должна присутствовать в waves/kanban-board.json");
    assert.equal(card982.layout.container, "Congestion Control");
    assert.equal(card982.layout.dimensions, "500-8000 kbps");
    assert.deepEqual(BWE_DESIGN_TOKENS, card982.design_tokens);
    assert.deepEqual(BWE_GRAPH_STATES, card982.interactive_states[0]);

    // 2. Проверка диапазона битрейта 500..8000 kbps и констант протокола
    assert.equal(BWE_DEFAULT_CONFIG.minBitrateKbps, 500);
    assert.equal(BWE_DEFAULT_CONFIG.maxBitrateKbps, 8_000);
    assert.equal(BWE_DEFAULT_CONFIG.startBitrateKbps, 2_500);
    assert.equal(BWE_DEFAULT_CONFIG.fallbackMinBitrateBps, 150_000);
    assert.equal(BWE_DEFAULT_CONFIG.maxSingleStepDropRatio, 0.15);
    assert.equal(BWE_DEFAULT_CONFIG.rtcpRtpfbPt, 205);
    assert.equal(BWE_DEFAULT_CONFIG.rtcpPsfbPt, 206);
    assert.equal(BWE_DEFAULT_CONFIG.rtcpFmtRemb, 15);
    assert.equal(BWE_DEFAULT_CONFIG.rtcpFmtTwcc, 15);
    assert.equal(
      BWE_DEFAULT_CONFIG.twccExtensionUri,
      "http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01",
    );
    assert.equal(
      BWE_DEFAULT_CONFIG.trendlineExperimentSymbol,
      "kDiscordBandwidthEstimationExperimentTrendlineWindowDuration",
    );

    // 3. Проверка интерактивных состояний элемента bwe-graph (probing / limit bandwidth / adapted bitrate)
    assert.deepEqual(resolveBweGraphState({ mode: "probing", hovered: false }), {
      element: "bwe-graph",
      state: "default",
      label: "probing",
    });
    assert.deepEqual(resolveBweGraphState({ mode: "probing", hovered: true }), {
      element: "bwe-graph",
      state: "hover",
      label: "limit bandwidth",
    });
    assert.deepEqual(resolveBweGraphState({ mode: "adapted", hovered: false }), {
      element: "bwe-graph",
      state: "active",
      label: "adapted bitrate",
    });

    // 4. Сверка с RE-оракулами docs/re/native-re.md, docs/re/voice-deep.md и fixtures/voice/rtc-frames.jsonl
    const parity = verifyBweOracleParity({
      kanbanPath: KANBAN_BOARD_PATH,
      nativeRePath: NATIVE_RE_PATH,
      voiceDeepPath: VOICE_DEEP_PATH,
      rtcFramesPath: RTC_FRAMES_PATH,
    });
    assert.equal(parity.passed, true, `Нарушение паритета с оракулами: ${parity.reasons.join("; ")}`);
    assert.equal(parity.videoSsrc, 17_844);
    assert.equal(parity.rtxSsrc, 17_845);
    assert.equal(parity.av1PayloadType, 101);

    const run = runCompiledRustTests(testBinPath, "test_sub_982_1_");
    assert.equal(run.status, 0, `Тесты SUB-982-1 упали:\n${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /test tests::test_sub_982_1_.*ok/);
  });

  it("SUB-982-2: корректно кодирует и разбирает пакеты RTCP REMB (PT=206, FMT=15, 'REMB', 6-бит exp + 18-бит mantissa) и отсекает повреждённые пакеты", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");
    const {
      encodeRembBitrate,
      decodeRembBitrate,
      buildRembPacket,
      parseRembPacket,
    } = await import("../../scripts/verify-bwe.mjs");

    // 1. Проверка кодирования/декодирования битрейта REMB для 500 kbps, 2500 kbps и 8000 kbps
    for (const targetBps of [0, 500_000, 2_500_000, 8_000_000]) {
      const { exponent, mantissa } = encodeRembBitrate(targetBps);
      assert.ok(exponent >= 0 && exponent <= 63);
      assert.ok(mantissa >= 0 && mantissa <= 0x3_ffff);
      const decoded = decodeRembBitrate(exponent, mantissa);
      assert.ok(decoded !== null);
      if (targetBps === 0) {
        assert.equal(decoded, 0);
      } else {
        assert.ok(Math.abs(decoded - targetBps) / targetBps < 0.001);
      }
    }

    // 2. Сборка и разбор бинарного пакета RTCP REMB (PT=206, FMT=15)
    const wire = buildRembPacket({
      senderSsrc: 9_208,
      bitrateBps: 2_500_000,
      ssrcs: [17_844, 17_845],
    });
    assert.equal(wire.length, 28); // 20 байт базовый заголовок REMB + 2 * 4 байта SSRC
    assert.equal(wire[0], 0x8f); // V=2, P=0, FMT=15
    assert.equal(wire[1], 206); // PT=206 (PSFB)
    assert.equal(wire.subarray(12, 16).toString("ascii"), "REMB");

    const parsed = parseRembPacket(wire);
    assert.equal(parsed.senderSsrc, 9_208);
    assert.equal(parsed.mediaSsrc, 0);
    assert.deepEqual(parsed.ssrcs, [17_844, 17_845]);
    assert.ok(Math.abs(parsed.bitrateBps - 2_500_000) <= 64);

    // 3. Негативные сценарии REMB (обрезанный пакет, неверная сигнатура, несовпадение длины SSRC)
    assert.throws(() => parseRembPacket(wire.subarray(0, 12)), /Truncated|короткий/i);
    const badMagic = Buffer.from(wire);
    badMagic.write("XXXX", 12, "ascii");
    assert.throws(() => parseRembPacket(badMagic), /REMB|идентификатор/i);

    const badCount = Buffer.from(wire);
    badCount[16] = 99; // num_ssrc не совпадает с фактической длиной пакета
    assert.throws(() => parseRembPacket(badCount), /length|длин/i);

    const run = runCompiledRustTests(testBinPath, "test_sub_982_2_");
    assert.equal(run.status, 0, `Тесты SUB-982-2 упали:\n${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /test tests::test_sub_982_2_.*ok/);
  });

  it("SUB-982-3: корректно кодирует и разбирает пакеты RTCP TWCC (PT=205, FMT=15), обрабатывает переход sequence 65535 -> 0 и вычисляет градиенты задержки", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");
    const {
      buildTwccPacket,
      parseTwccPacket,
    } = await import("../../scripts/verify-bwe.mjs");

    // 1. Формирование пакета RTCP TWCC с переходом sequence через 65535 -> 0, SmallDelta, LargeOrNegativeDelta и NotReceived
    const twccWire = buildTwccPacket({
      senderSsrc: 9_208,
      mediaSsrc: 17_844,
      baseSeq: 65_534,
      referenceTime64ms: 1_000,
      fbPktCount: 7,
      packets: [
        { sequence: 65_534, received: true, recvDeltaUs: 20_000 }, // 20 ms (SmallDelta = 80 ticks * 250 us)
        { sequence: 65_535, received: true, recvDeltaUs: 25_500 }, // 25.5 ms (SmallDelta = 102 ticks)
        { sequence: 0, received: false, recvDeltaUs: 0 }, // потеря пакета на границе u16::MAX -> 0
        { sequence: 1, received: true, recvDeltaUs: 70_000 }, // 70 ms (LargeOrNegativeDelta > 63.75 ms)
        { sequence: 2, received: true, recvDeltaUs: -2_000 }, // -2 ms переупорядочивание (LargeOrNegativeDelta < 0)
      ],
    });

    assert.equal(twccWire[0], 0x8f); // V=2, P=0, FMT=15
    assert.equal(twccWire[1], 205); // PT=205 (RTPFB)
    assert.equal(twccWire.length % 4, 0, "TWCC пакет обязан быть выровнен по 32-битной границе");

    const parsedTwcc = parseTwccPacket(twccWire, 20_000);
    assert.equal(parsedTwcc.senderSsrc, 9_208);
    assert.equal(parsedTwcc.mediaSsrc, 17_844);
    assert.equal(parsedTwcc.baseSeq, 65_534);
    assert.equal(parsedTwcc.packetStatusCount, 5);
    assert.equal(parsedTwcc.referenceTime64ms, 1_000);
    assert.equal(parsedTwcc.fbPktCount, 7);
    assert.equal(parsedTwcc.feedbacks.length, 5);

    assert.equal(parsedTwcc.feedbacks[0]?.sequence, 65_534);
    assert.equal(parsedTwcc.feedbacks[0]?.received, true);
    assert.equal(parsedTwcc.feedbacks[0]?.recvDeltaUs, 20_000);

    assert.equal(parsedTwcc.feedbacks[1]?.sequence, 65_535);
    assert.equal(parsedTwcc.feedbacks[1]?.received, true);
    assert.equal(parsedTwcc.feedbacks[1]?.recvDeltaUs, 25_500);
    assert.equal(parsedTwcc.feedbacks[1]?.delayGradientMs, 5.5);

    assert.equal(parsedTwcc.feedbacks[2]?.sequence, 0);
    assert.equal(parsedTwcc.feedbacks[2]?.received, false);
    assert.equal(parsedTwcc.feedbacks[2]?.recvDeltaUs, null);

    assert.equal(parsedTwcc.feedbacks[3]?.sequence, 1);
    assert.equal(parsedTwcc.feedbacks[3]?.received, true);
    assert.equal(parsedTwcc.feedbacks[3]?.recvDeltaUs, 70_000);

    assert.equal(parsedTwcc.feedbacks[4]?.sequence, 2);
    assert.equal(parsedTwcc.feedbacks[4]?.received, true);
    assert.equal(parsedTwcc.feedbacks[4]?.recvDeltaUs, -2_000);

    // 2. Негативные сценарии TWCC
    assert.throws(() => parseTwccPacket(twccWire.subarray(0, 10), 20_000), /Truncated|короткий/i);

    const run = runCompiledRustTests(testBinPath, "test_sub_982_3_");
    assert.equal(run.status, 0, `Тесты SUB-982-3 упали:\n${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /test tests::test_sub_982_3_.*ok/);
  });

  it("SUB-982-4: плавно адаптирует битрейт в диапазоне 500..8000 kbps по REMB и TWCC без сброса соединения и проверяет CLI scripts/verify-bwe.mjs", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");
    const {
      GccCongestionController,
      runBweSimulationScenario,
      evaluateBweVerificationResult,
      redactBweSecrets,
    } = await import("../../scripts/verify-bwe.mjs");

    // 1. Проверка плавного роста (probing) от 2500 до 8000 kbps при отсутствии перегрузки
    const controller = new GccCongestionController({
      minBitrateKbps: 500,
      maxBitrateKbps: 8_000,
      startBitrateKbps: 2_500,
    });
    assert.equal(controller.snapshot().currentBitrateKbps, 2_500);
    assert.equal(controller.snapshot().interactiveState, "probing");
    assert.equal(controller.snapshot().connectionAlive, true);

    for (let step = 0; step < 25; step += 1) {
      controller.onTwccFeedback({
        baseSeq: step * 4,
        packets: [
          { sequence: step * 4, received: true, recvDeltaUs: 20_000, sendDeltaUs: 20_000 },
          { sequence: step * 4 + 1, received: true, recvDeltaUs: 20_000, sendDeltaUs: 20_000 },
          { sequence: step * 4 + 2, received: true, recvDeltaUs: 20_000, sendDeltaUs: 20_000 },
          { sequence: step * 4 + 3, received: true, recvDeltaUs: 20_000, sendDeltaUs: 20_000 },
        ],
      });
    }
    assert.equal(controller.snapshot().currentBitrateKbps, 8_000);
    assert.equal(controller.snapshot().resolutionTier.label, "1080p60");

    // 2. Перегрузка по TWCC (градиент задержки +22 мс на пакет): плавное снижение без обрыва (шаг <= 15%)
    let prevBitrate = controller.snapshot().currentBitrateKbps;
    for (let step = 0; step < 6; step += 1) {
      const snap = controller.onTwccFeedback({
        baseSeq: 200 + step * 4,
        packets: [
          { sequence: 200 + step * 4, received: true, recvDeltaUs: 42_000, sendDeltaUs: 20_000 },
          { sequence: 200 + step * 4 + 1, received: true, recvDeltaUs: 42_000, sendDeltaUs: 20_000 },
          { sequence: 200 + step * 4 + 2, received: true, recvDeltaUs: 42_000, sendDeltaUs: 20_000 },
          { sequence: 200 + step * 4 + 3, received: true, recvDeltaUs: 42_000, sendDeltaUs: 20_000 },
        ],
      });
      assert.ok(snap.currentBitrateKbps < prevBitrate, "Битрейт должен снижаться при перегрузке");
      const dropRatio = (prevBitrate - snap.currentBitrateKbps) / prevBitrate;
      assert.ok(
        dropRatio <= 0.1501,
        `Снижение за один шаг должно быть плавным (<= 15%), получено ${(dropRatio * 100).toFixed(2)}%`,
      );
      assert.equal(snap.connectionAlive, true, "Соединение не должно сбрасываться");
      assert.equal(snap.reconnectCount, 0, "Реконнектов быть не должно");
      prevBitrate = snap.currentBitrateKbps;
    }

    // 3. Ограничение по RTCP REMB до 500 kbps (и ниже пола — 150 kbps клэмпится к 500 kbps)
    for (let step = 0; step < 20; step += 1) {
      controller.onRembBitrateBps(150_000);
    }
    const floorSnap = controller.snapshot();
    assert.equal(floorSnap.currentBitrateKbps, 500);
    assert.equal(floorSnap.rembCapKbps, 500);
    assert.equal(floorSnap.connectionAlive, true);
    assert.equal(floorSnap.reconnectCount, 0);
    assert.equal(floorSnap.interactiveState, "adapted bitrate");
    assert.equal(floorSnap.resolutionTier.label, "480p20");

    // 4. Сквозной сценарий симуляции и проверка вердикта
    const simResult = runBweSimulationScenario();
    const verdict = evaluateBweVerificationResult(simResult);
    assert.equal(verdict.passed, true, `Сценарий BWE должен проходить: ${verdict.reasons.join("; ")}`);
    assert.equal(simResult.connectionDropped, false);
    assert.equal(simResult.reconnectCount, 0);
    assert.equal(simResult.minObservedBitrateKbps, 500);
    assert.equal(simResult.maxObservedBitrateKbps, 8_000);

    // 5. Маскирование секретов в логах
    const redacted = redactBweSecrets(
      'secret_key="0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20" token="mfa.secret982"',
    );
    assert.ok(redacted.includes("[REDACTED]"));
    assert.ok(redacted.includes("mfa.***"));

    // 6. Проверка CLI scripts/verify-bwe.mjs (коды выхода 0, 1, 2)
    const outJson = join(workDir, "bwe-report.json");
    const outMd = join(workDir, "BWE-REPORT.md");
    const cliOk = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--out-json", outJson, "--out-md", outMd],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliOk.status, 0, `CLI завершился с ошибкой: ${cliOk.stderr}`);
    assert.ok(existsSync(outJson));
    assert.ok(existsSync(outMd));

    const badArg = spawnSync(process.execPath, [SCRIPT_PATH, "--invalid-flag"], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(badArg.status, 1);

    const missingOracle = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--oracle", "/nonexistent/missing-kanban-982.json"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(missingOracle.status, 2);

    const run = runCompiledRustTests(testBinPath, "test_sub_982_4_");
    assert.equal(run.status, 0, `Тесты SUB-982-4 упали:\n${run.stdout}\n${run.stderr}`);
    assert.match(run.stdout, /test tests::test_sub_982_4_.*ok/);
  });

  it("SUB-982-5: убивает 5/5 контрольных мутаций модуля rust/chord-voice/src/congestion.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(MODULE_PATH), "Исходник модуля должен существовать");
    const source = readFileSync(MODULE_PATH, "utf8");

    const mutationSpecs = [
      {
        id: "MUT-congestion-01",
        operator: "MinBitrateFloorBypass",
        from: "pub const MIN_VIDEO_BITRATE_KBPS: u32 = 500;",
        to: "pub const MIN_VIDEO_BITRATE_KBPS: u32 = 100;",
      },
      {
        id: "MUT-congestion-02",
        operator: "MaxBitrateCeilingBypass",
        from: "pub const MAX_VIDEO_BITRATE_KBPS: u32 = 8_000;",
        to: "pub const MAX_VIDEO_BITRATE_KBPS: u32 = 12_000;",
      },
      {
        id: "MUT-congestion-03",
        operator: "RembMagicValidationBypass",
        from: "if bytes.get(12..16) != Some(&RTCP_REMB_MAGIC) {",
        to: "if false && bytes.get(12..16) != Some(&RTCP_REMB_MAGIC) {",
      },
      {
        id: "MUT-congestion-04",
        operator: "TwccOveruseThresholdInversion",
        from: "if self.smoothed_gradient_ms > self.overuse_threshold_ms || loss_pct > 10.0 {",
        to: "if self.smoothed_gradient_ms < -999.0 {",
      },
      {
        id: "MUT-congestion-05",
        operator: "SmoothStepDownLimitBypass",
        from: "let min_allowed = (self.current_bitrate_kbps * 85) / 100;",
        to: "let min_allowed = 0;",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        source.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в исходнике: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "congestion.rs",
        operator: spec.operator,
        mutatedSource: source.replace(spec.from, spec.to),
      };
    });

    const gate = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutantSrcPath = join(workDir, `mutant_${idx + 1}.rs`);
      const mutantBinPath = join(workDir, `mutant_${idx + 1}_bin`);
      writeFileSync(mutantSrcPath, mutant.mutatedSource, "utf8");

      const compiled = compileRustModuleTests(mutantSrcPath, mutantBinPath);
      if (compiled.status !== 0) {
        return { killed: true, reason: "compilation_error" };
      }
      const run = runCompiledRustTests(mutantBinPath);
      if (run.status !== 0) {
        return { killed: true, reason: "test_assertion_failed" };
      }
      return { killed: false, reason: "mutant_survived" };
    });

    assert.equal(
      gate.passed,
      true,
      `Мутационный гейт не пройден (выжили: ${gate.survivors.join(", ")})`,
    );
    assert.equal(gate.killed, 5);
    assert.equal(gate.total, 5);
    assert.equal(gate.killRate, 1.0);
  });
});
