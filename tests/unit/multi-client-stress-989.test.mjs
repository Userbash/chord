// @ts-check
/**
 * Контрактные и модульные тесты автоматизированного нагрузочного стенда
 * на 50 одновременных ботов/клиентов (5000 событий/с, всплески сообщений,
 * синхронные реакции, индикаторы набора текста и переходы говорящих) — TASK-989:
 * - SUB-989-0: Статический контракт `scripts/stress-test-50-peers.mjs` (размер файла < 800 строк,
 *   функции < 50 строк, 0 секретов по `secret-scan.mjs`).
 * - SUB-989-1: Сверка дизайн-токенов, состояний `stress-cluster` (`idle`, `inject load`,
 *   `5000 msg/s`), топологии 50 пиров и констант протокола с независимыми RE-оракулами
 *   (`waves/kanban-board.json`, `fixtures/gateway/by-type/023-MESSAGE_CREATE.json`,
 *   `fixtures/gateway/summary.json`, `fixtures/voice/rtc-frames.jsonl`).
 * - SUB-989-2: Эмуляция синтетической нагрузки 5000 событий/с от 50 пиров (всплески сообщений,
 *   синхронные реакции 50 участников, индикаторы набора `TYPING_START` с TTL 10 с, голосовое
 *   присутствие и переходы `SPEAKING` op 5) без фризов UI-потока (0 jank frames, 120 FPS
 *   бюджет <= 8.33 мс) и с дельтой памяти RSS <= 60 МБ.
 * - SUB-989-3: Проверка детерминированных переходов состояния (кольцевой буфер сообщений,
 *   идемпотентность реакций, истечение TTL набора текста и сброс при отправке сообщения,
 *   блокировка `SPEAKING` при `self_mute`/`suppress`).
 * - SUB-989-4: Покрытие всех 9 измерений `TEST-POLICY.md` (позитивные, негативные, граничные,
 *   ошибки 401/403/404/413/429/5xx, пустые, загрузочные 202, права доступа, восстановление
 *   `RECONNECT -> RESUMED`, наблюдаемость/маскирование) и CLI кодов выхода `0 / 1 / 2`.
 * - SUB-989-5: Мутационный гейт `5/5` (`100%` kill rate) через `evaluateMutationGate`.
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
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SCRIPT_PATH = join(ROOT, "scripts", "stress-test-50-peers.mjs");
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const MSG_CREATE_ORACLE_PATH = join(
  ROOT,
  "fixtures",
  "gateway",
  "by-type",
  "023-MESSAGE_CREATE.json",
);
const GW_SUMMARY_ORACLE_PATH = join(ROOT, "fixtures", "gateway", "summary.json");
const RTC_FRAMES_ORACLE_PATH = join(ROOT, "fixtures", "voice", "rtc-frames.jsonl");

/**
 * Проверяет, что каждая JS-функция в исходном тексте занимает < 50 строк.
 * @param {string} source
 */
function assertJsFunctionsUnder50Lines(source) {
  const lines = source.split("\n");
  for (let i = 0; i < lines.length; i += 1) {
    const m = (lines[i] ?? "").match(/\bfunction\s+([A-Za-z0-9_]+)\s*\(/);
    if (!m) continue;
    let depth = 0;
    let opened = false;
    let end = i;
    for (let j = i; j < lines.length; j += 1) {
      for (const ch of (lines[j] ?? "").split("//")[0] ?? "") {
        if (ch === "{") {
          depth += 1;
          opened = true;
        } else if (ch === "}") {
          depth -= 1;
        }
      }
      if (opened && depth <= 0) {
        end = j;
        break;
      }
    }
    const fnLines = end - i + 1;
    assert.ok(fnLines < 50, `Функция ${m[1]} содержит ${fnLines} строк (лимит < 50)`);
  }
}

/**
 * Проверяет CLI `scripts/stress-test-50-peers.mjs` (коды выхода 0, 1, 2 и генерацию отчётов).
 */
function verifyStressCliExitCodes() {
  const tmp = mkdtempSync(join(tmpdir(), "stress-50-peers-989-"));
  try {
    const outJson = join(tmp, "stress-summary.json");
    const outMd = join(tmp, "MULTI-CLIENT-STRESS-REPORT.md");
    const okRun = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--out-json", outJson, "--out-md", outMd],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(okRun.status, 0, `stderr: ${okRun.stderr}`);
    assert.ok(existsSync(outJson));
    assert.ok(existsSync(outMd));
    const report = JSON.parse(readFileSync(outJson, "utf8"));
    assert.equal(report.taskId, "TASK-989");
    assert.equal(report.status, "passed");
    assert.equal(report.simulation.peerCount, 50);
    assert.equal(report.simulation.eventsPerSecond, 5_000);

    const badArg = spawnSync(process.execPath, [SCRIPT_PATH, "--invalid-flag"], { cwd: ROOT, encoding: "utf8" });
    assert.equal(badArg.status, 1);

    const missingOracle = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--oracle", join(tmp, "missing-kanban.json")],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(missingOracle.status, 2);

    const brokenOracle = join(tmp, "broken-kanban.json");
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
}

describe("TASK-989: Нагрузочный стенд на 50 одновременных ботов/клиентов (scripts/stress-test-50-peers.mjs)", () => {
  it("SUB-989-0: скрипт существует, соблюдает лимиты размера (<50 строк на функцию, <800 на файл) и не содержит секретов", () => {
    assert.ok(existsSync(SCRIPT_PATH), "Файл scripts/stress-test-50-peers.mjs должен существовать");
    const source = readFileSync(SCRIPT_PATH, "utf8");
    const sizeCheck = checkRustSourceSizeLimits("scripts/stress-test-50-peers.mjs", source);
    assert.equal(sizeCheck.passed, true, sizeCheck.reason);
    assert.ok(sizeCheck.fileLines < 800, `Ожидалось < 800 строк, получено ${sizeCheck.fileLines}`);
    assertJsFunctionsUnder50Lines(source);
    const secrets = scanTextForSecrets(source, "scripts/stress-test-50-peers.mjs");
    assert.equal(secrets.length, 0, `Найдены секреты: ${JSON.stringify(secrets)}`);
  });

  it("SUB-989-1: сверяет дизайн-токены, состояния stress-cluster, топологию 50 пиров и константы с независимыми RE-оракулами", async () => {
    const {
      STRESS_DESIGN_TOKENS,
      STRESS_CLUSTER_STATES,
      STRESS_LAYOUT_CONTRACT,
      STRESS_DEFAULT_CONFIG,
      VOICE_SPEAKING_FLAGS,
      resolveStressClusterState,
      createPeerRoster,
      verifyStressOracleParity,
    } = await import("../../scripts/stress-test-50-peers.mjs");

    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    assert.equal(createHash("md5").update(kanbanRaw).digest("hex").length, 32);
    const card989 = JSON.parse(kanbanRaw).cards.find((/** @type {any} */ c) => c.id === "TASK-989");
    assert.ok(card989, "Карточка TASK-989 должна присутствовать в waves/kanban-board.json");
    assert.deepEqual(STRESS_DESIGN_TOKENS, card989.design_tokens);
    assert.deepEqual(STRESS_CLUSTER_STATES, card989.interactive_states[0]);
    assert.deepEqual(STRESS_LAYOUT_CONTRACT, card989.layout);

    assert.equal(STRESS_DEFAULT_CONFIG.peerCount, 50);
    assert.equal(STRESS_DEFAULT_CONFIG.targetEventsPerSec, 5_000);
    assert.equal(STRESS_DEFAULT_CONFIG.maxRssMb, 60.0);
    assert.equal(STRESS_DEFAULT_CONFIG.frameBudgetMs, 8.33);
    assert.equal(STRESS_DEFAULT_CONFIG.typingTtlMs, 10_000);
    assert.equal(STRESS_DEFAULT_CONFIG.maxChannelMessagesWindow, 200);
    assert.deepEqual(VOICE_SPEAKING_FLAGS, { NONE: 0, MICROPHONE: 1, SOUNDSHARE: 2, PRIORITY: 4 });

    assert.equal(resolveStressClusterState({ active: false, hovered: false }).label, "idle");
    assert.equal(resolveStressClusterState({ active: false, hovered: true }).label, "inject load");
    assert.equal(resolveStressClusterState({ active: true, hovered: false }).label, "5000 msg/s");

    const peers = createPeerRoster(50);
    assert.equal(peers.length, 50);
    assert.equal(new Set(peers.map((p) => p.userId)).size, 50);
    assert.equal(new Set(peers.map((p) => p.audioSsrc)).size, 50);

    const parity = verifyStressOracleParity({
      kanbanPath: KANBAN_BOARD_PATH,
      msgCreatePath: MSG_CREATE_ORACLE_PATH,
      gwSummaryPath: GW_SUMMARY_ORACLE_PATH,
      rtcFramesPath: RTC_FRAMES_ORACLE_PATH,
    });
    assert.equal(parity.passed, true, `Нарушение RE-оракула: ${parity.reasons.join("; ")}`);
    assert.equal(parity.guildId, "1014537851418980372");
    assert.equal(parity.channelId, "1163408591315812392");
    assert.equal(parity.baseAudioSsrc, 17843);
  });

  it("SUB-989-2: выдерживает поток 5000 событий/с от 50 пиров с нулевым jank UI-потока и дельтой памяти RSS <= 60 МБ", async () => {
    const { runMultiClientStressSimulation } = await import("../../scripts/stress-test-50-peers.mjs");

    const sim = runMultiClientStressSimulation({
      peerCount: 50,
      totalEvents: 5_000,
      durationSeconds: 1.0,
      seed: 0x9890_2026,
    });

    assert.equal(sim.passed, true, `Сбой нагрузочного прогона: ${sim.reasons.join("; ")}`);
    assert.equal(sim.peerCount, 50);
    assert.equal(sim.totalEventsProcessed, 5_000);
    assert.equal(sim.eventsPerSecond, 5_000);
    assert.equal(sim.clientCrashed, false);
    assert.equal(sim.uiThreadBlocked, false);
    assert.equal(sim.jankFrames, 0, "Число просевших кадров UI (jankFrames) обязано быть 0");
    assert.ok(sim.p99FrameTimeMs <= 8.33, `p99 кадра (${sim.p99FrameTimeMs} мс) должен быть <= 8.33 мс`);
    assert.ok(sim.rssDeltaMb <= 60.0, `Дельта RSS (${sim.rssDeltaMb} МБ) должна быть <= 60 МБ`);
    assert.ok(sim.estimatedClientRssMb <= 60.0, `Оценка RSS клиента (${sim.estimatedClientRssMb} МБ) <= 60 МБ`);
    assert.ok(sim.breakdown.messageEvents > 0);
    assert.ok(sim.breakdown.reactionEvents > 0);
    assert.ok(sim.breakdown.typingEvents > 0);
    assert.ok(sim.breakdown.voiceSpeakingEvents > 0);
    assert.equal(
      sim.breakdown.messageEvents +
        sim.breakdown.reactionEvents +
        sim.breakdown.typingEvents +
        sim.breakdown.voiceSpeakingEvents,
      5_000,
    );
    assert.ok(sim.stateSnapshot.retainedMessagesCount <= 200);
    assert.equal(sim.stateSnapshot.connectedVoicePeers, 50);
  });

  it("SUB-989-3: проверяет инварианты редьюсера: кольцо сообщений, синхронные реакции 50 пиров, TTL набора текста и запрет SPEAKING при mute", async () => {
    const { StressClusterState, VOICE_SPEAKING_FLAGS } = await import("../../scripts/stress-test-50-peers.mjs");
    const cluster = new StressClusterState({ peerCount: 50, maxMessagesWindow: 200 });

    for (let i = 1; i <= 250; i += 1) {
      const peer = cluster.getPeer(((i - 1) % 50) + 1);
      const res = cluster.applyEvent({
        type: "MESSAGE_CREATE",
        messageId: String(1561627527296979000n + BigInt(i)),
        channelId: "1163408591315812392",
        userId: peer.userId,
        content: `Нагрузочное сообщение #${i}`,
        timestampMs: 1_000 + i,
      });
      assert.equal(res.accepted, true);
    }
    assert.equal(cluster.totalMessagesProcessed, 250);
    assert.equal(cluster.getRetainedMessages().length, 200);

    const targetMsgId = String(1561627527296979000n + 250n);
    for (let p = 1; p <= 50; p += 1) {
      const peer = cluster.getPeer(p);
      assert.equal(cluster.applyEvent({ type: "MESSAGE_REACTION_ADD", messageId: targetMsgId, userId: peer.userId, emoji: "🔥", timestampMs: 2_000 + p }).accepted, true);
      assert.equal(cluster.applyEvent({ type: "MESSAGE_REACTION_ADD", messageId: targetMsgId, userId: peer.userId, emoji: "🔥", timestampMs: 2_100 + p }).accepted, false);
    }
    assert.equal(cluster.getReactionCount(targetMsgId, "🔥"), 50);

    const peer1 = cluster.getPeer(1);
    const peer2 = cluster.getPeer(2);
    cluster.applyEvent({ type: "TYPING_START", channelId: "1163408591315812392", userId: peer1.userId, timestampMs: 5_000 });
    cluster.applyEvent({ type: "TYPING_START", channelId: "1163408591315812392", userId: peer2.userId, timestampMs: 5_000 });
    assert.equal(cluster.getActiveTypingUsers("1163408591315812392", 14_999).length, 2);
    cluster.applyEvent({ type: "MESSAGE_CREATE", messageId: "1561627527296979999", channelId: "1163408591315812392", userId: peer1.userId, content: "Готово", timestampMs: 6_000 });
    assert.deepEqual(cluster.getActiveTypingUsers("1163408591315812392", 6_000), [peer2.userId]);
    assert.equal(cluster.getActiveTypingUsers("1163408591315812392", 15_000).length, 0);

    assert.equal(cluster.applyEvent({ type: "SPEAKING", userId: peer1.userId, speakingFlags: VOICE_SPEAKING_FLAGS.MICROPHONE | VOICE_SPEAKING_FLAGS.PRIORITY, timestampMs: 7_000 }).accepted, true);
    assert.equal(cluster.isPeerSpeaking(peer1.userId), true);
    cluster.applyEvent({ type: "VOICE_STATE_UPDATE", userId: peer1.userId, channelId: "1176785373989445703", selfMute: true, selfDeaf: false, suppress: false, timestampMs: 7_100 });
    assert.equal(cluster.isPeerSpeaking(peer1.userId), false);
    const speakMuted = cluster.applyEvent({ type: "SPEAKING", userId: peer1.userId, speakingFlags: VOICE_SPEAKING_FLAGS.MICROPHONE, timestampMs: 7_200 });
    assert.equal(speakMuted.accepted, false);
    assert.equal(speakMuted.errorCode, "PEER_MUTED_OR_SUPPRESSED");
  });

  it("SUB-989-4: покрывает все 9 измерений TEST-POLICY.md, оценку границ (60.0 vs 60.1 МБ, 5000 vs 4999 eps), маскирование секретов и CLI (коды 0/1/2)", async () => {
    const {
      evaluateStressSimulationResult,
      evaluateStressNineDimensions,
      formatUnreadBadge,
      redactStressTelemetryLog,
      runMultiClientStressSimulation,
    } = await import("../../scripts/stress-test-50-peers.mjs");

    assert.equal(formatUnreadBadge(0), "0");
    assert.equal(formatUnreadBadge(99), "99");
    assert.equal(formatUnreadBadge(100), "99+");
    assert.equal(formatUnreadBadge(5_000), "99+");

    const baseSim = runMultiClientStressSimulation({ peerCount: 50, totalEvents: 5_000, durationSeconds: 1.0 });
    assert.equal(evaluateStressSimulationResult({ ...baseSim, rssDeltaMb: 60.0 }).passed, true);
    assert.equal(evaluateStressSimulationResult({ ...baseSim, rssDeltaMb: 60.1 }).passed, false);
    assert.equal(evaluateStressSimulationResult({ ...baseSim, eventsPerSecond: 4_999 }).passed, false);
    assert.equal(evaluateStressSimulationResult({ ...baseSim, jankFrames: 1 }).passed, false);

    const nineDim = evaluateStressNineDimensions();
    assert.equal(nineDim.passed, true, `Сбой 9 измерений: ${nineDim.reasons.join("; ")}`);
    assert.equal(nineDim.boundaries.minSnowflake, "0");
    assert.equal(nineDim.boundaries.maxSnowflake, "18446744073709551615");
    assert.equal(nineDim.boundaries.maxMessageChars, 2_000);
    assert.equal(nineDim.boundaries.maxAttachmentsCount, 10);
    assert.equal(nineDim.boundaries.maxAttachmentSizeBytes, 25 * 1024 * 1024);
    assert.equal(nineDim.boundaries.maxSearchOffset, 9_975);
    assert.deepEqual(nineDim.boundaries.voiceVolumeBoundsPercent, [0, 200]);
    assert.equal(nineDim.boundaries.unreadBadgeCap, "99+");
    assert.equal(nineDim.boundaries.maxRssMb, 60.0);
    assert.deepEqual(nineDim.errorHandling.httpStatusCodesCovered, [401, 403, 404, 413, 429, 503]);
    assert.equal(nineDim.recovery.resumeReplayVerified, true);

    const secretSample = 'token="mfa.secret123456" auth="Bot MTIz.GhI.MnO" bearer="Bearer eyJhbGci" secret_key="deadbeef"';
    const sanitized = redactStressTelemetryLog(secretSample);
    assert.ok(!sanitized.includes("secret123456"));
    assert.match(sanitized, /mfa\.\*\*\*/);
    assert.match(sanitized, /Bot \*\*\*/);
    assert.match(sanitized, /Bearer \*\*\*/);

    verifyStressCliExitCodes();
  });

  it("SUB-989-5: убивает 5/5 контрольных мутаций стенда нагрузки 50 пиров (100% mutation kill rate)", async () => {
    const {
      generateStressTestMutations,
      runStressMutationEvaluation,
    } = await import("../../scripts/stress-test-50-peers.mjs");

    const mutants = generateStressTestMutations();
    assert.equal(mutants.length, 5);
    const gate = evaluateMutationGate(mutants, (mutant) => runStressMutationEvaluation(mutant));
    assert.equal(gate.passed, true, `Выжившие мутанты: ${gate.survivors.join(", ")}`);
    assert.equal(gate.killed, 5);
    assert.equal(gate.survived, 0);
    assert.equal(gate.killRate, 1);
  });
});
