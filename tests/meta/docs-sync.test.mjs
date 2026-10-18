// @ts-check
// Мета-тест (зелёный): README ↔ команды CLI. Каждая команда из блока «Команды:»
// справки HELP (`src/cli/run.mjs`) должна упоминаться в README.md — иначе новая
// функция осталась недокументированной. Краснеет при расхождении.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));

/**
 * Команды из блока «Команды:» справки HELP.
 * @param {string} source исходный текст `src/cli/run.mjs`
 * @returns {string[]}
 */
function parseHelpCommands(source) {
  const start = source.indexOf("Команды:");
  assert.ok(start >= 0, "src/cli/run.mjs: в HELP нет блока «Команды:»");
  /** @type {string[]} */
  const commands = [];
  const lines = source.slice(start).split("\n");
  for (const line of lines.slice(1)) {
    if (!line.startsWith("  ")) break;
    const m = /^ {2}([a-z][a-z0-9-]*)\b/.exec(line);
    const name = m?.[1];
    if (name) commands.push(name);
  }
  return commands;
}

describe("docs-sync: README ↔ команды CLI", () => {
  it("каждая команда из HELP упомянута в README.md", async () => {
    const source = await readFile(join(root, "src", "cli", "run.mjs"), "utf8");
    const commands = parseHelpCommands(source);
    assert.ok(commands.length >= 8, `распознано команд: ${commands.length} — ожидалось ≥ 8`);
    const readme = await readFile(join(root, "README.md"), "utf8");
    const missing = commands.filter((name) => !readme.includes("`" + name));
    assert.deepEqual(missing, [], "команды CLI из HELP без упоминания в README.md");
  });
});
