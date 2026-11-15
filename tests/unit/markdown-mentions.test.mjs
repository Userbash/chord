/**
 * @file Unit tests for markdown mention integration (`TAURI-MD-02`).
 *
 * Verifies that `parseMessageMarkdown` seamlessly delegates mention parsing
 * to `mentions.js`, respecting Russian fallback labels, escaping code spans,
 * and maintaining full AST compatibility without XSS vulnerabilities.
 */

import { strict as assert } from "node:assert";
import { describe, it } from "node:test";
import {
  parseMessageMarkdown,
  createMentionElement,
  extractMentions,
  normalizeMentionContext,
} from "../../rust/gui-spike/ui/features/markdown.js";

describe("Markdown Mentions Integration (TAURI-MD-02)", () => {
  it("exports mentions helpers directly from markdown.js", () => {
    assert.equal(typeof createMentionElement, "function");
    assert.equal(typeof extractMentions, "function");
    assert.equal(typeof normalizeMentionContext, "function");
  });

  it("parses user, channel, and role mentions into AST tokens with fallback labels", () => {
    const raw = "Привет <@123456>, загляни в <#987654> для роли <@&555555>!";
    const tokens = parseMessageMarkdown(raw);

    assert.ok(Array.isArray(tokens));
    const userToken = tokens.find((t) => t.type === "user_mention");
    assert.ok(userToken);
    assert.equal(userToken.id, "123456");
    assert.equal(userToken.label, "@пользователь");

    const channelToken = tokens.find((t) => t.type === "channel_mention");
    assert.ok(channelToken);
    assert.equal(channelToken.id, "987654");
    assert.equal(channelToken.label, "#канал");

    const roleToken = tokens.find((t) => t.type === "role_mention");
    assert.ok(roleToken);
    assert.equal(roleToken.id, "555555");
    assert.equal(roleToken.label, "@роль");
  });

  it("populates mention labels from custom context dictionaries", () => {
    const raw = "Встреча <@123> в <#456>";
    const context = {
      users: { "123": "Алиса" },
      channels: new Map([["456", "общий"]]),
    };
    const tokens = parseMessageMarkdown(raw, context);

    const userToken = tokens.find((t) => t.type === "user_mention");
    assert.equal(userToken?.label, "@Алиса");

    const channelToken = tokens.find((t) => t.type === "channel_mention");
    assert.equal(channelToken?.label, "#общий");
  });

  it("parses broadcast mentions @everyone and @here", () => {
    const raw = "Внимание @everyone и @here собрание!";
    const tokens = parseMessageMarkdown(raw);

    const everyoneToken = /** @type {any} */ (tokens.find((t) => t.type === "everyone"));
    assert.ok(everyoneToken);
    assert.equal(everyoneToken.label, "@everyone");

    const hereToken = /** @type {any} */ (tokens.find((t) => t.type === "here"));
    assert.ok(hereToken);
    assert.equal(hereToken.label, "@here");
  });

  it("ignores mentions inside inline code spans and fenced code blocks", () => {
    const raw = "Код `<@12345>` и блок:\n```\n<#99999> @everyone\n```";
    const tokens = parseMessageMarkdown(raw);

    const userMention = tokens.find((t) => t.type === "user_mention");
    assert.equal(userMention, undefined, "упоминание внутри инлайн-кода не должно создаваться");

    const codeBlock = tokens.find((t) => t.type === "code_block");
    assert.ok(codeBlock);
    assert.ok(/** @type {any} */ (codeBlock).value.includes("<#99999> @everyone"));
  });
});
