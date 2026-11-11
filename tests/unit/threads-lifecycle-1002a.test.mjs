// @ts-check
/**
 * Контрактные и модульные тесты жизненного цикла тредов: REST создание
 * `POST /channels/{id}/messages/{msg_id}/threads`, модель `ThreadMetadata`
 * и таймеры автоархивации `60..10080` мин (TASK-1002A):
 * - SUB-1002A-0: Статический и структурный контракт `rust/crates/discord-core/src/threads.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, соблюдение лимитов
 *   `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк, чистый `rustfmt --check --edition 2024`
 *   и компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1002A-1: Сверка карточки `TASK-1002A` в `waves/kanban-board.json`, дизайн-токенов, геометрии
 *   `Threads Engine` (`Backend Module`, `padding: 0px`, `spacing: 0px`), интерактивных состояний
 *   `thread-lifecycle` (`active` -> `archiving` -> `archived`), типов каналов `11` (`PUBLIC_THREAD`)
 *   и `12` (`PRIVATE_THREAD`), маршрутов REST (`POST /channels/{id}/messages/{msg_id}/threads`,
 *   `POST /channels/{id}/threads`, `GET /api/v9/channels/{id}/threads/search` в `re-deep/rest-d02.json`)
 *   и модулей RE-дампа (`45494`, `152007`, `219065`, `456874`, `970278`) с независимыми оракулами.
 * - SUB-1002A-2: Модель `ThreadMetadata`, создание тредов типов `11` (`PUBLIC_THREAD`) и `12` (`PRIVATE_THREAD`),
 *   формирование REST-конверта `POST /channels/{id}/messages/{msg_id}/threads` с ключами
 *   `{name,type,auto_archive_duration,location}`, проверка прав `CREATE_PUBLIC_THREADS` / `CREATE_PRIVATE_THREADS`
 *   и синхронизация участников `THREAD_MEMBERS_UPDATE` (`apply_thread_members_update`).
 * - SUB-1002A-3: Таймеры автоархивации (`60`, `1440`, `4320`, `10080` минут в диапазоне `60..=10080`),
 *   переходы жизненного цикла `active` -> `archiving` -> `archived`, расчёт оставшегося времени до архивации
 *   и защита `locked`-тредов без права `MANAGE_THREADS`.
 * - SUB-1002A-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные Snowflake `0`/`u64::MAX`, длительности `59/60`, `1439/1440`, `4319/4320`, `10079/10080`, `10081` мин,
 *   имя треда `1..100` vs `101`, 2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`,
 *   бейдж `99+`, коды `400/401/403/404/413/429/5xx`, пустые, `skeleton`/`HTTP 202`, права и `AGE_GATED_SPACES`,
 *   восстановление `THREAD_LIST_SYNC` и маскирование секретов в JSON-телеметрии).
 * - SUB-1002A-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
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
const MODULE_REL_PATH = "rust/crates/discord-core/src/threads.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const RE_REST_D02_PATH = "/home/sanya/re-deep/rest-d02.json";
const DOCS_REST_D02_JSON_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const PERMISSIONS_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-core",
  "src",
  "permissions.rs",
);
const REPLY_THREAD_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-ui",
  "src",
  "reply_thread.rs",
);
const THREADS_DRAWER_JS_PATH = join(
  ROOT,
  "rust",
  "gui-spike",
  "ui",
  "features",
  "threads-drawer.js",
);
const RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/crates/discord-core/src/threads.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1002A: Жизненный цикл тредов: REST создание и таймеры автоархивации (rust/crates/discord-core/src/threads.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "threads-lifecycle-1002a-"));
    testBinPath = join(workDir, "threads_lifecycle_1002a_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1002A-0: модуль rust/crates/discord-core/src/threads.rs существует, запрещает unsafe, проходит rustfmt 2024, лимиты (<50 строк на fn, <800 на файл), secret-scan и компилируется без предупреждений", () => {
    assert.ok(
      existsSync(MODULE_PATH),
      `Модуль ${MODULE_REL_PATH} должен существовать`,
    );

    const source = readFileSync(MODULE_PATH, "utf8");
    assert.match(
      source,
      /#!\[forbid\(unsafe_code\)\]/,
      "Модуль обязан содержать #![forbid(unsafe_code)]",
    );

    const prodCode = source.split("#[cfg(test)]")[0] ?? source;
    assert.doesNotMatch(
      prodCode,
      /\.unwrap\(\)|\.expect\(/,
      "В продуктовом коде threads.rs запрещены .unwrap() и .expect()",
    );

    const secretFindings = scanTextForSecrets(source, MODULE_REL_PATH);
    assert.equal(
      secretFindings.length,
      0,
      `Обнаружены незамаскированные секреты в исходнике: ${JSON.stringify(secretFindings)}`,
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

  it("SUB-1002A-1: сверяет карточку TASK-1002A, дизайн-токены, геометрию Threads Engine, интерактивные состояния thread-lifecycle, оракул re-deep/rest-d02.json и модули RE-дампа", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Независимый оракул: карточка TASK-1002A в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1002a = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1002A",
    );
    assert.ok(
      card1002a,
      "Карточка TASK-1002A должна существовать в waves/kanban-board.json",
    );
    assert.equal(card1002a.department, "G1 Backend");
    assert.equal(card1002a.layout.container, "Threads Engine");
    assert.equal(card1002a.layout.dimensions, "Backend Module");
    assert.equal(card1002a.layout.padding, "0px");
    assert.equal(card1002a.layout.spacing, "0px");
    assert.equal(card1002a.interactive_states[0].element, "thread-lifecycle");
    assert.equal(card1002a.interactive_states[0].default, "active");
    assert.equal(card1002a.interactive_states[0].hover, "archiving");
    assert.equal(card1002a.interactive_states[0].active, "archived");

    // 2. Независимый оракул: re-deep/rest-d02.json (и локальное зеркало docs/re/rest-d02.json)
    const restD02OraclePath = existsSync(RE_REST_D02_PATH)
      ? RE_REST_D02_PATH
      : DOCS_REST_D02_JSON_PATH;
    const restD02Text = readFileSync(restD02OraclePath, "utf8");
    assert.match(restD02Text, /"\/api\/v9\/channels\/\{id\}\/threads\/search"/);
    assert.match(restD02Text, /"THREAD_SEARCH"/);
    assert.match(
      restD02Text,
      /"\{threads,members,has_more,first_messages,most_recent_messages\}"/,
    );
    assert.match(restD02Text, /"run2\|95974\.7947"/);

    // 3. Независимый оракул: права тредов в rust/crates/discord-core/src/permissions.rs
    const permSrc = readFileSync(PERMISSIONS_RS_PATH, "utf8");
    assert.match(permSrc, /pub const MANAGE_THREADS: u64 = 1 << 34;/);
    assert.match(permSrc, /pub const CREATE_PUBLIC_THREADS: u64 = 1 << 35;/);
    assert.match(permSrc, /pub const CREATE_PRIVATE_THREADS: u64 = 1 << 36;/);
    assert.match(permSrc, /pub const SEND_MESSAGES_IN_THREADS: u64 = 1 << 38;/);

    // 4. Независимый оракул: ALLOWED_DURATIONS в rust/crates/discord-ui/src/reply_thread.rs и threads-drawer.js
    const replyThreadSrc = readFileSync(REPLY_THREAD_RS_PATH, "utf8");
    assert.match(
      replyThreadSrc,
      /pub const ALLOWED_DURATIONS: \[u32; 4\] = \[60, 1440, 4320, 10080\];/,
    );
    const drawerSrc = readFileSync(THREADS_DRAWER_JS_PATH, "utf8");
    assert.match(drawerSrc, /minutes:.*\(60\).*labelRu: "1 ч\."/);
    assert.match(drawerSrc, /minutes:.*\(1440\).*labelRu: "24 ч\."/);
    assert.match(drawerSrc, /minutes:.*\(4320\).*labelRu: "3 дн\."/);
    assert.match(drawerSrc, /minutes:.*\(10080\).*labelRu: "1 нед\."/);

    // 5. Независимый оракул: RE-дамп /home/sanya/re-deep/rest-static.json и modules-index.json
    if (existsSync(RE_REST_STATIC_PATH)) {
      const restStaticText = readFileSync(RE_REST_STATIC_PATH, "utf8");
      assert.match(
        restStaticText,
        /"\/channels\/\{arg\}\/messages\/\{arg\}\/threads"/,
      );
      assert.match(restStaticText, /"CHANNEL_MESSAGE_THREADS"/);
      assert.match(
        restStaticText,
        /"\{name,type,auto_archive_duration,location\}"/,
      );
    }
    if (existsSync(RE_MODULES_INDEX_PATH)) {
      const modulesIdx = JSON.parse(
        readFileSync(RE_MODULES_INDEX_PATH, "utf8"),
      );
      assert.ok(modulesIdx["45494"].names.includes("ArchivedThreadsStore"));
      assert.ok(modulesIdx["152007"].names.includes("JoinedThreadsStore"));
      assert.ok(modulesIdx["219065"].names.includes("ThreadMemberListStore"));
      assert.ok(modulesIdx["456874"].names.includes("ThreadMessageStore"));
      assert.ok(modulesIdx["970278"].names.includes("ActiveThreadsStore"));
    }

    // 6. Сверка исходника threads.rs с токенами и константами оракулов TASK-1002A
    const source = readFileSync(MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1002a.design_tokens)) {
      assert.ok(
        source.includes(tokenVal),
        `Дизайн-токен ${tokenVal} должен быть зафиксирован в threads.rs`,
      );
    }
    assert.ok(
      source.includes("/channels/{id}/messages/{msg_id}/threads"),
      "Шаблон маршрута POST /channels/{id}/messages/{msg_id}/threads должен присутствовать в threads.rs",
    );
    assert.ok(
      source.includes("/api/v9/channels/{id}/threads/search"),
      "Маршрут поиска тредов из re-deep/rest-d02.json должен присутствовать в threads.rs",
    );

    const run = runCompiledRustTests(testBinPath, "test_sub_1002a_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1002A-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1002a_1_.*ok/);
  });

  it("SUB-1002A-2: реализует модель ThreadMetadata, метод create_thread_from_message() с валидацией auto_archive_duration (60..10080 мин), проверяет права и синхронизирует участников THREAD_MEMBERS_UPDATE", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "CHANNEL_TYPE_PUBLIC_THREAD",
      "CHANNEL_TYPE_PRIVATE_THREAD",
      "REST_CREATE_MESSAGE_THREAD_TEMPLATE",
      "REST_THREAD_SEARCH_PATH_D02",
      "ThreadChannelKind",
      "ThreadMetadata",
      "CreateThreadFromMessageRequest",
      "RestThreadRequestEnvelope",
      "ThreadLifecycleManager",
      "create_thread_from_message",
      "apply_thread_members_update",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1002a_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1002A-2 (модель ThreadMetadata, create_thread_from_message и THREAD_MEMBERS_UPDATE) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1002a_2_.*ok/);
  });

  it("SUB-1002A-3: обрабатывает таймеры автоархивации (60, 1440, 4320, 10080 мин), состояния жизненного цикла (active -> archiving -> archived) и защиту locked-тредов без MANAGE_THREADS", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "ALLOWED_AUTO_ARCHIVE_DURATIONS",
      "MIN_AUTO_ARCHIVE_MINUTES",
      "MAX_AUTO_ARCHIVE_MINUTES",
      "AutoArchiveDuration",
      "ThreadLifecycleState",
      "evaluate_auto_archive_timeouts",
      "remaining_auto_archive_minutes",
      "lifecycle_state_at",
      "set_thread_archived",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1002a_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1002A-3 (таймеры автоархивации 60..10080 мин и состояния жизненного цикла) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1002a_3_.*ok/);
  });

  it("SUB-1002A-4: покрывает все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 59/60..10079/10080/10081 мин, 100 символов имени, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, коды 400/401/403/404/413/429/5xx, skeleton/202, права, THREAD_LIST_SYNC и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "validate_workload_boundaries",
      "handle_rest_response_status",
      "apply_thread_list_sync",
      "redact_thread_secrets",
      "export_threads_telemetry_json",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1002a_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1002A-4 (9 измерений TEST-POLICY.md) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1002a_4_.*ok/);
  });

  it("SUB-1002A-5: убивает 5/5 контрольных мутаций модуля rust/crates/discord-core/src/threads.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(MODULE_PATH), "Исходник модуля должен существовать");
    const source = readFileSync(MODULE_PATH, "utf8");

    const mutationSpecs = [
      {
        id: "MUT-threads-1002a-01",
        operator: "PublicThreadChannelTypeCorrupted",
        from: "pub const CHANNEL_TYPE_PUBLIC_THREAD: u8 = 11;",
        to: "pub const CHANNEL_TYPE_PUBLIC_THREAD: u8 = 10;",
      },
      {
        id: "MUT-threads-1002a-02",
        operator: "AutoArchiveInactivityBoundaryRelaxed",
        from: "if elapsed_minutes >= duration_minutes {",
        to: "if elapsed_minutes > duration_minutes {",
      },
      {
        id: "MUT-threads-1002a-03",
        operator: "LockedThreadUnarchiveGuardBypassed",
        from: "if t.metadata.locked && !has_manage {",
        to: "if false && t.metadata.locked && !has_manage {",
      },
      {
        id: "MUT-threads-1002a-04",
        operator: "ThreadMembersUpdateDeduplicationBroken",
        from: "if !t.member_ids.iter().any(|existing| existing == uid) {",
        to: "if true {",
      },
      {
        id: "MUT-threads-1002a-05",
        operator: "PrivateThreadPermissionCheckBypassed",
        from: "let priv_ok = (permissions_bits & PERM_CREATE_PRIVATE_THREADS) != 0;",
        to: "let priv_ok = true;",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        source.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в исходнике: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "discord-core/src/threads.rs",
        operator: spec.operator,
        mutatedSource: source.replace(spec.from, spec.to),
      };
    });

    const gate = evaluateMutationGate(mutants, (mutant, idx) => {
      const mutantSrcPath = join(workDir, `mutant_${idx + 1}.rs`);
      const mutantBinPath = join(workDir, `mutant_${idx + 1}_bin`);
      writeFileSync(mutantSrcPath, mutant.mutatedSource, "utf8");

      const compiled = compileRustModuleTests(mutantSrcPath, mutantBinPath);
      if (compiled.status !== 0) {
        return { killed: true, reason: "compilation_error" };
      }
      const run = runCompiledRustTests(mutantBinPath);
      if (run.status !== 0) {
        return { killed: true, reason: "test_assertion_failed" };
      }
      return { killed: false, reason: "mutant_survived" };
    });

    assert.equal(
      gate.passed,
      true,
      `Мутационный гейт не пройден (выжили: ${gate.survivors.join(", ")})`,
    );
    assert.equal(gate.killed, 5);
    assert.equal(gate.total, 5);
    assert.equal(gate.killRate, 1.0);
  });
});
