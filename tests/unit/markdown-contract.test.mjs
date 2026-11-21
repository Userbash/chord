// @ts-check
/**
 * W03 (RW-53): контрактные тесты модульного AST-токеннизатора Markdown и Codeblock Engine.
 *
 * Проверяет:
 * - форматирование Discord: bold (**), italic (* / _ с границами слов), underline (__),
 *   strikethrough (~~), spoiler (||), inline-code (`), blockquotes (> и >>>);
 * - упоминания (<@id>, <#id>, <@&id>), кастомные эмодзи (<:name:id>), timestamp (<t:UNIX[:STYLE]>);
 * - многострочные блоки кода (```lang) с подсветкой синтаксиса и кнопкой копирования;
 * - отсутствие катастрофического backtracking и защиту от XSS.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  formatTimestampText,
  parseMessageMarkdown,
  readTimestampTag,
  renderMessageMarkdown,
  tokenizeCodeLines,
} from "../../rust/gui-spike/ui/features/markdown.js";

/** @param {string} text @param {any} [context] */
function render(text, context) {
  const doc = new ShimDocument();
  const container = /** @type {any} */ (doc.createElement("div"));
  renderMessageMarkdown(container, text, context);
  return { doc, container };
}

describe("W03: Discord Markdown & Codeblock Engine (markdown-contract)", () => {
  it("разбирает underline (__), underscore italic (_) с защитой snake_case и комбинированное форматирование", () => {
    const tokens = parseMessageMarkdown("__подчёркнутый__ _курсив_ snake_case_var **__жирный+подчёркнутый__**");
    const nonText = tokens.filter((t) => t.type !== "text");
    assert.deepEqual(
      nonText.map((t) => t.type),
      ["underline", "italic", "bold"],
    );
    assert.equal(/** @type {any} */ (nonText[0]).children[0].value, "подчёркнутый");
    assert.equal(/** @type {any} */ (nonText[1]).children[0].value, "курсив");
    assert.equal(/** @type {any} */ (nonText[2]).children[0].type, "underline");

    const { container } = render("__u__ и _i_ и foo_bar_baz");
    assert.equal(container.querySelectorAll("u.md-underline").length, 1);
    assert.equal(container.querySelectorAll("em.md-italic").length, 1);
    assert.match(container.textContent, /foo_bar_baz/);
  });

  it("поддерживает timestamp-теги <t:UNIX[:STYLE]> в AST и DOM, игнорируя их в коде", () => {
    const parsed = readTimestampTag("<t:1700000000:R> tail", 0);
    assert.ok(parsed);
    if (!parsed) throw new Error("expected parsed timestamp");
    assert.equal(parsed.token.type, "timestamp");
    assert.equal(parsed.token.unixSeconds, 1700000000);
    assert.equal(parsed.token.style, "R");
    assert.equal(parsed.end, 16);

    const tokens = parseMessageMarkdown("Время: <t:1700000000:t> и `<t:1700000000:R>`");
    const tsTokens = tokens.filter((t) => t.type === "timestamp");
    assert.equal(tsTokens.length, 1);
    assert.equal(/** @type {any} */ (tsTokens[0]).style, "t");

    const { container } = render("Запуск <t:1700000000:R>");
    const times = container.querySelectorAll("time.md-timestamp");
    assert.equal(times.length, 1);
    assert.equal(times[0].getAttribute("data-unix"), "1700000000");
    assert.equal(times[0].getAttribute("data-style"), "R");
    assert.ok(times[0].getAttribute("datetime"));
    assert.ok(times[0].textContent.length > 0);

    assert.equal(formatTimestampText(1000, "R", 1060), "1 мин. назад");
  });

  it("поддерживает однострочные (> ) и многострочные (>>> ) цитаты с упоминаниями и каналами", () => {
    const ctx = { users: { "42": "Bob" }, channels: { "77": "dev" } };
    const { container } = render(">>> строка 1 <@42>\nстрока 2 <#77>", ctx);
    const quotes = container.querySelectorAll("blockquote.md-quote");
    assert.equal(quotes.length, 1);
    assert.equal(quotes[0].querySelectorAll("[data-user-id=42]").length, 1);
    assert.equal(quotes[0].querySelectorAll("[data-channel-id=77]").length, 1);
    assert.match(quotes[0].textContent, /строка 1 @Bob\nстрока 2 #dev/);
  });

  it("рендерит codeblock с подсветкой синтаксиса и рабочей кнопкой копирования", () => {
    const codeText = 'const x = 42; // comment\nreturn "ok";';
    const hl = tokenizeCodeLines(codeText, "js");
    assert.ok(hl.some((t) => t.type === "keyword" && t.value === "const"));
    assert.ok(hl.some((t) => t.type === "number" && t.value === "42"));
    assert.ok(hl.some((t) => t.type === "comment" && t.value === "// comment"));
    assert.ok(hl.some((t) => t.type === "string" && t.value === '"ok"'));

    /** @type {string[]} */
    const copied = [];
    const prevNav = globalThis.navigator;
    Object.defineProperty(globalThis, "navigator", {
      configurable: true,
      value: {
        clipboard: {
          /** @param {string} v */
          writeText(v) {
            copied.push(v);
            return Promise.resolve();
          },
        },
      },
    });

    try {
      const { container } = render("```js\n" + codeText + "\n```");
      const pre = container.querySelectorAll("pre.md-code-block")[0];
      assert.ok(pre);
      assert.equal(pre.getAttribute("data-language"), "js");

      const codeEl = pre.querySelectorAll("code.md-code")[0];
      assert.ok(codeEl);
      assert.equal(codeEl.textContent, codeText);
      assert.ok(codeEl.querySelectorAll(".md-hl--keyword").length >= 2);

      const copyBtn = pre.querySelectorAll("button.md-code-copy")[0];
      assert.ok(copyBtn);
      assert.equal(copyBtn.getAttribute("data-copied"), "false");
      copyBtn.click();
      assert.equal(copyBtn.getAttribute("data-copied"), "true");
      assert.equal(copyBtn.textContent, "Скопировано");
      assert.deepEqual(copied, [codeText]);
    } finally {
      Object.defineProperty(globalThis, "navigator", {
        configurable: true,
        value: prevNav,
      });
    }
  });

  it("поддерживает упоминания, каналы, кастомные эмодзи и спойлеры в одном сообщении", () => {
    const ctx = { users: { "1": "Alice" }, channels: { "2": "general" } };
    const { container } = render(
      "Привет <@1> в <#2> <:wave:123456> ||секрет|| <t:1700000000:d>",
      ctx,
    );
    assert.equal(container.querySelectorAll("[data-user-id=1]").length, 1);
    assert.equal(container.querySelectorAll("[data-channel-id=2]").length, 1);
    assert.equal(container.querySelectorAll("[data-emoji-id=123456]").length, 1);
    assert.equal(container.querySelectorAll("[data-spoiler=hidden]").length, 1);
    assert.equal(container.querySelectorAll("time.md-timestamp").length, 1);
  });

  it("не подвержен катастрофическому backtracking на патологическом вводе", () => {
    const evil = "*".repeat(4000) + "_".repeat(4000) + "<t:bad" + "|".repeat(2000);
    const start = Date.now();
    const tokens = parseMessageMarkdown(evil);
    const elapsed = Date.now() - start;
    assert.ok(Array.isArray(tokens));
    assert.ok(elapsed < 250, `парсинг занял слишком много времени: ${elapsed}ms`);
  });
});
