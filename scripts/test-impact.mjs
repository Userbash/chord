#!/usr/bin/env node
// @ts-check
// Test impact selector: выдаёт минимальный набор проверок по изменённым файлам.
// Не запускает тесты и не ослабляет gate. Использование:
//   node scripts/test-impact.mjs --changed rust/gui-spike/ui/features/chat.js
//   node scripts/test-impact.mjs            # git status
//   node scripts/test-impact.mjs --full     # release/T2
import { execFileSync } from "node:child_process";
import { pathToFileURL } from "node:url";

const FRONTEND = [
  "tests/unit/gui-spike-dom.test.mjs",
  "tests/unit/gui-spike-model.test.mjs",
  "tests/unit/gui-spike-bench.test.mjs",
  "tests/unit/gui-spike-features.test.mjs",
  "tests/unit/gui-spike-auth.test.mjs",
  "tests/unit/gui-spike-regressions.test.mjs",
  "tests/unit/guild-rail.test.mjs",
  "tests/unit/voice-ui.test.mjs",
  "tests/unit/channel-switch.test.mjs",
];

const CORE_UI = [
  "tests/unit/walk.test.mjs",
  "tests/unit/walk-log.test.mjs",
  "tests/unit/cli-run.test.mjs",
  "tests/unit/cdp.test.mjs",
];

/** @param {string} value */
const norm = (value) => value.replaceAll("\\", "/").replace(/^\.\//, "");

/**
 * Путь внутри каталога: сам каталог (`docs`) или файл в нём (`docs/map.md`).
 * @param {string} file
 * @param {string} dir
 * @returns {boolean}
 */
const under = (file, dir) => file === dir.replace(/\/$/, "") || file.startsWith(dir);

/**
 * @param {readonly string[]} changed
 * @returns {{ tier: string, commands: string[], reasons: Record<string,string> }}
 */
export function planFor(changed) {
  /** @type {Set<string>} */
  const commands = new Set(["npm run typecheck"]);
  /** @type {Record<string,string>} */
  const reasons = {};
  /** @param {string} command @param {string} why */
  const add = (command, why) => {
    commands.add(command);
    reasons[command] = reasons[command] ? `${reasons[command]}; ${why}` : why;
  };
  /** @param {string} file */
  const frontend = (file) => {
    add(
      `node --test ${FRONTEND.join(" ")}`,
      `${file}: frontend-контракт (shared DOM/state)`,
    );
    add("node waves/RW-39/PM/w39c/geometry-check.mjs", `${file}: геометрия`);
  };

  for (const raw of changed.map(norm)) {
    if (raw === "") continue;
    if (under(raw, "waves/")) {
      add("node --test tests/meta/waves-sync.test.mjs", `${raw}: журнал волн`);
    } else if (under(raw, "docs/") || raw === "README.md") {
      add("node --test tests/meta/docs-sync.test.mjs", `${raw}: docs-sync`);
    } else if (under(raw, "tests/meta/")) {
      add("node --test tests/meta/*.test.mjs", `${raw}: meta-контракт`);
    } else if (raw === "tests/unit/gui-spike-features.test.mjs") {
      frontend(raw);
      add("node --test tests/unit/gui-spike-features.test.mjs", `${raw}: доменный файл`);
    } else if (raw === "tests/unit/gui-spike-auth.test.mjs") {
      add("node --test tests/unit/gui-spike-auth.test.mjs", `${raw}: auth`);
    } else if (under(raw, "tests/unit/")) {
      add(`node --test ${raw}`, `${raw}: затронутый unit`);
      add("node --test tests/meta/plan-sync.test.mjs", `${raw}: §8/plan-sync`);
    } else if (under(raw, "tests/")) {
      add("node --test tests/meta/plan-sync.test.mjs tests/meta/test-policy.test.mjs", `${raw}: test-policy/§8`);
    } else if (under(raw, "src/core/cdp/")) {
      add("node --test tests/unit/cdp.test.mjs", `${raw}: CDP`);
    } else if (under(raw, "src/core/ui/")) {
      add(`node --test ${CORE_UI.join(" ")}`, `${raw}: walk/CLI`);
    } else if (under(raw, "src/cli/")) {
      add("node --test tests/unit/cli-run.test.mjs", `${raw}: CLI routing`);
      add("node --test tests/smoke/cli.smoke.test.mjs", `${raw}: CLI binary`);
    } else if (raw === "src/core/types.js") {
      add(`node --test ${CORE_UI.join(" ")}`, `${raw}: типы CLI`);
    } else if (under(raw, "src/")) {
      add("node --test tests/unit/*.test.mjs", `${raw}: затронутый src-модуль`);
    } else if (raw.startsWith("rust/gui-spike/ui/tokens.css")) {
      frontend(raw);
      add("node --test tests/meta/design-breakpoints.test.mjs", `${raw}: breakpoints`);
    } else if (raw.startsWith("rust/gui-spike/ui/app.js") || raw.startsWith("rust/gui-spike/ui/index.html")) {
      frontend(raw);
    } else if (raw.startsWith("rust/gui-spike/ui/features/join-server.js")) {
      add("node --test tests/unit/join-server.test.mjs", `${raw}: invite UI`);
      add("node --test tests/unit/gui-spike-features.test.mjs", `${raw}: shared UI`);
    } else if (raw.startsWith("rust/gui-spike/ui/features/voice.js")) {
      add("node --test tests/unit/voice-ui.test.mjs", `${raw}: voice UI`);
    } else if (raw.startsWith("rust/gui-spike/ui/features/chat.js")) {
      add("node --test tests/unit/channel-switch.test.mjs", `${raw}: channel switch`);
      add("node --test tests/unit/gui-spike-features.test.mjs", `${raw}: chat UI`);
      add("node --test tests/unit/gui-spike-model.test.mjs", `${raw}: heights`);
    } else if (under(raw, "rust/gui-spike/ui/")) {
      frontend(raw);
    } else if (under(raw, "rust/gui-spike/src/")) {
      add("cargo test --offline --manifest-path rust/gui-spike/Cargo.toml", `${raw}: gui backend`);
    } else if (under(raw, "rust/crates/discord-core/")) {
      add("cargo test --offline --manifest-path rust/Cargo.toml -p discord-core", `${raw}: core`);
    } else if (under(raw, "rust/crates/")) {
      add("cargo test --offline --manifest-path rust/Cargo.toml --workspace", `${raw}: crate`);
    } else if (raw.startsWith("rust/")) {
      add("cargo test --offline --manifest-path rust/Cargo.toml --workspace", `${raw}: Rust workspace`);
    } else {
      add("node --test tests/meta/plan-sync.test.mjs", `${raw}: fallback meta`);
    }
  }

  const sensitive = changed.map(norm).some((file) =>
    file === "rust/gui-spike/ui/app.js" ||
    file === "rust/gui-spike/ui/tokens.css" ||
    under(file, "rust/gui-spike/src/") ||
    under(file, "rust/crates/") ||
    under(file, "src/")
  );
  return {
    tier: sensitive ? "T1 (batch)" : "T0 (targeted)",
    commands: [...commands],
    reasons,
  };
}

/** @returns {string[]} */
function changedFromGit() {
  const raw = execFileSync("git", ["status", "--porcelain"], { encoding: "utf8" });
  return raw
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map((line) => line.slice(3).trim())
    .filter(Boolean);
}

function main() {
  const args = process.argv.slice(2);
  if (args.includes("--full")) {
    console.log(JSON.stringify({
      tier: "T2 (release)",
      commands: [
        "npm run typecheck",
        "npm run gate:release",
        "cd rust && cargo test --offline --workspace",
      ],
      reasons: { full: "--full: release/T2" },
    }, null, 2));
    return;
  }
  const at = args.indexOf("--changed");
  const changed = at >= 0
    ? args.slice(at + 1).filter((value) => !value.startsWith("--"))
    : changedFromGit();
  const plan = planFor(changed);
  if (args.includes("--json")) console.log(JSON.stringify({ changed, ...plan }, null, 2));
  else {
    console.log(`tier: ${plan.tier}`);
    console.log(`changed: ${changed.join(", ") || "(нет)"}`);
    for (const command of plan.commands) console.log(`  $ ${command}  # ${plan.reasons[command] ?? ""}`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
