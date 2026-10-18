// @ts-check
// RED (M2): hook to a guaranteed nonexistent pid must fail before waiting.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { chmod, mkdir, mkdtemp, rm } from "node:fs/promises";
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
    const readonly = join(temp, "readonly");
    try {
      await mkdir(readonly);
      await chmod(readonly, 0o555);
      const result = await runCli(["hook", "--pid", "99999999", "--out", join(readonly, "sub/x.jsonl")]);
      assert.equal(result.code, 2, result.stderr);
      assert.match(result.stderr, /каталог/);
      assert.doesNotMatch(result.stderr, /at .+:\d+/);
    } finally {
      await chmod(readonly, 0o755).catch(() => {});
      await rm(temp, { recursive: true, force: true });
    }
  });
});
