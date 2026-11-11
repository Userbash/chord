// @ts-check
/**
 * Контрактные и модульные тесты стенда эмуляции WAN профилей (150ms RTT, 5% loss, 30ms jitter),
 * валидации RTCP NACK / RTP RTX и устойчивости к потерям Opus PLC — TASK-978:
 * - SUB-978-1: Профили сети WAN 3G/LTE (150 мс RTT, 5% потерь пакетов, 30 мс джиттер),
 *   UI-контракт эмулятора (loss-slider, токены #1e1f22/#2b2d31/#5865f2) и сверка с эталонными
 *   RE-оракулами fixtures/voice/rtp-pt120-oracle.json и fixtures/voice/rtc-frames.jsonl.
 * - SUB-978-2: Устойчивость голосового тракта Opus (48 кГц, 20 мс = 960 сэмплов) при 5% потерь,
 *   150 мс RTT и 30 мс джиттере: In-Band FEC (LBRR) и сокрытие потерь Opus PLC (Packet Loss
 *   Concealment) без прерывания голоса (audioContinuityPct = 100%, underrunFrames = 0).
 * - SUB-978-3: Кодирование/декодирование RTCP Generic NACK (PT=205, FMT=1, RFC 4585 PID+BLP,
 *   обход переполнения 65535 -> 0), ретрансляция RTP RTX (RFC 4588, 2-байтный OSN, rtx_ssrc)
 *   и fallback RTCP PLI/FIR (PT=206) со 100% восстановлением ключевых кадров.
 * - SUB-978-4: Граничные и негативные сценарии (5.0% vs 10.0% vs 10.1% WebRTC Error 2003,
 *   искажение SRTP GCM тега, Replay, битые NACK/RTX пакеты, маскирование ключей) и CLI
 *   scripts/wan-simulation.mjs (коды выхода 0 / 1 / 2, генерация JSON и Markdown отчёта).
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SCRIPT_PATH = join(ROOT, "scripts", "wan-simulation.mjs");
const RTP_ORACLE_PATH = join(ROOT, "fixtures", "voice", "rtp-pt120-oracle.json");
const RTC_FRAMES_PATH = join(ROOT, "fixtures", "voice", "rtc-frames.jsonl");
const REPORT_PATH = join(ROOT, "waves", "RW-45", "QUALITY", "WAN-SIMULATION-REPORT.md");

describe("TASK-978: Стенд эмуляции WAN профилей (150ms RTT, 5% loss, 30ms jitter), NACK/RTX и Opus PLC (scripts/wan-simulation.mjs)", () => {
  it("SUB-978-1: инициализирует профиль WAN 3G/LTE (150ms RTT, 5% loss, 30ms jitter), UI-токены эмулятора и сверяет контракты с RE-оракулами rtp-pt120-oracle.json и rtc-frames.jsonl", async () => {
    const {
      WAN_DEFAULT_PROFILE,
      WAN_PROFILES,
      WAN_UI_CONTRACT,
      resolveLossSliderState,
      WanLinkEmulator,
      verifyWanOracleParity,
    } = await import("../../scripts/wan-simulation.mjs");

    // 1. Проверка целевого профиля WAN 3G/LTE (150 мс RTT, 5% потерь, 30 мс джиттер)
    assert.equal(WAN_DEFAULT_PROFILE.name, "WAN_3G_LTE");
    assert.equal(WAN_DEFAULT_PROFILE.rttMs, 150);
    assert.equal(WAN_DEFAULT_PROFILE.oneWayDelayMs, 75);
    assert.equal(WAN_DEFAULT_PROFILE.packetLossPct, 5.0);
    assert.equal(WAN_DEFAULT_PROFILE.jitterMs, 30);
    assert.equal(WAN_DEFAULT_PROFILE.sampleRateHz, 48_000);
    assert.equal(WAN_DEFAULT_PROFILE.frameDurationMs, 20);
    assert.equal(WAN_DEFAULT_PROFILE.pcmFrameSamples, 960);
    assert.equal(WAN_DEFAULT_PROFILE.audioPayloadType, 120);
    assert.equal(WAN_DEFAULT_PROFILE.videoPayloadType, 101);
    assert.equal(WAN_DEFAULT_PROFILE.rtxPayloadType, 102);
    assert.ok(WAN_PROFILES.LAN_IDEAL);
    assert.ok(WAN_PROFILES.WAN_3G_LTE);
    assert.ok(WAN_PROFILES.WAN_DEGRADED_ERROR_2003);

    // 2. Проверка UI-контракта карточки TASK-978 (Network Emulator, WAN 3G/LTE, loss-slider)
    assert.equal(WAN_UI_CONTRACT.container, "Network Emulator");
    assert.equal(WAN_UI_CONTRACT.dimensions, "WAN 3G/LTE");
    assert.equal(WAN_UI_CONTRACT.designTokens.background, "#1e1f22");
    assert.equal(WAN_UI_CONTRACT.designTokens.surface, "#2b2d31");
    assert.equal(WAN_UI_CONTRACT.designTokens.text_primary, "#ffffff");
    assert.equal(WAN_UI_CONTRACT.designTokens.accent, "#5865f2");
    assert.equal(WAN_UI_CONTRACT.designTokens.border, "#3f4147");

    const sliderDefault = resolveLossSliderState(0, false);
    assert.equal(sliderDefault.element, "loss-slider");
    assert.equal(sliderDefault.state, "default");
    assert.equal(sliderDefault.label, "0%");
    assert.equal(sliderDefault.plcActive, false);

    const sliderHover = resolveLossSliderState(0, true);
    assert.equal(sliderHover.state, "hover");
    assert.equal(sliderHover.action, "inject packet loss");

    const sliderActive = resolveLossSliderState(5, false);
    assert.equal(sliderActive.state, "active");
    assert.equal(sliderActive.action, "plc active");
    assert.equal(sliderActive.plcActive, true);
    assert.equal(sliderActive.clampedLossPct, 5);
    assert.equal(resolveLossSliderState(-10, false).clampedLossPct, 0);
    assert.equal(resolveLossSliderState(150, false).clampedLossPct, 100);
    assert.equal(resolveLossSliderState(Number.NaN, false).clampedLossPct, 0);

    // 3. Проверка детерминированного WAN-эмулятора канала (150 мс RTT = 75 мс one-way, 5% потерь, ±30 мс джиттер)
    const link = new WanLinkEmulator({
      rttMs: 150,
      packetLossPct: 5.0,
      jitterMs: 30.0,
      seed: 0x9780_2026,
    });
    let delivered = 0;
    let dropped = 0;
    let minDelay = Number.POSITIVE_INFINITY;
    let maxDelay = Number.NEGATIVE_INFINITY;

    for (let seq = 0; seq < 1000; seq += 1) {
      const tx = link.transmitPacket({ sequence: seq, sendTimeMs: seq * 20, sizeBytes: 120 });
      if (tx.dropped) {
        dropped += 1;
      } else {
        delivered += 1;
        if (tx.oneWayDelayMs < minDelay) minDelay = tx.oneWayDelayMs;
        if (tx.oneWayDelayMs > maxDelay) maxDelay = tx.oneWayDelayMs;
      }
    }
    const linkStats = link.stats();
    assert.equal(delivered + dropped, 1000);
    // При детерминированном профиле 5% потерь на 1000 пакетов ожидается 50 потерь (5.0%)
    assert.equal(dropped, 50);
    assert.equal(linkStats.observedLossPct, 5.0);
    assert.ok(minDelay >= 45.0 && maxDelay <= 105.0, `Задержка one-way должна лежать в 75±30 мс, получено [${minDelay}, ${maxDelay}]`);
    assert.ok(Math.abs(linkStats.meanRttMs - 150.0) <= 5.0);

    // 4. Сверка с эталонными RE-оракулами fixtures/voice/rtp-pt120-oracle.json и fixtures/voice/rtc-frames.jsonl
    assert.ok(existsSync(RTP_ORACLE_PATH), "Оракул fixtures/voice/rtp-pt120-oracle.json должен существовать");
    assert.ok(existsSync(RTC_FRAMES_PATH), "Оракул fixtures/voice/rtc-frames.jsonl должен существовать");
    const rtpMd5 = createHash("md5").update(readFileSync(RTP_ORACLE_PATH, "utf8")).digest("hex");
    assert.equal(rtpMd5.length, 32);

    const parity = verifyWanOracleParity(RTP_ORACLE_PATH, RTC_FRAMES_PATH);
    assert.equal(parity.passed, true, `Несовпадение с RE-оракулом: ${parity.reasons.join("; ")}`);
    assert.equal(parity.rtpCapturePackets, 10);
    assert.equal(parity.rtpValidBedePackets, 8);
    assert.equal(parity.audioSsrcSession3, 9208);
    assert.equal(parity.audioSsrcSession2, 17843);
    assert.equal(parity.videoSsrcSession2, 17844);
    assert.equal(parity.rtxSsrcSession2, 17845);
    assert.equal(parity.opusPayloadType, 120);
    assert.equal(parity.av1PayloadType, 101);
    assert.equal(parity.av1RtxPayloadType, 102);
    assert.equal(parity.h264PayloadType, 105);
    assert.equal(parity.h264RtxPayloadType, 106);
  });

  it("SUB-978-2: подтверждает непрерывность голоса при 5% потерь, 150ms RTT и 30ms jitter за счёт Opus PLC и In-Band FEC", async () => {
    const {
      OpusPlcConcealer,
      simulateOpusVoiceOverWan,
    } = await import("../../scripts/wan-simulation.mjs");

    // 1. Модульная проверка сокрытия потерь Opus PLC (гармоническая экстраполяция + плавное затухание без щелчков)
    const plc = new OpusPlcConcealer({ sampleRateHz: 48_000, frameSamples: 960 });

    // Подаём эталонный тональный кадр речи (440 Гц @ 48 кГц, амплитуда 0.4)
    const speechFrame = new Float32Array(960);
    for (let i = 0; i < 960; i += 1) {
      speechFrame[i] = Math.fround(0.4 * Math.sin((2 * Math.PI * 440 * i) / 48_000));
    }
    const decodedOk = plc.recordGoodFrame(speechFrame, 100);
    assert.equal(decodedOk.length, 960);
    assert.equal(plc.stats().goodFrames, 1);

    // Имитируем потерю пакета 101 -> срабатывает Opus PLC (сокрытие потери без провала в тишину)
    const concealed1 = plc.concealLostFrame(101);
    assert.equal(concealed1.samples.length, 960);
    assert.equal(concealed1.mode, "plc_harmonic");
    assert.ok(concealed1.rmsEnergy > 0.15, `Энергия первого PLC-кадра должна сохранять речь (>0.15), получено ${concealed1.rmsEnergy}`);
    // Отсутствие ступеньки на стыке кадра (плавный crossfade с концом предыдущего кадра)
    const boundaryJump = Math.abs((concealed1.samples[0] ?? 0) - (speechFrame[959] ?? 0));
    assert.ok(boundaryJump < 0.08, `Стык PLC-кадра должен быть без щелчка (<0.08), получено ${boundaryJump}`);

    // Вторая подряд потеря -> плавное затухание коэффициента усиления (attenuation decay)
    const concealed2 = plc.concealLostFrame(102);
    assert.ok(concealed2.rmsEnergy < concealed1.rmsEnergy && concealed2.rmsEnergy > 0.05);

    // Восстановление по In-Band FEC (LBRR) из следующего пакета N+1
    const fecRecovered = plc.recoverWithInBandFec(speechFrame, 103);
    assert.equal(fecRecovered.mode, "inband_fec");
    assert.equal(fecRecovered.samples.length, 960);
    assert.ok(fecRecovered.rmsEnergy > 0.2);

    // 2. Сквозная симуляция голосового потока Opus через WAN 3G/LTE (150ms RTT, 5% loss, 30ms jitter, 500 пакетов = 10 с)
    const voiceSim = simulateOpusVoiceOverWan({
      rttMs: 150,
      packetLossPct: 5.0,
      jitterMs: 30.0,
      totalFrames: 500,
      inBandFecEnabled: true,
      seed: 0x9780_2026,
    });

    assert.equal(voiceSim.totalFramesSent, 500);
    assert.equal(voiceSim.networkLostPackets, 25); // ровно 5% из 500
    assert.ok(voiceSim.fecRecoveredFrames > 0, "Часть одиночных потерь должна восстанавливаться через Opus In-Band FEC");
    assert.ok(voiceSim.plcConcealedFrames > 0, "Остальные потери должны маскироваться через Opus PLC");
    assert.equal(
      voiceSim.directReceivedFrames + voiceSim.fecRecoveredFrames + voiceSim.plcConcealedFrames,
      500,
      "Все 500 голосовых кадров (10 секунд речи) должны быть выданы на воспроизведение без пропусков",
    );
    assert.equal(voiceSim.underrunFrames, 0, "Пропуски/провалы воспроизведения (underrun) должны быть строго равны 0");
    assert.equal(voiceSim.audioContinuityPct, 100.0, "Непрерывность голоса при 5% потерь обязана быть 100%");
    assert.equal(voiceSim.voiceInterrupted, false, "Голос не должен прерываться при 5% потерь");
    assert.equal(voiceSim.highPacketLossError2003, false);
  });

  it("SUB-978-3: проверяет битовую точность RTCP Generic NACK (PT=205, FMT=1), RTP RTX (RFC 4588) и 100% восстановление ключевых кадров", async () => {
    const {
      RTCP_RTPFB_PT,
      RTCP_PSFB_PT,
      RTCP_FMT_NACK,
      RTCP_FMT_PLI,
      RTCP_FMT_FIR,
      encodeNackEntries,
      decodeNackEntries,
      buildRtcpNack,
      parseRtcpNack,
      buildRtcpPli,
      parseRtcpPli,
      buildRtcpFir,
      parseRtcpFir,
      buildRtxPacket,
      restoreRtxPacket,
      simulateVideoNackRtxOverWan,
    } = await import("../../scripts/wan-simulation.mjs");

    assert.equal(RTCP_RTPFB_PT, 205);
    assert.equal(RTCP_PSFB_PT, 206);
    assert.equal(RTCP_FMT_NACK, 1);
    assert.equal(RTCP_FMT_PLI, 1);
    assert.equal(RTCP_FMT_FIR, 4);

    // 1. Сверка упаковки/распаковки RTCP Generic NACK (PID + BLP) с переполнением u16 (65535 -> 0 -> 1, 20)
    const entries = encodeNackEntries([65_535, 0, 1, 20]);
    assert.deepEqual(entries, [
      { pid: 65_535, blp: 0b0000_0000_0000_0011 },
      { pid: 20, blp: 0 },
    ]);
    assert.deepEqual(decodeNackEntries(entries), [65_535, 0, 1, 20]);

    const nackWire = buildRtcpNack(9208, 9209, [65_535, 0, 1, 20]);
    assert.equal(nackWire.length, 20); // 12B заголовок + 2 * 4B FCI
    assert.equal(nackWire[0], 0x81); // V=2, P=0, FMT=1
    assert.equal(nackWire[1], 205); // PT=205 (RTPFB)
    assert.equal(nackWire.readUInt16BE(2), 4); // (20 / 4) - 1 = 4 words

    const parsedNack = parseRtcpNack(nackWire);
    assert.equal(parsedNack.senderSsrc, 9208);
    assert.equal(parsedNack.mediaSsrc, 9209);
    assert.deepEqual(parsedNack.lostSequences, [65_535, 0, 1, 20]);

    // 2. Проверка RTCP PLI (12 Б, PT=206, FMT=1) и RTCP FIR (20 Б, PT=206, FMT=4)
    const pliWire = buildRtcpPli(9208, 9209);
    assert.equal(pliWire.length, 12);
    const parsedPli = parseRtcpPli(pliWire);
    assert.deepEqual(parsedPli, { senderSsrc: 9208, mediaSsrc: 9209 });

    const firWire = buildRtcpFir(9208, 9209, 7);
    assert.equal(firWire.length, 20);
    const parsedFir = parseRtcpFir(firWire);
    assert.equal(parsedFir.senderSsrc, 9208);
    assert.equal(parsedFir.mediaSsrc, 0);
    assert.deepEqual(parsedFir.entries, [{ ssrc: 9209, sequenceNumber: 7 }]);

    // 3. Проверка упаковки и распаковки пакета ретрансляции RTP RTX по RFC 4588 (2-байтный OSN)
    const originalVideoPacket = {
      marker: true,
      payloadType: 101, // AV1
      sequence: 65_535,
      timestamp: 90_000,
      ssrc: 9209,
      extensionData: Buffer.from("a62bf0cc7fc0b826", "hex"),
      payload: Buffer.from([0x0a, 0x04, 0x01, 0x02, 0x03, 0x04]), // OBU_SEQUENCE_HEADER keyframe
    };

    const rtxWire = buildRtxPacket(originalVideoPacket, {
      rtxSequence: 400,
      rtxSsrc: 9210,
      rtxPayloadType: 102,
    });
    const restored = restoreRtxPacket(rtxWire, {
      rtxSsrc: 9210,
      rtxPayloadType: 102,
      videoSsrc: 9209,
      videoPayloadType: 101,
    });
    assert.equal(restored.header.ssrc, 9209);
    assert.equal(restored.header.payloadType, 101);
    assert.equal(restored.header.sequence, 65_535);
    assert.equal(restored.header.timestamp, 90_000);
    assert.equal(restored.header.marker, true);
    assert.deepEqual(restored.payload, originalVideoPacket.payload);

    // 4. Сквозная симуляция восстановления видео и ключевых кадров через NACK/RTX при 150ms RTT, 5% loss, 30ms jitter
    const videoSim = simulateVideoNackRtxOverWan({
      rttMs: 150,
      packetLossPct: 5.0,
      jitterMs: 30.0,
      totalKeyframes: 20,
      packetsPerKeyframe: 8,
      interFramesBetweenKeyframes: 14,
      seed: 0x9780_2026,
    });

    assert.equal(videoSim.totalKeyframesSent, 20);
    assert.ok(videoSim.lostVideoPackets > 0, "При 5% потерь часть пакетов видео должна теряться в канале");
    assert.ok(videoSim.nackPacketsSent > 0, "Приёмник обязан отправлять RTCP Generic NACK (PT=205, FMT=1)");
    assert.equal(
      videoSim.rtxPacketsRecovered,
      videoSim.lostVideoPackets,
      "Все потерянные пакеты должны быть восстановлены через RTP RTX (RFC 4588)",
    );
    assert.equal(videoSim.keyframesRecovered, 20, "Все 20 ключевых кадров обязаны быть успешно собраны");
    assert.equal(videoSim.keyframeRecoveryPct, 100.0, "Восстановление ключевых кадров через NACK/RTX должно составлять 100%");
  });

  it("SUB-978-4: проверяет граничные/негативные сценарии (5% vs 10% vs 10.1% Error 2003, битые NACK/RTX, SRTP GCM) и CLI scripts/wan-simulation.mjs", async () => {
    const {
      parseRtcpNack,
      parseRtcpPli,
      restoreRtxPacket,
      evaluateWanSimulationResult,
      redactWanSecrets,
      runWanSimulationSuite,
    } = await import("../../scripts/wan-simulation.mjs");

    // 1. Негативные сценарии разбора RTCP NACK / PLI / RTP RTX
    assert.throws(() => parseRtcpNack(Buffer.from([0x81, 205, 0x00])), /Truncated|короче/i);
    assert.throws(
      () => parseRtcpNack(Buffer.from([0x41, 205, 0, 3, 0, 0, 0, 1, 0, 0, 0, 2, 0, 1, 0, 0])),
      /InvalidVersion|версия/i,
    );
    assert.throws(
      () => parseRtcpNack(Buffer.from([0xa1, 205, 0, 3, 0, 0, 0, 1, 0, 0, 0, 2, 0, 1, 0, 0])),
      /UnsupportedPadding|padding/i,
    );
    // Пустой NACK без FCI (12 байт вместо минимум 16) отклоняется по RFC 4585
    assert.throws(
      () => parseRtcpNack(Buffer.from([0x81, 205, 0, 2, 0, 0, 0, 1, 0, 0, 0, 2])),
      /InvalidLength|длина/i,
    );
    assert.throws(
      () => parseRtcpPli(Buffer.from([0x81, 206, 0, 3, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0])),
      /InvalidLength|длина/i,
    );
    assert.throws(
      () =>
        restoreRtxPacket(Buffer.alloc(14, 0), {
          rtxSsrc: 9210,
          rtxPayloadType: 102,
          videoSsrc: 9209,
          videoPayloadType: 101,
        }),
      /BadVersion|RTX|SSRC/i,
    );

    // 2. Граничные проверки критериев DoD (RTT, джиттер, потери 10.0% vs 10.1% Error 2003, непрерывность голоса, NACK/RTX)
    const baseSummary = runWanSimulationSuite();
    assert.equal(baseSummary.passed, true, `Базовый прогон обязан быть PASS: ${baseSummary.reasons.join("; ")}`);

    // Граница потерь WebRTC Error 2003: 10.0% PASS vs 10.1% FAIL
    assert.equal(evaluateWanSimulationResult({ ...baseSummary, packetLossPct: 10.0 }).passed, true);
    const highLossFail = evaluateWanSimulationResult({ ...baseSummary, packetLossPct: 10.1 });
    assert.equal(highLossFail.passed, false);
    assert.match(highLossFail.reasons.join("; "), /2003|потер/i);

    // Граница непрерывности голоса: 100.0% PASS vs 99.9% FAIL
    const voiceDropFail = evaluateWanSimulationResult({ ...baseSummary, audioContinuityPct: 99.9 });
    assert.equal(voiceDropFail.passed, false);
    assert.match(voiceDropFail.reasons.join("; "), /прерыв|голос|continuity/i);

    // Граница восстановления ключевых кадров NACK/RTX: 100.0% PASS vs 99.9% FAIL
    const keyframeFail = evaluateWanSimulationResult({ ...baseSummary, keyframeRecoveryPct: 99.9 });
    assert.equal(keyframeFail.passed, false);
    assert.match(keyframeFail.reasons.join("; "), /ключев|NACK|RTX/i);

    // 3. Наблюдаемость и маскирование ключей/токенов
    const secretHex = "aa".repeat(32);
    const sanitized = redactWanSecrets(`secret_key="${secretHex}" token="mfa.abcdef123456"`);
    assert.ok(!sanitized.includes(secretHex));
    assert.ok(sanitized.includes("[REDACTED]"));
    assert.ok(sanitized.includes("mfa.***"));

    // 4. Интеграционный запуск CLI scripts/wan-simulation.mjs (коды выхода 0, 1, 2)
    const tmp = mkdtempSync(join(tmpdir(), "wan-sim-978-"));
    try {
      const outJson = join(tmp, "wan-summary.json");
      const outMd = join(tmp, "WAN-SIMULATION-REPORT.md");

      const runOk = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--out-json", outJson, "--out-md", outMd],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(runOk.status, 0, `stderr: ${runOk.stderr}`);
      assert.ok(existsSync(outJson), "JSON-отчёт должен быть создан");
      assert.ok(existsSync(outMd), "Markdown-отчёт должен быть создан");

      const jsonReport = JSON.parse(readFileSync(outJson, "utf8"));
      assert.equal(jsonReport.taskId, "TASK-978");
      assert.equal(jsonReport.status, "passed");
      assert.equal(jsonReport.profile.rttMs, 150);
      assert.equal(jsonReport.profile.packetLossPct, 5.0);
      assert.equal(jsonReport.profile.jitterMs, 30);
      assert.equal(jsonReport.audioContinuityPct, 100.0);
      assert.equal(jsonReport.keyframeRecoveryPct, 100.0);
      assert.equal(jsonReport.oracleParity.passed, true);

      // Код выхода 1 на неизвестный флаг
      const badFlag = spawnSync(process.execPath, [SCRIPT_PATH, "--invalid-flag"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badFlag.status, 1);

      // Код выхода 2 при отсутствии файла оракула
      const missingOracle = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--rtp-oracle", "/nonexistent/missing-oracle.json"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(missingOracle.status, 2);

      // Проверка целевого отчёта waves/RW-45/QUALITY/WAN-SIMULATION-REPORT.md
      assert.ok(existsSync(REPORT_PATH), "Отчёт WAN-SIMULATION-REPORT.md должен существовать");
      const mdContent = readFileSync(REPORT_PATH, "utf8");
      assert.match(mdContent, /TASK-978/);
      assert.match(mdContent, /150/);
      assert.match(mdContent, /Opus PLC/);
      assert.match(mdContent, /NACK/);
      assert.match(mdContent, /RTX/);
      assert.match(mdContent, /PASS/i);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
