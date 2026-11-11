// @ts-check
/**
 * Контрактные и модульные тесты мгновенного переключения аккаунтов (Multi-Account Switcher & Instant Handoff — TASK-987):
 * - SUB-987-0: Статический и структурный контракт `rust/discord-gpui/src/multi_account_switcher.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, соблюдение лимитов
 *   `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк, чистый `rustfmt --check --edition 2024`
 *   и компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-987-1: Сверка констант, дизайн-токенов, геометрии `Account Switcher Popup` (`280px x auto`, `padding: 8px`,
 *   `spacing: 4px`), интерактивных состояний `account-item` (`inactive` -> `active avatar` -> `switched`),
 *   3 IPC-каналов `DISCORD_SAFE_STORAGE_*`, префикса `dQw4w9WgXcQ:`, схемы `com.discordlab.Token` и
 *   модулей оригинального дампа Discord (`274303` `MultiAccountStore`, `376463` `MultiAccountSwitchStore`,
 *   `429707` `MultiAccountActionCreators`, `573879` `switch-accounts-modal`) с независимыми оракулами.
 * - SUB-987-2: Хранение до 5 профилей пользователей (`MAX_MULTI_ACCOUNTS = 5`) и строгая изоляция токенов
 *   разных аккаунтов (`SafeStorageTokenVault`: индивидуальное шифрование `dQw4w9WgXcQ:`, запрет коллизий токенов
 *   между аккаунтами, запрет кросс-аккаунтного чтения чужого токена, вытеснение 6-го профиля со стиранием
 *   секрета через `zeroize_slice`, защита `hide_tokens`/`show_tokens` и маскирование в `Debug`/`Display`).
 * - SUB-987-3: Мгновенное переключение Gateway и REST сессий (`< 300 мс`, `MAX_SWITCH_HANDOFF_MS = 300`)
 *   без перезапуска GPUI клиента (`gpui_window_restarted == false`), полный сброс сессионных данных
 *   предыдущего аккаунта (`guilds`, `messages`, `relationships`, `voice_session`), переупорядочивание
 *   профилей (`MULTI_ACCOUNT_MOVE_ACCOUNT`) и валидация токенов через `/api/v9/users/@me`.
 * - SUB-987-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные Snowflake `0`/`u64::MAX`, `5` vs `6` аккаунтов, `299 мс` vs `300 мс`, 2000 символов,
 *   10 файлов/25 МБ, `offset=9975`, громкость `0..200%`, бейдж `99+`, ошибки `401/403/404/413/429/5xx`
 *   и Gateway `4004`, пустые, `skeleton`/`HTTP 202`/`VALIDATING`, права `VIEW_CHANNEL`/`AGE_GATED_SPACES`,
 *   откат при сбое переключения, маскирование секретов в JSON-телеметрии).
 * - SUB-987-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
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
const MODULE_REL_PATH = "rust/discord-gpui/src/multi_account_switcher.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const BACKEND_IPC_JSON_PATH = join(ROOT, "docs", "re", "backend-ipc.json");
const REST_D02_JSON_PATH = join(ROOT, "docs", "re", "rest-d02.json");
const KEYRING_RS_PATH = join(
  ROOT,
  "rust",
  "crates",
  "discord-platform",
  "src",
  "keyring.rs",
);
const UI_ACCOUNT_SWITCHER_PATH = join(
  ROOT,
  "rust",
  "discord-gpui",
  "src",
  "ui",
  "account_switcher.rs",
);
const STATE_MOD_PATH = join(
  ROOT,
  "rust",
  "discord-gpui",
  "src",
  "state",
  "mod.rs",
);
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-gpui/src/multi_account_switcher.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-987: Мгновенное переключение аккаунтов (Multi-Account Switcher — rust/discord-gpui/src/multi_account_switcher.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "multi-account-switcher-987-"));
    testBinPath = join(workDir, "multi_account_switcher_987_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-987-0: модуль rust/discord-gpui/src/multi_account_switcher.rs существует, запрещает unsafe, проходит rustfmt 2024, лимиты (<50 строк на fn, <800 на файл), secret-scan и компилируется без предупреждений", () => {
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
      "В продуктовом коде multi_account_switcher.rs запрещены .unwrap() и .expect()",
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

  it("SUB-987-1: сверяет карточку TASK-987, дизайн-токены, геометрию Account Switcher Popup (280px, padding 8px, spacing 4px), 3 IPC-канала DISCORD_SAFE_STORAGE_* и модули RE-дампа (274303, 376463, 429707, 573879) с независимыми оракулами", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Независимый оракул: карточка TASK-987 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card987 = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-987",
    );
    assert.ok(
      card987,
      "Карточка TASK-987 должна существовать в waves/kanban-board.json",
    );
    assert.equal(card987.department, "G1 Backend");
    assert.equal(card987.layout.container, "Account Switcher Popup");
    assert.equal(card987.layout.dimensions, "280px x auto");
    assert.equal(card987.layout.padding, "8px");
    assert.equal(card987.layout.spacing, "4px");
    assert.equal(card987.interactive_states[0].element, "account-item");
    assert.equal(card987.interactive_states[0].default, "inactive");
    assert.equal(card987.interactive_states[0].hover, "active avatar");
    assert.equal(card987.interactive_states[0].active, "switched");

    // 2. Независимый оракул: 3 IPC-канала safeStorage в docs/re/backend-ipc.json
    const ipcData = JSON.parse(readFileSync(BACKEND_IPC_JSON_PATH, "utf8"));
    const ipcChannelNames = new Set(
      (ipcData.channels ?? []).map((/** @type {any} */ ch) => ch.name),
    );
    const expectedSafeStorageIpc = [
      "DISCORD_SAFE_STORAGE_ENCRYPT_STRING",
      "DISCORD_SAFE_STORAGE_DECRYPT_STRING",
      "DISCORD_SAFE_STORAGE_IS_ENCRYPTION_AVAILABLE",
    ];
    for (const ch of expectedSafeStorageIpc) {
      assert.ok(
        ipcChannelNames.has(ch),
        `IPC-канал ${ch} должен присутствовать в docs/re/backend-ipc.json`,
      );
    }

    // 3. Независимый оракул: маршрут /api/v9/users/@me в docs/re/rest-d02.json
    const restText = readFileSync(REST_D02_JSON_PATH, "utf8");
    assert.match(restText, /"\/api\/v9\/users\/@me"/);

    // 4. Независимый оракул: схема и префикс в rust/crates/discord-platform/src/keyring.rs
    const keyringSrc = readFileSync(KEYRING_RS_PATH, "utf8");
    assert.match(keyringSrc, /pub const SCHEMA_NAME: &str = "com\.discordlab\.Token";/);
    assert.match(keyringSrc, /pub const ENCRYPTED_TOKEN_PREFIX: &str = "dQw4w9WgXcQ:";/);

    // 5. Независимый оракул: UI и AppState в rust/discord-gpui/src/ui/account_switcher.rs и state/mod.rs
    const uiSwitcherSrc = readFileSync(UI_ACCOUNT_SWITCHER_PATH, "utf8");
    assert.match(uiSwitcherSrc, /\.w\(px\(280\.\)\)/);
    assert.match(uiSwitcherSrc, /\.p\(px\(8\.\)\)/);
    assert.match(uiSwitcherSrc, /\.gap\(px\(4\.\)\)/);
    const stateModSrc = readFileSync(STATE_MOD_PATH, "utf8");
    assert.match(stateModSrc, /pub fn switch_account\(&mut self, token: &str\)/);

    // 6. Независимый оракул: RE-дамп /home/sanya/re-deep/modules-index.json (если доступен на хосте)
    if (existsSync(RE_MODULES_INDEX_PATH)) {
      const modulesIdx = JSON.parse(readFileSync(RE_MODULES_INDEX_PATH, "utf8"));
      assert.ok(modulesIdx["274303"].names.includes("MultiAccountStore"));
      assert.ok(modulesIdx["376463"].names.includes("MultiAccountSwitchStore"));
      assert.ok(
        modulesIdx["429707"].strings.includes("MultiAccountActionCreators"),
      );
      assert.ok(modulesIdx["573879"].strings.includes("switch-accounts-modal"));
    }

    // 7. Сверка исходника multi_account_switcher.rs с токенами и константами оракулов
    const source = readFileSync(MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card987.design_tokens)) {
      assert.ok(
        source.includes(tokenVal),
        `Дизайн-токен ${tokenVal} должен быть зафиксирован в multi_account_switcher.rs`,
      );
    }
    for (const ch of expectedSafeStorageIpc) {
      assert.ok(
        source.includes(`"${ch}"`),
        `IPC-канал ${ch} должен быть определён в multi_account_switcher.rs`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_987_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-987-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_987_1_.*ok/);
  });

  it("SUB-987-2: обеспечивает хранение до 5 профилей пользователей и строгую криптографическую изоляцию токенов разных аккаунтов (SafeStorageTokenVault, защита от коллизий и кросс-чтения, zeroize_slice при удалении/вытеснении)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "MAX_MULTI_ACCOUNTS",
      "ENCRYPTED_TOKEN_PREFIX",
      "KEYRING_SCHEMA_NAME",
      "MultiAccountTokenStatus",
      "SafeStorageTokenVault",
      "AccountProfile",
      "MultiAccountSwitcher",
      "zeroize_slice",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_987_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-987-2 (хранение до 5 профилей и изоляция токенов) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_987_2_.*ok/);
  });

  it("SUB-987-3: выполняет мгновенное переключение Gateway и REST сессий (< 300 мс) без перезапуска GPUI клиента со сбросом предыдущей сессии и валидацией /api/v9/users/@me", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "MAX_SWITCH_HANDOFF_MS",
      "REST_USERS_ME_PATH",
      "SwitchHandoffPhaseTimings",
      "SessionHandoffReceipt",
      "AccountItemInteractiveState",
      "SwitchOrigin",
      "switch_to_account",
      "validate_account_token_response",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_987_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-987-3 (мгновенное переключение сессии < 300 мс) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_987_3_.*ok/);
  });

  it("SUB-987-4: покрывает все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 5 vs 6 профилей, 299 vs 300 мс, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, коды 401/403/404/413/429/5xx, Gateway 4004, права, откат и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "validate_workload_boundaries",
      "handle_gateway_close_during_switch",
      "redact_switcher_secrets",
      "export_switcher_telemetry_json",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_987_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-987-4 (9 измерений TEST-POLICY.md) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_987_4_.*ok/);
  });

  it("SUB-987-5: убивает 5/5 контрольных мутаций модуля multi_account_switcher.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(MODULE_PATH), "Исходник модуля должен существовать");
    const source = readFileSync(MODULE_PATH, "utf8");

    const mutationSpecs = [
      {
        id: "MUT-multi_account_switcher-01",
        operator: "MaxMultiAccountsCapRelaxed",
        from: "pub const MAX_MULTI_ACCOUNTS: usize = 5;",
        to: "pub const MAX_MULTI_ACCOUNTS: usize = 8;",
      },
      {
        id: "MUT-multi_account_switcher-02",
        operator: "SwitchHandoffSlaRelaxed",
        from: "if total_ms >= MAX_SWITCH_HANDOFF_MS {",
        to: "if total_ms > MAX_SWITCH_HANDOFF_MS {",
      },
      {
        id: "MUT-multi_account_switcher-03",
        operator: "CrossAccountTokenIsolationBypass",
        from: "if requester_user_id != target_user_id {",
        to: "if false && requester_user_id != target_user_id {",
      },
      {
        id: "MUT-multi_account_switcher-04",
        operator: "TokenCollisionGuardBypass",
        from: "if existing_uid != user_id && entry.token_fingerprint == fingerprint {",
        to: "if false && existing_uid != user_id && entry.token_fingerprint == fingerprint {",
      },
      {
        id: "MUT-multi_account_switcher-05",
        operator: "SessionStateIsolationClearOmitted",
        from: "guilds_cleared: true,",
        to: "guilds_cleared: false,",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        source.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в исходнике: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "multi_account_switcher.rs",
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
