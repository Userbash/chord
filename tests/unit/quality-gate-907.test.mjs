// @ts-check
/**
 * Unit-тесты автоматизированного гейта качества кода (TASK-907):
 * - SUB-907-1: cargo clippy --all-targets -- -D warnings, rustfmt --check --edition 2024,
 *   cargo audit / cargo-deny, статический контроль размера (функции < 50 строк, файлы < 800 строк).
 * - SUB-907-2: динамические санитайзеры LLVM ASan (-Zsanitizer=address) и TSan (-Zsanitizer=thread),
 *   отсутствие дедлоков в связке Tokio <-> GPUI Event Loop.
 * - SUB-907-3: автоматический 5/5 мутационный гейт тестирования (100% kill rate).
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SCRIPT_PATH = join(ROOT, "scripts", "quality-gate.mjs");

describe("TASK-907: Автоматизированный гейт качества кода (scripts/quality-gate.mjs)", () => {
  it("SUB-907-1: блокирует коммит при наличии предупреждений Clippy (-D warnings) и проверяет rustfmt 2024", async () => {
    const {
      buildClippyArgs,
      buildRustfmtArgs,
      evaluateClippyOutput,
      checkRustSourceSizeLimits,
    } = await import("../../scripts/quality-gate.mjs");

    assert.deepEqual(buildClippyArgs({ offline: true }), [
      "clippy",
      "--offline",
      "--all-targets",
      "--",
      "-D",
      "warnings",
    ]);
    assert.deepEqual(buildRustfmtArgs(["src/main.rs"]), [
      "--check",
      "--edition",
      "2024",
      "src/main.rs",
    ]);

    const cleanClippy = evaluateClippyOutput(0, "Checking discord-gpui v0.1.0\nFinished dev profile\n");
    assert.equal(cleanClippy.passed, true);
    assert.equal(cleanClippy.warnings, 0);

    const warnClippy = evaluateClippyOutput(
      101,
      "warning: unused import `std::io`\nerror: could not compile `discord-gpui` due to 1 previous error\n",
    );
    assert.equal(warnClippy.passed, false);
    assert.equal(warnClippy.warnings, 1);

    // Контроль размера функций (< 50 строк) и файлов модулей (< 800 строк)
    const validSrc = "pub fn short_fn(x: u32) -> u32 {\n    x + 1\n}\n";
    const validLimits = checkRustSourceSizeLimits("src/short.rs", validSrc);
    assert.equal(validLimits.passed, true);

    const longFnLines = ["pub fn too_long() {"];
    for (let i = 0; i < 50; i += 1) longFnLines.push(`    let v_${i} = ${i};`);
    longFnLines.push("}");
    const badFnLimits = checkRustSourceSizeLimits("src/long_fn.rs", longFnLines.join("\n"));
    assert.equal(badFnLimits.passed, false);
    assert.match(badFnLimits.reason, /too_long.*>= 50/);

    const longModuleLines = Array.from({ length: 801 }, (_, i) => `pub const C_${i}: u32 = ${i};`).join("\n");
    const badModLimits = checkRustSourceSizeLimits("src/huge_mod.rs", longModuleLines);
    assert.equal(badModLimits.passed, false);
    assert.match(badModLimits.reason, /800/);
  });

  it("SUB-907-1 (Шаг 1.3): проверяет лицензии cargo-deny и обнаруживает уязвимости cargo audit", async () => {
    const { auditCargoLockAndDeny } = await import("../../scripts/quality-gate.mjs");

    const denyPath = join(ROOT, "rust", "chord-gpui", "deny.toml");
    const lockPath = join(ROOT, "rust", "chord-gpui", "Cargo.lock");

    const cleanAudit = auditCargoLockAndDeny({ denyPath, lockPath, advisories: [] });
    assert.equal(cleanAudit.passed, true);
    assert.ok(cleanAudit.packagesAudited >= 20);

    const vulnAudit = auditCargoLockAndDeny({
      denyPath,
      lockPath,
      advisories: [
        {
          id: "RUSTSEC-2026-0907",
          package: "tokio",
          version: cleanAudit.tokioVersion,
          title: "Test advisory injection",
        },
      ],
    });
    assert.equal(vulnAudit.passed, false);
    assert.equal(vulnAudit.vulnerabilities[0]?.id, "RUSTSEC-2026-0907");
  });

  it("SUB-907-2: формирует конфигурации ASan (-Zsanitizer=address), TSan (-Zsanitizer=thread) и проверяет граф дедлоков Tokio <-> GPUI", async () => {
    const {
      getSanitizerConfig,
      parseSanitizerReport,
      verifyTokioGpuiLockOrder,
    } = await import("../../scripts/quality-gate.mjs");

    const asanCfg = getSanitizerConfig("address");
    assert.match(asanCfg.RUSTFLAGS, /-Zsanitizer=address/);
    const tsanCfg = getSanitizerConfig("thread");
    assert.match(tsanCfg.RUSTFLAGS, /-Zsanitizer=thread/);

    const asanParsed = parseSanitizerReport(
      "==101==ERROR: AddressSanitizer: heap-use-after-free on address 0x602000000010",
    );
    assert.equal(asanParsed.clean, false);
    assert.equal(asanParsed.kind, "heap-use-after-free");

    const tsanParsed = parseSanitizerReport(
      "WARNING: ThreadSanitizer: data race (pid=202)\n  Write of size 8 at 0x7ff000 by thread T2:",
    );
    assert.equal(tsanParsed.clean, false);
    assert.equal(tsanParsed.kind, "data-race");

    const cleanLocks = verifyTokioGpuiLockOrder([
      ["UiFrameLock", "AppStateLock", "AudioRingLock"],
      ["AppStateLock", "AudioRingLock"],
    ]);
    assert.equal(cleanLocks.deadlockFree, true);

    const cyclicLocks = verifyTokioGpuiLockOrder([
      ["AppStateLock", "TokioChannelLock"],
      ["TokioChannelLock", "AppStateLock"],
    ]);
    assert.equal(cyclicLocks.deadlockFree, false);
    assert.match(cyclicLocks.cyclePath, /AppStateLock -> TokioChannelLock -> AppStateLock/);
  });

  it("SUB-907-3: генерирует 5 контрольных мутаций на модуль и требует 5/5 (100% kill rate)", async () => {
    const { generateFiveControlMutations, evaluateMutationGate } = await import(
      "../../scripts/quality-gate.mjs"
    );

    const sampleSource = [
      "pub const MAX_CHARS: usize = 2000;",
      "pub fn validate(msg: &str, authed: bool, seq: u64) -> Result<u64, &'static str> {",
      "    if !authed { return Err(\"unauthed\"); }",
      "    if msg.is_empty() || msg.len() > MAX_CHARS { return Err(\"len\"); }",
      "    let next = seq + 1;",
      "    Ok(next)",
      "}",
    ].join("\n");

    const mutants = generateFiveControlMutations("chat_validator", sampleSource);
    assert.equal(mutants.length, 5);

    const passGate = evaluateMutationGate(mutants, () => ({ killed: true, reason: "assertion failed" }));
    assert.equal(passGate.passed, true);
    assert.equal(passGate.killed, 5);
    assert.equal(passGate.total, 5);
    assert.equal(passGate.killRate, 1.0);
    assert.equal(passGate.verdict, "5/5 mutations killed PASS");

    const failGate = evaluateMutationGate(mutants, (_m, idx) => ({
      killed: idx !== 2,
      reason: idx === 2 ? "survived" : "killed",
    }));
    assert.equal(failGate.passed, false);
    assert.equal(failGate.killed, 4);
    assert.equal(failGate.survived, 1);
  });

  it("CLI: --dry-run возвращает 0 и печатает полный план гейта, а неверный флаг возвращает 1", () => {
    const dry = spawnSync(process.execPath, [SCRIPT_PATH, "--dry-run"], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(dry.status, 0, dry.stderr);
    assert.match(dry.stdout, /cargo clippy --offline --all-targets -- -D warnings/);
    assert.match(dry.stdout, /rustfmt --check --edition 2024/);
    assert.match(dry.stdout, /-Zsanitizer=address/);
    assert.match(dry.stdout, /-Zsanitizer=thread/);
    assert.match(dry.stdout, /5\/5 mutations killed PASS/);

    const bad = spawnSync(process.execPath, [SCRIPT_PATH, "--unknown-flag"], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(bad.status, 1);
  });
});
