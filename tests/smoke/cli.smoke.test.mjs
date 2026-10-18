// @ts-check
// RED (M1): реализация — bin/discord-lab.mjs + src/cli/run.mjs.
// Контракт CLI: help / version / неизвестная команда / ошибка CDP.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { projectDir, runCli } from "../helpers/cli.mjs";

describe("discord-lab CLI", () => {
  it("--help: код 0 и список команд", async () => {
    const result = await runCli(["--help"]);
    assert.equal(result.code, 0, result.stderr);
    for (const word of ["discord-lab", "pids", "inventory", "hook", "cdp", "walk", "summarize", "domains", "static"]) {
      assert.ok(result.stdout.includes(word), `в справке нет "${word}"`);
    }
  });

  it("без аргументов печатает справку и выходит с 0", async () => {
    const result = await runCli([]);
    assert.equal(result.code, 0, result.stderr);
    assert.ok(result.stdout.includes("discord-lab"));
  });

  it("--version печатает версию из package.json", async () => {
    const packageJson = JSON.parse(await readFile(join(projectDir(), "package.json"), "utf8"));
    const result = await runCli(["--version"]);
    assert.equal(result.code, 0, result.stderr);
    assert.equal(result.stdout.trim(), packageJson.version);
  });

  it("неизвестная команда → код 1 и сообщение в stderr", async () => {
    const result = await runCli(["badcmd"]);
    assert.equal(result.code, 1);
    assert.match(result.stderr, /Неизвестная команда: badcmd/);
  });

  it("cdp при недоступном endpoint → код 2 и понятная ошибка", async () => {
    const result = await runCli(["cdp", "targets"], { env: { CDP_ENDPOINT: "http://127.0.0.1:9" } });
    assert.equal(result.code, 2);
    assert.match(result.stderr, /CDP/);
    assert.match(result.stderr, /127\.0\.0\.1:9/);
  });
});
