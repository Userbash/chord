// @ts-check
// Мета-тест PLAT-02: live-кейсы keyring не имеют права выглядеть «passed», если не выполнялись.
//
// Причина: в stable-libtest нет динамического skip. Ранний `return` — это успешное завершение
// теста (`passed += 1`), а `println!("SKIP: …")` не парсит ни libtest, ни gate.mjs. Единственный
// маркер «не выполнялся» — `#[ignore]`, который гейт видит как `ignored → skipped`.
//
// T1 — в keyring_contract.rs нет `skip()`/`return`-гейтов, у всех 12 кейсов есть `#[ignore]`;
// T2 — в scripts/gate.mjs есть live-фаза (release/qualify, `--ignored`, LIVE=1, failOnSkips);
// T3 — cargo реально печатает `0 passed; 0 failed; 12 ignored` и `--list --ignored` даёт 12 имён.
//
// Краснеет при возврате раннего `return`, снятии `--ignored`/`env`/`failOnSkips` с фазы,
// переносе live-фазы в worker-набор и при ослаблении порогов k02.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const CONTRACT = "rust/crates/chord-platform/tests/keyring_contract.rs";
const GATE = "scripts/gate.mjs";

/** Ожидаемый live-набор: имена k01…k12 не переименовываются и не удаляются. */
const LIVE_CASES = [
  "k01_connect_to_default_collection",
  "k02_store_then_lookup_roundtrip_without_leak",
  "k03_lookup_missing_returns_none",
  "k04_clear_existing_then_lookup_none",
  "k05_overwrite_replaces_value",
  "k06_unicode_secret_roundtrip",
  "k07_secret_tool_reads_our_store",
  "k08_our_lookup_reads_secret_tool_store",
  "k09_empty_key_is_invalid",
  "k10_nul_in_key_or_secret_is_error_without_panic",
  "k11_repeated_clear_returns_false",
  "k12_debug_does_not_print_secret_or_pointers",
];

/** @param {string} path @returns {string} */
const read = (path) => readFileSync(join(root, path), "utf8");
/** @param {string} text @returns {string[]} */
const lines = (text) => text.split("\n");

/**
 * Тело функции от строки сигнатуры до парной закрывающей скобки (счёт по `{`/`}`).
 * @param {string[]} all
 * @param {number} from индекс строки с `fn …(`
 * @returns {{ body: string, end: number }}
 */
function functionBody(all, from) {
  const collected = [];
  let depth = 0;
  let opened = false;
  for (let index = from; index < all.length; index += 1) {
    const line = all[index] ?? "";
    collected.push(line);
    for (const character of line) {
      if (character === "{") {
        depth += 1;
        opened = true;
      } else if (character === "}") {
        depth -= 1;
        if (opened && depth === 0) return { body: collected.join("\n"), end: index };
      }
    }
  }
  throw new Error(`непарные скобки в функции со строки ${from + 1}`);
}

/**
 * Все `#[test]`-функции тестового файла: имя, следующий атрибут и тело.
 * @param {string} text
 * @returns {{ name: string, attrLines: string[], body: string }[]}
 */
function testFunctions(text) {
  const all = lines(text);
  /** @type {{ name: string, attrLines: string[], body: string }[]} */
  const found = [];
  for (let index = 0; index < all.length; index += 1) {
    if (!/#\[test\]/u.test(all[index] ?? "")) continue;
    const attrLines = [];
    let cursor = index + 1;
    while (cursor < all.length && (all[cursor] ?? "").trim().startsWith("#[")) {
      attrLines.push((all[cursor] ?? "").trim());
      cursor += 1;
    }
    const signature = all[cursor] ?? "";
    const name = signature.match(/fn\s+([a-z0-9_]+)\s*\(/u)?.[1];
    assert.ok(name, `строка ${index + 1}: не разобрано имя функции после #[test]`);
    found.push({ name, attrLines, body: functionBody(all, cursor).body });
  }
  return found;
}

describe("PLAT-02 T1: live keyring-кейсы помечены ignore, а не возвращаются молча", () => {
  const contract = read(CONTRACT);

  it("в файле нет skip()-хелпера и его вызовов", () => {
    assert.doesNotMatch(contract, /fn\s+skip\s*\(/u, "в keyring_contract.rs остался fn skip()");
    assert.doesNotMatch(contract, /(?<![\w.])skip\s*\(/u, "в keyring_contract.rs остался вызов skip(");
  });

  it("ровно 12 #[test] и 12 #[ignore] с причиной live-keyring", () => {
    // Комментарии (`//`, `//!`) не считаются: маркер обязан быть настоящим атрибутом.
    const code = lines(contract)
      .filter((/** @type {string} */ line) => !line.trim().startsWith("//"))
      .join("\n");
    const tests = testFunctions(code);
    const ignores = code.match(/#\[ignore\s*=\s*"live-keyring: [^"]+"\]/gu) ?? [];
    assert.equal(tests.length, 12, `#[test] в ${CONTRACT}: ${tests.length}, ожидалось 12`);
    assert.equal(ignores.length, 12, `#[ignore = "live-keyring: …"]: ${ignores.length}, ожидалось 12`);
  });

  it("у каждого #[test] сразу следует #[ignore = \"live-keyring: …\"]", () => {
    for (const { name, attrLines } of testFunctions(contract)) {
      assert.ok(
        attrLines.some((line) => /^#\[ignore\s*=\s*"live-keyring: [^"]+"\]$/u.test(line)),
        `${name}: нет #[ignore = "live-keyring: …"] (skip-маркера нет, тест засчитается passed)`,
      );
    }
  });

  it("в телах live-кейсов нет раннего return под видом skip", () => {
    const offenders = testFunctions(contract)
      .filter(({ body }) => /\breturn\b/u.test(body))
      .map(({ name }) => name);
    assert.deepEqual(
      offenders,
      [],
      "ранний return делает кейс passed; live-кейс обязан падать с KEYRING-LIVE-причиной",
    );
  });

  it("имена и число кейсов не изменились", () => {
    assert.deepEqual(testFunctions(contract).map(({ name }) => name), LIVE_CASES);
  });

  it("пороги security-кейсов не ослаблены", () => {
    assert.match(contract, /const RSS_LIMIT_KILOBYTES: u64 = 4 \* 1024;/u, "RSS-порог k02 изменён");
    assert.match(contract, /const LEAK_ITERATIONS: usize = 2000;/u, "число итераций k02 изменено");
    assert.match(contract, /const LEAK_SECRET_BYTES: usize = 4096;/u, "размер секрета k02 изменён");
    const k02 = testFunctions(contract).find(({ name }) => name.startsWith("k02_"));
    assert.ok(k02, "кейс k02 отсутствует");
    assert.match(k02.body, /for _ in 0\.\.LEAK_ITERATIONS/u, "k02 не выполняет серию lookup");
    assert.match(k02.body, /after\.saturating_sub\(before\) < RSS_LIMIT_KILOBYTES/u, "в k02 убран assert RSS");
  });

  it("live_guard различает not-enabled, tool-missing и service-unavailable", () => {
    for (const prefix of [
      "KEYRING-LIVE: not-enabled:",
      "KEYRING-LIVE: service-unavailable:",
      "KEYRING-LIVE: tool-missing:",
    ]) {
      assert.ok(contract.includes(prefix), `в keyring_contract.rs нет причины «${prefix}»`);
    }
  });
});

describe("PLAT-02 T2: live-фаза гейта видима и не пропускается", () => {
  const gate = read(GATE);
  const phase =
    gate.match(/const keyringLivePhase = \{[\s\S]*?\n\};/u)?.[0] ?? "";

  it("фаза keyring-live объявлена и запускает ignored-набор cargo", () => {
    assert.ok(phase, "в scripts/gate.mjs нет const keyringLivePhase");
    assert.match(phase, /command: "cargo"/u, "фаза keyring-live должна запускать cargo");
    assert.match(phase, /"--offline"/u, "фаза keyring-live должна работать офлайн");
    assert.match(phase, /"-p",\s*"chord-platform"/u, "фаза должна брать пакет chord-platform");
    assert.match(phase, /"--test",\s*"keyring_contract"/u, "фаза должна гонять keyring_contract");
    assert.match(phase, /"--",\s*"--ignored"/u, "без `-- --ignored` live-кейсы не запустятся");
    assert.match(phase, /timeoutMs: 900000/u, "timeoutMs фазы должен совпадать с workspace-rust");
    assert.match(phase, /env:\s*keyringLiveEnv\(\)/u, "фаза должна получать env live-флага");
    assert.match(phase, /failOnSkips: true/u, "live-фаза обязана падать на skip");
  });

  it("live-флаг включается только фазой release/qualify", () => {
    assert.match(
      gate,
      /function keyringLiveEnv\(\)[\s\S]*?DISCORD_LAB_KEYRING_LIVE: "1"[\s\S]*?\n\}/u,
      "keyringLiveEnv должна выставлять DISCORD_LAB_KEYRING_LIVE=1",
    );
    const common = gate.match(/const commonPhases = \[[\s\S]*?\n\];/u)?.[0] ?? "";
    assert.ok(common, "в gate.mjs нет commonPhases");
    assert.doesNotMatch(common, /keyring-live/u, "worker-набор не должен запускать live keyring");
    const release = gate.match(/if \(options\.scope !== "worker"\) \{[\s\S]*?\n {2}\}/u)?.[0] ?? "";
    assert.ok(release, "не найден блок фаз release/qualify");
    assert.match(release, /keyringLivePhase/u, "release/qualify обязаны включать keyring-live");
  });

  it("failOnSkips превращает skip в failed, а не в warning", () => {
    const execute = gate.match(/function executePhase\(phase\) \{[\s\S]*?\n\}/u)?.[0] ?? "";
    assert.match(execute, /phase\.failOnSkips/u, "executePhase не учитывает failOnSkips");
    assert.match(execute, /skipped\s*>\s*0/u, "failOnSkips должен срабатывать при skipped > 0");
    assert.match(execute, /ok:[^\n]*skipFailure/u, "failOnSkips должен гасить ok фазы, а не только warning");
    assert.match(gate, /preflight/u, "у live-фазы должен быть preflight (env + secret-tool)");
  });

  it("cargoTestCounts по-прежнему считает ignored как skipped", () => {
    const source = gate.match(/function cargoTestCounts\(output\) \{[\s\S]*?\n\}/u)?.[0];
    assert.ok(source, "в gate.mjs нет cargoTestCounts");
    const counts = /** @type {(input: string) => { tests: number, pass: number, fail: number, skipped: number } | null} */ (
      new Function(`${source}; return cargoTestCounts;`)()
    );
    assert.deepEqual(counts("test result: ok. 0 passed; 0 failed; 12 ignored\n"), {
      tests: 12,
      pass: 0,
      fail: 0,
      skipped: 12,
    });
    assert.equal(counts("нет счётчиков"), null);
    assert.match(gate, /passed_with_skips/u, "вердикт passed_with_skips должен сохраниться");
    assert.match(
      gate,
      /пропущено \$\{skipped\} тестов — пропуск прохождением не считается/u,
      "warning о пропуске должен остаться в отчёте",
    );
    assert.match(
      gate,
      /skip \$\{result\.counts\.skipped/u,
      "в строке фазы должен печататься skip N",
    );
  });
});

describe("PLAT-02 T3: cargo подтверждает ignored-набор", () => {
  /** @param {string[]} args @returns {string} */
  const cargo = (args) =>
    execFileSync("cargo", args, {
      cwd: join(root, "rust"),
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      timeout: 900000,
      env: { ...process.env, DISCORD_LAB_KEYRING_LIVE: "0" },
    });
  /** @param {unknown} error @returns {string} */
  const spawnError = (error) => {
    const detail = /** @type {{ stdout?: string, stderr?: string }} */ (error);
    return `${detail?.stdout ?? ""}${detail?.stderr ?? ""}`;
  };

  it("в дефолтном прогоне 0 passed и 12 ignored", () => {
    let output;
    try {
      output = cargo(["test", "--offline", "-p", "chord-platform", "--test", "keyring_contract"]);
    } catch (error) {
      // cargo отсутствует или тесты упали — это провал, а не skip (гейт всё равно требует cargo).
      assert.fail(`cargo test keyring_contract не выполнился: ${spawnError(error).slice(-2000)}`);
    }
    assert.match(
      output,
      /test result: ok\. 0 passed; 0 failed; 12 ignored/u,
      `ожидалось «0 passed; 0 failed; 12 ignored», получено: ${output.slice(-2000)}`,
    );
  });

  it("--list --ignored перечисляет ровно 12 кейсов k01…k12", () => {
    let output;
    try {
      output = cargo([
        "test", "--offline", "-p", "chord-platform", "--test", "keyring_contract", "--",
        "--list", "--ignored",
      ]);
    } catch (error) {
      assert.fail(`cargo test --list --ignored не выполнился: ${spawnError(error).slice(-2000)}`);
    }
    const names = output
      .split("\n")
      .map((line) => line.match(/^([a-z0-9_]+): test$/u)?.[1])
      .filter((name) => name !== undefined);
    assert.deepEqual(names, LIVE_CASES, "`--list --ignored` должен давать ровно 12 live-кейсов");
  });
});
