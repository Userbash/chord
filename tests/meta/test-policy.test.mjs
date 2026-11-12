// @ts-check
// Мета-тест политики: не даёт тихо ослабить тесты и отключить проверки.
// Сама политика — TEST-POLICY.md; правила — §2, §3, §8.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));

/** @param {string} dir @returns {string[]} */
function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "node_modules" || entry.name === "target") return [];
      return walk(path);
    }
    return [path];
  });
}

/** Исходник гейта: из него извлекаются чистые функции правил (как в keyring-live.test.mjs). */
const gateSource = readFileSync(join(root, "scripts", "gate.mjs"), "utf8");

/**
 * Тело функции по имени: от `function name(` до парной закрывающей скобки.
 * @param {string} source
 * @param {string} name
 * @returns {string}
 */
function extractFunction(source, name) {
  const start = source.indexOf(`function ${name}(`);
  assert.notEqual(start, -1, `в scripts/gate.mjs нет функции ${name}`);
  let depth = 0;
  for (let index = source.indexOf("{", start); index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    else if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(start, index + 1);
    }
  }
  return assert.fail(`${name}: непарные скобки`);
}

/**
 * Исполняет чистую функцию из gate.mjs в изоляции и возвращает её.
 * @param {string} name
 * @returns {(...args: any[]) => any}
 */
function loadGateFunction(name) {
  return new Function(`${extractFunction(gateSource, name)}; return ${name};`)();
}

describe("TEST-POLICY: запрет ослабления тестов", () => {
  it("политика существует и содержит обязательные разделы", () => {
    const text = readFileSync(join(root, "TEST-POLICY.md"), "utf8");
    const requiredSections = [
      ["Абсолютные запреты", "Absolute Prohibitions"],
      ["RED", "RED"],
      ["Изменение тестов", "Test Modifications"],
      ["waiver", "waiver"],
      ["Definition of Done", "Definition of Done"],
      ["решением мимо тестов", "decisions bypassing tests"],
    ];
    for (const [ru, en] of requiredSections) {
      assert.ok(
        (ru && text.includes(ru)) || (en && text.includes(en)),
        `в TEST-POLICY.md нет раздела «${ru}» / «${en}»`,
      );
    }
  });

  it("в тестах нет .only, todo и пустых assert", () => {
    const bad = [];
    for (const file of walk(join(root, "tests")).filter((p) => p.endsWith(".mjs"))) {
      if (file.endsWith("test-policy.test.mjs")) continue;
      const text = readFileSync(file, "utf8");
      const rel = relative(root, file);
      if (/\b(?:it|test|describe)\.only\s*\(/.test(text)) bad.push(`${rel}: .only`);
      if (/\b(?:it|test)\.todo\s*\(/.test(text)) bad.push(`${rel}: todo`);
      if (/assert\.ok\(\s*true\s*\)/.test(text)) bad.push(`${rel}: assert.ok(true)`);
      if (/assert\.equal\(\s*true\s*,\s*true\s*\)/.test(text)) bad.push(`${rel}: assert.equal(true,true)`);
      if (/assert\.deepEqual\(\s*\[\s*\]\s*,\s*\[\s*\]\s*\)/.test(text)) bad.push(`${rel}: пустой deepEqual`);
    }
    assert.deepEqual(bad, [], "найдены ослабления тестов");
  });

  it("скрипты не обходят хуки и проверки", () => {
    const files = [
      ...walk(join(root, "scripts")),
      join(root, "package.json"),
    ].filter((p) => p.endsWith(".mjs") || p.endsWith(".json"));
    const bad = [];
    for (const file of files) {
      const text = readFileSync(file, "utf8");
      if (/--no-verify\b/.test(text)) bad.push(`${relative(root, file)}: --no-verify`);
      if (/--no-gpg-sign\b/.test(text)) bad.push(`${relative(root, file)}: --no-gpg-sign`);
    }
    assert.deepEqual(bad, [], "найдены обходы проверок");
  });

  it("гейт содержит обязательные фазы", () => {
    const gate = readFileSync(join(root, "scripts", "gate.mjs"), "utf8");
    for (const phase of ["typecheck", "meta", "unit", "smoke", "e2e", "coverage"]) {
      assert.ok(gate.includes(`"${phase}"`), `в gate.mjs нет фазы ${phase}`);
    }
  });

  it("гейт запускает Rust workspace целиком, а не только gui-spike", () => {
    const gate = readFileSync(join(root, "scripts", "gate.mjs"), "utf8");
    const phases = new Map(
      [...gate.matchAll(/\{[^{}]*name: "([a-z0-9-]+)"[^{}]*\}/gu)].map((match) => [match[1], match[0]]),
    );

    const workspace = phases.get("workspace-rust");
    assert.ok(workspace, "в gate.mjs нет фазы workspace-rust");
    assert.match(workspace, /command: "cargo"/u, "workspace-rust должна запускать cargo");
    assert.match(workspace, /"--offline"/u, "workspace-rust должна работать офлайн");
    assert.match(workspace, /"--workspace"/u, "workspace-rust должна прогонять весь workspace");
    assert.match(workspace, /timeoutMs: 900000/u, "у workspace-rust должен быть timeout 900000");
    assert.doesNotMatch(workspace, /gui-spike/u, "workspace-rust не должна подменяться gui-spike");
    assert.match(
      workspace,
      /(?:"--manifest-path", "rust\/Cargo\.toml"|cwd: "rust")/u,
      "workspace-rust должна идти от rust/Cargo.toml (manifest-path или cwd)",
    );

    const spike = phases.get("spike-rust");
    assert.ok(spike, "фаза spike-rust (gui-spike) должна остаться отдельной");
    assert.match(spike, /gui-spike\/Cargo\.toml/u, "spike-rust должна по-прежнему гонять gui-spike");

    const common = gate.match(/const commonPhases = \[[\s\S]*?\n\];/u)?.[0] ?? "";
    assert.match(common, /name: "workspace-rust"/u, "workspace-rust должна быть в commonPhases");
  });
});

// QA-M2-01: `skipped 4 / pass 0` давал `passed_with_skips` и exit 0 — M2 принимался без
// единого браузерного assert. Правило: skip ≠ pass, приёмка e2e — по счётчикам, а не по вердикту.
describe("QA-M2-01: e2e-skip не является прохождением", () => {
  /** @returns {any} */
  const phaseOutcome = () => loadGateFunction("phaseOutcome");
  /** @returns {any} */
  const computeVerdict = () => loadGateFunction("computeVerdict");
  /** @returns {any} */
  const e2eStatus = () => loadGateFunction("e2eStatus");
  /** @returns {any} */
  const computeAcceptance = () => loadGateFunction("computeAcceptance");

  /** Счётчики полностью пропущенного прогона (как e2e без браузера). */
  const allSkipped = { tests: 4, pass: 0, fail: 0, skipped: 4 };
  /** Счётчики реально выполненного fixture-прогона. */
  const allPassed = { tests: 2, pass: 2, fail: 0, skipped: 0 };

  it("failOnSkips превращает skip в провал фазы (пример, а не декларация)", () => {
    const outcome = phaseOutcome()({ failOnSkips: true, minPass: 1 }, allSkipped, true);
    assert.equal(outcome.ok, false, "skip при failOnSkips не имеет права быть ok");
    assert.equal(outcome.skipFailure, true, "skipFailure должен быть выставлен");
    assert.match(outcome.reasons.join(" "), /failOnSkips: skip 4/u, "причина skip-провала не записана");
  });

  it("без failOnSkips тот же skip не валит фазу (unit/smoke skip-aware)", () => {
    const outcome = phaseOutcome()({}, allSkipped, true);
    assert.equal(outcome.ok, true, "skip-aware unit/smoke не должны ломаться");
    assert.equal(outcome.skipFailure, false);
  });

  it("нулевой pass — e2e_unmet даже без skip", () => {
    const outcome = phaseOutcome()({ minPass: 1 }, { tests: 0, pass: 0, fail: 0, skipped: 0 }, true);
    assert.equal(outcome.ok, false, "0 выполненных тестов — не доказательство");
    assert.equal(outcome.unmet, true, "причина должна быть unmet, а не pass");
    assert.match(outcome.reasons.join(" "), /e2e_unmet: pass 0 < 1/u);
  });

  it("выполненный набор без skip и fail проходит", () => {
    assert.equal(phaseOutcome()({ failOnSkips: true, minPass: 1 }, allPassed, true).ok, true);
    assert.equal(phaseOutcome()({ failOnSkips: true, minPass: 1 }, allPassed, false).ok, false);
  });

  it("вердикт при упавшей e2e-фазе — failed, а не passed_with_skips", () => {
    const verdict = computeVerdict()([
      { name: "e2e", ok: false, unmet: false, counts: allSkipped },
      { name: "meta", ok: true, unmet: false, counts: { tests: 65, pass: 65, fail: 0, skipped: 0 } },
    ]);
    assert.equal(verdict, "failed", "skip в e2e обязан давать failed");
  });

  it("live-фаза без среды — passed_with_unmet, не passed и не skip", () => {
    const verdict = computeVerdict()([
      { name: "e2e", ok: true, unmet: false, counts: allPassed },
      { name: "e2e-live", ok: false, unmet: true, counts: null },
    ]);
    assert.equal(verdict, "passed_with_unmet", "unmet должен быть отдельным исходом");
  });

  it("Rust ignored не смешивается с e2e-skip", () => {
    const verdict = computeVerdict()([
      {
        name: "workspace-rust", ok: true, unmet: false, skipKind: "ignored",
        counts: { tests: 1104, pass: 1092, fail: 0, skipped: 12 },
      },
      { name: "e2e", ok: true, unmet: false, counts: allPassed },
    ]);
    assert.equal(verdict, "passed", "объяснённый ignored-набор не делает релиз skip-вердиктом");
  });

  it("статус e2e считается по counts: pass 0 / skip 4 ⇒ unmet", () => {
    const status = e2eStatus();
    assert.equal(status([{ name: "e2e", ok: true, unmet: false, counts: allSkipped }], "e2e"), "unmet");
    assert.equal(status([{ name: "e2e", ok: true, unmet: false, counts: { tests: 0, pass: 0, fail: 0, skipped: 0 } }], "e2e"), "unmet");
    assert.equal(status([{ name: "e2e", ok: true, unmet: false, counts: allPassed }], "e2e"), "met");
    assert.equal(status([{ name: "e2e", ok: false, unmet: true, counts: null }], "e2e"), "unmet");
    assert.equal(status([], "e2e"), "absent", "фазы нет — это отсутствие проверки, не pass");
  });

  it("приёмка запрещена без fixture-доказательства и без live в qualify", () => {
    const acceptance = computeAcceptance();
    const release = acceptance("release", "failed", "unmet", "absent");
    assert.equal(release.accepted, false, "failed-прогон не приёмка");
    assert.equal(acceptance("release", "passed", "unmet", "absent").accepted, false, "e2e.unmet не приёмка");
    assert.equal(acceptance("release", "passed", "met", "absent").accepted, true, "fixture-доказательство есть");
    const qualify = acceptance("qualify", "passed_with_unmet", "met", "unmet");
    assert.equal(qualify.accepted, false, "qualify без live не приёмка");
    assert.match(String(qualify.reason), /e2e\.live=unmet/u, "причина отказа не записана");
    assert.equal(acceptance("qualify", "passed", "met", "met").accepted, true);
    assert.equal(acceptance("worker", "passed", "absent", "absent"), null, "worker не приёмка релиза");
  });

  it("fixture-набор e2e исключает live и объявлен с failOnSkips", () => {
    const fixture = extractFunction(gateSource, "e2eFixturePhase");
    assert.match(fixture, /name: "e2e"/u, "фаза должна называться e2e");
    assert.match(fixture, /failOnSkips: true/u, "fixture-набор обязан падать на skip");
    assert.match(fixture, /minPass: 1/u, "нужен минимальный порог выполненных тестов");
    assert.match(fixture, /--test-concurrency=1/u, "e2e обязан идти последовательно (один браузер)");
    assert.match(gateSource, /!entry\.name\.startsWith\("live-"\)/u, "fixture-набор обязан исключать live-*");

    const live = gateSource.match(/const e2eLivePhase = \{[\s\S]*?\n\};/u)?.[0] ?? "";
    assert.ok(live, "в gate.mjs нет отдельной фазы e2e-live");
    assert.match(live, /name: "e2e-live"/u);
    assert.match(live, /requiresEnv: "DISCORD_LAB_LIVE_CDP"/u, "live-фаза обязана требовать env, а не skip");
    assert.match(live, /failOnSkips: true/u, "live-фаза обязана падать на skip");
  });

  it("live-фаза отдельная и только в qualify", () => {
    const release = gateSource.match(/if \(options\.scope !== "worker"\) \{[\s\S]*?\n {2}\}/u)?.[0] ?? "";
    assert.ok(release, "не найден блок фаз release/qualify");
    assert.match(release, /e2eFixturePhase\(\)/u, "release/qualify обязаны гонять fixture e2e");
    assert.doesNotMatch(release, /e2eLivePhase/u, "live не должен попадать в release (T2 — fixture)");
    const qualify = gateSource.match(/if \(options\.scope === "qualify"\) \{[\s\S]*?\n {2}\}/u)?.[0] ?? "";
    assert.match(qualify, /e2eLivePhase/u, "qualify (T3) обязан включать live-фазу");
  });

  it("оба результата повтора сохраняются в attempts[]", () => {
    const execute = extractFunction(gateSource, "executePhase");
    assert.match(execute, /attempts\.push\(\{\s*attempt,/u, "попытки должны накапливаться, а не теряться");
    assert.match(execute, /reasons/u, "у попытки должна быть причина");
    assert.match(execute, /tail: tail\(last\.output/u, "у попытки должен сохраняться её вывод");
    const record = gateSource.match(/phases: results\.map\([\s\S]*?\n {4}\}\)/u)?.[0] ?? "";
    assert.ok(record, "не найдено формирование phases в JSON");
    assert.match(record, /attempts: result\.attempts/u, "attempts не попадают в JSON гейта");
    assert.match(gateSource, /первый прогон: \$\{attempts\[0\]\?\.ok/u, "note о первом прогоне должен остаться");
  });
});
