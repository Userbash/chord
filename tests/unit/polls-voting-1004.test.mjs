// @ts-check
/**
 * Контрактные и модульные тесты интерактивных опросов в сообщениях (Polls & Voting — TASK-1004):
 * - SUB-1004-0: Статический и компиляционный контракт `rust/chord-gpui/src/chat/polls.rs`
 *   и `scripts/verify-polls.mjs` (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде,
 *   соблюдение лимитов `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк,
 *   чистый `rustfmt --check --edition 2024`, компиляция `rustc --test --edition 2024 -D warnings`
 *   без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1004-1: Сверка карточки `TASK-1004` (и `TASK-1004A`/`TASK-1004B`) в `waves/kanban-board.json`,
 *   дизайн-токенов (`#2b2d31`, `#1e1f22`, `#ffffff`, `#5865f2`, `#3f4147`, заливка `#5865f233`),
 *   геометрии `Poll Card` (`100% x auto`, `padding: 12px`, `spacing: 8px`), интерактивных состояний
 *   `poll-option` (`radio` -> `hover bg` -> `voted`), события шлюза `MESSAGE_POLL_VOTE_ADD` (`4` в дампе
 *   `docs/re/gateway-protocol.json`), маршрутов REST (`PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}`,
 *   `PUT /channels/{id}/polls/{msg_id}/answers/@me`, `POST /channels/{id}/polls/{msg_id}/expire`,
 *   `GET /channels/{id}/polls/{msg_id}/answers/{answer_id}`) и модулей RE-дампа (`290794`, `412136`,
 *   `789646`, `969632`) с независимыми оракулами.
 * - SUB-1004-2: Одиночный (`allow_multiselect = false`, radio) и множественный (`allow_multiselect = true`,
 *   checkbox до `10` вариантов) выбор ответов, валидация дедлайнов истечения (`1, 4, 8, 24, 72, 168, 336` ч,
 *   русские подписи обратного отсчёта) и формирование REST-запросов голосования и досрочного завершения.
 * - SUB-1004-3: Расчёт процентных индикаторов в реальном времени с защитой от деления на ноль (`0%` при `0` голосов,
 *   `round((count * 100) / totalVotes)`), анимированная шкала `#5865f233`, определение лидирующего и победившего
 *   варианта по завершении опроса, обработка live-событий Gateway `MESSAGE_POLL_VOTE_ADD` / `MESSAGE_POLL_VOTE_REMOVE`
 *   на 4 реальных кадрах из `/home/sanya/re-deep/gw-proto/msgs-run2-95974_8269.jsonl` (`s=114, 168, 170, 473`).
 * - SUB-1004-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные, граничные:
 *   Snowflake `0`/`u64::MAX`, вопрос `1..300` vs `301` симв., ответ `1..55` vs `56` симв., `1..10` ответов,
 *   длительность `1..336` ч, 2000 символов сообщения, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`,
 *   бейдж `99+`; ошибки `400/401/403/404/413/429/5xx`; пустые `0` голосов; `skeleton`/`HTTP 202`;
 *   права `VIEW_CHANNEL`/`SEND_MESSAGES`/`SEND_POLLS = 1 << 49`/`AGE_GATED_SPACES`; откат оптимистичного
 *   голоса при сетевом сбое; маскирование секретов) и проверка CLI `scripts/verify-polls.mjs` (коды `0 / 1 / 2`).
 * - SUB-1004-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/chord-gpui/src/chat/polls.rs`
 *   и `scripts/verify-polls.mjs` по контракту `scripts/quality-gate.mjs`.
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
  computePollResults as spikeComputePollResults,
  formatPollExpiryRu as spikeFormatPollExpiryRu,
  togglePollAnswerSelection as spikeTogglePollAnswerSelection,
} from "../../rust/gui-spike/ui/features/poll-progress.js";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const RUST_MODULE_REL_PATH = "rust/chord-gpui/src/chat/polls.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const SCRIPT_REL_PATH = "scripts/verify-polls.mjs";
const SCRIPT_PATH = join(ROOT, SCRIPT_REL_PATH);

const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const GATEWAY_PROTOCOL_JSON_PATH = join(ROOT, "docs", "re", "gateway-protocol.json");
const REST_D02_JSON_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/chord-gpui/src/chat/polls.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1004: Интерактивные опросы в сообщениях (Polls & Voting — rust/chord-gpui/src/chat/polls.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "polls-voting-1004-"));
    testBinPath = join(workDir, "polls_voting_1004_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1004-0: модули rust/chord-gpui/src/chat/polls.rs и scripts/verify-polls.mjs существуют, соблюдают лимиты (<50 строк на fn, <800 на файл), запрещают unsafe и компилируются без предупреждений", () => {
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
      "В продуктовом коде polls.rs запрещены .unwrap() и .expect()",
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
      `Встроенные тесты polls.rs должны проходить:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test result: ok\./);
  });

  it("SUB-1004-1: сверяет дизайн-токены, раскладку Poll Card, состояния poll-option, эндпоинты REST и события Gateway с независимыми оракулами", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      POLL_CARD_LAYOUT,
      POLL_DESIGN_TOKENS,
      POLL_OPTION_STATES,
      POLL_PROTOCOL_CONSTANTS,
      LIVE_POLL_VOTE_EVENTS_ORACLE,
      resolvePollOptionState,
      verifyPollsOracleParity,
    } = await import("../../scripts/verify-polls.mjs");

    // 1. Оракул: карточка TASK-1004 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1004 = kanban.cards.find((/** @type {any} */ c) => c.id === "TASK-1004");
    assert.ok(card1004, "Карточка TASK-1004 должна присутствовать в waves/kanban-board.json");
    assert.deepEqual(POLL_CARD_LAYOUT, card1004.layout);
    assert.deepEqual(POLL_DESIGN_TOKENS, card1004.design_tokens);
    assert.deepEqual(POLL_OPTION_STATES, card1004.interactive_states[0]);

    // 2. Интерактивные состояния poll-option: radio -> hover bg -> voted
    assert.deepEqual(resolvePollOptionState({ voted: false, hovered: false }), {
      element: "poll-option",
      state: "default",
      label: "radio",
    });
    assert.deepEqual(resolvePollOptionState({ voted: false, hovered: true }), {
      element: "poll-option",
      state: "hover",
      label: "hover bg",
    });
    assert.deepEqual(resolvePollOptionState({ voted: true, hovered: false }), {
      element: "poll-option",
      state: "active",
      label: "voted",
    });

    // 3. Оракул docs/re/gateway-protocol.json (MESSAGE_POLL_VOTE_ADD = 4)
    const gwProto = JSON.parse(readFileSync(GATEWAY_PROTOCOL_JSON_PATH, "utf8"));
    assert.equal(gwProto.opcodes.dispatchTypes.MESSAGE_POLL_VOTE_ADD, 4);
    assert.equal(LIVE_POLL_VOTE_EVENTS_ORACLE.length, 4);

    // 4. Константы протокола опросов из модуля 412136 и 969632 RE-дампа
    assert.equal(POLL_PROTOCOL_CONSTANTS.MIN_ANSWERS, 1);
    assert.equal(POLL_PROTOCOL_CONSTANTS.MAX_ANSWERS, 10);
    assert.equal(POLL_PROTOCOL_CONSTANTS.MAX_QUESTION_CHARS, 300);
    assert.equal(POLL_PROTOCOL_CONSTANTS.MAX_ANSWER_CHARS, 55);
    assert.equal(POLL_PROTOCOL_CONSTANTS.MAX_DURATION_HOURS, 336);
    assert.equal(POLL_PROTOCOL_CONSTANTS.PROGRESS_FILL_HEX, "#5865f233");
    assert.deepEqual(POLL_PROTOCOL_CONSTANTS.DURATION_PRESETS_HOURS, [1, 4, 8, 24, 72, 168, 336]);

    // 5. Полная сверка паритета с оракулами
    const parity = verifyPollsOracleParity({
      kanbanPath: KANBAN_BOARD_PATH,
      gatewayProtocolPath: GATEWAY_PROTOCOL_JSON_PATH,
      restD02Path: REST_D02_JSON_PATH,
      reRestStaticPath: existsSync(RE_REST_STATIC_PATH) ? RE_REST_STATIC_PATH : undefined,
      reModulesIndexPath: existsSync(RE_MODULES_INDEX_PATH) ? RE_MODULES_INDEX_PATH : undefined,
    });
    assert.equal(parity.passed, true, `Нарушение паритета с оракулами: ${parity.reasons.join("; ")}`);
    assert.equal(parity.gatewayVoteAddCount, 4);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1004_1_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1004-1 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1004_1_.*ok/);
  });

  it("SUB-1004-2: проверяет одиночный и множественный выбор ответов, дедлайны истечения и маршруты голосования PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      togglePollSelection,
      formatPollExpiryCountdownRu,
      isPollExpired,
      buildPollVoteRequest,
      buildPollBatchAnswersRequest,
      buildPollExpireRequest,
      buildPollAnswerVotersRequest,
    } = await import("../../scripts/verify-polls.mjs");

    // 1. Сверка переключения выбора (single-choice radio vs multi-choice checkbox) с эталоном poll-progress.js
    let singleSel = /** @type {number[]} */ ([]);
    singleSel = togglePollSelection(singleSel, 1, false, 10);
    assert.deepEqual(singleSel, [1]);
    singleSel = togglePollSelection(singleSel, 2, false, 10);
    assert.deepEqual(singleSel, [2]);
    singleSel = togglePollSelection(singleSel, 2, false, 10);
    assert.deepEqual(singleSel, []);
    assert.deepEqual(
      spikeTogglePollAnswerSelection(["1"], "2", false, 10),
      ["2"],
    );

    let multiSel = /** @type {number[]} */ ([]);
    multiSel = togglePollSelection(multiSel, 1, true, 3);
    multiSel = togglePollSelection(multiSel, 2, true, 3);
    multiSel = togglePollSelection(multiSel, 3, true, 3);
    multiSel = togglePollSelection(multiSel, 4, true, 3); // на лимите maxChoices=3 не добавляется
    assert.deepEqual(multiSel, [1, 2, 3]);
    multiSel = togglePollSelection(multiSel, 2, true, 3); // повторный клик снимает выбор
    assert.deepEqual(multiSel, [1, 3]);

    // 2. Проверка дедлайнов истечения и русских строк обратного отсчёта (в полном соответствии с poll-progress.js)
    const nowMs = 1_762_000_000_000;
    assert.equal(isPollExpired(nowMs - 1, nowMs), true);
    assert.equal(isPollExpired(nowMs, nowMs), true);
    assert.equal(isPollExpired(nowMs + 1, nowMs), false);

    for (const deltaMs of [-5_000, 0, 30_000, 15 * 60_000, 5 * 3_600_000, 3 * 86_400_000]) {
      const expMs = nowMs + deltaMs;
      assert.equal(
        formatPollExpiryCountdownRu(expMs, nowMs),
        spikeFormatPollExpiryRu(expMs, nowMs),
      );
    }

    // 3. Проверка формирования REST-запроса PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}
    const putReq = buildPollVoteRequest({
      channelId: "1294815650186788916",
      messageId: "1558237752917758108",
      answerId: 8,
      action: "add",
    });
    assert.equal(putReq.method, "PUT");
    assert.equal(
      putReq.path,
      "/channels/1294815650186788916/polls/1558237752917758108/answers/8",
    );
    assert.equal(
      putReq.apiV9Path,
      "/api/v9/channels/1294815650186788916/polls/1558237752917758108/answers/8",
    );

    const delReq = buildPollVoteRequest({
      channelId: "1294815650186788916",
      messageId: "1558237752917758108",
      answerId: 8,
      action: "remove",
    });
    assert.equal(delReq.method, "DELETE");
    assert.equal(
      delReq.path,
      "/channels/1294815650186788916/polls/1558237752917758108/answers/8",
    );

    // 4. Пакетное голосование PUT /channels/{id}/polls/{msg_id}/answers/@me и досрочное завершение POST /expire
    const batchReq = buildPollBatchAnswersRequest({
      channelId: "1294815650186788916",
      messageId: "1558237967334772848",
      answerIds: [1, 7],
    });
    assert.equal(batchReq.method, "PUT");
    assert.equal(
      batchReq.path,
      "/channels/1294815650186788916/polls/1558237967334772848/answers/@me",
    );
    assert.deepEqual(batchReq.body, { answer_ids: ["1", "7"] });

    const expireReq = buildPollExpireRequest({
      channelId: "1294815650186788916",
      messageId: "1558237967334772848",
    });
    assert.equal(expireReq.method, "POST");
    assert.equal(
      expireReq.path,
      "/channels/1294815650186788916/polls/1558237967334772848/expire",
    );

    const votersReq = buildPollAnswerVotersRequest({
      channelId: "1294815650186788916",
      messageId: "1558237967334772848",
      answerId: 7,
      limit: 25,
      after: "479659568322969611",
    });
    assert.equal(votersReq.method, "GET");
    assert.equal(
      votersReq.path,
      "/channels/1294815650186788916/polls/1558237967334772848/answers/7?limit=25&after=479659568322969611",
    );

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1004_2_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1004-2 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1004_2_.*ok/);
  });

  it("SUB-1004-3: вычисляет процентные индикаторы и счетчики голосов в реальном времени и воспроизводит 4 живых события MESSAGE_POLL_VOTE_ADD из RE-дампа", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      computePollPercentages,
      InteractivePollEngine,
      LIVE_POLL_VOTE_EVENTS_ORACLE,
    } = await import("../../scripts/verify-polls.mjs");

    // 1. Защита от деления на ноль при 0 голосов
    const zeroRes = computePollPercentages(
      [
        { answerId: 1, text: "Rust + GPUI" },
        { answerId: 2, text: "Electron" },
      ],
      [],
      false,
    );
    assert.equal(zeroRes.totalVotes, 0);
    assert.equal(zeroRes.items[0]?.percentage, 0);
    assert.equal(zeroRes.items[0]?.isLeading, false);
    assert.equal(zeroRes.items[0]?.isWinner, false);

    // 2. Сверка процентных индикаторов с независимым оракулом spikeComputePollResults
    const answers = [
      { answerId: 1, text: "GPUI 120 FPS" },
      { answerId: 7, text: "Wayland Native" },
      { answerId: 8, text: "Zero-Copy Voice" },
    ];
    const counts = [
      { id: 1, count: 3, meVoted: true },
      { id: 7, count: 1, meVoted: false },
      { id: 8, count: 2, meVoted: false },
    ];
    const calc = computePollPercentages(answers, counts, true);
    const spikeCalc = spikeComputePollResults(
      answers.map((a) => ({ answerId: String(a.answerId), text: a.text })),
      counts.map((c) => ({ id: String(c.id), count: c.count, meVoted: c.meVoted })),
    );
    assert.equal(calc.totalVotes, spikeCalc.totalVotes);
    assert.equal(calc.totalVotes, 6);
    assert.deepEqual(
      calc.items.map((i) => i.percentage),
      spikeCalc.items.map((i) => i.percentage),
    );
    assert.deepEqual(
      calc.items.map((i) => i.percentage),
      [50, 17, 33],
    );
    assert.equal(calc.items[0]?.isLeading, true);
    assert.equal(calc.items[0]?.isWinner, true);
    assert.equal(calc.items[0]?.fillWidthPercent, 50);
    assert.equal(calc.items[0]?.fillColorHex, "#5865f233");

    // 3. Воспроизведение 4 реальных событий MESSAGE_POLL_VOTE_ADD из /home/sanya/re-deep/gw-proto/msgs-run2-95974_8269.jsonl
    const engine = new InteractivePollEngine({
      channelId: "1294815650186788916",
      messageId: "1558237752917758108",
      guildId: "942355007271997450",
      currentUserId: "479659568322969611",
      question: "Какой модуль тестируем в первую очередь?",
      answers: [
        { answerId: 1, text: "Gateway ETF + Zstd" },
        { answerId: 7, text: "Voice DAVE MLS" },
        { answerId: 8, text: "Interactive Polls" },
      ],
      allowMultiselect: false,
      expiryMs: 1_800_000_000_000,
    });

    for (const ev of LIVE_POLL_VOTE_EVENTS_ORACLE) {
      engine.applyGatewayVoteEvent(ev);
    }

    // Для сообщения 1558237752917758108 пришли 2 голоса за answer_id=8 (user 606781270382346250 и currentUser 479659568322969611)
    const snap = engine.snapshot(1_792_394_000_000);
    assert.equal(snap.totalVotes, 2);
    const ans8 = snap.items.find((i) => i.answerId === 8);
    assert.ok(ans8);
    assert.equal(ans8.count, 2);
    assert.equal(ans8.percentage, 100);
    assert.equal(ans8.meVoted, true);
    assert.equal(ans8.isLeading, true);
    assert.equal(ans8.state, "voted");

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1004_3_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1004-3 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1004_3_.*ok/);
  });

  it("SUB-1004-4: покрывает все 9 обязательных измерений TEST-POLICY.md и проверяет CLI scripts/verify-polls.mjs (коды 0 / 1 / 2)", async () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const {
      evaluatePollNineDimensions,
      redactPollTelemetry,
      runPollsCli,
    } = await import("../../scripts/verify-polls.mjs");

    // 1. Все 9 измерений TEST-POLICY.md
    const dimReport = evaluatePollNineDimensions();
    assert.equal(
      dimReport.passed,
      true,
      `Сбой в 9 измерениях TEST-POLICY.md: ${dimReport.failures.join("; ")}`,
    );
    assert.equal(dimReport.dimensionsChecked, 9);

    // 2. Маскирование токенов в логах диагностики
    const rawLog =
      'PUT /api/v9/channels/1294815650186788916/polls/1558237752917758108/answers/8 Authorization="Bearer mfa.secret_poll_token_1004" token=dGVzdG1vY2t0b2tlbnZhbHVl.GhIjKl.MnOpQrStUvWxYz0123456789abcdef';
    const redacted = redactPollTelemetry(rawLog);
    assert.doesNotMatch(redacted, /mfa\.secret_poll_token_1004/);
    assert.doesNotMatch(redacted, /GhIjKl\.MnOpQrStUvWxYz0123456789abcdef/);
    assert.match(redacted, /\[REDACTED\]/);

    // 3. Проверка CLI scripts/verify-polls.mjs (коды выхода 0 / 1 / 2)
    const outJson = join(workDir, "polls-report.json");
    const outMd = join(workDir, "POLLS-REPORT.md");
    assert.equal(runPollsCli(["--help"]), 0);
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
      [SCRIPT_PATH, "--oracle", "/nonexistent/missing-kanban-1004.json"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(cliMissingOracle.status, 2);

    const rustRun = runCompiledRustTests(testBinPath, "test_sub_1004_4_");
    assert.equal(
      rustRun.status,
      0,
      `Тесты SUB-1004-4 в Rust упали:\n${rustRun.stdout}\n${rustRun.stderr}`,
    );
    assert.match(rustRun.stdout, /test tests::test_sub_1004_4_.*ok/);
  });

  it("SUB-1004-5: убивает 5/5 контрольных мутаций в rust/chord-gpui/src/chat/polls.rs и scripts/verify-polls.mjs (100% mutation kill rate)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник Rust должен быть собран");

    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalRustMd5 = createHash("md5").update(originalRust).digest("hex");

    const rustMutationSpecs = [
      {
        id: "MUT-1004-RS-01",
        operator: "MaxPollAnswersTamper",
        from: "pub const MAX_POLL_ANSWERS: usize = 10;",
        to: "pub const MAX_POLL_ANSWERS: usize = 2;",
      },
      {
        id: "MUT-1004-RS-02",
        operator: "PollVoteRouteVerbTamper",
        from: 'PollVoteAction::Add => "PUT",',
        to: 'PollVoteAction::Add => "POST",',
      },
      {
        id: "MUT-1004-RS-03",
        operator: "PercentageRoundingTamper",
        from: "((count as u64 * 100 + (total_votes as u64 / 2)) / total_votes as u64) as u8",
        to: "0u8",
      },
      {
        id: "MUT-1004-RS-04",
        operator: "ExpiryComparisonInversion",
        from: "now_ms >= expiry_ms",
        to: "now_ms < expiry_ms",
      },
      {
        id: "MUT-1004-RS-05",
        operator: "SendPollsPermissionBypass",
        from: "if !is_private && (permissions & PERM_SEND_POLLS) == 0 {",
        to: "if false && !is_private && (permissions & PERM_SEND_POLLS) == 0 {",
      },
    ];

    const rustMutants = rustMutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в polls.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "polls.rs",
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
    assert.equal(afterRustMd5, originalRustMd5, "MD5 исходного файла polls.rs не должен измениться");
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
        id: "MUT-1004-JS-01",
        operator: "PutVoteMethodTamper",
        from: 'const method = params.action === "remove" ? "DELETE" : "PUT";',
        to: 'const method = params.action === "remove" ? "DELETE" : "POST";',
      },
      {
        id: "MUT-1004-JS-02",
        operator: "PollOptionActiveStateTamper",
        from: 'active: "voted",',
        to: 'active: "idle",',
      },
      {
        id: "MUT-1004-JS-03",
        operator: "PercentageCalculationTamper",
        from: "totalVotes > 0 ? Math.round((item.count * 100) / totalVotes) : 0",
        to: "0",
      },
      {
        id: "MUT-1004-JS-04",
        operator: "PollExpiryCheckTamper",
        from: "return expiryMs <= nowMs;",
        to: "return expiryMs > nowMs;",
      },
      {
        id: "MUT-1004-JS-05",
        operator: "MultiselectBranchInversion",
        from: "if (!allowMultiselect) {",
        to: "if (allowMultiselect) {",
      },
    ];

    const jsMutants = jsMutationSpecs.map((spec) => {
      assert.ok(
        originalJs.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в verify-polls.mjs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "verify-polls.mjs",
        operator: spec.operator,
        mutatedSource: originalJs.replace(spec.from, spec.to),
      };
    });

    const jsGate = evaluateMutationGate(jsMutants, (mutant, index) => {
      const mutantJsPath = join(workDir, `mutant_polls_${index + 1}.mjs`);
      const harnessPath = join(workDir, `mutant_polls_harness_${index + 1}.mjs`);
      writeFileSync(mutantJsPath, mutant.mutatedSource, "utf8");
      writeFileSync(
        harnessPath,
        [
          'import assert from "node:assert/strict";',
          `import { POLL_OPTION_STATES, buildPollVoteRequest, computePollPercentages, isPollExpired, togglePollSelection } from ${JSON.stringify(mutantJsPath)};`,
          'assert.equal(POLL_OPTION_STATES.active, "voted");',
          'assert.equal(buildPollVoteRequest({ channelId: "1294815650186788916", messageId: "1558237752917758108", answerId: 8, action: "add" }).method, "PUT");',
          'assert.equal(computePollPercentages([{ answerId: 1, text: "A" }], [{ id: 1, count: 4, meVoted: true }], false).items[0]?.percentage, 100);',
          "assert.equal(isPollExpired(1000, 2000), true);",
          "assert.deepEqual(togglePollSelection([1], 2, false, 10), [2]);",
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
