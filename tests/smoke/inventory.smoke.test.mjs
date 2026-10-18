// @ts-check
// RED (M2): inventory for a guaranteed nonexistent pid must report a skipped target.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { runCli } from "../helpers/cli.mjs";

describe("discord-lab inventory", () => {
  it("неизвестный pid → код 2 из-за недоступной Frida или пропущенной цели", async () => {
    const result = await runCli(["inventory", "--pid", "99999999"]);
    assert.equal(result.code, 2, result.stderr);
    assert.match(result.stderr, /Frida недоступна|пропущено/i);
    assert.doesNotMatch(result.stderr, /появится в M2/);
  });
});
