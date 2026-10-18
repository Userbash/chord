// @ts-check
// Live-прогон против запущенного Discord — только чтение, opt-in:
//   DISCORD_LAB_LIVE_CDP=http://127.0.0.1:9223 npm run test:e2e
// (Discord должен быть запущен с --remote-debugging-port=9223).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseJson, runCli } from "../helpers/cli.mjs";

const liveEndpoint = process.env.DISCORD_LAB_LIVE_CDP;
const skipReason = liveEndpoint
  ? false
  : "задайте DISCORD_LAB_LIVE_CDP=http://127.0.0.1:9223 (Discord с --remote-debugging-port)";

describe("live Discord (только чтение)", () => {
  it("targets показывает страницы Discord", { skip: skipReason }, async () => {
    const result = await runCli(["cdp", "targets"], { env: { CDP_ENDPOINT: liveEndpoint ?? "" } });
    assert.equal(result.code, 0, result.stderr);
    assert.match(result.stdout, /\tpage\t/);
  });

  it("eval 1+1 выполняется на живой странице", { skip: skipReason }, async () => {
    const result = await runCli(["cdp", "eval", "1+1"], { env: { CDP_ENDPOINT: liveEndpoint ?? "" } });
    assert.equal(result.code, 0, result.stderr);
    assert.equal(parseJson(result.stdout), 2);
  });
});
