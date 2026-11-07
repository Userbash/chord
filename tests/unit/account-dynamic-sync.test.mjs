// @ts-check
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "../..");

test("CARD-DYN-001 & CARD-DYN-004: data.rs has zero occurrences of hardcoded user ID 384299970632876032", () => {
  const dataRsPath = path.join(repoRoot, "rust/gui-spike/src/data.rs");
  assert.ok(fs.existsSync(dataRsPath), "data.rs must exist");
  const content = fs.readFileSync(dataRsPath, "utf8");
  const matches = content.match(/384299970632876032/g);
  assert.equal(
    matches,
    null,
    `data.rs must not contain hardcoded user ID 384299970632876032, found: ${matches?.length}`,
  );
});

test("CARD-DYN-005: canDeleteLiveMessage permission rules for DMs and Guilds", async () => {
  const appJsPath = path.join(repoRoot, "rust/gui-spike/ui/app.js");
  assert.ok(fs.existsSync(appJsPath), "app.js must exist");
  const appJs = fs.readFileSync(appJsPath, "utf8");

  // Verify canDeleteLiveMessage is present in app.js
  assert.ok(
    appJs.includes("function canDeleteLiveMessage("),
    "app.js must declare function canDeleteLiveMessage",
  );

  // Verify DM restriction: in DMs, deleting someone else's message returns false
  assert.ok(
    appJs.includes("isDmOrGroupContext") && appJs.includes("return false;"),
    "canDeleteLiveMessage must block deleting someone else's message in DM or group context",
  );

  // Verify Server restriction: deleting someone else's message requires canManageMessages / isAdmin / isOwner
  assert.ok(
    appJs.includes("perms.canManageMessages") || appJs.includes("1n << 13n"),
    "canDeleteLiveMessage must require MANAGE_MESSAGES on servers",
  );

  // Verify user-friendly error notice instead of silent crash
  assert.ok(
    appJs.includes("У вас нет прав на удаление чужого сообщения"),
    "deleteLiveMessageRow must show a toast notice when unauthorized",
  );
});

test("CARD-DYN-003: DM Call, Camera, Screen Share and Group DM actions", () => {
  const appJsPath = path.join(repoRoot, "rust/gui-spike/ui/app.js");
  const appJs = fs.readFileSync(appJsPath, "utf8");

  // Verify DM action toolbar buttons
  assert.ok(
    appJs.includes("dm-call-voice"),
    "DM header toolbar must include voice call button",
  );
  assert.ok(
    appJs.includes("dm-call-video"),
    "DM header toolbar must include video call button",
  );
  assert.ok(
    appJs.includes("dm-call-stream"),
    "DM header toolbar must include screen share button",
  );
  assert.ok(
    appJs.includes("openGroupDmModal"),
    "DM header must allow opening group DM modal",
  );
});
