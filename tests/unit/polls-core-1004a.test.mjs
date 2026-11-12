// @ts-check
/**
 * Контрактные и модульные тесты модели опроса `Poll`, REST-голосования
 * `PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}` (`PUT /polls/{id}/answers/{id}`)
 * и математики процентных шкал (TASK-1004A):
 * - SUB-1004A-0: Статический и компиляционный контракт `rust/crates/discord-core/src/polls.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, лимиты `scripts/quality-gate.mjs`:
 *   функции < 50 строк, модуль < 800 строк, `rustfmt --check --edition 2024`, компиляция
 *   `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1004A-1: Сверка карточки `TASK-1004A` в `waves/kanban-board.json`, дизайн-токенов, геометрии
 *   `Poll Engine` (`Backend Module`, `padding: 0px`, `spacing: 0px`), интерактивных состояний `poll-vote`
 *   (`ready -> voting -> voted`), REST-оракулов `/home/sanya/re-deep/rest-d02.json` (`docs/re/rest-d02.json`),
 *   `/home/sanya/re-deep/rest-static.json` (`POLL_ANSWERS`, `POLL_EXPIRE`, `POLL_ANSWER_VOTERS`),
 *   Gateway-оракула `MESSAGE_POLL_VOTE_ADD` (`4` события в `gateway-protocol.json` и `msgs-run2-95974_8269.jsonl`)
 *   и модулей RE-дампа (`412136`, `744593`, `505527`, `438437`, `335759`, `951727`, `969632`).
 * - SUB-1004A-2: Десериализация структуры `Poll` (`question`, `answers[]`, `expiry`, `allow_multiselect`,
 *   `layout_type`, `results`), `PollAnswer`, `PollMedia`, `PollResults`, валидация лимитов RE-модуля `412136`
 *   (вопрос `1..=300` символов, ответы `1..=10` по `1..=55` символов, длительности `[1, 4, 8, 24, 72, 168, 336]` ч)
 *   и применение живых кадров `MESSAGE_POLL_VOTE_ADD` / `MESSAGE_POLL_VOTE_REMOVE`.
 * - SUB-1004A-3: REST-голосование (`cast_poll_vote`, `remove_poll_vote`, `expire_poll_early`) через маршруты
 *   `PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}`, `PUT /polls/{id}/answers/{id}`,
 *   `DELETE /channels/{id}/polls/{msg_id}/answers/{answer_id}` и `PUT /channels/{id}/polls/{msg_id}/answers/@me`
 *   с валидацией дедлайна истечения и `allow_multiselect`, а также расчёт процентных шкал
 *   `(votes_i / sum(votes)) * 100%` с защитой от деления на ноль при `0` голосов и определением победителя (`is_victor`).
 * - SUB-1004A-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные Snowflake `0`/`u64::MAX`, `2000` символов, `10` файлов/`25 МБ`, `offset=9975`, громкость `0..200%`,
 *   бейдж `99+`, коды HTTP `400/401/403/404/413/429 retry_after_ms/5xx`, пустые `0` голосов, `skeleton`/`HTTP 202`,
 *   права `VIEW_CHANNEL`/`SEND_MESSAGES`/`SEND_POLLS (1 << 49)`/`AGE_GATED_SPACES`, откат оптимистичного голоса
 *   при сбое сети и маскирование секретов `mfa.***`).
 * - SUB-1004A-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`
 *   (деление на ноль при 0 голосов, приём голоса в истекший опрос, обход `allow_multiselect`, искажение округления
 *   процентной шкалы и пропуск декремента при отзыве голоса).
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
const MODULE_REL_PATH = "rust/crates/discord-core/src/polls.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const RE_REST_D02_PATH = "/home/sanya/re-deep/rest-d02.json";
const DOCS_REST_D02_JSON_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
const RE_GATEWAY_PROTO_PATH = "/home/sanya/re-deep/gateway-protocol.json";
const DOCS_GATEWAY_PROTO_PATH = join(
  ROOT,
  "docs",
  "re",
  "gateway-protocol.json",
);
const RE_GW_MSGS_8269_PATH =
  "/home/sanya/re-deep/gw-proto/msgs-run2-95974_8269.jsonl";
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";
const DISPATCH_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-core",
  "src",
  "dispatch.rs",
);

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/crates/discord-core/src/polls.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1004A: Десериализация Poll, эндпоинты PUT /polls/{id}/answers/{id} и математика процентных шкал (rust/crates/discord-core/src/polls.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "polls-core-1004a-"));
    testBinPath = join(workDir, "polls_core_1004a_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1004A-0: проверяет статический и компиляционный контракт rust/crates/discord-core/src/polls.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024, secret-scan и rustc --test -D warnings)", () => {
    assert.ok(
      existsSync(MODULE_PATH),
      `Отсутствует Rust-модуль ${MODULE_REL_PATH}`,
    );

    const rustSource = readFileSync(MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле polls.rs обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде polls.rs запрещены .unwrap() и .expect()",
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

  it("SUB-1004A-1: сверяет карточку TASK-1004A в waves/kanban-board.json, дизайн-токены, геометрию Poll Engine, состояния poll-vote и RE-оракулы (rest-d02.json, rest-static.json, gateway-protocol.json, msgs-run2-95974_8269.jsonl)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Оракул: карточка TASK-1004A в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1004A = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1004A",
    );
    assert.ok(
      card1004A,
      "Карточка TASK-1004A должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1004A.department, "G1 Backend");
    assert.equal(card1004A.priority, "P1");
    assert.deepEqual(card1004A.layout, {
      container: "Poll Engine",
      dimensions: "Backend Module",
      padding: "0px",
      spacing: "0px",
    });
    assert.deepEqual(card1004A.design_tokens, {
      background: "#1e1f22",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#5865f2",
      border: "#3f4147",
    });
    assert.deepEqual(card1004A.interactive_states[0], {
      element: "poll-vote",
      default: "ready",
      hover: "voting",
      active: "voted",
    });

    // 2. Оракул: re-deep/rest-d02.json (и локальное зеркало docs/re/rest-d02.json)
    const restD02OraclePath = existsSync(RE_REST_D02_PATH)
      ? RE_REST_D02_PATH
      : DOCS_REST_D02_JSON_PATH;
    const restD02Raw = readFileSync(restD02OraclePath, "utf8");
    const restD02 = JSON.parse(restD02Raw);
    assert.equal(restD02.snapshotAt, "2026-10-19T11:51:47+05:00");
    assert.equal(restD02.totals.apiRequests, 133);
    assert.equal(restD02.totals.apiUniqueEndpoints, 47);
    const reactionPutEndpoint = restD02.endpoints.find(
      (/** @type {any} */ ep) =>
        ep.method === "PUT" &&
        ep.path ===
          "/api/v9/channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me",
    );
    assert.ok(
      reactionPutEndpoint,
      "В rest-d02.json должен быть зафиксирован PUT маршрут реакций/голосов со статусом 204",
    );
    assert.deepEqual(reactionPutEndpoint.statuses, [204]);
    assert.ok(reactionPutEndpoint.requestIds.includes("run2|95974.8046"));

    // 3. Оракул: /home/sanya/re-deep/rest-static.json (POLL_ANSWERS, POLL_EXPIRE, POLL_ANSWER_VOTERS)
    if (existsSync(RE_REST_STATIC_PATH)) {
      const restStaticText = readFileSync(RE_REST_STATIC_PATH, "utf8");
      assert.match(restStaticText, /"\/channels\/\{arg\}\/polls\/\{arg\}\/answers\/@me"/);
      assert.match(restStaticText, /"POLL_ANSWERS"/);
      assert.match(restStaticText, /"\{answer_ids\}"/);
      assert.match(restStaticText, /"\/channels\/\{arg\}\/polls\/\{arg\}\/expire"/);
      assert.match(restStaticText, /"POLL_EXPIRE"/);
      assert.match(restStaticText, /"\/channels\/\{arg\}\/polls\/\{arg\}\/answers\/\{arg\}"/);
      assert.match(restStaticText, /"POLL_ANSWER_VOTERS"/);
    }

    // 4. Оракул: gateway-protocol.json и живой захват msgs-run2-95974_8269.jsonl (MESSAGE_POLL_VOTE_ADD = 4)
    const gwProtoPath = existsSync(RE_GATEWAY_PROTO_PATH)
      ? RE_GATEWAY_PROTO_PATH
      : DOCS_GATEWAY_PROTO_PATH;
    const gwProto = JSON.parse(readFileSync(gwProtoPath, "utf8"));
    assert.equal(gwProto.opcodes.dispatchTypes.MESSAGE_POLL_VOTE_ADD, 4);

    if (existsSync(RE_GW_MSGS_8269_PATH)) {
      const lines = readFileSync(RE_GW_MSGS_8269_PATH, "utf8")
        .split("\n")
        .filter((l) => l.includes("MESSAGE_POLL_VOTE_ADD"))
        .map((l) => JSON.parse(l));
      assert.equal(lines.length, 4);
      assert.equal(lines[0].term.d.guild_id, "942355007271997450");
      assert.equal(lines[0].term.d.channel_id, "1294815650186788916");
      assert.equal(lines[0].term.d.message_id, "1558237752917758108");
      assert.equal(lines[0].term.d.answer_id, 8);
      assert.equal(lines[1].term.d.answer_id, 1);
      assert.equal(lines[3].term.d.answer_id, 7);
    }

    if (existsSync(RE_MODULES_INDEX_PATH)) {
      const modIdx = JSON.parse(readFileSync(RE_MODULES_INDEX_PATH, "utf8"));
      assert.ok(modIdx["412136"].strings.includes("POLL_CREATION_MODAL_KEY"));
      assert.ok(modIdx["789646"].strings.includes("POLL_ANSWERS"));
      assert.ok(modIdx["969632"].strings.includes("poll_question_text"));
    }

    const dispatchSrc = readFileSync(DISPATCH_RS_PATH, "utf8");
    assert.match(
      dispatchSrc,
      /Self::MessagePollVoteAdd => "MESSAGE_POLL_VOTE_ADD"/,
    );

    // 5. Сверка исходника polls.rs с дизайн-токенами и константами оракула
    const rustSource = readFileSync(MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1004A.design_tokens)) {
      assert.ok(
        rustSource.includes(tokenVal),
        `Дизайн-токен ${tokenVal} карточки TASK-1004A должен присутствовать в polls.rs`,
      );
    }
    assert.ok(rustSource.includes("Poll Engine"));
    assert.ok(rustSource.includes("Backend Module"));
    assert.ok(rustSource.includes("poll-vote"));

    const run = runCompiledRustTests(testBinPath, "test_sub_1004a_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1004A-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1004a_1_.*ok/);
  });

  it("SUB-1004A-2: проверяет Struct Poll/PollAnswer/PollMedia/PollResults, десериализацию объекта poll (question, answers[], expiry, allow_multiselect, results) и обработку MESSAGE_POLL_VOTE_ADD", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(MODULE_PATH, "utf8");
    for (const symbol of [
      "Poll",
      "PollAnswer",
      "PollMedia",
      "PollEmoji",
      "PollResults",
      "PollAnswerCount",
      "PollLayoutType",
      "PollVoteState",
      "PollVoteGatewayEvent",
      "deserialize_poll_json",
      "apply_gateway_poll_vote_add",
      "apply_gateway_poll_vote_remove",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В polls.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1004a_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1004A-2 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1004a_2_.*ok/);
  });

  it("SUB-1004A-3: проверяет REST голосование (cast_poll_vote, remove_poll_vote, PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}, PUT /polls/{id}/answers/{id}), валидацию expiry и allow_multiselect, а также математику процентных шкал с защитой от деления на ноль", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(MODULE_PATH, "utf8");
    for (const symbol of [
      "REST_POLL_ANSWER_VOTE_TEMPLATE",
      "REST_POLL_SHORT_VOTE_TEMPLATE",
      "REST_POLL_ANSWERS_ME_TEMPLATE",
      "REST_POLL_EXPIRE_TEMPLATE",
      "RestPollVoteRequest",
      "PollAnswerPercentage",
      "cast_poll_vote",
      "remove_poll_vote",
      "expire_poll_early",
      "calculate_vote_percentage_f64",
      "calculate_vote_percentage_rounded",
      "compute_poll_percentages",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В polls.rs должен присутствовать символ ${symbol}`,
      );
    }
    assert.ok(
      rustSource.includes("/channels/{id}/polls/{msg_id}/answers/{answer_id}"),
      "Шаблон PUT /channels/{id}/polls/{msg_id}/answers/{answer_id} должен присутствовать в polls.rs",
    );
    assert.ok(
      rustSource.includes("/polls/{id}/answers/{id}"),
      "Короткий шаблон PUT /polls/{id}/answers/{id} должен присутствовать в polls.rs",
    );

    const run = runCompiledRustTests(testBinPath, "test_sub_1004a_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1004A-3 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1004a_3_.*ok/);
  });

  it("SUB-1004A-4: проверяет все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 300/55 символов, 1..10 ответов, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, коды 400..5xx, skeleton/202, права SEND_POLLS/AGE_GATED_SPACES, откат голоса и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(MODULE_PATH, "utf8");
    for (const symbol of [
      "PERM_SEND_POLLS",
      "validate_poll_workload_boundaries",
      "classify_poll_http_status",
      "format_poll_votes_badge",
      "rollback_optimistic_vote",
      "redact_poll_log",
      "export_poll_telemetry_json",
    ]) {
      assert.match(
        rustSource,
        new RegExp(`\\b${symbol}\\b`),
        `В polls.rs должен присутствовать символ ${symbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1004a_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1004A-4 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1004a_4_.*ok/);
  });

  it("SUB-1004A-5: убивает 5/5 контрольных мутаций в rust/crates/discord-core/src/polls.rs (100% mutation kill rate: деление на 0 при 0 голосов, прием голоса в истекший опрос, обход allow_multiselect, искажение округления процентов, пропуск декремента при remove_poll_vote)", () => {
    assert.ok(existsSync(MODULE_PATH), "Модуль polls.rs должен существовать");
    const originalRust = readFileSync(MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutations = [
      {
        id: "MUT-1004A-RS-01",
        moduleName: "polls",
        operator: "ZeroVotesDivisionGuardTamper",
        from: "if total_votes == 0 {",
        to: "if false && total_votes == 0 {",
      },
      {
        id: "MUT-1004A-RS-02",
        moduleName: "polls",
        operator: "ExpiredPollVoteAcceptBypass",
        from: "if self.is_expired_at(now_ms) {",
        to: "if false && self.is_expired_at(now_ms) {",
      },
      {
        id: "MUT-1004A-RS-03",
        moduleName: "polls",
        operator: "AllowMultiselectConstraintBypass",
        from: "if !self.allow_multiselect && unique_ids.len() > 1 {",
        to: "if false && !self.allow_multiselect && unique_ids.len() > 1 {",
      },
      {
        id: "MUT-1004A-RS-04",
        moduleName: "polls",
        operator: "PercentageRoundingFormulaTamper",
        from: "let pct = ((votes as f64 / total_votes as f64) * 100.0).round();",
        to: "let pct = ((votes as f64 / total_votes as f64) * 10.0).floor();",
      },
      {
        id: "MUT-1004A-RS-05",
        moduleName: "polls",
        operator: "RemovePollVoteDecrementOmission",
        from: "entry.count = entry.count.saturating_sub(1);",
        to: "entry.count = entry.count;",
      },
    ];

    const mutants = rustMutations.map((m) => {
      assert.ok(
        originalRust.includes(m.from),
        `Целевая строка для ${m.id} должна присутствовать в polls.rs: ${m.from}`,
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
      "MD5 исходного файла polls.rs не должен измениться после мутационного гейта",
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
