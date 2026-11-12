// @ts-check
import assert from "node:assert/strict";
import test from "node:test";
import {
  createSignal,
  createComputed,
  createEffect,
  batch,
  createAppReactiveStore,
} from "../../rust/gui-spike/ui/features/reactive-store.js";

test("ReactiveStore: createSignal, createComputed, createEffect, and batch coalesce updates deterministically", () => {
  const count = createSignal(2);
  const multiplier = createSignal(3);
  const product = createComputed(() => count.value * multiplier.value);

  /** @type {number[]} */
  const observed = [];
  const dispose = createEffect(() => {
    observed.push(product.value);
  });

  assert.deepEqual(observed, [6], "Effect runs immediately with initial computed value");

  count.value = 4;
  assert.equal(product.value, 12);
  assert.deepEqual(observed, [6, 12]);

  // Setting identical value must not re-trigger effect
  count.value = 4;
  assert.deepEqual(observed, [6, 12]);

  // Batching multiple signal writes triggers effect only once at the end of batch
  batch(() => {
    count.value = 10;
    multiplier.value = 5;
  });
  assert.equal(product.value, 50);
  assert.deepEqual(observed, [6, 12, 50]);

  dispose();
  count.value = 20;
  assert.deepEqual(observed, [6, 12, 50], "Disposed effect must not run");
});

test("ReactiveStore: createAppReactiveStore synchronizes voiceStates, activeVoiceConnection, and dmCallStage", () => {
  const store = createAppReactiveStore();
  /** @type {Array<{inCall: boolean, participantCount: number, anyStreaming: boolean}>} */
  const snapshots = [];

  const stop = createEffect(() => {
    snapshots.push(store.dmCallStageSummary.value);
  });

  assert.deepEqual(snapshots.at(-1), {
    inCall: false,
    channelId: null,
    participantCount: 0,
    anyStreaming: false,
  });

  store.applyLivePushPayload({
    voiceConnection: {
      guildId: "@me",
      channelId: "1566834523038294047",
      channelName: "DemoUser",
      selfMute: false,
      selfDeaf: false,
      selfVideo: false,
      selfStream: true,
    },
    voiceStates: {
      "@me": [
        {
          user_id: "1566799713523466370",
          channel_id: "1566834523038294047",
          self_stream: true,
        },
        {
          user_id: "384299970632876032",
          channel_id: "1566834523038294047",
          self_stream: false,
        },
      ],
    },
  });

  assert.deepEqual(snapshots.at(-1), {
    inCall: true,
    channelId: "1566834523038294047",
    participantCount: 2,
    anyStreaming: true,
  });

  stop();
});
