// @ts-check
// QA-IMPACT (RW-40): контракт селектора тестов scripts/test-impact.mjs.
// Проверяются только правила выбора команд: без live, без сети, без спавна окна.
// Каждый ожидаемый набор сверяется с независимым оракулом (пути/имена тестов),
// а не с текущим выводом скрипта.

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
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
  it("даёт полный frontend-контракт и геометрию", () => {
    const plan = planFor(["rust/gui-spike/ui/app.js"]);
    const frontend = plan.commands.filter((c) => c.startsWith("node --test "));
    assert.equal(frontend.length, 1, `ожидалась одна frontend-команда: ${plan.commands.join(" | ")}`);
    const cmd = frontend[0] ?? "";
    for (const test of [
      "gui-spike-dom.test.mjs",
      "gui-spike-model.test.mjs",
      "gui-spike-features.test.mjs",
      "guild-rail.test.mjs",
      "voice-ui.test.mjs",
      "channel-switch.test.mjs",
    ]) {
      assert.ok(cmd.includes(test), `frontend-прогон без ${test}`);
    }
    assert.ok(has(plan, "geometry-check.mjs"), "нет проверки геометрии");
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

describe("test-impact: fallback и tier", () => {
  it("неизвестный файл уходит в meta plan-sync", () => {
    const plan = planFor(["LICENSE"]);
    assert.ok(has(plan, "npm run typecheck"), "нет typecheck");
    assert.ok(has(plan, "tests/meta/plan-sync.test.mjs"), "нет fallback plan-sync");
    assert.equal(plan.commands.length, 2, `fallback не должен разрастаться: ${plan.commands.join(" | ")}`);
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
