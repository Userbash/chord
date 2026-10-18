// @ts-check
// RED (M1): реализация — src/core/processes.mjs. Контракт: отбор процессов Discord.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { filterDiscordProcesses, isDiscordProcess } from "../../src/core/processes.mjs";

describe("isDiscordProcess", () => {
  // Имена живых инвентарей: jq -r '.process' functions/pid-*.json выводит Discord;
  // допускаемые варианты определяются контрактом /discord/i && !/vesktop/i.
  it("распознаёт процессы Discord без учёта регистра", () => {
    assert.equal(isDiscordProcess("Discord"), true);
    assert.equal(isDiscordProcess("discord"), true);
    assert.equal(isDiscordProcess("Discord Canary"), true);
  });

  it("исключает vesktop", () => {
    assert.equal(isDiscordProcess("Vesktop"), false);
    assert.equal(isDiscordProcess("vesktop"), false);
  });

  it("применяет только подстрочные правила без расширения списка имён", () => {
    assert.equal(isDiscordProcess("DiscordPTB"), true);
    assert.equal(isDiscordProcess("DiscordCanary"), true);
    assert.equal(isDiscordProcess("Vesktop Discord"), false);
    assert.equal(isDiscordProcess("Vesktop"), false);
  });

  it("не трогает посторонние процессы", () => {
    assert.equal(isDiscordProcess("chrome"), false);
    assert.equal(isDiscordProcess("sleep"), false);
  });
});

describe("filterDiscordProcesses", () => {
  it("оставляет только Discord-процессы, сохраняя порядок", () => {
    const input = [
      { pid: 1, name: "chrome" },
      { pid: 2, name: "Discord" },
      { pid: 3, name: "Vesktop" },
      { pid: 4, name: "discord-canary" },
    ];
    assert.deepEqual(filterDiscordProcesses(input), [
      { pid: 2, name: "Discord" },
      { pid: 4, name: "discord-canary" },
    ]);
  });

  it("пустой список → пустой список", () => {
    assert.deepEqual(filterDiscordProcesses([]), []);
  });
});
