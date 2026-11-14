// @ts-check
/**
 * Контрактные и модульные тесты парсера `AuditLogActionType 1..145` и декодера диффов `old/new`
 * со связыванием объектов `users[]`, `roles[]` и `channels[]` (TASK-1008A):
 * - SUB-1008A-0: Статический и компиляционный контракт `rust/crates/chord-core/src/audit_log.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, модуль < 800 строк, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1008A-1: Сверка карточки `TASK-1008A` в `waves/kanban-board.json`, дизайн-токенов
 *   (`#1e1f22`, `#2b2d31`, `#ffffff`, `#5865f2`, `#3f4147`), геометрии `Audit Protocol`
 *   (`Core Module`, `padding: 0px`, `spacing: 0px`), интерактивных состояний `audit-decode`
 *   (`raw -> decoding -> human readable`), REST-оракулов `/home/sanya/re-deep/rest-d02.json`
 *   (`docs/re/rest-d02.json`), `/home/sanya/re-deep/rest-static.json` (`GUILD_AUDIT_LOG`,
 *   `X-Audit-Log-Reason`), модулей RE-дампа (`117497`, `165259`, `411153`, `523599`, `555067`)
 *   и флага `VIEW_AUDIT_LOG = 1 << 7` в `rust/crates/chord-core/src/permissions.rs`.
 * - SUB-1008A-2: Парсер `AuditLogActionType` (`1..=145`), классификация категорий (`Create`,
 *   `Update`, `Delete`, `Other`) и типов целей (`Guild`, `Channel`, `ChannelOverwrite`, `User`,
 *   `Role`, `Invite`, `Webhook`, `Emoji`, `Sticker`, `Thread`, `AutoModerationRule`), паритет
 *   русских меток с `rust/gui-spike/ui/features/audit-log.js` и построение REST-запроса
 *   `GET /guilds/{id}/audit-logs` (`limit 1..=100`, по умолчанию `50`).
 * - SUB-1008A-3: Декодирование старых и новых значений `AuditLogChange` (`old_value`, `new_value`),
 *   сопоставление `user_id` модератора с объектом `users[]`, разрешение `target_id` и ссылочных
 *   полей по `users[]`, `roles[]`, `channels[]`, форматирование дельт ролей `$add`/`$remove`,
 *   битовых масок прав и маскирование секретов.
 * - SUB-1008A-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные Snowflake `0`/`u64::MAX`, лимиты `1..=100`, причина `1..=512`, `2000` символов,
 *   `10` файлов/`25 МБ`, `offset=9975`, громкость `0..200%`, бейдж `99+`, коды HTTP
 *   `200/202/204/400/401/403/404/413/429 retry_after_ms/5xx`, пустые диффы, `skeleton`/`HTTP 202`,
 *   права `VIEW_AUDIT_LOG`/`ADMINISTRATOR`/`AGE_GATED_SPACES`, группировка 30 мин и очистка кэша,
 *   маскирование секретов `mfa.***` и `dQw4w9WgXcQ:***`).
 * - SUB-1008A-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
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
} from "../../rust/gui-spike/ui/features/audit-log.js";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const MODULE_REL_PATH = "rust/crates/chord-core/src/audit_log.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const RE_REST_D02_PATH = "/home/sanya/re-deep/rest-d02.json";
const DOCS_REST_D02_JSON_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";
const PERMISSIONS_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "chord-core",
  "src",
  "permissions.rs",
);

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/crates/chord-core/src/audit_log.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1008A: Парсер AuditLogActionType 1..145 и декодер диффов old/new (rust/crates/chord-core/src/audit_log.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "audit-log-parser-1008a-"));
    testBinPath = join(workDir, "audit_log_parser_1008a_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1008A-0: проверяет статический и компиляционный контракт rust/crates/chord-core/src/audit_log.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024, secret-scan и rustc --test -D warnings)", () => {
    assert.ok(
      existsSync(MODULE_PATH),
      `Отсутствует Rust-модуль ${MODULE_REL_PATH}`,
    );

    const rustSource = readFileSync(MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле audit_log.rs обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде audit_log.rs запрещены .unwrap() и .expect()",
    );

    const sizeCheck = checkRustSourceSizeLimits(MODULE_REL_PATH, rustSource);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines >= 250 && sizeCheck.fileLines < 800,
      `Ожидался размер модуля 250..799 строк, получено: ${sizeCheck.fileLines}`,
    );

    const secretFindings = scanTextForSecrets(rustSource, MODULE_REL_PATH);
    assert.equal(
      secretFindings.length,
      0,
      `Обнаружены секреты в исходнике: ${JSON.stringify(secretFindings)}`,
    );

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check завершился с ошибкой: ${fmtCheck.stderr || fmtCheck.stdout}`,
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

  it("SUB-1008A-1: сверяет карточку TASK-1008A в waves/kanban-board.json, дизайн-токены, геометрию Audit Protocol, состояния audit-decode и RE-оракулы (rest-d02.json, rest-static.json, modules-index.json, permissions.rs)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Оракул: карточка TASK-1008A в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1008A = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1008A",
    );
    assert.ok(
      card1008A,
      "Карточка TASK-1008A должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1008A.department, "G1 Backend");
    assert.equal(card1008A.priority, "P1");
    assert.deepEqual(card1008A.layout, {
      container: "Audit Protocol",
      dimensions: "Core Module",
      padding: "0px",
      spacing: "0px",
    });
    assert.deepEqual(card1008A.design_tokens, {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#5865f2",
      border: "#3f4147",
    });
    assert.deepEqual(card1008A.interactive_states[0], {
      element: "audit-decode",
      default: "raw",
      hover: "decoding",
      active: "human readable",
    });

    // 2. Оракул: re-deep/rest-d02.json (и локальное зеркало docs/re/rest-d02.json)
    const restD02OraclePath = existsSync(RE_REST_D02_PATH)
      ? RE_REST_D02_PATH
      : DOCS_REST_D02_JSON_PATH;
    const restD02 = JSON.parse(readFileSync(restD02OraclePath, "utf8"));
    assert.equal(restD02.snapshotAt, "2026-10-19T11:51:47+05:00");
    assert.equal(restD02.totals.apiRequests, 133);
    assert.equal(restD02.totals.apiUniqueEndpoints, 47);
    const guildAppCmdEp = restD02.endpoints.find(
      (/** @type {any} */ ep) =>
        ep.method === "GET" &&
        ep.path === "/api/v9/guilds/{id}/application-command-index",
    );
    assert.ok(
      guildAppCmdEp,
      "В rest-d02.json должен присутствовать GET /api/v9/guilds/{id}/application-command-index",
    );
    assert.deepEqual(guildAppCmdEp.statuses, [200]);
    assert.ok(guildAppCmdEp.requestIds.includes("run2|95974.8044"));

    // 3. Оракул: /home/sanya/re-deep/rest-static.json (GUILD_AUDIT_LOG + X-Audit-Log-Reason)
    if (existsSync(RE_REST_STATIC_PATH)) {
      const restStatic = JSON.parse(readFileSync(RE_REST_STATIC_PATH, "utf8"));
      const auditEndpoint = (restStatic.endpoints ?? []).find(
        (/** @type {any} */ e) => e.key === "GUILD_AUDIT_LOG",
      );
      assert.ok(auditEndpoint, "В rest-static.json обязан быть GUILD_AUDIT_LOG");
      assert.equal(auditEndpoint.method, "GET");
      assert.equal(auditEndpoint.path, "/guilds/{arg}/audit-logs");
      const extraHeaders = restStatic.auth?.requestAuth?.extraHeaders ?? [];
      assert.ok(extraHeaders.includes("X-Audit-Log-Reason"));
    }

    // 4. Оракул: /home/sanya/re-deep/modules-index.json (модули 117497, 165259, 411153, 523599, 555067)
    if (existsSync(RE_MODULES_INDEX_PATH)) {
      const modIdx = JSON.parse(readFileSync(RE_MODULES_INDEX_PATH, "utf8"));
      assert.ok(modIdx["117497"].strings.includes("auditLogSection_b2f52f"));
      assert.ok(modIdx["165259"].strings.includes("auditLog__43dab"));
      assert.ok(modIdx["411153"]);
      assert.ok(
        modIdx["523599"].names.includes("GuildSettingsAuditLogStore"),
      );
      assert.ok(modIdx["555067"].strings.includes("AuditLogUtils"));
    }

    // 5. Оракул: permissions.rs (VIEW_AUDIT_LOG = 1 << 7)
    const permsSrc = readFileSync(PERMISSIONS_RS_PATH, "utf8");
    assert.match(permsSrc, /pub const VIEW_AUDIT_LOG: u64 = 1 << 7;/);

    // 6. Сверка констант в исходнике audit_log.rs
    const rustSource = readFileSync(MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1008A.design_tokens)) {
      assert.ok(
        rustSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} карточки TASK-1008A должен присутствовать в audit_log.rs`,
      );
    }
    assert.ok(rustSource.includes("Audit Protocol"));
    assert.ok(rustSource.includes("Core Module"));
    assert.ok(rustSource.includes("audit-decode"));
    assert.ok(rustSource.includes("/guilds/{id}/audit-logs"));

    const run = runCompiledRustTests(testBinPath, "test_sub_1008a_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1008A-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1008a_1_.*ok/);
  });

  it("SUB-1008A-2: проверяет парсер AuditLogActionType 1..145, категории (Create/Update/Delete), типы целей, паритет русских меток с audit-log.js и построение GET /guilds/{id}/audit-logs", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(MODULE_PATH, "utf8");
    for (const symbol of [
      "AuditLogActionType",
      "AuditActionCategory",
      "AuditTargetKind",
      "AuditDecodePhase",
      "GuildAuditLogQuery",
      "RestAuditLogRequest",
      "build_guild_audit_logs_request",
      "validate_audit_reason_header",
      "audit_action_label_ru",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В audit_log.rs должен присутствовать символ ${symbol}`,
      );
    }

    // Проверка паритета меток из rust/gui-spike/ui/features/audit-log.js
    for (const code of Object.values(spikeActionTypes)) {
      const labelRu = spikeActionLabelsRu[code];
      if (!labelRu) continue;
      assert.ok(
        rustSource.includes(labelRu),
        `Русская метка "${labelRu}" для кода ${code} должна присутствовать в audit_log.rs`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1008a_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1008A-2 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1008a_2_.*ok/);
  });

  it("SUB-1008A-3: проверяет декодирование старых и новых значений AuditLogChange (old_value, new_value), сопоставление user_id модератора с users[] и разрешение ссылок roles[]/channels[]", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(MODULE_PATH, "utf8");
    for (const symbol of [
      "AuditChangeValue",
      "AuditLogChange",
      "DecodedAuditChange",
      "AuditLogUser",
      "AuditLogRole",
      "AuditLogChannel",
      "AuditLogContext",
      "RawAuditLogEntry",
      "DecodedAuditLogEntry",
      "decode_audit_log_change",
      "decode_audit_log_entry",
      "resolve_moderator_user",
      "resolve_target_entity",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В audit_log.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1008a_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1008A-3 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1008a_3_.*ok/);
  });

  it("SUB-1008A-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, лимиты 1..100, причина 1..512, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, коды 200..5xx, skeleton/202, права VIEW_AUDIT_LOG/AGE_GATED_SPACES, 30-мин группировка и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(MODULE_PATH, "utf8");
    for (const symbol of [
      "PERM_VIEW_AUDIT_LOG",
      "PERM_ADMINISTRATOR",
      "validate_audit_workload_boundaries",
      "classify_audit_http_status",
      "check_audit_log_access",
      "can_group_consecutive_entries",
      "format_audit_badge",
      "redact_audit_telemetry",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В audit_log.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1008a_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1008A-4 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1008a_4_.*ok/);
  });

  it("SUB-1008A-5: убивает 5/5 контрольных мутаций в rust/crates/chord-core/src/audit_log.rs (100% mutation kill rate: подмена стрелки диффа old/new, обход сопоставления модератора в users[], искажение кода AutoModeration 145, обход маскирования чувствительных ключей, искажение дефолтного лимита 50)", () => {
    assert.ok(existsSync(MODULE_PATH), "Модуль audit_log.rs должен существовать");
    const originalRust = readFileSync(MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1008A-RS-01",
        moduleName: "audit_log",
        operator: "ChangeDiffOldNewOrderSwap",
        from: 'let formatted_ru = format!("{}: {old_display} → {new_display}", change.key);',
        to: 'let formatted_ru = format!("{}: {new_display} → {old_display}", change.key);',
      },
      {
        id: "MUT-1008A-RS-02",
        moduleName: "audit_log",
        operator: "ModeratorUserLookupBypass",
        from: "if let Some(user) = uid.and_then(|id| ctx.users.get(&id)) {",
        to: "if let Some(user) = None::<&AuditLogUser> {",
      },
      {
        id: "MUT-1008A-RS-03",
        moduleName: "audit_log",
        operator: "ActionType145MappingTamper",
        from: "145 => Self::AutoModerationUserCommunicationDisabled,",
        to: "145 => Self::Unknown(145),",
      },
      {
        id: "MUT-1008A-RS-04",
        moduleName: "audit_log",
        operator: "SensitiveChangeKeyRedactionBypass",
        from: "if is_sensitive_change_key(key) {",
        to: "if false && is_sensitive_change_key(key) {",
      },
      {
        id: "MUT-1008A-RS-05",
        moduleName: "audit_log",
        operator: "DefaultAuditLogPageLimitTamper",
        from: "pub const DEFAULT_AUDIT_LOG_LIMIT: u8 = 50;",
        to: "pub const DEFAULT_AUDIT_LOG_LIMIT: u8 = 20;",
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в audit_log.rs: ${m.from}`,
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
      .update(readFileSync(MODULE_PATH, "utf8"))
      .digest("hex");
    assert.equal(
      afterMd5,
      originalMd5,
      "MD5 исходного файла audit_log.rs не должен измениться после мутационного гейта",
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
