#!/usr/bin/env node
// @ts-check
// Test impact selector: выдаёт минимальный набор проверок по изменённым файлам.
// Не запускает тесты и не ослабляет gate. Использование:
//   node scripts/test-impact.mjs --changed rust/gui-spike/ui/features/chat.js
//   node scripts/test-impact.mjs            # git status
//   node scripts/test-impact.mjs --full     # release/T2
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));

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

// TEST-STRATEGY-V2 §5.2.1: профильный тест модуля + общие интеграционные контракты.
// Первый в списке — профильный тест, остальные — общий UI-контракт.
// Пути — реальный layout: модули RW-40 живут в `ui/features/<mod>.js` (BOARD.md,
// task-113); плоский `ui/<mod>.js` — фантомный путь, его в таблице быть не должно.
/** @type {Record<string, readonly string[]>} */
const UI_MODULE_TESTS = {
  "rust/gui-spike/ui/features/notifications.js": [
    "tests/unit/notifications-ui.test.mjs",
    "tests/unit/gui-spike-dom.test.mjs",
    "tests/unit/gui-spike-features.test.mjs",
  ],
  "rust/gui-spike/ui/features/voice-devices-settings.js": [
    "tests/unit/voice-devices-settings.test.mjs",
    "tests/unit/gui-spike-dom.test.mjs",
    "tests/unit/gui-spike-features.test.mjs",
  ],
  "rust/gui-spike/ui/features/dm-friends.js": [
    "tests/unit/dm-friends-ui.test.mjs",
    "tests/unit/gui-spike-dom.test.mjs",
    "tests/unit/gui-spike-features.test.mjs",
  ],
  "rust/gui-spike/ui/features/search.js": [
    "tests/unit/search-remote-real-index.test.mjs",
    "tests/unit/gui-spike-dom.test.mjs",
    "tests/unit/gui-spike-features.test.mjs",
  ],
  "rust/gui-spike/ui/features/search-overlay.js": [
    "tests/unit/search-remote-real-index.test.mjs",
    "tests/unit/gui-spike-dom.test.mjs",
    "tests/unit/gui-spike-features.test.mjs",
  ],
  // chat-reactions.test.mjs импортирует `features/reactions.js`; отдельного
  // модуля `ui/chat-reactions.js` в репозитории нет.
  "rust/gui-spike/ui/features/reactions.js": [
    "tests/unit/chat-reactions.test.mjs",
    "tests/unit/gui-spike-dom.test.mjs",
    "tests/unit/gui-spike-features.test.mjs",
  ],
};

// TEST-STRATEGY-V2 §5.2.2: модуль BFF → контрактный таргет `--test <mod>_contract`.
// Исключения взяты из имён реальных файлов `rust/gui-spike/tests/*.rs`
// (`settings_store.rs → settings_contract`, `core_bridge.rs → core_bridge`);
// `ipc_contract` (wiring-контракт; в стратегии — `ipc_wiring_contract`) добавляется всегда.
const CARGO_SPIKE = "cargo test --offline --manifest-path rust/gui-spike/Cargo.toml";

/** @type {Record<string, string>} */
const RUST_MODULE_CONTRACTS = {
  "rust/gui-spike/src/attachments.rs": "attachments_contract",
  "rust/gui-spike/src/auth.rs": "auth_contract",
  "rust/gui-spike/src/automod.rs": "automod_contract",
  "rust/gui-spike/src/core_bridge.rs": "core_bridge",
  "rust/gui-spike/src/data.rs": "data_contract",
  "rust/gui-spike/src/dm_channels.rs": "dm_channels_contract",
  "rust/gui-spike/src/dm_friends.rs": "dm_friends_contract",
  "rust/gui-spike/src/expressions.rs": "expressions_contract",
  "rust/gui-spike/src/forum.rs": "forum_contract",
  "rust/gui-spike/src/guilds.rs": "guilds_contract",
  "rust/gui-spike/src/invites.rs": "invites_contract",
  "rust/gui-spike/src/ipc_ext.rs": "ipc_ext_contract",
  "rust/gui-spike/src/messages.rs": "messages_contract",
  "rust/gui-spike/src/moderation.rs": "moderation_contract",
  "rust/gui-spike/src/net.rs": "net_contract",
  "rust/gui-spike/src/notifications.rs": "notifications_contract",
  "rust/gui-spike/src/pins_threads.rs": "pins_threads_contract",
  "rust/gui-spike/src/polls.rs": "polls_contract",
  "rust/gui-spike/src/profile.rs": "profile_contract",
  "rust/gui-spike/src/profile_ext.rs": "profile_ext_contract",
  "rust/gui-spike/src/reactions.rs": "reactions_contract",
  "rust/gui-spike/src/relationships.rs": "relationships_contract",
  "rust/gui-spike/src/rest_failure.rs": "rest_failure_contract",
  "rust/gui-spike/src/roles.rs": "roles_contract",
  "rust/gui-spike/src/scheduled_events.rs": "scheduled_events_contract",
  "rust/gui-spike/src/search.rs": "search_contract",
  "rust/gui-spike/src/search_bff.rs": "search_bff_contract",
  "rust/gui-spike/src/settings_store.rs": "settings_contract",
  "rust/gui-spike/src/typing.rs": "typing_contract",
  "rust/gui-spike/src/voice_devices.rs": "voice_devices_contract",
  "rust/gui-spike/src/voice_stage.rs": "voice_stage_contract",
  "rust/gui-spike/src/webhooks.rs": "webhooks_contract",
  "rust/gui-spike/src/dto.rs": "dto_contract",
  "rust/gui-spike/src/permissions.rs": "permissions_contract",
  "rust/gui-spike/src/voice_diagnostics.rs": "voice_diagnostics_contract",
  "rust/gui-spike/src/bff_domains_router.rs": "bff_domains_router_contract",
  "rust/gui-spike/src/onboarding.rs": "onboarding_contract",
  "rust/gui-spike/src/slash_commands.rs": "slash_commands_contract",
  "rust/gui-spike/src/soundboard.rs": "soundboard_contract",
  "rust/gui-spike/src/stage_instances.rs": "stage_instances_contract",
};

// TEST-STRATEGY-V2 §5.2.3: полный прогон для общих файлов и для самого селектора.
// `npm test` — все `tests/**/*.test.mjs`; отдельная строка на контракт селектора,
// чтобы правка `scripts/test-impact.mjs` гарантированно его запускала.
const FULL_RUN = [
  "node --test tests/unit/test-impact.test.mjs",
  "npm test",
  CARGO_SPIKE,
  "cargo test --offline --manifest-path rust/Cargo.toml --workspace",
];

const GEOMETRY_CHECK = "node waves/RW-39/PM/w39c/geometry-check.mjs";

/**
 * Общие файлы из §5.2.3 (включая `ui/app.js`), правка которых требует полного прогона.
 * @param {string} file
 * @returns {boolean}
 */
const isSafetyNet = (file) =>
  file === "package.json" ||
  file === "package-lock.json" ||
  under(file, "scripts/") ||
  file === "rust/gui-spike/Cargo.toml" ||
  file === "rust/Cargo.toml" ||
  file === "rust/gui-spike/src/lib.rs" ||
  file === "rust/gui-spike/src/main.rs" ||
  file === "rust/gui-spike/ui/app.js";

/**
 * Профильный тест попадает в план, только если файл существует: тесты смежных
 * карточек (`notifications-ui`, `attachments-render`, ...) появляются вместе со
 * своими модулями, а команда на отсутствующий файл — гарантированное падение
 * плана (§5.2.1, P2-3).
 * @param {string} test
 * @returns {boolean}
 */
const testOnDisk = (test) => existsSync(join(ROOT, test));

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
    add(GEOMETRY_CHECK, `${file}: геометрия`);
  };

  /** @param {string} test @param {string} why */
  const addTest = (test, why) => {
    if (testOnDisk(test)) add(`node --test ${test}`, why);
  };

  /** @type {string | null} */
  let safetyNet = null;
  for (const raw of changed.map(norm)) {
    if (raw === "") continue;
    if (isSafetyNet(raw)) {
      safetyNet = safetyNet ?? raw;
      for (const command of FULL_RUN) add(command, `${raw}: safety net §5.2.3 — полный прогон`);
      // `npm test` не содержит geometry-check, поэтому для общего UI-файла проверка
      // геометрии остаётся отдельной строкой.
      if (raw === "rust/gui-spike/ui/app.js") {
        add(GEOMETRY_CHECK, `${raw}: геометрия (не входит в npm test)`);
      }
    } else if (under(raw, "waves/")) {
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
      addTest("tests/unit/attachments-render.test.mjs", `${raw}: вложения (features/chat.js)`);
      addTest("tests/unit/channel-switch.test.mjs", `${raw}: channel switch`);
      addTest("tests/unit/gui-spike-features.test.mjs", `${raw}: chat UI`);
      addTest("tests/unit/gui-spike-model.test.mjs", `${raw}: heights`);
    } else if (Object.hasOwn(RUST_MODULE_CONTRACTS, raw)) {
      const contract = RUST_MODULE_CONTRACTS[raw] ?? "";
      add(
        `${CARGO_SPIKE} --test ${contract} --test ipc_contract`,
        `${raw}: BFF-контракт ${contract} + ipc_contract`,
      );
    } else if (Object.hasOwn(UI_MODULE_TESTS, raw)) {
      const moduleTests = UI_MODULE_TESTS[raw] ?? [];
      for (const [index, test] of moduleTests.entries()) {
        addTest(
          test,
          `${raw}: ${index === 0 ? "профильный UI-модуль" : "общий UI-контракт"}`,
        );
      }
    } else if (under(raw, "rust/gui-spike/ui/")) {
      frontend(raw);
    } else if (under(raw, "rust/gui-spike/src/")) {
      add("cargo test --offline --manifest-path rust/gui-spike/Cargo.toml", `${raw}: gui backend`);
    } else if (under(raw, "rust/crates/chord-core/")) {
      add("cargo test --offline --manifest-path rust/Cargo.toml -p chord-core", `${raw}: core`);
    } else if (under(raw, "rust/crates/")) {
      add("cargo test --offline --manifest-path rust/Cargo.toml --workspace", `${raw}: crate`);
    } else if (raw.startsWith("rust/")) {
      add("cargo test --offline --manifest-path rust/Cargo.toml --workspace", `${raw}: Rust workspace`);
    } else {
      // §5.2.3: неизвестный путь → режим `all: true` (полный прогон), а не молчаливый
      // meta-фоллбэк, который пропускает unit- и Rust-прогоны.
      safetyNet = safetyNet ?? raw;
      for (const command of FULL_RUN) {
        add(command, `${raw}: неизвестный путь §5.2.3 — полный прогон`);
      }
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
    tier: safetyNet ? "T2 (release)" : sensitive ? "T1 (batch)" : "T0 (targeted)",
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
