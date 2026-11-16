// @ts-check
/**
 * Meta test & CI gate: Modular Controller Wiring & Anti-Regression Invariant.
 * Prevents regressions during monolith decomposition and sub-controller extraction
 * (such as missing dependency injection, unhandled Map.get() TypeErrors on user clicks,
 * or lost DOM references in extracted view renderers).
 */

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));

describe("Modular Controller Wiring & Dependency Injection Invariant", () => {
  it("app.js passes all mandatory voice dependencies into createVoiceCallController", async () => {
    const appJs = await readFile(join(root, "rust", "gui-spike", "ui", "app.js"), "utf8");
    const callIdx = appJs.indexOf("createVoiceCallController({");
    assert.ok(callIdx > 0, "app.js must call createVoiceCallController");
    const endIdx = appJs.indexOf("});", callIdx);
    assert.ok(endIdx > callIdx, "createVoiceCallController block must close");
    const callBlock = appJs.slice(callIdx, endIdx);

    const requiredDeps = [
      "lastVoiceUiSoundAtByKind",
      "getVoiceHost",
      "voiceHost",
      "selfVoicePrefs",
      "liveVoiceStatesByGuild",
      "getActiveVoiceConnection",
      "getLiveRtcStatus",
    ];

    for (const dep of requiredDeps) {
      assert.ok(
        callBlock.includes(dep),
        `app.js createVoiceCallController dependencies must include '${dep}'`,
      );
    }
  });

  it("voice-call-orchestrator.js defines resilient fallbacks for collections and DOM hosts", async () => {
    const orchJs = await readFile(
      join(root, "rust", "gui-spike", "ui", "features", "voice-call-orchestrator.js"),
      "utf8",
    );

    assert.match(
      orchJs,
      /lastVoiceUiSoundAtByKind:\s*deps\.lastVoiceUiSoundAtByKind\s*\?\?\s*new Map\(\)/,
      "voice-call-orchestrator.js must provide fallback Map for lastVoiceUiSoundAtByKind",
    );

    assert.match(
      orchJs,
      /getVoiceHost:\s*\(\)\s*=>[\s\S]*?document\.getElementById\("voice-panel"\)/,
      "voice-call-orchestrator.js getVoiceHost must fall back to DOM #voice-panel",
    );

    assert.match(
      orchJs,
      /get\s+voiceHost\(\)\s*\{[\s\S]*?document\.getElementById\("voice-panel"\)/,
      "voice-call-orchestrator.js voiceHost getter must fall back to DOM #voice-panel",
    );
  });

  it("voice-state-sync.js safely guards playVoiceUiSound against uninitialized sound map", async () => {
    const stateSyncJs = await readFile(
      join(root, "rust", "gui-spike", "ui", "features", "voice-state-sync.js"),
      "utf8",
    );

    const fnIdx = stateSyncJs.indexOf("function playVoiceUiSound(");
    assert.ok(fnIdx > 0, "playVoiceUiSound must exist in voice-state-sync.js");
    const fnBlock = stateSyncJs.slice(fnIdx, fnIdx + 600);

    assert.match(
      fnBlock,
      /(?:lastVoiceUiSoundAtByKind\s*=\s*(?:ctx\.)?lastVoiceUiSoundAtByKind\s*\?\?\s*new Map\(\)|if\s*\(!ctx\.lastVoiceUiSoundAtByKind\)\s*\{\s*ctx\.lastVoiceUiSoundAtByKind\s*=\s*new Map\(\);\s*\})/,
      "playVoiceUiSound must defensively ensure lastVoiceUiSoundAtByKind is a Map",
    );
    assert.match(
      fnBlock,
      /lastVoiceUiSoundAtByKind\.get\?\.\(kind\)/,
      "playVoiceUiSound must use optional chaining on Map.get()",
    );
  });

  it("voice-stage-renderer.js safely resolves voiceHost container without early return", async () => {
    const rendererJs = await readFile(
      join(root, "rust", "gui-spike", "ui", "features", "voice-stage-renderer.js"),
      "utf8",
    );

    const fnIdx = rendererJs.indexOf("function renderLiveVoicePanel(");
    assert.ok(fnIdx > 0, "renderLiveVoicePanel must exist in voice-stage-renderer.js");
    const fnBlock = rendererJs.slice(fnIdx, fnIdx + 400);

    assert.match(
      fnBlock,
      /document\.getElementById\("voice-panel"\)/,
      "renderLiveVoicePanel must fall back to document.getElementById('voice-panel')",
    );
  });
});
