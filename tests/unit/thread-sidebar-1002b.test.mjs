// @ts-check
/**
 * Контрактные и модульные тесты правой сплит-панели треда `ThreadSidebar` (360px)
 * с независимым виртуальным скроллом и буфером сообщений (TASK-1002B):
 * - SUB-1002B-0: Статический и компиляционный контракт `rust/discord-gpui/src/chat/thread_sidebar.rs`
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, соблюдение лимитов
 *   `scripts/quality-gate.mjs`: функции < 50 строк, модуль < 800 строк, чистый `rustfmt --check --edition 2024`,
 *   компиляция `rustc --test --edition 2024 -D warnings` без предупреждений, 0 утечек в `secret-scan.mjs`).
 * - SUB-1002B-1: Сверка карточек `TASK-1002B` и `TASK-1002` в `waves/kanban-board.json`, дизайн-токенов
 *   (`#2b2d31`, `#1e1f22`, `#ffffff`, `#5865f2`, `#3f4147`), геометрии `Thread Split View` (`360px width`,
 *   `padding: 8px`, `spacing: 8px`), интерактивных состояний `thread-split` (`closed` -> `open button` ->
 *   `sidebar open`) и `thread-indicator` (`link` -> `underline` -> `open sidebar`), компонентов RE-дампа
 *   `/home/sanya/re-deep/frontend-d03.json` (`Scroller` module `6095` `scrollerBase_d125d2`, `listItems_affa7e`,
 *   `scrollbarGutterStable`, `disableScrollAnchor`; `Button` module `657718` `button_a22cb0`;
 *   `TextInput / TextArea` module `103557` `data-mana-component="text-area"`; `Badge` module `306091`
 *   `numberBadge__463b7`; `Avatar` module `311582` `avatar_fdd9da`) и сторов `modules-index.json`
 *   (`456874` `ThreadMessageStore`, `780057` `ThreadSummaryStore`, `802958` `ThreadMembersStore`,
 *   `863005` `ActiveJoinedThreadsStore`, `970278` `ActiveThreadsStore`) с независимыми оракулами.
 * - SUB-1002B-2: Разметка GPUI `ThreadSidebar` (фиксированная ширина `360px` при ресайзе окна `900..3840px`,
 *   шапка с заголовком треда, бейджем автоархивации и кнопкой закрытия) и независимый экземпляр
 *   `virtual_list` с изолированным буфером сообщений каждого треда и сохранением позиции скролла.
 * - SUB-1002B-3: Индикатор под родительским сообщением в основном чате (`«N ответов • Последний X мин назад»`,
 *   префикс удалённого исходного сообщения) и привязка к `AppState` (открытие/переключение/закрытие треда,
 *   сохранение черновика ввода и удержание фокуса поля ввода при переключении тредов, изолированная
 *   отправка сообщений в буфер треда).
 * - SUB-1002B-4: Покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные,
 *   граничные Snowflake `0`/`u64::MAX`, 2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`,
 *   бейдж `99+`, коды `400/401/403/404/413/429/5xx`, пустые, `skeleton`/`HTTP 202`, права доступа и
 *   `AGE_GATED_SPACES`, восстановление `THREAD_LIST_SYNC` и маскирование секретов в JSON-телеметрии).
 * - SUB-1002B-5: Мутационный гейт `5/5` (`100%` kill rate) по контракту `scripts/quality-gate.mjs`.
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
const MODULE_REL_PATH = "rust/discord-gpui/src/chat/thread_sidebar.rs";
const MODULE_PATH = join(ROOT, MODULE_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const FRONTEND_D03_REPO_PATH = join(ROOT, "docs", "re", "frontend-d03.json");
const RE_FRONTEND_D03_PATH = "/home/sanya/re-deep/frontend-d03.json";
const RE_MODULES_INDEX_PATH = "/home/sanya/re-deep/modules-index.json";
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

/** @type {string} */
let workDir = "";
/** @type {string} */
let testBinPath = "";

/**
 * Компилирует `rust/discord-gpui/src/chat/thread_sidebar.rs` через `rustc --test --edition 2024 -D warnings`.
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

describe("TASK-1002B: Правая сплит-панель треда ThreadSidebar (360px) с независимым буфером сообщений и виртуальным скроллом (rust/discord-gpui/src/chat/thread_sidebar.rs)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "thread-sidebar-1002b-"));
    testBinPath = join(workDir, "thread_sidebar_1002b_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1002B-0: модуль rust/discord-gpui/src/chat/thread_sidebar.rs существует, запрещает unsafe, проходит rustfmt 2024, лимиты (<50 строк на fn, <800 на файл), secret-scan и компилируется без предупреждений", () => {
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
      "В продуктовом коде thread_sidebar.rs запрещены .unwrap() и .expect()",
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
      sizeCheck.fileLines > 250 && sizeCheck.fileLines < 800,
      `Ожидался размер файла 250..800 строк, получено: ${sizeCheck.fileLines}`,
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

  it("SUB-1002B-1: сверяет карточку TASK-1002B, дизайн-токены, геометрию Thread Split View (360px width, padding 8px, spacing 8px), компоненты frontend-d03.json и сторы RE-дампа с независимыми оракулами", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    // 1. Независимый оракул: карточки TASK-1002B и TASK-1002 в waves/kanban-board.json
    const kanbanRaw = readFileSync(KANBAN_BOARD_PATH, "utf8");
    const kanbanMd5 = createHash("md5").update(kanbanRaw).digest("hex");
    assert.equal(kanbanMd5.length, 32);
    const kanban = JSON.parse(kanbanRaw);
    const card1002b = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1002B",
    );
    assert.ok(
      card1002b,
      "Карточка TASK-1002B должна существовать в waves/kanban-board.json",
    );
    assert.equal(card1002b.department, "G2 Frontend");
    assert.equal(card1002b.priority, "P1");
    assert.equal(card1002b.layout.container, "Thread Split View");
    assert.equal(card1002b.layout.dimensions, "360px width");
    assert.equal(card1002b.layout.padding, "8px");
    assert.equal(card1002b.layout.spacing, "8px");
    assert.equal(card1002b.interactive_states[0].element, "thread-split");
    assert.equal(card1002b.interactive_states[0].default, "closed");
    assert.equal(card1002b.interactive_states[0].hover, "open button");
    assert.equal(card1002b.interactive_states[0].active, "sidebar open");

    const card1002 = kanban.cards.find(
      (/** @type {any} */ c) => c.id === "TASK-1002",
    );
    assert.ok(card1002, "Карточка TASK-1002 должна существовать в waves/kanban-board.json");
    assert.equal(card1002.interactive_states[0].element, "thread-indicator");
    assert.equal(card1002.interactive_states[0].default, "link");
    assert.equal(card1002.interactive_states[0].hover, "underline");
    assert.equal(card1002.interactive_states[0].active, "open sidebar");

    // 2. Независимый оракул: /home/sanya/re-deep/frontend-d03.json (или docs/re/frontend-d03.json)
    const d03Path = existsSync(RE_FRONTEND_D03_PATH)
      ? RE_FRONTEND_D03_PATH
      : FRONTEND_D03_REPO_PATH;
    const d03Raw = readFileSync(d03Path, "utf8");
    const d03 = JSON.parse(d03Raw);
    assert.equal(d03.meta.task, "D-03");

    const scrollerComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Scroller",
    );
    assert.ok(scrollerComp, "Компонент Scroller должен присутствовать в frontend-d03.json");
    assert.ok(scrollerComp.variants.includes("thin"));
    assert.ok(scrollerComp.variants.includes("виртуализованный список (listItems)"));
    assert.ok(scrollerComp.states.includes("scrollbarGutterStable"));
    assert.ok(scrollerComp.states.includes("disableScrollAnchor"));
    assert.ok(scrollerComp.tokens.includes("--scrollbar-thin-thumb"));
    assert.ok(scrollerComp.tokens.includes("--scrollbar-thin-track"));
    assert.match(scrollerComp.evidence[0], /module 6095/);
    assert.match(scrollerComp.evidence[0], /scrollerBase_d125d2/);
    assert.match(scrollerComp.evidence[1], /listItems_affa7e/);

    const buttonComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Button",
    );
    assert.ok(buttonComp, "Компонент Button должен присутствовать в frontend-d03.json");
    assert.ok(buttonComp.variants.includes("icon-only"));
    assert.match(buttonComp.evidence[1], /button_a22cb0/);

    const textInputComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "TextInput / TextArea",
    );
    assert.ok(textInputComp, "Компонент TextInput / TextArea должен присутствовать в frontend-d03.json");
    assert.match(textInputComp.evidence[0], /text-area/);
    assert.match(textInputComp.evidence[1], /input__75098/);

    const badgeComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Badge",
    );
    assert.ok(badgeComp, "Компонент Badge должен присутствовать в frontend-d03.json");
    assert.match(badgeComp.evidence[0], /numberBadge__463b7/);

    const avatarComp = d03.components.find(
      (/** @type {any} */ c) => c.name === "Avatar",
    );
    assert.ok(avatarComp, "Компонент Avatar должен присутствовать в frontend-d03.json");
    assert.match(avatarComp.evidence[0], /avatar_fdd9da/);

    // 3. Независимый оракул: сторы тредов в /home/sanya/re-deep/modules-index.json
    if (existsSync(RE_MODULES_INDEX_PATH)) {
      const modulesIdx = JSON.parse(readFileSync(RE_MODULES_INDEX_PATH, "utf8"));
      assert.ok(modulesIdx["456874"].names.includes("ThreadMessageStore"));
      assert.ok(modulesIdx["780057"].names.includes("ThreadSummaryStore"));
      assert.ok(modulesIdx["802958"].names.includes("ThreadMembersStore"));
      assert.ok(modulesIdx["863005"].names.includes("ActiveJoinedThreadsStore"));
      assert.ok(modulesIdx["970278"].names.includes("ActiveThreadsStore"));
    }

    // 4. Независимый оракул: права и длительности автоархивации
    const permSrc = readFileSync(PERMISSIONS_RS_PATH, "utf8");
    assert.match(permSrc, /pub const MANAGE_THREADS: u64 = 1 << 34;/);
    assert.match(permSrc, /pub const SEND_MESSAGES_IN_THREADS: u64 = 1 << 38;/);

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

    // 5. Сверка исходника thread_sidebar.rs с токенами и классами frontend-d03.json
    const source = readFileSync(MODULE_PATH, "utf8");
    for (const tokenVal of Object.values(card1002b.design_tokens)) {
      assert.ok(
        source.includes(tokenVal),
        `Дизайн-токен ${tokenVal} должен быть зафиксирован в thread_sidebar.rs`,
      );
    }
    for (const d03Marker of [
      "scrollerBase_d125d2",
      "listItems_affa7e",
      "button_a22cb0",
      "input__75098",
      "numberBadge__463b7",
      "avatar_fdd9da",
      "--scrollbar-thin-thumb",
      "--scrollbar-thin-track",
      "ThreadMessageStore",
      "ThreadSummaryStore",
    ]) {
      assert.ok(
        source.includes(d03Marker),
        `Маркер оракула frontend-d03 / RE-дампа ${d03Marker} должен присутствовать в thread_sidebar.rs`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1002b_1_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1002B-1 упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1002b_1_.*ok/);
  });

  it("SUB-1002B-2: проверяет разметку GPUI правой сплит-панели фиксированной ширины 360px (устойчивость к ресайзу окна, шапка и кнопка закрытия) и независимый virtual_list с отдельным буфером сообщений каждого треда", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "THREAD_SIDEBAR_WIDTH_PX",
      "THREAD_SIDEBAR_PADDING_PX",
      "THREAD_SIDEBAR_SPACING_PX",
      "ThreadSplitInteractiveState",
      "ThreadSidebarLayoutSpec",
      "VirtualListViewport",
      "ThreadMessageBuffer",
      "compute_split_view_layout",
      "scroll_thread_virtual_list",
      "scroll_main_chat_virtual_list",
      "visible_virtual_list_slice",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1002b_2_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1002B-2 (GPUI 360px разметка и независимый virtual_list) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1002b_2_.*ok/);
  });

  it("SUB-1002B-3: проверяет индикатор треда под сообщением («N ответов • Последний X мин назад») и привязку к AppState (открытие/переключение/закрытие, сохранение фокуса и черновика ввода, отправка в буфер треда)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "ThreadIndicatorInteractiveState",
      "StarterThreadIndicatorSpec",
      "ThreadSidebarAppState",
      "format_thread_indicator_ru",
      "build_starter_thread_indicator",
      "open_thread_in_sidebar",
      "switch_active_thread",
      "close_thread_sidebar",
      "set_active_thread_draft",
      "send_thread_reply",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1002b_3_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1002B-3 (индикатор треда и привязка к AppState) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1002b_3_.*ok/);
  });

  it("SUB-1002B-4: покрывает все 9 обязательных измерений TEST-POLICY.md (граничные Snowflake 0/u64::MAX, 2000 символов, 10 файлов/25 МБ, offset=9975, громкость 0..200%, бейдж 99+, коды 400/401/403/404/413/429/5xx, пустые, skeleton/202, права и AGE_GATED_SPACES, THREAD_LIST_SYNC и маскирование секретов)", () => {
    assert.ok(existsSync(testBinPath), "Тестовый бинарник должен быть собран");

    const source = readFileSync(MODULE_PATH, "utf8");
    for (const requiredSymbol of [
      "validate_sidebar_workload_boundaries",
      "classify_thread_http_status",
      "apply_thread_list_sync_recovery",
      "redact_sidebar_secrets",
      "export_thread_sidebar_telemetry_json",
    ]) {
      assert.match(
        source,
        new RegExp(`\\b${requiredSymbol}\\b`),
        `В модуле должен присутствовать символ ${requiredSymbol}`,
      );
    }

    const run = runCompiledRustTests(testBinPath, "test_sub_1002b_4_");
    assert.equal(
      run.status,
      0,
      `Тесты SUB-1002B-4 (9 измерений TEST-POLICY.md) упали:\n${run.stdout}\n${run.stderr}`,
    );
    assert.match(run.stdout, /test tests::test_sub_1002b_4_.*ok/);
  });

  it("SUB-1002B-5: убивает 5/5 контрольных мутаций модуля thread_sidebar.rs (100% mutation kill rate)", () => {
    assert.ok(existsSync(MODULE_PATH), "Исходник модуля должен существовать");
    const source = readFileSync(MODULE_PATH, "utf8");

    const mutationSpecs = [
      {
        id: "MUT-thread-sidebar-01",
        operator: "SidebarFixedWidthBrokenOnResize",
        from: "let sidebar_w = u32::from(sidebar_open) * THREAD_SIDEBAR_WIDTH_PX;",
        to: "let sidebar_w = u32::from(sidebar_open) * (clamped_window / 3);",
      },
      {
        id: "MUT-thread-sidebar-02",
        operator: "VirtualListScrollCoupledToMainChat",
        from: "buf.viewport = next;",
        to: "self.main_chat_viewport = next;",
      },
      {
        id: "MUT-thread-sidebar-03",
        operator: "InputFocusLostOnThreadSwitch",
        from: "self.input_focused = true;",
        to: "self.input_focused = false;",
      },
      {
        id: "MUT-thread-sidebar-04",
        operator: "ThreadIndicatorSummaryFormatCorrupted",
        from: "\"{prefix}{reply_count} ответов • Последний {elapsed_min} мин назад\"",
        to: "\"{prefix}{reply_count} ответов\"",
      },
      {
        id: "MUT-thread-sidebar-05",
        operator: "SendMessagesInThreadsPermissionBypassed",
        from: "let has_send = (permissions_bits & PERM_SEND_MESSAGES_IN_THREADS) != 0;",
        to: "let has_send = true;",
      },
    ];

    const mutants = mutationSpecs.map((spec) => {
      assert.ok(
        source.includes(spec.from),
        `Целевая строка для мутации ${spec.id} должна присутствовать в исходнике: ${spec.from}`,
      );
      return {
        id: spec.id,
        moduleName: "thread_sidebar.rs",
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
