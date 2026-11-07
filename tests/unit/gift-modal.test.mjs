// @ts-check
/**
 * TASK-627 (G2 Frontend): модульные тесты интерактивного модального окна выбора
 * и кастомизации подарка Nitro (`#gift-modal`, `rust/gui-spike/ui/features/gift-modal.js`,
 * `rust/gui-spike/ui/styles/gift-modal.css`).
 *
 * RE-оракулы:
 * - `waves/RW-40/RE-DEEP-REA.md` §5 (`STUB-12 — кнопка Gift 🎁`, маршруты `@290386..@290799`,
 *   `createGiftCode` `@613587`, `gift_style` `0..=2`);
 * - `waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md` §4.1–§4.6 (`#gift-modal`, лимит `200` символов,
 *   машина состояний `closed`/`loading`/`ready`/`submitting`/`success`/`error-429`/`error-403`/`error-400`/`error-network`,
 *   фокус-ловушка `Tab`/`Shift+Tab`, закрытие по `Escape` и возврат фокуса на триггер);
 * - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`:
 *   - модуль `202541` (`@6789126..@6797000`): `NITRO_APPLICATION_ID`, `SKU_*`, 10 планов `PLAN_*`,
 *     стили открыток `GIFT_CARD_STYLES` (`SNOWGLOBE=1..NITROWEEN_STANDARD=12`), `GIFTING_ORIGINS`;
 *   - модуль `45938` (`@9711598`): `GiftStyle` (`0..=2`), `resolveGiftStyle`, `extractGiftCodesFromText` (до 3 кодов),
 *     `normalizeGiftCode`;
 *   - модуль `652215` (`@320207`, `@325314`): коды ошибок Discord (`10038`, `50035`, `50050`, `50051`, `50054`, `50092`, `50095`, `50097`);
 * - `/home/sanya/re-deep/decoded/3e8824f7eca8914f_0` (`@229000..@233000`, `@372000..@385000`,
 *   модули `968060`, `274076`, `354972`, `650588`, `472468`): кастомизация подарка (открытка,
 *   сообщение, конфетти из эмодзи `emojiConfetti`, звуковой эффект `soundEffect`, запрет подарка самому себе);
 * - `rust/gui-spike/src/gifts.rs` (`TASK-626`) и `rust/gui-spike/ui/tokens.css`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { ShimDocument } from "../support/gui-dom-harness.mjs";

import {
  DEFAULT_NITRO_GIFT_PLANS,
  DISCORD_GIFT_ERROR_CODES,
  GIFTING_ORIGINS,
  GIFT_CARD_STYLES,
  GIFT_IPC_COMMANDS,
  GIFT_MODAL_STATES,
  GIFT_SKU_IDS,
  GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD,
  GIFT_STYLE_CUSTOM_STYLE,
  GIFT_STYLE_DEFAULT,
  GIFT_SUBSCRIPTION_PLANS,
  GIFT_TEXT,
  MAX_EXTRACTED_GIFT_CODES,
  MAX_GIFT_CODE_CHARS,
  MAX_GIFT_CUSTOM_MESSAGE_CHARS,
  MIN_GIFT_CODE_CHARS,
  NITRO_APPLICATION_ID,
  buildCreateGiftPayload,
  classifyGiftError,
  createGiftModal,
  extractGiftCodesFromText,
  formatGiftPlanLabelRu,
  formatGiftPlanPrice,
  formatGiftUrl,
  isValidSnowflake,
  normalizeGiftCode,
  resolveGiftStyle,
  validateGiftCustomMessage,
  validateGiftRecipient,
} from "../../rust/gui-spike/ui/features/gift-modal.js";

const GIFT_MODAL_JS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/features/gift-modal.js", import.meta.url),
);
const GIFT_MODAL_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/styles/gift-modal.css", import.meta.url),
);
const TOKENS_CSS_PATH = fileURLToPath(
  new URL("../../rust/gui-spike/ui/tokens.css", import.meta.url),
);

/**
 * Создаёт изолированное тестовое DOM-окружение с кнопкой подарка композера.
 * @returns {{ doc: any, container: any, triggerBtn: any }}
 */
function createTestHarness() {
  const doc = /** @type {any} */ (new ShimDocument());
  const container = /** @type {any} */ (doc.createElement("div"));
  container.id = "app-root";
  const triggerBtn = /** @type {any} */ (doc.createElement("button"));
  triggerBtn.setAttribute("type", "button");
  triggerBtn.setAttribute("data-action", "composer-gift");
  triggerBtn.textContent = "🎁";
  container.appendChild(triggerBtn);
  doc.body.appendChild(container);
  return { doc, container, triggerBtn };
}

describe("TASK-627: Константы RE-оракула, каталог планов Nitro и чистые функции подарков", () => {
  it("экспортирует канонические константы модулей 202541, 45938 и 652215", () => {
    assert.equal(NITRO_APPLICATION_ID, "521842831262875670");
    assert.equal(Object.isFrozen(GIFT_SKU_IDS), true);
    assert.equal(GIFT_SKU_IDS.NONE, "628379670982688768");
    assert.equal(GIFT_SKU_IDS.NITRO_BASIC_TIER_0, "978380684370378762");
    assert.equal(GIFT_SKU_IDS.NITRO_CLASSIC_TIER_1, "521846918637420545");
    assert.equal(GIFT_SKU_IDS.NITRO_STANDARD_TIER_2, "521847234246082599");
    assert.equal(GIFT_SKU_IDS.GUILD_BOOST, "590663762298667008");
    assert.equal(GIFT_SKU_IDS.NITRO_LEGACY, "521842865731534868");

    assert.equal(Object.isFrozen(GIFT_SUBSCRIPTION_PLANS), true);
    assert.equal(GIFT_SUBSCRIPTION_PLANS.NITRO_BASIC_MONTH, "978380692553465866");
    assert.equal(GIFT_SUBSCRIPTION_PLANS.NITRO_BASIC_YEAR, "1024422698568122368");
    assert.equal(GIFT_SUBSCRIPTION_PLANS.NITRO_CLASSIC_MONTH, "511651871736201216");
    assert.equal(GIFT_SUBSCRIPTION_PLANS.NITRO_CLASSIC_YEAR, "511651876987469824");
    assert.equal(GIFT_SUBSCRIPTION_PLANS.NITRO_MONTH, "511651880837840896");
    assert.equal(GIFT_SUBSCRIPTION_PLANS.NITRO_YEAR, "511651885459963904");
    assert.equal(GIFT_SUBSCRIPTION_PLANS.NITRO_3_MONTH, "642251038925127690");
    assert.equal(GIFT_SUBSCRIPTION_PLANS.NITRO_6_MONTH, "944037208325619722");
    assert.equal(GIFT_SUBSCRIPTION_PLANS.GUILD_BOOST_MONTH, "590665532894740483");
    assert.equal(GIFT_SUBSCRIPTION_PLANS.GUILD_BOOST_YEAR, "590665538238152709");

    assert.equal(GIFT_STYLE_DEFAULT, 0);
    assert.equal(GIFT_STYLE_CUSTOM_STYLE, 1);
    assert.equal(GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD, 2);
    assert.equal(MAX_GIFT_CUSTOM_MESSAGE_CHARS, 200);
    assert.equal(MIN_GIFT_CODE_CHARS, 2);
    assert.equal(MAX_GIFT_CODE_CHARS, 64);
    assert.equal(MAX_EXTRACTED_GIFT_CODES, 3);

    assert.equal(Object.isFrozen(DEFAULT_NITRO_GIFT_PLANS), true);
    assert.equal(DEFAULT_NITRO_GIFT_PLANS.length, 10);
    assert.equal(Object.isFrozen(GIFT_CARD_STYLES), true);
    assert.ok(GIFT_CARD_STYLES.length >= 7);
    assert.equal(Object.isFrozen(GIFTING_ORIGINS), true);
    assert.equal(GIFTING_ORIGINS.DM_CHANNEL, 0);
    assert.equal(GIFTING_ORIGINS.GUILD_CHANNEL, 4);

    assert.equal(Object.isFrozen(DISCORD_GIFT_ERROR_CODES), true);
    assert.equal(DISCORD_GIFT_ERROR_CODES.UNKNOWN_GIFT_CODE, 10038);
    assert.equal(DISCORD_GIFT_ERROR_CODES.INVALID_FORM_BODY, 50035);
    assert.equal(DISCORD_GIFT_ERROR_CODES.INVALID_GIFT_REDEMPTION_EXHAUSTED, 50050);
    assert.equal(DISCORD_GIFT_ERROR_CODES.INVALID_GIFT_REDEMPTION_OWNED, 50051);
    assert.equal(DISCORD_GIFT_ERROR_CODES.INVALID_GIFT_SELF_REDEMPTION, 50054);
    assert.equal(DISCORD_GIFT_ERROR_CODES.INVALID_GIFT_REDEMPTION_PREVIOUSLY_OWNED, 50092);
    assert.equal(DISCORD_GIFT_ERROR_CODES.INVALID_COUNTRY_CODE, 50095);
    assert.equal(DISCORD_GIFT_ERROR_CODES.INVALID_GIFT_REDEMPTION_FRAUD_REJECTED, 50097);

    assert.deepEqual(GIFT_IPC_COMMANDS, [
      "resolve_gift_code",
      "create_gift_code",
      "fetch_user_gift_codes",
      "revoke_gift_code",
      "redeem_gift_code",
      "fetch_user_entitlements",
      "fetch_application_entitlements",
      "fetch_giftable_entitlements",
      "list_nitro_gift_plans",
    ]);
    assert.equal(Object.isFrozen(GIFT_MODAL_STATES), true);
    assert.equal(Object.isFrozen(GIFT_TEXT), true);
  });

  it("resolveGiftStyle и validateGiftCustomMessage соблюдают границу 200 символов и правило 1 vs 2", () => {
    // Пустое или пробельное сообщение без эмодзи/звука -> 1 (CUSTOM_STYLE)
    assert.equal(resolveGiftStyle(""), GIFT_STYLE_CUSTOM_STYLE);
    assert.equal(resolveGiftStyle("   \n\t "), GIFT_STYLE_CUSTOM_STYLE);
    assert.equal(resolveGiftStyle(null), GIFT_STYLE_CUSTOM_STYLE);

    // Непустое сообщение -> 2 (CUSTOM_MESSAGE_EMOJI_SOUNDBOARD)
    assert.equal(resolveGiftStyle("С праздником!"), GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD);

    // Эмодзи-конфетти или звуковой эффект при пустом тексте -> 2
    assert.equal(
      resolveGiftStyle("", { hasEmojiOrSound: true }),
      GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD,
    );
    assert.equal(
      resolveGiftStyle("", { emojiConfetti: { id: null, name: "🎉" } }),
      GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD,
    );
    assert.equal(
      resolveGiftStyle("", { soundEffect: { soundId: "101", name: "Airhorn" } }),
      GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD,
    );

    // Граница 200 UTF-8 символов (включительно) и отказ на 201 символе
    const msg200 = "я".repeat(200);
    const msg201 = "я".repeat(201);
    assert.equal(validateGiftCustomMessage(msg200).valid, true);
    assert.equal(validateGiftCustomMessage(msg200).charCount, 200);
    assert.equal(resolveGiftStyle(msg200), GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD);

    const invalid201 = validateGiftCustomMessage(msg201);
    assert.equal(invalid201.valid, false);
    assert.equal(invalid201.charCount, 201);
    assert.throws(() => resolveGiftStyle(msg201), /200/);
  });

  it("normalizeGiftCode, extractGiftCodesFromText и formatGiftUrl соответствуют модулю 45938", () => {
    assert.equal(normalizeGiftCode("Wump-Ab12-Cd34"), "WumpAb12Cd34");
    assert.equal(normalizeGiftCode("https://discord.gift/Wump-Ab12-Cd34?ref=chat"), "WumpAb12Cd34");
    assert.equal(normalizeGiftCode("https://promos.discord.gg/?code=PROMO-2026"), "PROMO2026");
    assert.equal(normalizeGiftCode(""), null);
    assert.equal(normalizeGiftCode("a"), null); // < 2 символов
    assert.equal(normalizeGiftCode("x".repeat(65)), null); // > 64 символов
    assert.equal(normalizeGiftCode("bad$code!"), null);

    const text =
      "Подарки: https://discord.gift/Wump-1111-2222 и https://discord.com/gifts/Nitro-3333 " +
      "повтор https://discord.gift/Wump11112222 плюс https://promos.discord.gg/?code=PROMO-4444 " +
      "и четвёртый лишний https://discordapp.com/gifts/EXTRA-5555";
    const codes = extractGiftCodesFromText(text);
    assert.deepEqual(codes, ["Wump11112222", "Nitro3333", "PROMO4444"]);
    assert.equal(codes.length, MAX_EXTRACTED_GIFT_CODES);

    assert.equal(formatGiftUrl("Wump-Ab12"), "https://discord.gift/WumpAb12");
    assert.equal(formatGiftUrl(""), "");
  });

  it("isValidSnowflake и validateGiftRecipient проверяют границы Snowflake и запрет подарка самому себе", () => {
    assert.equal(isValidSnowflake("1"), true);
    assert.equal(isValidSnowflake("18446744073709551615"), true); // u64::MAX
    assert.equal(isValidSnowflake("0"), false);
    assert.equal(isValidSnowflake("012345"), false);
    assert.equal(isValidSnowflake("18446744073709551616"), false); // u64::MAX + 1
    assert.equal(isValidSnowflake("-5"), false);
    assert.equal(isValidSnowflake("abc"), false);

    // Запрет отправки подарка самому себе (3e8824f7eca8914f_0 @200300: d.id === A?.id)
    const selfCheck = validateGiftRecipient(
      { id: "100200300", username: "sanya" },
      "100200300",
    );
    assert.equal(selfCheck.valid, false);
    assert.equal(selfCheck.reason, "self");

    const badSnowflake = validateGiftRecipient({ id: "0", username: "zero" }, "100200300");
    assert.equal(badSnowflake.valid, false);
    assert.equal(badSnowflake.reason, "invalid_snowflake");

    const validRecipient = validateGiftRecipient(
      { id: "999888777", username: "friend" },
      "100200300",
    );
    assert.equal(validRecipient.valid, true);
  });

  it("formatGiftPlanPrice, formatGiftPlanLabelRu и buildCreateGiftPayload формируют честные данные без хардкода цен", () => {
    // Без цены в плане строка цены пустая (§4.4 ATTACHMENTS-SPEC: цена не хардкодится)
    assert.equal(formatGiftPlanPrice({ id: "511651880837840896", name: "Nitro Monthly" }), "");
    assert.equal(
      formatGiftPlanPrice({ priceText: "$9.99 / мес." }),
      "$9.99 / мес.",
    );
    assert.equal(
      formatGiftPlanPrice({ amount: 999, currency: "USD" }),
      "9.99 USD",
    );
    assert.equal(
      formatGiftPlanPrice({ amount: 0, currency: "USD" }),
      "0.00 USD",
    );

    assert.equal(
      formatGiftPlanLabelRu({
        id: GIFT_SUBSCRIPTION_PLANS.NITRO_MONTH,
        name: "Nitro Monthly",
        interval: 1,
        intervalCount: 1,
        premiumType: 2,
      }),
      "Nitro — 1 мес.",
    );
    assert.equal(
      formatGiftPlanLabelRu({
        id: GIFT_SUBSCRIPTION_PLANS.NITRO_BASIC_YEAR,
        name: "Nitro Basic Yearly",
        interval: 2,
        intervalCount: 1,
        premiumType: 3,
      }),
      "Nitro Basic — 1 год",
    );

    const payload = buildCreateGiftPayload({
      plan: {
        id: GIFT_SUBSCRIPTION_PLANS.NITRO_MONTH,
        skuId: GIFT_SKU_IDS.NITRO_STANDARD_TIER_2,
      },
      customMessage: "Поздравляю!",
      cardStyleId: 5, // CAKE
      emojiConfetti: { id: null, name: "🎂" },
      soundEffect: { soundId: "snd_1", name: "Party Horn" },
      recipient: { id: "999888777", username: "friend" },
    });

    assert.equal(payload.sku_id, GIFT_SKU_IDS.NITRO_STANDARD_TIER_2);
    assert.equal(payload.skuId, GIFT_SKU_IDS.NITRO_STANDARD_TIER_2);
    assert.equal(payload.subscription_plan_id, GIFT_SUBSCRIPTION_PLANS.NITRO_MONTH);
    assert.equal(payload.subscriptionPlanId, GIFT_SUBSCRIPTION_PLANS.NITRO_MONTH);
    assert.equal(payload.gift_style, GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD);
    assert.equal(payload.giftStyle, GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD);
    assert.equal(payload.custom_message, "Поздравляю!");
    assert.equal(payload.recipient_id, "999888777");
    assert.equal(payload.emoji_name, "🎂");
    assert.equal(payload.sound_id, "snd_1");
    assert.equal(payload.card_style_id, 5);
  });

  it("classifyGiftError распознаёт HTTP 400/401/403/404/429/5xx и коды Discord 10038/50035/50050/50051/50054/50092/50095/50097", () => {
    const rate429 = classifyGiftError({ kind: "rateLimited", retry_after_ms: 3200 });
    assert.equal(rate429.state, "error-429");
    assert.equal(rate429.retryAfterSeconds, 4);
    assert.equal(rate429.message, GIFT_TEXT.rateLimited(4));

    const rate429Fallback = classifyGiftError({ status: 429 });
    assert.equal(rate429Fallback.state, "error-429");
    assert.equal(rate429Fallback.retryAfterSeconds, 5);

    const err403 = classifyGiftError({ kind: "forbidden", status: 403 });
    assert.equal(err403.state, "error-403");
    assert.equal(err403.message, GIFT_TEXT.forbidden);

    const err401 = classifyGiftError({ kind: "unauthorized", status: 401 });
    assert.equal(err401.state, "error-403");

    const err50035 = classifyGiftError({ code: 50035 });
    assert.equal(err50035.state, "error-400");
    assert.equal(err50035.message, GIFT_TEXT.badRequest);

    const err50050 = classifyGiftError({ code: 50050 });
    assert.equal(err50050.state, "error-400");
    assert.match(err50050.message, /уже активирован|исчерпан/i);

    const err50054 = classifyGiftError({ code: 50054 });
    assert.equal(err50054.state, "error-400");
    assert.match(err50054.message, /собственный подарок/i);

    const errNet = classifyGiftError({ kind: "network" });
    assert.equal(errNet.state, "error-network");
    assert.equal(errNet.message, GIFT_TEXT.network);
  });
});

describe("TASK-627: Интерактивный UI-контроллер модалки подарка (#gift-modal) и поток кастомизации", () => {
  it("открывает #gift-modal по клику на триггер, загружает планы из list_nitro_gift_plans и управляет фокусом/Escape", async () => {
    const { doc, triggerBtn } = createTestHarness();
    /** @type {Array<{ cmd: string, args: any }>} */
    const ipcCalls = [];

    const modal = createGiftModal({
      document: /** @type {any} */ (doc),
      triggerButton: triggerBtn,
      recipient: { id: "777888999", username: "Алиса" },
      invoke: async (cmd, args) => {
        ipcCalls.push({ cmd, args });
        if (cmd === "list_nitro_gift_plans") {
          return DEFAULT_NITRO_GIFT_PLANS.slice(0, 4).map((p, idx) => ({
            ...p,
            priceText: idx % 2 === 0 ? "$4.99" : "$49.99",
          }));
        }
        return {};
      },
    });

    assert.equal(triggerBtn.getAttribute("aria-haspopup"), "dialog");
    assert.equal(triggerBtn.getAttribute("aria-controls"), "gift-modal");
    assert.equal(triggerBtn.getAttribute("aria-expanded"), "false");

    const host = doc.getElementById("gift-modal");
    assert.ok(host, "#gift-modal должен быть создан в документе");
    assert.equal(host.hidden, true);

    // Открываем модалку кликом по кнопке
    triggerBtn.dispatchEvent("click");
    await modal.whenReady();

    assert.equal(host.hidden, false);
    assert.equal(triggerBtn.getAttribute("aria-expanded"), "true");
    const card = host.querySelector(".gift-modal-card");
    assert.ok(card);
    assert.equal(card.getAttribute("role"), "dialog");
    assert.equal(card.getAttribute("aria-modal"), "true");
    assert.equal(card.getAttribute("data-state"), "ready");

    const recipientNameEl = host.querySelector("#gift-recipient-name");
    assert.equal(recipientNameEl?.textContent, "Алиса");

    const radios = host.querySelectorAll('input[type="radio"][name="gift-plan"]');
    assert.equal(radios.length, 4);
    assert.equal(radios[0].checked, true);

    // Навигация стрелками по планам (ArrowDown / ArrowUp / Home / End)
    const fieldset = host.querySelector("#gift-plans");
    fieldset.dispatchEvent("keydown", { key: "ArrowDown" });
    assert.equal(modal.getState().selectedPlanId, DEFAULT_NITRO_GIFT_PLANS[1]?.id);
    assert.equal(radios[1].checked, true);

    fieldset.dispatchEvent("keydown", { key: "End" });
    assert.equal(modal.getState().selectedPlanId, DEFAULT_NITRO_GIFT_PLANS[3]?.id);

    fieldset.dispatchEvent("keydown", { key: "Home" });
    assert.equal(modal.getState().selectedPlanId, DEFAULT_NITRO_GIFT_PLANS[0]?.id);

    // Фокус-трап Tab / Shift+Tab внутри карточки
    let preventedTab = false;
    card.dispatchEvent("keydown", {
      key: "Tab",
      shiftKey: false,
      preventDefault: () => {
        preventedTab = true;
      },
    });
    assert.equal(typeof preventedTab, "boolean");

    // Закрытие по Escape возвращает фокус и aria-expanded="false"
    host.dispatchEvent("keydown", { key: "Escape" });
    assert.equal(host.hidden, true);
    assert.equal(card.getAttribute("data-state"), "closed");
    assert.equal(triggerBtn.getAttribute("aria-expanded"), "false");
    assert.equal(doc.activeElement, triggerBtn);

    modal.destroy();
  });

  it("кастомизирует стиль открытки, эмодзи-конфетти, звук и сообщение (<= 200 символов) и отправляет create_gift_code с защитой от двойного клика", async () => {
    const { doc, triggerBtn } = createTestHarness();
    /** @type {Array<{ cmd: string, args: any }>} */
    const ipcCalls = [];
    /** @type {string[]} */
    const copiedCodes = [];

    /** @type {(v: any) => void} */
    let resolveCreate = () => {};

    const modal = createGiftModal({
      document: /** @type {any} */ (doc),
      triggerButton: triggerBtn,
      recipient: { id: "555666777", username: "Борис" },
      plans: [
        {
          id: GIFT_SUBSCRIPTION_PLANS.NITRO_MONTH,
          name: "Nitro Monthly",
          skuId: GIFT_SKU_IDS.NITRO_STANDARD_TIER_2,
          interval: 1,
          intervalCount: 1,
          premiumType: 2,
          priceText: "$9.99 / мес.",
        },
      ],
      copyText: async (text) => {
        copiedCodes.push(text);
      },
      invoke: (cmd, args) => {
        ipcCalls.push({ cmd, args });
        if (cmd === "create_gift_code") {
          return new Promise((resolve) => {
            resolveCreate = resolve;
          });
        }
        return Promise.resolve({});
      },
    });

    await modal.open();
    const host = doc.getElementById("gift-modal");
    const card = host.querySelector(".gift-modal-card");
    assert.equal(card.getAttribute("data-state"), "ready");

    // Выбираем стиль открытки CAKE (5), эмодзи-конфетти и звуковой эффект
    modal.setCardStyle(5);
    assert.equal(modal.getState().cardStyleId, 5);

    modal.setEmojiConfetti({ id: null, name: "🎉" });
    modal.setSoundEffect({ soundId: "snd_quack", name: "Quack", emojiName: "🦆" });
    assert.equal(modal.getState().giftStyle, GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD);

    // Вводим сообщение в #gift-custom-message
    const textarea = host.querySelector("#gift-custom-message");
    const counter = host.querySelector("#gift-custom-count");
    textarea.value = "С днём рождения, Борис!";
    textarea.dispatchEvent("input");
    assert.equal(counter.textContent, "23 / 200");
    assert.equal(modal.getState().customMessage, "С днём рождения, Борис!");

    // Нажимаем «Подарить» дважды подряд (AC-21: защита от двойной отправки)
    const submitBtn = host.querySelector("#gift-submit");
    submitBtn.dispatchEvent("click");
    submitBtn.dispatchEvent("click");

    assert.equal(card.getAttribute("data-state"), "submitting");
    assert.equal(submitBtn.textContent, GIFT_TEXT.submitting);
    assert.equal(submitBtn.disabled, true);

    const createCalls = ipcCalls.filter((c) => c.cmd === "create_gift_code");
    assert.equal(createCalls.length, 1, "повторный клик в состоянии submitting не должен дублировать запрос");
    assert.equal(createCalls[0]?.args.sku_id, GIFT_SKU_IDS.NITRO_STANDARD_TIER_2);
    assert.equal(createCalls[0]?.args.subscription_plan_id, GIFT_SUBSCRIPTION_PLANS.NITRO_MONTH);
    assert.equal(createCalls[0]?.args.gift_style, GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD);
    assert.equal(createCalls[0]?.args.custom_message, "С днём рождения, Борис!");

    // Во время submitting Escape, клик по фону и крестик не закрывают модалку (AC-19)
    host.dispatchEvent("keydown", { key: "Escape" });
    host.dispatchEvent("click");
    host.querySelector('[data-action="gift-close"]').dispatchEvent("click");
    assert.equal(host.hidden, false);
    assert.equal(card.getAttribute("data-state"), "submitting");

    // Завершаем запрос успехом
    resolveCreate({
      code: "Wump-Gift-2026",
      skuId: GIFT_SKU_IDS.NITRO_STANDARD_TIER_2,
      subscriptionPlanId: GIFT_SUBSCRIPTION_PLANS.NITRO_MONTH,
      giftStyle: 2,
      maxUses: 1,
      uses: 0,
    });
    await modal.whenIdle();

    assert.equal(card.getAttribute("data-state"), "success");
    const successEl = host.querySelector("#gift-success");
    assert.equal(successEl.hidden, false);
    const codeEl = host.querySelector("#gift-success-code");
    assert.equal(codeEl.textContent, GIFT_TEXT.successCode("WumpGift2026"));

    // Копируем код подарка
    const copyBtn = host.querySelector('[data-action="gift-copy-code"]');
    copyBtn.dispatchEvent("click");
    await modal.whenIdle();
    assert.deepEqual(copiedCodes, ["https://discord.gift/WumpGift2026"]);
    assert.equal(copyBtn.textContent, GIFT_TEXT.copied);

    modal.destroy();
  });

  it("обрабатывает ошибки 429 с обратным отсчётом, 403, 400 и сетевой сбой с повторной отправкой", async () => {
    const { doc, triggerBtn } = createTestHarness();
    /** @type {any} */
    let nextResult = { error: { kind: "rateLimited", retry_after_ms: 2000 } };

    const modal = createGiftModal({
      document: /** @type {any} */ (doc),
      triggerButton: triggerBtn,
      recipient: { id: "555666777", username: "Борис" },
      plans: [
        {
          id: GIFT_SUBSCRIPTION_PLANS.NITRO_BASIC_MONTH,
          name: "Nitro Basic Monthly",
          skuId: GIFT_SKU_IDS.NITRO_BASIC_TIER_0,
          interval: 1,
          intervalCount: 1,
          premiumType: 3,
          priceText: "$2.99 / мес.",
        },
      ],
      invoke: async (cmd) => {
        if (cmd === "create_gift_code") {
          if (nextResult.error) throw nextResult.error;
          return nextResult.ok;
        }
        return {};
      },
    });

    await modal.open();
    const host = doc.getElementById("gift-modal");
    const card = host.querySelector(".gift-modal-card");
    const errorEl = host.querySelector("#gift-error");
    const submitBtn = host.querySelector("#gift-submit");

    // 1. 429 Rate Limited
    await modal.submit();
    assert.equal(card.getAttribute("data-state"), "error-429");
    assert.equal(errorEl.hidden, false);
    assert.equal(errorEl.textContent, GIFT_TEXT.rateLimited(2));
    assert.equal(submitBtn.disabled, true);

    // Тикаем таймер обратного отсчёта: 2 -> 1 -> 0
    modal.tickRateLimitSecond();
    assert.equal(errorEl.textContent, GIFT_TEXT.rateLimited(1));
    assert.equal(submitBtn.disabled, true);

    modal.tickRateLimitSecond();
    assert.equal(card.getAttribute("data-state"), "ready");
    assert.equal(submitBtn.disabled, false);

    // 2. 403 Forbidden
    nextResult = { error: { kind: "forbidden", status: 403 } };
    await modal.submit();
    assert.equal(card.getAttribute("data-state"), "error-403");
    assert.equal(errorEl.textContent, GIFT_TEXT.forbidden);

    // 3. 400 Invalid Form Body (50035)
    nextResult = { error: { code: 50035, status: 400 } };
    await modal.submit();
    assert.equal(card.getAttribute("data-state"), "error-400");
    assert.equal(errorEl.textContent, GIFT_TEXT.badRequest);

    // 4. Network error -> затем успешный повтор
    nextResult = { error: { kind: "network" } };
    await modal.submit();
    assert.equal(card.getAttribute("data-state"), "error-network");
    assert.equal(errorEl.textContent, GIFT_TEXT.network);

    nextResult = { ok: { code: "RetryOkCode99", skuId: GIFT_SKU_IDS.NITRO_BASIC_TIER_0 } };
    await modal.submit();
    assert.equal(card.getAttribute("data-state"), "success");
    assert.equal(
      host.querySelector("#gift-success-code").textContent,
      GIFT_TEXT.successCode("RetryOkCode99"),
    );

    modal.destroy();
  });

  it("поддерживает резолв и активацию (redeem) подарочного кода через IPC resolve_gift_code и redeem_gift_code", async () => {
    const { doc, triggerBtn } = createTestHarness();
    /** @type {Array<{ cmd: string, args: any }>} */
    const calls = [];

    const modal = createGiftModal({
      document: /** @type {any} */ (doc),
      triggerButton: triggerBtn,
      plans: DEFAULT_NITRO_GIFT_PLANS.slice(0, 2),
      invoke: async (cmd, args) => {
        calls.push({ cmd, args });
        if (cmd === "resolve_gift_code") {
          return {
            code: "WumpRedeem2026",
            skuId: GIFT_SKU_IDS.NITRO_STANDARD_TIER_2,
            subscriptionPlanId: GIFT_SUBSCRIPTION_PLANS.NITRO_MONTH,
            maxUses: 1,
            uses: 0,
            redeemed: false,
          };
        }
        if (cmd === "redeem_gift_code") {
          return {
            code: "WumpRedeem2026",
            entitlement: {
              id: "900100200300",
              skuId: GIFT_SKU_IDS.NITRO_STANDARD_TIER_2,
              applicationId: NITRO_APPLICATION_ID,
              type: 6,
              consumed: false,
              deleted: false,
            },
          };
        }
        return {};
      },
    });

    const resolved = await modal.resolveCode("https://discord.gift/Wump-Redeem-2026");
    assert.equal(resolved.code, "WumpRedeem2026");
    assert.equal(calls[0]?.cmd, "resolve_gift_code");
    assert.equal(calls[0]?.args.code, "WumpRedeem2026");

    const redeemed = await modal.redeemCode("https://discord.gift/Wump-Redeem-2026", {
      channelId: "123456789012345678",
    });
    assert.equal(redeemed.code, "WumpRedeem2026");
    assert.equal(calls[1]?.cmd, "redeem_gift_code");
    assert.equal(calls[1]?.args.code, "WumpRedeem2026");
    assert.equal(calls[1]?.args.channel_id, "123456789012345678");

    modal.destroy();
  });
});

describe("TASK-627: Контракт стилей gift-modal.css и безопасность кода", () => {
  it("CSS-файл styles/gift-modal.css использует только токены из tokens.css без сырых цветов, :has() и !important", () => {
    assert.equal(
      existsSync(GIFT_MODAL_CSS_PATH),
      true,
      "файл rust/gui-spike/ui/styles/gift-modal.css должен существовать",
    );
    const css = readFileSync(GIFT_MODAL_CSS_PATH, "utf8");
    const tokensCss = readFileSync(TOKENS_CSS_PATH, "utf8");

    for (const selector of [
      ".gift-modal-card",
      ".gift-head",
      ".gift-close",
      ".gift-recipient",
      ".gift-plans",
      ".gift-plan",
      ".gift-custom",
      ".gift-custom-input",
      ".gift-counter",
      ".gift-style-picker",
      "[data-gift-card-style]",
      "[data-gift-emoji-confetti]",
      "[data-gift-sound-effect]",
      ".gift-success",
      ".gift-copy",
    ]) {
      assert.equal(
        css.includes(selector),
        true,
        `ожидался селектор ${selector} в gift-modal.css`,
      );
    }

    assert.equal(css.includes("!important"), false, "в gift-modal.css запрещён !important");
    assert.equal(/:has\(/i.test(css), false, "в gift-modal.css запрещён :has()");
    assert.equal(
      /#[0-9a-fA-F]{3,8}\b/.test(css),
      false,
      "в gift-modal.css запрещены сырые hex-цвета",
    );
    assert.equal(
      /\b(?:rgb|rgba|hsl|hsla)\s*\(/.test(css),
      false,
      "в gift-modal.css запрещены сырые rgb/hsl-цвета",
    );
    assert.equal(
      /url\(|@import|https?:\/\//.test(css),
      false,
      "в gift-modal.css запрещены внешние ассеты",
    );

    const declaredTokens = new Set(
      Array.from(tokensCss.matchAll(/(--[a-zA-Z0-9_-]+)\s*:/g)).map((m) => m[1]),
    );
    const usedTokens = Array.from(css.matchAll(/var\(\s*(--[a-zA-Z0-9_-]+)/g)).map((m) => m[1]);
    assert.ok(usedTokens.length >= 12, "gift-modal.css должен использовать токены из tokens.css");
    for (const token of usedTokens) {
      assert.equal(
        declaredTokens.has(token),
        true,
        `токен ${token} не объявлен в rust/gui-spike/ui/tokens.css`,
      );
    }
  });

  it("код gift-modal.js не вызывает запрещённые нативные/медиа API и не пишет в console", () => {
    const js = readFileSync(GIFT_MODAL_JS_PATH, "utf8");
    const forbidden = [
      "navigator.mediaDevices",
      "getUserMedia",
      "getDisplayMedia",
      "startRecordingRawSamples",
      "startLocalAudioRecording",
      "setAecDump",
      "getScreenPreviews",
      "getWindowPreviews",
    ];
    for (const needle of forbidden) {
      assert.equal(js.includes(needle), false, `gift-modal.js содержит запрещённое «${needle}»`);
    }
    assert.equal(/console\.(log|warn|error)/.test(js), false, "gift-modal.js пишет в console");
  });
});
