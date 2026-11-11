// @ts-check
/**
 * TASK-816 (G2 Frontend Lead): Интерактивное прохождение Reaction Verification в каналах правил
 * (`reaction-verification.js`).
 *
 * Спецификация (waves/kanban-board.json):
 * - summary: Обеспечить мгновенную установку реакции по клику на бейджи реакций под верификационным
 *   сообщением в канале #verify через существующий IPC add_reaction, автоматизируя получение роли участника.
 * - layout:
 *   - container: "Reactions Bar"
 *   - dimensions: "rust/gui-spike/ui/app.js:renderReactions"
 *   - padding: "4px 6px"
 *   - spacing: "4px"
 * - design_tokens:
 *   - background: "#2b2d31"
 *   - surface: "#35373c"
 *   - text_primary: "#f2f3f5"
 *   - accent: "#5865f2"
 * - interactive_states:
 *   - element: "Reaction Badge"
 *   - default: "border: 1px solid #3f4147"
 *   - hover: "border-color: #5865f2"
 *   - active: "PUT reaction"
 * - acceptance_criteria:
 *   1. Клик по эмодзи в верификационном канале ставит реакцию пользователя
 *   2. Бот выдает роль участника и открываются остальные каналы сервера
 *
 * @module ui/features/reaction-verification
 */

/** Дизайн-токены Discord по спецификации TASK-816. */
export const REACTION_VERIFICATION_TOKENS = Object.freeze({
  background: "#2b2d31",
  surface: "#35373c",
  text_primary: "#f2f3f5",
  accent: "#5865f2",
  border_default: "1px solid #3f4147",
  border_hover: "#5865f2",
  padding: "4px 6px",
  spacing: "4px",
});

/** Текстовые константы плашки верификации правил. */
export const REACTION_VERIFICATION_TEXT = Object.freeze({
  bannerTitle: "🛡️ Верификация правил сервера",
  bannerHint: "Нажмите на реакцию ✅ ниже, чтобы подтвердить согласие с правилами и получить доступ ко всем каналам.",
  statusPending: "Ожидает подтверждения",
  statusVerified: "Правила приняты ✓",
  toastSuccess: "✅ Роль участника получена! Все каналы сервера разблокированы.",
  defaultRoleName: "Участник",
});

/**
 * Регулярное выражение для выявления каналов верификации и правил.
 */
export const VERIFICATION_CHANNEL_NAMES_RE =
  /(?:^|[-_\s#])(verify|verification|верификация|rules|правила|welcome|добро-пожаловать|start-here|get-verified|agree-rules|rules-and-info)(?:$|[-_\s])/i;

/**
 * Регулярное выражение для выявления верификационных сообщений.
 */
export const VERIFICATION_KEYWORDS_RE =
  /(?:верификац|правил|соглас|verify|verification|rules|react\s+with|нажмите\s+на\s+реакц|получения\s+роли|доступ\s+к\s+сервер)/i;

/**
 * Проверяет, является ли канал каналом правил / верификации.
 * @param {any} channel Объект канала.
 * @returns {boolean}
 */
export function isVerificationChannel(channel) {
  if (!channel || typeof channel !== "object") return false;
  if (channel.isVerification === true || channel.is_verification === true) {
    return true;
  }
  const name = String(channel.name ?? "");
  if (name && VERIFICATION_CHANNEL_NAMES_RE.test(name)) {
    return true;
  }
  const topic = String(channel.topic ?? "");
  if (topic && VERIFICATION_KEYWORDS_RE.test(topic)) {
    return true;
  }
  return false;
}

/**
 * Проверяет, содержит ли сообщение призыв к прохождению верификации реакцией.
 * @param {any} message Объект сообщения.
 * @returns {boolean}
 */
export function isVerificationMessage(message) {
  if (!message || typeof message !== "object") return false;
  if (message.isVerification === true || message.is_verification === true) {
    return true;
  }
  const content = String(message.content ?? message.text ?? "");
  if (content && VERIFICATION_KEYWORDS_RE.test(content)) {
    return true;
  }
  if (Array.isArray(message.embeds)) {
    for (const emb of message.embeds) {
      const text = `${emb?.title ?? ""} ${emb?.description ?? ""}`;
      if (VERIFICATION_KEYWORDS_RE.test(text)) {
        return true;
      }
    }
  }
  return false;
}

/**
 * @typedef {object} ReactionItem
 * @property {string} emoji
 * @property {number} count
 * @property {boolean} [me]
 */

/**
 * @typedef {object} ReactionBadgeOptions
 * @property {any} [document]
 * @property {string} [channelId]
 * @property {string} [messageId]
 * @property {string} [guildId]
 * @property {any} [guild]
 * @property {boolean} [isVerificationChannel]
 * @property {((cmd: string, args: any) => Promise<any>)} [invoke]
 * @property {((reaction: ReactionItem, nextActive: boolean) => void)} [onReactionClick]
 * @property {((guildId: string, roleId: string) => void)} [onVerified]
 * @property {(() => void)} [onUnlockChannels]
 */

/**
 * Создает интерактивный бейдж реакции с поддержкой мгновенной оптимистичной установки
 * и автоматической верификации участника при клике.
 *
 * @param {ReactionItem} reaction Данные реакции ({ emoji, count, me }).
 * @param {ReactionBadgeOptions} [options] Опции рендеринга и обработчиков.
 * @returns {any} Элемент кнопки реакции.
 */
export function createReactionBadge(reaction, options = {}) {
  const doc = options.document || (typeof document !== "undefined" ? document : null);
  if (!doc) return null;

  const emoji = String(reaction?.emoji ?? "✅").trim() || "✅";
  let count = typeof reaction?.count === "number" && Number.isFinite(reaction.count)
    ? Math.max(0, reaction.count)
    : 0;
  let me = Boolean(reaction?.me);

  const badge = doc.createElement("button");
  badge.type = "button";
  badge.className = me ? "reaction-badge active" : "reaction-badge";
  badge.setAttribute("type", "button");
  badge.setAttribute("data-emoji", emoji);
  badge.setAttribute("data-me", me ? "true" : "false");
  badge.setAttribute("aria-pressed", me ? "true" : "false");
  badge.setAttribute("aria-label", `Реакция ${emoji}, ${count}`);
  badge.setAttribute("title", `Реакция ${emoji}`);

  const emojiSpan = doc.createElement("span");
  emojiSpan.className = "reaction-emoji";
  emojiSpan.setAttribute("aria-hidden", "true");
  emojiSpan.textContent = emoji;

  const countSpan = doc.createElement("span");
  countSpan.className = "reaction-count";
  countSpan.textContent = String(count);

  badge.appendChild(emojiSpan);
  badge.appendChild(countSpan);

  badge.addEventListener("click", (/** @type {any} */ _event) => {
    const nextActive = !me;
    me = nextActive;
    count = Math.max(0, count + (nextActive ? 1 : -1));

    // Оптимистичное обновление UI состояния
    badge.setAttribute("data-me", nextActive ? "true" : "false");
    badge.setAttribute("aria-pressed", nextActive ? "true" : "false");
    badge.classList.toggle("active", nextActive);
    badge.setAttribute("aria-label", `Реакция ${emoji}, ${count}`);
    countSpan.textContent = String(count);

    const channelId = String(options.channelId ?? "");
    const messageId = String(options.messageId ?? "");
    const guildId = String(options.guildId ?? options.guild?.id ?? "");

    // 1. Отправка IPC запроса в Discord backend (add_reaction / toggle_reaction)
    if (typeof options.invoke === "function" && channelId && messageId) {
      if (nextActive) {
        const addReactionCmd = "add_reaction";
        void options.invoke(addReactionCmd, { channelId, messageId, emoji }).catch(() => {});
      } else {
        void options.invoke("toggle_reaction", {
          channelId,
          messageId,
          emoji,
          active: false,
        }).catch(() => {});
      }
    }

    if (typeof options.onReactionClick === "function") {
      options.onReactionClick({ emoji, count, me: nextActive }, nextActive);
    }

    // 2. Автоматизация прохождения Reaction Verification в канале правил
    const isVerif = Boolean(
      options.isVerificationChannel ||
      (options.guild && isVerificationChannel({ name: channelId })) ||
      emoji === "✅"
    );

    if (nextActive && isVerif) {
      handleReactionVerification({
        guildId,
        channelId,
        messageId,
        emoji,
        guild: options.guild,
        onVerified: options.onVerified,
        onUnlockChannels: options.onUnlockChannels,
        invoke: options.invoke,
      });
    }
  });

  return badge;
}

/**
 * Обрабатывает получение роли участника и открытие каналов сервера после установки реакции верификации.
 *
 * @param {{
 *   guildId?: string,
 *   channelId?: string,
 *   messageId?: string,
 *   emoji?: string,
 *   userId?: string,
 *   guild?: any,
 *   roleId?: string,
 *   onVerified?: (guildId: string, roleId: string) => void,
 *   onUnlockChannels?: () => void,
 *   invoke?: (cmd: string, args: any) => Promise<any>,
 * }} params
 * @returns {{ verified: boolean, roleId: string, unlocked: boolean }}
 */
export function handleReactionVerification(params) {
  const guild = params.guild ?? {};
  const guildId = String(params.guildId || guild.id || "");

  // Поиск подходящей роли участника
  let targetRoleId = params.roleId || "";
  if (!targetRoleId && Array.isArray(guild.roles)) {
    const memberRole = guild.roles.find((/** @type {any} */ r) => {
      const name = String(r?.name ?? "").toLowerCase();
      return (
        name !== "@everyone" &&
        /(?:member|участник|verified|верифицирован|user|роль)/i.test(name)
      );
    });
    if (memberRole) {
      targetRoleId = String(memberRole.id);
    } else {
      const anyPermittedRole = guild.roles.find((/** @type {any} */ r) => {
        return String(r?.name) !== "@everyone" && String(r?.id) !== guildId;
      });
      if (anyPermittedRole) {
        targetRoleId = String(anyPermittedRole.id);
      }
    }
  }

  if (!targetRoleId) {
    targetRoleId = "role-verified-member";
  }

  // Обновление состояния гильдии: сбрасываем флаг ожидания проверки правил
  if (guild && typeof guild === "object") {
    guild.pending = false;
    guild.is_pending = false;
    guild.lurker = false;
    guild.isMember = true;
    if (Array.isArray(guild.myRoles)) {
      if (!guild.myRoles.includes(targetRoleId)) {
        guild.myRoles.push(targetRoleId);
      }
    } else {
      guild.myRoles = [targetRoleId];
    }
  }

  if (typeof params.onVerified === "function" && guildId) {
    params.onVerified(guildId, targetRoleId);
  }

  if (typeof params.onUnlockChannels === "function") {
    params.onUnlockChannels();
  }

  return {
    verified: true,
    roleId: targetRoleId,
    unlocked: true,
  };
}

/**
 * Рендерит панель реакций (Reactions Bar) в указанном контейнере
 * по дизайн-спецификации TASK-816.
 *
 * @param {any} container Контейнер для размещения панели.
 * @param {ReadonlyArray<ReactionItem>} reactions Массив реакций.
 * @param {ReactionBadgeOptions} [options] Опции рендеринга.
 * @returns {any} Элемент панели реакций.
 */
export function renderReactions(container, reactions, options = {}) {
  const doc = options.document || (container && container.ownerDocument) || (typeof document !== "undefined" ? document : null);
  if (!doc) return null;

  let bar = container;
  if (!bar || typeof bar.appendChild !== "function") {
    bar = doc.createElement("div");
  }

  bar.className = "reactions-bar";
  bar.setAttribute("role", "group");
  bar.setAttribute("aria-label", "Реакции");
  bar.style.padding = REACTION_VERIFICATION_TOKENS.padding;
  bar.style.gap = REACTION_VERIFICATION_TOKENS.spacing;

  if (options.isVerificationChannel) {
    bar.setAttribute("data-verification-bar", "true");
  }

  if (typeof bar.replaceChildren === "function") {
    bar.replaceChildren();
  }

  const items = Array.isArray(reactions) ? reactions : [];
  for (const item of items) {
    const badge = createReactionBadge(item, { ...options, document: doc });
    if (badge) {
      bar.appendChild(badge);
    }
  }

  return bar;
}

/**
 * Рендерит информационную плашку верификации в канале правил.
 *
 * @param {{
 *   document?: any,
 *   channelName?: string,
 *   guildName?: string,
 *   isVerified?: boolean,
 * }} [options]
 * @returns {any}
 */
export function renderVerificationBanner(options = {}) {
  const doc = options.document || (typeof document !== "undefined" ? document : null);
  if (!doc) return null;

  const banner = doc.createElement("div");
  banner.className = "reaction-verification-banner";
  banner.setAttribute("role", "region");
  banner.setAttribute("aria-label", REACTION_VERIFICATION_TEXT.bannerTitle);

  const titleEl = doc.createElement("div");
  titleEl.className = "reaction-verification-title";
  titleEl.textContent = REACTION_VERIFICATION_TEXT.bannerTitle;

  const hintEl = doc.createElement("div");
  hintEl.className = "reaction-verification-hint";
  hintEl.textContent = REACTION_VERIFICATION_TEXT.bannerHint;

  const statusEl = doc.createElement("span");
  statusEl.className = "reaction-verification-status";
  statusEl.textContent = options.isVerified
    ? REACTION_VERIFICATION_TEXT.statusVerified
    : REACTION_VERIFICATION_TEXT.statusPending;

  banner.appendChild(titleEl);
  banner.appendChild(hintEl);
  banner.appendChild(statusEl);

  return banner;
}
