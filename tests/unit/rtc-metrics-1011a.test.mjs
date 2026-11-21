// @ts-check
/**
 * Контрактные и модульные тесты сборщика RTCP метрик (`RingBuffer 120`, 60 с с шагом 500 мс)
 * и неблокирующего мониторинга эпох DAVE MLS v1 (`TASK-1011A` — `RE-GAP-22`):
 * - SUB-1011A-0: Статический и компиляционный контракт `rust/crates/chord-voice/src/rtc_metrics.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, файл < 800 строк, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1011A-1: Сверка карточки `TASK-1011A` в `waves/kanban-board.json`, дизайн-токенов
 *   (`#1e1f22`, `#2b2d31`, `#ffffff`, `#23a55a`, `#3f4147`), геометрии `RTCP Collector`
 *   (`RingBuffer 120`, `padding: 0px`, `spacing: 0px`), интерактивных состояний `telemetry-probe`
 *   (`polling` -> `collect` -> `buffered`) и независимых оракулов (`waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`,
 *   `docs/re/voice-deep.json`, фикстур `waves/RW-26/research/fixtures/rtcp-voice-udp{3,4}-server.json`).
 * - SUB-1011A-2: Кольцевой буфер `RtcpMetricsRingBuffer` на 120 измерений (окно 60 000 мс с шагом 500 мс),
 *   разбор бинарных пакетов RTCP Sender Report (`PT=200`, 28B/60B SRTCP) и Receiver Report (`PT=201`, 32B/52B SRTCP),
 *   расчёт RTT по `LSR`/`DLSR` (`1/65536` с), межпакетного джиттера RFC 3550 (`48 кГц -> мс`),
 *   процента потерь исходящих пакетов `outboundLossRate` по дельтам, битрейта и качества `ConnectionQuality` (`PingHistory` 199/20).
 * - SUB-1011A-3: Неблокирующий (lock-free атомарный) мониторинг эпох DAVE MLS v1 (`DaveMlsEpochMonitor`):
 *   хуки аудио-потока без мьютексов (`on_frame_encrypted`, `on_frame_decrypted`, `on_frame_failure`,
 *   `on_passthrough_frame`), переходы эпох opcodes `21..=31` (`on_prepare_epoch`, `on_execute_transition`,
 *   защита от downgrade) и генерация 30-значного `Voice Privacy Code` (`6x5` из `epochAuthenticator`).
 * - SUB-1011A-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные: Snowflake `0`/`u64::MAX`, `2000` символов, `10` файлов/`25 МБ`, `offset=9975`,
 *   громкость `0..200%`, бейдж `99+`, пороги пинга `250/500` мс и потерь `5%/10%`; ошибки HTTP
 *   `400/401/403/404/413/429 retry_after_ms/5xx`; пустые; `skeleton`/`HTTP 202`; права
 *   `VIEW_CHANNEL`/`CONNECT`/`SPEAK`/`AGE_GATED_SPACES`; восстановление после 3 таймаутов пинга `4000 мс`;
 *   маскирование секретов).
 * - SUB-1011A-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/crates/chord-voice/src/rtc_metrics.rs`.
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

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/crates/chord-voice/src/rtc_metrics.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);

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
 * Компилирует `rust/crates/chord-voice/src/rtc_metrics.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1011A: Сборщик RTCP метрик (RingBuffer 120, 60с с шагом 500мс) и мониторинг эпох DAVE MLS v1", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "rtc-metrics-1011a-"));
    testBinPath = join(workDir, "rtc_metrics_1011a_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1011A-0: проверяет статический контракт rust/crates/chord-voice/src/rtc_metrics.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024 и rustc --test -D warnings)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Отсутствует Rust-модуль ${RUST_MODULE_REL_PATH}`,
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде rtc_metrics.rs запрещены .unwrap() и .expect()",
    );

    const sizeCheck = checkRustSourceSizeLimits(
      RUST_MODULE_REL_PATH,
      rustSource,
    );
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines >= 250 && sizeCheck.fileLines < 800,
      `Ожидался размер модуля 250..799 строк, получено: ${sizeCheck.fileLines}`,
    );

    const secretFindings = scanTextForSecrets(rustSource, RUST_MODULE_REL_PATH);
    assert.equal(
      secretFindings.length,
      0,
      `Обнаружены секреты в исходнике: ${JSON.stringify(secretFindings)}`,
    );

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", RUST_MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check завершился с ошибкой: ${fmtCheck.stderr || fmtCheck.stdout}`,
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
      `Компиляция не должна порождать предупреждений:\n${compiled.stderr}`,
    );

    const allTests = runCompiledRustTests(testBinPath);
    assert.equal(
      allTests.status,
      0,
      `Ошибка выполнения встроенных Rust-тестов:\n${allTests.stderr || allTests.stdout}`,
    );
    assert.match(allTests.stdout, /test result: ok\./);
  });

  it("SUB-1011A-1: сверяет карточку TASK-1011A в waves/kanban-board.json, дизайн-токены, геометрию RTCP Collector (RingBuffer 120), состояния telemetry-probe и RE-оракул RE-GAP-22-VOICE-DIAGNOSTICS.md", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Оракул: карточка TASK-1011A в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1011A = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1011A",
    );
    assert.ok(
      card1011A,
      "Карточка TASK-1011A должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1011A.department, "G5 Voice / Platform");
    assert.deepEqual(card1011A.layout, {
      container: "RTCP Collector",
      dimensions: "RingBuffer 120",
      padding: "0px",
      spacing: "0px",
    });
    assert.deepEqual(card1011A.design_tokens, {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#23a55a",
      border: "#3f4147",
    });
    assert.deepEqual(card1011A.interactive_states[0], {
      element: "telemetry-probe",
      default: "polling",
      hover: "collect",
      active: "buffered",
    });

    // 2. Оракул: waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md
    const reGapRaw = readFileSync(RE_GAP_22_PATH, "utf8");
    assert.match(reGapRaw, /e > 500/);
    assert.match(reGapRaw, /e > 250/);
    assert.match(reGapRaw, /_outboundLossRate > 10/);
    assert.match(reGapRaw, /_outboundLossRate > 5/);
    assert.match(reGapRaw, /199/);
    assert.match(reGapRaw, /4000/);
    assert.match(reGapRaw, /epochAuthenticator/);

    // 3. Оракул: docs/re/voice-deep.json и фикстуры RTCP
    const voiceDeep = JSON.parse(readFileSync(VOICE_DEEP_JSON_PATH, "utf8"));
    assert.equal(voiceDeep.signaling.session_params.s2.ssrc, 17843);
    assert.equal(voiceDeep.signaling.session_params.s3.ssrc, 9208);
    assert.equal(voiceDeep.udp.rtcp.sr.pt, 200);
    assert.equal(voiceDeep.udp.rtcp.rr_server.pt, 201);

    const udp3 = JSON.parse(readFileSync(RTCP_UDP3_FIXTURE_PATH, "utf8"));
    const udp4 = JSON.parse(readFileSync(RTCP_UDP4_FIXTURE_PATH, "utf8"));
    assert.equal(udp3.packets.length, 12);
    assert.equal(udp4.packets.length, 20);
    assert.equal(udp3.packets[0].packets[0].payload_type, 201);
    assert.equal(udp3.packets[0].packets[0].sender_ssrc, 17843);

    // 4. Сверка констант в исходнике rtc_metrics.rs
    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1011A.design_tokens)) {
      assert.ok(
        rustSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} карточки TASK-1011A должен присутствовать в rtc_metrics.rs`,
      );
    }
    assert.ok(rustSource.includes("RTCP Collector"));
    assert.ok(rustSource.includes("RingBuffer 120"));
    assert.ok(rustSource.includes("telemetry-probe"));

    const run = runCompiledRustTests(testBinPath, "test_sub_1011a_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1011A-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1011a_1_.*ok/);
  });

  it("SUB-1011A-2: проверяет кольцевой буфер RtcpMetricsRingBuffer (120 измерений, 60с с шагом 500мс), парсинг RTCP SR/RR, расчёт RTT по LSR/DLSR, джиттера RFC 3550, дельты потерь, битрейта и качества ConnectionQuality", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "RTC_METRICS_RING_CAPACITY",
      "RTC_METRICS_STEP_MS",
      "RTC_METRICS_WINDOW_MS",
      "RtcpMetricsRingBuffer",
      "RtcTelemetrySample",
      "InterarrivalJitterEstimator",
      "PingHistory",
      "ConnectionQuality",
      "parse_rtcp_packet",
      "build_srtcp_sender_report",
      "build_srtcp_receiver_report",
      "compute_rtt_from_lsr_dlsr_ms",
      "compute_outbound_loss_delta_pct",
      "compute_bitrate_kbps",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В rtc_metrics.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1011a_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1011A-2 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1011a_2_.*ok/);
  });

  it("SUB-1011A-3: проверяет неблокирующий мониторинг эпох DAVE MLS v1 (DaveMlsEpochMonitor с атомарными хуками аудио-потока без мьютексов, opcodes 21..=31 и 30-значный Voice Privacy Code 6x5)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "DAVE_PROTOCOL_VERSION_V1",
      "VOICE_OP_DAVE_PREPARE_TRANSITION",
      "VOICE_OP_DAVE_EXECUTE_TRANSITION",
      "VOICE_OP_DAVE_READY_FOR_TRANSITION",
      "VOICE_OP_DAVE_PREPARE_EPOCH",
      "VOICE_OP_MLS_PROPOSALS",
      "VOICE_OP_MLS_COMMIT_WELCOME",
      "VOICE_OP_MLS_PREPARE_COMMIT",
      "DaveMlsEpochMonitor",
      "DaveEpochSnapshot",
      "derive_privacy_code_from_authenticator",
      "format_voice_privacy_code",
      "should_show_verification_code",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В rtc_metrics.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1011a_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1011A-3 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1011a_3_.*ok/);
  });

  it("SUB-1011A-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, HTTP 400..5xx, skeleton/202, права VIEW_CHANNEL/CONNECT/SPEAK/AGE_GATED_SPACES, восстановление после 3 таймаутов пинга и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "validate_snowflake_id",
      "evaluate_voice_metrics_permissions",
      "validate_platform_boundaries",
      "classify_rtc_http_status",
      "format_badge_cap",
      "redact_rtc_metrics_log",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В rtc_metrics.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1011a_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1011A-4 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1011a_4_.*ok/);
  });

  it("SUB-1011A-5: убивает 5/5 контрольных мутаций в rust/crates/chord-voice/src/rtc_metrics.rs (100% mutation kill rate)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      "Модуль rtc_metrics.rs должен существовать",
    );
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1011A-RS-01",
        moduleName: "rtc_metrics",
        operator: "RingBufferCapacity120Tamper",
        from: "pub const RTC_METRICS_RING_CAPACITY: usize = 120;",
        to: "pub const RTC_METRICS_RING_CAPACITY: usize = 12;",
      },
      {
        id: "MUT-1011A-RS-02",
        moduleName: "rtc_metrics",
        operator: "MetricsStep500MsTamper",
        from: "pub const RTC_METRICS_STEP_MS: u64 = 500;",
        to: "pub const RTC_METRICS_STEP_MS: u64 = 100;",
      },
      {
        id: "MUT-1011A-RS-03",
        moduleName: "rtc_metrics",
        operator: "PingFineMaxThresholdTamper",
        from: "pub const PING_FINE_MAX_MS: u64 = 250;",
        to: "pub const PING_FINE_MAX_MS: u64 = 50;",
      },
      {
        id: "MUT-1011A-RS-04",
        moduleName: "rtc_metrics",
        operator: "DaveDowngradeGuardBypass",
        from: "if requested_version < current_ver {",
        to: "if false && requested_version < current_ver {",
      },
      {
        id: "MUT-1011A-RS-05",
        moduleName: "rtc_metrics",
        operator: "TelemetryProbeBufferedStateTamper",
        from: 'Self::Buffered => "buffered",',
        to: 'Self::Buffered => "polling",',
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в rtc_metrics.rs: ${m.from}`,
      );
      return {
        id: m.id,
        moduleName: m.moduleName,
        operator: m.operator,
        mutatedSource: originalRust.replace(m.from, m.to),
      };
    });

    const gateResult = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutSrcPath = join(workDir, `mutant_${idx + 1}.rs`);
      const mutBinPath = join(workDir, `mutant_${idx + 1}_bin`);
      writeFileSync(mutSrcPath, mutant.mutatedSource, "utf8");
      const comp = compileRustModuleTests(mutSrcPath, mutBinPath);
      if (comp.status !== 0) {
        return { killed: true, reason: "compile_error" };
      }
      const run = runCompiledRustTests(mutBinPath);
      return {
        killed: run.status !== 0,
        reason: run.status !== 0 ? "test_failed" : "survived",
      };
    });

    const afterMd5 = createHash("md5")
      .update(readFileSync(RUST_MODULE_PATH, "utf8"))
      .digest("hex");
    assert.equal(
      afterMd5,
      originalMd5,
      "MD5 исходного файла rtc_metrics.rs не должен измениться после мутационного гейта",
    );
    assert.equal(
      gateResult.passed,
      true,
      `Выжившие мутанты: ${gateResult.survivors.join(", ")}`,
    );
    assert.equal(gateResult.killed, 5);
    assert.equal(gateResult.total, 5);
    assert.equal(gateResult.killRate, 1);
  });
});
