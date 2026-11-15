// @ts-check
/**
 * Chat message rendering facade and live author DOM hydration.
 * Extracted from `app.js` as part of RW51-EPIC2-T4.
 * @module ui/features/message-rendering
 */

import { renderChannelMessage } from "./chat.js";

/**
 * Render a chat message with attachments, markdown, and message components (ActionRow & Buttons).
 * Supports both `(rowEl, item, index, total, options)` and `(item, options)` call signatures.
 *
 * @param {any} rowOrItem
 * @param {any} [maybeItemOrOptions]
 * @param {number} [index]
 * @param {number} [total]
 * @param {any} [options]
 * @returns {any}
 */
export function renderMessage(rowOrItem, maybeItemOrOptions, index = 0, total = 1, options = {}) {
  if (
    rowOrItem &&
    typeof rowOrItem === "object" &&
    ("className" in rowOrItem || "appendChild" in rowOrItem || "setAttribute" in rowOrItem)
  ) {
    const row = rowOrItem;
    const item =
      maybeItemOrOptions && typeof maybeItemOrOptions === "object" ? maybeItemOrOptions : {};
    const opts = options && typeof options === "object" ? options : {};
    renderChannelMessage(row, item, index, total, opts);
    return row;
  }
  const item = rowOrItem && typeof rowOrItem === "object" ? rowOrItem : {};
  const opts =
    maybeItemOrOptions && typeof maybeItemOrOptions === "object" ? maybeItemOrOptions : {};
  const doc = (opts && opts.document) || (typeof document !== "undefined" ? document : null);
  if (!doc) return null;
  const row = doc.createElement("div");
  renderChannelMessage(row, item, index, total, opts);
  return row;
}

/**
 * Hydrate role colors, display names, clan badges, and avatars on already-rendered
 * `#chat-list` message rows for the active guild.
 *
 * @param {any} ctx
 */
export function refreshLiveChatAuthorsInDom(ctx) {
  const {
    isLiveSession,
    currentGuildInfo,
    rail,
    selectedGuildId,
    HOME_GUILD_ID,
    cleanDiscordStr,
    resolveUserRoleInfo,
    liveUsersById,
    liveMemberClanTagByUser,
    formatClanTagBadge,
  } = ctx;

  if (!isLiveSession()) return;
  const gId = currentGuildInfo().id || rail?.selectedId || selectedGuildId() || "";
  if (!gId || gId === HOME_GUILD_ID) return;
  const rows = document.querySelectorAll("#chat-list .row.channel-message[data-author-id]");
  for (const r of rows) {
    const uid = cleanDiscordStr(r.getAttribute("data-author-id"));
    if (!uid) continue;
    const roleInfo = resolveUserRoleInfo(gId, uid);
    const nameEl = /** @type {HTMLElement | null} */ (r.querySelector(".author-name"));
    if (nameEl) {
      if (roleInfo.colorHex) {
        nameEl.style.color = roleInfo.colorHex;
        nameEl.setAttribute("data-role-color", roleInfo.colorHex);
      } else {
        nameEl.style.color = "";
        nameEl.removeAttribute("data-role-color");
      }
    }
    const knownUser = liveUsersById.get(uid);
    const disp =
      cleanDiscordStr(knownUser?.nick) ||
      cleanDiscordStr(knownUser?.globalName) ||
      cleanDiscordStr(knownUser?.global_name) ||
      cleanDiscordStr(knownUser?.username);
    if (
      nameEl &&
      disp &&
      (nameEl.textContent === uid || nameEl.textContent === "?" || nameEl.textContent === "nil")
    ) {
      nameEl.textContent = disp;
    }
    const clanTag =
      cleanDiscordStr(liveMemberClanTagByUser.get(uid)) ||
      cleanDiscordStr(knownUser?.clan?.tag) ||
      cleanDiscordStr(knownUser?.primary_guild?.tag);
    const authorEl = /** @type {HTMLElement | null} */ (r.querySelector(".author"));
    if (authorEl && nameEl && clanTag && !authorEl.querySelector(".chat-clan-badge")) {
      const cBadge = document.createElement("span");
      cBadge.className = "chat-clan-badge";
      cBadge.style.cssText =
        "display:inline-flex;align-items:center;gap:3px;padding:1px 5px;margin-left:6px;border-radius:4px;background:#2b2d31;border:1px solid rgba(255,255,255,0.08);color:#dbdee1;font-size:11px;font-weight:700;vertical-align:middle;";
      cBadge.textContent = formatClanTagBadge(clanTag);
      nameEl.after(cBadge);
    }
    const avHash = cleanDiscordStr(knownUser?.avatar);
    const avEl = /** @type {HTMLElement | null} */ (r.querySelector(".avatar"));
    if (avEl && avHash && !avEl.style.backgroundImage && typeof globalThis.Image === "function") {
      const avatarUrl = `https://cdn.discordapp.com/avatars/${uid}/${avHash}.png?size=64`;
      const img = new globalThis.Image();
      img.onload = () => {
        avEl.style.backgroundImage = `url("${avatarUrl}")`;
        avEl.style.backgroundSize = "cover";
        avEl.style.backgroundPosition = "center";
        avEl.style.color = "transparent";
      };
      img.src = avatarUrl;
    }
  }
}
