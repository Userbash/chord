// @ts-check
// Meta-contract: Model & Effort Routing Discipline.
// Ensures strong models (Carbon / Argon-High) and high effort levels are never wasted
// on simple UI, docs, DevOps, or P2 tasks, and all rule/config files stay synchronized.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { resolveModelHarmony } from "../../scripts/harmony-router.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));

/**
 * Runs `scripts/ecc-orca-bridge.sh` in dry-run mode (`ECC_SPAWN_ORCA=0`) against a stub `orca` CLI.
 * @param {string[]} args
 * @returns {string}
 */
function runBridgeDry(args) {
  const dir = mkdtempSync(join(tmpdir(), "chord-bridge-test-"));
  try {
    const fakeOrca = join(dir, "orca");
    writeFileSync(
      fakeOrca,
      "#!/usr/bin/env bash\nif [[ \"$2\" == \"worker-list\" ]]; then exit 0; fi\necho '{\"runId\":\"run_test_stub\"}'\n",
      "utf8",
    );
    chmodSync(fakeOrca, 0o755);
    const stateFile = join(dir, "run.json");
    writeFileSync(stateFile, JSON.stringify({ runId: "run_test_stub" }), "utf8");

    return execFileSync("bash", [join(root, "scripts", "ecc-orca-bridge.sh"), ...args], {
      cwd: root,
      encoding: "utf8",
      env: {
        ...process.env,
        ECC_SPAWN_ORCA: "0",
        ORCA_BIN: fakeOrca,
        ECC_RUN_STATE_FILE: stateFile,
      },
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/**
 * Extracts `[agent_profiles.<name>]` -> `model = "..."` map from TOML text.
 * @param {string} toml
 * @returns {Record<string, string>}
 */
function parseProfileModels(toml) {
  /** @type {Record<string, string>} */
  const profiles = {};
  let current = "";
  for (const raw of toml.split("\n")) {
    const line = raw.trim();
    const sec = /^\[agent_profiles\.([a-z0-9_]+)\]$/.exec(line);
    if (sec?.[1]) {
      current = sec[1];
      continue;
    }
    if (line.startsWith("[")) {
      current = "";
      continue;
    }
    if (current) {
      const m = /^model\s*=\s*"([^"]+)"/.exec(line);
      if (m?.[1]) profiles[current] = m[1];
    }
  }
  return profiles;
}

/**
 * Extracts step `name -> profile` map from `[[orchestration_templates.*.steps]]`.
 * @param {string} toml
 * @returns {Record<string, string>}
 */
function parseTemplateStepProfiles(toml) {
  /** @type {Record<string, string>} */
  const steps = {};
  const blocks = toml.split(/\[\[orchestration_templates\.[^\]]+\.steps\]\]/).slice(1);
  for (const block of blocks) {
    const body = block.split(/\n\[/)[0] ?? "";
    const name = /^\s*name\s*=\s*"([^"]+)"/m.exec(body)?.[1];
    const profile = /^\s*profile\s*=\s*"([^"]+)"/m.exec(body)?.[1];
    if (name && profile) steps[name] = profile;
  }
  return steps;
}

describe("model-routing: ecc-orca-bridge preserves granular models and effort", () => {
  it("preserves low/medium/high sub-tiers instead of inflating to high", () => {
    /** @type {Array<[string, string, string]>} */
    const cases = [
      ["gemini-3.8-flash-low", "gemini-3.8-flash-low", "low"],
      ["gemini-3.8-flash-medium", "gemini-3.8-flash-medium", "medium"],
      ["gemini-3.8-flash-high", "gemini-3.8-flash-high", "medium"],
      ["gemini-4-argon-low", "gemini-4-argon-low", "low"],
      ["gemini-4-argon-medium", "gemini-4-argon-medium", "medium"],
      ["gemini-4-argon-high", "gemini-4-argon-high", "high"],
      ["gemini-4-carbon-low", "gemini-4-carbon-low", "low"],
      ["gemini-4-carbon-medium", "gemini-4-carbon-medium", "medium"],
      ["gemini-4-carbon-high", "gemini-4-carbon-high", "high"],
    ];

    for (const [inputModel, expectedModel, expectedEffort] of cases) {
      const out = runBridgeDry(["--model", inputModel, "--task", "verify-routing"]);
      assert.match(
        out,
        new RegExp(`Model=${expectedModel}\\s+Effort=${expectedEffort}\\b`),
        `input ${inputModel} produced unexpected routing:\n${out}`,
      );
    }
  });

  it("parses explicit --effort flag without leaking it into --task text", () => {
    const out = runBridgeDry([
      "--model",
      "gemini-4-argon-medium",
      "--effort",
      "low",
      "--task",
      "one-line-switch",
    ]);
    assert.match(out, /Model=gemini-4-argon-medium\s+Effort=low\b/);
    assert.match(out, /\[ECC-ORCA-BRIDGE\] Task: one-line-switch$/m);
  });
});

describe("model-routing: ecc2.toml right-sizes profiles and matches workspace", () => {
  it(".claude/ecc2.toml and root ecc2.toml are synchronized and contain no stale /home/user paths or opencode runners", () => {
    const claudeToml = readFileSync(join(root, ".claude", "ecc2.toml"), "utf8");
    const rootToml = readFileSync(join(root, "ecc2.toml"), "utf8");

    assert.equal(claudeToml, rootToml, ".claude/ecc2.toml and root ecc2.toml must stay identical");
    assert.ok(!claudeToml.includes("/home/user/orca"), "ecc2.toml must not reference stale /home/user/orca paths");
    assert.ok(
      !claudeToml.includes("[harness_runners.opencode]"),
      "ecc2.toml must not define banned opencode runner",
    );
  });

  it("reserves gemini-4-carbon-high strictly for core_carbon XL tasks and keeps routine profiles economical", () => {
    const toml = readFileSync(join(root, ".claude", "ecc2.toml"), "utf8");
    const profiles = parseProfileModels(toml);
    const steps = parseTemplateStepProfiles(toml);

    assert.equal(profiles.g6_tech_writer, "gemini-3.8-flash-low");
    assert.equal(profiles.g3_design, "gemini-3.8-flash-low");
    assert.equal(profiles.g2_frontend, "gemini-3.8-flash-medium");
    assert.equal(profiles.pm_review, "gemini-3.8-flash-high");
    assert.equal(profiles.devops, "gemini-4-argon-low");
    assert.equal(profiles.g1_backend, "gemini-4-argon-medium");
    assert.equal(profiles.g4_qa, "gemini-4-argon-medium");
    assert.equal(profiles.g5_voice_platform, "gemini-4-argon-medium");
    assert.equal(profiles.g7_re_rea, "gemini-4-argon-high");
    assert.equal(profiles.tech_reviewer, "gemini-4-argon-high");
    assert.equal(profiles.core_carbon, "gemini-4-carbon-high");

    // Simple P2 / standard BFF tasks must NOT consume Carbon-High
    for (const routineStep of [
      "task-604-stream-volume-mixer-antiecho",
      "task-606-discord-rpc-ipc-socket",
      "task-607-webview-navigation-guard",
      "task-808-optimistic-chat-delivery",
      "task-809-voice-invite-error-handling",
    ]) {
      const prof = steps[routineStep];
      assert.ok(prof, `missing orchestration step ${routineStep}`);
      assert.notEqual(
        profiles[prof],
        "gemini-4-carbon-high",
        `${routineStep} must not spend gemini-4-carbon-high (got profile ${prof})`,
      );
    }

    // True XL concurrency/crypto tasks MUST route to core_carbon
    for (const xlStep of [
      "task-605-dave-mls-e2ee-srtp",
      "task-615-rtp-packetizer-dave-mls-v1",
      "task-701-voice-calls-fsm-and-dave-e2ee",
      "task-702-appstate-model-tokio-gpui",
    ]) {
      assert.equal(steps[xlStep], "core_carbon", `${xlStep} should use core_carbon`);
    }
  });
});

describe("model-routing: harmony-router separates priority from complexity tier", () => {
  it("never wastes Argon/Carbon on P0 UI, Design, Docs, or trivial one-liner tasks", () => {
    const designP0 = resolveModelHarmony("TASK-703", "P0", "G3 Design");
    assert.equal(designP0.model, "gemini-3.8-flash-low");
    assert.equal(designP0.effort, "low");

    const uiP0 = resolveModelHarmony("TASK-701", "P0", "G2 Frontend");
    assert.equal(uiP0.model, "gemini-3.8-flash-medium");
    assert.equal(uiP0.effort, "medium");

    const trivialP0 = resolveModelHarmony("P0-1.1", "P0", "G1 Backend");
    assert.equal(trivialP0.model, "gemini-3.8-flash-low");
    assert.equal(trivialP0.effort, "low");

    const docsTask = resolveModelHarmony("TASK-999", "P1", "G6 Tech Writer");
    assert.equal(docsTask.model, "gemini-3.8-flash-low");
    assert.equal(docsTask.effort, "low");
  });

  it("routes P1/P2 Rust G1/G5 tasks to Argon-Low/Medium instead of dropping them into Flash docs fallback", () => {
    const voiceP1 = resolveModelHarmony("P1-1.2", "P1", "G5 Voice / Platform");
    assert.equal(voiceP1.model, "gemini-4-argon-medium");
    assert.equal(voiceP1.effort, "medium");

    const rustP2 = resolveModelHarmony("TASK-1010A", "P2", "G1 Backend");
    assert.equal(rustP2.model, "gemini-4-argon-low");
    assert.equal(rustP2.effort, "low");

    const afkP2 = resolveModelHarmony("TASK-1010B", "P2", "G5 Voice / Platform");
    assert.equal(afkP2.model, "gemini-4-argon-low");
    assert.equal(afkP2.effort, "low");
  });

  it("reserves Carbon-High strictly for XL concurrency/crypto tasks with high effort by default", () => {
    for (const xlId of ["TASK-614", "TASK-615", "TASK-702", "TASK-1021B"]) {
      const res = resolveModelHarmony(xlId, "P0", "G5 Voice / Platform");
      assert.equal(res.model, "gemini-4-carbon-high", `${xlId} expected carbon-high`);
      assert.equal(res.effort, "high", `${xlId} should use high (not wasteful xhigh) by default`);
    }
  });
});

describe("model-routing: agent rule files have no contradictory provider rules", () => {
  it("rules do not instruct workers to use banned deepseek/opencode or exhausted Claude Haiku/Sonnet/Opus", () => {
    const trackedFiles = ["AGENTS.md", "TEAM.md", ".agents/rules/worker-routing.md"];
    const optionalLocalFiles = [
      ".agents/rules/common-agents.md",
      ".agents/rules/ecc-discord-lab-departments.md",
      ".agents/rules/common-performance.md",
    ];

    for (const rel of trackedFiles) {
      assert.ok(existsSync(join(root, rel)), `${rel} must exist in repository`);
    }

    const files = [
      ...trackedFiles,
      ...optionalLocalFiles.filter((rel) => existsSync(join(root, rel))),
    ];

    for (const rel of files) {
      const content = readFileSync(join(root, rel), "utf8");
      assert.ok(
        !/deepseek\/deepseek-v4/i.test(content),
        `${rel} still instructs using banned deepseek model`,
      );
      assert.ok(
        !/--agent\s+opencode/i.test(content),
        `${rel} still instructs launching --agent opencode`,
      );
      assert.ok(
        !/\*\*Haiku\*\*|\*\*Sonnet\*\*|\*\*Opus\*\*/.test(content),
        `${rel} still instructs using Claude Haiku/Sonnet/Opus instead of project Gemini tiers`,
      );
    }
  });
});
