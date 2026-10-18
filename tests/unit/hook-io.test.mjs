// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { openCallLog, waitForStop } from "../../src/core/hook-io.mjs";

/** @extends {EventEmitter} */
class FakeStream extends EventEmitter {
  /** @type {string[]} */
  writes = [];
  ended = false;

  /** @param {string} line */
  write(line) {
    this.writes.push(line);
  }

  end() {
    this.ended = true;
  }
}

describe("openCallLog", () => {
  it("добавляет строки в реальный temp-файл и закрывается без ошибок", async () => {
    const dir = await mkdtemp(join(tmpdir(), "hook-io-"));
    const path = join(dir, "calls.jsonl");
    try {
      const writer = await openCallLog(path);
      writer.write("a\n");
      writer.write("b\n");
      assert.equal(await writer.close(), 0);
      assert.equal(await readFile(path, "utf8"), "a\nb\n");
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });

  it("отклоняет ошибку открытия исходной ошибкой", async () => {
    const failure = Object.assign(new Error("EACCES: permission denied"), { code: "EACCES" });
    await assert.rejects(
      openCallLog("ignored", {
        createStream: () => {
          const stream = new FakeStream();
          queueMicrotask(() => stream.emit("error", failure));
          return stream;
        },
      }),
      (error) => error === failure && error instanceof Error && /EACCES/.test(error.message),
    );
  });

  it("считает ошибки после открытия и возвращает тот же результат при повторном close", async () => {
    const stream = new FakeStream();
    const opening = openCallLog("ignored", { createStream: () => stream });
    stream.emit("open", 1);
    const writer = await opening;
    stream.emit("error", new Error("write failed"));
    stream.emit("close");
    assert.equal(await writer.close(), 1);
    assert.equal(await writer.close(), 1);
  });

  it("close ждёт событие close после end", async () => {
    const stream = new FakeStream();
    const opening = openCallLog("ignored", { createStream: () => stream });
    stream.emit("open", 1);
    const writer = await opening;
    let settled = false;
    const closing = writer.close().then((count) => {
      settled = true;
      return count;
    });
    await Promise.resolve();
    assert.equal(stream.ended, true);
    assert.equal(settled, false);
    stream.emit("close");
    assert.equal(await closing, 0);
    assert.equal(settled, true);
  });
});

describe("waitForStop", () => {
  it("при duration 0 завершается по сигналу и снимает оба обработчика", async () => {
    const signals = new EventEmitter();
    const waiting = waitForStop(0, { signals });
    signals.emit("SIGINT");
    assert.equal(await waiting, "signal");
    assert.equal(signals.listenerCount("SIGINT"), 0);
    assert.equal(signals.listenerCount("SIGTERM"), 0);
  });

  it("по таймауту снимает слушателей, последующий сигнал не меняет исход", async () => {
    const signals = new EventEmitter();
    const timer = /** @type {{ callback?: () => void }} */ ({});
    /** @type {(ReturnType<typeof setTimeout> | number)[]} */
    const cleared = [];
    const result = waitForStop(20, {
      signals,
      setTimeoutFn: (fn) => {
        timer.callback = fn;
        return 7;
      },
      clearTimeoutFn: (handle) => cleared.push(handle),
    });
    assert.equal(typeof timer.callback, "function");
    timer.callback?.();
    assert.equal(await result, "timeout");
    assert.equal(signals.listenerCount("SIGINT"), 0);
    assert.equal(signals.listenerCount("SIGTERM"), 0);
    signals.emit("SIGINT");
    assert.deepEqual(cleared, [7]);
  });

  it("сигнал раньше таймера очищает таймер", async () => {
    const signals = new EventEmitter();
    /** @type {(ReturnType<typeof setTimeout> | number)[]} */
    const cleared = [];
    const waiting = waitForStop(100, {
      signals,
      setTimeoutFn: () => 9,
      clearTimeoutFn: (handle) => cleared.push(handle),
    });
    signals.emit("SIGTERM");
    assert.equal(await waiting, "signal");
    assert.deepEqual(cleared, [9]);
    assert.equal(signals.listenerCount("SIGINT"), 0);
    assert.equal(signals.listenerCount("SIGTERM"), 0);
  });
});
