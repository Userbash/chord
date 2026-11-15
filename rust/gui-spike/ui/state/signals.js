// @ts-check
/**
 * @fileoverview Lightweight, zero-dependency reactive state management layer
 * providing signals, computed values, reactive effects, and batched updates.
 */

/**
 * @template T
 * @typedef {Object} ReadonlySignal
 * @property {T} value - The current value of the signal. Accessing registers a dependency if inside an observer.
 * @property {() => T} peek - Reads the current value without registering a reactive dependency.
 * @property {(listener: (val: T) => void) => () => void} subscribe - Subscribes to value changes. Returns an unsubscribe function.
 */

/**
 * @template T
 * @typedef {Object} Signal
 * @property {T} value - The current value of the signal. Setting triggers dependent observers.
 * @property {() => T} peek - Reads the current value without registering a reactive dependency.
 * @property {(listener: (val: T) => void) => () => void} subscribe - Subscribes to value changes. Returns an unsubscribe function.
 */

/**
 * Interface for reactive observers (effects and computed signals).
 * @typedef {Object} ObserverNode
 * @property {Set<SignalNode>} deps - Set of signals and computeds observed by this node.
 * @property {() => void} notify - Invoked when an upstream dependency changes.
 */

/**
 * Interface for reactive dependency providers (signals and computed signals).
 * @typedef {Object} SignalNode
 * @property {Set<ObserverNode>} subscribers - Set of observers subscribed to this dependency.
 */

/** @type {ObserverNode[]} */
const observerStack = [];

let batchDepth = 0;
/** @type {Set<{ run: () => void }>} */
const pendingEffects = new Set();

/**
 * Executes a function within a batch. Signal updates during the batch
 * do not trigger effects immediately; all pending effects are coalesced
 * and executed once when the outermost batch completes.
 *
 * @template R
 * @param {() => R} fn - The callback containing signal mutations to batch.
 * @returns {R} The return value of the callback.
 */
export function batch(fn) {
  batchDepth += 1;
  try {
    return fn();
  } finally {
    batchDepth -= 1;
    if (batchDepth === 0) {
      flushEffects();
    }
  }
}

/**
 * Flushes all pending effects queued during a batch transaction.
 */
function flushEffects() {
  while (pendingEffects.size > 0) {
    const toRun = Array.from(pendingEffects);
    pendingEffects.clear();
    for (const effectNode of toRun) {
      effectNode.run();
    }
  }
}

/**
 * Creates a reactive signal holding an initial value.
 *
 * @template T
 * @param {T} initialValue - The initial value stored in the signal.
 * @returns {Signal<T>} A reactive signal object with value getter/setter, peek, and subscribe.
 */
export function signal(initialValue) {
  let currentValue = initialValue;

  /** @type {Set<ObserverNode>} */
  const subscribers = new Set();

  /** @type {SignalNode} */
  const signalNode = { subscribers };

  return {
    get value() {
      const active = observerStack[observerStack.length - 1];
      if (active) {
        active.deps.add(signalNode);
        subscribers.add(active);
      }
      return currentValue;
    },
    set value(nextValue) {
      if (Object.is(currentValue, nextValue)) {
        return;
      }
      currentValue = nextValue;

      if (batchDepth > 0) {
        for (const sub of Array.from(subscribers)) {
          sub.notify();
        }
      } else {
        batch(() => {
          for (const sub of Array.from(subscribers)) {
            sub.notify();
          }
        });
      }
    },
    peek() {
      return currentValue;
    },
    subscribe(listener) {
      return effect(() => {
        listener(this.value);
      });
    },
  };
}

/**
 * Creates a computed (derived) signal whose value is memoized and recomputed
 * whenever any of the signals read during `computeFn` change.
 *
 * @template T
 * @param {() => T} computeFn - The pure derivation function that calculates the computed value.
 * @returns {ReadonlySignal<T>} A readonly reactive signal for the derived value.
 */
export function computed(computeFn) {
  /** @type {T} */
  let cachedValue;
  let isDirty = true;
  let isComputing = false;

  /** @type {Set<ObserverNode>} */
  const subscribers = new Set();

  /** @type {ObserverNode & SignalNode} */
  const computedNode = {
    deps: new Set(),
    subscribers,
    notify() {
      if (!isDirty) {
        isDirty = true;
        for (const sub of Array.from(subscribers)) {
          sub.notify();
        }
      }
    },
  };

  /**
   * Re-evaluates computeFn while tracking dependencies and cleaning up obsolete ones.
   */
  function recompute() {
    if (isComputing) {
      throw new Error("Cycle detected in computed evaluation");
    }
    isComputing = true;
    const prevDeps = computedNode.deps;
    computedNode.deps = new Set();
    observerStack.push(computedNode);

    try {
      cachedValue = computeFn();
      isDirty = false;
    } finally {
      observerStack.pop();
      isComputing = false;

      // Unsubscribe from dependencies that were not accessed in this run
      for (const dep of prevDeps) {
        if (!computedNode.deps.has(dep)) {
          dep.subscribers.delete(computedNode);
        }
      }
    }
  }

  return {
    get value() {
      const active = observerStack[observerStack.length - 1];
      if (active) {
        active.deps.add(computedNode);
        subscribers.add(active);
      }

      if (isDirty) {
        recompute();
      }
      return cachedValue;
    },
    peek() {
      if (isDirty) {
        recompute();
      }
      return cachedValue;
    },
    subscribe(listener) {
      return effect(() => {
        listener(this.value);
      });
    },
  };
}

/**
 * Creates a reactive effect that immediately runs `fn` and tracks all reactive
 * signals or computed values accessed during execution. When any dependency
 * changes, `fn` is re-executed.
 *
 * If `fn` returns a cleanup function, it will be called before each re-execution
 * and when the effect is disposed.
 *
 * @param {() => (void | (() => void))} fn - The effect callback to execute.
 * @returns {() => void} A dispose function that stops the effect and cleans up subscriptions.
 */
export function effect(fn) {
  let disposed = false;
  let isRunning = false;
  /** @type {(() => void) | void} */
  let cleanup;

  /** @type {ObserverNode & { run: () => void }} */
  const effectNode = {
    deps: new Set(),
    notify() {
      if (disposed) {
        return;
      }
      if (batchDepth > 0) {
        pendingEffects.add(effectNode);
      } else {
        batch(() => {
          pendingEffects.add(effectNode);
        });
      }
    },
    run() {
      if (disposed || isRunning) {
        return;
      }
      if (typeof cleanup === "function") {
        try {
          cleanup();
        } catch {
          // Suppress cleanup error to avoid breaking reactive update cycles
        }
        cleanup = undefined;
      }

      isRunning = true;
      const prevDeps = effectNode.deps;
      effectNode.deps = new Set();
      observerStack.push(effectNode);

      try {
        cleanup = fn();
      } finally {
        observerStack.pop();
        isRunning = false;

        // Unsubscribe from dependencies that were not accessed in this run
        for (const dep of prevDeps) {
          if (!effectNode.deps.has(dep)) {
            dep.subscribers.delete(effectNode);
          }
        }
      }
    },
  };

  // Run immediately on creation
  effectNode.run();

  return function dispose() {
    if (disposed) {
      return;
    }
    disposed = true;
    pendingEffects.delete(effectNode);

    if (typeof cleanup === "function") {
      try {
        cleanup();
      } catch {
        // Suppress cleanup error to avoid breaking reactive update cycles
      }
      cleanup = undefined;
    }

    for (const dep of effectNode.deps) {
      dep.subscribers.delete(effectNode);
    }
    effectNode.deps.clear();
  };
}
