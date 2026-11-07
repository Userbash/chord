// @ts-check
/**
 * AT-03 (RW-40): поведенческие unit-тесты рендера вложений в ленте сообщений
 * (`waves/RW-40/DESIGN/ATTACHMENTS-RENDER-SPEC.md` §1–§8, `waves/RW-40/ATTACH-RENDER-RE.md`).
 *
 * Покрывает матрицу верификации T-AT03-01…T-AT03-12:
 * - T-AT03-01: фильтрация `IS_THUMBNAIL = 2`, пустые вложения/опросы и инвариант 2 детей `.row`
 * - T-AT03-02: 3 классификатора (`Rr` по расширению, `FE` по MIME, `TX` таблица `klass`)
 * - T-AT03-03: сетка Mosaic (`1..5+`), `aspect-ratio` (`computeSingleMediaBox`), бейджи GIF/ALT/REMIX
 * - T-AT03-04: состояние загрузки Skeleton, `load`/`error`, правило `placeholder_version === 1`
 * - T-AT03-05: спойлер `'СПОЙЛЕР'`, префикс `SPOILER_`, harm-флаги и двухшаговое раскрытие
 * - T-AT03-06: модалка Lightbox (`#attachment-lightbox`), навигация по сетке, `Escape` и возврат фокуса
 * - T-AT03-07: видео-вложение (`vkclips_20261015062453.mp4`), постер `?format=webp`, `m:ss`, `forceExternal`
 * - T-AT03-08: маршрутизация `AUDIO` vs `VOICE_MESSAGE` по `MessageFlags.IS_VOICE_MESSAGE = 8192`
 * - T-AT03-09: декодирование base64 `waveform` (`decodeWaveform`) и прогресс столбиков `data-played`
 * - T-AT03-10: карточка файла `.attachment-file-card`, 15 классов `klass` и размеры Б / КБ / МБ / ГБ
 * - T-AT03-11: защита `safeHref`, первый сегмент `content_type`, экранирование XSS и `refresh-urls`
 * - T-AT03-12: 100% привязка CSS-правил `chat.css` к токенам `rust/gui-spike/ui/tokens.css`
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { ShimDocument, ShimElement } from "../support/gui-dom-harness.mjs";
import * as chatModule from "../../rust/gui-spike/ui/features/chat.js";
import {
  CHAT_ROWS,
  makeMessage,
  parseChannelPage,
  renderChannelMessage,
  renderChatRow,
} from "../../rust/gui-spike/ui/features/chat.js";

const chat = /** @type {typeof chatModule & Record<string, any>} */ (chatModule);

/**
 * Живые объекты из RE-досье `waves/RW-40/ATTACH-RENDER-RE.md` §2.1 (независимый оракул).
 */
const LIVE_VIDEO_ATTACHMENT = Object.freeze({
  id: "1560312729041113179",
  filename: "vkclips_20261015062453.mp4",
  size: 9256495,
  url: "https://cdn.discordapp.com/attachments/1058348665342808104/1560312729041113179/vkclips_20261015062453.mp4?ex=6ad58db7&is=6ad43c37&hm=33319bbe&",
  proxy_url: "https://media.discordapp.net/attachments/1058348665342808104/1560312729041113179/vkclips_20261015062453.mp4?ex=6ad58db7&is=6ad43c37&hm=33319bbe&",
  width: 720,
  height: 1280,
  duration_secs: 36.0955924987793,
  content_type: "video/mp4",
  original_content_type: "video/mp4",
  content_scan_version: 4,
  placeholder: "FAgGDAJN+lpnaGOIqRgCdTVAWA==",
  placeholder_version: 1,
});

const LIVE_VOICE_ATTACHMENT = Object.freeze({
  id: "1359580913469751459",
  filename: "voice-message.ogg",
  size: 26921,
  url: "https://cdn.discordapp.com/attachments/1058348665342808104/1359580913469751459/voice-message.ogg?ex=6ad58db7&is=6ad43c37&hm=112233&",
  proxy_url: "https://media.discordapp.net/attachments/1058348665342808104/1359580913469751459/voice-message.ogg?ex=6ad58db7&is=6ad43c37&hm=112233&",
  duration_secs: 8.119976997375488,
  waveform: "AAAAAAAAAAAAABwnJBArEiAnJQAAACQTDBkAEg4DJycgIBsDDgMBAQAADBsOExkDIBUeDhwOGxAQEwcSEgUAAAAAAAAAAAAAAAAAAAAAAAAA",
  content_type: "audio/ogg",
  content_scan_version: 1,
});

const LIVE_FILE_ATTACHMENT = Object.freeze({
  id: "1434205937689952266",
  filename: "vbs.bat",
  size: 2563,
  url: "https://cdn.discordapp.com/attachments/1058348665342808104/1434205937689952266/vbs.bat?ex=6ad5919d&is=6ad4401d&hm=abcdef&",
  proxy_url: "https://media.discordapp.net/attachments/1058348665342808104/1434205937689952266/vbs.bat?ex=6ad5919d&is=6ad4401d&hm=abcdef&",
  content_type: "application/x-msdos-program; charset=utf-8",
  content_scan_version: 4,
});

/**
 * Выполняет проверку с установленным `globalThis.document` для `chat.js`.
 * @template T
 * @param {(doc: ShimDocument) => T} fn
 * @returns {T}
 */
function withChatDoc(fn) {
  const prevDoc = globalThis.document;
  const doc = new ShimDocument();
  globalThis.document = /** @type {any} */ (doc);
  try {
    return fn(doc);
  } finally {
    if (prevDoc === undefined) {
      delete /** @type {any} */ (globalThis).document;
    } else {
      globalThis.document = prevDoc;
    }
  }
}

/**
 * @param {ShimElement} root
 * @param {string} selector
 * @returns {ShimElement}
 */
function only(root, selector) {
  const matches = root.querySelectorAll(selector);
  assert.equal(matches.length, 1, `ожидался ровно один элемент «${selector}», найдено ${matches.length}`);
  return /** @type {ShimElement} */ (matches[0]);
}

/**
 * Базовое сообщение реального канала для тестов рендера.
 * @param {Partial<import("../../rust/gui-spike/ui/features/chat.js").ChannelMessage> & Record<string, any>} overrides
 * @returns {import("../../rust/gui-spike/ui/features/chat.js").ChannelMessage & Record<string, any>}
 */
function makeChannelMsg(overrides = {}) {
  return {
    id: "1561379697533911060",
    author: "sasha",
    authorId: "1001",
    text: "Сообщение с вложением",
    time: "19:05",
    iso: "2026-10-30T19:05:00Z",
    day: "2026-10-30",
    dayLabel: "",
    group: "start",
    edited: false,
    bot: false,
    hue: 210,
    channelId: "1058348665342808104",
    reactions: [],
    attachments: [],
    flags: 0,
    ...overrides,
  };
}

describe("AT-03: рендер карточек вложений в сообщениях чата", () => {
  it("T-AT03-01: фильтрует IS_THUMBNAIL=2, пропускает пустые/опросы и сохраняет инвариант 2 детей .row (.author, .text)", () => {
    withChatDoc((doc) => {
      // 1. Смешанный список (IS_THUMBNAIL=2 + обычное изображение + реакции) -> инвариант .row и порядок в .text
      const mixedRow = doc.createElement("div");
      renderChannelMessage(
        /** @type {any} */ (mixedRow),
        makeChannelMsg({
          reactions: [{ emoji: "🔥", count: 2, me: false }],
          attachments: [
            {
              id: "thumb-skip",
              filename: "skip.png",
              size: 500,
              url: "https://cdn.discordapp.com/attachments/1/2/skip.png",
              flags: 2,
            },
            {
              id: "img-keep",
              filename: "screenshot.png",
              size: 430080,
              url: "https://cdn.discordapp.com/attachments/1058/1561/screenshot.png",
              proxy_url: "https://media.discordapp.net/attachments/1058/1561/screenshot.png",
              content_type: "image/png",
              width: 1280,
              height: 720,
            },
          ],
        }),
        0,
        1,
      );
      assert.equal(mixedRow.children.length, 2, "DOM-CONTRACT §7.1: у .row ровно два ребёнка (.author и .text)");
      assert.equal(mixedRow.children[0]?.className, "author");
      assert.equal(mixedRow.children[1]?.className, "text");

      const textEl = only(mixedRow, ".text");
      assert.equal(textEl.getAttribute("data-stack"), "true");
      assert.equal(textEl.children[0]?.className, "text-body", ".text-body всегда первый ребёнок .text");
      const attachContainer = only(textEl, ".message-attachments");
      assert.equal(attachContainer.getAttribute("role"), "group");
      assert.equal(attachContainer.getAttribute("aria-label"), "Вложения сообщения");

      const extrasTokens = (mixedRow.getAttribute("data-extras") ?? "").split(/\s+/);
      assert.ok(extrasTokens.includes("attachments"), "data-extras на .row должен включать 'attachments'");
      assert.ok(extrasTokens.includes("reactions"), "data-extras на .row должен включать 'reactions'");

      const childClasses = textEl.children.map((c) => c.className);
      const attachIdx = childClasses.indexOf("message-attachments");
      const reactIdx = childClasses.indexOf("reactions-row");
      assert.ok(attachIdx > 0 && attachIdx < reactIdx, ".message-attachments должен идти после .text-body и перед .reactions-row");

      const mediaItems = attachContainer.querySelectorAll(".attachment-media-item");
      assert.equal(mediaItems.length, 1, "отрисовывается только вложение без IS_THUMBNAIL");
      assert.equal(mediaItems[0]?.getAttribute("data-attachment-id"), "img-keep");

      // 2. renderChatRow также поддерживает вложения без нарушения инварианта 2 детей .row
      const benchRow = doc.createElement("div");
      const benchItem = {
        ...makeMessage(12),
        attachments: [LIVE_FILE_ATTACHMENT],
      };
      renderChatRow(/** @type {any} */ (benchRow), benchItem, 12);
      assert.equal(benchRow.children.length, 2, "в renderChatRow у .row ровно 2 ребёнка (.author и .text)");
      assert.equal(benchRow.getAttribute("aria-setsize"), String(CHAT_ROWS));
      assert.ok(benchRow.querySelector(".text .message-attachments .attachment-file-card"));

      // 3. Пустой массив вложений, IS_THUMBNAIL=2 и опрос (isPoll) -> .message-attachments не создаётся
      const emptyRow = doc.createElement("div");
      renderChannelMessage(/** @type {any} */ (emptyRow), makeChannelMsg({ attachments: [] }), 0, 1);
      assert.equal(emptyRow.querySelector(".message-attachments"), null);

      const thumbOnlyRow = doc.createElement("div");
      renderChannelMessage(
        /** @type {any} */ (thumbOnlyRow),
        makeChannelMsg({
          attachments: [
            {
              id: "thumb-1",
              filename: "thumb.png",
              size: 1024,
              url: "https://cdn.discordapp.com/attachments/1/2/thumb.png",
              proxy_url: "https://media.discordapp.net/attachments/1/2/thumb.png",
              content_type: "image/png",
              width: 320,
              height: 180,
              flags: 2,
            },
          ],
        }),
        0,
        1,
      );
      assert.equal(thumbOnlyRow.querySelector(".message-attachments"), null, "IS_THUMBNAIL=2 обязан отфильтровываться");

      const pollRow = doc.createElement("div");
      renderChannelMessage(
        /** @type {any} */ (pollRow),
        makeChannelMsg({
          isPoll: true,
          attachments: [LIVE_FILE_ATTACHMENT],
        }),
        0,
        1,
      );
      assert.equal(pollRow.querySelector(".message-attachments"), null, "опросы не рендерят .message-attachments");

      // 4. parseChannelPage сохраняет сырые вложения и flags сообщения
      const parsed = parseChannelPage("1058348665342808104", {
        items: [
          {
            id: "1359580913469751459",
            author: { id: "u1", username: "sasha" },
            content: "",
            timestamp: "2026-10-30T19:00:00Z",
            flags: 8192,
            attachments: [LIVE_VOICE_ATTACHMENT],
          },
        ],
      });
      assert.equal(parsed.length, 1);
      assert.equal(/** @type {any} */ (parsed[0]).flags, 8192);
      assert.equal(/** @type {any} */ (parsed[0]).attachments?.length, 1);
      assert.equal(/** @type {any} */ (parsed[0]).attachments[0].waveform, LIVE_VOICE_ATTACHMENT.waveform);
      assert.equal(/** @type {any} */ (parsed[0]).attachments[0].duration_secs, LIVE_VOICE_ATTACHMENT.duration_secs);

      // 5. Константы битовых масок
      assert.equal(chat.AttachmentFlags?.IS_CLIP, 1);
      assert.equal(chat.AttachmentFlags?.IS_THUMBNAIL, 2);
      assert.equal(chat.AttachmentFlags?.IS_REMIX, 4);
      assert.equal(chat.AttachmentFlags?.IS_SPOILER, 8);
      assert.equal(chat.AttachmentFlags?.CONTAINS_EXPLICIT_MEDIA, 16);
      assert.equal(chat.AttachmentFlags?.IS_ANIMATED, 32);
      assert.equal(chat.AttachmentFlags?.CONTAINS_GORE_CONTENT, 64);
      assert.equal(chat.AttachmentFlags?.CONTAINS_SELF_HARM_CONTENT, 128);
      assert.equal(chat.AttachmentFlags?.HAS_TIMELINE_COMMENTS, 256);
      assert.equal(chat.MessageFlags?.IS_VOICE_MESSAGE, 8192);
      assert.equal(chat.MessageFlags?.EPHEMERAL, 64);
    });
  });

  it("T-AT03-02: классифицирует inline-медиа строго по расширению filename (Rr), а файлы без расширения, .svg и .psd направляет в file-card", () => {
    withChatDoc((doc) => {
      const row = doc.createElement("div");
      renderChannelMessage(
        /** @type {any} */ (row),
        makeChannelMsg({
          attachments: [
            {
              id: "a-img",
              filename: "cat.png",
              size: 12000,
              url: "https://cdn.discordapp.com/attachments/1/2/cat.png",
              proxy_url: "https://media.discordapp.net/attachments/1/2/cat.png",
              content_type: "image/png",
              width: 800,
              height: 600,
            },
            {
              id: "a-noext",
              filename: "cat",
              size: 12000,
              url: "https://cdn.discordapp.com/attachments/1/2/cat",
              content_type: "image/png",
            },
            {
              id: "a-svg",
              filename: "logo.svg",
              size: 4096,
              url: "https://cdn.discordapp.com/attachments/1/2/logo.svg",
              content_type: "image/svg+xml",
            },
            {
              id: "a-psd",
              filename: "mockup.psd",
              size: 204800,
              url: "https://cdn.discordapp.com/attachments/1/2/mockup.psd",
              content_type: "image/vnd.adobe.photoshop",
            },
          ],
        }),
        0,
        1,
      );

      const mosaic = only(row, ".attachments-mosaic");
      assert.equal(mosaic.getAttribute("data-mosaic-count"), "1");
      assert.equal(mosaic.getAttribute("data-layout"), "single");
      const mediaItems = mosaic.querySelectorAll(".attachment-media-item");
      assert.equal(mediaItems.length, 1);
      assert.equal(mediaItems[0]?.getAttribute("data-attachment-id"), "a-img");

      const fileCards = row.querySelectorAll(".attachment-file-card");
      assert.equal(fileCards.length, 3, "cat без расширения, logo.svg и mockup.psd должны быть карточками файлов");
      assert.equal(fileCards[0]?.getAttribute("data-file-klass"), "image");
      assert.equal(fileCards[1]?.getAttribute("data-file-klass"), "webcode");
      assert.equal(fileCards[2]?.getAttribute("data-file-klass"), "photoshop");

      assert.equal(chat.classifyInlineMedia({ filename: "cat.png", content_type: "image/png" }), "IMAGE");
      assert.equal(chat.classifyInlineMedia({ filename: "photo.AVIF", content_type: "image/avif" }), "IMAGE");
      assert.equal(chat.classifyInlineMedia({ filename: "cat", content_type: "image/png" }), "INVALID");
      assert.equal(chat.classifyInlineMedia({ filename: "logo.svg", content_type: "image/svg+xml" }), "INVALID");
      assert.equal(chat.classifyInlineMedia({ filename: "design.psd", content_type: "image/vnd.adobe.photoshop" }), "INVALID");
      assert.equal(
        chat.classifyInlineMedia({
          filename: "clip.mp4",
          content_type: "video/mp4",
          proxy_url: "https://media.discordapp.net/attachments/1/2/clip.mp4",
        }),
        "VIDEO",
      );
      assert.equal(
        chat.classifyInlineMedia({
          filename: "clip",
          content_type: "video/mp4",
          proxy_url: "https://media.discordapp.net/attachments/1/2/clip",
        }),
        "INVALID",
      );
    });
  });

  it("T-AT03-03: строит сетку Mosaic (1..5+), сохраняет aspect-ratio (computeSingleMediaBox) и рендерит бейджи GIF, ALT и IS_REMIX=4", () => {
    withChatDoc((doc) => {
      const expectedLayouts = [
        [1, "single"],
        [2, "mosaic-2"],
        [3, "mosaic-3"],
        [4, "mosaic-4"],
        [5, "mosaic-many"],
      ];

      for (const [count, layout] of expectedLayouts) {
        const row = doc.createElement("div");
        const attachments = Array.from({ length: Number(count) }, (_, i) => ({
          id: `img-${i + 1}`,
          filename: i === 0 ? "anim.gif" : `pic-${i + 1}.png`,
          size: 10240 * (i + 1),
          url: `https://cdn.discordapp.com/attachments/1/2/pic-${i + 1}.png`,
          proxy_url: `https://media.discordapp.net/attachments/1/2/pic-${i + 1}.png`,
          content_type: i === 0 ? "image/gif" : "image/png",
          width: i === 1 ? undefined : 1280,
          height: i === 1 ? undefined : 720,
          flags: i === 0 ? 32 | 4 : 0, // IS_ANIMATED=32 | IS_REMIX=4
          description: i === 0 ? "Скриншот окна настроек" : null,
        }));

        renderChannelMessage(/** @type {any} */ (row), makeChannelMsg({ attachments }), 0, 1);
        const mosaic = only(row, ".attachments-mosaic");
        assert.equal(mosaic.getAttribute("data-mosaic-count"), String(count));
        assert.equal(mosaic.getAttribute("data-layout"), layout);

        const items = mosaic.querySelectorAll(".attachment-media-item");
        assert.equal(items.length, Number(count));

        const first = /** @type {ShimElement} */ (items[0]);
        assert.equal(first.getAttribute("data-animated"), "true");
        assert.equal(first.getAttribute("data-remix"), "true");
        assert.equal(first.getAttribute("data-attachment-flags"), "36");
        assert.equal(first.getAttribute("data-width"), "1280");
        assert.equal(first.getAttribute("data-height"), "720");
        assert.ok(first.querySelector(".attachment-remix-badge"), "при IS_REMIX=4 должен присутствовать бейдж ремикса");

        const gifBadge = only(first, ".attachment-gif-badge");
        assert.equal(gifBadge.textContent, "GIF");
        assert.equal(gifBadge.hasAttribute("hidden"), false, "у раскрытого анимированного изображения виден бейдж GIF");

        const altBadge = only(first, ".attachment-alt-badge");
        assert.equal(altBadge.textContent, "ALT");
        assert.equal(altBadge.getAttribute("aria-expanded"), "false");
        assert.equal(altBadge.getAttribute("title"), "ПОДСКАЗКА: Скриншот окна настроек");
        altBadge.click();
        assert.equal(altBadge.getAttribute("aria-expanded"), "true");

        if (Number(count) >= 2) {
          const second = /** @type {ShimElement} */ (items[1]);
          assert.equal(second.getAttribute("data-aspect-fallback"), "true", "без width/height ставится data-aspect-fallback=true");
        }
        if (Number(count) === 5) {
          const fourth = /** @type {ShimElement} */ (items[3]);
          const overflow = only(fourth, ".attachment-mosaic-overflow");
          assert.equal(overflow.getAttribute("data-overflow-extra"), "1");
          assert.equal(overflow.textContent, "+1");
        }
      }

      const box = chat.computeSingleMediaBox(1280, 720);
      assert.equal(box.aspectRatio, "1280 / 720");
      assert.ok(box.width <= 442 && box.width >= 150);
      assert.ok(box.height <= 400 && box.height >= 110);
      assert.equal(chat.computeSingleMediaBox(null, null).aspectRatio, "16 / 9");
    });
  });

  it("T-AT03-04: управляет состояниями Skeleton (loading -> loaded / error) и активирует blurhash строго при placeholder_version === 1", () => {
    withChatDoc((doc) => {
      const row = doc.createElement("div");
      renderChannelMessage(
        /** @type {any} */ (row),
        makeChannelMsg({
          attachments: [
            {
              id: "img-v1",
              filename: "one.png",
              size: 4096,
              url: "https://cdn.discordapp.com/attachments/1/2/one.png",
              proxy_url: "https://media.discordapp.net/attachments/1/2/one.png",
              content_type: "image/png",
              width: 640,
              height: 360,
              placeholder: "FAgGDAJN+lpnaGOIqRgCdTVAWA==",
              placeholder_version: 1,
            },
            {
              id: "img-v2",
              filename: "two.png",
              size: 4096,
              url: "https://cdn.discordapp.com/attachments/1/2/two.png",
              proxy_url: "https://media.discordapp.net/attachments/1/2/two.png",
              content_type: "image/png",
              width: 640,
              height: 360,
              placeholder: "FAgGDAJN+lpnaGOIqRgCdTVAWA==",
              placeholder_version: 2,
            },
          ],
        }),
        0,
        1,
      );

      const items = row.querySelectorAll(".attachment-media-item");
      assert.equal(items.length, 2);

      const fig1 = /** @type {ShimElement} */ (items[0]);
      const fig2 = /** @type {ShimElement} */ (items[1]);

      assert.equal(fig1.getAttribute("data-state"), "loading");
      assert.equal(fig1.getAttribute("aria-busy"), "true");
      const skel1 = only(fig1, ".attachment-skeleton");
      assert.equal(skel1.hasAttribute("hidden"), false);
      assert.equal(skel1.getAttribute("data-placeholder"), "blurhash");
      assert.equal(skel1.getAttribute("data-placeholder-version"), "1");

      const skel2 = only(fig2, ".attachment-skeleton");
      assert.equal(skel2.getAttribute("data-placeholder"), null, "placeholder_version !== 1 обязан игнорироваться");

      // Событие load переводит первую картинку в loaded и скрывает skeleton
      const img1 = only(fig1, ".attachment-image");
      img1.emit("load");
      assert.equal(fig1.getAttribute("data-state"), "loaded");
      assert.equal(fig1.getAttribute("aria-busy"), "false");
      assert.equal(skel1.hasAttribute("hidden"), true);

      // Событие error переводит вторую картинку в error и показывает .attachment-media-error
      const img2 = only(fig2, ".attachment-image");
      img2.emit("error");
      assert.equal(fig2.getAttribute("data-state"), "error");
      assert.equal(fig2.getAttribute("aria-busy"), "false");
      const err2 = only(fig2, ".attachment-media-error");
      assert.equal(err2.hasAttribute("hidden"), false);
      assert.match(err2.textContent, /Не удалось загрузить изображение\./);
    });
  });

  it("T-AT03-05: рендерит спойлер-оверлей 'СПОЙЛЕР' (IS_SPOILER=8, префикс SPOILER_, harm-флаги) и раскрывает в два шага по клику/Enter/Space", () => {
    withChatDoc((doc) => {
      const row = doc.createElement("div");
      doc.body.append(row);
      renderChannelMessage(
        /** @type {any} */ (row),
        makeChannelMsg({
          attachments: [
            {
              id: "sp-img-1",
              filename: "SPOILER_secret.png",
              size: 20480,
              url: "https://cdn.discordapp.com/attachments/1/2/SPOILER_secret.png",
              proxy_url: "https://media.discordapp.net/attachments/1/2/SPOILER_secret.png",
              content_type: "image/png",
              width: 800,
              height: 600,
              flags: 0,
            },
            {
              id: "sp-img-2",
              filename: "warn.png",
              size: 20480,
              url: "https://cdn.discordapp.com/attachments/1/2/warn.png",
              proxy_url: "https://media.discordapp.net/attachments/1/2/warn.png",
              content_type: "image/png",
              width: 800,
              height: 600,
              flags: 16, // CONTAINS_EXPLICIT_MEDIA
            },
            {
              id: "sp-file-1",
              filename: "archive.zip",
              size: 4096,
              url: "https://cdn.discordapp.com/attachments/1/2/archive.zip",
              content_type: "application/zip",
              flags: 8, // IS_SPOILER
            },
          ],
        }),
        0,
        1,
      );

      const mediaItems = row.querySelectorAll(".attachment-media-item");
      assert.equal(mediaItems.length, 2, "ожидались 2 медиа-элемента со спойлерами");
      const fig1 = /** @type {ShimElement} */ (mediaItems[0]);
      const fig2 = /** @type {ShimElement} */ (mediaItems[1]);

      assert.equal(fig1.getAttribute("data-spoiler"), "hidden");
      assert.equal(fig1.getAttribute("data-obscure-reason"), "spoiler");
      const trigger1 = only(fig1, ".attachment-media-trigger");
      assert.equal(trigger1.getAttribute("tabindex"), "-1");
      assert.equal(trigger1.getAttribute("aria-hidden"), "true");

      const overlay1 = only(fig1, ".attachment-spoiler-overlay");
      assert.equal(overlay1.getAttribute("aria-expanded"), "false");
      assert.equal(only(overlay1, ".attachment-spoiler-pill").textContent, "СПОЙЛЕР");
      assert.equal(only(overlay1, ".attachment-spoiler-hint").textContent, "Нажмите, чтобы узнать больше");

      const overlay2 = only(fig2, ".attachment-spoiler-overlay");
      assert.equal(only(overlay2, ".attachment-spoiler-pill").textContent, "Предупреждение о контенте");

      // Клик по оверлею раскрывает спойлер, переводит фокус на trigger, но НЕ открывает lightbox
      overlay1.click();
      assert.equal(fig1.getAttribute("data-spoiler"), "revealed");
      assert.equal(overlay1.getAttribute("aria-expanded"), "true");
      assert.equal(overlay1.hasAttribute("hidden"), true);
      assert.equal(trigger1.getAttribute("tabindex"), "0");
      assert.equal(trigger1.hasAttribute("aria-hidden"), false);
      assert.equal(doc.activeElement, trigger1, "после раскрытия фокус должен перейти на .attachment-media-trigger");
      const lightbox = doc.getElementById("attachment-lightbox");
      assert.equal(lightbox === null || lightbox.hasAttribute("hidden"), true, "раскрытие спойлера не должно открывать lightbox");

      // Раскрытие второго спойлера с клавиатуры (Space)
      const keyEvt = overlay2.dispatchEvent("keydown", { key: " " });
      assert.equal(keyEvt.prevented, true);
      assert.equal(fig2.getAttribute("data-spoiler"), "revealed");

      // Спойлер на файловой карточке
      const fileCard = only(row, ".attachment-file-card");
      assert.equal(fileCard.getAttribute("data-spoiler"), "hidden");
      const fileLink = only(fileCard, ".attachment-file-name");
      assert.equal(fileLink.getAttribute("tabindex"), "-1");
      const fileOverlay = only(fileCard, ".attachment-spoiler-overlay");
      assert.equal(only(fileOverlay, ".attachment-spoiler-hint").textContent, "Файл скрыт под спойлером");
      fileOverlay.dispatchEvent("keydown", { key: "Enter" });
      assert.equal(fileCard.getAttribute("data-spoiler"), "revealed");
      assert.equal(fileLink.getAttribute("tabindex"), "0");
      assert.equal(doc.activeElement, fileLink);

      assert.equal(chat.getObscureReason({ filename: "a.png", flags: 8 }), "spoiler");
      assert.equal(chat.getObscureReason({ filename: "SPOILER_secret.png", flags: 0 }), "spoiler");
      assert.equal(chat.getObscureReason({ filename: "a.png", flags: 16 | 8 }), "explicit_content");
      assert.equal(chat.getObscureReason({ filename: "a.png", flags: 64 }), "gore_content");
      assert.equal(chat.getObscureReason({ filename: "a.png", flags: 128 }), "self_harm_content");
      assert.equal(chat.getObscureReason({ filename: "a.png", flags: 0, content_scan_version: -1 }), "potential_explicit_content");
      assert.equal(chat.getObscureReason({ filename: "a.png", flags: 0, content_scan_version: 4 }), null);
    });
  });

  it("T-AT03-06: открывает #attachment-lightbox по клику на раскрытое изображение, листает стрелками и закрывает по Escape с возвратом фокуса", () => {
    withChatDoc((doc) => {
      const row = doc.createElement("div");
      doc.body.append(row);
      renderChannelMessage(
        /** @type {any} */ (row),
        makeChannelMsg({
          attachments: [
            {
              id: "lb-1",
              filename: "first.png",
              size: 430080,
              url: "https://cdn.discordapp.com/attachments/1058/1561/first.png",
              proxy_url: "https://media.discordapp.net/attachments/1058/1561/first.png",
              content_type: "image/png",
              width: 1280,
              height: 720,
              description: "Первый кадр",
            },
            {
              id: "lb-2",
              filename: "second.png",
              size: 204800,
              url: "https://cdn.discordapp.com/attachments/1058/1561/second.png",
              proxy_url: "https://media.discordapp.net/attachments/1058/1561/second.png",
              content_type: "image/png",
              width: 1920,
              height: 1080,
              description: "Второй кадр",
            },
          ],
        }),
        0,
        1,
      );

      const triggers = row.querySelectorAll(".attachment-media-trigger");
      assert.equal(triggers.length, 2);
      const firstTrigger = /** @type {ShimElement} */ (triggers[0]);
      firstTrigger.click();

      const lightbox = doc.getElementById("attachment-lightbox");
      assert.ok(lightbox, "#attachment-lightbox должен монтироваться в документ");
      assert.equal(lightbox.hasAttribute("hidden"), false);
      const dialog = only(lightbox, ".attachment-lightbox-dialog");
      assert.equal(dialog.getAttribute("role"), "dialog");
      assert.equal(dialog.getAttribute("aria-modal"), "true");

      const title = doc.getElementById("attachment-lightbox-title");
      const meta = doc.getElementById("attachment-lightbox-meta");
      const img = doc.getElementById("attachment-lightbox-img");
      const closeBtn = doc.getElementById("attachment-lightbox-close");
      const nextBtn = doc.getElementById("attachment-lightbox-next");
      const prevBtn = doc.getElementById("attachment-lightbox-prev");

      assert.ok(title && meta && img && closeBtn && nextBtn && prevBtn);
      assert.equal(doc.activeElement, closeBtn, "фокус при открытии lightbox переводится на #attachment-lightbox-close");
      assert.equal(title.textContent, "first.png");
      assert.match(meta.textContent, /1280 × 720/);
      assert.equal(img.getAttribute("alt"), "Первый кадр");
      assert.equal(nextBtn.hasAttribute("hidden"), false, "при 2 изображениях кнопки навигации видимы");

      // Переключение на следующее изображение по ArrowRight
      lightbox.dispatchEvent("keydown", { key: "ArrowRight" });
      assert.equal(title.textContent, "second.png");
      assert.match(meta.textContent, /1920 × 1080/);
      assert.equal(img.getAttribute("alt"), "Второй кадр");

      // Возврат на первое изображение по ArrowLeft
      lightbox.dispatchEvent("keydown", { key: "ArrowLeft" });
      assert.equal(title.textContent, "first.png");

      // Focus-trap по Tab внутри диалога
      const tabEvt = lightbox.dispatchEvent("keydown", { key: "Tab" });
      assert.equal(tabEvt.prevented, true);

      // Закрытие по Escape возвращает фокус на исходный триггер
      lightbox.dispatchEvent("keydown", { key: "Escape" });
      assert.equal(lightbox.hasAttribute("hidden"), true);
      assert.equal(doc.activeElement, firstTrigger, "после закрытия lightbox фокус возвращается на триггер");
    });
  });

  it("T-AT03-07: рендерит видео-вложение (постер proxy_url?format=webp, длительность m:ss, контролы и forceExternal > 6016×3384)", () => {
    withChatDoc((doc) => {
      const row = doc.createElement("div");
      renderChannelMessage(
        /** @type {any} */ (row),
        makeChannelMsg({
          attachments: [
            LIVE_VIDEO_ATTACHMENT,
            {
              ...LIVE_VIDEO_ATTACHMENT,
              id: "vid-huge",
              filename: "8k-demo.mp4",
              width: 7680,
              height: 4320,
            },
          ],
        }),
        0,
        1,
      );

      const videoItems = row.querySelectorAll(".attachment-video-item");
      assert.equal(videoItems.length, 2, "ожидались 2 карточки видео-вложений");

      const normalVidFig = /** @type {ShimElement} */ (videoItems[0]);
      assert.equal(normalVidFig.getAttribute("data-media-type"), "VIDEO");
      assert.equal(normalVidFig.getAttribute("data-player-state"), "idle");
      assert.equal(normalVidFig.getAttribute("data-force-external"), "false");

      const videoEl = only(normalVidFig, "video.attachment-video");
      assert.match(videoEl.getAttribute("poster") ?? "", /format=webp/, "постер видео обязан содержать format=webp");
      assert.equal(only(normalVidFig, ".attachment-video-duration").textContent, "0:36");

      const seekSlider = only(normalVidFig, ".attachment-ctrl-seek");
      assert.equal(seekSlider.getAttribute("role"), "slider");
      assert.equal(seekSlider.getAttribute("aria-valuemax"), "36");

      const playOverlay = only(normalVidFig, ".attachment-video-play-overlay");
      playOverlay.click();
      assert.equal(normalVidFig.getAttribute("data-player-state"), "playing");
      assert.equal(playOverlay.hasAttribute("hidden"), true);

      // Перемотка видео обновляет счётчик текущего времени
      seekSlider.value = "15";
      seekSlider.setAttribute("value", "15");
      seekSlider.emit("input");
      assert.equal(only(normalVidFig, ".attachment-time-current").textContent, "0:15");
      assert.equal(seekSlider.getAttribute("aria-valuenow"), "15");
      assert.equal(seekSlider.getAttribute("aria-valuetext"), "0:15 из 0:36");

      // Завершение видео (ended)
      videoEl.emit("ended");
      assert.equal(normalVidFig.getAttribute("data-player-state"), "ended");
      assert.equal(playOverlay.hasAttribute("hidden"), false);

      // Второе видео (7680×4320 > 6016×3384) -> forceExternal
      const hugeVidFig = /** @type {ShimElement} */ (videoItems[1]);
      assert.equal(hugeVidFig.getAttribute("data-force-external"), "true");
      assert.equal(hugeVidFig.querySelector("video"), null, "при forceExternal встроенный <video> не создаётся");
      assert.ok(hugeVidFig.querySelector("a.attachment-video-external"), "при forceExternal рендерится внешняя ссылка");

      assert.equal(chat.formatDuration(36.0955924987793), "0:36");
      assert.equal(chat.formatDuration(8.119976997375488), "0:08");
      assert.equal(chat.formatDuration(125.9), "2:05");
      assert.equal(chat.formatDuration(null), "--:--");
      assert.equal(chat.formatDuration(Number.NaN), "--:--");
      assert.equal(chat.formatDuration(-5), "--:--");
    });
  });

  it("T-AT03-08 & T-AT03-09: разделяет AUDIO и VOICE_MESSAGE по MessageFlags.IS_VOICE_MESSAGE=8192 и декодирует base64 waveform", () => {
    withChatDoc((doc) => {
      // 1. Обычное аудио (flags = 0) -> .attachment-audio-card
      const audioRow = doc.createElement("div");
      renderChannelMessage(
        /** @type {any} */ (audioRow),
        makeChannelMsg({
          flags: 0,
          attachments: [LIVE_VOICE_ATTACHMENT],
        }),
        0,
        1,
      );
      assert.equal(audioRow.querySelector(".attachment-voice-card"), null);
      const audioCard = only(audioRow, ".attachment-audio-card");
      assert.equal(audioCard.getAttribute("data-media-type"), "AUDIO");
      assert.equal(only(audioCard, ".attachment-audio-name").textContent, "voice-message.ogg");
      assert.equal(only(audioCard, ".attachment-audio-size").textContent, "26,3 КБ");
      assert.equal(only(audioCard, ".attachment-time-total").textContent, "0:08");

      // 2. Голосовое сообщение (flags = 8192) -> .attachment-voice-card с waveform
      const voiceRow = doc.createElement("div");
      renderChannelMessage(
        /** @type {any} */ (voiceRow),
        makeChannelMsg({
          flags: 8192,
          attachments: [LIVE_VOICE_ATTACHMENT],
        }),
        0,
        1,
      );
      assert.equal(voiceRow.querySelector(".attachment-audio-card"), null);
      const voiceCard = only(voiceRow, ".attachment-voice-card");
      assert.equal(voiceCard.getAttribute("data-media-type"), "VOICE_MESSAGE");
      assert.equal(voiceCard.getAttribute("aria-label"), "Голосовое сообщение, длительность 0:08");
      assert.equal(only(voiceCard, ".attachment-voice-duration").textContent, "0:08");

      const barNodes = voiceCard.querySelectorAll(".attachment-waveform-bar");
      assert.equal(barNodes.length, 32);
      assert.ok(barNodes.every((b) => b.getAttribute("data-played") === "false"));

      // Перемотка слайдера волны на половину (4 из 8 с) закрашивает половину столбиков
      const waveSlider = only(voiceCard, ".attachment-waveform-slider");
      waveSlider.value = "4";
      waveSlider.setAttribute("value", "4");
      waveSlider.emit("input");
      const playedCount = barNodes.filter((b) => b.getAttribute("data-played") === "true").length;
      assert.equal(playedCount, 16, "при прогрессе 4/8 ровно 16 из 32 столбиков получают data-played='true'");

      // Переключение скорости воспроизведения
      const speedBtn = only(voiceCard, ".attachment-voice-speed");
      assert.equal(speedBtn.getAttribute("data-speed"), "1");
      assert.equal(speedBtn.textContent, "1x");
      speedBtn.click();
      assert.equal(speedBtn.getAttribute("data-speed"), "1.5");
      assert.equal(speedBtn.textContent, "1.5x");

      const bars = chat.decodeWaveform(LIVE_VOICE_ATTACHMENT.waveform, 192);
      assert.equal(bars.length, 32, "при ширине 192px формула floor((192+4)/6) даёт 32 столбика");
      assert.equal(bars[0], 0, "первые байты AAAA в base64 соответствуют нулевой амплитуде");
      assert.ok(bars.some((/** @type {number} */ v) => v > 0.05 && v <= 1), "средние корзины содержат ненулевые амплитуды 0..1");
    });
  });

  it("T-AT03-10: классифицирует все 15 типов файлов classifyFile (TX) и форматирует размеры в Б / КБ / МБ / ГБ", () => {
    withChatDoc((doc) => {
      // Рендер живого объекта vbs.bat (LIVE_FILE_ATTACHMENT)
      const row = doc.createElement("div");
      renderChannelMessage(
        /** @type {any} */ (row),
        makeChannelMsg({ attachments: [LIVE_FILE_ATTACHMENT] }),
        0,
        1,
      );
      const card = only(row, ".attachment-file-card");
      assert.equal(card.getAttribute("data-attachment-id"), "1434205937689952266");
      assert.equal(card.getAttribute("data-media-type"), "FILE");
      assert.equal(card.getAttribute("data-file-klass"), "unknown");
      assert.equal(card.getAttribute("aria-label"), "Файл «vbs.bat», 2,5 КБ");

      const nameLink = only(card, "a.attachment-file-name");
      assert.equal(nameLink.textContent, "vbs.bat");
      assert.equal(nameLink.getAttribute("href"), LIVE_FILE_ATTACHMENT.url);
      assert.equal(nameLink.getAttribute("rel"), "noreferrer noopener");
      assert.equal(only(card, ".attachment-file-size").textContent, "2,5 КБ");

      const dlLink = only(card, "a.attachment-file-download");
      assert.equal(dlLink.getAttribute("download"), "vbs.bat");
      assert.equal(dlLink.getAttribute("aria-label"), "Скачать файл «vbs.bat» (2,5 КБ)");

      // Проверка оракулов размера из таблицы §5.2
      assert.equal(chat.formatAttachmentSize(0), "0 Б");
      assert.equal(chat.formatAttachmentSize(512), "512 Б");
      assert.equal(chat.formatAttachmentSize(1024), "1 КБ");
      assert.equal(chat.formatAttachmentSize(2563), "2,5 КБ");
      assert.equal(chat.formatAttachmentSize(26921), "26,3 КБ");
      assert.equal(chat.formatAttachmentSize(1048576), "1 МБ");
      assert.equal(chat.formatAttachmentSize(9256495), "8,8 МБ");
      assert.equal(chat.formatAttachmentSize(1073741824), "1 ГБ");

      // Проверка всех 15 правил таблицы §5.1
      assert.equal(chat.classifyFile("design.psd", "image/vnd.adobe.photoshop"), "photoshop");
      assert.equal(chat.classifyFile("icon.svg", "image/svg+xml"), "webcode");
      assert.equal(chat.classifyFile("raw_img", "image/png"), "image");
      assert.equal(chat.classifyFile("raw_vid", "video/quicktime"), "video");
      assert.equal(chat.classifyFile("manual.pdf", "application/pdf"), "acrobat");
      assert.equal(chat.classifyFile("comp.ae", "application/octet-stream"), "ae");
      assert.equal(chat.classifyFile("ui.sketch", "application/octet-stream"), "sketch");
      assert.equal(chat.classifyFile("vector.ai", "application/postscript"), "ai");
      assert.equal(chat.classifyFile("bundle.tar.gz", "application/gzip"), "archive");
      assert.equal(chat.classifyFile("main.rs", "text/plain; charset=utf-8"), "code");
      assert.equal(chat.classifyFile("README.md", "text/markdown"), "document");
      assert.equal(chat.classifyFile("report.csv", "text/csv"), "spreadsheet");
      assert.equal(chat.classifyFile("index.html", "text/html"), "webcode");
      assert.equal(chat.classifyFile("sample.flac", "application/octet-stream"), "audio");
      assert.equal(chat.classifyFile("vbs.bat", "application/x-msdos-program; charset=utf-8"), "unknown");
    });
  });

  it("T-AT03-11: блокирует опасные URL через safeHref, парсит первый сегмент content_type, экранирует XSS и обновляет истёкшие CDN-ссылки", async () => {
    await withChatDoc(async (doc) => {
      const row = doc.createElement("div");
      /** @type {string[][]} */
      const refreshedBatches = [];
      renderChannelMessage(
        /** @type {any} */ (row),
        makeChannelMsg({
          attachments: [
            {
              id: "xss-file",
              filename: "<script>alert(1)</script><img src=x onerror=alert(1)>.txt",
              size: 1024,
              url: "javascript:alert(1)",
              content_type: "text/plain; charset=utf-8",
            },
            {
              id: "xss-img",
              filename: "bad.png",
              size: 2048,
              url: "data:text/html,<script>alert(1)</script>",
              proxy_url: "javascript:evil()",
              content_type: "image/png",
              width: 400,
              height: 300,
            },
            {
              id: "exp-img",
              filename: "expiring.png",
              size: 4096,
              // ex=00000010 (давно истёк относительно текущего времени)
              url: "https://cdn.discordapp.com/attachments/1058/1561/expiring.png?ex=00000010&is=00000001&hm=abcd",
              proxy_url: "https://media.discordapp.net/attachments/1058/1561/expiring.png?ex=00000010&is=00000001&hm=abcd",
              content_type: "image/png",
              width: 400,
              height: 300,
            },
          ],
        }),
        0,
        1,
        {
          channelId: "1058348665342808104",
          refreshUrls: async (/** @type {string[]} */ urls) => {
            refreshedBatches.push(urls);
            return urls.map((u) => ({
              original: u,
              refreshed: u.replace("ex=00000010", "ex=ffffffff"),
            }));
          },
        },
      );

      const fileCard = only(row, ".attachment-file-card");
      assert.equal(row.querySelectorAll("script").length, 0, "XSS-<script> из имени файла не должен попадать в DOM");

      const fileNameLink = only(fileCard, ".attachment-file-name");
      assert.equal(fileNameLink.getAttribute("href"), null, "javascript:-ссылка обязана блокироваться");
      assert.equal(fileNameLink.getAttribute("aria-disabled"), "true");
      const fileDlLink = only(fileCard, ".attachment-file-download");
      assert.equal(fileDlLink.getAttribute("href"), null);
      assert.equal(fileDlLink.getAttribute("aria-disabled"), "true");

      const mediaItems = row.querySelectorAll(".attachment-media-item");
      const badImgFig = /** @type {ShimElement} */ (mediaItems[0]);
      assert.equal(badImgFig.getAttribute("data-state"), "error", "невалидный URL изображения переводит карточку в data-state='error'");

      // Проверка обновления истёкшего CDN URL
      await Promise.resolve();
      assert.equal(refreshedBatches.length, 1, "истёкший ex должен вызвать один refresh запрос");
      const expImg = only(/** @type {ShimElement} */ (mediaItems[1]), "img.attachment-image");
      assert.match(expImg.getAttribute("src") ?? "", /ex=ffffffff/, "src изображения должен обновиться на свежий URL");

      assert.equal(chat.firstMimeSegment("text/plain; charset=utf-8"), "text");
      assert.equal(chat.firstMimeSegment("AUDIO/OGG; codecs=opus"), "audio");
      assert.equal(chat.firstMimeSegment("application/x-msdos-program; charset=utf-8"), "application");
    });
  });

  it("T-AT03-12: chat.css содержит все селекторы AT-03 и использует исключительно токены из rust/gui-spike/ui/tokens.css", () => {
    const chatCss = readFileSync(
      fileURLToPath(new URL("../../rust/gui-spike/ui/components/chat.css", import.meta.url)),
      "utf8",
    );
    const tokensCss = readFileSync(
      fileURLToPath(new URL("../../rust/gui-spike/ui/tokens.css", import.meta.url)),
      "utf8",
    );

    const requiredSelectors = [
      ".message-attachments",
      ".attachments-mosaic",
      ".attachment-media-item",
      ".attachment-spoiler-overlay",
      ".attachment-video-stage",
      ".attachment-video-controls",
      ".attachment-audio-card",
      ".attachment-voice-card",
      ".attachment-waveform-bar",
      ".attachment-file-card",
      ".attachment-lightbox-dialog",
    ];
    for (const sel of requiredSelectors) {
      assert.ok(chatCss.includes(sel), `в chat.css отсутствует обязательный селектор ${sel}`);
    }

    const withoutComments = chatCss.replace(/\/\*[\s\S]*?\*\//g, "");
    assert.doesNotMatch(withoutComments, /#[0-9a-fA-F]{3,8}\b/, "в chat.css запрещены сырые hex-цвета");
    assert.doesNotMatch(withoutComments, /\brgba?\s*\(/i, "в chat.css запрещены сырые rgb/rgba()");
    assert.doesNotMatch(withoutComments, /\bhsla?\s*\(/i, "в chat.css запрещены сырые hsl/hsla()");
    assert.doesNotMatch(withoutComments, /!important/, "в chat.css запрещён !important");

    const definedTokens = new Set(
      [...tokensCss.matchAll(/(--[a-z0-9-]+)\s*:/gi)].map((m) => m[1]),
    );
    const usedTokens = [...withoutComments.matchAll(/var\(\s*(--[a-z0-9-]+)\s*(?:,[^)]+)?\)/gi)].map((m) => m[1]);
    assert.ok(usedTokens.length >= 25);
    for (const token of usedTokens) {
      assert.ok(definedTokens.has(token), `токен ${token} в chat.css не объявлен в tokens.css`);
    }
  });
});
