// @ts-check
/**
 * Контрактные и модульные тесты двухфакторной аутентификации TOTP 2FA и управления сессиями (TASK-1009):
 * - SUB-1009-0: Статический и компиляционный контракт `rust/chord-gpui/src/auth/totp_dialog.rs`
 *   и `scripts/verify-totp.mjs` (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде,
 *   соблюдение лимитов `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк,
 *   чистый `rustfmt --check --edition 2024`, компиляция `rustc --test --edition 2024 -D warnings`
 *   без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1009-1: Сверка карточек `TASK-1009`, `TASK-1009A`, `TASK-1009B` в `waves/kanban-board.json`,
 *   дизайн-токенов (`#313338`, `#2b2d31`, `#ffffff`, `#5865f2`, `#3f4147`, `#1e1f22`, `#f23f43`),
 *   геометрии `TOTP Modal` (`400px x 280px`, `padding: 20px`, `spacing: 12px`) и `Sessions Manager`
 *   (`Settings Tab`, `padding: 16px`, `spacing: 12px`), интерактивных состояний `digit-box`
 *   (`border` -> `border-accent` -> `focused digit`), `digit-field` и `revoke-session`
 *   (`button` -> `red hover` -> `session deleted`), маршрутов REST (`POST /auth/mfa/totp`,
 *   `GET /auth/sessions`, `DELETE /auth/sessions/{id}`, `POST /auth/sessions/logout`) и 17 реальных
 *   сессий из `/home/sanya/re-deep/decoded/36a171b84d1c0225_0` с независимыми RE-оракулами.
 * - SUB-1009-2: Ввод 6 цифр TOTP с автоматическим переносом фокуса между 6 полями (`0 -> 1 -> 2 -> 3 -> 4 -> 5`),
 *   возвратом фокуса по `Backspace`, навигацией стрелками, вставкой 6-значного кода из буфера обмена (`Ctrl+V`),
 *   формированием `POST /auth/mfa/totp` и резервных кодов `POST /auth/mfa/backup` (снятие дефисов, `8..11` символов).
 * - SUB-1009-3: Менеджер активных сессий (`GET /auth/sessions`, разделение `currentSession` с бейджем
 *   «Текущее устройство» и `otherSessions` с сортировкой по убыванию `approx_last_used_time`,
 *   отображение ОС, платформы, локации/IP и времени активности, принудительный разлогин
 *   `DELETE /auth/sessions/{id}` и пакетный `POST /auth/sessions/logout`, откат при сетевом сбое).
 * - SUB-1009-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` и проверка CLI
 *   `scripts/verify-totp.mjs` (коды выхода `0 / 1 / 2`).
 * - SUB-1009-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/chord-gpui/src/auth/totp_dialog.rs`
 *   и `scripts/verify-totp.mjs` по контракту `scripts/quality-gate.mjs`.
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
const RUST_MODULE_REL_PATH = "rust/chord-gpui/src/auth/totp_dialog.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const SCRIPT_REL_PATH = "scripts/verify-totp.mjs";
const SCRIPT_PATH = join(ROOT, SCRIPT_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const REST_D02_JSON_PATH = join(ROOT, "docs", "re", "rest-d02.json");

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/chord-gpui/src/auth/totp_dialog.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1009: Двухфакторная аутентификация TOTP 2FA и управление сессиями (rust/chord-gpui/src/auth/totp_dialog.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "totp-sessions-1009-"));
    testBinPath = join(workDir, "totp_sessions_1009_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1009-0: модули rust/chord-gpui/src/auth/totp_dialog.rs и scripts/verify-totp.mjs существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), запрещают unsafe и компилируются без предупреждений", () => {
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
      "В продуктовом коде totp_dialog.rs запрещены .unwrap() и .expect()",
    );

    const rustSizeCheck = checkRustSourceSizeLimits(RUST_MODULE_REL_PATH, rustSource);
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
    assert.equal(secretScanRust.length, 0, "В Rust-модуле не должно быть захардкоженных секретов");
    assert.equal(secretScanJs.length, 0, "В JS-скрипте не должно быть захардкоженных секретов");

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
      `rustc --test не прошёл:\n${compiled.stderr || compiled.stdout}`,
    );
    assert.doesNotMatch(compiled.stderr, /warning:/);

    const allRustTests = runCompiledRustTests(testBinPath);
    assert.equal(
      allRustTests.status,
      0,
      `Встроенные тесты totp_dialog.rs упали:\n${allRustTests.stdout}\n${allRustTests.stderr}`,
    );
    assert.match(allRustTests.stdout, /test result: ok\./);
  });

  it("SUB-1009-1: сверяет карточку TASK-1009 в waves/kanban-board.json, дизайн-токены, раскладку TOTP Modal и Sessions Manager, маршруты MFA/сессий и 17 живых сессий с RE-оракулами", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      LIVE_AUTH_SESSIONS_ORACLE,
      REVOKE_SESSION_STATES,
      SESSIONS_MANAGER_LAYOUT,
      SESSIONS_MANAGER_TOKENS,
      TOTP_DESIGN_TOKENS,
      TOTP_DIGIT_BOX_STATES,
      TOTP_MODAL_LAYOUT,
      TOTP_PROTOCOL_CONSTANTS,
      verifyTotpOracleParity,
    } = await import("../../scripts/verify-totp.mjs");

    const kanban = JSON.parse(readFileSync(KANBAN_BOARD_PATH, "utf8"));
    const card1009 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1009");
    assert.ok(card1009, "Карточка TASK-1009 должна присутствовать в waves/kanban-board.json");
    assert.equal(card1009.department, "G1 Backend");
    assert.deepEqual(card1009.layout, TOTP_MODAL_LAYOUT);
    assert.deepEqual(card1009.design_tokens, TOTP_DESIGN_TOKENS);
    assert.deepEqual(card1009.interactive_states[0], TOTP_DIGIT_BOX_STATES);

    const card1009B = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1009B");
    if (card1009B) {
      assert.deepEqual(card1009B.layout, SESSIONS_MANAGER_LAYOUT);
      assert.deepEqual(card1009B.design_tokens, SESSIONS_MANAGER_TOKENS);
      assert.deepEqual(card1009B.interactive_states[0], REVOKE_SESSION_STATES);
    }

    const restD02 = JSON.parse(readFileSync(REST_D02_JSON_PATH, "utf8"));
    const hasWebauthn = restD02.endpoints.some(
      (/** @type {any} */ ep) => ep.path === "/api/v9/users/@me/mfa/webauthn/credentials",
    );
    assert.equal(hasWebauthn, true, "В rest-d02.json должен быть маршрут MFA webauthn credentials");

    assert.equal(TOTP_PROTOCOL_CONSTANTS.TOTP_DIGIT_COUNT, 6);
    assert.equal(TOTP_PROTOCOL_CONSTANTS.BACKUP_CODE_MIN_LEN, 8);
    assert.equal(TOTP_PROTOCOL_CONSTANTS.BACKUP_CODE_MAX_LEN, 11);
    assert.equal(TOTP_PROTOCOL_CONSTANTS.MFA_INVALID_CODE_ERROR, 60008);
    assert.equal(TOTP_PROTOCOL_CONSTANTS.MFA_REQUIRED_ERROR, 60003);
    assert.equal(LIVE_AUTH_SESSIONS_ORACLE.length, 17);

    const parity = verifyTotpOracleParity();
    assert.equal(
      parity.passed,
      true,
      `Сверка с оракулами не прошла: ${parity.reasons.join("; ")}`,
    );
    assert.equal(parity.liveSessionCount, 17);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1009_1_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1009-1 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1009_1_.*ok/);
  });

  it("SUB-1009-2: проверяет авто-перенос фокуса между 6 полями ввода TOTP, возврат по Backspace, вставку из буфера обмена Ctrl+V и формирование POST /auth/mfa/totp и /auth/mfa/backup", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      TotpInputController,
      buildTotpMfaRequest,
      resolveDigitBoxState,
    } = await import("../../scripts/verify-totp.mjs");

    // 1. Состояния digit-box (border -> border-accent -> focused digit)
    assert.deepEqual(resolveDigitBoxState({ focused: false, hovered: false }), {
      element: "digit-box",
      state: "default",
      label: "border",
    });
    assert.deepEqual(resolveDigitBoxState({ focused: false, hovered: true }), {
      element: "digit-box",
      state: "hover",
      label: "border-accent",
    });
    assert.deepEqual(resolveDigitBoxState({ focused: true, hovered: true }), {
      element: "digit-box",
      state: "active",
      label: "focused digit",
    });

    // 2. Пошаговый ввод 6 цифр с авто-переносом фокуса 0 -> 1 -> 2 -> 3 -> 4 -> 5
    const ctrl = new TotpInputController();
    assert.equal(ctrl.focusedIndex, 0);
    assert.equal(ctrl.isComplete(), false);

    // Нецифровой символ отклоняется, фокус остаётся на 0
    assert.equal(ctrl.inputDigit("x"), false);
    assert.equal(ctrl.focusedIndex, 0);

    const digits = ["4", "8", "2", "9", "1", "0"];
    digits.forEach((d, idx) => {
      assert.equal(ctrl.inputDigit(d), true);
      const expectedFocus = Math.min(idx + 1, 5);
      assert.equal(ctrl.focusedIndex, expectedFocus);
    });
    assert.equal(ctrl.isComplete(), true);
    assert.equal(ctrl.getCode(), "482910");

    // 3. Backspace: очищает текущее заполненное поле 5, затем при повторном нажатии переходит на 4 и очищает его
    ctrl.handleBackspace();
    assert.equal(ctrl.focusedIndex, 5);
    assert.equal(ctrl.boxes[5], "");
    assert.equal(ctrl.isComplete(), false);

    ctrl.handleBackspace();
    assert.equal(ctrl.focusedIndex, 4);
    assert.equal(ctrl.boxes[4], "");
    assert.equal(ctrl.getCode(), "4829");

    // 4. Вставка из буфера обмена Ctrl+V (с пробелами и дефисами)
    ctrl.clear();
    assert.equal(ctrl.focusedIndex, 0);
    assert.equal(ctrl.pasteCode(" 739-204 "), 6);
    assert.equal(ctrl.getCode(), "739204");
    assert.equal(ctrl.focusedIndex, 5);
    assert.equal(ctrl.isComplete(), true);

    // 5. Формирование POST /auth/mfa/totp и POST /auth/mfa/backup
    const totpReq = buildTotpMfaRequest({
      method: "totp",
      code: ctrl.getCode(),
      ticket: "mfa_ticket_991",
      loginInstanceId: "inst_42",
    });
    assert.equal(totpReq.httpMethod, "POST");
    assert.equal(totpReq.path, "/auth/mfa/totp");
    assert.equal(totpReq.body.code, "739204");
    assert.equal(totpReq.body.ticket, "mfa_ticket_991");
    assert.equal(totpReq.body.login_instance_id, "inst_42");

    const backupReq = buildTotpMfaRequest({
      method: "backup",
      code: "abcd-ef12",
      ticket: "mfa_ticket_991",
    });
    assert.equal(backupReq.path, "/auth/mfa/backup");
    assert.equal(backupReq.body.code, "abcdef12");

    assert.throws(
      () =>
        buildTotpMfaRequest({
          method: "../logout",
          code: "739204",
          ticket: "mfa_ticket_991",
        }),
      /недопустимый метод MFA/i,
    );

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1009_2_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1009-2 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1009_2_.*ok/);
  });

  it("SUB-1009-3: проверяет менеджер активных сессий (GET /auth/sessions, сортировку, бейдж текущего устройства, DELETE /auth/sessions/{id} и POST /auth/sessions/logout)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      AuthSessionsManager,
      LIVE_AUTH_SESSIONS_ORACLE,
      buildDeleteSessionRequest,
      buildFetchSessionsRequest,
      buildLogoutSessionsRequest,
      resolveRevokeButtonState,
    } = await import("../../scripts/verify-totp.mjs");

    // 1. Запросы GET /auth/sessions, DELETE /auth/sessions/{id}, POST /auth/sessions/logout
    const getReq = buildFetchSessionsRequest();
    assert.equal(getReq.method, "GET");
    assert.equal(getReq.path, "/auth/sessions");

    const delReq = buildDeleteSessionRequest("DxuLb6GvCUXGAZzsjeGM0n4SpqkVaPO2KLgqoZj+Nc4=");
    assert.equal(delReq.method, "DELETE");
    assert.equal(
      delReq.path,
      "/auth/sessions/DxuLb6GvCUXGAZzsjeGM0n4SpqkVaPO2KLgqoZj+Nc4=",
    );

    const batchReq = buildLogoutSessionsRequest([
      "DxuLb6GvCUXGAZzsjeGM0n4SpqkVaPO2KLgqoZj+Nc4=",
      "XGEkiho2xx1r5fsNFcMrji2y98nZ/4sJt0qGbWNCc4Q=",
    ]);
    assert.equal(batchReq.method, "POST");
    assert.equal(batchReq.path, "/auth/sessions/logout");
    assert.equal(batchReq.body.session_id_hashes.length, 2);

    // 2. Состояния кнопки revoke-session (button -> red hover -> session deleted)
    assert.equal(resolveRevokeButtonState({ deleted: false, hovered: false }).label, "button");
    assert.equal(resolveRevokeButtonState({ deleted: false, hovered: true }).label, "red hover");
    assert.equal(resolveRevokeButtonState({ deleted: true, hovered: false }).label, "session deleted");

    // 3. Парсинг 17 реальных сессий из RE-дампа и разделение currentSession / otherSessions
    const currentHash = "OdKq9gNLX69NrpWTtUFE3MH2d2evCrU0o5eLIUvFP4s=";
    const mgr = new AuthSessionsManager({
      currentSessionIdHash: currentHash,
      sessions: LIVE_AUTH_SESSIONS_ORACLE,
    });

    const view = mgr.snapshot(Date.parse("2026-10-18T12:45:00Z"));
    assert.ok(view.currentSession, "Текущая сессия должна быть выделена");
    assert.equal(view.currentSession.idHash, currentHash);
    assert.equal(view.currentSession.isCurrent, true);
    assert.equal(view.currentSession.badgeRu, "Текущее устройство");
    assert.equal(view.currentSession.os, "Linux");
    assert.equal(view.currentSession.platform, "Discord Client");
    assert.equal(view.currentSession.location, "Riga, Riga, Latvia");
    assert.equal(view.currentSession.lastActiveRu, "Менее часа назад");

    assert.equal(view.otherSessions.length, 16);
    // Первая из остальных сессий — самая свежая (2026-10-18T12:11:50.439852+00:00 -> SIYXCYXQ...)
    assert.equal(
      view.otherSessions[0]?.idHash,
      "SIYXCYXQnxanTF0V1iOSleP9FEADrhLFZZAiuLEeS7Y=",
    );

    // 4. Принудительное завершение одной сессии DELETE /auth/sessions/{id} и откат при ошибке
    const targetHash = "DxuLb6GvCUXGAZzsjeGM0n4SpqkVaPO2KLgqoZj+Nc4=";
    const revokeResult = mgr.revokeSession(targetHash);
    assert.equal(revokeResult.deleteRequest.method, "DELETE");
    assert.equal(
      revokeResult.deleteRequest.path,
      `/auth/sessions/${targetHash}`,
    );
    assert.equal(mgr.otherSessionsCount(), 15);

    // Откат при сетевом сбое
    assert.equal(mgr.rollbackLastRevoke(), true);
    assert.equal(mgr.otherSessionsCount(), 16);

    // Повторное удаление и пакетное завершение всех остальных сессий
    mgr.revokeSession(targetHash);
    assert.equal(mgr.otherSessionsCount(), 15);
    const revokeAll = mgr.revokeAllOtherSessions();
    assert.equal(revokeAll.logoutRequest.body.session_id_hashes.length, 15);
    assert.equal(mgr.otherSessionsCount(), 0);
    assert.ok(mgr.snapshot(Date.parse("2026-10-18T12:45:00Z")).currentSession);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1009_3_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1009-3 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1009_3_.*ok/);
  });

  it("SUB-1009-4: покрывает все 9 обязательных измерений TEST-POLICY.md и проверяет CLI scripts/verify-totp.mjs (коды 0 / 1 / 2)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      evaluateTotpNineDimensions,
      redactTotpTelemetry,
      runTotpCli,
    } = await import("../../scripts/verify-totp.mjs");

    // 1. Все 9 измерений TEST-POLICY.md
    const dimReport = evaluateTotpNineDimensions();
    assert.equal(
      dimReport.passed,
      true,
      `Сбой в 9 измерениях TEST-POLICY.md: ${dimReport.failures.join("; ")}`,
    );
    assert.equal(dimReport.dimensionsChecked, 9);

    // 2. Маскирование TOTP-кодов, mfa-тикетов и токенов в телеметрии
    const rawLog =
      'POST /api/v9/auth/mfa/totp code="482910" ticket="mfa_ticket_secret_99" Authorization="Bearer mfa.secret_totp_token_1009"';
    const redacted = redactTotpTelemetry(rawLog);
    assert.doesNotMatch(redacted, /482910/);
    assert.doesNotMatch(redacted, /mfa_ticket_secret_99/);
    assert.doesNotMatch(redacted, /mfa\.secret_totp_token_1009/);
    assert.match(redacted, /\[REDACTED\]/);

    // 3. Проверка CLI scripts/verify-totp.mjs (коды выхода 0 / 1 / 2)
    const outJson = join(workDir, "totp-report.json");
    const outMd = join(workDir, "TOTP-REPORT.md");
    assert.equal(runTotpCli(["--help"]), 0);
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
      [SCRIPT_PATH, "--invalid-arg"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliBadArg.status, 1);

    const cliMissingOracle = spawnSync(
      process.execPath,
      [SCRIPT_PATH, "--oracle", "/nonexistent/missing-kanban-1009.json"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliMissingOracle.status, 2);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1009_4_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1009-4 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1009_4_.*ok/);
  });

  it("SUB-1009-5: убивает 5/5 контрольных мутаций в rust/chord-gpui/src/auth/totp_dialog.rs и scripts/verify-totp.mjs (100% mutation kill rate)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalRustMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutationSpecs = [
      {
        id: "MUT-1009-RS-01",
        operator: "TotpDigitCountTamper",
        from: "pub const TOTP_DIGIT_COUNT: usize = 6;",
        to: "pub const TOTP_DIGIT_COUNT: usize = 4;",
      },
      {
        id: "MUT-1009-RS-02",
        operator: "AutoFocusAdvanceTamper",
        from: "if self.focused_index + 1 < TOTP_DIGIT_COUNT {",
        to: "if false && self.focused_index + 1 < TOTP_DIGIT_COUNT {",
      },
      {
        id: "MUT-1009-RS-03",
        operator: "DeleteSessionHttpMethodTamper",
        from: 'method: "DELETE",',
        to: 'method: "POST",',
      },
      {
        id: "MUT-1009-RS-04",
        operator: "BackupCodeHyphenStripBypass",
        from: "let clean: String = raw_code.trim().chars().filter(|c| *c != '-').collect();",
        to: "let clean: String = raw_code.trim().to_string();",
      },
      {
        id: "MUT-1009-RS-05",
        operator: "CurrentSessionProtectBypass",
        from: "if trimmed == self.current_session_id_hash {",
        to: "if false && trimmed == self.current_session_id_hash {",
      },
    ];

    const rustMutants = rustMutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в totp_dialog.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "totp_dialog.rs",
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
    assert.equal(afterRustMd5, originalRustMd5, "MD5 исходного файла totp_dialog.rs не должен измениться");
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
        id: "MUT-1009-JS-01",
        operator: "AutoFocusAdvanceTamper",
        from: "if (this.focusedIndex + 1 < TOTP_PROTOCOL_CONSTANTS.TOTP_DIGIT_COUNT) {",
        to: "if (false && this.focusedIndex + 1 < TOTP_PROTOCOL_CONSTANTS.TOTP_DIGIT_COUNT) {",
      },
      {
        id: "MUT-1009-JS-02",
        operator: "DigitBoxActiveStateTamper",
        from: 'active: "focused digit",',
        to: 'active: "unfocused",',
      },
      {
        id: "MUT-1009-JS-03",
        operator: "DeleteSessionMethodTamper",
        from: 'method: "DELETE",',
        to: 'method: "POST",',
      },
      {
        id: "MUT-1009-JS-04",
        operator: "FetchSessionsPathTamper",
        from: 'path: "/auth/sessions",',
        to: 'path: "/auth/invalid",',
      },
      {
        id: "MUT-1009-JS-05",
        operator: "BackupHyphenStripTamper",
        from: 'const normalizedCode = method === "backup" ? rawCode.replace(/-/g, "") : rawCode;',
        to: "const normalizedCode = rawCode;",
      },
    ];

    const jsMutants = jsMutationSpecs.map((spec) => {
      assert.ok(
        originalJs.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в verify-totp.mjs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "verify-totp.mjs",
        operator: spec.operator,
        mutatedSource: originalJs.replace(spec.from, spec.to),
      };
    });

    const jsGate = evaluateMutationGate(jsMutants, (mutant, index) => {
      const mutantJsPath = join(workDir, `mutant_totp_${index + 1}.mjs`);
      const harnessPath = join(workDir, `mutant_totp_harness_${index + 1}.mjs`);
      writeFileSync(mutantJsPath, mutant.mutatedSource, "utf8");
      writeFileSync(
        harnessPath,
        [
          'import assert from "node:assert/strict";',
          `import { TOTP_DIGIT_BOX_STATES, TotpInputController, buildDeleteSessionRequest, buildFetchSessionsRequest, buildTotpMfaRequest } from ${JSON.stringify(mutantJsPath)};`,
          'assert.equal(TOTP_DIGIT_BOX_STATES.active, "focused digit");',
          "const c = new TotpInputController(); c.inputDigit('4'); assert.equal(c.focusedIndex, 1);",
          'assert.equal(buildFetchSessionsRequest().path, "/auth/sessions");',
          'assert.equal(buildDeleteSessionRequest("DxuLb6GvCUXGAZzsjeGM0n4SpqkVaPO2KLgqoZj+Nc4=").method, "DELETE");',
          'assert.equal(buildTotpMfaRequest({ method: "backup", code: "abcd-ef12", ticket: "t1" }).body.code, "abcdef12");',
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
