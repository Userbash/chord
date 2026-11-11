// @ts-check
/**
 * W7 Unit & DOM Contract Tests (`TASK-822` & `TASK-823`):
 * 1. `join-server.js`:
 *    - Non-blocking loading indicator (`#join-loading-indicator`, `data-loading="true"`, `aria-busy="true"`)
 *      preventing UI freeze on rapid 3x clicks during invite resolution.
 *    - `resolveInviteWithRetry` & `INVITE_POOL_CONFIG`: HTTP/2 + Keep-Alive (90s) connection pool
 *      and retry-with-backoff (150ms initial backoff) on transient network/5xx errors without retrying 404.
 * 2. `rtc-quality-status-bar.js`:
 *    - Defeating Discord AVError codes `2004, 2005, 2009, 2010, 2016, 2017` (`DISCORD_AV_ERROR_CODES`,
 *      `discordMinEncodeFpsThreshold`, `defeatDiscordAvError`, `evaluateStreamAvHealth`).
 *    - RTC Quality Status Bar (`240px x 40px` in channel tree footer) with RTT/loss color indicator
 *      (`#23a55a` fine <= 250ms, `#f0b232` average <= 500ms, `#f23f43` bad > 500ms),
 *      diagnostic popover (Ping, Loss %, Opus, AV1/H.264, `DAVE MLS v1 Verified`, AVError shield),
 *      and call state transition sound effects (`connect`, `disconnect`, `mute`, `deafen`, `stream_start`, `stream_stop`).
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { loadApp, ShimDocument } from "../support/gui-dom-harness.mjs";

const READY = Object.freeze({
  user: { id: "42", username: "tester", globalName: "Тестер", avatar: null },
  guilds: [
    { id: "111", name: "Первый сервер", icon: null, memberCount: 3, channelCount: 2 },
  ],
  guildCount: 1,
  privateChannelCount: 0,
  relationshipCount: 0,
  readStateCount: 0,
  session: null,
});

const PREVIEW = Object.freeze({
  code: "eSrxp2YGUC",
  guildId: "274093052673392641",
  guildName: "ITSKIDON PC BOOST",
  icon: "a_1b2c3d",
  memberCount: 14520,
  presenceCount: 1890,
  expiresAt: null,
  channelId: "274093052673392642",
});

describe("W7 TASK-822: join-server.js non-blocking loading indicator & HTTP/2 retry-with-backoff", () => {
  it("exports INVITE_POOL_CONFIG (90s keep-alive, 150ms backoff, HTTP/2) and resolveInviteWithRetry", async () => {
    const mod = /** @type {any} */ (await import("../../rust/gui-spike/ui/features/join-server.js"));
    assert.ok(mod.INVITE_POOL_CONFIG, "INVITE_POOL_CONFIG must be exported");
    assert.equal(mod.INVITE_POOL_CONFIG.keepAliveSecs, 90);
    assert.equal(mod.INVITE_POOL_CONFIG.initialBackoffMs, 150);
    assert.equal(mod.INVITE_POOL_CONFIG.maxAttempts, 3);
    assert.equal(mod.INVITE_POOL_CONFIG.httpVersion, "HTTP/2");
    assert.equal(typeof mod.resolveInviteWithRetry, "function");

    // Transient network error on attempt 1 + 502 server error on attempt 2 -> recovers on attempt 3
    let calls = 0;
    /** @type {number[]} */
    const recordedBackoffs = [];
    const result = await mod.resolveInviteWithRetry(
      async (/** @type {string} */ _cmd, /** @type {any} */ _args) => {
        calls += 1;
        if (calls === 1) throw { kind: "network" };
        if (calls === 2) throw { kind: "server", status: 502 };
        return PREVIEW;
      },
      "https://discord.gg/eSrxp2YGUC",
      {
        onBackoff: (/** @type {number} */ delayMs) => recordedBackoffs.push(delayMs),
        sleepFn: async () => {},
      },
    );

    assert.equal(calls, 3);
    assert.deepEqual(recordedBackoffs, [150, 300]);
    assert.equal(result.preview.guildId, "274093052673392641");
    assert.equal(result.metrics.attempts, 3);
    assert.equal(result.metrics.httpVersion, "HTTP/2");

    // Non-transient 404 NotFound -> fails immediately on attempt 1 without retry
    let calls404 = 0;
    await assert.rejects(
      () =>
        mod.resolveInviteWithRetry(
          async () => {
            calls404 += 1;
            throw { kind: "notFound" };
          },
          "deadinvite",
          { sleepFn: async () => {} },
        ),
      (/** @type {any} */ err) => err?.kind === "notFound",
    );
    assert.equal(calls404, 1, "404 NotFound must not be retried");
  });

  it("displays non-blocking #join-loading-indicator with data-loading=true during resolve and clears on preview", async () => {
    /** @type {(v: any) => void} */
    let release = () => {};
    const pending = new Promise((r) => {
      release = r;
    });

    const app = await loadApp({
      invokeHandler: (cmd) => {
        if (cmd === "auth_status") {
          return { status: "ready", stored: true, user: { id: "42", username: "tester" } };
        }
        if (cmd === "ready") return READY;
        if (cmd === "channels") return [];
        if (cmd === "resolve_invite") return pending;
        return undefined;
      },
    });

    try {
      const plus = app.document.querySelector('#guild-rail [data-action="add-server"]');
      assert.ok(plus);
      plus.click();

      const indicator = app.document.querySelector("#join-loading-indicator");
      assert.ok(indicator, "#join-loading-indicator must exist inside #join-server-dialog");
      assert.notEqual(indicator.getAttribute("hidden"), null, "indicator must be hidden when idle");

      const input = /** @type {any} */ (app.document.querySelector("#join-invite-input"));
      input.value = "eSrxp2YGUC";
      input.emit("input");

      const submit = app.document.querySelector("#join-submit");
      assert.ok(submit);
      // Simulate rapid 3x clicks from live telemetry report
      submit.click();
      submit.click();
      submit.click();
      await app.settle(2);

      assert.equal(indicator.getAttribute("hidden"), null, "indicator must be visible while resolving");
      assert.equal(indicator.getAttribute("data-loading"), "true");
      assert.equal(submit.getAttribute("data-loading"), "true");
      assert.equal(submit.getAttribute("aria-busy"), "true");
      assert.equal(app.callsOf("resolve_invite").length, 1, "3 rapid clicks must trigger only 1 IPC request");

      release(PREVIEW);
      await app.settle(3);

      assert.notEqual(indicator.getAttribute("hidden"), null, "indicator must hide once preview is ready");
      assert.equal(indicator.getAttribute("data-loading"), "false");
      assert.equal(submit.getAttribute("data-loading"), "false");
    } finally {
      app.dispose();
    }
  });
});

describe("W7 TASK-823: Defeat Discord AVError codes 2004, 2005, 2009, 2010, 2016, 2017 & RTC Quality Status Bar", () => {
  it("defeats all six Discord AVError codes (2004, 2005, 2009, 2010, 2016, 2017) with exact RE oracle thresholds", async () => {
    const rtc = await import("../../rust/gui-spike/ui/features/rtc-quality-status-bar.js");

    assert.deepEqual([...rtc.DISCORD_AV_ERROR_CODES], [2004, 2005, 2009, 2010, 2016, 2017]);
    assert.equal(rtc.discordMinEncodeFpsThreshold(5), 3);
    assert.equal(rtc.discordMinEncodeFpsThreshold(15), 8);
    assert.equal(rtc.discordMinEncodeFpsThreshold(30), 15);
    assert.equal(rtc.discordMinEncodeFpsThreshold(60), 30);

    for (const code of rtc.DISCORD_AV_ERROR_CODES) {
      const plan = rtc.defeatDiscordAvError(code);
      assert.ok(plan, `missing mitigation plan for code ${code}`);
      assert.equal(plan.code, code);
      assert.equal(plan.defeated, true);
      assert.ok(plan.slug.length > 0);
    }

    const health = rtc.evaluateStreamAvHealth({
      shortPacketLossRate: 0.16,
      longPacketLossRate: 0.12,
      shortEncodeFps: 10,
      longEncodeFps: 14,
      targetMaxFps: 60,
      currentBitrateBps: 5_000_000,
      videoEncodeError: true,
      activeEncoder: "nvenc_h264",
      streamDeleteReason: "stream_full",
      activeViewerCount: 10,
      maxViewerCapacity: 10,
      videoBackgroundFilterError: true,
      transportCodecDesync: true,
    });

    assert.deepEqual(health.defeatedCodes, [2004, 2005, 2009, 2010, 2016, 2017]);
    assert.equal(health.fecEnabled, true);
    assert.ok(health.effectivePacketLossPct <= 10.0);
    assert.ok(health.adjustedBitrateBps < 5_000_000);
    assert.equal(health.duplicateFrameKeepaliveInjected, true);
    assert.ok(health.effectiveEncodeFps >= 30);
    assert.equal(health.activeEncoder, "software_openh264");
    assert.equal(health.isStreamMarkedFull, false);
    assert.equal(health.backgroundFilterMode, "passthrough_raw");
    assert.equal(health.transportReconciled, true);
    assert.equal(health.streamAlive, true);
  });

  it("renders 240x40 RTC Quality Status Bar with RTT colors, DAVE MLS v1 Verified popover, and call sound effects", async () => {
    const { createRtcQualityStatusBar } = await import(
      "../../rust/gui-spike/ui/features/rtc-quality-status-bar.js"
    );
    const doc = new ShimDocument();
    const host = doc.createElement("div");
    doc.body.append(host);

    const bar = createRtcQualityStatusBar(host, {
      channelName: "Пекарня",
      pingMs: 38,
      packetLossPct: 0.4,
      audioCodec: "Opus 48 kHz",
      videoCodec: "AV1",
      daveVerified: true,
      privacyCode: "11111 22222 33333 44444 55555 66666",
    });

    const root = host.querySelector('[data-rtc-quality-bar="true"]');
    assert.ok(root, "RTC quality status bar root must be rendered");
    assert.equal(root.getAttribute("data-width"), "240");
    assert.equal(root.getAttribute("data-height"), "40");
    assert.equal(root.getAttribute("data-quality"), "fine");
    assert.equal(root.getAttribute("data-indicator-color"), "#23a55a");

    // Update to yellow (average: 310ms) and red (bad: 620ms)
    bar.updateMetrics({ pingMs: 310, packetLossPct: 3.5 });
    assert.equal(root.getAttribute("data-quality"), "average");
    assert.equal(root.getAttribute("data-indicator-color"), "#f0b232");

    bar.updateMetrics({ pingMs: 620, packetLossPct: 14.2 });
    assert.equal(root.getAttribute("data-quality"), "bad");
    assert.equal(root.getAttribute("data-indicator-color"), "#f23f43");

    // Restore fine metrics and open diagnostic popover
    bar.updateMetrics({ pingMs: 42, packetLossPct: 0.5, videoCodec: "AV1", daveVerified: true });
    const trigger = host.querySelector('[data-rtc-quality-trigger="true"]');
    assert.ok(trigger);
    const popout = host.querySelector('[data-rtc-quality-popout="true"]');
    assert.ok(popout);
    assert.notEqual(popout.getAttribute("hidden"), null, "popout must be hidden before click");

    trigger.click();
    assert.equal(popout.getAttribute("hidden"), null, "popout must open on click");
    assert.match(popout.textContent ?? "", /42\s*мс/);
    assert.match(popout.textContent ?? "", /0\.5%/);
    assert.match(popout.textContent ?? "", /Opus/);
    assert.match(popout.textContent ?? "", /AV1/);
    assert.match(popout.textContent ?? "", /DAVE MLS v1 Verified/);
    for (const code of [2004, 2005, 2009, 2010, 2016, 2017]) {
      assert.match(popout.textContent ?? "", new RegExp(String(code)));
    }

    // Verify all call state sound effects fire deterministically
    for (const cue of ["connect", "mute", "unmute", "deafen", "undeafen", "stream_start", "stream_stop", "disconnect"]) {
      const sound = bar.playCallSound(/** @type {any} */ (cue));
      assert.equal(sound.kind, cue);
      assert.ok(sound.frequencyHz > 0);
      assert.ok(sound.durationMs > 0);
    }
    assert.equal(bar.getPlayedSounds().length, 8);
  });
});
