// @ts-check
import { createWriteStream } from "node:fs";
import { EventEmitter } from "node:events";

/**
 * @typedef {EventEmitter & { write(line: string): unknown; end(): unknown }} CallLogStream
 * @typedef {object} HookIoOptions
 * @property {(out: string, opts: { flags: "a" }) => CallLogStream} [createStream]
 * @typedef {object} StopOptions
 * @property {EventEmitter} [signals]
 * @property {(callback: () => void, durationMs: number) => (ReturnType<typeof setTimeout> | number)} [setTimeoutFn]
 * @property {(handle: ReturnType<typeof setTimeout> | number) => void} [clearTimeoutFn]
 */

/**
 * Открывает JSONL-журнал в режиме дозаписи.
 * @param {string} out
 * @param {HookIoOptions} [options]
 * @returns {Promise<{ write(line: string): void; close(): Promise<number>; readonly writeErrors: number }>}
 */
export async function openCallLog(out, options = {}) {
  const stream = options.createStream
    ? options.createStream(out, { flags: "a" })
    : /** @type {CallLogStream} */ (/** @type {unknown} */ (createWriteStream(out, { flags: "a" })));

  await new Promise((resolve, reject) => {
    const onOpen = () => {
      stream.removeListener("error", onOpenError);
      resolve(undefined);
    };
    /** @param {unknown} error */
    const onOpenError = (error) => {
      stream.removeListener("open", onOpen);
      reject(error);
    };
    stream.once("open", onOpen);
    stream.once("error", onOpenError);
  });

  let writeErrors = 0;
  let closed = false;
  /** @type {Promise<number> | undefined} */
  let closePromise;
  stream.on("error", () => {
    writeErrors += 1;
  });
  stream.once("close", () => {
    closed = true;
  });

  return {
    write(line) {
      stream.write(line);
    },
    close() {
      if (closed) return Promise.resolve(writeErrors);
      if (!closePromise) {
        closePromise = new Promise((resolve) => {
          stream.once("close", () => resolve(writeErrors));
          stream.end();
        });
      }
      return closePromise;
    },
    get writeErrors() {
      return writeErrors;
    },
  };
}

/**
 * Ждёт первый сигнал остановки или истечение лимита времени.
 * @param {number} [durationMs]
 * @param {StopOptions} [options]
 * @returns {Promise<"signal" | "timeout">}
 */
export function waitForStop(durationMs = 0, options = {}) {
  const signals = options.signals ?? process;
  const setTimeoutFn = options.setTimeoutFn ?? globalThis.setTimeout;
  const clearTimeoutFn = options.clearTimeoutFn ?? globalThis.clearTimeout;

  return new Promise((resolve) => {
    /** @type {ReturnType<typeof setTimeout> | number | undefined} */
    let timer;
    let settled = false;
    const cleanup = () => {
      signals.removeListener("SIGINT", onSignal);
      signals.removeListener("SIGTERM", onSignal);
      if (durationMs > 0 && timer !== undefined) clearTimeoutFn(timer);
    };
    /** @param {"signal" | "timeout"} result */
    const finish = (result) => {
      if (settled) return;
      settled = true;
      cleanup();
      resolve(result);
    };
    const onSignal = () => finish("signal");
    signals.once("SIGINT", onSignal);
    signals.once("SIGTERM", onSignal);
    if (durationMs > 0) {
      timer = setTimeoutFn(() => finish("timeout"), durationMs);
    }
  });
}
