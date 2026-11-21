// @ts-check
// RED (M1/M3): CLI cdp против реального headless-браузера на fixture-странице.
// Нет браузера (CHROME_BIN или chromium/chrome/brave в PATH) — тест пропускается.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseJson, runCli } from "../helpers/cli.mjs";
import { findBrowser } from "../helpers/browser.mjs";
import { openStaticFixture } from "../helpers/static-fixture.mjs";

const fixture = fileURLToPath(new URL("../fixtures/app/index.html", import.meta.url));

describe("cdp против headless-браузера", () => {
  it("targets/eval/clicktext/list/shot на fixture-странице", async (t) => {
    const bin = findBrowser();
    if (!bin) return t.skip("нет браузера: задайте CHROME_BIN (например, /usr/bin/chromium)");

    const { browser, dir } = await openStaticFixture(t, { bin, file: fixture });
    const env = { CDP_ENDPOINT: browser.endpoint, CDP_TARGET: "index.html" };

    const targets = await runCli(["cdp", "targets"], { env });
    assert.equal(targets.code, 0, targets.stderr);
    assert.ok(targets.stdout.includes("index.html"), targets.stdout);

    const title = await runCli(["cdp", "eval", "document.title"], { env });
    assert.equal(title.code, 0, title.stderr);
    assert.equal(parseJson(title.stdout), "Discord Lab Fixture");

    const sum = await runCli(["cdp", "eval", "1+1"], { env });
    assert.equal(parseJson(sum.stdout), 2);

    const click = await runCli(["cdp", "clicktext", "Мой аккаунт"], { env });
    assert.equal(parseJson(click.stdout).clicked, true);

    const panel = await runCli(["cdp", "eval", "document.querySelector('#panel').textContent"], { env });
    assert.match(parseJson(panel.stdout), /Аккаунт/);

    const list = await runCli(["cdp", "list", "button"], { env });
    /** @type {Array<{ tag: string }>} */
    const buttons = parseJson(list.stdout);
    assert.equal(buttons.length, 6);
    assert.ok(buttons.every((entry) => entry.tag === "BUTTON"));

    const shotPath = join(dir, "shot.png");
    const shot = await runCli(["cdp", "shot", shotPath], { env });
    assert.equal(shot.code, 0, shot.stderr);
    const png = await readFile(shotPath);
    assert.ok(png.length > 100, "PNG слишком маленький");
    assert.deepEqual([...png.subarray(0, 4)], [0x89, 0x50, 0x4e, 0x47]);
  });
});
