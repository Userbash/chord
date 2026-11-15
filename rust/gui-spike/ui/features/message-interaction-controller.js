// @ts-check
/**
 * Live chat message permission checks, edit/delete/pin actions, context menu,
 * and hover toolbar bindings.
 * Extracted from `app.js` as part of RW51-EPIC2-T4.
 * @module ui/features/message-interaction-controller
 */

/**
 * Create the message interaction controller for live chat rows.
 *
 * @param {any} ctx
 */
export function createMessageInteractionController(ctx) {
  const {
    byId,
    content,
    chat,
    rail,
    tree,
    dmFriendsController,
    selectedGuildId,
    cleanDiscordStr,
    computeEffectiveChannelPermissions,
    computeServerPermissionsAndHierarchy,
    clearErrorState,
    showVoiceToastNotice,
    invoke,
    isLiveSession,
    PIN_CHANNEL_MESSAGE_IPC_CMD,
    emojiPickerController,
  } = ctx;

  /**
   * Read the current authenticated user object from context (supports getter or static object).
   * @returns {any}
   */
  function getSelfUser() {
    return typeof ctx.getLiveSelfUser === "function" ? ctx.getLiveSelfUser() : ctx.liveSelfUser;
  }

  /**
   * Check whether the active or specified channel belongs to a DM or group DM (`@me`).
   *
   * @param {string} [channelIdOverride]
   * @returns {boolean}
   */
  function isDmOrGroupContext(channelIdOverride) {
    const activeGuild = cleanDiscordStr(String(rail.selectedId || selectedGuildId() || ""));
    if (!activeGuild || activeGuild === "@me") return true;
    const chId = cleanDiscordStr(String(channelIdOverride || chat.channelId || ""));
    const pcs = dmFriendsController?.getPrivateChannels?.();
    if (
      chId &&
      Array.isArray(pcs) &&
      pcs.some((/** @type {any} */ pc) => cleanDiscordStr(String(pc?.id ?? "")) === chId)
    ) {
      return true;
    }
    return false;
  }

  /**
   * Check whether a message row was authored by the currently signed-in user.
   * An empty author never counts as an own message.
   *
   * @param {HTMLElement | null | undefined} msgRow
   * @returns {boolean}
   */
  function isOwnLiveMessage(msgRow) {
    if (!msgRow) return false;
    const liveSelfUser = getSelfUser();
    const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
    const rowAuthorId = cleanDiscordStr(String(msgRow.getAttribute("data-author-id") || ""));
    if (selfId && rowAuthorId) {
      return rowAuthorId === selfId;
    }
    const authorName = cleanDiscordStr(
      String(
        msgRow.querySelector(".author-name")?.textContent ||
          msgRow.querySelector(".author .username")?.textContent ||
          "",
      ),
    );
    if (!authorName) return false;
    const selfNames = new Set(
      [
        cleanDiscordStr(String(liveSelfUser?.globalName || "")),
        cleanDiscordStr(String(liveSelfUser?.global_name || "")),
        cleanDiscordStr(String(liveSelfUser?.username || "")),
      ].filter(Boolean),
    );
    return selfNames.size > 0 && selfNames.has(authorName);
  }

  /**
   * Check whether a message row is a system message (calls, joins, boosts, etc.)
   * which Discord forbids editing or deleting via REST.
   *
   * @param {HTMLElement | null | undefined} msgRow
   * @returns {boolean}
   */
  function isSystemLiveMessage(msgRow) {
    if (!msgRow) return false;
    const msgType = Number(msgRow.getAttribute("data-message-type") || 0);
    if (msgType !== 0 && msgType !== 19) return true;
    const body = msgRow.querySelector(".text-body");
    if (body?.getAttribute("data-system-call") === "true") return true;
    const txt = String(body?.textContent ?? "").trim();
    if (txt.startsWith("📞")) return true;
    return false;
  }

  /**
   * Check whether the current user can edit the message row.
   *
   * @param {HTMLElement | null | undefined} msgRow
   * @returns {boolean}
   */
  function canEditLiveMessage(msgRow) {
    if (!msgRow || isSystemLiveMessage(msgRow)) return false;
    return isOwnLiveMessage(msgRow);
  }

  /**
   * Shared guild `MANAGE_MESSAGES` (`1n << 13n`) / admin / owner permission check.
   *
   * @param {string} chId
   * @param {string} rowAuthorId
   * @returns {boolean}
   */
  function hasGuildManageMessagesPerm(chId, rowAuthorId) {
    const activeGuildId = cleanDiscordStr(String(rail.selectedId || selectedGuildId() || ""));
    if (!activeGuildId || activeGuildId === "@me") return false;
    const chObj = tree.channels.find((/** @type {any} */ c) => cleanDiscordStr(String(c?.id ?? "")) === chId);
    if (chObj) {
      const chPerms = computeEffectiveChannelPermissions(activeGuildId, chObj, tree.channels);
      if (chPerms !== null) {
        return (chPerms & (1n << 13n)) !== 0n;
      }
    }
    const perms = computeServerPermissionsAndHierarchy(activeGuildId, rowAuthorId);
    return Boolean(perms.isOwner || perms.isAdmin || perms.canManageMessages);
  }

  /**
   * Check whether the current user can delete the message row.
   * In DMs/group DMs only own non-system messages can be deleted; in guilds,
   * own messages or messages covered by `MANAGE_MESSAGES` can be deleted.
   *
   * @param {HTMLElement | null | undefined} msgRow
   * @returns {boolean}
   */
  function canDeleteLiveMessage(msgRow) {
    if (!msgRow || isSystemLiveMessage(msgRow)) return false;
    if (isOwnLiveMessage(msgRow)) return true;
    const chId = cleanDiscordStr(
      String(msgRow.getAttribute("data-channel-id") || chat.channelId || ""),
    );
    if (isDmOrGroupContext(chId)) {
      return false;
    }
    const rowAuthorId = cleanDiscordStr(String(msgRow.getAttribute("data-author-id") || ""));
    return hasGuildManageMessagesPerm(chId, rowAuthorId);
  }

  /**
   * Check whether the current user can pin the message row.
   *
   * @param {HTMLElement | null | undefined} msgRow
   * @returns {boolean}
   */
  function canPinLiveMessage(msgRow) {
    const chId = cleanDiscordStr(
      String(msgRow?.getAttribute("data-channel-id") || chat.channelId || ""),
    );
    if (isDmOrGroupContext(chId)) return true;
    return hasGuildManageMessagesPerm(chId, "");
  }

  /**
   * Delete a message via `delete_message` IPC, optimistically hiding the row
   * and rolling back on network/API failure.
   *
   * @param {HTMLElement} msgRow
   */
  async function deleteLiveMessageRow(msgRow) {
    if (!msgRow) return;
    if (isSystemLiveMessage(msgRow)) {
      showVoiceToastNotice("Системные сообщения (звонки, добавления) нельзя удалять");
      return;
    }
    if (!canDeleteLiveMessage(msgRow)) {
      showVoiceToastNotice("У вас нет прав на удаление чужого сообщения");
      return;
    }
    const msgId = String(
      msgRow.getAttribute("data-message-id") || msgRow.getAttribute("data-id") || "",
    ).trim();
    let chId = String(
      msgRow.getAttribute("data-channel-id") || chat.channelId || "",
    ).trim();
    if (!chId || chId === "news") {
      chId = String(
        document
          .querySelector('#channel-tree .channel-item[aria-current="page"]')
          ?.getAttribute("data-channel-id") ||
          document.querySelector("#chat-list")?.getAttribute("data-channel-id") ||
          chat.channel?.id ||
          "",
      ).trim();
    }
    if (!msgId || !chId || chId === "news") {
      showVoiceToastNotice("Не удалось определить ID канала или сообщения для удаления");
      return;
    }
    msgRow.setAttribute("data-state", "deleted");
    msgRow.style.display = "none";
    try {
      await invoke("delete_message", {
        channelId: chId,
        messageId: msgId,
        channel_id: chId,
        message_id: msgId,
      });
      /** @type {any} */ (chat).removeMessageFromWindow?.(msgId, chId);
      msgRow.remove();
      showVoiceToastNotice("Сообщение удалено");
    } catch (err) {
      msgRow.style.display = "";
      msgRow.removeAttribute("data-state");
      clearErrorState(content);
      const detail =
        typeof err === "object" && err !== null
          ? /** @type {any} */ (err).message || /** @type {any} */ (err).kind || ""
          : String(err || "");
      showVoiceToastNotice(
        "Не удалось удалить сообщение в Discord" + (detail ? `: ${detail}` : ""),
      );
    }
  }

  /**
   * Open the full Discord message context menu (`#live-msg-context-menu`).
   *
   * @param {HTMLElement} msgRow
   * @param {number} clientX
   * @param {number} clientY
   */
  function openLiveMessageContextMenu(msgRow, clientX, clientY) {
    document.getElementById("live-msg-context-menu")?.remove();
    document.getElementById("live-emoji-picker-popout")?.remove();

    const menu = document.createElement("div");
    menu.id = "live-msg-context-menu";
    menu.setAttribute("role", "menu");
    menu.setAttribute("aria-label", "Действия с сообщением");
    const winW = window.innerWidth || 1280;
    const winH = window.innerHeight || 800;
    const left = Math.max(12, Math.min(winW - 252, clientX));
    const top = Math.max(12, Math.min(winH - 490, clientY));
    menu.style.cssText =
      `position:fixed;left:${left}px;top:${top}px;width:236px;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:8px;padding:6px;box-shadow:0 16px 36px rgba(0,0,0,0.78);z-index:10040;display:flex;flex-direction:column;gap:2px;font-size:13px;user-select:none;`;

    const recentReactionEmojis = emojiPickerController.getRecentReactionEmojis();
    const quickBar = document.createElement("div");
    quickBar.style.cssText =
      "display:grid;grid-template-columns:repeat(4,1fr);gap:4px;padding:4px 2px 6px;border-bottom:1px solid #2b2d31;margin-bottom:2px;position:relative;";
    for (const item of recentReactionEmojis.slice(0, 4)) {
      const qb = document.createElement("button");
      qb.type = "button";
      qb.textContent = item.glyph;
      qb.title = item.shortcode;
      qb.style.cssText =
        "height:36px;border:none;border-radius:6px;background:#1e1f22;font-size:20px;cursor:pointer;display:flex;align-items:center;justify-content:center;transition:background 100ms ease,transform 100ms ease;";
      qb.addEventListener("mouseenter", () => {
        qb.style.background = "#35373c";
        qb.style.transform = "scale(1.1)";
      });
      qb.addEventListener("mouseleave", () => {
        qb.style.background = "#1e1f22";
        qb.style.transform = "scale(1)";
      });
      qb.addEventListener("click", () => {
        emojiPickerController.applyQuickReactionToRow(msgRow, item.glyph, item.shortcode);
        menu.remove();
      });
      quickBar.append(qb);
    }
    menu.append(quickBar);

    const addDivider = () => {
      const d = document.createElement("div");
      d.style.cssText = "height:1px;background:#2b2d31;margin:3px 4px;";
      menu.append(d);
    };

    const addItem = (
      /** @type {string} */ label,
      /** @type {string} */ rightGlyph,
      /** @type {() => void} */ onClick,
      /** @type {{ danger?: boolean }} */ opts = {},
    ) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.setAttribute("role", "menuitem");
      const baseColor = opts.danger ? "#f23f43" : "#dbdee1";
      btn.style.cssText =
        `width:100%;padding:6px 8px;border:none;border-radius:4px;background:transparent;color:${baseColor};font-size:13px;font-weight:500;cursor:pointer;display:flex;align-items:center;justify-content:space-between;gap:8px;text-align:left;`;
      btn.innerHTML = `<span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${label}</span><span style="font-size:13px;opacity:0.85;flex-shrink:0;">${rightGlyph}</span>`;
      btn.addEventListener("mouseenter", () => {
        btn.style.background = opts.danger ? "#da373c" : "#5865f2";
        btn.style.color = "#ffffff";
      });
      btn.addEventListener("mouseleave", () => {
        btn.style.background = "transparent";
        btn.style.color = baseColor;
      });
      btn.addEventListener("click", () => {
        onClick();
      });
      menu.append(btn);
      return btn;
    };

    const msgText = String(msgRow.querySelector(".text-body")?.textContent || "");
    const msgId = String(
      msgRow.getAttribute("data-message-id") || msgRow.getAttribute("data-id") || "",
    ).trim();
    const chId = String(
      msgRow.getAttribute("data-channel-id") || chat.channelId || "",
    ).trim();
    const authorName = String(
      msgRow.querySelector(".author-name")?.textContent ||
        msgRow.querySelector(".author .username")?.textContent ||
        "",
    );
    const canEdit = canEditLiveMessage(msgRow);
    const canDelete = canDeleteLiveMessage(msgRow);
    const canPin = canPinLiveMessage(msgRow);

    addItem("Добавить реакцию", "›", () => {
      const rect = menu.getBoundingClientRect();
      emojiPickerController.openLiveMessageEmojiPicker(
        msgRow,
        Math.max(16, rect.left - 386),
        rect.top,
      );
    });
    addDivider();
    if (canEdit) {
      addItem("Редактировать", "✏️", () => {
        menu.remove();
        const bodyEl = /** @type {HTMLElement | null} */ (msgRow.querySelector(".text-body"));
        if (!bodyEl) return;
        const orig = bodyEl.textContent || "";
        const editWrap = document.createElement("div");
        editWrap.style.cssText = "display:flex;gap:6px;margin-top:4px;";
        const inp = document.createElement("input");
        inp.type = "text";
        inp.value = orig;
        inp.style.cssText =
          "flex:1;padding:6px 8px;border-radius:6px;border:1px solid #5865f2;background:#1e1f22;color:#f2f3f5;font-size:13px;";
        const saveB = document.createElement("button");
        saveB.type = "button";
        saveB.textContent = "Сохранить";
        saveB.style.cssText =
          "padding:5px 10px;border-radius:6px;border:none;background:#5865f2;color:#fff;font-size:12px;font-weight:600;cursor:pointer;";
        saveB.addEventListener("click", () => {
          const nextContent = inp.value.trim();
          editWrap.remove();
          if (!nextContent || nextContent === orig) return;
          bodyEl.textContent = nextContent;
          if (chId && msgId) {
            void invoke("edit_message", {
              channelId: chId,
              messageId: msgId,
              content: nextContent,
            })
              .then(() => {
                /** @type {any} */ (chat).updateMessageInWindow?.(msgId, nextContent, chId);
              })
              .catch(() => {
                bodyEl.textContent = orig;
                clearErrorState(content);
                showVoiceToastNotice("Не удалось изменить сообщение");
              });
          }
        });
        editWrap.append(inp, saveB);
        bodyEl.after(editWrap);
        inp.focus();
      });
    }
    addItem("Ответить", "↩", () => {
      menu.remove();
      const inputEl = /** @type {HTMLTextAreaElement | HTMLInputElement | null} */ (
        byId("composer-input") || byId("chat-input")
      );
      if (inputEl) {
        inputEl.value = `> ${msgText.slice(0, 80)}\n@${authorName || "user"} `;
        inputEl.focus();
      }
    });
    addItem("Переслать", "↪", () => {
      menu.remove();
      try {
        void navigator.clipboard?.writeText?.(msgText);
      } catch {}
    });
    addDivider();
    addItem("Скопировать текст", "📋", () => {
      menu.remove();
      try {
        void navigator.clipboard?.writeText?.(msgText);
      } catch {}
    });
    if (canPin) {
      addItem("Закрепить сообщение", "📌", () => {
        menu.remove();
        msgRow.style.borderLeft = "3px solid #f0b232";
        msgRow.style.paddingLeft = "8px";
        if (chId && msgId) {
          void invoke(PIN_CHANNEL_MESSAGE_IPC_CMD, {
            channelId: chId,
            messageId: msgId,
          }).catch(() => {});
        }
      });
    }
    addItem("Добавить в закладки", "🔖", () => {
      menu.remove();
    });
    addItem("Создать напоминание", "›", () => {
      menu.remove();
    });
    addItem("Приложения", "›", () => {
      menu.remove();
    });
    addItem("Отметить как непрочитанное", "📩", () => {
      menu.remove();
    });
    addItem("Скопировать ссылку на сообщение", "🔗", () => {
      menu.remove();
      const url = `https://discord.com/channels/${rail.selectedId || "@me"}/${chId || "0"}/${msgId}`;
      try {
        void navigator.clipboard?.writeText?.(url);
      } catch {}
    });
    addItem("Зачитать сообщение", "🔊", () => {
      menu.remove();
      try {
        if (
          typeof globalThis.speechSynthesis?.speak === "function" &&
          typeof globalThis.SpeechSynthesisUtterance === "function"
        ) {
          const u = new globalThis.SpeechSynthesisUtterance(msgText);
          u.lang = "ru-RU";
          globalThis.speechSynthesis.speak(u);
        }
      } catch {}
    });
    if (canDelete) {
      addDivider();
      addItem(
        "Удалить сообщение",
        "🗑",
        () => {
          menu.remove();
          void deleteLiveMessageRow(msgRow);
        },
        { danger: true },
      );
    }
    addDivider();
    addItem("Копировать ID сообщения", "ID", () => {
      menu.remove();
      try {
        void navigator.clipboard?.writeText?.(msgId);
      } catch {}
    });

    document.body.append(menu);
  }

  /**
   * Bind `contextmenu`, `dblclick`, and `mouseover` hover-toolbar listeners on `#chat-list`
   * when running a live session outside QA harness mode.
   *
   * @param {HTMLElement | null} [chatListOverride]
   */
  function bindLiveChatMessageEvents(chatListOverride) {
    if (!isLiveSession() || /** @type {any} */ (globalThis).__QA__) return;
    const chatListEl = chatListOverride ?? byId("chat-list");
    if (!chatListEl) return;

    chatListEl.addEventListener("contextmenu", (/** @type {any} */ ev) => {
      const target = typeof Element !== "undefined" && ev.target instanceof Element ? ev.target : null;
      const row = /** @type {HTMLElement | null} */ (target?.closest?.(".row.channel-message") ?? null);
      if (!row) return;
      ev.preventDefault?.();
      ev.stopPropagation?.();
      openLiveMessageContextMenu(row, ev.clientX || 240, ev.clientY || 240);
    });

    chatListEl.addEventListener("dblclick", (/** @type {any} */ ev) => {
      const target = typeof Element !== "undefined" && ev.target instanceof Element ? ev.target : null;
      if (target?.closest?.("button, a, input, .reaction-pill, .live-msg-hover-toolbar")) return;
      const row = /** @type {HTMLElement | null} */ (target?.closest?.(".row.channel-message") ?? null);
      if (!row) return;
      ev.preventDefault?.();
      ev.stopPropagation?.();
      emojiPickerController.openLiveMessageEmojiPicker(row, ev.clientX || 260, ev.clientY || 220);
    });

    chatListEl.addEventListener("mouseover", (/** @type {any} */ ev) => {
      const target = typeof Element !== "undefined" && ev.target instanceof Element ? ev.target : null;
      const row = /** @type {HTMLElement | null} */ (target?.closest?.(".row.channel-message") ?? null);
      if (!row || row.querySelector(".live-msg-hover-toolbar")) return;
      for (const old of document.querySelectorAll(".live-msg-hover-toolbar")) {
        old.remove();
      }
      row.style.position = "relative";
      const bar = document.createElement("div");
      bar.className = "live-msg-hover-toolbar";
      bar.style.cssText =
        "position:absolute;top:-14px;right:16px;background:#313338;border:1px solid #232428;border-radius:8px;padding:2px 4px;display:inline-flex;align-items:center;gap:2px;box-shadow:0 4px 12px rgba(0,0,0,0.35);z-index:30;";

      const addHoverBtn = (
        /** @type {string} */ txt,
        /** @type {string} */ titleTxt,
        /** @type {(e: MouseEvent) => void} */ fn,
      ) => {
        const b = document.createElement("button");
        b.type = "button";
        b.textContent = txt;
        b.title = titleTxt;
        b.style.cssText =
          "width:28px;height:28px;border:none;border-radius:6px;background:transparent;color:#b5bac1;font-size:15px;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;";
        b.addEventListener("mouseenter", () => {
          b.style.background = "#3f4147";
          b.style.color = "#f2f3f5";
        });
        b.addEventListener("mouseleave", () => {
          b.style.background = "transparent";
          b.style.color = "#b5bac1";
        });
        b.addEventListener("click", (e) => {
          e.stopPropagation();
          fn(e);
        });
        bar.append(b);
      };

      const recentReactionEmojis = emojiPickerController.getRecentReactionEmojis();
      for (const r of recentReactionEmojis.slice(0, 3)) {
        addHoverBtn(r.glyph, r.shortcode, () =>
          emojiPickerController.applyQuickReactionToRow(row, r.glyph, r.shortcode),
        );
      }
      addHoverBtn("😊", "Добавить реакцию", (e) => {
        emojiPickerController.openLiveMessageEmojiPicker(row, e.clientX || 300, e.clientY || 220);
      });
      addHoverBtn("↩", "Ответить", () => {
        const inputEl = /** @type {HTMLTextAreaElement | HTMLInputElement | null} */ (
          byId("composer-input") || byId("chat-input")
        );
        const authorName = String(
          row.querySelector(".author-name")?.textContent ||
            row.querySelector(".author .username")?.textContent ||
            "user",
        );
        if (inputEl) {
          inputEl.value = `@${authorName} `;
          inputEl.focus();
        }
      });
      if (canDeleteLiveMessage(row)) {
        addHoverBtn("🗑", "Удалить сообщение", () => {
          void deleteLiveMessageRow(row);
        });
      }
      addHoverBtn("⋯", "Ещё", (e) => {
        openLiveMessageContextMenu(row, e.clientX || 320, e.clientY || 220);
      });
      row.append(bar);
      row.addEventListener(
        "mouseleave",
        () => {
          bar.remove();
        },
        { once: true },
      );
    });
  }

  return {
    isDmOrGroupContext,
    isOwnLiveMessage,
    isSystemLiveMessage,
    canEditLiveMessage,
    canDeleteLiveMessage,
    canPinLiveMessage,
    deleteLiveMessageRow,
    openLiveMessageContextMenu,
    bindLiveChatMessageEvents,
  };
}
