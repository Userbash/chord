// @ts-check
// RED (M1): реализация — src/core/ui/profile.mjs. Контракт: topFunctions
// (паритет ui-walk-settings2 / ui-walk-menus / ui-menu-extra).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { topFunctions } from "../../src/core/ui/profile.mjs";

/** @typedef {import("../../src/core/types.js").CpuProfile} CpuProfile */
/** @typedef {import("../../src/core/types.js").CpuProfileNode} CpuProfileNode */

/**
 * @param {number | undefined} hitCount
 * @param {string | undefined} functionName
 * @param {string | undefined} url
 * @returns {CpuProfileNode}
 */
const node = (hitCount, functionName, url) => ({
  hitCount,
  callFrame: { functionName, url },
});

const walkFixtures = [
  fileURLToPath(new URL("../fixtures/logs/walk-settings.jsonl", import.meta.url)),
  fileURLToPath(new URL("../fixtures/logs/walk-menus.jsonl", import.meta.url)),
];

describe("topFunctions", () => {
  it("сортирует по hits по убыванию; url — последний сегмент пути", () => {
    /** @type {CpuProfile} */
    const profile = {
      nodes: [
        node(5, "alpha", "https://discord.com/assets/a.js"),
        node(9, "beta", "https://discord.com/assets/web.123.js"),
      ],
    };
    assert.deepEqual(topFunctions(profile), [
      { fn: "beta", url: "web.123.js", hits: 9 },
      { fn: "alpha", url: "a.js", hits: 5 },
    ]);
  });

  it("отбрасывает узлы без hitCount (0 и undefined)", () => {
    const profile = {
      nodes: [node(3, "kept", "x.js"), node(0, "zero", "x.js"), node(undefined, "none", "x.js")],
    };
    assert.deepEqual(topFunctions(profile), [{ fn: "kept", url: "x.js", hits: 3 }]);
  });

  it("отбрасывает шум: (idle), (program), (garbage collector), (root) и пустые имена", () => {
    /** @type {CpuProfile} */
    const profile = {
      nodes: [
        node(1, "(idle)", "x.js"),
        node(2, "(program)", "x.js"),
        node(3, "(garbage collector)", "x.js"),
        node(4, "(root)", "x.js"),
        node(8, "", "x.js"),
      ],
    };
    assert.deepEqual(topFunctions(profile), []);
  });

  it("обрезает url до 60 символов последнего сегмента; пустой url допустим", () => {
    const longSegment = `${"z".repeat(80)}.js`;
    const profile = {
      nodes: [node(2, "long", `https://cdn.example/${longSegment}`), node(1, "nourl", "")],
    };
    assert.deepEqual(topFunctions(profile), [
      { fn: "long", url: "z".repeat(60), hits: 2 },
      { fn: "nourl", url: "", hits: 1 },
    ]);
  });

  it("по умолчанию не больше 8 записей; limit можно задать", () => {
    const nodes = Array.from({ length: 10 }, (_, index) => node(10 - index, `fn${index}`, "x.js"));
    assert.equal(topFunctions({ nodes }).length, 8);
    assert.deepEqual(
      topFunctions({ nodes }, 2).map((entry) => entry.fn),
      ["fn0", "fn1"],
    );
  });

  it("при равных hits сохраняется исходный порядок (стабильная сортировка)", () => {
    const profile = { nodes: [node(3, "first", "x.js"), node(3, "second", "x.js")] };
    assert.deepEqual(
      topFunctions(profile).map((entry) => entry.fn),
      ["first", "second"],
    );
  });

  // Независимый оракул: node --input-type=module -e 'import {readFileSync} from "node:fs"; const paths=["tests/fixtures/logs/walk-settings.jsonl","tests/fixtures/logs/walk-menus.jsonl"]; const records=paths.flatMap(p=>readFileSync(p,"utf8").trim().split("\n").map(JSON.parse)); const nodes=records.flatMap(r=>r.topFunctions); const noise=new Set(["(idle)","(program)","(garbage collector)","(root)","(anonymous)"]); const valid=nodes.filter(n=>n.hits&&!noise.has(n.fn||"(anonymous)")).sort((a,b)=>b.hits-a.hits).slice(0,8).map(n=>({fn:n.fn||"(anonymous)",url:(n.url||"").split("/").pop()?.slice(0,60)??"",hits:n.hits})); console.log(JSON.stringify({nodes:nodes.length,filtered:nodes.filter(n=>n.hits&&!noise.has(n.fn||"(anonymous)")).length,result:valid}));' → 771 узел/697 после фильтра.
  it("фильтрует шум и совпадает с golden top-8 реальных журналов", async () => {
    const records = (await Promise.all(walkFixtures.map(async (path) =>
      (await readFile(path, "utf8")).trim().split("\n").map((line) => JSON.parse(line)),
    ))).flat();
    const source = records.flatMap((record) => record.topFunctions);
    assert.ok(source.some((entry) => entry.fn === "(idle)"));
    assert.ok(source.some((entry) => entry.fn === "(program)"));
    const result = topFunctions({ nodes: source.map((entry) => node(entry.hits, entry.fn, entry.url)) });
    assert.ok(result.length <= 8);
    assert.ok(result.every((entry, index) => index === 0 || (result[index - 1]?.hits ?? 0) >= entry.hits));
    assert.ok(result.every((entry) => entry.fn !== "(idle)" && entry.fn !== "(program)"));
    assert.deepEqual(result, [
      { fn: "getShaderInfoLog", url: "", hits: 191 },
      { fn: "getContext", url: "", hits: 26 },
      { fn: "focus", url: "", hits: 25 },
      { fn: "focus", url: "", hits: 20 },
      { fn: "focus", url: "", hits: 19 },
      { fn: "getBoundingClientRect", url: "", hits: 16 },
      { fn: "callback", url: "web.4d4b2f75fdea5470.js", hits: 15 },
      { fn: "focus", url: "", hits: 15 },
    ]);
  });

  it("обрезает самый длинный реальный URL до 60 символов сегмента", async () => {
    const records = await Promise.all(walkFixtures.map(async (path) =>
      (await readFile(path, "utf8")).trim().split("\n").map((line) => JSON.parse(line)),
    ));
    /** @type {string[]} */
    const urls = records.flat().flatMap((record) => record.topFunctions.map((/** @type {{ url: string }} */ entry) => entry.url));
    const longest = urls.reduce((best, url) => url.length > best.length ? url : best, "");
    assert.equal(longest, "34aec3bbe8b005d0.module.wasm");
    const result = topFunctions({ nodes: [node(2, "long-real-url", `https://cdn.discordapp.com/${longest}`)] });
    assert.equal(result[0]?.url, longest.slice(0, 60));
  });
});
