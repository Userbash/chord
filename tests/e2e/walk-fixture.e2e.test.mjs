// @ts-check
// RED (M3): CLI walk fixture — полный UI-прогон на fixture-странице:
// клики по обычным пунктам, пропуск запрещённых, журнал JSONL + скриншоты.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { runCli } from "../helpers/cli.mjs";
import { findBrowser } from "../helpers/browser.mjs";
import { openStaticFixture } from "../helpers/static-fixture.mjs";

/** @typedef {import("../../src/core/types.js").WalkRecord} WalkRecord */

const fixture = fileURLToPath(new URL("../fixtures/app/index.html", import.meta.url));

describe("walk fixture", () => {
  it("обходит обычные пункты, пропускает запрещённые, пишет журнал и скриншоты", async (t) => {
    const bin = findBrowser();
    if (!bin) return t.skip("нет браузера: задайте CHROME_BIN (например, /usr/bin/chromium)");

    const { browser, dir, url } = await openStaticFixture(t, { bin, file: fixture });

    const out = join(dir, "ui-walk-fixture.jsonl");
    const screens = join(dir, "screens");
    const result = await runCli(
      ["walk", "fixture", "--url", url, "--out", out, "--screens", screens],
      { env: { CDP_ENDPOINT: browser.endpoint } },
    );
    assert.equal(result.code, 0, result.stderr);
    assert.match(result.stdout, /ПРОПУСК \(запрещено\): Покинуть сервер/);
    assert.match(result.stdout, /ПРОПУСК \(запрещено\): Выйти/);

    /** @type {WalkRecord[]} */
    const records = (await readFile(out, "utf8"))
      .split("\n")
      .filter((line) => line.trim())
      .map((line) => JSON.parse(line));
    assert.deepEqual(
      records.map((record) => record.label),
      ["Мой аккаунт", "Профиль", "Уведомления", "Голос и видео"],
    );

    for (const record of records) {
      assert.equal(record.clicked, true, record.label);
      assert.ok(record.screenshot, `нет скриншота у "${record.label}"`);
      const png = await readFile(record.screenshot);
      assert.deepEqual([...png.subarray(0, 4)], [0x89, 0x50, 0x4e, 0x47]);
    }

    const notifications = records.find((record) => record.label === "Уведомления");
    assert.equal(notifications?.consoleErrors, 1);
    assert.ok((notifications?.consoleErrorTexts.length ?? 0) >= 1);

    const profile = records.find((record) => record.label === "Профиль");
    assert.equal(profile?.consoleErrors, 0);

    // Аддитивные усиления QA4-01b (существующие assert выше не ослаблены):
    // нумерация шагов, число PNG, page-side журнал кликов и текст ошибки консоли.
    assert.deepEqual(records.map((record) => record.step), [1, 2, 3, 4]);
    assert.equal((await readdir(screens)).filter((name) => name.endsWith(".png")).length, 4);
    assert.match((notifications?.consoleErrorTexts ?? []).join(" "), /уведомлен/iu);

    const clickLog = await runCli(
      ["cdp", "eval", "window.__clickLog.map((item) => item.text)"],
      { env: { CDP_ENDPOINT: browser.endpoint, CDP_TARGET: "index.html" } },
    );
    assert.equal(clickLog.code, 0, clickLog.stderr);
    assert.deepEqual(JSON.parse(clickLog.stdout), [
      "Мой аккаунт",
      "Профиль",
      "Уведомления",
      "Голос и видео",
    ]);
  });
});
