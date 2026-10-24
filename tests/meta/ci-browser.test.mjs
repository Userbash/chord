// @ts-check
// Мета-тест QA-M2-04: CI обязан запускать fixture browser-e2e на pinned Chromium,
// с обязательным preflight (нет браузера → fail, а не skip), последовательным прогоном
// и artifacts `.gate`. Live/CDP в browser-job запрещён — он живёт отдельной disabled-джобой.
//
// Парсер намеренно без YAML-зависимости: workflow маленький, а `npm ci` в CI
// не должен тянуть ещё один пакет только ради мета-проверки.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const workflowUrl = new URL("../../.github/workflows/ci.yml", import.meta.url);
const packageUrl = new URL("../../package.json", import.meta.url);

const workflowPromise = readFile(workflowUrl, "utf8");
const packagePromise = readFile(packageUrl, "utf8").then((text) => JSON.parse(text));

/**
 * Разрезает `jobs:` на блоки по именам джоб (ключ с отступом 2).
 * @param {string} text
 * @returns {Map<string, string>}
 */
function parseJobs(text) {
  const lines = text.split("\n");
  const start = lines.findIndex((line) => /^jobs:\s*$/.test(line));
  assert.ok(start >= 0, "ci.yml: не найден блок jobs:");
  /** @type {Map<string, string>} */
  const jobs = new Map();
  /** @type {string | null} */
  let current = null;
  /** @type {string[]} */
  let buffer = [];
  const flush = () => {
    if (current !== null) jobs.set(current, buffer.join("\n"));
  };
  for (let i = start + 1; i < lines.length; i += 1) {
    const line = lines[i] ?? "";
    if (/^\S/.test(line)) break; // следующий top-level ключ
    const header = /^ {2}([A-Za-z0-9_-]+):\s*$/.exec(line);
    if (header) {
      flush();
      current = header[1] ?? null;
      buffer = [];
      continue;
    }
    if (current !== null) buffer.push(line);
  }
  flush();
  return jobs;
}

/** @param {string} block @returns {string[]} */
const linesOf = (block) => block.split("\n");

/**
 * Строки блока, которые не являются пустыми или комментариями.
 * @param {string} block
 * @returns {string[]}
 */
function codeLines(block) {
  return linesOf(block).filter((line) => line.trim() !== "" && !line.trim().startsWith("#"));
}

describe("QA-M2-04: CI browser-job (pinned Chromium, без live)", () => {
  it("qa-browser job существует, ubuntu, ограничен по времени", async () => {
    const jobs = parseJobs(await workflowPromise);
    assert.ok(jobs.has("qa-browser"), "ci.yml: нет job `qa-browser`");
    const block = jobs.get("qa-browser") ?? "";
    assert.match(block, /^ {4}name:\s*\S+/m, "qa-browser: нет человекочитаемого name");
    assert.match(
      block,
      /^ {4}runs-on:\s*ubuntu-/m,
      "qa-browser: ожидается runs-on: ubuntu-* (headless Chromium)",
    );
    assert.match(
      block,
      /^ {4}timeout-minutes:\s*\d+/m,
      "qa-browser: нет timeout-minutes — зависший браузер не должен держать runner",
    );
    const timeout = Number(/^ {4}timeout-minutes:\s*(\d+)/m.exec(block)?.[1] ?? "0");
    assert.ok(timeout > 0 && timeout <= 30, `qa-browser: timeout-minutes=${timeout} вне 1..30`);
  });

  it("Chromium закреплён по точной версии и проброшен в CHROME_BIN", async () => {
    const jobs = parseJobs(await workflowPromise);
    const block = jobs.get("qa-browser") ?? "";
    const pinned = /^ {10}chrome-version:\s*"?(\d+\.\d+\.\d+\.\d+)"?\s*$/m.exec(block);
    assert.ok(
      pinned,
      "qa-browser: chrome-version должен быть точным (например 131.0.6778.85), не stable/latest",
    );
    assert.doesNotMatch(
      block,
      /^ {10}chrome-version:\s*"?(?:stable|latest|beta|dev)"?\s*$/m,
      "qa-browser: плавающий chrome-version запрещён (нужен pinned Chromium)",
    );
    assert.match(
      block,
      /steps\.[A-Za-z0-9_-]+\.outputs\.chrome-path/,
      "qa-browser: путь браузера должен браться из outputs.chrome-path setup-шага",
    );
    assert.match(
      block,
      /CHROME_BIN=\$\{\{\s*steps\.[A-Za-z0-9_-]+\.outputs\.chrome-path\s*\}\}/,
      "qa-browser: CHROME_BIN не заполняется из chrome-path",
    );
    assert.match(
      block,
      />>\s*"?\$GITHUB_ENV"?|>>\s*"?\$\{GITHUB_ENV\}"?/,
      "qa-browser: CHROME_BIN не экспортируется в последующие шаги через $GITHUB_ENV",
    );
  });

  it("preflight: отсутствие браузера — fail, а не skip", async () => {
    const jobs = parseJobs(await workflowPromise);
    const block = jobs.get("qa-browser") ?? "";
    assert.match(block, /test -x "\$CHROME_BIN"/, "qa-browser: нет preflight `test -x \"$CHROME_BIN\"`");
    assert.match(block, /exit 1/, "qa-browser: preflight не завершает job с кодом 1");
    assert.match(
      block,
      /::error::/,
      "qa-browser: preflight не помечает ошибку аннотацией ::error::",
    );
    assert.match(block, /--version/, "qa-browser: preflight не печатает версию браузера");
  });

  it("fixture-набор без live: live-* исключён, прогон последовательный", async () => {
    const jobs = parseJobs(await workflowPromise);
    const block = jobs.get("qa-browser") ?? "";
    assert.match(
      block,
      /--test-concurrency=1/,
      "qa-browser: нет `--test-concurrency=1` — браузерные тесты нельзя параллелить",
    );
    assert.match(block, /tests\/e2e/, "qa-browser: job не запускает tests/e2e");
    assert.match(
      block,
      /!?-name\s+'?live-\*'?|!-name\s+"?live-\*"?/,
      "qa-browser: live-* не исключён из fixture-набора",
    );
  });

  it("live/CDP не запускаются в qa-browser", async () => {
    const jobs = parseJobs(await workflowPromise);
    const block = jobs.get("qa-browser") ?? "";
    assert.doesNotMatch(block, /secrets\./, "qa-browser: не должен использовать secrets (live)");
    assert.doesNotMatch(block, /CDP_ENDPOINT|DISCORD_LAB_LIVE_CDP\s*[:=]\s*[^\s]/, "qa-browser: live-переменные запрещены");
    assert.doesNotMatch(block, /discord\.com|remote-debugging-port=9223/, "qa-browser: live Discord запрещён");
    // Разрешено только явное снятие переменной из окружения.
    const liveMentions = codeLines(block).filter((line) => line.includes("DISCORD_LAB_LIVE_CDP"));
    for (const line of liveMentions) {
      assert.match(
        line,
        /env -u DISCORD_LAB_LIVE_CDP|DISCORD_LAB_LIVE_CDP= /,
        `qa-browser: live-переменная встречается не как снятие окружения: ${line.trim()}`,
      );
    }
  });

  it("artifacts `.gate` выгружаются всегда, даже при падении", async () => {
    const jobs = parseJobs(await workflowPromise);
    const block = jobs.get("qa-browser") ?? "";
    assert.match(block, /uses:\s*actions\/upload-artifact@v\d+/, "qa-browser: нет upload-artifact");
    const dotGate = /^ {10}path:\s*"?\.gate\/?"?\s*$/m.test(block);
    assert.ok(dotGate, "qa-browser: upload-artifact должен забирать path: .gate");
    assert.match(
      block,
      /^ {8}if:\s*always\(\)/m,
      "qa-browser: artifacts должны грузиться с if: always() (лог нужен при падении)",
    );
    assert.match(block, /if-no-files-found:\s*ignore/, "qa-browser: пустой артефакт не должен валить job");
  });

  it("live job вынесен отдельно и выключен по умолчанию", async () => {
    const jobs = parseJobs(await workflowPromise);
    assert.ok(jobs.has("qa-live"), "ci.yml: нет отдельного live-job `qa-live`");
    const block = jobs.get("qa-live") ?? "";
    assert.match(
      block,
      /^ {4}if:\s*\$\{\{\s*false\s*\}\}\s*$/m,
      "qa-live: должен быть disabled (if: ${{ false }}) — live только по окну T3",
    );
    assert.match(block, /DISCORD_LAB_LIVE_CDP/, "qa-live: endpoint берётся из DISCORD_LAB_LIVE_CDP");
    assert.match(block, /qualify/, "qa-live: live-фаза должна идти через gate:qualify, не fixture-джобу");
  });

  it("Node-версия job согласована с package.json engines", async () => {
    const pkg = await packagePromise;
    const engines = /(\d+)/.exec(String(pkg.engines?.node ?? ""));
    assert.ok(engines, "package.json: engines.node не задан");
    assert.ok(Number(engines[1]) >= 24, `package.json: engines.node=${pkg.engines?.node}, ожидается ≥24`);
    const jobs = parseJobs(await workflowPromise);
    const block = jobs.get("qa-browser") ?? "";
    const jobNode = /^ {10}node-version:\s*"?(\d+)/m.exec(block);
    assert.ok(jobNode, "qa-browser: нет actions/setup-node с node-version");
    assert.equal(
      Number(jobNode[1]),
      Number(engines[1]),
      `qa-browser: node-version=${jobNode[1]} не совпадает с engines.node=${engines[1]}`,
    );
    assert.match(block, /npm ci/, "qa-browser: зависимости не ставятся через npm ci");
  });

  it("Rust-джобы не изменены по контракту", async () => {
    const jobs = parseJobs(await workflowPromise);
    const test = jobs.get("test") ?? "";
    assert.match(test, /scripts\/ci\.sh --online --skip-release/, "test: потерян локальный CI-контракт");
    const pkg = jobs.get("package") ?? "";
    assert.match(pkg, /^ {4}needs:\s*test\s*$/m, "package: должен зависеть от test");
    assert.match(pkg, /scripts\/release\.sh --online/, "package: потеряна release-команда");
  });

  it("workflow остаётся read-only и не получает прав на запись", async () => {
    const text = await workflowPromise;
    assert.match(text, /^permissions:\s*$/m, "ci.yml: нет блока permissions");
    assert.match(text, /^ {2}contents:\s*read\s*$/m, "ci.yml: permissions.contents должен быть read");
    assert.doesNotMatch(text, /^ {2}\w+:\s*write\s*$/m, "ci.yml: write-права не нужны");
  });
});
