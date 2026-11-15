// @ts-check
/**
 * TAURI-MD-02 (RW-49): интеграционные тесты взаимодействия `markdown.js` и `mentions.js`.
 *
 * Проверяет:
 * - отсутствие дублирующихся регулярных выражений упоминаний в `markdown.js` (делегирование в `mentions.js`),
 * - реэкспорт `extractMentions`, `readMention`, `readBroadcast`, `readAnyMention`, `createMentionElement`,
 *   `normalizeMentionContext` из `markdown.js`,
 * - разбор и рендеринг упоминаний (`<@id>`, `<@!id>`, `<#id>`, `<@&id>`, `@everyone`, `@here`)
 *   внутри форматированного Markdown (`**жирный**`, `*курсив*`, `~~зачёркнутый~~`, `||спойлер||`,
 *   `> цитата`, `[ссылка](https://...)`),
 * - игнорирование упоминаний внутри инлайн-кода и блоков кода как в `parseMessageMarkdown`,
 *   так и в `extractMentions`,
 * - безопасный DOM-рендеринг через `renderMessageMarkdown` с делегированием в `createMentionElement`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  BROADCAST_RE,
  DEFAULT_MENTION_FALLBACKS,
  MAX_MENTION_ID,
  MENTION_RE,
  createMentionElement,
  extractMentions,
  normalizeMentionContext,
  parseMessageMarkdown,
  readAnyMention,
  readBroadcast,
  readMention,
  renderMessageMarkdown,
  toMentionRecord,
} from "../../rust/gui-spike/ui/features/markdown.js";
import * as mentionsModule from "../../rust/gui-spike/ui/features/mentions.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const markdownSourcePath = path.resolve(
  __dirname,
  "../../rust/gui-spike/ui/features/markdown.js",
);

function makeDomContainer() {
  const doc = {
    /** @param {string} tagName */
    createElement(tagName) {
      /** @type {Record<string, string>} */
      const attrs = {};
      /** @type {any[]} */
      const children = [];
      const node = {
        tagName: tagName.toUpperCase(),
        className: "",
        textContent: "",
        attrs,
        children,
        ownerDocument: null,
        /** @param {string} k @param {string} v */
        setAttribute(k, v) {
          attrs[k] = v;
        },
        /** @param {string} k */
        getAttribute(k) {
          return attrs[k] ?? null;
        },
        /** @param {...any} nodes */
        append(...nodes) {
          for (const item of nodes) children.push(item);
        },
        addEventListener() {},
      };
      node.ownerDocument = /** @type {any} */ (doc);
      return node;
    },
    /** @param {string} value */
    createTextNode(value) {
      return { nodeType: 3, textContent: value };
    },
  };
  return /** @type {any} */ (doc.createElement("div"));
}

describe("Интеграция mentions.js в markdown.js (TAURI-MD-02)", () => {
  it("реэкспортирует функции и константы из mentions.js без дублирования регулярных выражений в markdown.js", () => {
    assert.equal(extractMentions, mentionsModule.extractMentions);
    assert.equal(readMention, mentionsModule.readMention);
    assert.equal(readBroadcast, mentionsModule.readBroadcast);
    assert.equal(readAnyMention, mentionsModule.readAnyMention);
    assert.equal(createMentionElement, mentionsModule.createMentionElement);
    assert.equal(normalizeMentionContext, mentionsModule.normalizeMentionContext);
    assert.equal(toMentionRecord, mentionsModule.toMentionRecord);
    assert.equal(MAX_MENTION_ID, mentionsModule.MAX_MENTION_ID);
    assert.equal(MENTION_RE, mentionsModule.MENTION_RE);
    assert.equal(BROADCAST_RE, mentionsModule.BROADCAST_RE);
    assert.equal(DEFAULT_MENTION_FALLBACKS, mentionsModule.DEFAULT_MENTION_FALLBACKS);

    const source = fs.readFileSync(markdownSourcePath, "utf8");
    assert.equal(
      source.includes('from "./mentions.js"'),
      true,
      "markdown.js обязан импортировать mentions.js",
    );
    assert.equal(
      source.includes("new RegExp("),
      false,
      "markdown.js не должен объявлять дублирующийся MENTION_RE через new RegExp",
    );
    assert.equal(
      source.includes("function toRecord("),
      false,
      "markdown.js должен использовать normalizeMentionContext из mentions.js вместо локального toRecord",
    );
  });

  it("разбирает упоминания внутри жирного, курсива, спойлера, цитаты и ссылок", () => {
    const context = {
      users: new Map([["101", "Алиса"]]),
      channels: { "202": "релизы" },
      roles: { "303": "QA" },
    };
    const input = [
      "**Привет <@101>!** Загляни в *<#202>* или пингани ||<@&303>||.",
      "> Внимание @everyone и @here в [канале <#202>](https://discord.com/channels/1/202)",
    ].join("\n");

    const tokens = parseMessageMarkdown(input, context);
    assert.equal(tokens.length, 7);

    const boldToken = /** @type {any} */ (tokens[0]);
    assert.equal(boldToken.type, "bold");
    assert.deepEqual(boldToken.children, [
      { type: "text", value: "Привет " },
      { type: "user_mention", id: "101", label: "@Алиса" },
      { type: "text", value: "!" },
    ]);

    const italicToken = /** @type {any} */ (tokens[2]);
    assert.equal(italicToken.type, "italic");
    assert.deepEqual(italicToken.children, [
      { type: "channel_mention", id: "202", label: "#релизы" },
    ]);

    const spoilerToken = /** @type {any} */ (tokens[4]);
    assert.equal(spoilerToken.type, "spoiler");
    assert.deepEqual(spoilerToken.children, [
      { type: "role_mention", id: "303", label: "@QA" },
    ]);

    const quoteToken = /** @type {any} */ (tokens[6]);
    assert.equal(quoteToken.type, "quote");
    assert.deepEqual(quoteToken.children, [
      { type: "text", value: "Внимание " },
      { type: "everyone", label: "@everyone" },
      { type: "text", value: " и " },
      { type: "here", label: "@here" },
      { type: "text", value: " в " },
      {
        type: "link",
        href: "https://discord.com/channels/1/202",
        children: [
          { type: "text", value: "канале " },
          { type: "channel_mention", id: "202", label: "#релизы" },
        ],
      },
    ]);

    // extractMentions на том же входе возвращает все 6 упоминаний по порядку:
    assert.deepEqual(extractMentions(input, context), [
      { type: "user_mention", id: "101", label: "@Алиса" },
      { type: "channel_mention", id: "202", label: "#релизы" },
      { type: "role_mention", id: "303", label: "@QA" },
      { type: "everyone", label: "@everyone" },
      { type: "here", label: "@here" },
      { type: "channel_mention", id: "202", label: "#релизы" },
    ]);
  });

  it("не разбирает упоминания внутри инлайн-кода и fenced-блоков кода", () => {
    const text = "До `<@101> @everyone` после\n```js\nconst m = '<#202> <@&303> @here';\n```\nКонец <@!101>";
    const context = { users: { "101": "Боб" } };

    const tokens = parseMessageMarkdown(text, context);
    assert.deepEqual(tokens, [
      { type: "text", value: "До " },
      { type: "code", value: "<@101> @everyone" },
      { type: "text", value: " после" },
      {
        type: "code_block",
        language: "js",
        value: "const m = '<#202> <@&303> @here';",
      },
      { type: "text", value: "Конец " },
      { type: "user_mention", id: "101", label: "@Боб" },
    ]);

    assert.deepEqual(extractMentions(text, context), [
      { type: "user_mention", id: "101", label: "@Боб" },
    ]);
  });

  it("рендерит DOM-узлы упоминаний через renderMessageMarkdown с использованием createMentionElement", () => {
    const container = makeDomContainer();
    const context = {
      users: { "7": "Георгий" },
      channels: { "8": "чат" },
      roles: { "9": "Тимлид" },
    };

    renderMessageMarkdown(
      container,
      "<@7> <#8> <@&9> <@999> @everyone @here",
      context,
    );

    const mentionNodes = container.children.filter(
      (/** @type {any} */ node) => node.tagName === "SPAN",
    );
    assert.equal(mentionNodes.length, 6);

    assert.equal(mentionNodes[0].className, "mention mention--user");
    assert.equal(mentionNodes[0].getAttribute("data-user-id"), "7");
    assert.equal(mentionNodes[0].textContent, "@Георгий");

    assert.equal(mentionNodes[1].className, "mention mention--channel");
    assert.equal(mentionNodes[1].getAttribute("data-channel-id"), "8");
    assert.equal(mentionNodes[1].textContent, "#чат");

    assert.equal(mentionNodes[2].className, "mention mention--role");
    assert.equal(mentionNodes[2].getAttribute("data-role-id"), "9");
    assert.equal(mentionNodes[2].textContent, "@Тимлид");

    assert.equal(mentionNodes[3].className, "mention mention--user");
    assert.equal(mentionNodes[3].getAttribute("data-user-id"), "999");
    assert.equal(mentionNodes[3].textContent, "@пользователь");

    assert.equal(mentionNodes[4].className, "mention mention--everyone");
    assert.equal(mentionNodes[4].getAttribute("data-mention"), "everyone");
    assert.equal(mentionNodes[4].textContent, "@everyone");

    assert.equal(mentionNodes[5].className, "mention mention--here");
    assert.equal(mentionNodes[5].getAttribute("data-mention"), "here");
    assert.equal(mentionNodes[5].textContent, "@here");
  });
});
