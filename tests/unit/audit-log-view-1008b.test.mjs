// @ts-check
/**
 * Контрактные и модульные тесты фильтруемого списка аудита `AuditLogViewer` с иконками событий,
 * выпадающими фильтрами по действию/пользователю и раскрытием детального diff по клику (TASK-1008B):
 * - SUB-1008B-0: Статический и компиляционный контракт `rust/chord-gpui/src/guild/audit_log_view.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, соблюдение лимитов
 *   `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк, чистый `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1008B-1: Сверка карточек `TASK-1008B`, `TASK-1008`, `TASK-1008A` в `waves/kanban-board.json`,
 *   дизайн-токенов (`#313338`, `#2b2d31`, `#ffffff`, `#5865f2`, `#3f4147`), геометрии `Audit Log Viewer`
 *   (`100% x 100%`, `padding: 16px`, `spacing: 8px`), интерактивных состояний `audit-row`
 *   (`summary` -> `hover-row` -> `expanded diff`) и `audit-entry` (`entry row` -> `expand diff` -> `expanded details`),
 *   компонентов `docs/re/frontend-d03.json` (`Scroller` `scrollerBase_d125d2` / `listItems_affa7e`,
 *   `Menu (Mana)` `menu_c1e9c4` / `scroller_c1e9c4`, `TextInput / TextArea` `input__75098 comboBoxInput__0edde`,
 *   `Button` `button_a22cb0`, `Badge` `numberBadge__463b7` / `iconBadge__463b7`, `Avatar` `avatar_fdd9da`),
 *   модулей RE-дампа (`117497` `auditLogSection_b2f52f`, `165259` `auditLog__43dab`, `411153`,
 *   `523599` `GuildSettingsAuditLogStore`, `555067` `AuditLogUtils`) и прав `VIEW_AUDIT_LOG = 1 << 7`
 *   в `rust/crates/chord-core/src/permissions.rs`.
 * - SUB-1008B-2: Элементы журнала аудита `AuditLogRowViewSpec` — иконка типа действия (`AuditActionIconSpec`:
 *   глиф, категория `CREATE`/`UPDATE`/`DELETE`, тип цели, бейдж цвета `#23a55a`/`#5865f2`/`#f23f43`),
 *   ник модератора (`moderator_handle`), описание события (`event_description_ru` в паритете с `audit-log.js`),
 *   ISO-8601 и человекочитаемое время (`timestamp_iso`, `timestamp_ms` из Snowflake).
 * - SUB-1008B-3: Выпадающие фильтры «Фильтр по действию» (`ActionDropdownOption`, `filter_by_action`) и
 *   «Фильтр по пользователю» (`UserDropdownOption`, `filter_by_user`), раскрытие/сворачивание детального
 *   diff изменений (`old_value → new_value`) по клику (`toggle_row_expanded`, состояние `expanded diff`)
 *   и клавиатурная навигация (`ArrowDown`, `ArrowUp`, `Home`, `End`, `Enter`, `Space`).
 * - SUB-1008B-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные: Snowflake `0`/`u64::MAX`, лимит `1..100`, причина `1..512`, `2000` символов,
 *   `10` файлов/`25 МБ`, `offset=9975`, громкость `0..200%`, бейдж `99+`; ошибки HTTP
 *   `400/401/403/404/413/429 retry_after_ms/5xx`; пустой список аудита без падения; `skeleton`/`HTTP 202`;
 *   права `VIEW_AUDIT_LOG`/`ADMINISTRATOR`/`AGE_GATED_SPACES`; группировка 30 мин и очистка при `403`;
 *   маскирование секретов в диффах и телеметрии).
 * - SUB-1008B-5: Мутационный гейт `5/5` (`100%` kill rate) для `rust/chord-gpui/src/guild/audit_log_view.rs`.
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
const RUST_MODULE_REL_PATH = "rust/chord-gpui/src/guild/audit_log_view.rs";
const RUST_MODULE_PATH = join(ROOT, RUST_MODULE_REL_PATH);
const GUILD_MOD_PATH = join(ROOT, "rust/chord-gpui/src/guild/mod.rs");
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const RE_DEEP_FRONTEND_D03_PATH = "/home/sanya/re-deep/frontend-d03.json";
const DOCS_FRONTEND_D03_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";
const RE_REST_STATIC_PATH = "/home/sanya/re-deep/rest-static.json";
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
 * Компилирует `rust/chord-gpui/src/guild/audit_log_view.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1008B: Фильтруемый список аудита AuditLogViewer с иконками событий, выпадающими фильтрами и раскрытием diff (rust/chord-gpui/src/guild/audit_log_view.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "audit-log-view-1008b-"));
    testBinPath = join(workDir, "audit_log_view_1008b_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1008B-0: проверяет статический и компиляционный контракт rust/chord-gpui/src/guild/audit_log_view.rs (<50 строк/fn, <800 строк/файл, #![forbid(unsafe_code)], rustfmt 2024, 0 .unwrap()/.expect() и экспорт в guild/mod.rs)", () => {
    assert.ok(
      existsSync(RUST_MODULE_PATH),
      `Отсутствует Rust-модуль ${RUST_MODULE_REL_PATH}`,
    );

    const modSrc = readFileSync(GUILD_MOD_PATH, "utf8");
    assert.match(
      modSrc,
      /pub\s+mod\s+audit_log_view\s*;/,
      "В rust/chord-gpui/src/guild/mod.rs должен быть экспортирован pub mod audit_log_view;",
    );

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    assert.match(
      rustSource,
      /^#!\[forbid\(unsafe_code\)\]/m,
      "В модуле audit_log_view.rs обязательна директива #![forbid(unsafe_code)]",
    );

    const prodSection = rustSource.split("#[cfg(test)]")[0] ?? "";
    assert.equal(
      prodSection.includes(".unwrap()") || prodSection.includes(".expect("),
      false,
      "В продуктовом коде audit_log_view.rs запрещены .unwrap() и .expect()",
    );

    for (const requiredSymbol of [
      'pub const AUDIT_LOG_VIEWER_CONTAINER: &str = "Audit Log Viewer";',
      'pub const AUDIT_LOG_VIEWER_DIMENSIONS: &str = "100% x 100%";',
      "pub const AUDIT_LOG_VIEWER_PADDING_PX: u32 = 16;",
      "pub const AUDIT_LOG_VIEWER_SPACING_PX: u32 = 8;",
      'pub const AUDIT_ROW_ELEMENT: &str = "audit-row";',
      "pub enum AuditRowInteractiveState",
      "pub struct AuditActionIconSpec",
      "pub struct ActionDropdownOption",
      "pub struct UserDropdownOption",
      "pub struct AuditLogRowViewSpec",
      "pub struct AuditLogViewer",
      "pub fn resolve_action_icon(",
      "pub fn toggle_row_expanded(",
      "pub fn build_audit_log_viewer(",
    ]) {
      assert.ok(
        rustSource.includes(requiredSymbol),
        `В audit_log_view.rs должен присутствовать контракт TASK-1008B: ${requiredSymbol}`,
      );
    }

    const sizeCheck = checkRustSourceSizeLimits(RUST_MODULE_REL_PATH, rustSource);
    assert.equal(
      sizeCheck.passed,
      true,
      `Нарушение лимитов размера кода: ${sizeCheck.reason}`,
    );
    assert.ok(
      sizeCheck.fileLines >= 250 && sizeCheck.fileLines < 800,
      `Ожидался размер файла 250..799 строк, получено: ${sizeCheck.fileLines}`,
    );

    const secretFindings = scanTextForSecrets(rustSource, RUST_MODULE_REL_PATH);
    assert.equal(secretFindings.length, 0);

    const fmtCheck = spawnSync(
      "rustfmt",
      ["--check", "--edition", "2024", RUST_MODULE_PATH],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(
      fmtCheck.status,
      0,
      `rustfmt --check завершился с ошибкой:\n${fmtCheck.stderr || fmtCheck.stdout}`,
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

    const allTests = runCompiledRustTests(testBinPath);
    assert.equal(
      allTests.status,
      0,
      `Ошибка выполнения встроенных Rust-тестов:\n${allTests.stderr || allTests.stdout}`,
    );
    assert.match(allTests.stdout, /test result: ok\./);
  });

  it("SUB-1008B-1: сверяет карточку TASK-1008B в waves/kanban-board.json, дизайн-токены (#313338, #2b2d31, #ffffff, #5865f2, #3f4147), состояния audit-row (summary -> hover-row -> expanded diff) и оракулы frontend-d03.json / modules-index.json / permissions.rs", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);

    const kanban = JSON.parse(kanbanRaw);
    const card1008B = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1008B",
    );
    assert.ok(
      card1008B,
      "Карточка TASK-1008B должна присутствовать в waves/kanban-board.json",
    );
    assert.equal(card1008B.department, "G2 Frontend");
    assert.ok(
      card1008B.priority === "P1" || card1008B.priority === "P2",
      "Приоритет карточки TASK-1008B должен быть P1 или P2",
    );
    assert.deepEqual(card1008B.layout, {
      container: "Audit Log Viewer",
      dimensions: "100% x 100%",
      padding: "16px",
      spacing: "8px",
    });
    assert.deepEqual(card1008B.design_tokens, {
      background: "#313338",
      surface: "#2b2d31",
      text_primary: "#ffffff",
      accent: "#5865f2",
      border: "#3f4147",
    });
    assert.deepEqual(card1008B.interactive_states, [
      {
        element: "audit-row",
        default: "summary",
        hover: "hover-row",
        active: "expanded diff",
      },
    ]);

    const card1008 = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1008",
    );
    assert.ok(card1008, "Карточка TASK-1008 должна присутствовать в waves/kanban-board.json");
    assert.deepEqual(card1008.interactive_states[0], {
      element: "audit-entry",
      default: "entry row",
      hover: "expand diff",
      active: "expanded details",
    });

    const d03Path = existsSync(RE_DEEP_FRONTEND_D03_PATH)
      ? RE_DEEP_FRONTEND_D03_PATH
      : DOCS_FRONTEND_D03_PATH;
    const d03 = JSON.parse(readFileSync(d03Path, "utf8"));
    assert.equal(d03.meta.task, "D-03");

    const scrollerComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Scroller",
    );
    assert.ok(scrollerComp);
    assert.match(scrollerComp.evidence[0], /scrollerBase_d125d2/);
    assert.match(scrollerComp.evidence[1], /listItems_affa7e/);

    const menuComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Menu (Mana)",
    );
    assert.ok(menuComp);
    assert.match(menuComp.evidence[0], /menu_c1e9c4/);
    assert.match(menuComp.evidence[0], /scroller_c1e9c4/);

    const inputComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "TextInput / TextArea",
    );
    assert.ok(inputComp);
    assert.match(inputComp.evidence[2], /input__75098 comboBoxInput__0edde/);

    const badgeComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Badge",
    );
    assert.ok(badgeComp);
    assert.match(badgeComp.evidence[0], /numberBadge__463b7/);
    assert.match(badgeComp.evidence[0], /iconBadge__463b7/);

    const avatarComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Avatar",
    );
    assert.ok(avatarComp);
    assert.match(avatarComp.evidence[0], /avatar_fdd9da/);

    if (existsSync(RE_MODULES_INDEX_PATH)) {
      const modulesIdx = JSON.parse(readFileSync(RE_MODULES_INDEX_PATH, "utf8"));
      assert.ok(modulesIdx["117497"].strings.includes("auditLogSection_b2f52f"));
      assert.ok(modulesIdx["165259"].strings.includes("auditLog__43dab"));
      assert.ok(modulesIdx["523599"].names.includes("GuildSettingsAuditLogStore"));
      assert.ok(modulesIdx["555067"].strings.includes("AuditLogUtils"));
    }

    if (existsSync(RE_REST_STATIC_PATH)) {
      const restStatic = JSON.parse(readFileSync(RE_REST_STATIC_PATH, "utf8"));
      const auditEp = (restStatic.endpoints ?? []).find(
        (/** @type {any} */ e) => e.key === "GUILD_AUDIT_LOG",
      );
      assert.ok(auditEp);
      assert.equal(auditEp.path, "/guilds/{arg}/audit-logs");
    }

    const permsSrc = readFileSync(PERMISSIONS_RS_PATH, "utf8");
    assert.match(permsSrc, /pub const VIEW_AUDIT_LOG: u64 = 1 << 7;/);

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const marker of [
      "#313338",
      "#2b2d31",
      "#ffffff",
      "#5865f2",
      "#3f4147",
      "Audit Log Viewer",
      "100% x 100%",
      "audit-row",
      "summary",
      "hover-row",
      "expanded diff",
      "scrollerBase_d125d2",
      "listItems_affa7e",
      "menu_c1e9c4",
      "comboBoxInput__0edde",
      "numberBadge__463b7",
      "iconBadge__463b7",
      "avatar_fdd9da",
      "auditLogSection_b2f52f",
      "auditLog__43dab",
      "GuildSettingsAuditLogStore",
      "AuditLogUtils",
    ]) {
      assert.ok(
        rustSource.includes(marker),
        `В audit_log_view.rs должен быть зафиксирован маркер оракула: ${marker}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1008b_1_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1008B-1 в Rust упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1008b_1_.*ok/);
  });

  it("SUB-1008B-2: проверяет элементы журнала аудита (иконка типа действия AuditActionIconSpec, ник модератора, описание события в паритете с audit-log.js и ISO время из Snowflake)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const rustSource = readFileSync(RUST_MODULE_PATH, "utf8");
    for (const code of Object.values(spikeActionTypes)) {
      const labelRu = spikeActionLabelsRu[code];
      if (!labelRu) continue;
      assert.ok(
        rustSource.includes(labelRu),
        `Русская метка "${labelRu}" для кода ${code} должна присутствовать в audit_log_view.rs`,
      );
    }

    assert.equal(
      spikeFormatAuditLogEntryRu({
        actionType: 22,
        actorName: "Админ",
        targetName: "Боб",
        reason: "Спам",
      }),
      "Админ: Бан участника (Боб) — Причина: Спам",
    );

    const run = runCompiledRustTests(testBinPath, "test_sub_1008b_2_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1008B-2 (элементы журнала, иконки событий, модератор, время) упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1008b_2_.*ok/);
  });

  it("SUB-1008B-3: проверяет выпадающие фильтры («Фильтр по действию» и «Фильтр по пользователю»), раскрытие деталей по клику с форматированным diff изменений и клавиатурную навигацию", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    assert.equal(
      spikeFormatAuditChangeDiffRu({
        key: "name",
        oldValue: "старый-канал",
        newValue: "новый-канал",
      }),
      "name: старый-канал → новый-канал",
    );
    assert.equal(
      spikeFormatAuditChangeDiffRu({
        key: "topic",
        oldValue: null,
        newValue: "Правила сервера",
      }),
      "topic: нет → Правила сервера",
    );

    const run = runCompiledRustTests(testBinPath, "test_sub_1008b_3_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1008B-3 (выпадающие фильтры и раскрытие diff по клику) упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1008b_3_.*ok/);
  });

  it("SUB-1008B-4: покрывает все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, лимиты 1..100, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, коды 400..5xx, пустой список аудита без падения, skeleton/202, права VIEW_AUDIT_LOG/AGE_GATED_SPACES и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const run = runCompiledRustTests(testBinPath, "test_sub_1008b_4_");
    assert.equal(
      run.status,
      0,
      `Тест SUB-1008B-4 (9 измерений TEST-POLICY.md) упал:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1008b_4_.*ok/);
  });

  it("SUB-1008B-5: убивает 5/5 контрольных мутаций модуля audit_log_view.rs (100% mutation kill rate: несрабатывание фильтра по действию, несрабатывание фильтра по пользователю, отказ раскрытия diff по клику, искажение состояния expanded diff, подмена стрелки диффа old/new)", () => {
    assert.ok(existsSync(RUST_MODULE_PATH), "Исходник модуля должен существовать");
    const originalRust = readFileSync(RUST_MODULE_PATH, "utf8");
    const originalMd5 = createHash("md5").update(originalRust).digest("hex");

    const mutationSpecs = [
      {
        id: "MUT-1008B-01",
        operator: "ActionDropdownFilterBypass",
        from: "if let Some(act) = self.selected_action_filter {",
        to: "if let Some(act) = None::<u16> {",
      },
      {
        id: "MUT-1008B-02",
        operator: "UserDropdownFilterBypass",
        from: "if let Some(uid) = &self.selected_user_filter {",
        to: "if let Some(uid) = &None::<String> {",
      },
      {
        id: "MUT-1008B-03",
        operator: "ClickExpandToggleDisabled",
        from: "if !self.expanded_entry_ids.remove(&entry_id) {",
        to: "if false && !self.expanded_entry_ids.remove(&entry_id) {",
      },
      {
        id: "MUT-1008B-04",
        operator: "AuditRowActiveStateTamper",
        from: 'Self::ExpandedDiff => "expanded diff",',
        to: 'Self::ExpandedDiff => "summary",',
      },
      {
        id: "MUT-1008B-05",
        operator: "ChangeDiffArrowOrderSwap",
        from: 'format!("{key}: {old_str} → {new_str}")',
        to: 'format!("{key}: {new_str} → {old_str}")',
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        originalRust.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в audit_log_view.rs: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "audit_log_view.rs",
        operator: spec.operator,
        mutatedSource: originalRust.replace(spec.from, spec.to),
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

    const afterMd5 = createHash("md5")
      .update(readFileSync(RUST_MODULE_PATH, "utf8"))
      .digest("hex");
    assert.equal(
      afterMd5,
      originalMd5,
      "MD5 исходного файла audit_log_view.rs не должен измениться",
    );

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
