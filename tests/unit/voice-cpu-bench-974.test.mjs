// @ts-check
/**
 * Контрактные и модульные тесты бенчмарка нагрузки CPU и джиттера в голосовом канале
 * на 25 участников (1250 пак/сек Opus, AES-256-GCM SRTP) — TASK-974:
 * - SUB-974-1: Формирование и разбор RTP-пакетов Opus (PT=120, 48 кГц, 20 мс = 960 сэмплов)
 *   с расширением RFC 5285 0xBEDE (2 слова = 8 Б) и шифрованием SRTP aead_aes256_gcm_rtpsize
 *   (32 Б ключ OP 4, 16 Б GCM tag, 4 Б LE трейлер 0x80000000 | n) со сверкой по независимому
 *   оракулу fixtures/voice/rtp-pt120-oracle.json (/home/sanya/re-deep/voice/voice-udp4.pcap).
 * - SUB-974-2: Статистика приёма RFC 3550 §6.4.1 (джиттер 222.141 ед. = 4.628 мс на оракуле
 *   voice-udp4.pcap, обход переполнения sequence 65535 -> 0, дубликаты, переупорядочивание)
 *   и адаптивный джиттер-буфер с удержанием задержки < 25 мс.
 * - SUB-974-3: Эмуляция звонка на 25 участников (25 * 50 = 1250 пак/сек Opus SRTP, микширование
 *   960 сэмплов @ 48 кГц с мягким лимитером tanh и индивидуальной громкостью 0..200%),
 *   контроль бюджета нагрузки CPU (< 5.0%) и задержки джиттер-буфера (< 25.0 мс).
 * - SUB-974-4: Валидация граничных и негативных сценариев (4.99% vs 5.00% CPU, 24.99 мс vs 25.00 мс
 *   джиттер, подделка тега AES-256-GCM, replay-атака, потери > 10% WebRTC Error 2003, маскирование ключей)
 *   и проверка CLI scripts/bench-voice-cpu.mjs (коды выхода 0 / 1 / 2, JSON/MD отчёт).
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
const SCRIPT_PATH = join(ROOT, "scripts", "bench-voice-cpu.mjs");
const ORACLE_PATH = join(ROOT, "fixtures", "voice", "rtp-pt120-oracle.json");
const REPORT_PATH = join(ROOT, "waves", "RW-45", "QUALITY", "VOICE-CPU-BENCH-REPORT.md");

describe("TASK-974: Бенчмарк нагрузки CPU (<5%) и джиттера (<25мс) в голосовом канале на 25 участников (scripts/bench-voice-cpu.mjs)", () => {
  it("SUB-974-1: шифрует и расшифровывает RTP Opus (PT=120, 0xBEDE) в режиме aead_aes256_gcm_rtpsize и сверяет раскладку с оракулом fixtures/voice/rtp-pt120-oracle.json", async () => {
    const {
      VOICE_BENCH_DEFAULT_CONFIG,
      encodeRtpHeaderBede,
      parseRtpHeaderBede,
      SrtpAes256GcmRtpSizeCipher,
      verifyOracleCaptureParity,
    } = await import("../../scripts/bench-voice-cpu.mjs");

    // 1. Проверка базовых констант голосового тракта по /home/sanya/re-deep/voice-deep.md §2.3, §3
    assert.equal(VOICE_BENCH_DEFAULT_CONFIG.participantsCount, 25);
    assert.equal(VOICE_BENCH_DEFAULT_CONFIG.packetsPerSecPerParticipant, 50);
    assert.equal(VOICE_BENCH_DEFAULT_CONFIG.totalPacketsPerSecond, 1_250);
    assert.equal(VOICE_BENCH_DEFAULT_CONFIG.sampleRateHz, 48_000);
    assert.equal(VOICE_BENCH_DEFAULT_CONFIG.frameDurationMs, 20);
    assert.equal(VOICE_BENCH_DEFAULT_CONFIG.pcmFrameSamples, 960);
    assert.equal(VOICE_BENCH_DEFAULT_CONFIG.rtpPayloadType, 120);
    assert.equal(VOICE_BENCH_DEFAULT_CONFIG.rtpExtensionProfile, 0xbede);
    assert.equal(VOICE_BENCH_DEFAULT_CONFIG.encryptionMode, "aead_aes256_gcm_rtpsize");
    assert.equal(VOICE_BENCH_DEFAULT_CONFIG.maxCpuLoadPct, 5.0);
    assert.equal(VOICE_BENCH_DEFAULT_CONFIG.maxJitterBufferDelayMs, 25.0);

    // 2. Сверка с независимым эталонным оракулом fixtures/voice/rtp-pt120-oracle.json (voice-udp4.pcap)
    assert.ok(existsSync(ORACLE_PATH), "Эталонный оракул fixtures/voice/rtp-pt120-oracle.json должен существовать");
    const oracleRaw = readFileSync(ORACLE_PATH, "utf8");
    const oracleMd5 = createHash("md5").update(oracleRaw).digest("hex");
    assert.equal(oracleMd5.length, 32);

    const oracleCheck = verifyOracleCaptureParity(ORACLE_PATH);
    assert.equal(oracleCheck.passed, true, `Несовпадение с оракулом: ${oracleCheck.reasons.join("; ")}`);
    assert.equal(oracleCheck.totalCapturePackets, 10);
    assert.equal(oracleCheck.warmupAnomalyPackets, 2);
    assert.equal(oracleCheck.validBedePackets, 8);
    assert.equal(oracleCheck.ssrc, 9208);
    assert.equal(oracleCheck.payloadType, 120);
    assert.ok(
      Math.abs(oracleCheck.jitterUnits - 222.141) <= 0.01,
      `Ожидался джиттер 222.141 ед. по RFC 3550, получено: ${oracleCheck.jitterUnits}`,
    );
    assert.ok(
      Math.abs(oracleCheck.jitterMs - 4.628) <= 0.001,
      `Ожидался джиттер 4.628 мс по RFC 3550, получено: ${oracleCheck.jitterMs}`,
    );

    // 3. Проверка кодирования/разбора 24-байтного заголовка RTP + RFC 5285 0xBEDE (2 слова = 8 Б)
    const extBytes = Buffer.from("a62bf0cc7fc0b826", "hex");
    const hdrBytes = encodeRtpHeaderBede({
      marker: false,
      payloadType: 120,
      sequence: 17_466,
      timestamp: 278_470_988,
      ssrc: 9_208,
      extensionData: extBytes,
    });
    assert.equal(hdrBytes.length, 24);
    assert.equal(hdrBytes.toString("hex"), "9078443a1099214c000023f8bede0002a62bf0cc7fc0b826");

    const parsedHdr = parseRtpHeaderBede(hdrBytes);
    assert.equal(parsedHdr.version, 2);
    assert.equal(parsedHdr.marker, false);
    assert.equal(parsedHdr.payloadType, 120);
    assert.equal(parsedHdr.sequence, 17_466);
    assert.equal(parsedHdr.timestamp, 278_470_988);
    assert.equal(parsedHdr.ssrc, 9_208);
    assert.equal(parsedHdr.headerLength, 24);
    assert.equal(parsedHdr.extensionProfile, 0xbede);
    assert.equal(parsedHdr.extensionWords, 2);

    // 4. Положительный и негативный тест SRTP AES-256-GCM (aead_aes256_gcm_rtpsize)
    const secretKey = Buffer.alloc(32);
    for (let i = 0; i < 32; i += 1) secretKey[i] = i + 1;

    const senderCipher = new SrtpAes256GcmRtpSizeCipher(secretKey);
    const receiverCipher = new SrtpAes256GcmRtpSizeCipher(secretKey);

    // Comfort-noise 3 Б payload -> 24 Б AAD + 3 Б CT + 16 Б Tag + 4 Б LE Counter = 47 Б (как в voice-udp4.pcap)
    const cnPayload = Buffer.from([0xf8, 0xff, 0xfe]);
    const wire47 = senderCipher.encryptRtp(hdrBytes, cnPayload);
    assert.equal(wire47.length, 47);
    assert.equal(wire47.subarray(0, 24).toString("hex"), hdrBytes.toString("hex"));
    assert.equal(wire47.readUInt32LE(43), 0x8000_0000);

    const opened47 = receiverCipher.decryptRtp(wire47);
    assert.deepEqual(opened47.payload, cnPayload);
    assert.equal(opened47.counter, 0x8000_0000);
    assert.equal(opened47.header.sequence, 17_466);

    // Защита от replay-атаки: повторная подача того же пакета с тем же counter отклоняется
    assert.throws(
      () => receiverCipher.decryptRtp(wire47),
      /Replay|дубликат/i,
    );

    // Защита от подделки AAD/шифротекста/тега: искажение 1 бита вызывает ошибку аутентификации GCM
    const wireNext = senderCipher.encryptRtp(hdrBytes, Buffer.alloc(60, 0x55));
    const tampered = Buffer.from(wireNext);
    tampered[10] = (tampered[10] ?? 0) ^ 0x01; // искажаем байт SSRC в открытом заголовке (AAD)
    assert.throws(
      () => receiverCipher.decryptRtp(tampered),
      /AuthenticationFailed|аутентификации/i,
    );

    // Проверка zeroize очистки ключа в памяти
    const zeroStat = senderCipher.zeroizeKey();
    assert.equal(zeroStat.zeroizedBytes, 32);
    assert.equal(zeroStat.allZero, true);
  });

  it("SUB-974-2: отслеживает потери/переупорядочивание по RFC 3550 и удерживает задержку адаптивного джиттер-буфера < 25 мс", async () => {
    const {
      RtpReceptionTracker,
      AdaptiveVoiceJitterBuffer,
    } = await import("../../scripts/bench-voice-cpu.mjs");

    // 1. Проверка перехода sequence через границу u16::MAX (65534, 65535, 0, 1) и timestamp wrap
    const tracker = new RtpReceptionTracker(48_000);
    const tsBase = 0xffff_ff00;
    const seqs = [65_534, 65_535, 0, 1];
    for (let i = 0; i < seqs.length; i += 1) {
      const obs = tracker.observe(
        {
          payloadType: 120,
          marker: i === 0,
          sequence: seqs[i] ?? 0,
          timestamp: (tsBase + i * 960) >>> 0,
          ssrc: 1001,
        },
        i * 20.0,
      );
      assert.equal(obs, "in_order");
    }
    const snapWrap = tracker.snapshot();
    assert.equal(snapWrap.received, 4);
    assert.equal(snapWrap.expected, 4);
    assert.equal(snapWrap.lost, 0);
    assert.equal(snapWrap.duplicates, 0);
    assert.equal(snapWrap.highestSequence, 1);
    assert.ok(snapWrap.jitterMs < 0.001, `При идеальном шаге 20 мс джиттер должен быть ~0 мс, получено ${snapWrap.jitterMs}`);

    // 2. Проверка дубликатов и переупорядочивания (1, 3, 2, 4)
    const reorderTracker = new RtpReceptionTracker(48_000);
    assert.equal(reorderTracker.observe({ payloadType: 120, marker: true, sequence: 1, timestamp: 960, ssrc: 7 }, 0), "in_order");
    assert.equal(reorderTracker.observe({ payloadType: 120, marker: false, sequence: 3, timestamp: 2880, ssrc: 7 }, 22), "in_order");
    assert.equal(reorderTracker.observe({ payloadType: 120, marker: false, sequence: 2, timestamp: 1920, ssrc: 7 }, 25), "reordered");
    assert.equal(reorderTracker.observe({ payloadType: 120, marker: false, sequence: 2, timestamp: 1920, ssrc: 7 }, 30), "duplicate");
    assert.equal(reorderTracker.observe({ payloadType: 120, marker: false, sequence: 4, timestamp: 3840, ssrc: 7 }, 60), "in_order");

    const reorderSnap = reorderTracker.snapshot();
    assert.equal(reorderSnap.received, 4);
    assert.equal(reorderSnap.expected, 4);
    assert.equal(reorderSnap.lost, 0);
    assert.equal(reorderSnap.duplicates, 1);

    // 3. Проверка адаптивного джиттер-буфера (удержание задержки строго < 25.0 мс при сетевом джиттере)
    const jb = new AdaptiveVoiceJitterBuffer({
      ssrc: 9_208,
      frameDurationMs: 20,
      maxDelayMs: 25.0,
      targetDelayMs: 16.0,
    });

    // Подаём 50 кадров (1 секунда речи) с реалистичными колебаниями времени прихода (±4.5 мс) и переупорядочиванием
    for (let i = 0; i < 50; i += 1) {
      const jitterOffsetMs = ((i * 7) % 9) - 4; // от -4 до +4 мс
      const arrivalMs = i * 20 + jitterOffsetMs;
      jb.pushPacket({
        sequence: (1000 + i) & 0xffff,
        timestamp: (i * 960) >>> 0,
        arrivalMs,
        pcmSamples: new Float32Array(960).fill(0.15),
      });
      if (i >= 1) {
        const popped = jb.popFrame(i * 20);
        assert.equal(popped.samples.length, 960);
      }
    }

    const jbStats = jb.stats();
    assert.ok(
      jbStats.peakDelayMs < 25.0,
      `Пиковая задержка джиттер-буфера обязана быть < 25.0 мс, получено: ${jbStats.peakDelayMs}`,
    );
    assert.ok(
      jbStats.averageDelayMs < 25.0,
      `Средняя задержка джиттер-буфера обязана быть < 25.0 мс, получено: ${jbStats.averageDelayMs}`,
    );
  });

  it("SUB-974-3: эмулирует звонок на 25 участников (1250 пак/сек Opus SRTP, микшер tanh), подтверждая нагрузку CPU < 5% и джиттер < 25 мс", async () => {
    const {
      VoiceChannelMixer25,
      softLimitTanh,
      runVoiceCpuBenchmark,
      evaluateVoiceBenchResult,
    } = await import("../../scripts/bench-voice-cpu.mjs");

    // 1. Проверка микшера на 25 участников и независимого регулятора streamVolume с мягким лимитером tanh
    const mixer = new VoiceChannelMixer25();
    for (let peer = 1; peer <= 25; peer += 1) {
      mixer.announce(peer, 10_000 + peer);
      mixer.setParticipantVolumePercent(peer, 100);
    }
    // Коллизия SSRC с другим участником отклоняется
    assert.throws(
      () => mixer.announce(999, 10_001),
      /SSRC belongs to another participant|коллизия SSRC/i,
    );

    // Граничные значения громкости 0..200% (0.0..=2.0) и NaN
    assert.equal(mixer.setParticipantVolume(1, 3.5), 2.0);
    assert.equal(mixer.setParticipantVolume(1, -0.5), 0.0);
    assert.equal(mixer.setParticipantVolume(1, Number.NaN), 1.0);
    assert.equal(mixer.setStreamVolumePercent(150), 1.5);
    assert.equal(mixer.participantVolume(1), 1.0);

    // Подаём PCM-кадры от всех 25 участников и микшируем 20 мс окно (960 сэмплов)
    const sampleFrame = new Float32Array(960).fill(0.08);
    for (let peer = 1; peer <= 25; peer += 1) {
      assert.equal(mixer.pushSsrcPcm(10_000 + peer, sampleFrame), true);
    }
    const mixed = mixer.popMixedFrame();
    assert.ok(mixed.frame instanceof Float32Array);
    assert.equal(mixed.frame.length, 960);
    assert.equal(mixed.activeSpeakersCount, 25);
    // 25 * 0.08 = 2.0 -> tanh(2.0) ≈ 0.96402758 (строго без клиппинга <= 1.0)
    const expectedTanh = Math.tanh(25 * 0.08);
    assert.ok(Math.abs((mixed.frame[0] ?? 0) - expectedTanh) < 1e-5);
    assert.equal(softLimitTanh(Number.NaN), 0.0);
    assert.equal(softLimitTanh(Number.POSITIVE_INFINITY), 1.0);

    // 2. Полный прогон бенчмарка на 25 участников (1250 пак/сек Opus AES-256-GCM SRTP)
    const benchResult = runVoiceCpuBenchmark({
      participantsCount: 25,
      packetsPerSecPerParticipant: 50,
      durationSeconds: 1,
    });

    assert.equal(benchResult.participantsCount, 25);
    assert.equal(benchResult.packetsPerSecPerParticipant, 50);
    assert.equal(benchResult.totalPacketsPerSecond, 1_250);
    assert.equal(benchResult.totalPacketsProcessed, 1_250);
    assert.equal(benchResult.encryptionMode, "aead_aes256_gcm_rtpsize");
    assert.equal(benchResult.sampleRateHz, 48_000);
    assert.equal(benchResult.pcmFrameSamples, 960);
    assert.equal(benchResult.authFailures, 0);
    assert.equal(benchResult.replayDrops, 0);
    assert.ok(
      benchResult.cpuLoadPct < 5.0,
      `Нагрузка CPU при 25 пирах (1250 пак/сек) должна быть < 5.0%, получено: ${benchResult.cpuLoadPct}%`,
    );
    assert.ok(
      benchResult.jitterBufferDelayMs < 25.0,
      `Задержка джиттер-буфера должна быть < 25.0 мс, получено: ${benchResult.jitterBufferDelayMs} мс`,
    );
    assert.ok(
      benchResult.maxJitterMs < 25.0,
      `Максимальный джиттер RFC 3550 должен быть < 25.0 мс, получено: ${benchResult.maxJitterMs} мс`,
    );

    const verdict = evaluateVoiceBenchResult(benchResult);
    assert.equal(verdict.passed, true, `Бенчмарк должен проходить: ${verdict.reasons.join("; ")}`);
  });

  it("SUB-974-4: проверяет граничные условия (4.99% vs 5.00% CPU, 24.99 vs 25.00 мс джиттер, потери >10%) и CLI scripts/bench-voice-cpu.mjs", async () => {
    const {
      evaluateVoiceBenchResult,
      redactVoiceSecrets,
    } = await import("../../scripts/bench-voice-cpu.mjs");

    // 1. Граничные проверки нагрузки CPU: 4.99% (PASS) vs 5.00% (FAIL)
    const baseMetrics = {
      participantsCount: 25,
      packetsPerSecPerParticipant: 50,
      totalPacketsPerSecond: 1_250,
      totalPacketsProcessed: 1_250,
      cpuLoadPct: 4.99,
      jitterBufferDelayMs: 18.5,
      maxJitterMs: 4.628,
      packetLossPct: 0.0,
      authFailures: 0,
    };
    assert.equal(evaluateVoiceBenchResult(baseMetrics).passed, true);

    const cpuFailBoundary = evaluateVoiceBenchResult({
      ...baseMetrics,
      cpuLoadPct: 5.0,
    });
    assert.equal(cpuFailBoundary.passed, false);
    assert.match(cpuFailBoundary.reasons.join("; "), /CPU/i);

    // 2. Граничные проверки джиттер-буфера: 24.99 мс (PASS) vs 25.00 мс (FAIL)
    assert.equal(
      evaluateVoiceBenchResult({ ...baseMetrics, jitterBufferDelayMs: 24.99 }).passed,
      true,
    );
    const jitterFailBoundary = evaluateVoiceBenchResult({
      ...baseMetrics,
      jitterBufferDelayMs: 25.0,
    });
    assert.equal(jitterFailBoundary.passed, false);
    assert.match(jitterFailBoundary.reasons.join("; "), /джиттер/i);

    // 3. Граничные проверки потерь пакетов WebRTC Error 2003: 10.0% (PASS) vs 10.1% (FAIL)
    assert.equal(
      evaluateVoiceBenchResult({ ...baseMetrics, packetLossPct: 10.0 }).passed,
      true,
    );
    const lossFail = evaluateVoiceBenchResult({
      ...baseMetrics,
      packetLossPct: 10.1,
    });
    assert.equal(lossFail.passed, false);
    assert.match(lossFail.reasons.join("; "), /2003|потер/i);

    // 4. Наблюдаемость и маскирование ключей secret_key / токенов в логах
    const rawSecretHex = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
    const redacted = redactVoiceSecrets(
      `OP4 session_description secret_key="${rawSecretHex}" token="mfa.secret12345"`,
    );
    assert.ok(!redacted.includes(rawSecretHex), "secret_key обязан быть замаскирован");
    assert.ok(redacted.includes("[REDACTED]"));
    assert.ok(redacted.includes("mfa.***"));

    // 5. Интеграционный запуск CLI scripts/bench-voice-cpu.mjs и проверка кодов выхода (0, 1, 2)
    const tmp = mkdtempSync(join(tmpdir(), "voice-cpu-bench-974-"));
    try {
      const outJson = join(tmp, "voice-bench.json");
      const outMd = join(tmp, "VOICE-CPU-BENCH-REPORT.md");

      const runOk = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--out-json", outJson, "--out-md", outMd],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(runOk.status, 0, `stderr: ${runOk.stderr}`);
      assert.ok(existsSync(outJson), "JSON-сводка должна быть создана");
      assert.ok(existsSync(outMd), "Markdown-отчёт должен быть создан");

      const summary = JSON.parse(readFileSync(outJson, "utf8"));
      assert.equal(summary.taskId, "TASK-974");
      assert.equal(summary.status, "passed");
      assert.equal(summary.participantsCount, 25);
      assert.equal(summary.totalPacketsPerSecond, 1_250);
      assert.equal(summary.encryptionMode, "aead_aes256_gcm_rtpsize");
      assert.ok(summary.cpuLoadPct < 5.0);
      assert.ok(summary.jitterBufferDelayMs < 25.0);
      assert.equal(summary.oracleParity.passed, true);

      // Код выхода 1 на неизвестный аргумент
      const badArg = spawnSync(process.execPath, [SCRIPT_PATH, "--unknown-flag"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badArg.status, 1);

      // Код выхода 2 при отсутствии файла оракула
      const missingOracle = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--oracle", "/nonexistent/rtp-pt120-oracle-missing.json"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(missingOracle.status, 2);

      // Проверка сгенерированного отчёта waves/RW-45/QUALITY/VOICE-CPU-BENCH-REPORT.md
      assert.ok(existsSync(REPORT_PATH), "Целевой отчёт VOICE-CPU-BENCH-REPORT.md должен существовать");
      const reportMd = readFileSync(REPORT_PATH, "utf8");
      assert.match(reportMd, /TASK-974/);
      assert.match(reportMd, /aead_aes256_gcm_rtpsize/);
      assert.match(reportMd, /1250/);
      assert.match(reportMd, /25/);
      assert.match(reportMd, /PASS/i);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
