// @ts-check
// RED (M1): реализация — src/core/args.mjs. Контракт: parseArgs, паритет scripts/logger.mjs.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { parseArgs, parseNumericArg, parseRegexArg } from "../../src/core/args.mjs";

describe("parseNumericArg", () => {
  it("разбирает целое число и принимает включительные границы", () => {
    assert.equal(parseNumericArg("--x", "42"), 42);
    assert.equal(parseNumericArg("--x", "1", { min: 1, max: 2 }), 1);
    assert.equal(parseNumericArg("--x", "2", { min: 1, max: 2 }), 2);
  });

  it("отклоняет нечисловые, дробные, вне диапазона, флаг и небезопасные значения", () => {
    for (const value of ["abc", "1.5", "0", true, "9999999999999999999"]) {
      assert.throws(() => parseNumericArg("--x", value, { min: 1 }));
    }
  });

  it("принимает целое число и отклоняет нецелое", () => {
    assert.equal(parseNumericArg("--x", 42), 42);
    assert.throws(() => parseNumericArg("--x", 1.5));
  });
});

describe("parseRegexArg", () => {
  it("компилирует regex, отклоняет синтаксическую ошибку и длину больше лимита", () => {
    assert.ok(parseRegexArg("--pattern", "Task|Poll") instanceof RegExp);
    assert.throws(() => parseRegexArg("--pattern", "("), /regex/i);
    assert.throws(() => parseRegexArg("--pattern", "x".repeat(513)), /regex/i);
  });

  it("undefined-паттерн даёт пустой regex", () => {
    assert.equal(parseRegexArg("--pattern", undefined).source, "(?:)");
  });
});

describe("parseArgs", () => {
  it("пустой argv → пустые _ и modules", () => {
    assert.deepEqual(parseArgs([]), { _: [], modules: [] });
  });

  it("позиционные токены попадают в _ в порядке появления", () => {
    const parsed = parseArgs(["hook", "extra"]);
    assert.deepEqual(parsed._, ["hook", "extra"]);
  });

  it("--key value съедает следующий токен как значение", () => {
    const parsed = parseArgs(["hook", "--pid", "123"]);
    assert.equal(parsed.pid, "123");
    assert.deepEqual(parsed._, ["hook"]);
  });

  it("--key=value берёт значение из токена", () => {
    const parsed = parseArgs(["--limit=32"]);
    assert.equal(parsed.limit, "32");
  });

  it("--flag без значения → true", () => {
    const parsed = parseArgs(["--spawn"]);
    assert.equal(parsed.spawn, true);
  });

  it("--key= даёт пустую строку, а не true", () => {
    const parsed = parseArgs(["--kind="]);
    assert.equal(parsed.kind, "");
  });

  it("флаг перед другим флагом не съедает его", () => {
    const parsed = parseArgs(["--pid", "--spawn"]);
    assert.equal(parsed.pid, true);
    assert.equal(parsed.spawn, true);
  });

  it("--module повторяется и копится в modules (ключа module в результате нет)", () => {
    const parsed = parseArgs(["--module", "a.node", "--module=b.node"]);
    assert.deepEqual(parsed.modules, ["a.node", "b.node"]);
    assert.equal(parsed.module, undefined);
  });

  it("числовые значения остаются строками", () => {
    const parsed = parseArgs(["--duration-ms", "5000", "--arg-count", "2"]);
    assert.equal(parsed["duration-ms"], "5000");
    assert.equal(parsed["arg-count"], "2");
  });

  it("значение может содержать любые символы (regex, слеши)", () => {
    const parsed = parseArgs(["--pattern", "Task::Run|Poll(\\d+)"]);
    assert.equal(parsed.pattern, "Task::Run|Poll(\\d+)");
  });

  it("разбор продолжается после пары ключ-значение", () => {
    const parsed = parseArgs(["cdp", "eval", "--out", "/tmp/x.json"]);
    assert.deepEqual(parsed._, ["cdp", "eval"]);
    assert.equal(parsed.out, "/tmp/x.json");
  });

  // Паритет: scripts/logger.mjs parseArgs присваивает parsed[key] при каждом появлении ключа.
  it("повтор обычного ключа перезаписывает предыдущее значение (scripts/logger.mjs)", () => {
    const parsed = parseArgs(["--pid", "1", "--pid=2"]);
    assert.equal(parsed.pid, "2");
  });

  // Паритет: const [key, inline] = token.slice(2).split("=") отбрасывает последующие части.
  it("inline-значение с несколькими равно сохраняет только сегмент после первого (паритет старого split('='))", () => {
    const parsed = parseArgs(["--pattern=a=b"]);
    assert.equal(parsed.pattern, "a");
  });

  // В старом цикле нет особой ветки для `--`: startsWith("--"), slice(2) даёт пустой ключ.
  it("старый парсер трактует -- как пустой ключ-флаг и продолжает разбор", () => {
    const parsed = parseArgs(["--", "--pid", "7"]);
    assert.equal(parsed[""], true);
    assert.deepEqual(parsed._, []);
    assert.equal(parsed.pid, "7");
  });

  it("--_ поглощает значение, не перезаписывая позиционные аргументы", () => {
    assert.deepEqual(parseArgs(["--_", "x", "cmd"]), { _: ["cmd"], modules: [] });
  });

  it("--modules не перезаписывает массив modules", () => {
    assert.deepEqual(parseArgs(["--modules", "a,b"]), { _: [], modules: [] });
  });

  it("служебные ключи прототипа не записываются в результат", () => {
    const parsed = parseArgs(["--__proto__", "p", "--constructor", "c", "--prototype=q", "cmd"]);
    assert.deepEqual(parsed, { _: ["cmd"], modules: [] });
    assert.equal(Object.hasOwn(parsed, "constructor"), false);
    assert.equal(Object.getPrototypeOf(parsed), Object.prototype);
    assert.equal(Object.hasOwn(parsed, "__proto__"), false);
  });

  it("--module продолжает копить повторяющиеся значения", () => {
    assert.deepEqual(parseArgs(["--module", "A", "--module=B"]).modules, ["A", "B"]);
  });
});
