// @ts-check
// RED (M2): реализация — CLI pids (Frida). Skip-aware: Frida недоступна —
// тест честно пропускается, а не «зеленеет» молча.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { runCli } from "../helpers/cli.mjs";

describe("discord-lab pids", () => {
  it("pids --all перечисляет процессы в формате '<pid>\\t<name>'", async (t) => {
    const sleeper = spawn("sleep", ["30"]);
    t.after(() => sleeper.kill("SIGKILL"));

    const result = await runCli(["pids", "--all"]);
    if (result.code === 2 && /frida/i.test(result.stderr)) {
      return t.skip(`Frida недоступна: ${result.stderr.trim().slice(0, 120)}`);
    }
    assert.equal(result.code, 0, result.stderr);

    for (const line of result.stdout.split("\n").filter((line) => line.trim())) {
      assert.match(line, /^\d+\t.+$/);
    }
    assert.ok(
      result.stdout.includes(`${sleeper.pid}\tsleep`),
      `в выводе нет процесса sleep (${sleeper.pid})`,
    );
  });
});
