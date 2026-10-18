// @ts-check
// RED (M1): реализация — bin/discord-lab.mjs + src/cli/run.mjs.
// Контракт CLI: help / version / неизвестная команда / ошибка CDP.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { cp, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";
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

  it("числовые и regex аргументы отклоняются до обращения к Frida", async () => {
    /** @type {[string[], RegExp][]} */
    const cases = [
      [["inventory", "--pid", "abc"], /Неверное значение --pid/],
      [["hook", "--limit", "abc"], /Неверное значение --limit/],
      [["hook", "--arg-count", "9"], /Неверное значение --arg-count/],
      [["hook", "--pattern", "("], /regex/i],
    ];
    for (const [args, pattern] of cases) {
      const result = await runCli(args);
      assert.equal(result.code, 1, result.stderr);
      assert.match(result.stderr, pattern);
    }
  });

  it("битый package.json для --version возвращает код 2 без стека", async () => {
    const temp = await mkdtemp(join(tmpdir(), "discord-lab-version-"));
    try {
      await cp(join(projectDir(), "bin"), join(temp, "bin"), { recursive: true });
      await cp(join(projectDir(), "src"), join(temp, "src"), { recursive: true });
      await writeFile(join(temp, "package.json"), "{ broken");
      const result = await new Promise((resolve, reject) => {
        const child = spawn(process.execPath, [join(temp, "bin/discord-lab.mjs"), "--version"]);
        let stdout = "";
        let stderr = "";
        child.stdout.setEncoding("utf8").on("data", (chunk) => { stdout += chunk; });
        child.stderr.setEncoding("utf8").on("data", (chunk) => { stderr += chunk; });
        child.once("error", reject);
        child.once("close", (code) => resolve({ code, stdout, stderr }));
      });
      assert.equal(result.code, 2, result.stderr);
      assert.match(result.stderr, /Не удалось прочитать версию/);
      assert.doesNotMatch(result.stderr, /at .+:\d+/);
    } finally {
      await rm(temp, { recursive: true, force: true });
    }
  });
});
