// @ts-check
/**
 * @fileoverview High-performance reactive state management layer powered by
 * `@preact/signals-core` providing fine-grained reactivity, computed signals,
 * reactive effects, and atomic batched updates.
 */

import {
  signal as coreSignal,
  computed as coreComputed,
  effect as coreEffect,
  batch as coreBatch,
} from "@preact/signals-core";

export const isSignalsCorePowered = true;

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
 * Executes a function within an atomic batch. Signal updates during the batch
 * do not trigger effects immediately; all pending effects are coalesced
 * and executed once when the outermost batch completes.
 *
 * @template R
 * @param {() => R} fn - The callback containing signal mutations to batch.
 * @returns {R} The return value of the callback.
 */
export const batch = coreBatch;

/**
 * Creates a reactive signal holding an initial value.
 *
 * @template T
 * @param {T} initialValue - The initial value stored in the signal.
 * @returns {Signal<T>} A reactive signal object with value getter/setter, peek, and subscribe.
 */
export function signal(initialValue) {
  const s = coreSignal(initialValue);
  return {
    get value() {
      return s.value;
    },
    set value(next) {
      if (Object.is(s.peek(), next)) {
        return;
      }
      s.value = next;
    },
    peek() {
      return s.peek();
    },
    subscribe(listener) {
      return s.subscribe(listener);
    },
  };
}

/**
 * Creates a computed, read-only signal derived from other signals.
 *
 * @template T
 * @param {() => T} computeFn - Pure function deriving the computed value.
 * @returns {ReadonlySignal<T>} A read-only signal that lazily caches and reacts to dependency changes.
 */
export function computed(computeFn) {
  return /** @type {ReadonlySignal<T>} */ (coreComputed(computeFn));
}

/**
 * Creates an effect that runs immediately and re-runs whenever its read signals update.
 * If the effect callback returns a function, that function is invoked before re-running
 * and upon effect disposal.
 *
 * @param {() => void | (() => void)} fn - Effect function to run and track dependencies.
 * @returns {() => void} Dispose function that permanently tears down the effect and runs its cleanup.
 */
export function effect(fn) {
  return coreEffect(fn);
}
