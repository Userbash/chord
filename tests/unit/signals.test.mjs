// @ts-check
import assert from "node:assert/strict";
import test from "node:test";
import {
  signal,
  computed,
  effect,
  batch,
} from "../../rust/gui-spike/ui/state/signals.js";

test("signals: signal() basic read, write, peek, and Object.is equality", () => {
  const count = signal(0);
  assert.equal(count.value, 0);
  assert.equal(count.peek(), 0);

  count.value = 5;
  assert.equal(count.value, 5);
  assert.equal(count.peek(), 5);

  // NaN equality check
  const nanSignal = signal(NaN);
  assert.ok(Number.isNaN(nanSignal.value));
  let runs = 0;
  const dispose = effect(() => {
    nanSignal.value;
    runs += 1;
  });
  assert.equal(runs, 1);
  nanSignal.value = NaN; // Should not trigger update because Object.is(NaN, NaN) is true
  assert.equal(runs, 1);
  dispose();
});

test("signals: effect() tracks dependencies and reacts to changes", () => {
  const name = signal("Alice");
  const age = signal(25);
  /** @type {string[]} */
  const logs = [];

  const dispose = effect(() => {
    logs.push(`${name.value} is ${age.value}`);
  });

  assert.deepEqual(logs, ["Alice is 25"]);

  name.value = "Bob";
  assert.deepEqual(logs, ["Alice is 25", "Bob is 25"]);

  age.value = 30;
  assert.deepEqual(logs, ["Alice is 25", "Bob is 25", "Bob is 30"]);

  dispose();
  name.value = "Charlie";
  age.value = 35;
  assert.deepEqual(logs, ["Alice is 25", "Bob is 25", "Bob is 30"], "Disposed effect stops listening");
});

test("signals: effect() executes cleanup function on rerun and dispose", () => {
  const toggle = signal(1);
  /** @type {string[]} */
  const events = [];

  const dispose = effect(() => {
    const current = toggle.value;
    events.push(`run:${current}`);
    return () => {
      events.push(`cleanup:${current}`);
    };
  });

  assert.deepEqual(events, ["run:1"]);

  toggle.value = 2;
  assert.deepEqual(events, ["run:1", "cleanup:1", "run:2"]);

  dispose();
  assert.deepEqual(events, ["run:1", "cleanup:1", "run:2", "cleanup:2"]);
});

test("signals: peek() does not track dependencies inside effect or computed", () => {
  const tracked = signal(1);
  const untracked = signal(100);
  /** @type {number[]} */
  const results = [];

  const dispose = effect(() => {
    results.push(tracked.value + untracked.peek());
  });

  assert.deepEqual(results, [101]);

  untracked.value = 200;
  assert.deepEqual(results, [101], "Changing untracked signal via peek does not rerun effect");

  tracked.value = 2;
  assert.deepEqual(results, [101, 202], "Changing tracked signal reads latest peek value");

  dispose();
});

test("signals: computed() memoization and reactive updates", () => {
  const a = signal(2);
  const b = signal(3);
  let computations = 0;

  const sum = computed(() => {
    computations += 1;
    return a.value + b.value;
  });

  assert.equal(computations, 0, "Computed is lazy or initialized on first read");
  assert.equal(sum.value, 5);
  assert.equal(computations, 1);

  // Re-reading without changes uses cached value
  assert.equal(sum.value, 5);
  assert.equal(computations, 1);

  a.value = 10;
  assert.equal(sum.value, 13);
  assert.equal(computations, 2);

  // Peek on computed returns value without subscribing
  assert.equal(sum.peek(), 13);
});

test("signals: dynamic dependency branching in effect and computed", () => {
  const cond = signal(true);
  const left = signal("L");
  const right = signal("R");
  /** @type {string[]} */
  const values = [];

  const choice = computed(() => (cond.value ? left.value : right.value));

  const dispose = effect(() => {
    values.push(choice.value);
  });

  assert.deepEqual(values, ["L"]);

  // Updating right should not trigger effect while cond is true
  right.value = "R2";
  assert.deepEqual(values, ["L"]);

  // Updating left should trigger
  left.value = "L2";
  assert.deepEqual(values, ["L", "L2"]);

  // Switch branch to right
  cond.value = false;
  assert.deepEqual(values, ["L", "L2", "R2"]);

  // Now updating left should not trigger
  left.value = "L3";
  assert.deepEqual(values, ["L", "L2", "R2"]);

  // Updating right triggers
  right.value = "R3";
  assert.deepEqual(values, ["L", "L2", "R2", "R3"]);

  dispose();
});

test("signals: diamond dependency resolves cleanly without glitch", () => {
  //       s
  //      / \
  //     a   b
  //      \ /
  //       c
  const s = signal(1);
  const a = computed(() => s.value * 2);
  const b = computed(() => s.value * 10);
  const c = computed(() => `${a.value}+${b.value}`);

  /** @type {string[]} */
  const runs = [];
  const dispose = effect(() => {
    runs.push(c.value);
  });

  assert.deepEqual(runs, ["2+10"]);

  s.value = 2;
  // c should evaluate to "4+20" in a single step
  assert.deepEqual(runs, ["2+10", "4+20"]);

  dispose();
});

test("signals: batch() coalesces multiple signal updates", () => {
  const first = signal("Hello");
  const last = signal("World");
  let effectRuns = 0;

  const dispose = effect(() => {
    effectRuns += 1;
    `${first.value} ${last.value}`;
  });

  assert.equal(effectRuns, 1);

  const result = batch(() => {
    first.value = "Hi";
    last.value = "Earth";
    return "batch_done";
  });

  assert.equal(result, "batch_done");
  assert.equal(effectRuns, 2, "Effect ran only once after batch completed");

  // Nested batches
  batch(() => {
    first.value = "Hey";
    batch(() => {
      last.value = "Universe";
    });
    // intermediate state should not trigger effects yet
    assert.equal(effectRuns, 2);
  });

  assert.equal(effectRuns, 3);
  dispose();
});

test("signals: batch() safely restores depth on thrown error", () => {
  const s = signal(0);
  let runs = 0;
  const dispose = effect(() => {
    s.value;
    runs += 1;
  });
  assert.equal(runs, 1);

  assert.throws(
    () => {
      batch(() => {
        s.value = 1;
        throw new Error("Batch failed");
      });
    },
    /Batch failed/
  );

  // Further updates outside batch work normally
  s.value = 2;
  assert.equal(runs, 3);
  dispose();
});

test("signals: signal.subscribe() provides direct subscription API", () => {
  const count = signal(10);
  /** @type {number[]} */
  const history = [];

  const unsubscribe = count.subscribe((val) => {
    history.push(val);
  });

  assert.deepEqual(history, [10], "Initial value received on subscribe");

  count.value = 20;
  count.value = 30;
  assert.deepEqual(history, [10, 20, 30]);

  unsubscribe();
  count.value = 40;
  assert.deepEqual(history, [10, 20, 30]);
});
