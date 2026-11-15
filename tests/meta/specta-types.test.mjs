// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const bindingsJsPath = join(root, "rust/gui-spike/ui/bindings.js");
const bindingsDtsPath = join(root, "rust/gui-spike/ui/bindings.d.ts");
const appJsPath = join(root, "rust/gui-spike/ui/app.js");
const bindingsRsPath = join(root, "rust/gui-spike/src/bindings.rs");

describe("RW51-EPIC1-T1 / RW51-EPIC5-T1: Specta TypeScript & JSDoc Type Generation Verification", () => {
  it("verifies that ui/bindings.js and ui/bindings.d.ts exist on disk", () => {
    assert.ok(existsSync(bindingsJsPath), "rust/gui-spike/ui/bindings.js must exist");
    assert.ok(existsSync(bindingsDtsPath), "rust/gui-spike/ui/bindings.d.ts must exist");
  });

  it("verifies that generated bindings contain zero `any` types (strict typing contract)", () => {
    const jsContent = readFileSync(bindingsJsPath, "utf8");
    const dtsContent = readFileSync(bindingsDtsPath, "utf8");

    const anyPattern = /:\s*any\b|<any>|\[any\]|\bArray<any>|\bRecord<[^,>]+,\s*any>|@param\s+\{[^}]*any[^}]*\}|@returns\s+\{[^}]*any[^}]*\}/;

    assert.equal(
      anyPattern.test(jsContent),
      false,
      "ui/bindings.js must not contain any `any` types"
    );
    assert.equal(
      anyPattern.test(dtsContent),
      false,
      "ui/bindings.d.ts must not contain any `any` types"
    );
  });

  it("verifies all core Specta IPC command methods in ui/bindings.js and ui/bindings.d.ts", () => {
    const jsContent = readFileSync(bindingsJsPath, "utf8");
    const dtsContent = readFileSync(bindingsDtsPath, "utf8");

    const expectedMethods = [
      "ready",
      "syncLiveState",
      "voiceConnect",
      "voiceRtcStatus",
    ];

    for (const method of expectedMethods) {
      assert.ok(
        jsContent.includes(`async ${method}(`),
        `ui/bindings.js must declare client method '${method}'`
      );
      assert.ok(
        dtsContent.includes(`${method}(`),
        `ui/bindings.d.ts must declare TypedCommands method '${method}'`
      );
    }
  });

  it("verifies all core Specta DTO interfaces in ui/bindings.d.ts", () => {
    const dtsContent = readFileSync(bindingsDtsPath, "utf8");

    const expectedDtos = [
      "ReadyUserDto",
      "SessionSummaryDto",
      "GuildSummaryDto",
      "ReadyPayloadDto",
      "GuildSyncSummaryDto",
      "VoiceRtcStatusDto",
      "ChannelAccessDto",
      "LiveSyncSnapshotDto",
      "VoiceConnectOptionsDto",
      "VoiceConnectResultDto",
    ];

    for (const dto of expectedDtos) {
      assert.ok(
        dtsContent.includes(`export interface ${dto}`) || dtsContent.includes(`interface ${dto}`),
        `ui/bindings.d.ts must declare interface '${dto}'`
      );
    }
  });

  it("verifies that ui/app.js references generated commands.ready()", () => {
    assert.ok(existsSync(appJsPath), "rust/gui-spike/ui/app.js must exist");
    const appJsContent = readFileSync(appJsPath, "utf8");
    assert.ok(
      appJsContent.includes("commands.ready()") || appJsContent.includes("createTypedCommands"),
      "ui/app.js must wire generated typed commands"
    );
  });

  it("verifies that Rust source bindings.rs registers all 9 backend Specta commands", () => {
    assert.ok(existsSync(bindingsRsPath), "rust/gui-spike/src/bindings.rs must exist");
    const bindingsRsContent = readFileSync(bindingsRsPath, "utf8");

    const backendCommands = [
      "ready",
      "read_state",
      "channels",
      "messages_page",
      "members_page",
      "settings",
      "voice_connect",
      "voice_rtc_status",
      "voice_set_audio_device",
    ];

    for (const cmd of backendCommands) {
      assert.ok(
        bindingsRsContent.includes(`name: "${cmd}"`),
        `bindings.rs must register SpectaCommandSpec for '${cmd}'`
      );
    }

    assert.ok(
      bindingsRsContent.includes("generate_specta_builder"),
      "bindings.rs must expose generate_specta_builder()"
    );
    assert.ok(
      bindingsRsContent.includes("validate_no_any"),
      "bindings.rs must provide validate_no_any validator"
    );
  });

  it("verifies exported Discord permission constants in ui/bindings.js and ui/bindings.d.ts", () => {
    const jsContent = readFileSync(bindingsJsPath, "utf8");
    const dtsContent = readFileSync(bindingsDtsPath, "utf8");

    const expectedPerms = [
      "PERM_VIEW_CHANNEL",
      "PERM_SEND_MESSAGES",
      "PERM_CONNECT",
      "PERM_SPEAK",
      "LURKER_PERMISSIONS_MASK",
    ];

    for (const perm of expectedPerms) {
      assert.ok(
        jsContent.includes(`export const ${perm}`),
        `ui/bindings.js must export constant '${perm}'`
      );
      assert.ok(
        dtsContent.includes(`export declare const ${perm}: bigint;`),
        `ui/bindings.d.ts must declare constant '${perm}'`
      );
    }
  });
});
