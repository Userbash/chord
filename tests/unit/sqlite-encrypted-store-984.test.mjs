// @ts-check
/**
 * Контрактные и модульные тесты шифрования локального хранилища SQLite
 * (AES-256-GCM / SQLCipher page-level encryption & Linux Keyring `org.freedesktop.secrets`) — TASK-984:
 * - SUB-984-0: Статический и структурный контракт `rust/chord-gpui/src/encrypted_store.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, лимиты `scripts/quality-gate.mjs`
 *   < 50 строк на функцию и < 800 строк на файл, чистый `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-984-1: Сверка констант, дизайн-токенов, состояний `keyring-auth` (`locked -> unlocking -> decrypted`),
 *   3 IPC-каналов `DISCORD_SAFE_STORAGE_*` и схемы `com.discordlab.Token` (`org.freedesktop.secrets`,
 *   префикс `dQw4w9WgXcQ:`) с независимыми оракулами (`waves/kanban-board.json`,
 *   `docs/re/backend-ipc.json`, `docs/re/rest-static.md`, `rust/crates/chord-platform/src/keyring.rs`,
 *   `rust/crates/chord-store/src/lib.rs`).
 * - SUB-984-2: Извлечение мастер-ключа из Linux Keyring (`org.freedesktop.secrets`) без утечки в логи,
 *   деривация ключей `page_key` / `wal_key` / `token_key` / `header_mac_key` по RFC 5869 HKDF-SHA256
 *   (побайтовая сверка с независимым оракулом `node:crypto`), шифрование токенов `dQw4w9WgXcQ:`,
 *   маскирование `<скрыто>` в `Debug`/`Display` и физическое затирание памяти `zeroize_slice` (`black_box(0)` + `Drop`).
 * - SUB-984-3: Страничное шифрование реальной базы данных SQLite (`page_size = 4096`) алгоритмом
 *   AES-256-GCM (NIST SP 800-38D): побайтовая сверка шифротекста и 128-битных тегов GCM с оракулом
 *   `node:crypto`, проверка отсутствия открытого заголовка `"SQLite format 3\0"` и текстов сообщений
 *   на диске, проверка высокой энтропии (> 7.8 бит/байт), дешифрование и проверка `PRAGMA integrity_check = ok`
 *   через `node:sqlite` (`DatabaseSync`), защита от подмены/перестановки страниц (AAD `page_no || generation || page_size`).
 * - SUB-984-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные Snowflake `0`/`u64::MAX`, 2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`,
 *   бейдж `99+`, ошибки `401/403/404/413/429/5xx`, пустые, `skeleton`/`HTTP 202`, права `VIEW_CHANNEL` и
 *   `AGE_GATED_SPACES`, восстановление WAL после сбоя питания, маскирование секретов в логах).
 * - SUB-984-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  createCipheriv,
  createDecipheriv,
  createHash,
  createHmac,
} from "node:crypto";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  checkRustSourceSizeLimits,
  evaluateMutationGate,
} from "../../scripts/quality-gate.mjs";
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const MODULE_REL_PATH = "rust/chord-gpui/src/encrypted_store.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const BACKEND_IPC_JSON_PATH = join(ROOT, "docs", "re", "backend-ipc.json");
const KEYRING_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "chord-platform",
  "src",
  "keyring.rs",
);
const STORE_LIB_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "chord-store",
  "src",
  "lib.rs",
);
const RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.md";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/chord-gpui/src/encrypted_store.rs` через `rustc --test --edition 2024 -D warnings`.
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

/**
 * Независимый оракул `HKDF-Extract(salt, ikm)` на `node:crypto` HMAC-SHA256 (RFC 5869).
 * @param {Buffer} salt
 * @param {Buffer} ikm
 * @returns {Buffer}
 */
function oracleHkdfExtract(salt, ikm) {
  const key = salt.length === 0 ? Buffer.alloc(32, 0) : salt;
  return createHmac("sha256", key).update(ikm).digest();
}

/**
 * Независимый оракул `HKDF-Expand(prk, info, length)` на `node:crypto` HMAC-SHA256 (RFC 5869).
 * @param {Buffer} prk
 * @param {Buffer} info
 * @param {number} outLen
 * @returns {Buffer}
 */
function oracleHkdfExpand(prk, info, outLen) {
  /** @type {Buffer[]} */
  const chunks = [];
  let prev = Buffer.alloc(0);
  let total = 0;
  let counter = 1;
  while (total < outLen) {
    prev = createHmac("sha256", prk)
      .update(prev)
      .update(info)
      .update(Buffer.from([counter]))
      .digest();
    chunks.push(prev);
    total += prev.length;
    counter += 1;
  }
  return Buffer.concat(chunks).subarray(0, outLen);
}

/**
 * Независимый оракул формирования 12-байтного nonce и 10-байтного AAD страницы SQLite.
 * @param {Buffer} salt16 16-байтная соль контейнера БД
 * @param {number} pageNo 1-базовый номер страницы SQLite
 * @param {number} generation поколение перезаписи страницы
 * @param {number} pageSize размер страницы (4096)
 * @returns {{ nonce: Buffer, aad: Buffer }}
 */
function oraclePageNonceAndAad(salt16, pageNo, generation, pageSize = 4096) {
  const nonce = Buffer.alloc(12);
  salt16.copy(nonce, 0, 0, 4);
  nonce.writeUInt32BE(pageNo >>> 0, 4);
  nonce.writeUInt32BE(generation >>> 0, 8);

  const aad = Buffer.alloc(10);
  aad.writeUInt32BE(pageNo >>> 0, 0);
  aad.writeUInt32BE(generation >>> 0, 4);
  aad.writeUInt16BE(pageSize & 0xffff, 8);
  return { nonce, aad };
}

/**
 * Независимый оракул страничного шифрования AES-256-GCM на `node:crypto`.
 * @param {Buffer} pageKey32
 * @param {Buffer} salt16
 * @param {number} pageNo
 * @param {number} generation
 * @param {Buffer} plainPage4096
 * @returns {{ nonce: Buffer, ciphertext: Buffer, tag: Buffer }}
 */
function oracleEncryptPageAes256Gcm(
  pageKey32,
  salt16,
  pageNo,
  generation,
  plainPage4096,
) {
  const { nonce, aad } = oraclePageNonceAndAad(
    salt16,
    pageNo,
    generation,
    plainPage4096.length,
  );
  const cipher = createCipheriv("aes-256-gcm", pageKey32, nonce);
  cipher.setAAD(aad);
  const ciphertext = Buffer.concat([
    cipher.update(plainPage4096),
    cipher.final(),
  ]);
  const tag = cipher.getAuthTag();
  return { nonce, ciphertext, tag };
}

/**
 * Вычисляет энтропию Шеннона (0..8 бит/байт) для проверки отсутствия открытых паттернов.
 * @param {Buffer} buf
 * @returns {number}
 */
function computeShannonEntropy(buf) {
  if (buf.length === 0) return 0;
  const counts = new Uint32Array(256);
  for (let i = 0; i < buf.length; i += 1) {
    const b = buf[i] ?? 0;
    counts[b] = (counts[b] ?? 0) + 1;
  }
  let entropy = 0;
  for (let i = 0; i < 256; i += 1) {
    const c = counts[i] ?? 0;
    if (c > 0) {
      const p = c / buf.length;
      entropy -= p * Math.log2(p);
    }
  }
  return entropy;
}

describe("TASK-984: Шифрование локального хранилища SQLite (AES-256-GCM / Linux Keyring)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "sqlite-enc-984-"));
    testBinPath = join(workDir, "encrypted_store_984_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-984-0: модуль rust/chord-gpui/src/encrypted_store.rs существует, запрещает unsafe, проходит rustfmt 2024, лимиты (<50 строк на fn, <800 на файл) и компилируется без предупреждений", () => {
    assert.ok(
      existsSync(MODULE_PATH),
      `Модуль ${MODULE_REL_PATH} должен существовать`,
    );

    const source = readFileSync(MODULE_PATH, "utf8");
    assert.match(
      source,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "Модуль обязан содержать #![forbid(unsafe_code)]",
    );

    const prodSection = source.split("#[cfg(test)]")[0] ?? source;
    assert.doesNotMatch(
      prodSection,
      /\.unwrap\(\)|\.expect\(/,
      "В продуктовом коде encrypted_store.rs запрещены .unwrap() и .expect()",
    );

    const sizeCheck = checkRustSourceSizeLimits(MODULE_REL_PATH, source);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines > 200 && sizeCheck.fileLines < 800,
      `Ожидался размер файла 200..800 строк, получено: ${sizeCheck.fileLines}`,
    );

    const secretFindings = scanTextForSecrets(source, MODULE_REL_PATH);
    assert.equal(
      secretFindings.length,
      0,
      `В исходнике обнаружены незамаскированные секреты: ${JSON.stringify(secretFindings)}`,
    );

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check не прошёл:\n${fmtCheck.stderr || fmtCheck.stdout}`,
    );

    const compiled = compileRustModuleTests(MODULE_PATH, testBinPath);
    assert.equal(
      compiled.status,
      0,
      `Ошибка компиляции rustc --test -D warnings:\n${compiled.stderr}`,
    );
    assert.equal(
      compiled.stderr.includes("warning:"),
      false,
      `Не должно быть предупреждений компилятора:\n${compiled.stderr}`,
    );
  });

  it("SUB-984-1: сверяет константы AES-256-GCM, дизайн-токены, интерактивные состояния keyring-auth и IPC-каналы safeStorage с независимыми оракулами", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Независимый оракул: карточка TASK-984 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card984 = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-984",
    );
    assert.ok(
      card984,
      "Карточка TASK-984 должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card984.department, "G1 Backend");
    assert.equal(card984.layout.container, "Encrypted Store");
    assert.equal(card984.layout.dimensions, "AES-256-GCM");
    assert.deepEqual(card984.interactive_states[0], {
      element: "keyring-auth",
      default: "locked",
      hover: "unlocking",
      active: "decrypted",
    });

    // 2. Независимый оракул: 3 канала safeStorage в docs/re/backend-ipc.json
    const ipcRaw = readFileSync(BACKEND_IPC_JSON_PATH, "utf8");
    const ipcData = JSON.parse(ipcRaw);
    const ipcChannelNames = new Set(
      (ipcData.channels ?? []).map((/** @type {any} */ ch) => ch.name),
    );
    const expectedSafeStorageChannels = [
      "DISCORD_SAFE_STORAGE_DECRYPT_STRING",
      "DISCORD_SAFE_STORAGE_ENCRYPT_STRING",
      "DISCORD_SAFE_STORAGE_IS_ENCRYPTION_AVAILABLE",
    ];
    for (const ch of expectedSafeStorageChannels) {
      assert.ok(
        ipcChannelNames.has(ch),
        `Канал ${ch} должен присутствовать в оракуле docs/re/backend-ipc.json`,
      );
    }

    // 3. Независимый оракул: rust/crates/chord-platform/src/keyring.rs и /home/sanya/re-deep/rest-static.md
    const keyringRs = readFileSync(KEYRING_RS_PATH, "utf8");
    assert.match(keyringRs, /pub const SCHEMA_NAME: &str = "com\.discordlab\.Token";/);
    assert.match(keyringRs, /pub const ENCRYPTED_TOKEN_PREFIX: &str = "dQw4w9WgXcQ:";/);
    assert.match(keyringRs, /org\.freedesktop\.secrets/);

    if (existsSync(RE_REST_STATIC_PATH)) {
      const restStaticMd = readFileSync(RE_REST_STATIC_PATH, "utf8");
      assert.match(restStaticMd, /dQw4w9WgXcQ:/);
      assert.match(restStaticMd, /safeStorage/);
    }

    // 4. Независимый оракул: rust/crates/chord-store/src/lib.rs
    const storeLibRs = readFileSync(STORE_LIB_RS_PATH, "utf8");
    assert.match(
      storeLibRs,
      /pub const CORE_TABLES: \[&str; 5\] = \["guilds", "channels", "roles", "users", "emoji"\];/,
    );

    // 5. Сверка наличия токенов, IPC-каналов и состояний в исходнике encrypted_store.rs
    const source = readFileSync(MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card984.design_tokens)) {
      assert.ok(
        source.includes(tokenVal),
        `Дизайн-токен ${tokenVal} из TASK-984 должен присутствовать в encrypted_store.rs`,
      );
    }
    for (const ch of expectedSafeStorageChannels) {
      assert.ok(
        source.includes(`"${ch}"`),
        `IPC-канал ${ch} должен быть зафиксирован в encrypted_store.rs`,
      );
    }
    for (const stateLabel of ["locked", "unlocking", "decrypted"]) {
      assert.ok(
        source.includes(`"${stateLabel}"`),
        `Интерактивное состояние ${stateLabel} должно быть зафиксировано в encrypted_store.rs`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_984_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-984-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_984_1_.*ok/);
  });

  it("SUB-984-2: извлекает ключ из Linux Keyring без утечки в логи, деривирует подключи по HKDF-SHA256 (сверка с node:crypto), маскирует <скрыто> и затирает память zeroize", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const run = runCompiledRustTests(testBinPath, "test_sub_984_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-984-2 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_984_2_.*ok/);

    // Независимая побайтовая сверка HKDF-SHA256 и SafeStorage AES-256-GCM с node:crypto
    const vectorMatch = run.stdout.match(/SUB_984_2_ORACLE_JSON:(\{.*\})/);
    assert.ok(
      vectorMatch && vectorMatch[1],
      "Вывод SUB-984-2 должен содержать JSON-векторы для сверки с оракулом node:crypto",
    );
    const v = JSON.parse(vectorMatch[1]);

    const masterKey = Buffer.from(v.master_key_hex, "hex");
    const salt = Buffer.from(v.salt_hex, "hex");
    const prk = oracleHkdfExtract(salt, masterKey);
    assert.equal(prk.toString("hex"), v.prk_hex, "HKDF-Extract PRK обязан побайтно совпадать с node:crypto");

    const expectedPageKey = oracleHkdfExpand(
      prk,
      Buffer.from("discord-lab/sqlite-page-aes256gcm/v1", "utf8"),
      32,
    );
    const expectedWalKey = oracleHkdfExpand(
      prk,
      Buffer.from("discord-lab/sqlite-wal-aes256gcm/v1", "utf8"),
      32,
    );
    const expectedTokenKey = oracleHkdfExpand(
      prk,
      Buffer.from("discord-lab/safe-storage-token-aes256gcm/v1", "utf8"),
      32,
    );
    const expectedHeaderMacKey = oracleHkdfExpand(
      prk,
      Buffer.from("discord-lab/sqlite-header-hmac-sha256/v1", "utf8"),
      32,
    );

    assert.equal(expectedPageKey.toString("hex"), v.page_key_hex);
    assert.equal(expectedWalKey.toString("hex"), v.wal_key_hex);
    assert.equal(expectedTokenKey.toString("hex"), v.token_key_hex);
    assert.equal(expectedHeaderMacKey.toString("hex"), v.header_mac_key_hex);

    // Проверяем, что зашифрованный SafeStorage токен ("dQw4w9WgXcQ:<base64>") расшифровывается независимым node:crypto
    assert.ok(
      typeof v.safe_storage_token === "string" &&
        v.safe_storage_token.startsWith("dQw4w9WgXcQ:"),
    );
    const rawBlob = Buffer.from(
      v.safe_storage_token.slice("dQw4w9WgXcQ:".length),
      "base64",
    );
    const tokNonce = rawBlob.subarray(0, 12);
    const tokTag = rawBlob.subarray(rawBlob.length - 16);
    const tokCiphertext = rawBlob.subarray(12, rawBlob.length - 16);
    const decipher = createDecipheriv("aes-256-gcm", expectedTokenKey, tokNonce);
    decipher.setAAD(Buffer.from("discord-safe-storage-v1", "utf8"));
    decipher.setAuthTag(tokTag);
    const decryptedToken = Buffer.concat([
      decipher.update(tokCiphertext),
      decipher.final(),
    ]).toString("utf8");
    assert.equal(decryptedToken, v.expected_plain_token);

    // Проверяем отсутствие утечки ключа и токена в Debug и логах
    assert.equal(v.debug_leaked_secret, false);
    assert.equal(v.log_leaked_secret, false);
    assert.equal(scanTextForSecrets( v.redacted_log, "redacted_log").length, 0);
  });

  it("SUB-984-3: шифрует реальную БД SQLite на уровне страниц (4096 байт AES-256-GCM), скрывает заголовок 'SQLite format 3' и сообщения на диске, проходит сверку с node:crypto и node:sqlite", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Создаём реальную многостраничную базу SQLite на диске через node:sqlite (DatabaseSync)
    const plainDbPath = join(workDir, "plain-source.sqlite");
    const encDbPath = join(workDir, "encrypted-store.db.enc");
    const decDbPath = join(workDir, "decrypted-restored.sqlite");

    const db = new DatabaseSync(plainDbPath);
    try {
      db.exec(`
        PRAGMA page_size = 4096;
        PRAGMA journal_mode = DELETE;
        CREATE TABLE guilds (id TEXT PRIMARY KEY, name TEXT NOT NULL);
        CREATE TABLE channels (id TEXT PRIMARY KEY, guild_id TEXT, name TEXT NOT NULL, kind INTEGER NOT NULL, position INTEGER NOT NULL);
        CREATE TABLE messages (id TEXT PRIMARY KEY, channel_id TEXT NOT NULL, author_id TEXT NOT NULL, content TEXT NOT NULL, timestamp INTEGER NOT NULL);
        CREATE TABLE kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
      `);
      db.prepare("INSERT INTO guilds (id, name) VALUES (?, ?)").run(
        "100000000000000001",
        "Discord Lab Security Guild",
      );
      db.prepare(
        "INSERT INTO channels (id, guild_id, name, kind, position) VALUES (?, ?, ?, ?, ?)",
      ).run("200000000000000001", "100000000000000001", "encrypted-general", 0, 1);

      const insertMsg = db.prepare(
        "INSERT INTO messages (id, channel_id, author_id, content, timestamp) VALUES (?, ?, ?, ?, ?)",
      );
      for (let i = 1; i <= 60; i += 1) {
        insertMsg.run(
          String(300000000000000000n + BigInt(i)),
          "200000000000000001",
          "900000000000000001",
          `TOP_SECRET_MESSAGE_PAYLOAD_${i}_AES256GCM_SQLCIPHER_GUARD_${"X".repeat(120)}`,
          1_700_000_000_000 + i * 1000,
        );
      }
      db.prepare("INSERT INTO kv (key, value) VALUES (?, ?)").run(
        "session_token_ref",
        "SECRET_SESSION_MARKER_DO_NOT_LEAK_ON_DISK",
      );
    } finally {
      db.close();
    }

    const plainBytes = readFileSync(plainDbPath);
    assert.ok(
      plainBytes.length >= 8192 && plainBytes.length % 4096 === 0,
      `Ожидался размер SQLite кратный 4096 (>= 2 страниц), получено: ${plainBytes.length}`,
    );
    assert.equal(
      plainBytes.subarray(0, 16).toString("latin1"),
      "SQLite format 3\0",
      "Исходный файл SQLite содержит открытый заголовок 'SQLite format 3\\0'",
    );

    // 2. Запускаем Rust-тест SUB-984-3, передавая пути через переменные окружения
    const res = spawnSync(testBinPath, ["test_sub_984_3_", "--nocapture"], {
      cwd: ROOT,
      encoding: "utf8",
      timeout: 15_000,
      env: {
        ...process.env,
        TASK_984_PLAIN_DB: plainDbPath,
        TASK_984_ENC_DB: encDbPath,
        TASK_984_DEC_DB: decDbPath,
      },
    });
    assert.equal(
      res.status,
      0,
      `Тесты SUB-984-3 упали:\n${res.stdout}\n${res.stderr}`,
    );
    assert.match(res.stdout, /test tests::test_sub_984_3_.*ok/);

    // 3. Проверяем зашифрованный файл на диске независимым оракулом Node.js
    assert.ok(existsSync(encDbPath), "Зашифрованный файл БД должен быть записан на диск");
    const encBytes = readFileSync(encDbPath);

    // Зашифрованный файл начинается с сигнатуры контейнера "DLABENC1", а НЕ "SQLite format 3\0"
    assert.equal(encBytes.subarray(0, 8).toString("ascii"), "DLABENC1");
    assert.equal(
      encBytes.includes(Buffer.from("SQLite format 3")),
      false,
      "Зашифрованный файл БД на диске НЕ должен содержать открытый заголовок 'SQLite format 3'",
    );
    assert.equal(
      encBytes.includes(Buffer.from("TOP_SECRET_MESSAGE_PAYLOAD_1")),
      false,
      "Зашифрованный файл БД на диске НЕ должен содержать открытый текст сообщений",
    );
    assert.equal(
      encBytes.includes(Buffer.from("SECRET_SESSION_MARKER_DO_NOT_LEAK_ON_DISK")),
      false,
      "Зашифрованный файл БД на диске НЕ должен содержать открытые маркеры токенов",
    );

    // Энтропия Шеннона зашифрованных страниц должна быть > 7.8 бит/байт
    const entropy = computeShannonEntropy(encBytes.subarray(64));
    assert.ok(
      entropy > 7.8,
      `Энтропия зашифрованных страниц должна быть > 7.8 бит/байт, получено: ${entropy.toFixed(4)}`,
    );

    // 4. Независимая побайтовая проверка каждой зашифрованной страницы через node:crypto AES-256-GCM
    const metaMatch = res.stdout.match(/SUB_984_3_META_JSON:(\{.*\})/);
    assert.ok(metaMatch && metaMatch[1], "Ожидались метаданные шифрования SUB_984_3_META_JSON");
    const meta = JSON.parse(metaMatch[1]);
    const pageKey = Buffer.from(meta.page_key_hex, "hex");
    const salt16 = encBytes.subarray(16, 32);
    const pageCount = encBytes.readUInt32BE(12);
    assert.equal(pageCount, plainBytes.length / 4096);

    const RECORD_SIZE = 4 + 4 + 12 + 4096 + 16; // 4132 байта на страницу
    assert.equal(encBytes.length, 64 + pageCount * RECORD_SIZE);

    for (let i = 0; i < pageCount; i += 1) {
      const offset = 64 + i * RECORD_SIZE;
      const recPageNo = encBytes.readUInt32BE(offset);
      const recGen = encBytes.readUInt32BE(offset + 4);
      const recNonce = encBytes.subarray(offset + 8, offset + 20);
      const recCiphertext = encBytes.subarray(offset + 20, offset + 20 + 4096);
      const recTag = encBytes.subarray(offset + 20 + 4096, offset + RECORD_SIZE);

      assert.equal(recPageNo, i + 1);
      const plainSlice = plainBytes.subarray(i * 4096, (i + 1) * 4096);
      const oracle = oracleEncryptPageAes256Gcm(
        pageKey,
        salt16,
        recPageNo,
        recGen,
        plainSlice,
      );
      assert.equal(recNonce.toString("hex"), oracle.nonce.toString("hex"));
      assert.equal(recCiphertext.toString("hex"), oracle.ciphertext.toString("hex"));
      assert.equal(recTag.toString("hex"), oracle.tag.toString("hex"));
    }

    // 5. Открываем расшифрованный файл через node:sqlite DatabaseSync и проверяем PRAGMA integrity_check = ok
    const restoredBytes = readFileSync(decDbPath);
    assert.equal(
      createHash("sha256").update(restoredBytes).digest("hex"),
      createHash("sha256").update(plainBytes).digest("hex"),
      "Расшифрованная БД должна побайтно совпадать с исходной БД SQLite",
    );

    const restoredDb = new DatabaseSync(decDbPath, { readOnly: true });
    try {
      const integrity = restoredDb.prepare("PRAGMA integrity_check").get();
      assert.equal(integrity?.integrity_check, "ok");
      const msgCount = restoredDb
        .prepare("SELECT COUNT(*) AS cnt FROM messages")
        .get();
      assert.equal(Number(msgCount?.cnt), 60);
      const kvRow = restoredDb
        .prepare("SELECT value FROM kv WHERE key = ?")
        .get("session_token_ref");
      assert.equal(
        kvRow?.value,
        "SECRET_SESSION_MARKER_DO_NOT_LEAK_ON_DISK",
      );
    } finally {
      restoredDb.close();
    }
  });

  it("SUB-984-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake/2000/25MB/offset=9975/200%/99+, ошибки 401..5xx, HTTP 202, права AGE_GATED_SPACES, восстановление WAL)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const run = runCompiledRustTests(testBinPath, "test_sub_984_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-984-4 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_984_4_.*ok/);

    const reportMatch = run.stdout.match(/SUB_984_4_AUDIT_JSON:(\{.*\})/);
    assert.ok(reportMatch && reportMatch[1], "Ожидался JSON-отчёт аудита SUB_984_4_AUDIT_JSON");
    const audit = JSON.parse(reportMatch[1]);
    assert.equal(audit.task_id, "TASK-984");
    assert.equal(audit.cipher, "AES-256-GCM");
    assert.equal(audit.page_size, 4096);
    assert.equal(audit.keyring_state, "decrypted");
    assert.equal(audit.wal_recovered_frames, 2);
    assert.equal(audit.wal_discarded_uncommitted, 1);
    assert.equal(audit.secret_leaks, 0);
  });

  it("SUB-984-5: проходит мутационный гейт 5/5 (100% kill rate) и восстанавливает исходный MD5 модуля", () => {
    const originalSource = readFileSync(MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalSource).digest("hex");

    const mutationSpecs = [
      {
        id: "M1-KeyringDebugSecretLeak",
        operator: "DebugRedactionBypass",
        from: 'f.write_str("KeyringMasterSecret(<скрыто>)")',
        to: 'write!(f, "KeyringMasterSecret({:02x?})", self.0)',
      },
      {
        id: "M2-PageNonceReuseConstant",
        operator: "PageIvReuseConstantPageNo",
        from: "nonce[4..8].copy_from_slice(&page_no.to_be_bytes());",
        to: "nonce[4..8].copy_from_slice(&0u32.to_be_bytes());",
      },
      {
        id: "M3-GcmAuthTagVerificationBypass",
        operator: "GcmTagCheckBypass",
        from: "if !constant_time_eq(&expected_tag, tag) {",
        to: "if false {",
      },
      {
        id: "M4-PageAadSwapProtectionBypass",
        operator: "AadPageNumberBindingOmission",
        from: "aad[0..4].copy_from_slice(&page_no.to_be_bytes());",
        to: "aad[0..4].copy_from_slice(&0u32.to_be_bytes());",
      },
      {
        id: "M5-WalUncommittedPowerLossAccept",
        operator: "WalCommitFlagCheckBypass",
        from: "if frame.is_commit {",
        to: "if true {",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        originalSource.includes(spec.from),
        `Исходный код должен содержать целевую строку для мутации ${spec.id}: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "encrypted_store",
        operator: spec.operator,
        mutatedSource: originalSource.replace(spec.from, spec.to),
      };
    });

    const gateResult = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutSrcPath = join(workDir, `mutant_${idx + 1}.rs`);
      const mutBinPath = join(workDir, `mutant_${idx + 1}_bin`);
      writeFileSync(mutSrcPath, mutant.mutatedSource, "utf8");
      const compiled = compileRustModuleTests(mutSrcPath, mutBinPath);
      if (compiled.status !== 0) {
        return { killed: true, reason: "compile_error" };
      }
      const run = runCompiledRustTests(mutBinPath);
      const killed = run.status !== 0;
      return {
        killed,
        reason: killed ? "test_failed_as_expected" : "mutant_survived",
      };
    });

    const afterSource = readFileSync(MODULE_PATH, "utf8");
    const afterMd5 = createHash("md5").update(afterSource).digest("hex");
    assert.equal(
      afterMd5,
      originalMd5,
      "MD5 исходного файла encrypted_store.rs не должен измениться после мутационного тестирования",
    );

    assert.equal(
      gateResult.passed,
      true,
      `Мутационный гейт не пройден (${gateResult.verdict}): выжили [${gateResult.survivors.join(", ")}]`,
    );
    assert.equal(gateResult.killed, 5);
    assert.equal(gateResult.total, 5);
    assert.equal(gateResult.killRate, 1);
  });
});
