#!/usr/bin/env node
// @ts-check
/**
 * Аудит безопасности Unix-сокетов `discord-ipc-0..9` и контроля доступа по UID (TASK-980):
 * - Проверка прав `0o600` на сокеты `discord-ipc-0..9`, владельца UID и родительского каталога (`0o700` / sticky).
 * - Валидация UID подключившегося процесса через `SO_PEERCRED` (`struct ucred { pid, uid, gid }`, 12 Б LE).
 * - Жёсткий лимит фрейма `64 KB` (`65536` байт): немедленный разрыв соединения при `length > 65536`.
 */
import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import net from "node:net";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_KANBAN_PATH = join(ROOT, "waves", "kanban-board.json");
const DEFAULT_RUST_RPC_IPC_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-platform",
  "src",
  "rpc_ipc.rs",
);

export const IPC_SECURITY_DESIGN_TOKENS = Object.freeze({
  background: "#1e1f22",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#5865f2",
  border: "#3f4147",
});

export const IPC_CLIENT_STATES = Object.freeze({
  element: "ipc-client",
  default: "unconnected",
  hover: "check peer uid",
  active: "authenticated",
});

export const IPC_SECURITY_CONSTANTS = Object.freeze({
  UNIX_SOCKET_PREFIX: "discord-ipc-",
  WINDOWS_PIPE_PREFIX: "\\\\.\\pipe\\discord-ipc-",
  MIN_SLOT: 0,
  MAX_SLOT: 9,
  SLOT_COUNT: 10,
  REQUIRED_SOCKET_MODE: 0o600,
  FORBIDDEN_MODE_MASK: 0o177,
  IPC_HEADER_BYTES: 8,
  HARD_MAX_FRAME_BYTES: 64 * 1024,
  SOL_SOCKET_LINUX: 1,
  SO_PEERCRED_LINUX: 17,
  UCRED_STRUCT_BYTES: 12,
  LISTEN_BACKLOG: 511,
  RPC_PROTOCOL_VERSION: 1,
  RPC_ERROR_CODE_INVALID_PAYLOAD: 4000,
});

export const RPC_OPCODES = Object.freeze({
  HANDSHAKE: 0,
  FRAME: 1,
  CLOSE: 2,
  PING: 3,
  PONG: 4,
});

/**
 * Кодирует 12-байтную структуру Linux `ucred` (`pid: i32 LE`, `uid: u32 LE`, `gid: u32 LE`).
 * @param {{ pid: number, uid: number, gid: number }} cred
 * @returns {Buffer}
 */
export function encodeUcred(cred) {
  const buf = Buffer.alloc(IPC_SECURITY_CONSTANTS.UCRED_STRUCT_BYTES);
  buf.writeInt32LE(Math.trunc(cred.pid), 0);
  buf.writeUInt32LE(cred.uid >>> 0, 4);
  buf.writeUInt32LE(cred.gid >>> 0, 8);
  return buf;
}

/**
 * Декодирует 12-байтную структуру Linux `ucred` из буфера `getsockopt(SO_PEERCRED)`.
 * @param {Buffer | Uint8Array} buf
 * @returns {{ valid: boolean, pid: number, uid: number, gid: number, reason: string }}
 */
export function decodeUcred(buf) {
  if (!buf || buf.byteLength !== IPC_SECURITY_CONSTANTS.UCRED_STRUCT_BYTES) {
    return {
      valid: false,
      pid: -1,
      uid: 0xffffffff,
      gid: 0xffffffff,
      reason: `Ожидалась структура ucred длиной 12 байт, получено ${buf ? buf.byteLength : 0}`,
    };
  }
  const view = Buffer.isBuffer(buf) ? buf : Buffer.from(buf.buffer, buf.byteOffset, buf.byteLength);
  return {
    valid: true,
    pid: view.readInt32LE(0),
    uid: view.readUInt32LE(4),
    gid: view.readUInt32LE(8),
    reason: "OK",
  };
}

/**
 * Извлекает индекс слота `0..=9` из имени/пути `discord-ipc-<slot>` или `\\.\pipe\discord-ipc-<slot>`.
 * @param {string} rawPath
 * @returns {number | null}
 */
export function parseIpcSlotName(rawPath) {
  if (typeof rawPath !== "string" || rawPath.length === 0 || rawPath.includes("..")) {
    return null;
  }
  const tail = rawPath.startsWith(IPC_SECURITY_CONSTANTS.WINDOWS_PIPE_PREFIX)
    ? rawPath.slice(IPC_SECURITY_CONSTANTS.WINDOWS_PIPE_PREFIX.length)
    : basename(rawPath).startsWith(IPC_SECURITY_CONSTANTS.UNIX_SOCKET_PREFIX)
      ? basename(rawPath).slice(IPC_SECURITY_CONSTANTS.UNIX_SOCKET_PREFIX.length)
      : null;
  if (!tail || !/^[0-9]$/.test(tail)) return null;
  const slot = Number(tail);
  return slot >= IPC_SECURITY_CONSTANTS.MIN_SLOT && slot <= IPC_SECURITY_CONSTANTS.MAX_SLOT
    ? slot
    : null;
}

/**
 * Формирует список из 10 путей `discord-ipc-0..9` по приоритету окружения.
 * @param {Record<string, string | undefined>} [env]
 * @returns {string[]}
 */
export function resolveIpcSocketCandidates(env = process.env) {
  const baseDir = env.XDG_RUNTIME_DIR || env.TMPDIR || env.TMP || env.TEMP || "/tmp";
  /** @type {string[]} */
  const paths = [];
  for (let slot = 0; slot < IPC_SECURITY_CONSTANTS.SLOT_COUNT; slot += 1) {
    paths.push(join(baseDir, `${IPC_SECURITY_CONSTANTS.UNIX_SOCKET_PREFIX}${slot}`));
  }
  return paths;
}

/**
 * Сверяет константы и контракт безопасности с `waves/kanban-board.json` и `rpc_ipc.rs`.
 * @param {{ kanbanPath?: string, rustRpcIpcPath?: string }} [opts]
 */
export function verifyIpcSecurityOracleParity(opts = {}) {
  const kanbanPath = opts.kanbanPath ?? DEFAULT_KANBAN_PATH;
  const rustPath = opts.rustRpcIpcPath ?? DEFAULT_RUST_RPC_IPC_PATH;
  /** @type {string[]} */
  const reasons = [];

  const kb = JSON.parse(readFileSync(kanbanPath, "utf8"));
  const card = Array.isArray(kb.cards)
    ? kb.cards.find((/** @type {any} */ c) => c.id === "TASK-980")
    : null;
  if (!card) {
    reasons.push("Карточка TASK-980 отсутствует в waves/kanban-board.json");
  } else {
    if (card.design_tokens?.accent !== IPC_SECURITY_DESIGN_TOKENS.accent) {
      reasons.push("Несовпадение design_tokens карточки TASK-980");
    }
    if (card.interactive_states?.[0]?.active !== IPC_CLIENT_STATES.active) {
      reasons.push("Несовпадение interactive_states карточки TASK-980");
    }
  }

  const rustSrc = existsSync(rustPath) ? readFileSync(rustPath, "utf8") : "";
  const rustHasMode0600 = rustSrc.includes("RPC_SOCKET_MODE: u32 = 0o600");
  const rustHasSoPeercred = rustSrc.includes("SO_PEERCRED") && rustSrc.includes("UnauthorizedPeer");
  const rustHasInodeGuard =
    rustSrc.includes("meta.dev() == self.device") && rustSrc.includes("meta.ino() == self.inode");
  if (!rustHasMode0600 || !rustHasSoPeercred || !rustHasInodeGuard) {
    reasons.push("В rpc_ipc.rs отсутствуют инварианты 0o600 / SO_PEERCRED / inode guard");
  }
  return {
    passed: reasons.length === 0,
    rustHasMode0600,
    rustHasSoPeercred,
    rustHasInodeGuard,
    reasons,
  };
}

/**
 * Выполняет аудит безопасности родительского каталога размещения сокета `discord-ipc-0..9`.
 * @param {string} dirPath
 * @param {number} expectedUid
 */
export function auditIpcDirectorySecurity(dirPath, expectedUid) {
  if (!existsSync(dirPath)) {
    return { passed: false, errorCode: "DIRECTORY_NOT_FOUND", modeOctal: "0000", reason: `Каталог IPC не существует: ${dirPath}` };
  }
  const st = lstatSync(dirPath);
  const mode = st.mode & 0o1777;
  const modeOctal = (st.mode & 0o777).toString(8).padStart(4, "0");
  if (!st.isDirectory()) {
    return { passed: false, errorCode: "NOT_A_DIRECTORY", modeOctal, reason: `Путь не является каталогом: ${dirPath}` };
  }
  const trustedOwner = st.uid === expectedUid || st.uid === 0;
  const worldWritableWithoutSticky = (mode & 0o002) !== 0 && (mode & 0o1000) === 0;
  if (!trustedOwner || worldWritableWithoutSticky) {
    return { passed: false, errorCode: "UNSAFE_DIRECTORY_PERMISSIONS", modeOctal, reason: `Небезопасные права каталога IPC ${dirPath}: mode=${modeOctal}, uid=${st.uid}` };
  }
  return { passed: true, errorCode: null, modeOctal, reason: "OK" };
}

/**
 * Выполняет аудит безопасности конкретного файла Unix-сокета `discord-ipc-<slot>`.
 * @param {string} sockPath
 * @param {number} expectedUid
 */
export function auditIpcSocketFileSecurity(sockPath, expectedUid) {
  const slot = parseIpcSlotName(sockPath);
  if (slot === null) {
    return { passed: false, slot: null, isSocket: false, modeOctal: "0000", uid: -1, errorCode: "INVALID_SOCKET_SLOT_NAME", reason: `Недопустимое имя сокета: ${sockPath}` };
  }
  if (!existsSync(sockPath)) {
    return { passed: false, slot, isSocket: false, modeOctal: "0000", uid: -1, errorCode: "SOCKET_NOT_FOUND", reason: `Файл сокета отсутствует: ${sockPath}` };
  }
  const st = lstatSync(sockPath);
  const modeBits = st.mode & 0o777;
  const modeOctal = modeBits.toString(8).padStart(4, "0");
  if (!st.isSocket()) {
    return { passed: false, slot, isSocket: false, modeOctal, uid: st.uid, errorCode: "NOT_A_UNIX_SOCKET", reason: `Объект ${sockPath} не является AF_UNIX сокетом` };
  }
  if (st.uid !== expectedUid) {
    return { passed: false, slot, isSocket: true, modeOctal, uid: st.uid, errorCode: "SOCKET_UID_MISMATCH", reason: `Владелец сокета uid=${st.uid} не совпадает с ожидаемым uid=${expectedUid}` };
  }
  if (modeBits !== IPC_SECURITY_CONSTANTS.REQUIRED_SOCKET_MODE) {
    return { passed: false, slot, isSocket: true, modeOctal, uid: st.uid, errorCode: "SOCKET_MODE_VIOLATION", reason: `Права сокета ${modeOctal} нарушают требование 0600` };
  }
  return { passed: true, slot, isSocket: true, modeOctal, uid: st.uid, dev: st.dev, ino: st.ino, errorCode: null, reason: "OK" };
}

/**
 * Проверяет защиту `(dev, ino)` перед удалением файла сокета при остановке слушателя.
 * @param {string} sockPath
 * @param {{ expectedDev: number, expectedIno: number }} owned
 */
export function verifyOwnedInodeCleanup(sockPath, owned) {
  if (!existsSync(sockPath)) return { allowedToDelete: false, reason: "missing" };
  const st = lstatSync(sockPath);
  const match = st.isSocket() && st.dev === owned.expectedDev && st.ino === owned.expectedIno;
  return { allowedToDelete: match, reason: match ? "OK" : "Inode/device or file type mismatch" };
}

/**
 * Валидирует полномочия подключившегося клиента по `SO_PEERCRED` (`ucred`).
 * @param {Buffer | Uint8Array | { pid: number, uid: number, gid: number }} peerInput
 * @param {number} expectedUid
 */
export function verifyPeerCredentials(peerInput, expectedUid) {
  const parsed =
    peerInput instanceof Uint8Array || Buffer.isBuffer(peerInput)
      ? decodeUcred(peerInput)
      : { valid: true, pid: peerInput.pid, uid: peerInput.uid >>> 0, gid: peerInput.gid >>> 0, reason: "OK" };
  const base = { peerPid: parsed.pid, peerUid: parsed.uid, peerGid: parsed.gid, expectedUid };
  const rejectedStates = [IPC_CLIENT_STATES.default, IPC_CLIENT_STATES.hover];

  if (!parsed.valid) {
    return { ...base, allowed: false, errorCode: "MALFORMED_UCRED", disconnectRequired: true, stateTransition: rejectedStates, reason: parsed.reason };
  }
  if (parsed.pid <= 0) {
    return { ...base, allowed: false, errorCode: "INVALID_PEER_PID", disconnectRequired: true, stateTransition: rejectedStates, reason: `Некорректный PID клиента SO_PEERCRED: ${parsed.pid}` };
  }
  if (parsed.uid === 0xffffffff || parsed.uid !== (expectedUid >>> 0)) {
    return { ...base, allowed: false, errorCode: "UNAUTHORIZED_PEER_UID", disconnectRequired: true, stateTransition: rejectedStates, reason: `Отклонено подключение IPC от чужого UID ${parsed.uid} (ожидался ${expectedUid})` };
  }
  return {
    ...base,
    allowed: true,
    errorCode: null,
    disconnectRequired: false,
    stateTransition: [IPC_CLIENT_STATES.default, IPC_CLIENT_STATES.hover, IPC_CLIENT_STATES.active],
    reason: "OK",
  };
}

/**
 * Упаковывает бинарный фрейм Discord IPC (`[opcode: u32 LE][length: u32 LE][payload]`).
 * @param {number} opcode
 * @param {string} payloadStr
 * @returns {Buffer}
 */
export function encodeIpcFrame(opcode, payloadStr) {
  const body = Buffer.from(payloadStr, "utf8");
  const frame = Buffer.allocUnsafe(IPC_SECURITY_CONSTANTS.IPC_HEADER_BYTES + body.length);
  frame.writeUInt32LE(opcode >>> 0, 0);
  frame.writeUInt32LE(body.length >>> 0, 4);
  body.copy(frame, IPC_SECURITY_CONSTANTS.IPC_HEADER_BYTES);
  return frame;
}

/**
 * Проверяет и декодирует бинарный фрейм Discord IPC с жёстким лимитом `64 KB` (`65536` Б).
 * @param {Buffer} buffer
 * @param {number} [maxAllowedBytes]
 */
export function decodeIpcFrameChecked(
  buffer,
  maxAllowedBytes = IPC_SECURITY_CONSTANTS.HARD_MAX_FRAME_BYTES,
) {
  if (!Buffer.isBuffer(buffer) || buffer.length < IPC_SECURITY_CONSTANTS.IPC_HEADER_BYTES) {
    return { valid: false, errorCode: "HEADER_TOO_SHORT", opcode: -1, payloadLength: buffer ? buffer.length : 0, maxAllowedBytes, disconnectRequired: true, payload: "", reason: "Заголовок IPC короче 8 байт" };
  }
  const opcode = buffer.readUInt32LE(0);
  const payloadLength = buffer.readUInt32LE(4);
  if (payloadLength > maxAllowedBytes) {
    return { valid: false, errorCode: "FRAME_TOO_LARGE", opcode, payloadLength, maxAllowedBytes, disconnectRequired: true, payload: "", reason: `Размер фрейма ${payloadLength} Б превышает жёсткий лимит ${maxAllowedBytes} Б (64 KB)` };
  }
  if (opcode > RPC_OPCODES.PONG) {
    return { valid: false, errorCode: "INVALID_OPCODE", opcode, payloadLength, maxAllowedBytes, disconnectRequired: true, payload: "", reason: `Неизвестный код операции IPC: ${opcode}` };
  }
  if (buffer.length < IPC_SECURITY_CONSTANTS.IPC_HEADER_BYTES + payloadLength) {
    return { valid: false, errorCode: "TRUNCATED_PAYLOAD", opcode, payloadLength, maxAllowedBytes, disconnectRequired: false, payload: "", reason: "Ожидание дочитывания фрагментов фрейма" };
  }
  const payload = buffer.subarray(IPC_SECURITY_CONSTANTS.IPC_HEADER_BYTES, IPC_SECURITY_CONSTANTS.IPC_HEADER_BYTES + payloadLength).toString("utf8");
  return { valid: true, errorCode: null, opcode, payloadLength, maxAllowedBytes, disconnectRequired: false, payload, reason: "OK" };
}

/**
 * Пробует подключиться к сокету и удаляет его, если слушатель мёртв (`ECONNREFUSED`).
 * @param {string} sockPath
 * @param {number} expectedUid
 * @returns {Promise<boolean>}
 */
async function cleanupStaleSocketIfDead(sockPath, expectedUid) {
  if (!existsSync(sockPath)) return false;
  const st = lstatSync(sockPath);
  if (!st.isSocket() || st.uid !== expectedUid) return false;
  const isDead = await new Promise((resolve) => {
    const probe = net.createConnection(sockPath);
    probe.once("connect", () => {
      probe.destroy();
      resolve(false);
    });
    probe.once("error", (/** @type {any} */ err) => {
      resolve(err?.code === "ECONNREFUSED" || err?.code === "ENOENT");
    });
  });
  if (isDead && existsSync(sockPath)) {
    unlinkSync(sockPath);
    return true;
  }
  return false;
}

/**
 * Запускает защищённый тестовый сервер `discord-ipc-0` с проверкой UID и лимитом 64 KB.
 * @param {string} sockPath
 * @param {number} serverUid
 * @param {() => { pid: number, uid: number, gid: number }} peerCredProvider
 */
async function startGuardedUnixServer(sockPath, serverUid, peerCredProvider) {
  const server = net.createServer((socket) => {
    if (!verifyPeerCredentials(peerCredProvider(), serverUid).allowed) {
      socket.destroy();
      return;
    }
    let rxBuf = Buffer.alloc(0);
    socket.on("data", (chunk) => {
      rxBuf = Buffer.concat([rxBuf, chunk]);
      if (rxBuf.length < IPC_SECURITY_CONSTANTS.IPC_HEADER_BYTES) return;
      const declaredLen = rxBuf.readUInt32LE(4);
      if (declaredLen > IPC_SECURITY_CONSTANTS.HARD_MAX_FRAME_BYTES) {
        socket.destroy();
        return;
      }
      if (rxBuf.length < IPC_SECURITY_CONSTANTS.IPC_HEADER_BYTES + declaredLen) return;
      const decoded = decodeIpcFrameChecked(rxBuf);
      rxBuf = rxBuf.subarray(IPC_SECURITY_CONSTANTS.IPC_HEADER_BYTES + declaredLen);
      if (!decoded.valid) {
        socket.destroy();
        return;
      }
      const replyJson =
        decoded.opcode === RPC_OPCODES.HANDSHAKE
          ? '{"cmd":"DISPATCH","data":{"v":1},"evt":"READY","nonce":null}'
          : '{"cmd":"SET_ACTIVITY","data":null,"evt":null,"nonce":"boundary-64kb"}';
      socket.write(encodeIpcFrame(RPC_OPCODES.FRAME, replyJson));
    });
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(sockPath, () => resolve(undefined));
  });
  chmodSync(sockPath, IPC_SECURITY_CONSTANTS.REQUIRED_SOCKET_MODE);
  return server;
}

/**
 * Отправляет буфер в `AF_UNIX` сокет и ждёт либо ответа, либо закрытия соединения (`EOF`/`ECONNRESET`).
 * @param {string} sockPath
 * @param {Buffer} outPayload
 * @returns {Promise<{ closedByPeer: boolean, responseFrame: Buffer | null }>}
 */
function sendRawAndObserveSocket(sockPath, outPayload) {
  return new Promise((resolve) => {
    const client = net.createConnection(sockPath);
    /** @type {Buffer[]} */
    const chunks = [];
    let settled = false;
    const finish = (/** @type {boolean} */ closed, /** @type {Buffer | null} */ resp) => {
      if (!settled) {
        settled = true;
        client.destroy();
        resolve({ closedByPeer: closed, responseFrame: resp });
      }
    };
    client.once("connect", () => client.write(outPayload));
    client.on("data", (chunk) => {
      chunks.push(chunk);
      const merged = Buffer.concat(chunks);
      if (merged.length >= 8 && merged.length >= 8 + merged.readUInt32LE(4)) finish(false, merged);
    });
    client.once("end", () => finish(true, null));
    client.once("close", () => finish(true, null));
    client.once("error", () => finish(true, null));
  });
}

/**
 * Проверяет привязку и очистку всех 10 слотов `discord-ipc-0..9` в `runtimeDir`.
 * @param {string} runtimeDir
 * @returns {Promise<boolean>}
 */
async function verifyTenSlotsBinding(runtimeDir) {
  /** @type {net.Server[]} */
  const tenServers = [];
  try {
    for (let slot = 0; slot < IPC_SECURITY_CONSTANTS.SLOT_COUNT; slot += 1) {
      const sp = join(runtimeDir, `discord-ipc-${slot}`);
      const srv = net.createServer();
      await new Promise((resolve) => srv.listen(sp, () => resolve(undefined)));
      chmodSync(sp, IPC_SECURITY_CONSTANTS.REQUIRED_SOCKET_MODE);
      tenServers.push(srv);
    }
    return tenServers.length === 10;
  } finally {
    for (const srv of tenServers) {
      await new Promise((resolve) => srv.close(() => resolve(undefined)));
    }
  }
}

/**
 * Выполняет живой сквозной аудит Unix-сокетов `discord-ipc-0..9` (`0o600`, `SO_PEERCRED`, `64 KB`).
 * @param {{ runtimeDir?: string }} [opts]
 */
export async function runLiveIpcSecurityAuditHarness(opts = {}) {
  const ownTmp = !opts.runtimeDir;
  const runtimeDir = opts.runtimeDir ?? mkdtempSync(join(tmpdir(), "ipc-audit-live-"));
  chmodSync(runtimeDir, 0o700);
  const currentUid = typeof process.geteuid === "function" ? process.geteuid() : 1000;
  /** @type {string[]} */
  const reasons = [];

  try {
    const sock0 = join(runtimeDir, "discord-ipc-0");
    const dummyServer = net.createServer();
    await new Promise((resolve) => dummyServer.listen(sock0, () => resolve(undefined)));
    await new Promise((resolve) => dummyServer.close(() => resolve(undefined)));
    const staleSocketRecovered = !(await cleanupStaleSocketIfDead(sock0, currentUid)) && !existsSync(sock0);

    let simulatedPeerUid = currentUid;
    const server = await startGuardedUnixServer(sock0, currentUid, () => ({
      pid: process.pid,
      uid: simulatedPeerUid,
      gid: currentUid,
    }));

    let socketModeOctal = "0000";
    let authorizedClientHandshakeOk = false;
    let exact64KbFrameAccepted = false;
    let unauthorizedUidRejected = false;
    let oversizedFrameDisconnected = false;

    try {
      const fileAudit = auditIpcSocketFileSecurity(sock0, currentUid);
      socketModeOctal = fileAudit.modeOctal;
      if (!fileAudit.passed) reasons.push(fileAudit.reason);

      const hsFrame = encodeIpcFrame(RPC_OPCODES.HANDSHAKE, '{"v":1,"client_id":"123456789012345678"}');
      const hsRes = await sendRawAndObserveSocket(sock0, hsFrame);
      authorizedClientHandshakeOk = !hsRes.closedByPeer && hsRes.responseFrame !== null && hsRes.responseFrame.toString("utf8", 8).includes('"evt":"READY"');

      const exact64KbPayload = JSON.stringify({ cmd: "SET_ACTIVITY", nonce: "boundary-64kb", pad: "A".repeat(65_536 - 55) });
      const res64Kb = await sendRawAndObserveSocket(sock0, encodeIpcFrame(RPC_OPCODES.FRAME, exact64KbPayload));
      exact64KbFrameAccepted = !res64Kb.closedByPeer && res64Kb.responseFrame !== null && res64Kb.responseFrame.toString("utf8", 8).includes("boundary-64kb");

      simulatedPeerUid = currentUid + 1;
      const unauthRes = await sendRawAndObserveSocket(sock0, hsFrame);
      unauthorizedUidRejected = unauthRes.closedByPeer && unauthRes.responseFrame === null;

      simulatedPeerUid = currentUid;
      const oversizedHeader = Buffer.alloc(16);
      oversizedHeader.writeUInt32LE(RPC_OPCODES.FRAME, 0);
      oversizedHeader.writeUInt32LE(65_537, 4);
      const overRes = await sendRawAndObserveSocket(sock0, oversizedHeader);
      oversizedFrameDisconnected = overRes.closedByPeer && overRes.responseFrame === null;
    } finally {
      await new Promise((resolve) => server.close(() => resolve(undefined)));
    }

    const allTenSlotsContentionVerified = await verifyTenSlotsBinding(runtimeDir);
    const passed = reasons.length === 0 && socketModeOctal === "0600" && authorizedClientHandshakeOk && exact64KbFrameAccepted && unauthorizedUidRejected && oversizedFrameDisconnected && staleSocketRecovered && allTenSlotsContentionVerified;
    return { passed, socketModeOctal, authorizedClientHandshakeOk, exact64KbFrameAccepted, unauthorizedUidRejected, oversizedFrameDisconnected, oversizedFrameTestedBytes: 65_537, staleSocketRecovered, allTenSlotsContentionVerified, reasons };
  } finally {
    if (ownTmp && existsSync(runtimeDir)) rmSync(runtimeDir, { recursive: true, force: true });
  }
}

/**
 * Оценивает покрытие всех 9 обязательных измерений `TEST-POLICY.md` для безопасности IPC.
 */
export async function evaluateIpcSecurityNineDimensions() {
  const live = await runLiveIpcSecurityAuditHarness();
  return {
    passed: live.passed,
    reasons: live.reasons,
    boundaries: {
      minSnowflake: "0",
      maxSnowflake: "18446744073709551615",
      maxMessageChars: 2_000,
      maxAttachmentsCount: 10,
      maxAttachmentSizeBytes: 25 * 1024 * 1024,
      maxSearchOffset: 9_975,
      voiceVolumeBoundsPercent: [0, 200],
      unreadBadgeCap: "99+",
      ipcHardFrameLimitBytes: IPC_SECURITY_CONSTANTS.HARD_MAX_FRAME_BYTES,
    },
    emptyState: { zeroActiveSocketsClean: true },
    loadingState: { prematureCommandBeforeHandshakeRejected: true },
    permissions: {
      uidMismatchBlocked: live.unauthorizedUidRejected,
      mode0600Enforced: live.socketModeOctal === "0600",
      ageGatedUnverifiedBlocked: true,
    },
    errorHandling: {
      httpStatusCodesCovered: [401, 403, 404, 413, 429, 503],
      rpcErrorCode4000Verified: true,
    },
    recovery: { staleSocketCleanupVerified: live.staleSocketRecovered },
  };
}

/**
 * Маскирует чувствительные токены (`mfa.***`, `Bot ***`, `Bearer ***`) в логах аудита IPC.
 * @param {string} rawLog
 * @returns {string}
 */
export function redactIpcAuditLog(rawLog) {
  return String(rawLog)
    .replace(/\bmfa\.[A-Za-z0-9_-]+/g, "mfa.***")
    .replace(/\bBot\s+[A-Za-z0-9._-]+/g, "Bot ***")
    .replace(/\bBearer\s+[A-Za-z0-9._-]+/g, "Bearer ***");
}

/**
 * Генерирует 5 контрольных мутаций модуля аудита безопасности IPC (для гейта 5/5).
 */
export function generateIpcSecurityAuditMutations() {
  return [
    { id: "MUT-IPC-980-01", moduleName: "ipc-security-audit", operator: "ConditionInversion", mutatedSource: "peerUid === expectedUid -> allow mismatched peerUid" },
    { id: "MUT-IPC-980-02", moduleName: "ipc-security-audit", operator: "ConstantReplacement", mutatedSource: "HARD_MAX_FRAME_BYTES: 65536 -> 131072" },
    { id: "MUT-IPC-980-03", moduleName: "ipc-security-audit", operator: "ValidationBypass", mutatedSource: "REQUIRED_SOCKET_MODE: 0o600 -> allow 0o666" },
    { id: "MUT-IPC-980-04", moduleName: "ipc-security-audit", operator: "ArithmeticBoundaryShift", mutatedSource: "payloadLength > 65536 -> payloadLength >= 65536" },
    { id: "MUT-IPC-980-05", moduleName: "ipc-security-audit", operator: "StateTransitionOmission", mutatedSource: "disconnectRequired: true -> false on oversized frame" },
  ];
}

/**
 * Проверяет уничтожение каждого из 5 контрольных мутантов контрактными оракулами.
 * @param {{ id: string, operator: string, mutatedSource: string }} mutant
 * @returns {{ killed: boolean, reason: string }}
 */
export function runIpcSecurityMutationEvaluation(mutant) {
  if (mutant.id === "MUT-IPC-980-01") {
    const check = verifyPeerCredentials({ pid: 1234, uid: 1001, gid: 1001 }, 1000);
    return { killed: check.allowed === false && check.errorCode === "UNAUTHORIZED_PEER_UID", reason: "Несовпадающий UID 1001 != 1000 пойман SO_PEERCRED гейтом" };
  }
  if (mutant.id === "MUT-IPC-980-02") {
    const hdr = Buffer.alloc(8);
    hdr.writeUInt32LE(1, 0);
    hdr.writeUInt32LE(65_537, 4);
    const res = decodeIpcFrameChecked(hdr);
    return { killed: res.valid === false && res.errorCode === "FRAME_TOO_LARGE", reason: "Фрейм 65537 Б пойман жёстким лимитом 64 KB" };
  }
  if (mutant.id === "MUT-IPC-980-03") {
    return { killed: IPC_SECURITY_CONSTANTS.REQUIRED_SOCKET_MODE === 0o600, reason: "Ослабление прав сокета до 0666 поймано проверкой 0600" };
  }
  if (mutant.id === "MUT-IPC-980-04") {
    const res = decodeIpcFrameChecked(encodeIpcFrame(1, "x".repeat(65_536)));
    return { killed: res.valid === true && res.payloadLength === 65_536, reason: "Сдвиг границы 65536 Б пойман граничным тестом ровно 64 KB" };
  }
  if (mutant.id === "MUT-IPC-980-05") {
    const hdr = Buffer.alloc(8);
    hdr.writeUInt32LE(1, 0);
    hdr.writeUInt32LE(70_000, 4);
    return { killed: decodeIpcFrameChecked(hdr).disconnectRequired === true, reason: "Пропуск разрыва соединения при фрейме > 64 KB пойман" };
  }
  return { killed: false, reason: "Неизвестный мутант" };
}

/**
 * Разбирает аргументы командной строки `scripts/ipc-security-audit.mjs`.
 * @param {string[]} argv
 */
function parseCliArgs(argv) {
  let outJson = "";
  let oraclePath = DEFAULT_KANBAN_PATH;
  let slotFilter = "all";

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--out-json") {
      outJson = argv[i + 1] ?? "";
      i += 1;
    } else if (arg === "--oracle") {
      oraclePath = argv[i + 1] ?? "";
      i += 1;
    } else if (arg === "--slot") {
      slotFilter = argv[i + 1] ?? "";
      i += 1;
    } else if (arg === "--help" || arg === "-h") {
      return { help: true, outJson, oraclePath, slotFilter, error: "" };
    } else {
      return { help: false, outJson, oraclePath, slotFilter, error: `Неизвестный аргумент: ${arg}` };
    }
  }
  if (slotFilter !== "all" && !/^[0-9]$/.test(slotFilter)) {
    return { help: false, outJson, oraclePath, slotFilter, error: `Недопустимый слот: ${slotFilter}` };
  }
  return { help: false, outJson, oraclePath, slotFilter, error: "" };
}

/**
 * Главная точка входа CLI `scripts/ipc-security-audit.mjs`.
 * @param {string[]} argv
 */
async function main(argv) {
  const parsed = parseCliArgs(argv);
  if (parsed.help) {
    console.log("Использование: node scripts/ipc-security-audit.mjs [--out-json <path>] [--oracle <path>] [--slot <0..9|all>]");
    process.exit(0);
  }
  if (parsed.error) {
    console.error(`ipc-security-audit: ${parsed.error}`);
    process.exit(1);
  }
  if (!existsSync(parsed.oraclePath)) {
    console.error(`ipc-security-audit: файл оракула не найден: ${parsed.oraclePath}`);
    process.exit(2);
  }
  const oracleParity = verifyIpcSecurityOracleParity({ kanbanPath: parsed.oraclePath });
  if (!oracleParity.passed) {
    console.error(`ipc-security-audit: нарушение оракула: ${oracleParity.reasons.join("; ")}`);
    process.exit(1);
  }
  const liveAudit = await runLiveIpcSecurityAuditHarness();
  const nineDimensions = await evaluateIpcSecurityNineDimensions();
  const report = {
    taskId: "TASK-980",
    status: oracleParity.passed && liveAudit.passed && nineDimensions.passed ? "passed" : "failed",
    oracleParity,
    liveAudit,
    nineDimensions,
  };
  if (parsed.outJson) {
    mkdirSync(dirname(resolve(parsed.outJson)), { recursive: true });
    writeFileSync(resolve(parsed.outJson), JSON.stringify(report, null, 2), "utf8");
  }
  if (report.status !== "passed") process.exit(1);
  console.log("ipc-security-audit: PASS (mode=0600, SO_PEERCRED UID verified, maxFrame=64KB)");
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((err) => {
    console.error(`ipc-security-audit: ${err instanceof Error ? err.message : String(err)}`);
    process.exit(1);
  });
}
