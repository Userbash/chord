// @ts-check
// QA-IMPACT (RW-40): контракт селектора тестов scripts/test-impact.mjs.
// Проверяются только правила выбора команд: без live, без сети, без спавна окна.
// Каждый ожидаемый набор сверяется с независимым оракулом (пути/имена тестов),
// а не с текущим выводом скрипта.

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { planFor } from "../../scripts/test-impact.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const script = new URL("../../scripts/test-impact.mjs", import.meta.url).pathname;

/**
 * Запускает CLI-обёртку селектора и разбирает JSON.
 * @param {string[]} args
 * @returns {{ tier: string, commands: string[], reasons: Record<string, string>, changed?: string[] }}
 */
function runCli(args) {
  const out = execFileSync(process.execPath, [script, ...args], {
    cwd: root,
    encoding: "utf8",
  });
  return JSON.parse(out);
}

/** @param {{ commands: string[] }} plan @param {string} needle */
const has = (plan, needle) => plan.commands.some((c) => c.includes(needle));

/** Признаки живой операции/сети: таких команд в плане быть не должно. */
const FORBIDDEN = [
  /live-discord/i,
  /GUI_SPIKE_E2E/,
  /\bcurl\b/,
  /\bwget\b/,
  /https?:\/\//i,
  /discord\.com/i,
  /\battach\b/i,
  /gui-spike\.smoke/i,
  /--online\b/i,
];

/**
 * @param {{ commands: string[] }} plan
 * @param {string} label
 */
function assertNoLivePlan(plan, label) {
  for (const pattern of FORBIDDEN) {
    const bad = plan.commands.filter((c) => pattern.test(c));
    assert.deepEqual(bad, [], `${label}: в плане есть live/сетевая команда ${pattern}: ${bad.join(" | ")}`);
  }
}

describe("test-impact: docs → docs-sync", () => {
  it("docs-файл и каталог docs включают meta docs-sync", () => {
    for (const changed of [["docs/map.md"], ["docs"]]) {
      const plan = planFor(changed);
      assert.ok(has(plan, "npm run typecheck"), `${changed}: нет typecheck`);
      assert.ok(has(plan, "tests/meta/docs-sync.test.mjs"), `${changed}: нет docs-sync`);
      assert.equal(plan.tier, "T0 (targeted)", `${changed}: docs не требует T1`);
    }
  });
});

describe("test-impact: rust/gui-spike/ui/features/join-server.js", () => {
  const plan = planFor(["rust/gui-spike/ui/features/join-server.js"]);

  it("выбирает join-server + features + typecheck", () => {
    assert.ok(has(plan, "npm run typecheck"), "нет typecheck");
    assert.ok(has(plan, "tests/unit/join-server.test.mjs"), "нет join-server");
    assert.ok(has(plan, "tests/unit/gui-spike-features.test.mjs"), "нет features-контракта");
  });

  it("не запускает весь frontend и геометрию для одного feature", () => {
    assert.equal(plan.commands.length, 3, `ожидалось 3 команды, получено ${plan.commands.length}: ${plan.commands.join(" | ")}`);
    assert.ok(!has(plan, "guild-rail.test.mjs"), "лишний полный frontend-прогон");
    assert.ok(!has(plan, "geometry-check.mjs"), "лишняя геометрия для feature-модуля");
    assert.ok(!plan.commands.some((c) => c.includes("*")), "не должно быть wildcard-прогона");
    assertNoLivePlan(plan, "join-server");
  });
});

describe("test-impact: rust/crates/discord-core", () => {
  it("каталог и файл крейта дают targeted cargo discord-core, а не workspace", () => {
    for (const changed of [["rust/crates/discord-core"], ["rust/crates/discord-core/src/lib.rs"]]) {
      const plan = planFor(changed);
      const cargo = plan.commands.filter((c) => c.startsWith("cargo "));
      assert.deepEqual(cargo, [
        "cargo test --offline --manifest-path rust/Cargo.toml -p discord-core",
      ], `${changed}: cargo-команда должна быть targeted и offline`);
      assert.equal(plan.tier, "T1 (batch)", `${changed}: Rust-код — T1`);
      assertNoLivePlan(plan, String(changed));
    }
  });

  it("прочий крейт остаётся workspace-прогоном", () => {
    const plan = planFor(["rust/crates/discord-platform/src/lib.rs"]);
    assert.ok(
      plan.commands.includes("cargo test --offline --manifest-path rust/Cargo.toml --workspace"),
      "чужой крейт должен идти workspace-прогоном",
    );
  });
});

describe("test-impact: rust/gui-spike/ui/app.js", () => {
  it("общий UI-файл §5.2.3 → полный прогон T2 и геометрия", () => {
    const plan = planFor(["rust/gui-spike/ui/app.js"]);
    assert.equal(plan.tier, "T2 (release)", "app.js — общий файл из §5.2.3");
    assert.ok(has(plan, "npm test"), "app.js: нет полного JS-прогона");
    assert.ok(has(plan, "tests/unit/test-impact.test.mjs"), "app.js: нет контракта селектора");
    assert.ok(
      plan.commands.some((c) => c.includes("--manifest-path rust/gui-spike/Cargo.toml")),
      "app.js: нет прогона gui-spike",
    );
    assert.ok(has(plan, "geometry-check.mjs"), "app.js: геометрия не входит в npm test");
    assertNoLivePlan(plan, "app.js");
  });
});

describe("test-impact: src/cli", () => {
  it("каталог src/cli и файл дают cli-run + smoke, а не все unit-тесты", () => {
    for (const changed of [["src/cli"], ["src/cli/run.mjs"]]) {
      const plan = planFor(changed);
      assert.ok(has(plan, "tests/unit/cli-run.test.mjs"), `${changed}: нет cli-run`);
      assert.ok(has(plan, "tests/smoke/cli.smoke.test.mjs"), `${changed}: нет smoke`);
      assert.ok(!plan.commands.some((c) => c.includes("*")), `${changed}: wildcard-прогон`);
      assert.ok(!has(plan, "tests/unit/walk.test.mjs"), `${changed}: лишний walk`);
      assertNoLivePlan(plan, String(changed));
    }
  });
});

// QA-IMPACT-V2 (RW-40): TEST-STRATEGY-V2 §5.2.1 — каждому новому UI-модулю
// сопоставлен его профильный тест плюс общие интеграционные контракты
// gui-spike-dom и gui-spike-features. Оракул — реальный layout репозитория:
// модули RW-40 лежат в `rust/gui-spike/ui/features/<mod>.js` (BOARD.md, task-113),
// а не в корне `ui/`; профильные тесты — существующие файлы `tests/unit/`
// (chat-reactions.test.mjs импортирует features/reactions.js).
const UI_MODULE_TESTS = [
  {
    module: "rust/gui-spike/ui/features/notifications.js",
    tests: [
      "notifications-ui.test.mjs",
      "gui-spike-dom.test.mjs",
      "gui-spike-features.test.mjs",
    ],
  },
  {
    module: "rust/gui-spike/ui/features/voice-devices-settings.js",
    tests: [
      "voice-devices-settings.test.mjs",
      "gui-spike-dom.test.mjs",
      "gui-spike-features.test.mjs",
    ],
  },
  {
    module: "rust/gui-spike/ui/features/dm-friends.js",
    tests: [
      "dm-friends-ui.test.mjs",
      "gui-spike-dom.test.mjs",
      "gui-spike-features.test.mjs",
    ],
  },
  {
    module: "rust/gui-spike/ui/features/search.js",
    tests: [
      "search-remote-real-index.test.mjs",
      "gui-spike-dom.test.mjs",
      "gui-spike-features.test.mjs",
    ],
  },
  {
    module: "rust/gui-spike/ui/features/search-overlay.js",
    tests: [
      "search-remote-real-index.test.mjs",
      "gui-spike-dom.test.mjs",
      "gui-spike-features.test.mjs",
    ],
  },
  {
    // chat-reactions.test.mjs импортирует `features/reactions.js` (:31), поэтому
    // отдельного модуля `ui/chat-reactions.js` в репозитории нет и быть не должно.
    module: "rust/gui-spike/ui/features/reactions.js",
    tests: [
      "chat-reactions.test.mjs",
      "gui-spike-dom.test.mjs",
      "gui-spike-features.test.mjs",
    ],
  },
];

// Модули и профильные тесты RW-40, которые в этой ветке ещё не созданы смежными
// карточками (BOARD.md: task-113 и др.; TEST-STRATEGY-V2 §5.2.1). Пометка — не
// снимок ветки: она лишь разрешает отсутствие файла. Как только файл появился на
// диске, проверка идёт по нему, а команда в план попадает автоматически.
const PLANNED_MODULES = new Set([
  "rust/gui-spike/ui/features/notifications.js",
  "rust/gui-spike/ui/features/voice-devices-settings.js",
  "rust/gui-spike/ui/features/dm-friends.js",
]);

const PLANNED_TESTS = new Set([
  "notifications-ui.test.mjs",
  "voice-devices-settings.test.mjs",
  "dm-friends-ui.test.mjs",
  "attachments-render.test.mjs",
]);

// Профильные и общие тесты существующего модуля `features/chat.js` (§5.2.1).
const CHAT_TESTS = [
  "attachments-render.test.mjs",
  "channel-switch.test.mjs",
  "gui-spike-features.test.mjs",
  "gui-spike-model.test.mjs",
];

/** @param {string} test */
const unitPath = (test) => `tests/unit/${test}`;
/** @param {string} relative */
const onDisk = (relative) => existsSync(join(root, relative));

/**
 * Инвариант §5.2.1 вместо снимка ветки: каждый модуль таблицы либо лежит на диске,
 * либо помечен запланированным; каждый профильный тест либо существует, либо
 * помечен; пометки не ссылаются на ключи вне таблицы; правило выдаётся только для
 * `ui/features/`. Список «ровно N файлов на диске» здесь не проверяется — иначе
 * тест зеленел бы только на дереве без смежных карточек.
 * @param {readonly { module: string, tests: readonly string[] }[]} modules
 * @param {ReadonlySet<string>} plannedModules
 * @param {ReadonlySet<string>} plannedTests
 * @param {(relative: string) => boolean} exists
 * @param {readonly string[]} [extraTests] тесты правил вне таблицы (chat.js)
 * @returns {string[]} нарушения; пустой список — таблица корректна в этом layout
 */
function layoutProblems(modules, plannedModules, plannedTests, exists, extraTests = []) {
  const problems = [];
  const tableKeys = new Set(modules.map(({ module }) => module));
  const declaredTests = new Set([
    ...modules.flatMap(({ tests }) => tests),
    ...extraTests,
  ]);
  for (const { module } of modules) {
    if (!module.startsWith("rust/gui-spike/ui/features/")) {
      problems.push(`${module}: правило для файла вне ui/features/ — фантомный путь`);
    }
    if (!exists(module) && !plannedModules.has(module)) {
      problems.push(`${module}: файла нет на диске и он не помечен запланированным`);
    }
  }
  for (const module of plannedModules) {
    if (!tableKeys.has(module)) problems.push(`запланирован неизвестный модуль ${module}`);
  }
  for (const test of plannedTests) {
    if (!declaredTests.has(test)) problems.push(`запланирован неизвестный тест ${test}`);
  }
  for (const test of declaredTests) {
    if (!exists(unitPath(test)) && !plannedTests.has(test)) {
      problems.push(`${unitPath(test)}: теста нет на диске и он не помечен запланированным`);
    }
  }
  return problems;
}

describe("test-impact v2: точечный граф новых UI-модулей", () => {
  for (const entry of UI_MODULE_TESTS) {
    const { module } = entry;
    const existing = entry.tests.filter((test) => onDisk(unitPath(test)));
    it(`${module} → профильный тест + общие контракты`, () => {
      const plan = planFor([module]);
      assert.ok(has(plan, "npm run typecheck"), `${module}: нет typecheck`);
      assert.ok(existing.length > 0, `${module}: ни одного теста таблицы нет на диске`);
      for (const test of existing) {
        assert.ok(
          has(plan, unitPath(test)),
          `${module}: в плане нет ${unitPath(test)}: ${plan.commands.join(" | ")}`,
        );
      }
      for (const test of entry.tests.filter((test) => !existing.includes(test))) {
        assert.ok(
          !has(plan, unitPath(test)),
          `${module}: план выдаёт команду на отсутствующий ${unitPath(test)} (P2-3)`,
        );
      }
      const unitCommands = plan.commands.filter((c) => c.startsWith("node --test "));
      assert.equal(
        unitCommands.length,
        existing.length,
        `${module}: ожидались только тесты таблицы, лежащие на диске (${existing.length}), получено ${unitCommands.join(" | ")}`,
      );
      assert.equal(plan.tier, "T0 (targeted)", `${module}: UI-модуль не требует T1`);
      assert.ok(!has(plan, "geometry-check.mjs"), `${module}: лишняя геометрия`);
      assert.ok(!has(plan, "guild-rail.test.mjs"), `${module}: лишний полный frontend-прогон`);
      assert.ok(!has(plan, "channel-switch.test.mjs"), `${module}: лишний channel-switch`);
      assert.ok(!plan.commands.some((c) => c.includes("*")), `${module}: wildcard-прогон`);
      assertNoLivePlan(plan, module);
    });
  }

  it("каждый ключ — реальный layout ui/features/<mod>.js", () => {
    for (const { module } of UI_MODULE_TESTS) {
      assert.ok(
        module.startsWith("rust/gui-spike/ui/features/"),
        `${module}: модули RW-40 лежат в ui/features/, плоский ui/<mod>.js — фантомный путь`,
      );
    }
  });

  it("таблица согласована с диском: модуль либо есть, либо запланирован", () => {
    assert.deepEqual(
      layoutProblems(UI_MODULE_TESTS, PLANNED_MODULES, PLANNED_TESTS, onDisk, CHAT_TESTS),
      [],
      "таблица должна описывать текущий layout без снимка «ровно N файлов»",
    );
  });

  it("после интеграции смежных карточек инвариант остаётся зелёным", () => {
    // Симуляция дерева после merge (REVIEW-PASS2 §2, P1-1): все шесть модулей
    // таблицы и все их профильные тесты существуют, пометки «запланирован»
    // устарели и не должны ничего ломать.
    const merged = new Set([
      ...UI_MODULE_TESTS.flatMap(({ module, tests }) => [module, ...tests.map(unitPath)]),
      ...CHAT_TESTS.map(unitPath),
    ]);
    /** @param {string} relative */
    const exists = (relative) => merged.has(relative);
    assert.deepEqual(
      layoutProblems(UI_MODULE_TESTS, PLANNED_MODULES, PLANNED_TESTS, exists, CHAT_TESTS),
      [],
    );
  });

  it("фантомный ключ и плоский путь ловятся инвариантом, а не снимком", () => {
    const phantom = layoutProblems(
      [{ module: "rust/gui-spike/ui/features/notificationz.js", tests: ["gui-spike-dom.test.mjs"] }],
      PLANNED_MODULES,
      PLANNED_TESTS,
      onDisk,
    );
    assert.ok(
      phantom.some((problem) => problem.includes("notificationz")),
      `опечатка в ключе должна ломать инвариант: ${phantom.join(" | ")}`,
    );
    const flat = layoutProblems(
      [{ module: "rust/gui-spike/ui/notifications.js", tests: ["notifications-ui.test.mjs"] }],
      PLANNED_MODULES,
      PLANNED_TESTS,
      onDisk,
    );
    assert.ok(
      flat.some((problem) => problem.includes("вне ui/features/")),
      `плоский путь должен ломать инвариант: ${flat.join(" | ")}`,
    );
  });

  it("фантомный плоский путь ui/<mod>.js не получает точечное правило", () => {
    for (const { module, tests } of UI_MODULE_TESTS) {
      const flat = module.replace("/features/", "/");
      const plan = planFor([flat]);
      assert.ok(
        !has(plan, `tests/unit/${tests[0]}`),
        `${flat}: путь-фантом не должен получать профильный ${tests[0]}`,
      );
    }
  });

  it("профильные тесты таблицы либо в tests/unit, либо помечены запланированными", () => {
    for (const test of UI_MODULE_TESTS.flatMap(({ tests }) => tests)) {
      assert.ok(
        onDisk(unitPath(test)) || PLANNED_TESTS.has(test),
        `${unitPath(test)}: нет файла и нет пометки «запланирован»`,
      );
    }
    for (const test of ["chat-reactions.test.mjs", "search-remote-real-index.test.mjs"]) {
      assert.ok(onDisk(unitPath(test)), `${unitPath(test)}: профильный тест потерялся`);
    }
  });

  it("features/chat.js расширен профилем вложений и общими контрактами", () => {
    const plan = planFor(["rust/gui-spike/ui/features/chat.js"]);
    for (const test of [
      "attachments-render.test.mjs",
      "channel-switch.test.mjs",
      "gui-spike-features.test.mjs",
      "gui-spike-model.test.mjs",
    ]) {
      if (onDisk(unitPath(test))) {
        assert.ok(has(plan, unitPath(test)), `chat.js: нет ${unitPath(test)}`);
      } else {
        assert.ok(
          PLANNED_TESTS.has(test),
          `chat.js: ${unitPath(test)} нет на диске и нет пометки «запланирован»`,
        );
        assert.ok(
          !has(plan, unitPath(test)),
          `chat.js: план выдаёт команду на отсутствующий ${unitPath(test)} (P2-3)`,
        );
      }
    }
    assert.ok(has(plan, unitPath("channel-switch.test.mjs")), "chat.js: потерян channel-switch");
    assert.equal(plan.tier, "T0 (targeted)", "chat.js не требует T1");
    assert.ok(!has(plan, "geometry-check.mjs"), "chat.js: лишняя геометрия");
    assertNoLivePlan(plan, "features/chat.js");
  });

  it("search.js и search-overlay.js дают одинаковый целевой набор", () => {
    const search = planFor(["rust/gui-spike/ui/features/search.js"]);
    const overlay = planFor(["rust/gui-spike/ui/features/search-overlay.js"]);
    assert.deepEqual(
      [...overlay.commands].sort(),
      [...search.commands].sort(),
      "search-overlay.js должен идти тем же набором, что и search.js",
    );
  });

  it("несколько новых модулей объединяются без потери профильных тестов", () => {
    const plan = planFor([
      "rust/gui-spike/ui/features/notifications.js",
      "rust/gui-spike/ui/features/reactions.js",
    ]);
    for (const test of ["notifications-ui.test.mjs", "chat-reactions.test.mjs", "gui-spike-dom.test.mjs"]) {
      assert.ok(
        onDisk(unitPath(test)) || PLANNED_TESTS.has(test),
        `${unitPath(test)}: нет файла и нет пометки «запланирован»`,
      );
      if (!onDisk(unitPath(test))) continue;
      assert.ok(has(plan, unitPath(test)), `объединённый план без ${test}`);
    }
    assert.ok(has(plan, unitPath("chat-reactions.test.mjs")), "объединённый план без существующего профиля реакций");
    assert.ok(!has(plan, "geometry-check.mjs"), "объединённый план не должен тянуть геометрию");
    assertNoLivePlan(plan, "notifications+reactions");
  });
});

// QA-IMPACT-V2 (RW-40): TEST-STRATEGY-V2 §5.2.2 — каждому модулю BFF
// сопоставляется его контрактный таргет `--test <mod>_contract` плюс
// `ipc_contract` (wiring-контракт крейта; в стратегии он назван
// `ipc_wiring_contract`, в репозитории файл — tests/ipc_contract.rs).
const CARGO_SPIKE = "cargo test --offline --manifest-path rust/gui-spike/Cargo.toml";

/** Собирает ожидаемую cargo-команду из таргетов. @param {string[]} targets */
const cargoTargets = (targets) =>
  `${CARGO_SPIKE} ${targets.map((target) => `--test ${target}`).join(" ")}`;

const RUST_BFF_MODULES = [
  { module: "attachments.rs", contract: "attachments_contract" },
  { module: "auth.rs", contract: "auth_contract" },
  { module: "core_bridge.rs", contract: "core_bridge" },
  { module: "data.rs", contract: "data_contract" },
  { module: "dm_friends.rs", contract: "dm_friends_contract" },
  { module: "guilds.rs", contract: "guilds_contract" },
  { module: "invites.rs", contract: "invites_contract" },
  { module: "messages.rs", contract: "messages_contract" },
  { module: "net.rs", contract: "net_contract" },
  { module: "notifications.rs", contract: "notifications_contract" },
  { module: "pins_threads.rs", contract: "pins_threads_contract" },
  { module: "reactions.rs", contract: "reactions_contract" },
  { module: "rest_failure.rs", contract: "rest_failure_contract" },
  { module: "search.rs", contract: "search_contract" },
  { module: "settings_store.rs", contract: "settings_contract" },
  { module: "typing.rs", contract: "typing_contract" },
  { module: "voice_devices.rs", contract: "voice_devices_contract" },
];

describe("test-impact v2: точечный граф Rust BFF", () => {
  for (const { module, contract } of RUST_BFF_MODULES) {
    it(`src/${module} → ${contract} + ipc_contract`, () => {
      const plan = planFor([`rust/gui-spike/src/${module}`]);
      const cargo = plan.commands.filter((c) => c.startsWith("cargo "));
      assert.deepEqual(
        cargo,
        [cargoTargets([contract, "ipc_contract"])],
        `src/${module}: ожидался только контрактный таргет, без полного прогона крейта`,
      );
      assert.equal(plan.tier, "T1 (batch)", `src/${module}: Rust-код — T1`);
      assertNoLivePlan(plan, module);
    });
  }

  it("каждый таргет контракта существует в rust/gui-spike/tests", () => {
    for (const { module, contract } of RUST_BFF_MODULES) {
      for (const target of [contract, "ipc_contract"]) {
        assert.ok(
          existsSync(join(root, "rust/gui-spike/tests", `${target}.rs`)),
          `src/${module}: нет таргета rust/gui-spike/tests/${target}.rs`,
        );
      }
    }
  });

  it("каждый pub mod из lib.rs покрыт правилом", () => {
    const lib = readFileSync(join(root, "rust/gui-spike/src/lib.rs"), "utf8");
    const mods = [...lib.matchAll(/^pub mod (\w+);$/gm)].map((match) => String(match[1]));
    assert.ok(mods.length >= 10, `в lib.rs найдено мало модулей: ${mods.join(", ")}`);
    // Исключения — по именам реальных файлов tests/*.rs, а не по выдуманной схеме.
    /** @type {Record<string, string>} */
    const CONTRACT_NAME = { core_bridge: "core_bridge", settings_store: "settings_contract" };
    for (const mod of mods) {
      const target = CONTRACT_NAME[mod] ?? `${mod}_contract`;
      const plan = planFor([`rust/gui-spike/src/${mod}.rs`]);
      assert.ok(
        has(plan, `--test ${target}`),
        `lib.rs: pub mod ${mod} без таргета --test ${target}: ${plan.commands.join(" | ")}`,
      );
    }
  });

  it("lib.rs и main.rs дают полный прогон крейта, а не отдельный таргет", () => {
    for (const module of ["rust/gui-spike/src/lib.rs", "rust/gui-spike/src/main.rs"]) {
      const plan = planFor([module]);
      const cargo = plan.commands.filter((c) => c.startsWith("cargo "));
      assert.ok(cargo.includes(CARGO_SPIKE), `${module}: ожидался полный прогон крейта gui-spike`);
      assert.ok(
        !cargo.some((c) => c.includes("--test ")),
        `${module}: отдельный таргет не заменяет полный прогон: ${cargo.join(" | ")}`,
      );
    }
  });

  it("новый src/<mod>.rs без правила не теряет контракты — полный прогон", () => {
    const plan = planFor(["rust/gui-spike/src/unmapped_future_mod.rs"]);
    const cargo = plan.commands.filter((c) => c.startsWith("cargo "));
    assert.deepEqual(cargo, [CARGO_SPIKE], "неизвестный модуль BFF должен идти полным прогоном крейта");
  });
});

// QA-IMPACT-V2 (RW-40): TEST-STRATEGY-V2 §5.2.3 — safety net для общих файлов.
// Правка самого селектора обязана запускать его собственный контракт.
const SAFETY_NET_FILES = [
  "package.json",
  "scripts/test-impact.mjs",
  "rust/gui-spike/Cargo.toml",
  "rust/gui-spike/src/lib.rs",
  "rust/gui-spike/src/main.rs",
  "rust/gui-spike/ui/app.js",
];

describe("test-impact v2: safety net общих файлов", () => {
  for (const file of SAFETY_NET_FILES) {
    it(`${file} → полный прогон T2`, () => {
      const plan = planFor([file]);
      assert.equal(plan.tier, "T2 (release)", `${file}: safety net должен давать T2`);
      assert.ok(has(plan, "npm run typecheck"), `${file}: нет typecheck`);
      assert.ok(has(plan, "tests/unit/test-impact.test.mjs"), `${file}: селектор не проверяет свой контракт`);
      assert.ok(has(plan, "npm test"), `${file}: нет полного JS-прогона`);
      assert.ok(
        plan.commands.some((c) => c.includes("--manifest-path rust/gui-spike/Cargo.toml")),
        `${file}: нет полного прогона gui-spike`,
      );
      assert.ok(
        plan.commands.some((c) => c.includes("--manifest-path rust/Cargo.toml --workspace")),
        `${file}: нет прогона Rust workspace`,
      );
      assertNoLivePlan(plan, file);
    });
  }

  it("обычный модуль не получает release-набор", () => {
    const plan = planFor(["rust/gui-spike/ui/features/search.js"]);
    assert.equal(plan.tier, "T0 (targeted)", "рядовой UI-модуль не должен становиться T2");
    assert.ok(!has(plan, "npm test"), "рядовой модуль не должен тянуть полный прогон");
  });
});

describe("test-impact: fallback и tier", () => {
  it("неизвестный путь переключает полный прогон §5.2.3", () => {
    const plan = planFor(["LICENSE"]);
    assert.equal(plan.tier, "T2 (release)", "неизвестный путь — режим all: true");
    assert.ok(has(plan, "tests/unit/test-impact.test.mjs"), "нет контракта селектора");
    assert.ok(has(plan, "npm test"), "нет полного JS-прогона");
    assert.ok(
      plan.commands.some((c) => c.includes("--manifest-path rust/gui-spike/Cargo.toml")),
      "нет полного прогона gui-spike",
    );
    assert.ok(
      plan.commands.some((c) => c.includes("--manifest-path rust/Cargo.toml --workspace")),
      "нет прогона Rust workspace",
    );
    assertNoLivePlan(plan, "LICENSE");
  });

  it("--full даёт tier T2 и offline release-набор", () => {
    const plan = runCli(["--full"]);
    assert.match(plan.tier, /^T2/, `--full tier: ${plan.tier}`);
    assert.ok(plan.commands.includes("npm run gate:release"), "нет gate:release");
    assert.ok(plan.commands.some((c) => c.includes("cargo test --offline --workspace")), "cargo не offline/workspace");
    assertNoLivePlan(plan, "--full");
  });

  it("флаги после --changed не считаются изменёнными файлами", () => {
    const plan = runCli(["--changed", "rust/gui-spike/ui/features/join-server.js", "--json"]);
    assert.deepEqual(plan.changed, ["rust/gui-spike/ui/features/join-server.js"]);
    assert.ok(
      !plan.commands.some((c) => c.includes("tests/meta/plan-sync.test.mjs")),
      "--json попал в changed и добавил fallback",
    );
  });
});
