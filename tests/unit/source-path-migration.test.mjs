// @ts-check
import assert from "node:assert/strict";
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { test } from "node:test";
import { canonicalRustSourcePath } from "../../scripts/source-paths.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));

test("historical crate paths resolve to the independently declared Cargo packages", () => {
  const crates = readdirSync(join(root, "rust", "crates"), { withFileTypes: true })
    .filter(entry => entry.isDirectory() && entry.name.startsWith("chord-"));
  assert.ok(crates.length >= 12);
  for (const entry of crates) {
    const current = `rust/crates/${entry.name}/Cargo.toml`;
    const manifest = readFileSync(join(root, current), "utf8");
    assert.match(manifest, new RegExp(`name\\s*=\\s*"${entry.name}"`));
    const historical = `rust/crates/discord-${entry.name.slice("chord-".length)}/Cargo.toml`;
    assert.equal(canonicalRustSourcePath(historical), current);
    assert.equal(canonicalRustSourcePath(current), current);
  }
});

test("historical GUI and voice source locations retain their exact suffixes", () => {
  for (const directory of ["gpui", "voice"]) {
    const expected = `rust/chord-${directory}/src`;
    assert.ok(existsSync(join(root, expected)));
    assert.equal(canonicalRustSourcePath(`rust/discord-${directory}/src`), expected);
  }
  assert.equal(canonicalRustSourcePath("rust\\crates\\discord-core\\src\\lib.rs"),
    "rust/crates/chord-core/src/lib.rs");
});

test("unknown packages, release artifact names and clean-room evidence are not rewritten", () => {
  for (const unchanged of [
    "rust/crates/discord-unknown/src/lib.rs", "rust/crates/discord-core-extra/src/lib.rs",
    "rust/dist/discord-gpui", "rust/dist/discord-gpui-linux.tar.gz",
    "rust/discord-gpui-extra/src/main.rs", "docs/re/discord-core.json",
    "/home/sanya/re-deep/rest-d02.json", "src/discord-core.mjs", "",
    "C:\\private\\discord-core.json", "rust\\dist\\discord-gpui.exe",
  ]) assert.equal(canonicalRustSourcePath(unchanged), unchanged);
});
