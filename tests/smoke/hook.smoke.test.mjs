// @ts-check
// RED (M2): hook to a guaranteed nonexistent pid must fail before waiting.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runCli } from "../helpers/cli.mjs";

describe("discord-lab hook", () => {
  it("неизвестный pid → код 2 из-за недоступной Frida или attach-ошибки", async () => {
    const result = await runCli(["hook", "--pid", "99999999"]);
    assert.equal(result.code, 2, result.stderr);
    assert.match(result.stderr, /Frida недоступна|Не удалось подключиться/i);
    assert.doesNotMatch(result.stderr, /появится в M2/);
  });

  it("ошибка подготовки каталога лога → код 2 без стека", async () => {
    const temp = await mkdtemp(join(tmpdir(), "discord-lab-hook-"));
    const blocked = join(temp, "blocked");
    try {
      await writeFile(blocked, "regular file blocks directory creation");
      const result = await runCli(["hook", "--pid", "99999999", "--out", join(blocked, "sub/x.jsonl")]);
      assert.equal(result.code, 2, result.stderr);
      assert.match(result.stderr, /каталог/);
      assert.doesNotMatch(result.stderr, /at .+:\d+/);
    } finally {
      await rm(temp, { recursive: true, force: true });
    }
  });
});
