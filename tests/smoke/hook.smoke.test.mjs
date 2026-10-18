// @ts-check
// RED (M2): hook to a guaranteed nonexistent pid must fail before waiting.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { runCli } from "../helpers/cli.mjs";

describe("discord-lab hook", () => {
  it("неизвестный pid → код 2 из-за недоступной Frida или attach-ошибки", async () => {
    const result = await runCli(["hook", "--pid", "99999999"]);
    assert.equal(result.code, 2, result.stderr);
    assert.match(result.stderr, /Frida недоступна|Не удалось подключиться/i);
    assert.doesNotMatch(result.stderr, /появится в M2/);
  });
});
