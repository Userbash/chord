// @ts-check
/**
 * Контрактные и модульные тесты парсера `GUILD_FORUM` (`type: 15`) и REST-пагинации
 * активных постов `GET /channels/{id}/threads/active` (TASK-1001A):
 * - SUB-1001A-0: Статический и компиляционный контракт `rust/crates/discord-core/src/forum.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, файл < 800 строк, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений и прогон встроенных Rust-тестов).
 * - SUB-1001A-1: Сверка с независимыми оракулами репозитория (`waves/kanban-board.json` карточка `TASK-1001A`,
 *   `docs/re/gateway-schema.json` и `/home/sanya/re-deep/gateway-schema.json`, `docs/re/gateway-schema.md`,
 *   `docs/re/gateway-protocol.json` и `rust/gui-spike/src/forum.rs`).
 * - SUB-1001A-2: Парсер `CHANNEL_CREATE` / `GUILD_CREATE` типа `15` (`GUILD_FORUM`), структуры `ForumTag`
 *   (десериализация `available_tags` с флагом `moderated`), `ForumLayout`, `ForumSortOrder` (`default_sort_order`),
 *   валидация прав `MANAGE_THREADS` для модераторских тегов и состояния `forum-parser` (`idle -> active -> parsed`).
 * - SUB-1001A-3: REST-клиент `ForumRestClient` и пагинация активных постов через `GET /channels/{id}/threads/active`
 *   по Snowflake-курсорам без зацикливания, отбраковка архивных веток и сортировка с приоритетом `pinned`.
 * - SUB-1001A-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные Snowflake `0`/`u64::MAX`, `2000` символов, `10` файлов/`25 МБ`, `offset=9975`, громкость `0..200%`,
 *   бейдж `99+`, HTTP `400/401/403/404/413/429/5xx`, пустые, `skeleton`/`HTTP 202`, права и `AGE_GATED_SPACES`,
 *   восстановление при цикле пагинации и маскирование секретов).
 * - SUB-1001A-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/crates/discord-core/src/forum.rs`.
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
const RUST_MODULE_REL_PATH = "rust/crates/discord-core/src/forum.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const GATEWAY_SCHEMA_JSON_PATH = join(ROOT, "docs", "re", "gateway-schema.json");
const RE_DEEP_GATEWAY_SCHEMA_PATH = "/home/sanya/re-deep/gateway-schema.json";
const GATEWAY_SCHEMA_MD_PATH = join(ROOT, "docs", "re", "gateway-schema.md");
const GATEWAY_PROTOCOL_JSON_PATH = join(
  ROOT,
  "docs",
  "re",
  "gateway-protocol.json",
);
const SPIKE_FORUM_RS_PATH = join(ROOT, "rust", "gui-spike", "src", "forum.rs");

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/crates/discord-core/src/forum.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1001A: Парсер Gateway type: 15 (GUILD_FORUM) и REST GET /channels/{id}/threads/active (rust/crates/discord-core/src/forum.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "forum-protocol-1001a-"));
    testBinPath = join(workDir, "forum_protocol_1001a_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1001A-0: проверяет статический контракт rust/crates/discord-core/src/forum.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024 и rustc --test -D warnings)", () => {
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
      "В продуктовом коде forum.rs запрещены .unwrap() и .expect()",
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

  it("SUB-1001A-1: сверяет карточку TASK-1001A в waves/kanban-board.json, дизайн-токены, состояния forum-parser и схему канала type 15 с независимыми оракулами (gateway-schema.json, gateway-schema.md, gateway-protocol.json)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Оракул: карточка TASK-1001A в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1001A = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1001A",
    );
    assert.ok(
      card1001A,
      "Карточка TASK-1001A должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1001A.department, "G1 Backend");
    assert.deepEqual(card1001A.layout, {
      container: "Core Protocol",
      dimensions: "Data Struct",
      padding: "0px",
      spacing: "0px",
    });
    assert.deepEqual(card1001A.design_tokens, {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#5865f2",
      border: "#3f4147",
    });
    assert.deepEqual(card1001A.interactive_states[0], {
      element: "forum-parser",
      default: "idle",
      hover: "active",
      active: "parsed",
    });

    // 2. Оракул: docs/re/gateway-schema.json и /home/sanya/re-deep/gateway-schema.json
    const gwSchemaJson = JSON.parse(
      readFileSync(GATEWAY_SCHEMA_JSON_PATH, "utf8"),
    );
    const tagPaths0x6a = gwSchemaJson.encoding.tagPaths["tag tag=0x6a"];
    assert.ok(
      tagPaths0x6a.some(
        (/** @type {any} */ entry) =>
          entry[0] === "guilds[].channels[].available_tags" && entry[1] === 67,
      ),
      "В docs/re/gateway-schema.json должен быть зафиксирован путь guilds[].channels[].available_tags (67)",
    );
    if (existsSync(RE_DEEP_GATEWAY_SCHEMA_PATH)) {
      const reDeepSchema = JSON.parse(
        readFileSync(RE_DEEP_GATEWAY_SCHEMA_PATH, "utf8"),
      );
      const reTagPaths = reDeepSchema.encoding.tagPaths["tag tag=0x6a"];
      assert.ok(
        reTagPaths.some(
          (/** @type {any} */ entry) =>
            entry[0] === "guilds[].channels[].available_tags" &&
            entry[1] === 67,
        ),
      );
    }

    // 3. Оракул: docs/re/gateway-schema.md
    const gwSchemaMd = readFileSync(GATEWAY_SCHEMA_MD_PATH, "utf8");
    assert.match(
      gwSchemaMd,
      /\|\s*`available_tags`\s*\|\s*array\\\|null\s*\|\s*nil\/list\s*\|\s*68\/272\s*\|/,
    );
    assert.match(
      gwSchemaMd,
      /\|\s*`default_reaction_emoji`\s*\|\s*object\\\|null\s*\|\s*str\/map\s*\|\s*68\/272\s*\|/,
    );
    assert.match(
      gwSchemaMd,
      /\|\s*`default_sort_order`\s*\|\s*null\s*\|\s*str\s*\|\s*68\/272\s*\|/,
    );
    assert.match(
      gwSchemaMd,
      /\|\s*`default_forum_layout`\s*\|\s*int\s*\|\s*int\s*\|\s*1\/272\s*\|\s*2\s*\|/,
    );
    assert.match(
      gwSchemaMd,
      /\|\s*`default_auto_archive_duration`\s*\|\s*int\s*\|\s*int\s*\|\s*6\/272\s*\|\s*10080\s*\|/,
    );

    // 4. Оракул: docs/re/gateway-protocol.json (9 каналов type 15 в READY.guilds[])
    const gwProto = JSON.parse(
      readFileSync(GATEWAY_PROTOCOL_JSON_PATH, "utf8"),
    );
    const totalType15Channels = gwProto.ready.guildList.reduce(
      (/** @type {number} */ acc, /** @type {any} */ g) =>
        acc + (Number(g.channelTypes?.["15"]) || 0),
      0,
    );
    assert.equal(totalType15Channels, 9);

    // 5. Оракул: rust/gui-spike/src/forum.rs
    const spikeForumRs = readFileSync(SPIKE_FORUM_RS_PATH, "utf8");
    assert.match(spikeForumRs, /pub const MAX_FORUM_APPLIED_TAGS: usize = 5;/);
    assert.match(spikeForumRs, /pub const MAX_FORUM_TITLE_CHARS: usize = 100;/);
    assert.match(
      spikeForumRs,
      /pub const MAX_FORUM_CONTENT_CHARS: usize = 2000;/,
    );

    // 6. Сверка исходника rust/crates/discord-core/src/forum.rs
    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1001A.design_tokens)) {
      assert.ok(
        rustSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} карточки TASK-1001A должен присутствовать в forum.rs`,
      );
    }
    assert.ok(rustSource.includes("Core Protocol"));
    assert.ok(rustSource.includes("Data Struct"));
    assert.ok(rustSource.includes("forum-parser"));

    const run = runCompiledRustTests(testBinPath, "test_sub_1001a_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1001A-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1001a_1_.*ok/);
  });

  it("SUB-1001A-2: проверяет Struct ForumTag/ForumLayout/ForumSortOrder, десериализацию available_tags с флагом moderated и default_sort_order из CHANNEL_CREATE (type 15)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "CHANNEL_TYPE_GUILD_FORUM",
      "ForumTag",
      "ForumLayout",
      "ForumSortOrder",
      "ForumParserState",
      "ForumChannelSpec",
      "normalize_available_tags",
      "parse_gateway_forum_channel",
      "validate_forum_tag_selection",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В forum.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1001a_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1001A-2 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1001a_2_.*ok/);
  });

  it("SUB-1001A-3: проверяет ForumRestClient и пагинацию активных постов через REST GET /channels/{id}/threads/active по Snowflake без зацикливания", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "REST_ACTIVE_THREADS_ROUTE_TEMPLATE",
      "ForumActiveThread",
      "ActiveThreadsPageResponse",
      "RestActiveThreadsRequest",
      "ForumRestClient",
      "build_active_threads_request",
      "ingest_active_threads_page",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В forum.rs должен присутствовать символ ${symbol}`,
      );
    }
    assert.ok(
      rustSource.includes("/channels/{id}/threads/active"),
      "Шаблон маршрута GET /channels/{id}/threads/active должен быть в forum.rs",
    );

    const run = runCompiledRustTests(testBinPath, "test_sub_1001a_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1001A-3 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1001a_3_.*ok/);
  });

  it("SUB-1001A-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, HTTP 400..5xx, skeleton/202, права MANAGE_THREADS/AGE_GATED_SPACES, защита от цикла и редактирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const symbol of [
      "validate_forum_protocol_boundaries",
      "classify_forum_http_status",
      "format_reply_count_badge",
      "redact_forum_protocol_log",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В forum.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1001a_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1001A-4 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1001a_4_.*ok/);
  });

  it("SUB-1001A-5: убивает 5/5 контрольных мутаций в rust/crates/discord-core/src/forum.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(RUST_MODULE_PATH), "Модуль forum.rs должен существовать");
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1001A-RS-01",
        moduleName: "forum",
        operator: "ChannelTypeGuildForumTamper",
        from: "pub const CHANNEL_TYPE_GUILD_FORUM: u8 = 15;",
        to: "pub const CHANNEL_TYPE_GUILD_FORUM: u8 = 0;",
      },
      {
        id: "MUT-1001A-RS-02",
        moduleName: "forum",
        operator: "ModeratedFlagDropInNormalizer",
        from: "moderated: tag.moderated,",
        to: "moderated: false,",
      },
      {
        id: "MUT-1001A-RS-03",
        moduleName: "forum",
        operator: "DefaultSortOrderMappingTamper",
        from: "Some(1) => Ok(Self::CreationDate),",
        to: "Some(1) => Ok(Self::LatestActivity),",
      },
      {
        id: "MUT-1001A-RS-04",
        moduleName: "forum",
        operator: "PaginationLoopGuardBypass",
        from: "if self.seen_cursors.contains(&cursor) {",
        to: "if false && self.seen_cursors.contains(&cursor) {",
      },
      {
        id: "MUT-1001A-RS-05",
        moduleName: "forum",
        operator: "ModeratedTagManageThreadsBypass",
        from: "if tag.moderated && !can_manage_threads {",
        to: "if false && tag.moderated && !can_manage_threads {",
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в forum.rs: ${m.from}`,
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
      "MD5 исходного файла forum.rs не должен измениться после мутационного гейта",
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
