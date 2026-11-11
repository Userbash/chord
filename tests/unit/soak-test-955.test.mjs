// @ts-check
/**
 * Контрактные и модульные тесты стресс-теста долговременной стабильности (Soak Test 4h, TASK-955):
 * - SUB-955-1: Эмуляция 10 000 сообщений/мин, 500 переключений каналов, 50 голосовых звонков,
 *   непрерывного скролла 100k сообщений, 1000 циклов модалок и циклический сброс кэша медиа при лимите.
 * - SUB-955-2: Мониторинг /proc/$PID/status каждые 30 секунд (VmRSS < 60 МБ) и инспекция /proc/$PID/fd
 *   на отсутствие утечек открытых сокетов (socket:[...]) и файловых дескрипторов (0 утечек FD).
 * - SUB-955-3: Оценка деградации памяти (дрейф < 1 МБ/час за 4 часа) и генерация отчёта
 *   waves/RW-44/QUALITY/SOAK-TEST-REPORT.md.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SCRIPT_PATH = join(ROOT, "scripts", "soak-test.mjs");
const REPORT_PATH = join(ROOT, "waves", "RW-44", "QUALITY", "SOAK-TEST-REPORT.md");

describe("TASK-955: Стресс-тест долговременной стабильности (Soak Test 4h, scripts/soak-test.mjs)", () => {
  it("SUB-955-1: эмулирует 10 000 сообщений/мин, 500 переключений каналов, 50 голосовых звонков и циклический сброс кэша медиа", async () => {
    const {
      SOAK_DEFAULT_CONFIG,
      MediaCacheSimulator,
      runSoakWorkloadSimulation,
    } = await import("../../scripts/soak-test.mjs");

    assert.equal(SOAK_DEFAULT_CONFIG.messagesPerMinute, 10_000);
    assert.equal(SOAK_DEFAULT_CONFIG.channelSwitches, 500);
    assert.equal(SOAK_DEFAULT_CONFIG.voiceCalls, 50);
    assert.equal(SOAK_DEFAULT_CONFIG.durationHours, 4);
    assert.equal(SOAK_DEFAULT_CONFIG.pollIntervalSeconds, 30);
    assert.equal(SOAK_DEFAULT_CONFIG.maxVmrssMb, 60);
    assert.equal(SOAK_DEFAULT_CONFIG.maxDriftMbPerHour, 1.0);

    // Шаг 1.2: Проверка циклического сброса кэша медиа на границе лимита
    const cache = new MediaCacheSimulator(4 * 1024 * 1024); // 4 МБ лимит
    cache.put("media-1", 2 * 1024 * 1024);
    cache.put("media-2", 2 * 1024 * 1024);
    assert.equal(cache.liveBytes, 4 * 1024 * 1024);
    assert.equal(cache.resetCount, 0);

    // Превышение лимита на 1 байт вызывает циклический сброс кэша перед записью нового элемента
    cache.put("media-overflow", 1024);
    assert.equal(cache.resetCount, 1);
    assert.equal(cache.liveBytes, 1024);
    assert.equal(cache.entriesCount, 1);

    // Полная 4-часовая симуляция нагрузки (240 минут, 480 интервалов по 30 секунд)
    const sim = runSoakWorkloadSimulation({
      durationHours: 4,
      messagesPerMinute: 10_000,
      channelSwitches: 500,
      voiceCalls: 50,
      mediaCacheLimitBytes: 16 * 1024 * 1024,
    });

    assert.equal(sim.durationHours, 4);
    assert.equal(sim.totalSamples, 480); // 4 ч * 60 мин * 2 (каждые 30 сек)
    assert.equal(sim.totalMessagesProcessed, 2_400_000); // 10 000 сообщ/мин * 240 мин
    assert.equal(sim.channelSwitchesCompleted, 500);
    assert.equal(sim.voiceCallsCompleted, 50);
    assert.ok(sim.modalsToggled >= 1_000, "Должно быть выполнено не менее 1000 циклов модалок");
    assert.ok(sim.mediaCacheResets > 0, "Циклический сброс кэша медиа должен срабатывать при достижении лимита");
    assert.ok(sim.finalMediaCacheBytes <= 16 * 1024 * 1024);
  });

  it("SUB-955-2: считывает /proc/$PID/status и проверяет /proc/$PID/fd на 0 утечек сокетов и дескрипторов", async () => {
    const {
      parseProcStatus,
      inspectFdTargets,
      sampleProcessHealth,
    } = await import("../../scripts/soak-test.mjs");

    // 1. Чтение живого текущего процесса (/proc/self/status и /proc/self/fd)
    const selfHealth = sampleProcessHealth(process.pid);
    assert.ok(selfHealth.vmrssKb > 0, "VmRSS живого процесса должен быть > 0 КБ");
    assert.ok(selfHealth.totalFds > 0, "Число открытых FD живого процесса должно быть > 0");
    assert.ok(selfHealth.socketFds >= 0);

    // 2. Парсинг эталонного снимка /proc/$PID/status и граничные условия 60 МБ (61 440 КБ)
    const statusOk = parseProcStatus(
      "Name:\tdiscord-gpui\nVmPeak:\t94200 kB\nVmSize:\t82400 kB\nVmHWM:\t14800 kB\nVmRSS:\t61439 kB\nVmData:\t9200 kB\nThreads:\t8\n",
    );
    assert.equal(statusOk.name, "discord-gpui");
    assert.equal(statusOk.vmrssKb, 61_439);
    assert.equal(statusOk.vmhwmKb, 14_800);
    assert.equal(statusOk.threads, 8);
    assert.ok(statusOk.vmrssMb < 60.0, "61439 kB строго меньше 60 МБ");

    // Негативный тест: отсутствие поля VmRSS вызывает ошибку парсинга
    assert.throws(
      () => parseProcStatus("Name:\tbroken-proc\nThreads:\t1\n"),
      /VmRSS/,
    );

    // 3. Проверка дескрипторов и открытых сокетов в /proc/$PID/fd
    const baselineTargets = [
      "/dev/null",
      "pipe:[1001]",
      "anon_inode:[eventpoll]",
      "socket:[20480]",
    ];
    const afterCleanVoiceCallTargets = [
      "/dev/null",
      "pipe:[1001]",
      "anon_inode:[eventpoll]",
      "socket:[20480]",
    ];
    const cleanFd = inspectFdTargets(baselineTargets, afterCleanVoiceCallTargets);
    assert.equal(cleanFd.totalFds, 4);
    assert.equal(cleanFd.socketFds, 1);
    assert.equal(cleanFd.leakedFds, 0);
    assert.equal(cleanFd.leakedSockets, 0);
    assert.equal(cleanFd.passed, true);

    // Негативный сценарий: незакрытый UDP/WebRTC сокет после завершения звонка
    const leakedSocketTargets = [
      ...baselineTargets,
      "socket:[99991]",
      "socket:[99992]",
    ];
    const leakedFd = inspectFdTargets(baselineTargets, leakedSocketTargets);
    assert.equal(leakedFd.leakedFds, 2);
    assert.equal(leakedFd.leakedSockets, 2);
    assert.equal(leakedFd.passed, false);
  });

  it("SUB-955-3: оценивает деградацию памяти (< 1 МБ/час, пик < 60 МБ) и ловит утечки на границах", async () => {
    const { evaluateSoakStability } = await import("../../scripts/soak-test.mjs");

    // 1. Позитивный сценарий: 4 часа (480 замеров каждые 30с), базовый VmRSS 11.26 МБ, дрейф ~0.08 МБ/ч
    const cleanSamples = Array.from({ length: 481 }, (_, i) => {
      const elapsedSec = i * 30;
      const elapsedHours = elapsedSec / 3600;
      // Небольшие колебания вокруг 11 530 КБ (~11.26 МБ) + 80 КБ/ч (0.078 МБ/ч < 1 МБ/ч)
      const waveKb = (i % 12) * 16; // сбрасывается после GC кэша медиа
      const vmrssKb = Math.round(11_530 + elapsedHours * 80 + waveKb);
      return {
        sampleIndex: i,
        elapsedSeconds: elapsedSec,
        vmrssKb,
        openFds: 18,
        openSockets: 3,
      };
    });

    const cleanVerdict = evaluateSoakStability(cleanSamples);
    assert.equal(cleanVerdict.passed, true);
    assert.ok(cleanVerdict.peakVmrssMb < 60.0);
    assert.ok(cleanVerdict.driftMbPerHour < 1.0);
    assert.equal(cleanVerdict.leakedFds, 0);
    assert.equal(cleanVerdict.leakedSockets, 0);

    // 2. Граничный сценарий дрейфа: 0.99 МБ/ч (PASS) vs 1.00 МБ/ч (FAIL)
    const boundaryPassSamples = [
      { sampleIndex: 0, elapsedSeconds: 0, vmrssKb: 12_000, openFds: 16, openSockets: 2 },
      { sampleIndex: 480, elapsedSeconds: 14_400, vmrssKb: 12_000 + Math.floor(0.99 * 4 * 1024), openFds: 16, openSockets: 2 },
    ];
    assert.equal(evaluateSoakStability(boundaryPassSamples).passed, true);

    const boundaryFailDriftSamples = [
      { sampleIndex: 0, elapsedSeconds: 0, vmrssKb: 12_000, openFds: 16, openSockets: 2 },
      { sampleIndex: 480, elapsedSeconds: 14_400, vmrssKb: 12_000 + 4 * 1024, openFds: 16, openSockets: 2 }, // ровно 1.00 МБ/ч
    ];
    const failDrift = evaluateSoakStability(boundaryFailDriftSamples);
    assert.equal(failDrift.passed, false);
    assert.match(failDrift.reasons.join("; "), /дрейф памяти/i);

    // 3. Граничный сценарий VmRSS: 61 439 КБ (< 60 МБ -> PASS) vs 61 440 КБ (== 60 МБ -> FAIL)
    const boundaryPassRss = [
      { sampleIndex: 0, elapsedSeconds: 0, vmrssKb: 61_439, openFds: 16, openSockets: 2 },
      { sampleIndex: 480, elapsedSeconds: 14_400, vmrssKb: 61_439, openFds: 16, openSockets: 2 },
    ];
    assert.equal(evaluateSoakStability(boundaryPassRss).passed, true);

    const boundaryFailRss = [
      { sampleIndex: 0, elapsedSeconds: 0, vmrssKb: 12_000, openFds: 16, openSockets: 2 },
      { sampleIndex: 240, elapsedSeconds: 7_200, vmrssKb: 61_440, openFds: 16, openSockets: 2 },
      { sampleIndex: 480, elapsedSeconds: 14_400, vmrssKb: 12_100, openFds: 16, openSockets: 2 },
    ];
    const failRss = evaluateSoakStability(boundaryFailRss);
    assert.equal(failRss.passed, false);
    assert.match(failRss.reasons.join("; "), /VmRSS/i);
  });

  it("CLI и отчёт: запускает scripts/soak-test.mjs на релизном бинарнике и проверяет waves/RW-44/QUALITY/SOAK-TEST-REPORT.md", () => {
    const tmp = mkdtempSync(join(tmpdir(), "soak-test-955-"));
    try {
      const outJson = join(tmp, "soak-summary.json");
      const outMd = join(tmp, "SOAK-TEST-REPORT.md");

      const run = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--out-json", outJson, "--out-md", outMd],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(run.status, 0, `stderr: ${run.stderr}`);
      assert.ok(existsSync(outJson), "JSON-сводка должна быть записана");
      assert.ok(existsSync(outMd), "Markdown-отчёт должен быть записан");

      const summary = JSON.parse(readFileSync(outJson, "utf8"));
      assert.equal(summary.taskId, "TASK-955");
      assert.equal(summary.status, "passed");
      assert.ok(summary.peakVmrssMb < 60.0);
      assert.ok(summary.driftMbPerHour < 1.0);
      assert.equal(summary.leakedFds, 0);
      assert.equal(summary.leakedSockets, 0);

      // Проверка кодов выхода CLI: 1 на неверный флаг, 2 на отсутствующий бинарник
      const badFlag = spawnSync(process.execPath, [SCRIPT_PATH, "--invalid-flag"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badFlag.status, 1);

      const missingBin = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--binary", "/nonexistent/discord-gpui-missing"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(missingBin.status, 2);

      // Проверка целевого отчёта waves/RW-44/QUALITY/SOAK-TEST-REPORT.md
      assert.ok(existsSync(REPORT_PATH), "Отчёт waves/RW-44/QUALITY/SOAK-TEST-REPORT.md должен существовать");
      const reportMd = readFileSync(REPORT_PATH, "utf8");
      assert.match(reportMd, /TASK-955/);
      assert.match(reportMd, /VmRSS/);
      assert.match(reportMd, /60/);
      assert.match(reportMd, /\/proc\/\$PID\/status/);
      assert.match(reportMd, /\/proc\/\$PID\/fd/);
      assert.match(reportMd, /10\s*000/);
      assert.match(reportMd, /500/);
      assert.match(reportMd, /50/);
      assert.match(reportMd, /PASS/i);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
