// @ts-check
/**
 * Контрактные и модульные тесты журнала аудита сервера (Server Audit Log & REST API v9 — TASK-1008):
 * - SUB-1008-0: Статический и компиляционный контракт `rust/discord-gpui/src/guild/audit_log.rs`
 *   и `scripts/verify-audit-log.mjs` (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде,
 *   соблюдение лимитов `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк,
 *   чистый `rustfmt --check --edition 2024`, компиляция `rustc --test --edition 2024 -D warnings`
 *   без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1008-1: Сверка карточек `TASK-1008`, `TASK-1008A`, `TASK-1008B` в `waves/kanban-board.json`,
 *   дизайн-токенов (`#313338`, `#2b2d31`, `#ffffff`, `#5865f2`, `#3f4147`), геометрии `Audit Log Viewer`
 *   (`100% x 100%`, `padding: 16px`, `spacing: 8px`), интерактивных состояний `audit-entry`
 *   (`entry row` -> `expand diff` -> `expanded details`), `audit-decode` (`raw` -> `decoding` -> `human readable`),
 *   `audit-row` (`summary` -> `hover-row` -> `expanded diff`), маршрута REST `GET /guilds/{id}/audit-logs`
 *   (`GUILD_AUDIT_LOG` и заголовка `X-Audit-Log-Reason` в `/home/sanya/re-deep/rest-static.json`),
 *   модулей RE-дампа (`117497`, `165259`, `411153`, `523599`, `555067`) и прав `VIEW_AUDIT_LOG = 1 << 7`
 *   в `rust/crates/discord-core/src/permissions.rs` с независимыми оракулами.
 * - SUB-1008-2: Формирование REST-запросов `GET /guilds/{id}/audit-logs` с query-фильтрами (`limit 1..=100`
 *   по умолчанию `50`, `user_id`, `action_type`, курсоры `before` / `after`), заголовок `X-Audit-Log-Reason`
 *   (`1..=512` символов), классификация `AuditLogActionType` (`1..=213`, категории `CREATE` / `UPDATE` / `DELETE`,
 *   типы целей `GUILD`, `CHANNEL`, `USER`, `ROLE`, `INVITE`, `WEBHOOK`, `EMOJI`, `MESSAGE`, `THREAD`, `AUTO_MODERATION_RULE`)
 *   и паритет русских меток с `rust/gui-spike/ui/features/audit-log.js`.
 * - SUB-1008-3: Декодирование изменений `changes` (`old_value` vs `new_value`), разрешение ссылок на `users[]`,
 *   `roles[]`, `channels[]`, форматирование дельт ролей `$add` / `$remove`, битовых масок прав (`allow` / `deny` /
 *   `permissions`), маскирование секретов (`token`, `webhook_token`, `/webhooks/`) и 100% паритет с
 *   `formatAuditChangeDiffRu` и `formatAuditLogEntryRu`.
 * - SUB-1008-4: Фильтрация записей журнала аудита по типу события (`action_type`), категории, автору (`user_id`),
 *   цели (`target_id`), поисковому запросу и временному интервалу (`start_ms..=end_ms` через Snowflake timestamp),
 *   группировка смежных записей в окне 30 минут (до 50 изменений), покрытие всех 9 измерений `TEST-POLICY.md`
 *   и проверка CLI `scripts/verify-audit-log.mjs` (коды выхода `0 / 1 / 2`).
 * - SUB-1008-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/discord-gpui/src/guild/audit_log.rs`
 *   и `scripts/verify-audit-log.mjs` по контракту `scripts/quality-gate.mjs`.
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
import {
  AUDIT_ACTION_LABELS_RU as spikeActionLabelsRu,
  AUDIT_ACTION_TYPES as spikeActionTypes,
  formatAuditChangeDiffRu as spikeFormatAuditChangeDiffRu,
  formatAuditLogEntryRu as spikeFormatAuditLogEntryRu,
} from "../../rust/gui-spike/ui/features/audit-log.js";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/discord-gpui/src/guild/audit_log.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const SCRIPT_REL_PATH = "scripts/verify-audit-log.mjs";
const SCRIPT_PATH = join(ROOT, SCRIPT_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const REST_D02_JSON_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const CORE_PERMISSIONS_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-core",
  "src",
  "permissions.rs",
);
const RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-gpui/src/guild/audit_log.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1008: Журнал аудита сервера (Server Audit Log & REST API v9 — rust/discord-gpui/src/guild/audit_log.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "audit-log-1008-"));
    testBinPath = join(workDir, "audit_log_1008_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1008-0: модули rust/discord-gpui/src/guild/audit_log.rs и scripts/verify-audit-log.mjs существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), запрещают unsafe и компилируются без предупреждений", () => {
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
      "В продуктовом коде audit_log.rs запрещены .unwrap() и .expect()",
    );

    const rustSizeCheck = checkRustSourceSizeLimits(RUST_MODULE_REL_PATH, rustSource);
    assert.equal(
      rustSizeCheck.passed,
      true,
      `Нарушение лимитов размера Rust-кода: ${rustSizeCheck.reason}`,
    );
    assert.ok(
      rustSizeCheck.fileLines > 250 && rustSizeCheck.fileLines < 800,
      `Ожидался размер файла 250..799 строк, получено: ${rustSizeCheck.fileLines}`,
    );

    const jsSource = readFileSync(SCRIPT_PATH, "utf8");
    const jsLines = jsSource.split("\n").length;
    assert.ok(
      jsLines > 200 && jsLines < 800,
      `Ожидался размер JS-скрипта 200..799 строк, получено: ${jsLines}`,
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
      `Ошибка компиляции rustc --test -D warnings:\n${compiled.stderr}`,
    );
    assert.equal(
      compiled.stderr.includes("warning:"),
      false,
      `Не должно быть предупреждений компилятора:\n${compiled.stderr}`,
    );

    const rustRun = runCompiledRustTests(testBinPath);
    assert.equal(
      rustRun.status,
      0,
      `Встроенные тесты audit_log.rs должны проходить:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test result: ok\./);
  });

  it("SUB-1008-1: сверяет дизайн-токены, раскладку Audit Log Viewer, состояния audit-entry/audit-decode/audit-row, эндпоинт GUILD_AUDIT_LOG и модули RE-дампа с независимыми оракулами", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      AUDIT_LOG_VIEWER_LAYOUT,
      AUDIT_LOG_DESIGN_TOKENS,
      AUDIT_ENTRY_STATES,
      AUDIT_DECODE_STATES,
      AUDIT_ROW_STATES,
      AUDIT_LOG_PROTOCOL_CONSTANTS,
      resolveAuditEntryState,
      resolveAuditDecodeState,
      verifyAuditLogOracleParity,
    } = await import("../../scripts/verify-audit-log.mjs");

    // 1. Оракул: карточки TASK-1008, TASK-1008A, TASK-1008B в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1008 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1008");
    const card1008A = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1008A");
    const card1008B = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1008B");
    assert.ok(card1008, "Карточка TASK-1008 должна присутствовать в waves/kanban-board.json");
    assert.ok(card1008A, "Карточка TASK-1008A должна присутствовать в waves/kanban-board.json");
    assert.ok(card1008B, "Карточка TASK-1008B должна присутствовать в waves/kanban-board.json");

    assert.deepEqual(AUDIT_LOG_VIEWER_LAYOUT, card1008.layout);
    assert.deepEqual(AUDIT_LOG_DESIGN_TOKENS, card1008.design_tokens);
    assert.deepEqual(AUDIT_ENTRY_STATES, card1008.interactive_states[0]);
    assert.deepEqual(AUDIT_DECODE_STATES, card1008A.interactive_states[0]);
    assert.deepEqual(AUDIT_ROW_STATES, card1008B.interactive_states[0]);

    // 2. Интерактивные состояния audit-entry и audit-decode
    assert.deepEqual(resolveAuditEntryState({ expanded: false, hovered: false }), {
      element: "audit-entry",
      state: "default",
      label: "entry row",
    });
    assert.deepEqual(resolveAuditEntryState({ expanded: false, hovered: true }), {
      element: "audit-entry",
      state: "hover",
      label: "expand diff",
    });
    assert.deepEqual(resolveAuditEntryState({ expanded: true, hovered: false }), {
      element: "audit-entry",
      state: "active",
      label: "expanded details",
    });

    assert.deepEqual(resolveAuditDecodeState("raw"), {
      element: "audit-decode",
      state: "default",
      label: "raw",
    });
    assert.deepEqual(resolveAuditDecodeState("decoding"), {
      element: "audit-decode",
      state: "hover",
      label: "decoding",
    });
    assert.deepEqual(resolveAuditDecodeState("decoded"), {
      element: "audit-decode",
      state: "active",
      label: "human readable",
    });

    // 3. Оракул rust/crates/discord-core/src/permissions.rs (VIEW_AUDIT_LOG = 1 << 7)
    const corePermsRs = readFileSync(CORE_PERMISSIONS_RS_PATH, "utf8");
    assert.match(corePermsRs, /pub const VIEW_AUDIT_LOG: u64 = 1 << 7;/);
    assert.equal(AUDIT_LOG_PROTOCOL_CONSTANTS.PERM_VIEW_AUDIT_LOG, 1n << 7n);
    assert.equal(AUDIT_LOG_PROTOCOL_CONSTANTS.PERM_ADMINISTRATOR, 1n << 3n);
    assert.equal(AUDIT_LOG_PROTOCOL_CONSTANTS.DEFAULT_PAGE_LIMIT, 50);
    assert.equal(AUDIT_LOG_PROTOCOL_CONSTANTS.MIN_PAGE_LIMIT, 1);
    assert.equal(AUDIT_LOG_PROTOCOL_CONSTANTS.MAX_PAGE_LIMIT, 100);
    assert.equal(AUDIT_LOG_PROTOCOL_CONSTANTS.MERGE_WINDOW_MINUTES, 30);
    assert.equal(AUDIT_LOG_PROTOCOL_CONSTANTS.MAX_GROUPED_CHANGES, 50);

    // 4. Полная сверка паритета с оракулами (включая /home/sanya/re-deep/rest-static.json и modules-index.json)
    const parity = verifyAuditLogOracleParity({
      kanbanPath: KANBAN_BOARD_PATH,
      restD02Path: REST_D02_JSON_PATH,
      corePermissionsRsPath: CORE_PERMISSIONS_RS_PATH,
      reRestStaticPath: existsSync(RE_REST_STATIC_PATH) ? RE_REST_STATIC_PATH : undefined,
      reModulesIndexPath: existsSync(RE_MODULES_INDEX_PATH) ? RE_MODULES_INDEX_PATH : undefined,
    });
    assert.equal(parity.passed, true, `Нарушение паритета с оракулами: ${parity.reasons.join("; ")}`);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1008_1_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1008-1 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1008_1_.*ok/);
  });

  it("SUB-1008-2: формирует GET /guilds/{id}/audit-logs с query-фильтрами и классифицирует AuditLogActionType 1..213 (CREATE/UPDATE/DELETE и targetType)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      AUDIT_ACTION_TYPES,
      AUDIT_ACTION_LABELS_RU,
      buildGuildAuditLogRequest,
      buildAuditLogReasonHeader,
      classifyAuditAction,
    } = await import("../../scripts/verify-audit-log.mjs");

    // 1. Паритет базовых кодов действий и русских меток с rust/gui-spike/ui/features/audit-log.js
    for (const [key, code] of Object.entries(spikeActionTypes)) {
      assert.equal(
        AUDIT_ACTION_TYPES[/** @type {keyof typeof AUDIT_ACTION_TYPES} */ (key)],
        code,
        `Код действия ${key} должен совпадать с audit-log.js`,
      );
      assert.equal(
        AUDIT_ACTION_LABELS_RU[code],
        spikeActionLabelsRu[code],
        `Русская метка для кода ${code} должна совпадать с audit-log.js`,
      );
    }

    // 2. Построение GET /guilds/{id}/audit-logs с дефолтным limit=50 и полным набором фильтров
    const baseReq = buildGuildAuditLogRequest({
      guildId: "942355007271997450",
    });
    assert.equal(baseReq.method, "GET");
    assert.equal(baseReq.path, "/guilds/942355007271997450/audit-logs?limit=50");
    assert.equal(baseReq.apiV9Path, "/api/v9/guilds/942355007271997450/audit-logs?limit=50");

    const filteredReq = buildGuildAuditLogRequest({
      guildId: "942355007271997450",
      userId: "479659568322969611",
      actionType: 22,
      before: "1558237752917758108",
      limit: 25,
    });
    assert.equal(filteredReq.method, "GET");
    assert.equal(
      filteredReq.path,
      "/guilds/942355007271997450/audit-logs?limit=25&user_id=479659568322969611&action_type=22&before=1558237752917758108",
    );

    // 3. Проверка заголовка X-Audit-Log-Reason
    const reasonHdr = buildAuditLogReasonHeader("Нарушение правил канала #general");
    assert.equal(reasonHdr.headerName, "X-Audit-Log-Reason");
    assert.equal(
      decodeURIComponent(reasonHdr.headerValue),
      "Нарушение правил канала #general",
    );
    assert.throws(() => buildAuditLogReasonHeader("x".repeat(513)), RangeError);

    // 4. Классификация AuditLogActionType (CREATE / UPDATE / DELETE и targetType по модулю 411153 RE-дампа)
    assert.deepEqual(classifyAuditAction(1), {
      code: 1,
      category: "UPDATE",
      targetType: "GUILD",
      labelRu: "Обновление сервера",
    });
    assert.deepEqual(classifyAuditAction(10), {
      code: 10,
      category: "CREATE",
      targetType: "CHANNEL",
      labelRu: "Создание канала",
    });
    assert.deepEqual(classifyAuditAction(22), {
      code: 22,
      category: "DELETE",
      targetType: "USER",
      labelRu: "Бан участника",
    });
    assert.deepEqual(classifyAuditAction(31), {
      code: 31,
      category: "UPDATE",
      targetType: "ROLE",
      labelRu: "Изменение роли",
    });
    assert.deepEqual(classifyAuditAction(72), {
      code: 72,
      category: "DELETE",
      targetType: "USER",
      labelRu: "Удаление сообщения",
    });
    assert.deepEqual(classifyAuditAction(140), {
      code: 140,
      category: "CREATE",
      targetType: "AUTO_MODERATION_RULE",
      labelRu: "Создание правила автомодерации",
    });
    assert.deepEqual(classifyAuditAction(145), {
      code: 145,
      category: "UPDATE",
      targetType: "USER",
      labelRu: "Тайм-аут участника автомодерацией",
    });

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1008_2_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1008-2 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1008_2_.*ok/);
  });

  it("SUB-1008-3: декодирует изменения changes (old_value vs new_value), сопоставляет пользователей/роли/каналы и маскирует секреты в диффах", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      formatAuditChangeDiffRu,
      formatAuditLogEntryRu,
      decodeAuditChangeDetail,
      normalizeAuditLogEntry,
    } = await import("../../scripts/verify-audit-log.mjs");

    // 1. 100% паритет с эталонными функциями rust/gui-spike/ui/features/audit-log.js
    const diffSamples = [
      { key: "name", oldValue: "старое", newValue: "новое" },
      { key: "topic", oldValue: null, newValue: "тема" },
      { key: "topic", oldValue: "тема", newValue: undefined },
      { key: "nsfw", oldValue: false, newValue: true },
      { key: "rate_limit_per_user", oldValue: 0, newValue: 15 },
    ];
    for (const sample of diffSamples) {
      assert.equal(
        formatAuditChangeDiffRu(sample),
        spikeFormatAuditChangeDiffRu(sample),
      );
    }

    const entrySamples = [
      { actionType: 10, actorName: "Алиса", targetName: "#общий" },
      { actionType: 22, actorName: "Модератор", targetName: "Боб", reason: "  спам  " },
      { actionType: 30, actorName: "", targetName: "role" },
      { actionType: 999, actorName: "Алиса", targetName: "x" },
    ];
    for (const sample of entrySamples) {
      assert.equal(
        formatAuditLogEntryRu(sample),
        spikeFormatAuditLogEntryRu(sample),
      );
    }

    // 2. Глубокое декодирование изменений: $add / $remove ролей, битовые маски прав, чувствительные поля
    const roleAddDiff = decodeAuditChangeDetail({
      key: "$add",
      oldValue: null,
      newValue: [
        { id: "100000000000000002", name: "Developer" },
        { id: "100000000000000003", name: "Moderator" },
      ],
    });
    assert.equal(roleAddDiff.key, "$add");
    assert.equal(roleAddDiff.oldDisplay, "нет");
    assert.equal(roleAddDiff.newDisplay, "Developer (100000000000000002), Moderator (100000000000000003)");
    assert.equal(
      roleAddDiff.formattedRu,
      "$add: нет → Developer (100000000000000002), Moderator (100000000000000003)",
    );

    const secretDiff = decodeAuditChangeDetail({
      key: "token",
      oldValue: "old_secret_value",
      newValue: "new_secret_value",
    });
    assert.equal(secretDiff.oldDisplay, "[REDACTED]");
    assert.equal(secretDiff.newDisplay, "[REDACTED]");
    assert.doesNotMatch(secretDiff.formattedRu, /secret_value/);

    const webhookUrlDiff = decodeAuditChangeDetail({
      key: "avatar_url",
      oldValue: null,
      newValue: "https://discord.com/api/webhooks/12345/secret_webhook_token",
    });
    assert.equal(webhookUrlDiff.newDisplay, "[REDACTED]");

    // 3. Нормализация сырой записи ответа GET /guilds/{id}/audit-logs с сопоставлением users[], channels[], roles[]
    const context = {
      users: new Map([
        ["479659568322969611", { id: "479659568322969611", username: "admin_user", globalName: "Админ" }],
        ["606781270382346250", { id: "606781270382346250", username: "bob_user", globalName: "Боб" }],
      ]),
      channels: new Map([
        ["1294815650186788916", { id: "1294815650186788916", name: "general" }],
      ]),
      roles: new Map([
        ["100000000000000003", { id: "100000000000000003", name: "Moderator" }],
      ]),
    };

    const normalized = normalizeAuditLogEntry(
      {
        id: "1558237752917758108",
        user_id: "479659568322969611",
        target_id: "606781270382346250",
        action_type: 22,
        reason: "Флуд в канале",
        changes: [
          { key: "nick", old_value: "BobOld", new_value: "BobBanned" },
        ],
      },
      context,
    );
    assert.equal(normalized.actorName, "Админ");
    assert.equal(normalized.targetName, "Боб");
    assert.equal(
      normalized.summaryRu,
      "Админ: Бан участника (Боб) — Причина: Флуд в канале",
    );
    assert.equal(normalized.changes.length, 1);
    assert.equal(normalized.changes[0]?.formattedRu, "nick: BobOld → BobBanned");

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1008_3_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1008-3 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1008_3_.*ok/);
  });

  it("SUB-1008-4: фильтрует журнал по типу события, автору и временному интервалу, группирует смежные записи, покрывает 9 измерений TEST-POLICY.md и проверяет CLI (0/1/2)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      extractSnowflakeTimestampMs,
      filterAuditLogEntries,
      groupConsecutiveAuditEntries,
      evaluateAuditLogNineDimensions,
      redactAuditLogTelemetry,
      runAuditLogCli,
    } = await import("../../scripts/verify-audit-log.mjs");

    // 1. Извлечение времени из Snowflake ID ((id >> 22) + 1420070400000)
    const ts1 = extractSnowflakeTimestampMs("1558237752917758108");
    const ts2 = extractSnowflakeTimestampMs("1558237967334772848");
    assert.ok(ts2 > ts1);

    const sampleEntries = [
      {
        id: "1558237967334772848",
        userId: "479659568322969611",
        actorName: "Алиса",
        targetId: "1294815650186788916",
        targetName: "#общий",
        actionType: 11,
        reason: "Обновление темы",
        timestampMs: ts2,
        changes: [{ key: "topic", oldValue: "v1", newValue: "v2" }],
      },
      {
        id: "1558237752917758108",
        userId: "606781270382346250",
        actorName: "Борис",
        targetId: "1068752077230182481",
        targetName: "Нарушитель",
        actionType: 22,
        reason: "Спам-рассылка",
        timestampMs: ts1,
        changes: [],
      },
    ];

    // 2. Фильтрация по типу события (actionType), автору (userId), временному интервалу и тексту
    assert.equal(
      filterAuditLogEntries(sampleEntries, { actionType: 22 }).length,
      1,
    );
    assert.equal(
      filterAuditLogEntries(sampleEntries, { userId: "479659568322969611" }).length,
      1,
    );
    assert.equal(
      filterAuditLogEntries(sampleEntries, {
        startTimeMs: ts2 - 10,
        endTimeMs: ts2 + 10,
      }).length,
      1,
    );
    assert.equal(
      filterAuditLogEntries(sampleEntries, { searchQuery: "спам" }).length,
      1,
    );

    // 3. Группировка смежных однотипных записей в окне 30 минут (по модулю 523599 GuildSettingsAuditLogStore)
    const grouped = groupConsecutiveAuditEntries([
      {
        id: "1558237967334772848",
        userId: "479659568322969611",
        targetId: "1294815650186788916",
        actionType: 11,
        timestampMs: ts1 + 60_000,
        changes: [{ key: "name", oldValue: "a", newValue: "b" }],
      },
      {
        id: "1558237752917758108",
        userId: "479659568322969611",
        targetId: "1294815650186788916",
        actionType: 11,
        timestampMs: ts1,
        changes: [{ key: "topic", oldValue: "t1", newValue: "t2" }],
      },
    ]);
    assert.equal(grouped.length, 1);
    assert.equal(grouped[0]?.changes.length, 2);

    // 4. Все 9 измерений TEST-POLICY.md
    const dimReport = evaluateAuditLogNineDimensions();
    assert.equal(
      dimReport.passed,
      true,
      `Сбой в 9 измерениях TEST-POLICY.md: ${dimReport.failures.join("; ")}`,
    );
    assert.equal(dimReport.dimensionsChecked, 9);

    // 5. Маскирование секретов и проверка CLI scripts/verify-audit-log.mjs (коды 0 / 1 / 2)
    const rawLog =
      'GET /api/v9/guilds/942355007271997450/audit-logs Authorization="Bearer mfa.secret_audit_token_1008" token=MTIzNDU2Nzg5MDEyMzQ1Njc4.GhIjKl.MnOpQrStUvWxYz0123456789abcdef';
    const redacted = redactAuditLogTelemetry(rawLog);
    assert.doesNotMatch(redacted, /mfa\.secret_audit_token_1008/);
    assert.doesNotMatch(redacted, /GhIjKl\.MnOpQrStUvWxYz0123456789abcdef/);
    assert.match(redacted, /\[REDACTED\]/);

    const outJson = join(workDir, "audit-log-report.json");
    const outMd = join(workDir, "AUDIT-LOG-REPORT.md");
    assert.equal(runAuditLogCli(["--help"]), 0);
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
      [SCRIPT_PATH, "--oracle", "/nonexistent/missing-kanban-1008.json"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliMissingOracle.status, 2);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1008_4_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1008-4 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1008_4_.*ok/);
  });

  it("SUB-1008-5: убивает 5/5 контрольных мутаций в rust/discord-gpui/src/guild/audit_log.rs и scripts/verify-audit-log.mjs (100% mutation kill rate)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalRustMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutationSpecs = [
      {
        id: "MUT-1008-RS-01",
        operator: "ViewAuditLogPermissionBitTamper",
        from: "pub const PERM_VIEW_AUDIT_LOG: u64 = 1 << 7;",
        to: "pub const PERM_VIEW_AUDIT_LOG: u64 = 1 << 6;",
      },
      {
        id: "MUT-1008-RS-02",
        operator: "DefaultAuditLogLimitTamper",
        from: "pub const DEFAULT_AUDIT_LOG_LIMIT: u8 = 50;",
        to: "pub const DEFAULT_AUDIT_LOG_LIMIT: u8 = 10;",
      },
      {
        id: "MUT-1008-RS-03",
        operator: "ChangeDiffArrowTamper",
        from: 'format!("{key}: {old_str} → {new_str}")',
        to: 'format!("{key}: {new_str} → {old_str}")',
      },
      {
        id: "MUT-1008-RS-04",
        operator: "ActionTypeFilterInversion",
        from: ".is_some_and(|act| entry.action_type != act)",
        to: ".is_some_and(|act| entry.action_type == act)",
      },
      {
        id: "MUT-1008-RS-05",
        operator: "SensitiveKeyRedactionBypass",
        from: "if is_sensitive_change_key(key) {",
        to: "if false && is_sensitive_change_key(key) {",
      },
    ];

    const rustMutants = rustMutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в audit_log.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "audit_log.rs",
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
    assert.equal(afterRustMd5, originalRustMd5, "MD5 исходного файла audit_log.rs не должен измениться");
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
        id: "MUT-1008-JS-01",
        operator: "DefaultPageLimitTamper",
        from: "DEFAULT_PAGE_LIMIT: 50,",
        to: "DEFAULT_PAGE_LIMIT: 20,",
      },
      {
        id: "MUT-1008-JS-02",
        operator: "AuditEntryActiveStateTamper",
        from: 'active: "expanded details",',
        to: 'active: "collapsed",',
      },
      {
        id: "MUT-1008-JS-03",
        operator: "EmptyDiffPlaceholderTamper",
        from: 'const EMPTY_VALUE_RU = "нет";',
        to: 'const EMPTY_VALUE_RU = "пусто";',
      },
      {
        id: "MUT-1008-JS-04",
        operator: "FilterByActionInversion",
        from: "if (filter.actionType != null && entry.actionType !== filter.actionType) {",
        to: "if (filter.actionType != null && entry.actionType === filter.actionType) {",
      },
      {
        id: "MUT-1008-JS-05",
        operator: "FilterByUserInversion",
        from: "if (filter.userId && entry.userId !== filter.userId) {",
        to: "if (filter.userId && entry.userId === filter.userId) {",
      },
    ];

    const jsMutants = jsMutationSpecs.map((spec) => {
      assert.ok(
        originalJs.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в verify-audit-log.mjs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "verify-audit-log.mjs",
        operator: spec.operator,
        mutatedSource: originalJs.replace(spec.from, spec.to),
      };
    });

    const jsGate = evaluateMutationGate(jsMutants, (mutant, index) => {
      const mutantJsPath = join(workDir, `mutant_audit_${index + 1}.mjs`);
      const harnessPath = join(workDir, `mutant_audit_harness_${index + 1}.mjs`);
      writeFileSync(mutantJsPath, mutant.mutatedSource, "utf8");
      writeFileSync(
        harnessPath,
        [
          'import assert from "node:assert/strict";',
          `import { AUDIT_ENTRY_STATES, buildGuildAuditLogRequest, formatAuditChangeDiffRu, filterAuditLogEntries } from ${JSON.stringify(mutantJsPath)};`,
          'assert.equal(AUDIT_ENTRY_STATES.active, "expanded details");',
          'assert.equal(buildGuildAuditLogRequest({ guildId: "942355007271997450" }).path, "/guilds/942355007271997450/audit-logs?limit=50");',
          'assert.equal(formatAuditChangeDiffRu({ key: "topic", oldValue: null, newValue: "x" }), "topic: нет → x");',
          'assert.equal(filterAuditLogEntries([{ id: "1", userId: "10", actionType: 22, timestampMs: 1000, changes: [] }], { actionType: 22 }).length, 1);',
          'assert.equal(filterAuditLogEntries([{ id: "1", userId: "10", actionType: 22, timestampMs: 1000, changes: [] }], { userId: "10" }).length, 1);',
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
