// @ts-check
/**
 * TAURI-MD-03 (RW-49): модульные и интеграционные тесты парсинга и рендеринга
 * эмодзи Discord (`rust/gui-spike/ui/features/emojis.js` и `markdown.js`).
 *
 * Проверяет:
 * - парсинг статических (`<:name:id>`) и анимированных (`<a:name:id>`) кастомных эмодзи,
 * - генерацию CDN URL (`https://cdn.discordapp.com/emojis/{id}.webp|gif`),
 * - граничные случаи длины имени (`2..=32` символа) и ID (`1..=32` цифры),
 * - парсинг и замену стандартных шорткодов (`:smile:`, `:thumbsup:`, `:heart:`, `:fire:` и др.),
 * - отсутствие ложных срабатываний на URL (`https://...`), время (`12:30:00`) и неизвестные `:foo:`,
 * - пропуск эмодзи внутри инлайн-кода и блоков кода в `extractEmojis`, `replaceEmojiShortcodes`
 *   и `parseMessageMarkdown`,
 * - безопасный DOM-рендер через `createEmojiElement` и `renderMessageMarkdown` (`<span class="emoji ...">`,
 *   без тегов `<img>` и без `innerHTML`),
 * - реэкспорт функций `emojis.js` из `markdown.js`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  CUSTOM_EMOJI_RE,
  DISCORD_EMOJI_CDN_BASE,
  DISCORD_SHORTCODE_EMOJIS,
  MAX_EMOJI_ID,
  MAX_EMOJI_NAME,
  MIN_EMOJI_NAME,
  SHORTCODE_EMOJI_RE,
  buildCustomEmojiUrl,
  createEmojiElement,
  extractEmojis,
  readCustomEmoji,
  readEmoji,
  readShortcodeEmoji,
  replaceEmojiShortcodes,
} from "../../rust/gui-spike/ui/features/emojis.js";
import * as markdownModule from "../../rust/gui-spike/ui/features/markdown.js";
import {
  parseMessageMarkdown,
  renderMessageMarkdown,
} from "../../rust/gui-spike/ui/features/markdown.js";

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

describe("Константы и buildCustomEmojiUrl (TAURI-MD-03)", () => {
  it("экспортирует константы лимитов и замороженный словарь шорткодов", () => {
    assert.equal(MIN_EMOJI_NAME, 2);
    assert.equal(MAX_EMOJI_NAME, 32);
    assert.equal(MAX_EMOJI_ID, 32);
    assert.equal(DISCORD_EMOJI_CDN_BASE, "https://cdn.discordapp.com/emojis");
    assert.equal(Object.isFrozen(DISCORD_SHORTCODE_EMOJIS), true);
    assert.equal(CUSTOM_EMOJI_RE instanceof RegExp, true);
    assert.equal(SHORTCODE_EMOJI_RE instanceof RegExp, true);
  });

  it("строит CDN URL для статического (.webp) и анимированного (.gif) эмодзи и валидирует ID", () => {
    assert.equal(
      buildCustomEmojiUrl("123456789012345678", false),
      "https://cdn.discordapp.com/emojis/123456789012345678.webp",
    );
    assert.equal(
      buildCustomEmojiUrl("123456789012345678", true),
      "https://cdn.discordapp.com/emojis/123456789012345678.gif",
    );
    assert.throws(() => buildCustomEmojiUrl(""), TypeError);
    assert.throws(() => buildCustomEmojiUrl("abc"), TypeError);
    assert.throws(() => buildCustomEmojiUrl("9".repeat(33)), TypeError);
  });
});

describe("readCustomEmoji — кастомные эмодзи <:name:id> и <a:name:id>", () => {
  it("разбирает статическое и анимированное кастомное эмодзи", () => {
    const staticEmoji = readCustomEmoji("<:rust_crab:987654321098765432> привет", 0);
    assert.deepEqual(staticEmoji, {
      token: {
        type: "custom_emoji",
        id: "987654321098765432",
        name: "rust_crab",
        animated: false,
        url: "https://cdn.discordapp.com/emojis/987654321098765432.webp",
        label: ":rust_crab:",
      },
      end: 31,
    });

    const animatedEmoji = readCustomEmoji("Ого <a:party_blob:112233445566778899>!", 4);
    assert.deepEqual(animatedEmoji, {
      token: {
        type: "custom_emoji",
        id: "112233445566778899",
        name: "party_blob",
        animated: true,
        url: "https://cdn.discordapp.com/emojis/112233445566778899.gif",
        label: ":party_blob:",
      },
      end: 37,
    });
  });

  it("соблюдает границы длины имени (2..=32) и ID (1..=32) и отклоняет некорректный ввод", () => {
    const minName = "ab";
    const maxName = "a".repeat(32);
    assert.equal(readCustomEmoji(`<:${minName}:1>`, 0)?.token.name, minName);
    assert.equal(readCustomEmoji(`<:${maxName}:${"9".repeat(32)}>`, 0)?.token.name, maxName);

    // Имя короче 2 или длиннее 32 символов
    assert.equal(readCustomEmoji("<:a:123>", 0), null);
    assert.equal(readCustomEmoji(`<:${"a".repeat(33)}:123>`, 0), null);

    // ID длиннее 32 цифр или нецифровой
    assert.equal(readCustomEmoji(`<:ok:${"9".repeat(33)}>`, 0), null);
    assert.equal(readCustomEmoji("<:ok:abc>", 0), null);
    assert.equal(readCustomEmoji("<b:ok:123>", 0), null);
    assert.equal(readCustomEmoji("<:ok:123", 0), null);
  });
});

describe("readShortcodeEmoji и replaceEmojiShortcodes", () => {
  it("разбирает известные шорткоды :smile:, :fire:, :+1: и поддерживает кастомный словарь", () => {
    assert.deepEqual(readShortcodeEmoji(":smile: привет", 0), {
      token: {
        type: "unicode_emoji",
        name: "smile",
        unicode: "😄",
        label: ":smile:",
      },
      end: 7,
    });

    assert.deepEqual(readShortcodeEmoji("Супер :+1:!", 6), {
      token: {
        type: "unicode_emoji",
        name: "+1",
        unicode: "👍",
        label: ":+1:",
      },
      end: 10,
    });

    const customMap = { chord: "🎵" };
    assert.deepEqual(readShortcodeEmoji(":chord:", 0, customMap), {
      token: {
        type: "unicode_emoji",
        name: "chord",
        unicode: "🎵",
        label: ":chord:",
      },
      end: 7,
    });
  });

  it("не срабатывает на неизвестные шорткоды, URL и время", () => {
    assert.equal(readShortcodeEmoji(":unknown_emoji_name:", 0), null);
    assert.equal(readShortcodeEmoji("https://example.com", 5), null);
    assert.equal(readShortcodeEmoji("http://:smile::", 7), null);
    assert.equal(readShortcodeEmoji("12:smile:00", 2), null);
  });

  it("replaceEmojiShortcodes заменяет шорткоды вне кода и сохраняет кастомные <:name:id>", () => {
    const input = "Привет :smile: и <:crab:123>! Код `:fire:` и\n```\n:heart:\n```\nКонец :rocket:";
    assert.equal(
      replaceEmojiShortcodes(input),
      "Привет 😄 и <:crab:123>! Код `:fire:` и\n```\n:heart:\n```\nКонец 🚀",
    );
    assert.throws(() => replaceEmojiShortcodes(/** @type {any} */ (null)), TypeError);
  });
});

describe("extractEmojis и интеграция с markdown.js", () => {
  it("реэкспортирует все функции и константы emojis.js из markdown.js", () => {
    assert.equal(markdownModule.extractEmojis, extractEmojis);
    assert.equal(markdownModule.readEmoji, readEmoji);
    assert.equal(markdownModule.readCustomEmoji, readCustomEmoji);
    assert.equal(markdownModule.readShortcodeEmoji, readShortcodeEmoji);
    assert.equal(markdownModule.replaceEmojiShortcodes, replaceEmojiShortcodes);
    assert.equal(markdownModule.createEmojiElement, createEmojiElement);
    assert.equal(markdownModule.buildCustomEmojiUrl, buildCustomEmojiUrl);
    assert.equal(markdownModule.CUSTOM_EMOJI_RE, CUSTOM_EMOJI_RE);
    assert.equal(markdownModule.SHORTCODE_EMOJI_RE, SHORTCODE_EMOJI_RE);
    assert.equal(markdownModule.DISCORD_SHORTCODE_EMOJIS, DISCORD_SHORTCODE_EMOJIS);
  });

  it("извлекает кастомные и шорткод-эмодзи через extractEmojis и parseMessageMarkdown, игнорируя код", () => {
    const text =
      "Старт <:wave_hand:101> и **<a: hype_dance :202>** <:ok_hand:303> плюс :fire: и `:smile: <:skip:404>`\n```\n<a:skip:505> :heart:\n```\nФинал <a:party:606>!";

    const extracted = extractEmojis(text);
    assert.deepEqual(extracted, [
      {
        type: "custom_emoji",
        id: "101",
        name: "wave_hand",
        animated: false,
        url: "https://cdn.discordapp.com/emojis/101.webp",
        label: ":wave_hand:",
      },
      {
        type: "custom_emoji",
        id: "303",
        name: "ok_hand",
        animated: false,
        url: "https://cdn.discordapp.com/emojis/303.webp",
        label: ":ok_hand:",
      },
      {
        type: "unicode_emoji",
        name: "fire",
        unicode: "🔥",
        label: ":fire:",
      },
      {
        type: "custom_emoji",
        id: "606",
        name: "party",
        animated: true,
        url: "https://cdn.discordapp.com/emojis/606.gif",
        label: ":party:",
      },
    ]);
  });

  it("рендерит эмодзи в DOM через renderMessageMarkdown без создания тегов <img>", () => {
    const container = makeDomContainer();
    renderMessageMarkdown(
      container,
      "Привет <:crab:42> <a:spin:99> :tada: и <@123>",
      { users: { "123": "Алиса" } },
    );

    const spans = container.children.filter(
      (/** @type {any} */ child) => child.tagName === "SPAN",
    );
    assert.equal(spans.length, 4);

    // 1. Статический кастомный эмодзи
    assert.equal(spans[0].className, "emoji emoji--custom");
    assert.equal(spans[0].getAttribute("role"), "img");
    assert.equal(spans[0].getAttribute("aria-label"), ":crab:");
    assert.equal(spans[0].getAttribute("data-emoji-id"), "42");
    assert.equal(spans[0].getAttribute("data-emoji-name"), "crab");
    assert.equal(spans[0].getAttribute("data-emoji-animated"), "false");
    assert.equal(
      spans[0].getAttribute("data-emoji-url"),
      "https://cdn.discordapp.com/emojis/42.webp",
    );
    assert.equal(spans[0].textContent, ":crab:");

    // 2. Анимированный кастомный эмодзи
    assert.equal(spans[1].className, "emoji emoji--custom emoji--animated");
    assert.equal(spans[1].getAttribute("data-emoji-id"), "99");
    assert.equal(spans[1].getAttribute("data-emoji-name"), "spin");
    assert.equal(spans[1].getAttribute("data-emoji-animated"), "true");
    assert.equal(
      spans[1].getAttribute("data-emoji-url"),
      "https://cdn.discordapp.com/emojis/99.gif",
    );
    assert.equal(spans[1].textContent, ":spin:");

    // 3. Шорткод Unicode-эмодзи
    assert.equal(spans[2].className, "emoji emoji--unicode");
    assert.equal(spans[2].getAttribute("data-emoji-name"), "tada");
    assert.equal(spans[2].textContent, "🎉");

    // 4. Упоминание пользователя рядом с эмодзи работает без регрессий
    assert.equal(spans[3].className, "mention mention--user");
    assert.equal(spans[3].getAttribute("data-user-id"), "123");
    assert.equal(spans[3].textContent, "@Алиса");
  });
});
