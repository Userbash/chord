// @ts-check
// TR-194A: поведенческая сверка новых правил scripts/test-impact.mjs с диском.
import { existsSync } from "node:fs";
import { planFor } from "/home/sanya/orca/workspaces/project/qa-task-283-tsconfig-and-test-impact/scripts/test-impact.mjs";

const root = "/home/sanya/orca/workspaces/project/qa-task-283-tsconfig-and-test-impact";

const RUST = [
  "automod", "bff_domains_router", "dm_channels", "expressions", "forum", "ipc_ext",
  "moderation", "onboarding", "polls", "profile", "profile_ext", "relationships", "roles",
  "scheduled_events", "slash_commands", "soundboard", "stage_instances", "webhooks",
];
const UI = [
  "shell-controllers-registry", "chat-helpers-bridge", "composer-popovers-bridge", "app-shell-wiring",
];

let failures = 0;
for (const mod of RUST) {
  const plan = planFor([`rust/gui-spike/src/${mod}.rs`]);
  const cargo = plan.commands.filter((c) => c.startsWith("cargo ") && c.includes("--test "));
  const okTarget = cargo.length === 1 &&
    cargo[0].endsWith(`--test ${mod}_contract --test ipc_contract`);
  const okDisk = existsSync(`${root}/rust/gui-spike/tests/${mod}_contract.rs`) &&
    existsSync(`${root}/rust/gui-spike/src/${mod}.rs`);
  const okTier = plan.tier === "T1 (batch)";
  const ok = okTarget && okDisk && okTier;
  if (!ok) failures += 1;
  console.log(`${ok ? "OK  " : "FAIL"} rust/gui-spike/src/${mod}.rs → ${cargo.join(" ")} | tier=${plan.tier} | files=${okDisk}`);
}

for (const mod of UI) {
  const plan = planFor([`rust/gui-spike/ui/features/${mod}.js`]);
  const tests = plan.commands.filter((c) => c.startsWith("node --test ")).map((c) => c.replace("node --test ", ""));
  const expected = [
    `tests/unit/${mod}.test.mjs`,
    "tests/unit/gui-spike-dom.test.mjs",
    "tests/unit/gui-spike-features.test.mjs",
  ];
  const okTests = JSON.stringify(tests) === JSON.stringify(expected);
  const okDisk = expected.every((t) => existsSync(`${root}/${t}`));
  const okNoGeometry = !plan.commands.some((c) => c.includes("geometry-check.mjs"));
  const okTier = plan.tier === "T0 (targeted)";
  const ok = okTests && okDisk && okNoGeometry && okTier;
  if (!ok) failures += 1;
  console.log(`${ok ? "OK  " : "FAIL"} ui/features/${mod}.js → ${tests.join(", ")} | tier=${plan.tier} | geometry=${!okNoGeometry} | files=${okDisk}`);
}
console.log(`\nитого: ${failures === 0 ? "18+4 OK" : `${failures} расхождений`}`);
process.exitCode = failures === 0 ? 0 : 1;
