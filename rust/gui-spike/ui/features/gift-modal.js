// @ts-check
/**
 * TASK-627 (G2 Frontend): интерактивное модальное окно выбора и кастомизации подарка Nitro
 * (`#gift-modal`, `rust/gui-spike/ui/features/gift-modal.js`, `rust/gui-spike/ui/styles/gift-modal.css`).
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

/**
 * Канонический Snowflake приложения Discord Nitro (`4d2a64cb5df5fc4e_0 @6789126`, модуль `202541`, экспорт `TV`).
 * @type {string}
 */
export const NITRO_APPLICATION_ID = "521842831262875670";

/**
 * SKU идентификаторы подписок Discord Nitro и Server Boost (`4d2a64cb5df5fc4e_0 @6789162..@6789340`, экспорт `pe`).
 * @type {Readonly<{
 *   NONE: string,
 *   NITRO_BASIC_TIER_0: string,
 *   NITRO_CLASSIC_TIER_1: string,
 *   NITRO_STANDARD_TIER_2: string,
 *   GUILD_BOOST: string,
 *   NITRO_LEGACY: string,
 * }>}
 */
export const GIFT_SKU_IDS = Object.freeze({
  NONE: "628379670982688768",
  NITRO_BASIC_TIER_0: "978380684370378762",
  NITRO_CLASSIC_TIER_1: "521846918637420545",
  NITRO_STANDARD_TIER_2: "521847234246082599",
  GUILD_BOOST: "590663762298667008",
  NITRO_LEGACY: "521842865731534868",
});

/**
 * Идентификаторы планов подписки Discord Nitro (`4d2a64cb5df5fc4e_0 @6791125..@6791560`, экспорт `gD`).
 * @type {Readonly<{
 *   NITRO_BASIC_MONTH: string,
 *   NITRO_BASIC_YEAR: string,
 *   NITRO_CLASSIC_MONTH: string,
 *   NITRO_CLASSIC_YEAR: string,
 *   NITRO_MONTH: string,
 *   NITRO_YEAR: string,
 *   NITRO_3_MONTH: string,
 *   NITRO_6_MONTH: string,
 *   GUILD_BOOST_MONTH: string,
 *   GUILD_BOOST_YEAR: string,
 * }>}
 */
export const GIFT_SUBSCRIPTION_PLANS = Object.freeze({
  NITRO_BASIC_MONTH: "978380692553465866",
  NITRO_BASIC_YEAR: "1024422698568122368",
  NITRO_CLASSIC_MONTH: "511651871736201216",
  NITRO_CLASSIC_YEAR: "511651876987469824",
  NITRO_MONTH: "511651880837840896",
  NITRO_YEAR: "511651885459963904",
  NITRO_3_MONTH: "642251038925127690",
  NITRO_6_MONTH: "944037208325619722",
  GUILD_BOOST_MONTH: "590665532894740483",
  GUILD_BOOST_YEAR: "590665538238152709",
});

/**
 * Стандартный стиль подарка без кастомизации (`4d2a64cb5df5fc4e_0 @9711867`, `DEFAULT = 0`).
 * @type {number}
 */
export const GIFT_STYLE_DEFAULT = 0;

/**
 * Кастомный стиль обложки подарка без текстового сообщения/эмодзи/звука (`CUSTOM_STYLE = 1`).
 * @type {number}
 */
export const GIFT_STYLE_CUSTOM_STYLE = 1;

/**
 * Кастомный подарок с персональным сообщением, эмодзи или звуком (`CUSTOM_MESSAGE_EMOJI_SOUNDBOARD = 2`).
 * @type {number}
 */
export const GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD = 2;

/**
 * Максимальная длина кастомного сообщения к подарку в символах (`ATTACHMENTS-SPEC.md` §4.2, §4.4).
 * @type {number}
 */
export const MAX_GIFT_CUSTOM_MESSAGE_CHARS = 200;

/**
 * Минимальная длина валидного подарочного кода после удаления дефисов.
 * @type {number}
 */
export const MIN_GIFT_CODE_CHARS = 2;

/**
 * Максимальная длина валидного подарочного кода после удаления дефисов.
 * @type {number}
 */
export const MAX_GIFT_CODE_CHARS = 64;

/**
 * Максимальное число подарочных кодов, извлекаемых из одного текста сообщения (`4d2a64cb5df5fc4e_0 @9712437`, `.slice(0, 3)`).
 * @type {number}
 */
export const MAX_EXTRACTED_GIFT_CODES = 3;

/**
 * Канонический каталог планов подписки Discord Nitro (`rust/gui-spike/src/gifts.rs` / модуль `202541`).
 * @type {ReadonlyArray<Readonly<{
 *   id: string,
 *   name: string,
 *   skuId: string,
 *   interval: number,
 *   intervalCount: number,
 *   premiumType: number | null,
 * }>>}
 */
export const DEFAULT_NITRO_GIFT_PLANS = Object.freeze([
  Object.freeze({
    id: GIFT_SUBSCRIPTION_PLANS.NITRO_BASIC_MONTH,
    name: "Nitro Basic Monthly",
    skuId: GIFT_SKU_IDS.NITRO_BASIC_TIER_0,
    interval: 1,
    intervalCount: 1,
    premiumType: 3,
  }),
  Object.freeze({
    id: GIFT_SUBSCRIPTION_PLANS.NITRO_BASIC_YEAR,
    name: "Nitro Basic Yearly",
    skuId: GIFT_SKU_IDS.NITRO_BASIC_TIER_0,
    interval: 2,
    intervalCount: 1,
    premiumType: 3,
  }),
  Object.freeze({
    id: GIFT_SUBSCRIPTION_PLANS.NITRO_CLASSIC_MONTH,
    name: "Nitro Classic Monthly",
    skuId: GIFT_SKU_IDS.NITRO_CLASSIC_TIER_1,
    interval: 1,
    intervalCount: 1,
    premiumType: 1,
  }),
  Object.freeze({
    id: GIFT_SUBSCRIPTION_PLANS.NITRO_CLASSIC_YEAR,
    name: "Nitro Classic Yearly",
    skuId: GIFT_SKU_IDS.NITRO_CLASSIC_TIER_1,
    interval: 2,
    intervalCount: 1,
    premiumType: 1,
  }),
  Object.freeze({
    id: GIFT_SUBSCRIPTION_PLANS.NITRO_MONTH,
    name: "Nitro Monthly",
    skuId: GIFT_SKU_IDS.NITRO_STANDARD_TIER_2,
    interval: 1,
    intervalCount: 1,
    premiumType: 2,
  }),
  Object.freeze({
    id: GIFT_SUBSCRIPTION_PLANS.NITRO_YEAR,
    name: "Nitro Yearly",
    skuId: GIFT_SKU_IDS.NITRO_STANDARD_TIER_2,
    interval: 2,
    intervalCount: 1,
    premiumType: 2,
  }),
  Object.freeze({
    id: GIFT_SUBSCRIPTION_PLANS.NITRO_3_MONTH,
    name: "Nitro 3 Months",
    skuId: GIFT_SKU_IDS.NITRO_STANDARD_TIER_2,
    interval: 1,
    intervalCount: 3,
    premiumType: 2,
  }),
  Object.freeze({
    id: GIFT_SUBSCRIPTION_PLANS.NITRO_6_MONTH,
    name: "Nitro 6 Months",
    skuId: GIFT_SKU_IDS.NITRO_STANDARD_TIER_2,
    interval: 1,
    intervalCount: 6,
    premiumType: 2,
  }),
  Object.freeze({
    id: GIFT_SUBSCRIPTION_PLANS.GUILD_BOOST_MONTH,
    name: "Guild Boost Monthly",
    skuId: GIFT_SKU_IDS.GUILD_BOOST,
    interval: 1,
    intervalCount: 1,
    premiumType: null,
  }),
  Object.freeze({
    id: GIFT_SUBSCRIPTION_PLANS.GUILD_BOOST_YEAR,
    name: "Guild Boost Yearly",
    skuId: GIFT_SKU_IDS.GUILD_BOOST,
    interval: 2,
    intervalCount: 1,
    premiumType: null,
  }),
]);

/**
 * Стили подарочных открыток (`4d2a64cb5df5fc4e_0 @6796370`, модуль `202541`, экспорт `t_`).
 * @type {ReadonlyArray<Readonly<{ id: number, key: string, label: string }>>}
 */
export const GIFT_CARD_STYLES = Object.freeze([
  Object.freeze({ id: 1, key: "SNOWGLOBE", label: "Снежный шар" }),
  Object.freeze({ id: 2, key: "BOX", label: "Коробка" }),
  Object.freeze({ id: 3, key: "CUP", label: "Кубок" }),
  Object.freeze({ id: 4, key: "STANDARD_BOX", label: "Классическая коробка" }),
  Object.freeze({ id: 5, key: "CAKE", label: "Торт" }),
  Object.freeze({ id: 6, key: "CHEST", label: "Сундук" }),
  Object.freeze({ id: 7, key: "COFFEE", label: "Кофе" }),
  Object.freeze({ id: 8, key: "SEASONAL_STANDARD_BOX", label: "Сезонная коробка" }),
  Object.freeze({ id: 9, key: "SEASONAL_CAKE", label: "Сезонный торт" }),
  Object.freeze({ id: 10, key: "SEASONAL_CHEST", label: "Сезонный сундук" }),
  Object.freeze({ id: 11, key: "SEASONAL_COFFEE", label: "Сезонный кофе" }),
  Object.freeze({ id: 12, key: "NITROWEEN_STANDARD", label: "Хэллоуин Nitro" }),
]);

/**
 * Источники запуска флоу подарка (`4d2a64cb5df5fc4e_0 @6796605`, модуль `202541`, экспорт `tg`).
 * @type {Readonly<{
 *   DM_CHANNEL: number,
 *   SHOP_PAGE: number,
 *   USER_PROFILE_WISHLIST: number,
 *   DM_CHANNEL_WISHLIST: number,
 *   GUILD_CHANNEL: number,
 *   BADGE_DIRECTORY: number,
 * }>}
 */
export const GIFTING_ORIGINS = Object.freeze({
  DM_CHANNEL: 0,
  SHOP_PAGE: 1,
  USER_PROFILE_WISHLIST: 2,
  DM_CHANNEL_WISHLIST: 3,
  GUILD_CHANNEL: 4,
  BADGE_DIRECTORY: 5,
});

/**
 * Коды ошибок API Discord для флоу подарков (`4d2a64cb5df5fc4e_0 @320207`, `@325314`, модуль `652215`).
 * @type {Readonly<{
 *   UNKNOWN_GIFT_CODE: number,
 *   INVALID_FORM_BODY: number,
 *   INVALID_GIFT_REDEMPTION_EXHAUSTED: number,
 *   INVALID_GIFT_REDEMPTION_OWNED: number,
 *   INVALID_GIFT_SELF_REDEMPTION: number,
 *   INVALID_GIFT_REDEMPTION_PREVIOUSLY_OWNED: number,
 *   INVALID_COUNTRY_CODE: number,
 *   INVALID_GIFT_REDEMPTION_FRAUD_REJECTED: number,
 * }>}
 */
export const DISCORD_GIFT_ERROR_CODES = Object.freeze({
  UNKNOWN_GIFT_CODE: 10038,
  INVALID_FORM_BODY: 50035,
  INVALID_GIFT_REDEMPTION_EXHAUSTED: 50050,
  INVALID_GIFT_REDEMPTION_OWNED: 50051,
  INVALID_GIFT_SELF_REDEMPTION: 50054,
  INVALID_GIFT_REDEMPTION_PREVIOUSLY_OWNED: 50092,
  INVALID_COUNTRY_CODE: 50095,
  INVALID_GIFT_REDEMPTION_FRAUD_REJECTED: 50097,
});

/**
 * Канонический список Tauri IPC-команд подсистемы подарков (`rust/gui-spike/src/gifts.rs`).
 * @type {ReadonlyArray<string>}
 */
export const GIFT_IPC_COMMANDS = Object.freeze([
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

/**
 * Состояния модального окна подарка (`ATTACHMENTS-SPEC.md` §4.3).
 * @type {ReadonlyArray<string>}
 */
export const GIFT_MODAL_STATES = Object.freeze([
  "closed",
  "loading",
  "ready",
  "submitting",
  "success",
  "error-429",
  "error-403",
  "error-400",
  "error-network",
]);

/**
 * Русские строки локализации модального окна подарка (`ATTACHMENTS-SPEC.md` §4.2–§4.3 и `ru-catalog.json`).
 */
export const GIFT_TEXT = Object.freeze({
  title: "Подарить Discord Nitro",
  closeLabel: "Закрыть окно подарка",
  recipientPrefix: "Получатель: ",
  recipientAll: "Создать подарочную ссылку",
  plansLegend: "Выберите план",
  customStyleLegend: "Оформление открытки",
  customMessageLabel: "Персональное сообщение (необязательно)",
  customMessagePlaceholder: "Добавьте поздравление к подарку…",
  emojiConfettiButton: "Конфетти из эмодзи",
  soundEffectButton: "Звуковой эффект",
  cancel: "Отмена",
  submit: "Подарить",
  submitting: "Создание подарка…",
  copyLink: "Скопировать ссылку",
  copied: "Скопировано",
  emptyCatalog: "Планы подписки сейчас недоступны",
  forbidden: "Недостаточно прав или требуется подтверждение оплаты (403).",
  badRequest: "Не удалось создать подарок: проверьте выбранный план и сообщение.",
  network: "Сетевая ошибка при создании подарка. Проверьте подключение.",
  exhausted: "Этот подарочный код уже активирован или исчерпан.",
  alreadyOwned: "У вас уже есть этот предмет или активная подписка.",
  selfRedemption: "Вы не можете получить собственный подарок.",
  previouslyOwned: "Этот подарок недоступен: подписка уже оформлялась ранее.",
  invalidCountry: "Подарок недоступен в вашем регионе.",
  fraudRejected: "Не удалось активировать подарок: платёж отклонён проверкой безопасности.",
  unknownGift: "Подарочный код не найден или истёк.",
  /**
   * @param {number} seconds
   * @returns {string}
   */
  rateLimited: (seconds) => `Слишком много запросов. Повторите через ${seconds} с.`,
  /**
   * @param {string} code
   * @returns {string}
   */
  successCode: (code) => `Подарок создан: https://discord.gift/${code}`,
});

const MAX_U64_SNOWFLAKE = 18446744073709551615n;

/**
 * Проверяет, что строка является валидным Discord Snowflake (`1..=u64::MAX`, без ведущего нуля).
 * @param {unknown} raw
 * @returns {boolean}
 */
export function isValidSnowflake(raw) {
  if (typeof raw !== "string") return false;
  const trimmed = raw.trim();
  if (trimmed.length === 0 || trimmed.length > 20) return false;
  if (trimmed.startsWith("0")) return false;
  if (!/^[1-9][0-9]*$/.test(trimmed)) return false;
  try {
    const val = BigInt(trimmed);
    return val >= 1n && val <= MAX_U64_SNOWFLAKE;
  } catch {
    return false;
  }
}

/**
 * Проверяет получателя подарка: валидность Snowflake и запрет подарка самому себе
 * (`3e8824f7eca8914f_0 @200300`: `d.id === A?.id`).
 * @param {{ id?: string | null, username?: string | null } | null | undefined} recipient
 * @param {string | null | undefined} [currentUserId]
 * @returns {{ valid: boolean, reason: "ok" | "self" | "invalid_snowflake" }}
 */
export function validateGiftRecipient(recipient, currentUserId) {
  if (!recipient || recipient.id == null || recipient.id === "") {
    return { valid: true, reason: "ok" };
  }
  const id = String(recipient.id).trim();
  if (!isValidSnowflake(id)) {
    return { valid: false, reason: "invalid_snowflake" };
  }
  if (currentUserId && String(currentUserId).trim() === id) {
    return { valid: false, reason: "self" };
  }
  return { valid: true, reason: "ok" };
}

/**
 * Подсчитывает количество Unicode-кодовых точек в строке (эквивалент `.chars().count()` в Rust).
 * @param {string} text
 * @returns {number}
 */
function countCodePoints(text) {
  return Array.from(text).length;
}

/**
 * Валидирует длину кастомного сообщения к подарку (`<= 200` символов, `ATTACHMENTS-SPEC.md` §4.2, §4.4).
 * @param {string | null | undefined} customMessage
 * @returns {{ valid: boolean, charCount: number, maxChars: number, trimmed: string }}
 */
export function validateGiftCustomMessage(customMessage) {
  const raw = typeof customMessage === "string" ? customMessage : "";
  const charCount = countCodePoints(raw);
  return {
    valid: charCount <= MAX_GIFT_CUSTOM_MESSAGE_CHARS,
    charCount,
    maxChars: MAX_GIFT_CUSTOM_MESSAGE_CHARS,
    trimmed: raw.trim(),
  };
}

/**
 * Вычисляет `gift_style` (`1` или `2`) по наличию непустого сообщения, эмодзи-конфетти или звукового эффекта
 * (`4d2a64cb5df5fc4e_0 @9711867` и `rust/gui-spike/src/gifts.rs::resolve_gift_style`).
 * Выбрасывает ошибку, если длина сообщения превышает `200` символов.
 *
 * @param {string | null | undefined} customMessage
 * @param {{
 *   hasEmojiOrSound?: boolean,
 *   emojiConfetti?: { id?: string | null, name?: string | null } | null,
 *   soundEffect?: { soundId?: string | null, name?: string | null } | null,
 * }} [options]
 * @returns {number}
 */
export function resolveGiftStyle(customMessage, options = {}) {
  const check = validateGiftCustomMessage(customMessage);
  if (!check.valid) {
    throw new RangeError(
      `Длина сообщения подарка (${check.charCount}) превышает максимум ${MAX_GIFT_CUSTOM_MESSAGE_CHARS} символов`,
    );
  }
  const hasMessage = check.trimmed.length > 0;
  const hasEmoji = Boolean(options?.emojiConfetti && (options.emojiConfetti.name || options.emojiConfetti.id));
  const hasSound = Boolean(options?.soundEffect && (options.soundEffect.soundId || options.soundEffect.name));
  const hasExtras = Boolean(options?.hasEmojiOrSound) || hasEmoji || hasSound;
  return hasMessage || hasExtras
    ? GIFT_STYLE_CUSTOM_MESSAGE_EMOJI_SOUNDBOARD
    : GIFT_STYLE_CUSTOM_STYLE;
}

/**
 * Очищает и валидирует подарочный код (`4d2a64cb5df5fc4e_0 @9712505`, экспорт `e6`):
 * извлекает из ссылки при необходимости, удаляет дефисы `-` и проверяет `^[A-Za-z0-9_]{2,64}$`.
 * @param {unknown} raw
 * @returns {string | null}
 */
export function normalizeGiftCode(raw) {
  if (typeof raw !== "string") return null;
  const trimmed = raw.trim();
  if (!trimmed) return null;

  let candidate = trimmed;
  if (/^https?:\/\//i.test(trimmed) || trimmed.includes("discord.gift/") || trimmed.includes("/gifts/")) {
    const extracted = extractSingleCodeFromUrlLike(trimmed);
    if (!extracted) return null;
    candidate = extracted;
  }

  const normalized = candidate.replace(/-/g, "");
  if (
    normalized.length < MIN_GIFT_CODE_CHARS ||
    normalized.length > MAX_GIFT_CODE_CHARS ||
    !/^[A-Za-z0-9_]+$/.test(normalized)
  ) {
    return null;
  }
  return normalized;
}

/**
 * Извлекает один сырой код из URL-подобной строки (`discord.gift/<code>`, `discord.com/gifts/<code>`, `promos.discord.gg/?code=<code>`).
 * @param {string} input
 * @returns {string | null}
 */
function extractSingleCodeFromUrlLike(input) {
  const promoIdx = input.toLowerCase().indexOf("promos.discord.gg");
  if (promoIdx !== -1) {
    const qIdx = input.indexOf("?", promoIdx);
    if (qIdx !== -1) {
      const queryPart = input.slice(qIdx + 1).split(/[\s#"'<>]/)[0] || "";
      for (const pair of queryPart.split("&")) {
        const [k, v] = pair.split("=");
        if (k === "code" && v) {
          return v;
        }
      }
    }
  }

  const prefixes = [
    "discord.gift/",
    "discord.com/gifts/",
    "discordapp.com/gifts/",
  ];
  const lower = input.toLowerCase();
  for (const prefix of prefixes) {
    const idx = lower.indexOf(prefix);
    if (idx !== -1) {
      const start = idx + prefix.length;
      const tail = input.slice(start);
      const token = tail.split(/[\s/?#&"'<>.,;!()[\]{}]/)[0];
      if (token) return token;
    }
  }
  return null;
}

/**
 * Извлекает до 3 уникальных подарочных кодов из произвольного текста сообщения
 * (`4d2a64cb5df5fc4e_0 @9712358`, модуль `45938`, экспорт `pF`).
 * @param {string | null | undefined} content
 * @returns {string[]}
 */
export function extractGiftCodesFromText(content) {
  if (typeof content !== "string" || content.length === 0) return [];
  /** @type {string[]} */
  const codes = [];
  const regex =
    /(?:https?:\/\/)?(?:(?:ptb\.|canary\.)?(?:discord(?:app)?\.com\/gifts|discord\.gift)\/([A-Za-z0-9_-]+)|promos\.discord\.gg\/[^\s"'<>]*?[?&]code=([A-Za-z0-9_-]+))/gi;

  let match;
  while ((match = regex.exec(content)) !== null) {
    const rawCode = match[1] || match[2] || "";
    const normalized = normalizeGiftCode(rawCode);
    if (normalized && !codes.includes(normalized)) {
      codes.push(normalized);
      if (codes.length >= MAX_EXTRACTED_GIFT_CODES) {
        break;
      }
    }
  }
  return codes;
}

/**
 * Форматирует каноническую подарочную ссылку `https://discord.gift/<code>` (`4d2a64cb5df5fc4e_0 @9711953`, `t_`).
 * @param {string | null | undefined} code
 * @returns {string}
 */
export function formatGiftUrl(code) {
  const normalized = normalizeGiftCode(code);
  if (!normalized) return "";
  return `https://discord.gift/${normalized}`;
}

/**
 * Форматирует цену плана подписки. Если цена не пришла с бэкенда, возвращает пустую строку
 * (`ATTACHMENTS-SPEC.md` §4.4: запрет хардкода цен на клиенте).
 * @param {{ id?: string, name?: string, priceText?: string | null, amount?: number | null, currency?: string | null, [key: string]: any } | null | undefined} plan
 * @returns {string}
 */
export function formatGiftPlanPrice(plan) {
  if (!plan) return "";
  if (typeof plan.priceText === "string" && plan.priceText.trim().length > 0) {
    return plan.priceText.trim();
  }
  if (typeof plan.amount === "number" && Number.isFinite(plan.amount) && plan.amount >= 0) {
    const currency = typeof plan.currency === "string" && plan.currency.trim() ? plan.currency.trim().toUpperCase() : "USD";
    return `${(plan.amount / 100).toFixed(2)} ${currency}`;
  }
  return "";
}

/**
 * Форматирует локализованное русское название плана подписки Nitro.
 * @param {{
 *   id?: string,
 *   name?: string,
 *   skuId?: string,
 *   interval?: number,
 *   intervalCount?: number,
 *   premiumType?: number | null,
 * } | null | undefined} plan
 * @returns {string}
 */
export function formatGiftPlanLabelRu(plan) {
  if (!plan) return "Discord Nitro";
  let tierName = "Nitro";
  if (plan.premiumType === 3 || plan.skuId === GIFT_SKU_IDS.NITRO_BASIC_TIER_0 || /basic/i.test(plan.name || "")) {
    tierName = "Nitro Basic";
  } else if (plan.premiumType === 1 || plan.skuId === GIFT_SKU_IDS.NITRO_CLASSIC_TIER_1 || /classic/i.test(plan.name || "")) {
    tierName = "Nitro Classic";
  } else if (plan.skuId === GIFT_SKU_IDS.GUILD_BOOST || /boost/i.test(plan.name || "")) {
    tierName = "Буст сервера";
  }

  const count = typeof plan.intervalCount === "number" && plan.intervalCount > 0 ? plan.intervalCount : 1;
  if (plan.interval === 2) {
    return `${tierName} — ${count} ${count === 1 ? "год" : "г."}`;
  }
  if (plan.interval === 1) {
    return `${tierName} — ${count} мес.`;
  }
  return plan.name || tierName;
}

/**
 * Собирает IPC/REST payload для `create_gift_code` (`POST /api/v9/users/@me/entitlements/gift-codes`).
 * @param {{
 *   plan: { id: string, skuId: string },
 *   customMessage?: string | null,
 *   cardStyleId?: number | null,
 *   emojiConfetti?: { id?: string | null, name?: string | null } | null,
 *   soundEffect?: { soundId?: string | null, name?: string | null } | null,
 *   recipient?: { id?: string | null, username?: string | null } | null,
 *   currentUserId?: string | null,
 * }} params
 */
export function buildCreateGiftPayload(params) {
  if (!params || !params.plan || !isValidSnowflake(params.plan.id) || !isValidSnowflake(params.plan.skuId)) {
    throw new Error("Некорректный план подписки для создания подарка");
  }
  const recipientCheck = validateGiftRecipient(params.recipient, params.currentUserId);
  if (!recipientCheck.valid) {
    throw new Error(
      recipientCheck.reason === "self"
        ? GIFT_TEXT.selfRedemption
        : "Некорректный идентификатор получателя подарка",
    );
  }
  const msgCheck = validateGiftCustomMessage(params.customMessage);
  if (!msgCheck.valid) {
    throw new RangeError(GIFT_TEXT.badRequest);
  }
  const giftStyle = resolveGiftStyle(params.customMessage, {
    emojiConfetti: params.emojiConfetti,
    soundEffect: params.soundEffect,
  });
  const trimmedMsg = msgCheck.trimmed.length > 0 ? msgCheck.trimmed : null;

  return {
    sku_id: params.plan.skuId,
    skuId: params.plan.skuId,
    subscription_plan_id: params.plan.id,
    subscriptionPlanId: params.plan.id,
    gift_style: giftStyle,
    giftStyle,
    custom_message: trimmedMsg,
    customMessage: trimmedMsg,
    card_style_id: typeof params.cardStyleId === "number" ? params.cardStyleId : 1,
    recipient_id: params.recipient?.id && isValidSnowflake(params.recipient.id) ? params.recipient.id : null,
    emoji_id: params.emojiConfetti?.id || null,
    emoji_name: params.emojiConfetti?.name || null,
    sound_id: params.soundEffect?.soundId || null,
  };
}

/**
 * Классифицирует ошибку IPC / REST в целевое состояние `#gift-modal` и русское сообщение (`ATTACHMENTS-SPEC.md` §4.3).
 * @param {any} err
 * @returns {{
 *   state: "error-429" | "error-403" | "error-400" | "error-network",
 *   message: string,
 *   retryAfterSeconds: number,
 *   discordCode: number | null,
 * }}
 */
export function classifyGiftError(err) {
  const raw = err && typeof err === "object" ? err : { message: String(err || "") };
  const status = typeof raw.status === "number" ? raw.status : null;
  const kind = typeof raw.kind === "string" ? raw.kind : "";
  const discordCode = typeof raw.code === "number" ? raw.code : null;

  if (kind === "rateLimited" || status === 429) {
    const retryMs =
      typeof raw.retry_after_ms === "number"
        ? raw.retry_after_ms
        : typeof raw.retryAfterMs === "number"
          ? raw.retryAfterMs
          : 5000;
    const retryAfterSeconds = Math.max(1, Math.ceil(retryMs / 1000));
    return {
      state: "error-429",
      message: GIFT_TEXT.rateLimited(retryAfterSeconds),
      retryAfterSeconds,
      discordCode,
    };
  }

  if (kind === "forbidden" || kind === "unauthorized" || status === 403 || status === 401) {
    return {
      state: "error-403",
      message: GIFT_TEXT.forbidden,
      retryAfterSeconds: 0,
      discordCode,
    };
  }

  if (discordCode === DISCORD_GIFT_ERROR_CODES.INVALID_GIFT_REDEMPTION_EXHAUSTED) {
    return {
      state: "error-400",
      message: GIFT_TEXT.exhausted,
      retryAfterSeconds: 0,
      discordCode,
    };
  }
  if (discordCode === DISCORD_GIFT_ERROR_CODES.INVALID_GIFT_REDEMPTION_OWNED) {
    return {
      state: "error-400",
      message: GIFT_TEXT.alreadyOwned,
      retryAfterSeconds: 0,
      discordCode,
    };
  }
  if (discordCode === DISCORD_GIFT_ERROR_CODES.INVALID_GIFT_SELF_REDEMPTION) {
    return {
      state: "error-400",
      message: GIFT_TEXT.selfRedemption,
      retryAfterSeconds: 0,
      discordCode,
    };
  }
  if (discordCode === DISCORD_GIFT_ERROR_CODES.INVALID_GIFT_REDEMPTION_PREVIOUSLY_OWNED) {
    return {
      state: "error-400",
      message: GIFT_TEXT.previouslyOwned,
      retryAfterSeconds: 0,
      discordCode,
    };
  }
  if (discordCode === DISCORD_GIFT_ERROR_CODES.INVALID_COUNTRY_CODE) {
    return {
      state: "error-400",
      message: GIFT_TEXT.invalidCountry,
      retryAfterSeconds: 0,
      discordCode,
    };
  }
  if (discordCode === DISCORD_GIFT_ERROR_CODES.INVALID_GIFT_REDEMPTION_FRAUD_REJECTED) {
    return {
      state: "error-400",
      message: GIFT_TEXT.fraudRejected,
      retryAfterSeconds: 0,
      discordCode,
    };
  }
  if (discordCode === DISCORD_GIFT_ERROR_CODES.UNKNOWN_GIFT_CODE || status === 404 || kind === "notFound") {
    return {
      state: "error-400",
      message: GIFT_TEXT.unknownGift,
      retryAfterSeconds: 0,
      discordCode,
    };
  }
  if (
    discordCode === DISCORD_GIFT_ERROR_CODES.INVALID_FORM_BODY ||
    kind === "badRequest" ||
    status === 400
  ) {
    return {
      state: "error-400",
      message: GIFT_TEXT.badRequest,
      retryAfterSeconds: 0,
      discordCode,
    };
  }

  return {
    state: "error-network",
    message: GIFT_TEXT.network,
    retryAfterSeconds: 0,
    discordCode,
  };
}

/**
 * Создаёт и монтирует интерактивный контроллер модального окна выбора и кастомизации подарка (`#gift-modal`).
 *
 * @param {{
 *   document?: Document | any,
 *   container?: any,
 *   triggerButton?: any,
 *   recipient?: { id?: string | null, username?: string | null } | null,
 *   currentUserId?: string | null,
 *   plans?: ReadonlyArray<any>,
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   copyText?: (text: string) => Promise<void> | void,
 *   onGiftCreated?: (gift: any) => void,
 * }} [options]
 */
export function createGiftModal(options = {}) {
  const doc =
    options.document || (typeof globalThis.document !== "undefined" ? globalThis.document : null);
  if (!doc) {
    throw new Error("createGiftModal требует объект document");
  }

  const rootParent = options.container || doc.body;
  const triggerBtn =
    options.triggerButton || doc.querySelector?.('[data-action="composer-gift"]') || null;

  /** @type {(cmd: string, args?: any) => Promise<any>} */
  const invoke =
    typeof options.invoke === "function"
      ? options.invoke
      : async (cmd, args) => {
          const tauriInvoke =
            /** @type {any} */ (globalThis).__TAURI__?.core?.invoke ||
            /** @type {any} */ (globalThis).__TAURI_INTERNALS__?.invoke;
          if (typeof tauriInvoke === "function") {
            return tauriInvoke(cmd, args);
          }
          if (cmd === "list_nitro_gift_plans") {
            return [...DEFAULT_NITRO_GIFT_PLANS];
          }
          throw { kind: "network", message: "Tauri IPC недоступен" };
        };

  /** @type {(text: string) => Promise<void>} */
  const copyText =
    typeof options.copyText === "function"
      ? async (text) => {
          await options.copyText?.(text);
        }
      : async (text) => {
          const nav = /** @type {any} */ (globalThis).navigator;
          if (nav?.clipboard?.writeText) {
            await nav.clipboard.writeText(text);
          }
        };

  // Внутреннее состояние контроллера
  let modalState = "closed";
  /** @type {Array<any>} */
  let availablePlans = Array.isArray(options.plans) ? [...options.plans] : [];
  let selectedPlanId = availablePlans[0]?.id || "";
  let cardStyleId = GIFT_CARD_STYLES[0]?.id ?? 1;
  /** @type {{ id?: string | null, name?: string | null } | null} */
  let emojiConfetti = null;
  /** @type {{ soundId?: string | null, name?: string | null, emojiName?: string | null } | null} */
  let soundEffect = null;
  let customMessage = "";
  let createdGiftCode = "";
  let retryAfterSeconds = 0;
  /** @type {{ id?: string | null, username?: string | null } | null} */
  let recipient = options.recipient || null;
  /** @type {Promise<void>} */
  let pendingPromise = Promise.resolve();
  /** @type {any} */
  let lastTriggerEl = triggerBtn;

  if (triggerBtn) {
    triggerBtn.setAttribute("aria-haspopup", "dialog");
    triggerBtn.setAttribute("aria-controls", "gift-modal");
    triggerBtn.setAttribute("aria-expanded", "false");
  }

  // Строим DOM-дерево #gift-modal по контракту ATTACHMENTS-SPEC.md §4.2
  let host = doc.getElementById("gift-modal");
  if (!host) {
    host = doc.createElement("div");
    host.id = "gift-modal";
    host.className = "modal-backdrop";
    host.hidden = true;
    rootParent.appendChild(host);
  }

  const card = doc.createElement("div");
  card.className = "modal-card gift-modal-card";
  card.setAttribute("role", "dialog");
  card.setAttribute("aria-modal", "true");
  card.setAttribute("aria-labelledby", "gift-modal-title");
  card.setAttribute("aria-describedby", "gift-recipient-label");
  card.setAttribute("data-state", "closed");

  // Шапка
  const head = doc.createElement("header");
  head.className = "modal-head gift-head";
  const titleEl = doc.createElement("h2");
  titleEl.id = "gift-modal-title";
  titleEl.textContent = GIFT_TEXT.title;
  const closeBtn = doc.createElement("button");
  closeBtn.type = "button";
  closeBtn.setAttribute("type", "button");
  closeBtn.className = "btn-icon gift-close";
  closeBtn.setAttribute("data-action", "gift-close");
  closeBtn.setAttribute("aria-label", GIFT_TEXT.closeLabel);
  closeBtn.textContent = "✕";
  head.appendChild(titleEl);
  head.appendChild(closeBtn);
  card.appendChild(head);

  // Получатель
  const recipientP = doc.createElement("p");
  recipientP.id = "gift-recipient-label";
  recipientP.className = "gift-recipient";
  const recipientPrefixSpan = doc.createElement("span");
  recipientPrefixSpan.textContent = GIFT_TEXT.recipientPrefix;
  const recipientNameStrong = doc.createElement("strong");
  recipientNameStrong.id = "gift-recipient-name";
  recipientP.appendChild(recipientPrefixSpan);
  recipientP.appendChild(recipientNameStrong);
  card.appendChild(recipientP);

  // Скелетон загрузки
  const skeletonWrap = doc.createElement("div");
  skeletonWrap.className = "gift-skeleton-list";
  skeletonWrap.hidden = true;
  for (let i = 0; i < 2; i += 1) {
    const sk = doc.createElement("div");
    sk.className = "gift-plan-skeleton";
    skeletonWrap.appendChild(sk);
  }
  card.appendChild(skeletonWrap);

  // Список планов
  const plansFieldset = doc.createElement("fieldset");
  plansFieldset.id = "gift-plans";
  plansFieldset.className = "gift-plans";
  plansFieldset.setAttribute("role", "radiogroup");
  plansFieldset.setAttribute("aria-label", "План подписки Nitro");
  const plansLegend = doc.createElement("legend");
  plansLegend.textContent = GIFT_TEXT.plansLegend;
  plansFieldset.appendChild(plansLegend);
  const plansList = doc.createElement("div");
  plansList.className = "gift-plans-items";
  plansFieldset.appendChild(plansList);
  card.appendChild(plansFieldset);

  // Пикер оформления открытки, эмодзи-конфетти и звука (3e8824f7eca8914f_0 модули 968060, 274076, 650588)
  const stylePickerDiv = doc.createElement("div");
  stylePickerDiv.className = "gift-style-picker";
  const styleLabel = doc.createElement("span");
  styleLabel.className = "gift-style-picker-label";
  styleLabel.textContent = GIFT_TEXT.customStyleLegend;
  stylePickerDiv.appendChild(styleLabel);

  const styleGrid = doc.createElement("div");
  styleGrid.className = "gift-style-grid";
  /** @type {Map<number, any>} */
  const styleButtons = new Map();
  for (const styleDef of GIFT_CARD_STYLES.slice(0, 6)) {
    const sBtn = doc.createElement("button");
    sBtn.type = "button";
    sBtn.setAttribute("type", "button");
    sBtn.setAttribute("data-gift-card-style", String(styleDef.id));
    sBtn.setAttribute("aria-pressed", styleDef.id === cardStyleId ? "true" : "false");
    sBtn.textContent = styleDef.label;
    sBtn.addEventListener("click", () => {
      setCardStyle(styleDef.id);
    });
    styleButtons.set(styleDef.id, sBtn);
    styleGrid.appendChild(sBtn);
  }
  stylePickerDiv.appendChild(styleGrid);

  const extrasRow = doc.createElement("div");
  extrasRow.className = "gift-extras-row";
  const emojiBtn = doc.createElement("button");
  emojiBtn.type = "button";
  emojiBtn.setAttribute("type", "button");
  emojiBtn.setAttribute("data-gift-emoji-confetti", "🎉");
  emojiBtn.setAttribute("aria-pressed", "false");
  emojiBtn.textContent = GIFT_TEXT.emojiConfettiButton;
  emojiBtn.addEventListener("click", () => {
    setEmojiConfetti(emojiConfetti ? null : { id: null, name: "🎉" });
  });

  const soundBtn = doc.createElement("button");
  soundBtn.type = "button";
  soundBtn.setAttribute("type", "button");
  soundBtn.setAttribute("data-gift-sound-effect", "snd_airhorn");
  soundBtn.setAttribute("aria-pressed", "false");
  soundBtn.textContent = GIFT_TEXT.soundEffectButton;
  soundBtn.addEventListener("click", () => {
    setSoundEffect(soundEffect ? null : { soundId: "snd_airhorn", name: "Airhorn" });
  });

  extrasRow.appendChild(emojiBtn);
  extrasRow.appendChild(soundBtn);
  stylePickerDiv.appendChild(extrasRow);
  card.appendChild(stylePickerDiv);

  // Поле кастомного сообщения (<= 200 символов)
  const customLabel = doc.createElement("label");
  customLabel.className = "gift-custom";
  customLabel.setAttribute("for", "gift-custom-message");
  const customTitleSpan = doc.createElement("span");
  customTitleSpan.textContent = GIFT_TEXT.customMessageLabel;
  const customTextarea = doc.createElement("textarea");
  customTextarea.id = "gift-custom-message";
  customTextarea.className = "gift-custom-input";
  customTextarea.rows = 3;
  customTextarea.maxLength = MAX_GIFT_CUSTOM_MESSAGE_CHARS;
  customTextarea.setAttribute("maxlength", String(MAX_GIFT_CUSTOM_MESSAGE_CHARS));
  customTextarea.setAttribute("placeholder", GIFT_TEXT.customMessagePlaceholder);
  const customCounter = doc.createElement("span");
  customCounter.id = "gift-custom-count";
  customCounter.className = "gift-counter";
  customCounter.setAttribute("aria-live", "polite");
  customCounter.textContent = `0 / ${MAX_GIFT_CUSTOM_MESSAGE_CHARS}`;
  customLabel.appendChild(customTitleSpan);
  customLabel.appendChild(customTextarea);
  customLabel.appendChild(customCounter);
  card.appendChild(customLabel);

  // Блок ошибки
  const errorP = doc.createElement("p");
  errorP.id = "gift-error";
  errorP.className = "form-error";
  errorP.setAttribute("role", "alert");
  errorP.setAttribute("aria-live", "assertive");
  errorP.hidden = true;
  card.appendChild(errorP);

  // Блок успешного создания кода
  const successSection = doc.createElement("section");
  successSection.id = "gift-success";
  successSection.className = "gift-success";
  successSection.setAttribute("aria-live", "polite");
  successSection.hidden = true;
  const successCodeP = doc.createElement("p");
  successCodeP.id = "gift-success-code";
  successCodeP.className = "gift-code";
  const copyBtn = doc.createElement("button");
  copyBtn.type = "button";
  copyBtn.setAttribute("type", "button");
  copyBtn.className = "btn-secondary gift-copy";
  copyBtn.setAttribute("data-action", "gift-copy-code");
  copyBtn.textContent = GIFT_TEXT.copyLink;
  successSection.appendChild(successCodeP);
  successSection.appendChild(copyBtn);
  card.appendChild(successSection);

  // Футер
  const foot = doc.createElement("footer");
  foot.className = "modal-foot gift-foot";
  const cancelBtn = doc.createElement("button");
  cancelBtn.type = "button";
  cancelBtn.setAttribute("type", "button");
  cancelBtn.className = "btn-secondary";
  cancelBtn.setAttribute("data-action", "gift-cancel");
  cancelBtn.textContent = GIFT_TEXT.cancel;
  const submitBtn = doc.createElement("button");
  submitBtn.type = "button";
  submitBtn.setAttribute("type", "button");
  submitBtn.id = "gift-submit";
  submitBtn.className = "btn-primary";
  submitBtn.setAttribute("data-action", "gift-submit");
  submitBtn.textContent = GIFT_TEXT.submit;
  foot.appendChild(cancelBtn);
  foot.appendChild(submitBtn);
  card.appendChild(foot);

  host.replaceChildren(card);

  /**
   * Синхронизирует отображение имени получателя.
   */
  function syncRecipientLabel() {
    if (recipient && recipient.username) {
      recipientPrefixSpan.textContent = GIFT_TEXT.recipientPrefix;
      recipientNameStrong.textContent = recipient.username;
    } else {
      recipientPrefixSpan.textContent = "";
      recipientNameStrong.textContent = GIFT_TEXT.recipientAll;
    }
  }

  /**
   * Перерисовывает список радиокнопок планов подписки.
   */
  function renderPlanRadios() {
    /** @type {any[]} */
    const nodes = [];
    for (const plan of availablePlans) {
      const label = doc.createElement("label");
      label.className = "gift-plan";
      const radio = doc.createElement("input");
      radio.type = "radio";
      radio.name = "gift-plan";
      radio.setAttribute("type", "radio");
      radio.setAttribute("name", "gift-plan");
      radio.value = plan.id;
      radio.setAttribute("value", plan.id);
      radio.setAttribute("data-sku-id", plan.skuId || "");
      const isChecked = plan.id === selectedPlanId;
      radio.checked = isChecked;
      if (isChecked) {
        radio.setAttribute("checked", "checked");
      }
      radio.addEventListener("change", () => {
        selectPlan(plan.id);
      });

      const nameSpan = doc.createElement("span");
      nameSpan.className = "gift-plan-name";
      nameSpan.textContent = formatGiftPlanLabelRu(plan);

      const priceSpan = doc.createElement("span");
      priceSpan.className = "gift-plan-price";
      priceSpan.textContent = formatGiftPlanPrice(plan);

      label.appendChild(radio);
      label.appendChild(nameSpan);
      label.appendChild(priceSpan);
      nodes.push(label);
    }
    plansList.replaceChildren(...nodes);
  }

  /**
   * Переключает выбранный план по идентификатору.
   * @param {string} planId
   */
  function selectPlan(planId) {
    const exists = availablePlans.some((p) => p.id === planId);
    if (!exists) return;
    selectedPlanId = planId;
    const radios = plansFieldset.querySelectorAll('input[type="radio"][name="gift-plan"]');
    for (const r of radios) {
      const checked = r.value === planId || r.getAttribute("value") === planId;
      r.checked = checked;
      if (checked) {
        r.setAttribute("checked", "checked");
      } else {
        r.removeAttribute?.("checked");
      }
    }
    syncControlsByState();
  }

  /**
   * Синхронизирует состояние атрибутов, видимость блоков и доступность кнопок.
   */
  function syncControlsByState() {
    card.setAttribute("data-state", modalState);
    const isSubmitting = modalState === "submitting";
    const isLoading = modalState === "loading";
    const isRateLimited = modalState === "error-429";
    const hasPlans = availablePlans.length > 0 && Boolean(selectedPlanId);
    const msgValid = validateGiftCustomMessage(customMessage).valid;
    const recipientValid = validateGiftRecipient(recipient, options.currentUserId).valid;

    skeletonWrap.hidden = !isLoading;
    plansFieldset.hidden = isLoading;

    submitBtn.textContent = isSubmitting ? GIFT_TEXT.submitting : GIFT_TEXT.submit;
    submitBtn.disabled =
      isLoading ||
      isSubmitting ||
      isRateLimited ||
      !hasPlans ||
      !msgValid ||
      !recipientValid;
    if (submitBtn.disabled) {
      submitBtn.setAttribute("disabled", "disabled");
    } else {
      submitBtn.removeAttribute?.("disabled");
    }
  }

  /**
   * Устанавливает стиль подарочной открытки (`1..12`).
   * @param {number} styleId
   */
  function setCardStyle(styleId) {
    if (!GIFT_CARD_STYLES.some((s) => s.id === styleId)) return;
    cardStyleId = styleId;
    for (const [id, btn] of styleButtons.entries()) {
      btn.setAttribute("aria-pressed", id === cardStyleId ? "true" : "false");
    }
  }

  /**
   * Устанавливает или сбрасывает эмодзи-конфетти подарка.
   * @param {{ id?: string | null, name?: string | null } | null} emoji
   */
  function setEmojiConfetti(emoji) {
    emojiConfetti = emoji && (emoji.name || emoji.id) ? { ...emoji } : null;
    emojiBtn.setAttribute("aria-pressed", emojiConfetti ? "true" : "false");
  }

  /**
   * Устанавливает или сбрасывает звуковой эффект подарка.
   * @param {{ soundId?: string | null, name?: string | null, emojiName?: string | null } | null} sound
   */
  function setSoundEffect(sound) {
    soundEffect = sound && (sound.soundId || sound.name) ? { ...sound } : null;
    soundBtn.setAttribute("aria-pressed", soundEffect ? "true" : "false");
  }

  /**
   * Загружает планы подписки с бэкенда (`list_nitro_gift_plans`), если они не были переданы заранее.
   */
  async function loadPlansIfNeeded() {
    if (Array.isArray(options.plans) && options.plans.length > 0) {
      availablePlans = [...options.plans];
      if (!selectedPlanId && availablePlans[0]) {
        selectedPlanId = availablePlans[0].id;
      }
      renderPlanRadios();
      modalState = "ready";
      syncControlsByState();
      return;
    }

    modalState = "loading";
    errorP.hidden = true;
    syncControlsByState();

    try {
      const result = await invoke("list_nitro_gift_plans", {});
      availablePlans = Array.isArray(result) ? [...result] : [];
      selectedPlanId = availablePlans[0]?.id || "";
      renderPlanRadios();
      if (availablePlans.length === 0) {
        modalState = "ready";
        errorP.textContent = GIFT_TEXT.emptyCatalog;
        errorP.hidden = false;
      } else {
        modalState = "ready";
      }
    } catch (err) {
      const classified = classifyGiftError(err);
      modalState = classified.state;
      retryAfterSeconds = classified.retryAfterSeconds;
      errorP.textContent = classified.message;
      errorP.hidden = false;
    }
    syncControlsByState();
  }

  /**
   * Открывает модальное окно `#gift-modal`.
   */
  function open() {
    if (modalState !== "closed") {
      return pendingPromise;
    }
    lastTriggerEl = triggerBtn || doc.activeElement || null;
    host.hidden = false;
    if (triggerBtn) {
      triggerBtn.setAttribute("aria-expanded", "true");
    }
    syncRecipientLabel();
    successSection.hidden = true;
    errorP.hidden = true;
    copyBtn.textContent = GIFT_TEXT.copyLink;

    pendingPromise = loadPlansIfNeeded().then(() => {
      const firstRadio = plansFieldset.querySelector('input[type="radio"][name="gift-plan"]');
      if (firstRadio && typeof firstRadio.focus === "function") {
        firstRadio.focus();
      } else if (typeof closeBtn.focus === "function") {
        closeBtn.focus();
      }
    });
    return pendingPromise;
  }

  /**
   * Закрывает модальное окно `#gift-modal`, если не идёт отправка (`submitting`).
   */
  function close() {
    if (modalState === "submitting") {
      return false;
    }
    modalState = "closed";
    host.hidden = true;
    card.setAttribute("data-state", "closed");
    if (triggerBtn) {
      triggerBtn.setAttribute("aria-expanded", "false");
    }
    if (lastTriggerEl && typeof lastTriggerEl.focus === "function") {
      lastTriggerEl.focus();
    }
    return true;
  }

  /**
   * Отправляет запрос на создание подарочного кода (`create_gift_code`).
   */
  function submit() {
    if (modalState === "submitting" || modalState === "loading" || modalState === "error-429") {
      return pendingPromise;
    }
    const plan = availablePlans.find((p) => p.id === selectedPlanId);
    if (!plan) {
      modalState = "error-400";
      errorP.textContent = GIFT_TEXT.badRequest;
      errorP.hidden = false;
      syncControlsByState();
      return Promise.resolve();
    }

    let payload;
    try {
      payload = buildCreateGiftPayload({
        plan,
        customMessage,
        cardStyleId,
        emojiConfetti,
        soundEffect,
        recipient,
        currentUserId: options.currentUserId,
      });
    } catch (validationErr) {
      modalState = "error-400";
      errorP.textContent =
        validationErr instanceof Error ? validationErr.message : GIFT_TEXT.badRequest;
      errorP.hidden = false;
      syncControlsByState();
      return Promise.resolve();
    }

    modalState = "submitting";
    errorP.hidden = true;
    successSection.hidden = true;
    syncControlsByState();

    pendingPromise = (async () => invoke("create_gift_code", payload))()
      .then((res) => {
        const cleanCode = normalizeGiftCode(res?.code || "") || String(res?.code || "").trim();
        createdGiftCode = cleanCode;
        modalState = "success";
        successCodeP.textContent = GIFT_TEXT.successCode(cleanCode);
        successSection.hidden = false;
        copyBtn.textContent = GIFT_TEXT.copyLink;
        syncControlsByState();
        if (typeof options.onGiftCreated === "function") {
          options.onGiftCreated({ ...res, code: cleanCode });
        }
      })
      .catch((err) => {
        const classified = classifyGiftError(err);
        modalState = classified.state;
        retryAfterSeconds = classified.retryAfterSeconds;
        errorP.textContent = classified.message;
        errorP.hidden = false;
        syncControlsByState();
      });

    return pendingPromise;
  }

  /**
   * Уменьшает таймер `429 Rate Limit` на 1 секунду и разблокирует форму по достижении `0`.
   */
  function tickRateLimitSecond() {
    if (modalState !== "error-429") return;
    retryAfterSeconds = Math.max(0, retryAfterSeconds - 1);
    if (retryAfterSeconds > 0) {
      errorP.textContent = GIFT_TEXT.rateLimited(retryAfterSeconds);
      errorP.hidden = false;
    } else {
      modalState = "ready";
      errorP.hidden = true;
    }
    syncControlsByState();
  }

  /**
   * Проверяет подарочный код через Tauri IPC (`resolve_gift_code`).
   * @param {string} rawCodeOrUrl
   */
  async function resolveCode(rawCodeOrUrl) {
    const code = normalizeGiftCode(rawCodeOrUrl);
    if (!code) {
      throw { code: DISCORD_GIFT_ERROR_CODES.UNKNOWN_GIFT_CODE, status: 404 };
    }
    return invoke("resolve_gift_code", {
      code,
      with_application: true,
      with_subscription_plan: true,
    });
  }

  /**
   * Активирует (redeem) подарочный код через Tauri IPC (`redeem_gift_code`).
   * @param {string} rawCodeOrUrl
   * @param {{ channelId?: string | null, paymentSourceId?: string | null }} [redeemOpts]
   */
  async function redeemCode(rawCodeOrUrl, redeemOpts = {}) {
    const code = normalizeGiftCode(rawCodeOrUrl);
    if (!code) {
      throw { code: DISCORD_GIFT_ERROR_CODES.UNKNOWN_GIFT_CODE, status: 404 };
    }
    return invoke("redeem_gift_code", {
      code,
      channel_id: redeemOpts.channelId || null,
      payment_source_id: redeemOpts.paymentSourceId || null,
    });
  }

  // Обработчики событий
  const onTriggerClick = () => {
    if (modalState === "closed") {
      open();
    } else {
      close();
    }
  };

  if (triggerBtn) {
    triggerBtn.addEventListener("click", onTriggerClick);
  }

  customTextarea.addEventListener("input", () => {
    const rawVal = String(customTextarea.value || "");
    const cps = Array.from(rawVal);
    if (cps.length > MAX_GIFT_CUSTOM_MESSAGE_CHARS) {
      customMessage = cps.slice(0, MAX_GIFT_CUSTOM_MESSAGE_CHARS).join("");
      customTextarea.value = customMessage;
    } else {
      customMessage = rawVal;
    }
    const len = countCodePoints(customMessage);
    customCounter.textContent = `${len} / ${MAX_GIFT_CUSTOM_MESSAGE_CHARS}`;
    if (len >= 180) {
      customCounter.setAttribute("data-near-limit", "true");
    } else {
      customCounter.removeAttribute?.("data-near-limit");
    }
    syncControlsByState();
  });

  plansFieldset.addEventListener("keydown", (/** @type {any} */ ev) => {
    if (availablePlans.length === 0) return;
    const currentIdx = Math.max(
      0,
      availablePlans.findIndex((p) => p.id === selectedPlanId),
    );
    let nextIdx = currentIdx;
    if (ev.key === "ArrowDown" || ev.key === "ArrowRight") {
      nextIdx = (currentIdx + 1) % availablePlans.length;
    } else if (ev.key === "ArrowUp" || ev.key === "ArrowLeft") {
      nextIdx = (currentIdx - 1 + availablePlans.length) % availablePlans.length;
    } else if (ev.key === "Home") {
      nextIdx = 0;
    } else if (ev.key === "End") {
      nextIdx = availablePlans.length - 1;
    } else {
      return;
    }
    ev.preventDefault?.();
    selectPlan(availablePlans[nextIdx].id);
  });

  card.addEventListener("keydown", (/** @type {any} */ ev) => {
    if (ev.key === "Tab") {
      const focusable = [
        closeBtn,
        ...plansFieldset.querySelectorAll('input[type="radio"][name="gift-plan"]'),
        customTextarea,
        cancelBtn,
        ...(submitBtn.disabled ? [] : [submitBtn]),
      ];
      if (focusable.length > 0) {
        const active = doc.activeElement;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (ev.shiftKey && active === first) {
          ev.preventDefault?.();
          last.focus?.();
        } else if (!ev.shiftKey && active === last) {
          ev.preventDefault?.();
          first.focus?.();
        }
      }
    }
  });

  host.addEventListener("keydown", (/** @type {any} */ ev) => {
    if (ev.key === "Escape") {
      if (modalState !== "submitting") {
        ev.preventDefault?.();
        close();
      }
    }
  });

  host.addEventListener("click", (/** @type {any} */ ev) => {
    if (!ev?.target || ev.target === host) {
      close();
    }
  });

  closeBtn.addEventListener("click", () => {
    close();
  });

  cancelBtn.addEventListener("click", () => {
    close();
  });

  submitBtn.addEventListener("click", () => {
    submit();
  });

  copyBtn.addEventListener("click", () => {
    if (!createdGiftCode) return;
    const url = formatGiftUrl(createdGiftCode);
    pendingPromise = Promise.resolve(copyText(url)).then(() => {
      copyBtn.textContent = GIFT_TEXT.copied;
    });
  });

  syncRecipientLabel();
  syncControlsByState();

  return {
    open,
    close,
    submit,
    selectPlan,
    setCardStyle,
    setEmojiConfetti,
    setSoundEffect,
    tickRateLimitSecond,
    resolveCode,
    redeemCode,
    whenReady: () => pendingPromise,
    whenIdle: () => pendingPromise,
    getState: () => ({
      state: modalState,
      selectedPlanId,
      cardStyleId,
      emojiConfetti,
      soundEffect,
      customMessage,
      createdGiftCode,
      retryAfterSeconds,
      giftStyle: resolveGiftStyle(customMessage, { emojiConfetti, soundEffect }),
    }),
    destroy: () => {
      host.hidden = true;
      modalState = "closed";
    },
  };
}
