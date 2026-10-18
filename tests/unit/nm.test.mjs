// @ts-check
// RED (M1): реализация — src/core/report/nm.mjs. Контракт: разбор nm
// (паритет scripts/extract-static-functions.sh).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { parseNmPosix, staticFunctionsCsv } from "../../src/core/report/nm.mjs";

/** @param {string} name */
const fixture = (name) => readFileSync(fileURLToPath(new URL(`../fixtures/nm/${name}`, import.meta.url)), "utf8");

const NM_TEXT = [
  "0000000000000000 T _start",
  "0000000000000020 t local_helper",
  "0000000000000030 W weak_fn 16",
  "0000000000000040 D data_object 4",
  "0000000000000050 U undefined_ref",
  "0000000000000060 w another_weak 8",
  "updater.node: no symbols",
  "garbage",
  "",
].join("\n");

describe("parseNmPosix", () => {
  it("берёт только T/t/W/w; сохраняет адрес и size; короткие строки пропускает", () => {
    assert.deepEqual(parseNmPosix(NM_TEXT), [
      { name: "_start", type: "T", address: "0000000000000000", size: "" },
      { name: "local_helper", type: "t", address: "0000000000000020", size: "" },
      { name: "weak_fn", type: "W", address: "0000000000000030", size: "16" },
      { name: "another_weak", type: "w", address: "0000000000000060", size: "8" },
    ]);
  });

  it("пустой текст → пустой список", () => {
    assert.deepEqual(parseNmPosix(""), []);
  });
});

describe("staticFunctionsCsv", () => {
  it("строки name,type,address,size без финального перевода строки", () => {
    /** @type {import("../../src/core/types.js").NmEntry[]} */
    const entries = [
      { name: "_start", type: "T", address: "0000000000000000", size: "" },
      { name: "weak_fn", type: "W", address: "0000000000000030", size: "16" },
    ];
    assert.equal(
      staticFunctionsCsv(entries),
      "_start,T,0000000000000000,\nweak_fn,W,0000000000000030,16",
    );
  });

  it("совпадает с awk golden для префикса discord_utils", () => {
    // Источник functions/static/discord_utils.nm.txt; md5 958acab93b2e414bcc62800c6d205979.
    // head -n 30; оракул: awk 'NF>=3 && $2~/^[TtWw]$/ {printf "%s,%s,%s,%s\\n",$1,$2,$3,($4!=""?$4:"")}' .
    // Паритетный полный CSV: md5 872c8da313caee5efc87eb0825d6fadf; скрипт пишет без заголовка,
    // как и functions/static/discord_utils.csv (первая/последняя строки совпадают с форматом); копию
    // целиком не дублируем (353528 байт).
    const input = fixture("discord_utils.head.nm.txt");
    const expected = fixture("discord_utils.head.expected.csv").replace(/\n$/, "");
    assert.equal(parseNmPosix(input).length, 27); // awk NF>=3 && $2~/^[TtWw]$/
    assert.equal(input.trimEnd().split("\n").length - parseNmPosix(input).length, 3);
    assert.equal(staticFunctionsCsv(parseNmPosix(input)), expected);
    assert.equal(expected.split("\n").length, 27);
    assert.deepEqual(expected.split("\n").slice(0, 3), [
      "abort_message,t,fc5b0,c6",
      "__clang_call_terminate,t,b2e90,b",
      "__cxa_allocate_dependent_exception,t,fc7b0,39",
    ]);
    assert.match(expected.split("\n")[0] ?? "", /^abort_message,t,fc5b0,c6$/);
    assert.match(expected.split("\n").at(-1) ?? "", /^__cxa_uncaught_exceptions,t,fcd00,1c$/);
    assert.equal(parseNmPosix("only two\nfields").length, 0);
  });

  it("берёт четыре реальных типа и пустой size из voice sample", () => {
    // Источник functions/static/discord_voice.nm.txt; md5 bd7bd35ab5ed4b5648b0881c808d1473.
    // Выборка awk: первая строка каждого существующего типа T/t/W и первая функция NF==3.
    // Проверка всех functions/static/*.nm.txt через awk '$2=="w"' не нашла ни одного w.
    const entries = parseNmPosix(fixture("discord_voice.sample.nm.txt"));
    assert.deepEqual([...new Set(entries.map(({ type }) => type))].sort(), ["T", "W", "t"]);
    assert.ok(entries.some(({ size }) => size === ""));
  });
});
