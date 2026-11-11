// @ts-check
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SCRIPT_PATH = join(ROOT, "scripts", "fuzz-report.mjs");

describe("fuzz-report: автоматический генератор отчётов о паниках, ASan OOB и утечках (TASK-903.3)", () => {
  it("формирует зелёный JSON и Markdown отчёт при чистом прогоне 1M итераций без сбоев", async () => {
    const { analyzeFuzzRun } = await import("../../scripts/fuzz-report.mjs");
    const tmp = mkdtempSync(join(tmpdir(), "fuzz-report-clean-"));
    try {
      const logDir = join(tmp, "logs");
      const artifactDir = join(tmp, "artifacts");
      mkdirSync(logDir, { recursive: true });
      mkdirSync(artifactDir, { recursive: true });

      writeFileSync(
        join(logDir, "fuzz_anti_replay_window.log"),
        [
          "INFO: Seed: 123456789",
          "#2\tINITED cov: 48 ft: 92 corp: 2/31b exec/s: 0 rss: 38Mb",
          "#1000000\tDONE   cov: 64 ft: 215 corp: 19/612b lim: 2048 exec/s: 142857 rss: 44Mb",
          "Done 1000000 runs in 7 second(s)",
        ].join("\n"),
      );

      const report = analyzeFuzzRun({ logDir, artifactDir, minRuns: 1_000_000 });
      assert.equal(report.status, "passed");
      assert.equal(report.totalTargets, 1);
      assert.equal(report.crashes.length, 0);
      assert.equal(report.targets[0]?.name, "fuzz_anti_replay_window");
      assert.equal(report.targets[0]?.runsCompleted, 1_000_000);
      assert.equal(report.targets[0]?.peakRssMb, 44);
      assert.equal(report.targets[0]?.passed, true);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("классифицирует Rust panic, AddressSanitizer OOB и LeakSanitizer с хешем входного артефакта", async () => {
    const { analyzeFuzzRun, renderMarkdownReport } = await import("../../scripts/fuzz-report.mjs");
    const tmp = mkdtempSync(join(tmpdir(), "fuzz-report-crash-"));
    try {
      const logDir = join(tmp, "logs");
      const artifactDir = join(tmp, "artifacts");
      mkdirSync(join(artifactDir, "fuzz_rtp_header_extensions"), { recursive: true });
      mkdirSync(join(artifactDir, "fuzz_aes128_gcm_truncated8"), { recursive: true });
      mkdirSync(logDir, { recursive: true });

      const crashBytes = Buffer.from([0x90, 0x78, 0xbe, 0xde, 0xff]);
      const crashFile = join(
        artifactDir,
        "fuzz_rtp_header_extensions",
        "crash-0123456789abcdef",
      );
      writeFileSync(crashFile, crashBytes);

      writeFileSync(
        join(logDir, "fuzz_rtp_header_extensions.log"),
        [
          "thread '<unnamed>' panicked at crates/discord-voice/src/media/rtp.rs:312:18:",
          "attempt to multiply with overflow",
          "==4242== ERROR: libFuzzer: deadly signal",
          "    #0 0x55555561a120 in __sanitizer_print_stack_trace",
          "    #1 0x55555570b340 in discord_voice::media::rtp::parse_rtp",
          `Test unit written to ${crashFile}`,
        ].join("\n"),
      );

      writeFileSync(
        join(logDir, "fuzz_aes128_gcm_truncated8.log"),
        [
          "=================================================================",
          "==5151==ERROR: AddressSanitizer: heap-buffer-overflow on address 0x602000000018 at pc 0x555555801122",
          "READ of size 4 at 0x602000000018 thread T0",
          "    #0 0x555555801121 in discord_voice::crypto::gcm::ghash",
          "SUMMARY: AddressSanitizer: heap-buffer-overflow crates/discord-voice/src/crypto/gcm.rs:140",
        ].join("\n"),
      );

      writeFileSync(
        join(logDir, "fuzz_gateway_json_etf.log"),
        [
          "=================================================================",
          "==6161==ERROR: LeakSanitizer: detected memory leaks",
          "Direct leak of 128 byte(s) in 1 object(s) allocated from:",
          "    #0 0x555555600100 in malloc",
          "SUMMARY: AddressSanitizer: 128 byte(s) leaked in 1 allocation(s).",
        ].join("\n"),
      );

      const report = analyzeFuzzRun({ logDir, artifactDir, minRuns: 1_000_000 });
      assert.equal(report.status, "failed");
      assert.equal(report.crashes.length, 3);

      const byKind = new Map(report.crashes.map((c) => [c.target, c]));
      const rtpCrash = byKind.get("fuzz_rtp_header_extensions");
      assert.ok(rtpCrash);
      assert.equal(rtpCrash.kind, "panic");
      assert.match(rtpCrash.summary, /attempt to multiply with overflow/);
      assert.equal(rtpCrash.artifactSize, 5);
      assert.equal(rtpCrash.artifactHexPreview, "9078bedeff");
      assert.match(rtpCrash.reproduceCommand, /cargo-fuzz run .*fuzz_rtp_header_extensions/);

      const asanCrash = byKind.get("fuzz_aes128_gcm_truncated8");
      assert.ok(asanCrash);
      assert.equal(asanCrash.kind, "asan_oob");
      assert.match(asanCrash.summary, /heap-buffer-overflow/);

      const leakCrash = byKind.get("fuzz_gateway_json_etf");
      assert.ok(leakCrash);
      assert.equal(leakCrash.kind, "memory_leak");

      const md = renderMarkdownReport(report);
      assert.match(md, /Отчёт фаззинга протоколов и криптографии DAVE MLS v1/);
      assert.match(md, /asan_oob/);
      assert.match(md, /9078bedeff/);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("соблюдает контракт кодов выхода CLI (0 успех, 1 ошибка аргументов или краш, 2 нет каталога логов)", () => {
    const badArg = spawnSync(process.execPath, [SCRIPT_PATH, "--bad-flag"], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(badArg.status, 1);

    const missingEnv = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--log-dir", "/nonexistent/fuzz-logs-dir-xyz"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(missingEnv.status, 2);
  });
});
