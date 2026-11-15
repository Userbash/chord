// @ts-check
/**
 * TAURI-MD-01 (RW-49): модульные тесты парсера и DOM-рендерера упоминаний Discord
 * (`rust/gui-spike/ui/features/mentions.js`).
 *
 * Проверяет:
 * - парсинг `<@id>`, `<@!id>`, `<#id>`, `<@&id>`, `@everyone`, `@here`,
 * - приоритет роли `<@&id>` над пользователем `<@id>`,
 * - подстановку имён из `Record` и `Map`, а также русские фоллбэки (`@пользователь`, `#канал`, `@роль`),
 * - граничные случаи snowflake ID (`1..=32` цифр, отказ на 0 цифр, >32 цифр, нецифровых символах),
 * - Unicode-границу слова перед `@everyone` / `@here` (`почта@everyone.com`, `@everyone123`),
 * - пропуск упоминаний внутри инлайн-кода и блоков кода в `extractMentions`,
 * - безопасное создание DOM-элементов через `createMentionElement` (`textContent`, защита от XSS),
 * - эквивалентность токенов между `mentions.js` и `markdown.js`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  BROADCAST_RE,
  DEFAULT_MENTION_FALLBACKS,
  MAX_MENTION_ID,
  MENTION_RE,
  createMentionElement,
  extractMentions,
  formatMentionLabel,
  normalizeMentionContext,
  readAnyMention,
  readBroadcast,
  readMention,
  toMentionRecord,
} from "../../rust/gui-spike/ui/features/mentions.js";
import { parseMessageMarkdown } from "../../rust/gui-spike/ui/features/markdown.js";

describe("Константы mentions.js (TAURI-MD-01)", () => {
  it("экспортирует MAX_MENTION_ID = 32 и замороженные русские фоллбэки", () => {
    assert.equal(MAX_MENTION_ID, 32);
    assert.equal(Object.isFrozen(DEFAULT_MENTION_FALLBACKS), true);
    assert.deepEqual(DEFAULT_MENTION_FALLBACKS, {
      USER: "@пользователь",
      CHANNEL: "#канал",
      ROLE: "@роль",
    });
    assert.equal(MENTION_RE instanceof RegExp, true);
    assert.equal(BROADCAST_RE instanceof RegExp, true);
  });
});

describe("toMentionRecord и normalizeMentionContext", () => {
  it("преобразует Map и Record в плоский словарь строк, отбрасывая нестроковые значения", () => {
    const map = new Map([
      ["100", "Алиса"],
      ["200", /** @type {any} */ (42)],
    ]);
    assert.deepEqual(toMentionRecord(map), { "100": "Алиса" });

    const obj = { "300": "общий", "400": /** @type {any} */ (null) };
    assert.deepEqual(toMentionRecord(obj), { "300": "общий" });

    assert.deepEqual(toMentionRecord(null), {});
    assert.deepEqual(toMentionRecord(undefined), {});
  });

  it("нормализует пустой или частичный контекст упоминаний", () => {
    assert.deepEqual(normalizeMentionContext(), {
      users: {},
      channels: {},
      roles: {},
    });

    const ctx = normalizeMentionContext({
      users: new Map([["1", "Боб"]]),
      channels: { "2": "новости" },
    });
    assert.deepEqual(ctx, {
      users: { "1": "Боб" },
      channels: { "2": "новости" },
      roles: {},
    });
  });

  it("formatMentionLabel возвращает префикс + имя или фоллбэк на пустом/неизвестном id", () => {
    const users = { "1": "Алиса", "2": "" };
    assert.equal(formatMentionLabel(users, "1", "@", "@пользователь"), "@Алиса");
    assert.equal(formatMentionLabel(users, "2", "@", "@пользователь"), "@пользователь");
    assert.equal(formatMentionLabel(users, "999", "@", "@пользователь"), "@пользователь");
    assert.equal(formatMentionLabel(null, "1", "@", "@пользователь"), "@пользователь");
  });
});

describe("readMention — угловые упоминания <@id>, <@!id>, <#id>, <@&id>", () => {
  const context = {
    users: { "123456789012345678": "Алиса" },
    channels: { "456789012345678901": "общий-чат" },
    roles: { "789012345678901234": "Модераторы" },
  };

  it("разбирает упоминание пользователя <@id> и <@!id> с известным и неизвестным id", () => {
    const known = readMention("<@123456789012345678> привет", 0, context);
    assert.deepEqual(known, {
      token: {
        type: "user_mention",
        id: "123456789012345678",
        label: "@Алиса",
      },
      end: 21,
    });

    const nickStyle = readMention("Префикс <@!123456789012345678>!", 8, context);
    assert.deepEqual(nickStyle, {
      token: {
        type: "user_mention",
        id: "123456789012345678",
        label: "@Алиса",
      },
      end: 30,
    });

    const unknown = readMention("<@999>", 0, context);
    assert.deepEqual(unknown, {
      token: {
        type: "user_mention",
        id: "999",
        label: "@пользователь",
      },
      end: 6,
    });
  });

  it("разбирает упоминание канала <#id> с известным и неизвестным id", () => {
    const known = readMention("Загляни в <#456789012345678901>", 10, context);
    assert.deepEqual(known, {
      token: {
        type: "channel_mention",
        id: "456789012345678901",
        label: "#общий-чат",
      },
      end: 31,
    });

    const unknown = readMention("<#111>", 0, context);
    assert.deepEqual(unknown, {
      token: {
        type: "channel_mention",
        id: "111",
        label: "#канал",
      },
      end: 6,
    });
  });

  it("разбирает упоминание роли <@&id> раньше пользователя и поддерживает Map в контексте", () => {
    const mapContext = {
      roles: new Map([["789012345678901234", "Админы"]]),
    };
    const known = readMention("<@&789012345678901234>", 0, mapContext);
    assert.deepEqual(known, {
      token: {
        type: "role_mention",
        id: "789012345678901234",
        label: "@Админы",
      },
      end: 22,
    });

    const unknown = readMention("<@&555>", 0, {});
    assert.deepEqual(unknown, {
      token: {
        type: "role_mention",
        id: "555",
        label: "@роль",
      },
      end: 7,
    });
  });

  it("соблюдает границы длины ID (1..=32 цифры) и отклоняет некорректный синтаксис", () => {
    const id1 = "1";
    assert.equal(readMention(`<@${id1}>`, 0)?.token.id, "1");

    const id32 = "9".repeat(32);
    assert.equal(readMention(`<@${id32}>`, 0)?.token.id, id32);

    const id33 = "9".repeat(33);
    assert.equal(readMention(`<@${id33}>`, 0), null);

    for (const invalid of ["<@>", "<@!>", "<#>", "<@&>", "<@abc>", "<@12a>", "<@123", "текст"]) {
      assert.equal(readMention(invalid, 0), null, `ожидался null для ${invalid}`);
    }
    assert.equal(readMention("<@123>", -1), null);
    assert.equal(readMention("<@123>", 99), null);
  });
});

describe("readBroadcast — широковещательные упоминания @everyone и @here", () => {
  it("распознаёт @everyone и @here в начале строки, после пробела и знаков препинания", () => {
    assert.deepEqual(readBroadcast("@everyone внимание!", 0), {
      token: { type: "everyone", label: "@everyone" },
      end: 9,
    });

    assert.deepEqual(readBroadcast("Привет, @here!", 8), {
      token: { type: "here", label: "@here" },
      end: 13,
    });

    assert.deepEqual(readBroadcast("(@everyone)", 1), {
      token: { type: "everyone", label: "@everyone" },
      end: 10,
    });
  });

  it("отклоняет @everyone и @here внутри email, после Unicode-букв/цифр и при продолжении слова", () => {
    assert.equal(readBroadcast("почта@everyone.com", 5), null);
    assert.equal(readBroadcast("user@here.org", 4), null);
    assert.equal(readBroadcast("123@everyone", 3), null);
    assert.equal(readBroadcast("_@here", 1), null);
    assert.equal(readBroadcast("@@everyone", 1), null);
    assert.equal(readBroadcast("#@here", 1), null);
    assert.equal(readBroadcast("@everyone123", 0), null);
    assert.equal(readBroadcast("@here_now", 0), null);
    assert.equal(readBroadcast("@someone", 0), null);
  });
});

describe("readAnyMention и extractMentions", () => {
  it("извлекает все виды упоминаний в порядке следования и игнорирует код", () => {
    const context = {
      users: { "10": "Алиса" },
      channels: { "20": "general" },
      roles: { "30": "Dev" },
    };
    const text =
      "Привет <@10> и <@!99>, зайдите в <#20> для <@&30>! `@everyone в инлайн-коде <@10>`\n```\n<@10> @here\n```\nВнимание @everyone и @here!";

    const mentions = extractMentions(text, context);
    assert.deepEqual(mentions, [
      { type: "user_mention", id: "10", label: "@Алиса" },
      { type: "user_mention", id: "99", label: "@пользователь" },
      { type: "channel_mention", id: "20", label: "#general" },
      { type: "role_mention", id: "30", label: "@Dev" },
      { type: "everyone", label: "@everyone" },
      { type: "here", label: "@here" },
    ]);
  });

  it("readAnyMention делегирует на readMention и readBroadcast", () => {
    assert.deepEqual(readAnyMention("<#42>", 0), {
      token: { type: "channel_mention", id: "42", label: "#канал" },
      end: 5,
    });
    assert.deepEqual(readAnyMention("@here", 0), {
      token: { type: "here", label: "@here" },
      end: 5,
    });
    assert.equal(readAnyMention("обычный текст", 0), null);
  });

  it("выбрасывает TypeError при нестроковом аргументе в extractMentions", () => {
    assert.throws(() => extractMentions(/** @type {any} */ (null)), TypeError);
  });
});

describe("createMentionElement и совместимость с markdown.js", () => {
  function makeMockDocument() {
    return {
      /** @param {string} tagName */
      createElement(tagName) {
        /** @type {Record<string, string>} */
        const attrs = {};
        return {
          tagName: tagName.toUpperCase(),
          className: "",
          textContent: "",
          attrs,
          /** @param {string} k @param {string} v */
          setAttribute(k, v) {
            attrs[k] = v;
          },
          /** @param {string} k */
          getAttribute(k) {
            return attrs[k] ?? null;
          },
        };
      },
    };
  }

  it("создаёт DOM-узлы с безопасным textContent и точными data-атрибутами (защита от XSS)", () => {
    const doc = /** @type {any} */ (makeMockDocument());

    const xssUser = createMentionElement(doc, {
      type: "user_mention",
      id: "123",
      label: "@<img src=x onerror=alert(1)>",
    });
    assert.equal(xssUser.tagName, "SPAN");
    assert.equal(xssUser.className, "mention mention--user");
    assert.equal(xssUser.getAttribute("data-user-id"), "123");
    assert.equal(xssUser.textContent, "@<img src=x onerror=alert(1)>");

    const chanNode = createMentionElement(doc, {
      type: "channel_mention",
      id: "456",
      label: "#канал",
    });
    assert.equal(chanNode.className, "mention mention--channel");
    assert.equal(chanNode.getAttribute("data-channel-id"), "456");
    assert.equal(chanNode.textContent, "#канал");

    const roleNode = createMentionElement(doc, {
      type: "role_mention",
      id: "789",
      label: "@роль",
    });
    assert.equal(roleNode.className, "mention mention--role");
    assert.equal(roleNode.getAttribute("data-role-id"), "789");
    assert.equal(roleNode.textContent, "@роль");

    const everyoneNode = createMentionElement(doc, {
      type: "everyone",
      label: "@everyone",
    });
    assert.equal(everyoneNode.className, "mention mention--everyone");
    assert.equal(everyoneNode.getAttribute("data-mention"), "everyone");
    assert.equal(everyoneNode.textContent, "@everyone");
  });

  it("совпадает по структуре токенов с parseMessageMarkdown из markdown.js", () => {
    const ctx = {
      users: { "1": "Алиса" },
      channels: { "2": "новости" },
      roles: { "3": "Команда" },
    };
    const sample = "<@1> <@!9> <#2> <@&3> @everyone @here";
    const mdMentionTokens = parseMessageMarkdown(sample, ctx).filter(
      (t) => t.type !== "text",
    );
    const extracted = extractMentions(sample, ctx);
    assert.deepEqual(extracted, mdMentionTokens);
  });
});
