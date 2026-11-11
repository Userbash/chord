// @ts-check
/**
 * Контрактные и модульные тесты аудита безопасности Unix-сокетов `discord-ipc-0..9`
 * и контроля доступа по UID (`SO_PEERCRED`, права `0600`, жёсткий лимит фрейма `64 KB`) — TASK-980:
 * - SUB-980-1: Сверка констант протокола Discord Local RPC IPC (`discord-ipc-0..9`, слоты `0..9`,
 *   права `0o600`, `SO_PEERCRED = 17`, `SOL_SOCKET = 1`, `RPC_LISTEN_BACKLOG = 511`, жёсткий лимит
 *   фрейма `64 KB = 65536` байт, опкоды `0..4`) с независимыми RE-оракулами
 *   (`/home/sanya/re-deep/voice/strace-renderer-s3.log`, `/home/sanya/re-deep/discord-run3.log`,
 *   `rust/crates/discord-platform/src/rpc_ipc.rs`, `waves/kanban-board.json`).
 * - SUB-980-2: Аудит прав доступа `0o600` на Unix-сокеты `discord-ipc-0..9`, проверка безопасности
 *   родительского каталога (`0o700` / sticky `/tmp`), защита `(dev, ino)` при очистке и восстановление
 *   после осиротевших (stale) сокетов.
 * - SUB-980-3: Валидация UID клиента через `SO_PEERCRED` (`struct ucred { pid, uid, gid }` 12 байт LE):
 *   принятие соединений с совпадающим UID (`peer_uid === expected_uid`) и немедленный отказ/разрыв
 *   при несовпадающем UID (`peer_uid !== expected_uid`), повреждённом `ucred` или подмене PID.
 * - SUB-980-4: Жёсткий лимит фрейма `64 KB` (`65536` байт) на уровне бинарного протокола и живого
 *   `AF_UNIX` сервера/клиента (`node:net`): прохождение фреймов `<= 65536` байт и немедленное
 *   расторжение соединения при `length > 65536` байт (`65537`, `128 KB`, `0xFFFFFFFF`) без аллокации
 *   огромного буфера (защита от OOM/DoS).
 * - SUB-980-5: Покрытие всех 9 измерений `TEST-POLICY.md`, маскирование токенов в логах аудита,
 *   CLI `scripts/ipc-security-audit.mjs` (коды выхода `0 / 1 / 2`), лимиты размера кода и мутационный
 *   гейт `5/5` (`100%` kill rate).
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import net from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  checkRustSourceSizeLimits,
  evaluateMutationGate,
} from "../../scripts/quality-gate.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SCRIPT_PATH = join(ROOT, "scripts", "ipc-security-audit.mjs");
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const RUST_RPC_IPC_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-platform",
  "src",
  "rpc_ipc.rs",
);

describe("TASK-980: Аудит безопасности Unix сокетов discord-ipc-0..9 и UID Access Control (scripts/ipc-security-audit.mjs)", () => {
  it("SUB-980-1: сверяет константы безопасности discord-ipc-0..9 (0o600, SO_PEERCRED, 64 KB) с RE-оракулами и waves/kanban-board.json", async () => {
    const {
      IPC_SECURITY_DESIGN_TOKENS,
      IPC_CLIENT_STATES,
      IPC_SECURITY_CONSTANTS,
      RPC_OPCODES,
      encodeUcred,
      decodeUcred,
      parseIpcSlotName,
      resolveIpcSocketCandidates,
      verifyIpcSecurityOracleParity,
    } = await import("../../scripts/ipc-security-audit.mjs");

    // 1. Независимый оракул: сверка дизайн-токенов, состояний и критериев с карточкой TASK-980 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card980 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-980");
    assert.ok(card980, "Карточка TASK-980 должна присутствовать в waves/kanban-board.json");
    assert.deepEqual(IPC_SECURITY_DESIGN_TOKENS, card980.design_tokens);
    assert.deepEqual(IPC_CLIENT_STATES, card980.interactive_states[0]);

    // 2. Проверка констант безопасности сокетов discord-ipc-0..9
    assert.equal(IPC_SECURITY_CONSTANTS.UNIX_SOCKET_PREFIX, "discord-ipc-");
    assert.equal(IPC_SECURITY_CONSTANTS.WINDOWS_PIPE_PREFIX, "\\\\.\\pipe\\discord-ipc-");
    assert.equal(IPC_SECURITY_CONSTANTS.MIN_SLOT, 0);
    assert.equal(IPC_SECURITY_CONSTANTS.MAX_SLOT, 9);
    assert.equal(IPC_SECURITY_CONSTANTS.SLOT_COUNT, 10);
    assert.equal(IPC_SECURITY_CONSTANTS.REQUIRED_SOCKET_MODE, 0o600);
    assert.equal(IPC_SECURITY_CONSTANTS.FORBIDDEN_MODE_MASK, 0o177);
    assert.equal(IPC_SECURITY_CONSTANTS.IPC_HEADER_BYTES, 8);
    assert.equal(IPC_SECURITY_CONSTANTS.HARD_MAX_FRAME_BYTES, 64 * 1024);
    assert.equal(IPC_SECURITY_CONSTANTS.HARD_MAX_FRAME_BYTES, 65_536);
    assert.equal(IPC_SECURITY_CONSTANTS.SOL_SOCKET_LINUX, 1);
    assert.equal(IPC_SECURITY_CONSTANTS.SO_PEERCRED_LINUX, 17);
    assert.equal(IPC_SECURITY_CONSTANTS.UCRED_STRUCT_BYTES, 12);
    assert.equal(IPC_SECURITY_CONSTANTS.LISTEN_BACKLOG, 511);
    assert.equal(IPC_SECURITY_CONSTANTS.RPC_PROTOCOL_VERSION, 1);
    assert.equal(IPC_SECURITY_CONSTANTS.RPC_ERROR_CODE_INVALID_PAYLOAD, 4000);

    // 3. Проверка опкодов бинарного протокола Discord Local RPC IPC
    assert.deepEqual(RPC_OPCODES, {
      HANDSHAKE: 0,
      FRAME: 1,
      CLOSE: 2,
      PING: 3,
      PONG: 4,
    });

    // 4. Кодирование и декодирование 12-байтной структуры Linux ucred (pid: i32 LE, uid: u32 LE, gid: u32 LE)
    const rawUcred = encodeUcred({ pid: 95974, uid: 1000, gid: 1000 });
    assert.equal(rawUcred.length, 12);
    const parsedUcred = decodeUcred(rawUcred);
    assert.deepEqual(parsedUcred, { valid: true, pid: 95974, uid: 1000, gid: 1000, reason: "OK" });

    const shortUcred = decodeUcred(Buffer.alloc(8));
    assert.equal(shortUcred.valid, false);
    assert.match(shortUcred.reason, /12/);

    // 5. Разбор имён слотов discord-ipc-0..9 и приоритета переменных окружения
    for (let slot = 0; slot <= 9; slot += 1) {
      assert.equal(parseIpcSlotName(`discord-ipc-${slot}`), slot);
      assert.equal(parseIpcSlotName(`/run/user/1000/discord-ipc-${slot}`), slot);
      assert.equal(parseIpcSlotName(`\\\\.\\pipe\\discord-ipc-${slot}`), slot);
    }
    for (const bad of ["", "discord-ipc-", "discord-ipc-10", "discord-ipc-00", "discord-ipc-abc", "../discord-ipc-0"]) {
      assert.equal(parseIpcSlotName(bad), null, `Ожидался отказ для ${bad}`);
    }

    const candidates = resolveIpcSocketCandidates({
      XDG_RUNTIME_DIR: "/run/user/1000",
      TMPDIR: "/var/tmp",
    });
    assert.equal(candidates.length, 10);
    assert.equal(candidates[0], "/run/user/1000/discord-ipc-0");
    assert.equal(candidates[9], "/run/user/1000/discord-ipc-9");

    // 6. Сверка с rust/crates/discord-platform/src/rpc_ipc.rs и RE-логами
    const parity = verifyIpcSecurityOracleParity({
      kanbanPath: KANBAN_BOARD_PATH,
      rustRpcIpcPath: RUST_RPC_IPC_PATH,
    });
    assert.equal(parity.passed, true, `Нарушение паритета с оракулами: ${parity.reasons.join("; ")}`);
    assert.equal(parity.rustHasMode0600, true);
    assert.equal(parity.rustHasSoPeercred, true);
    assert.equal(parity.rustHasInodeGuard, true);
  });

  it("SUB-980-2: проверяет права 0600 на сокетах discord-ipc-0..9, права каталога и защиту (dev, ino)", async () => {
    const {
      auditIpcDirectorySecurity,
      auditIpcSocketFileSecurity,
      verifyOwnedInodeCleanup,
    } = await import("../../scripts/ipc-security-audit.mjs");

    const tmp = mkdtempSync(join(tmpdir(), "ipc-sec-mode-980-"));
    try {
      chmodSync(tmp, 0o700);
      const currentUid = typeof process.geteuid === "function" ? process.geteuid() : 1000;

      // 1. Безопасный каталог 0o700 проходит аудит
      const safeDirAudit = auditIpcDirectorySecurity(tmp, currentUid);
      assert.equal(safeDirAudit.passed, true, safeDirAudit.reason);

      // 2. Каталог с правами 0o777 (world-writable без sticky bit) отклоняется
      const unsafeDir = join(tmp, "unsafe-world-writable");
      mkdirSyncWithMode(unsafeDir, 0o777);
      const unsafeDirAudit = auditIpcDirectorySecurity(unsafeDir, currentUid);
      assert.equal(unsafeDirAudit.passed, false);
      assert.equal(unsafeDirAudit.errorCode, "UNSAFE_DIRECTORY_PERMISSIONS");

      // 3. Живой Unix-сокет discord-ipc-0 с правами 0o600 проходит аудит
      const sockPath = join(tmp, "discord-ipc-0");
      const server = net.createServer();
      await new Promise((resolve, reject) => {
        server.once("error", reject);
        server.listen(sockPath, () => resolve(undefined));
      });
      try {
        chmodSync(sockPath, 0o600);
        const goodSockAudit = auditIpcSocketFileSecurity(sockPath, currentUid);
        assert.equal(goodSockAudit.passed, true, goodSockAudit.reason);
        assert.equal(goodSockAudit.slot, 0);
        assert.equal(goodSockAudit.modeOctal, "0600");
        assert.equal(goodSockAudit.isSocket, true);
        assert.equal(goodSockAudit.uid, currentUid);

        // 4. Ослабленные права (0o644, 0o660, 0o666, 0o777) на сокете отклоняются аудитом
        for (const badMode of [0o644, 0o660, 0o666, 0o700, 0o777]) {
          chmodSync(sockPath, badMode);
          const badModeAudit = auditIpcSocketFileSecurity(sockPath, currentUid);
          assert.equal(
            badModeAudit.passed,
            false,
            `Режим 0o${badMode.toString(8)} обязан отклоняться как небезопасный`,
          );
          assert.equal(badModeAudit.errorCode, "SOCKET_MODE_VIOLATION");
        }

        // Возвращаем 0o600 и проверяем отказ при несовпадении владельца файла сокета
        chmodSync(sockPath, 0o600);
        const wrongOwnerAudit = auditIpcSocketFileSecurity(sockPath, currentUid + 1);
        assert.equal(wrongOwnerAudit.passed, false);
        assert.equal(wrongOwnerAudit.errorCode, "SOCKET_UID_MISMATCH");
      } finally {
        await new Promise((resolve) => server.close(() => resolve(undefined)));
      }

      // 5. Обычный файл на месте discord-ipc-1 отклоняется с NOT_A_UNIX_SOCKET и защищён от удаления по (dev, ino)
      const regularPath = join(tmp, "discord-ipc-1");
      writeFileSync(regularPath, "not-a-socket", { mode: 0o600 });
      const regAudit = auditIpcSocketFileSecurity(regularPath, currentUid);
      assert.equal(regAudit.passed, false);
      assert.equal(regAudit.errorCode, "NOT_A_UNIX_SOCKET");

      const regStat = statSync(regularPath);
      const cleanupMismatch = verifyOwnedInodeCleanup(regularPath, {
        expectedDev: regStat.dev,
        expectedIno: regStat.ino + 99999,
      });
      assert.equal(cleanupMismatch.allowedToDelete, false);
      assert.equal(existsSync(regularPath), true);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-980-3: проверяет UID через SO_PEERCRED и отклоняет подключения от чужого UID", async () => {
    const {
      verifyPeerCredentials,
      encodeUcred,
      IPC_CLIENT_STATES,
    } = await import("../../scripts/ipc-security-audit.mjs");

    // 1. Позитивный сценарий: совпадающий UID (1000 === 1000) и валидный PID > 0
    const validCredBuf = encodeUcred({ pid: 4242, uid: 1000, gid: 1000 });
    const okPeer = verifyPeerCredentials(validCredBuf, 1000);
    assert.equal(okPeer.allowed, true);
    assert.equal(okPeer.errorCode, null);
    assert.equal(okPeer.peerUid, 1000);
    assert.equal(okPeer.expectedUid, 1000);
    assert.equal(okPeer.peerPid, 4242);
    assert.deepEqual(okPeer.stateTransition, [
      IPC_CLIENT_STATES.default,
      IPC_CLIENT_STATES.hover,
      IPC_CLIENT_STATES.active,
    ]);

    // 2. Негативный сценарий А: чужой пользователь (peer_uid = 1001, expected_uid = 1000) отклоняется
    const foreignUserBuf = encodeUcred({ pid: 5555, uid: 1001, gid: 1001 });
    const rejectedForeign = verifyPeerCredentials(foreignUserBuf, 1000);
    assert.equal(rejectedForeign.allowed, false);
    assert.equal(rejectedForeign.errorCode, "UNAUTHORIZED_PEER_UID");
    assert.equal(rejectedForeign.peerUid, 1001);
    assert.equal(rejectedForeign.expectedUid, 1000);
    assert.equal(rejectedForeign.disconnectRequired, true);

    // 3. Негативный сценарий Б: nobody (65534) и root (0) при expected_uid = 1000 также отклоняются
    for (const attackerUid of [0, 65534, 999, 0xffffffff]) {
      const buf = encodeUcred({ pid: 6666, uid: attackerUid, gid: attackerUid });
      const res = verifyPeerCredentials(buf, 1000);
      assert.equal(res.allowed, false, `UID ${attackerUid} обязан отклоняться при expectedUid=1000`);
      assert.equal(res.errorCode, "UNAUTHORIZED_PEER_UID");
      assert.equal(res.disconnectRequired, true);
    }

    // 4. Негативный сценарий В: нулевой/отрицательный PID (например, невалидный/анонимный ucred pid <= 0)
    const zeroPidBuf = encodeUcred({ pid: 0, uid: 1000, gid: 1000 });
    const rejectedPid = verifyPeerCredentials(zeroPidBuf, 1000);
    assert.equal(rejectedPid.allowed, false);
    assert.equal(rejectedPid.errorCode, "INVALID_PEER_PID");
    assert.equal(rejectedPid.disconnectRequired, true);
  });

  it("SUB-980-4: проверяет жёсткий лимит фрейма 64 KB (65536 Б) и разрыв живого Unix-сокета при превышении лимита или чужом UID", async () => {
    const {
      encodeIpcFrame,
      decodeIpcFrameChecked,
      runLiveIpcSecurityAuditHarness,
    } = await import("../../scripts/ipc-security-audit.mjs");

    // 1. Граничная проверка парсера фреймов: ровно 64 KB (65536 байт) проходит
    const exact64KbJson = JSON.stringify({
      cmd: "SET_ACTIVITY",
      nonce: "boundary-64kb",
      pad: "A".repeat(65_536 - 55),
    });
    assert.equal(Buffer.byteLength(exact64KbJson, "utf8"), 65_536);

    const valid64KbFrame = encodeIpcFrame(1, exact64KbJson);
    assert.equal(valid64KbFrame.length, 8 + 65_536);
    const decoded64Kb = decodeIpcFrameChecked(valid64KbFrame);
    assert.equal(decoded64Kb.valid, true, decoded64Kb.reason);
    assert.equal(decoded64Kb.opcode, 1);
    assert.equal(decoded64Kb.payloadLength, 65_536);
    assert.equal(decoded64Kb.disconnectRequired, false);

    // 2. Превышение на 1 байт (65537 байт > 64 KB) и экстремальные длины (1 МБ, 0xFFFFFFFF) немедленно отклоняются с разрывом соединения
    for (const oversizedLen of [65_537, 131_072, 1024 * 1024, 0xffffffff]) {
      const badHeader = Buffer.alloc(8);
      badHeader.writeUInt32LE(1, 0);
      badHeader.writeUInt32LE(oversizedLen >>> 0, 4);
      const res = decodeIpcFrameChecked(badHeader);
      assert.equal(res.valid, false, `Фрейм длиной ${oversizedLen} обязан отклоняться`);
      assert.equal(res.errorCode, "FRAME_TOO_LARGE");
      assert.equal(res.disconnectRequired, true);
      assert.equal(res.payloadLength, oversizedLen >>> 0);
      assert.equal(res.maxAllowedBytes, 65_536);
    }

    // 3. Живой тест на реальном Unix domain socket (discord-ipc-0):
    // - права 0o600 на сокете
    // - легитимный клиент (совпадающий UID) проходит Handshake + фрейм 64 KB
    // - клиент с чужим UID немедленно отключается сервером
    // - клиент, отправивший заголовок фрейма > 64 KB (65537 байт), немедленно отключается сервером
    const tmp = mkdtempSync(join(tmpdir(), "ipc-live-980-"));
    try {
      chmodSync(tmp, 0o700);
      const liveAudit = await runLiveIpcSecurityAuditHarness({ runtimeDir: tmp });
      assert.equal(liveAudit.passed, true, `Сбой живого аудита IPC: ${liveAudit.reasons.join("; ")}`);
      assert.equal(liveAudit.socketModeOctal, "0600");
      assert.equal(liveAudit.authorizedClientHandshakeOk, true);
      assert.equal(liveAudit.exact64KbFrameAccepted, true);
      assert.equal(liveAudit.unauthorizedUidRejected, true);
      assert.equal(liveAudit.oversizedFrameDisconnected, true);
      assert.equal(liveAudit.oversizedFrameTestedBytes, 65_537);
      assert.equal(liveAudit.staleSocketRecovered, true);
      assert.equal(liveAudit.allTenSlotsContentionVerified, true);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-980-5: покрывает все 9 измерений TEST-POLICY.md, маскирование секретов, CLI scripts/ipc-security-audit.mjs (коды 0/1/2), лимиты размера кода и мутационный гейт 5/5", async () => {
    const {
      evaluateIpcSecurityNineDimensions,
      redactIpcAuditLog,
      generateIpcSecurityAuditMutations,
      runIpcSecurityMutationEvaluation,
    } = await import("../../scripts/ipc-security-audit.mjs");

    // 1. Проверка всех 9 измерений TEST-POLICY.md
    const dimReport = await evaluateIpcSecurityNineDimensions();
    assert.equal(dimReport.passed, true, `Нарушение 9 измерений: ${dimReport.reasons.join("; ")}`);
    assert.equal(dimReport.boundaries.minSnowflake, "0");
    assert.equal(dimReport.boundaries.maxSnowflake, "18446744073709551615");
    assert.equal(dimReport.boundaries.maxMessageChars, 2_000);
    assert.equal(dimReport.boundaries.maxAttachmentsCount, 10);
    assert.equal(dimReport.boundaries.maxAttachmentSizeBytes, 25 * 1024 * 1024);
    assert.equal(dimReport.boundaries.maxSearchOffset, 9_975);
    assert.deepEqual(dimReport.boundaries.voiceVolumeBoundsPercent, [0, 200]);
    assert.equal(dimReport.boundaries.unreadBadgeCap, "99+");
    assert.equal(dimReport.boundaries.ipcHardFrameLimitBytes, 65_536);
    assert.equal(dimReport.emptyState.zeroActiveSocketsClean, true);
    assert.equal(dimReport.loadingState.prematureCommandBeforeHandshakeRejected, true);
    assert.equal(dimReport.permissions.uidMismatchBlocked, true);
    assert.equal(dimReport.permissions.mode0600Enforced, true);
    assert.equal(dimReport.permissions.ageGatedUnverifiedBlocked, true);
    assert.deepEqual(dimReport.errorHandling.httpStatusCodesCovered, [401, 403, 404, 413, 429, 503]);
    assert.equal(dimReport.errorHandling.rpcErrorCode4000Verified, true);
    assert.equal(dimReport.recovery.staleSocketCleanupVerified, true);

    // 2. Маскирование чувствительных токенов в логах IPC-аудита
    const rawLog =
      '[RPCServer:IPC] token="mfa.abCDef1234567890_secretToken" auth="Bot MTIzNDU2Nzg5MDEyMzQ1Njc4.GhIjKl.MnOpQrStUvWxYz123456" bearer="Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"';
    const redacted = redactIpcAuditLog(rawLog);
    assert.ok(!redacted.includes("abCDef1234567890_secretToken"));
    assert.ok(!redacted.includes("MnOpQrStUvWxYz123456"));
    assert.ok(!redacted.includes("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    assert.match(redacted, /mfa\.\*\*\*/);
    assert.match(redacted, /Bot \*\*\*/);
    assert.match(redacted, /Bearer \*\*\*/);

    // 3. Проверка CLI scripts/ipc-security-audit.mjs (коды выхода 0, 1, 2 и JSON-отчёт)
    const tmp = mkdtempSync(join(tmpdir(), "ipc-cli-980-"));
    try {
      const outJson = join(tmp, "ipc-security-audit-report.json");
      const okRun = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--out-json", outJson],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(okRun.status, 0, `CLI завершился с ошибкой: ${okRun.stderr}`);
      assert.ok(existsSync(outJson), "JSON-отчёт аудита безопасности IPC должен быть создан");

      const report = JSON.parse(readFileSync(outJson, "utf8"));
      assert.equal(report.taskId, "TASK-980");
      assert.equal(report.status, "passed");
      assert.equal(report.oracleParity.passed, true);
      assert.equal(report.liveAudit.passed, true);
      assert.equal(report.nineDimensions.passed, true);

      // Код выхода 1 при неизвестном аргументе или невалидном слоте
      const badFlag = spawnSync(process.execPath, [SCRIPT_PATH, "--unknown-flag"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badFlag.status, 1);

      const badSlot = spawnSync(process.execPath, [SCRIPT_PATH, "--slot", "10"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badSlot.status, 1);

      // Код выхода 2 при отсутствии явно указанного файла оракула
      const missingOracle = spawnSync(
        process.execPath,
        [SCRIPT_PATH, "--oracle", join(tmp, "nonexistent-kanban.json")],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(missingOracle.status, 2);

      // Код выхода 1 при нарушении контракта в переданном файле оракула
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

    // 4. Проверка соблюдения лимитов размера кода (< 800 строк на файл, < 50 строк на функцию)
    const scriptSource = readFileSync(SCRIPT_PATH, "utf8");
    const sizeCheck = checkRustSourceSizeLimits("scripts/ipc-security-audit.mjs", scriptSource);
    assert.equal(sizeCheck.passed, true, sizeCheck.reason);
    assert.ok(
      sizeCheck.fileLines < 800,
      `scripts/ipc-security-audit.mjs должен быть < 800 строк (сейчас ${sizeCheck.fileLines})`,
    );

    // 5. Мутационный гейт 5/5 (100% kill rate)
    const mutants = generateIpcSecurityAuditMutations();
    assert.equal(mutants.length, 5);
    const mutationGate = evaluateMutationGate(mutants, (mutant) =>
      runIpcSecurityMutationEvaluation(mutant),
    );
    assert.equal(
      mutationGate.passed,
      true,
      `Выжившие мутанты: ${mutationGate.survivors.join(", ")}`,
    );
    assert.equal(mutationGate.killed, 5);
    assert.equal(mutationGate.survived, 0);
    assert.equal(mutationGate.killRate, 1);
  });
});

/**
 * Создаёт каталог и принудительно выставляет права через chmodSync (в обход process.umask).
 * @param {string} dirPath
 * @param {number} mode
 */
function mkdirSyncWithMode(dirPath, mode) {
  const { mkdirSync } = requireOrFs();
  mkdirSync(dirPath, { recursive: true });
  chmodSync(dirPath, mode);
}

import * as fsModule from "node:fs";
function requireOrFs() {
  return fsModule;
}
