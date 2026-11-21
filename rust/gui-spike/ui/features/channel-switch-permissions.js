// @ts-check
/**
 * Reactive channel permissions DOM synchronization, channel switch PiP minimization,
 * and DM header call action bindings.
 * Extracted from `app.js` as part of RW51-EPIC2-T3.
 * @module ui/features/channel-switch-permissions
 */

import { byId } from "./dom.js";
import { clearErrorState } from "./errors.js";

/**
 * Create a controller for `syncActiveChannelPermissionsInDom`, `minimizeStageViewerToPip`,
 * `onChannelSwitch`, and `installChatOpenChannelWrapper`.
 *
 * @param {any} ctx
 */
export function createChannelSwitchPermissionsController(ctx) {
  /**
   * Compute effective channel policy based on guild lurker/pending state and `SEND_MESSAGES` permission.
   * @param {any} resolvedTarget
   * @param {string} curGid
   * @param {boolean} isDm
   */
  function resolveChannelPolicyTarget(resolvedTarget, curGid, isDm) {
    if (isDm || !curGid || curGid === "@me") return resolvedTarget;

    const guildObj = ctx.getGuildById(curGid);
    const isLurkerGuild =
      guildObj?.lurker === true ||
      guildObj?.isMember === false ||
      guildObj?.is_member === false;
    const isPendingRules = guildObj?.pending === true;
    const effPerms = ctx.computeEffectiveChannelPermissions(curGid, resolvedTarget, ctx.getTreeChannels());

    if (isLurkerGuild) {
      return {
        ...resolvedTarget,
        canSendMessages: false,
        readOnly: true,
        permissionDeniedReason:
          "Режим предпросмотра сервера (вы не участник) — вступите на сервер, чтобы отправлять сообщения и ставить реакции.",
      };
    }
    if (isPendingRules) {
      return {
        ...resolvedTarget,
        canSendMessages: false,
        readOnly: true,
        permissionDeniedReason: "Примите правила сервера в Discord, чтобы отправлять сообщения.",
      };
    }
    if (effPerms !== null) {
      const PERM_SEND_MESSAGES = 1n << 11n;
      const canSend = (effPerms & PERM_SEND_MESSAGES) !== 0n;
      if (!canSend) {
        return {
          ...resolvedTarget,
          canSendMessages: false,
          readOnly: true,
          permissionDeniedReason: "У вас нет прав на отправку сообщений в этом канале.",
        };
      }
      return {
        ...resolvedTarget,
        canSendMessages: true,
        readOnly: false,
        permissionDeniedReason: undefined,
      };
    }
    return resolvedTarget;
  }

  /**
   * Render or update the lurker guild preview join banner above `#composer`.
   * @param {any} existingBanner
   * @param {HTMLElement} compForm
   * @param {any} compParent
   * @param {string} curGid
   * @param {any} guildObjNow
   * @param {any} rawTarget
   */
  function renderLurkerPreviewBanner(
    existingBanner,
    compForm,
    compParent,
    curGid,
    guildObjNow,
    rawTarget,
  ) {
    const banner = existingBanner || document.createElement("div");
    banner.id = "guild-preview-join-banner";
    banner.setAttribute("data-access-banner", "lurker");
    banner.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;gap:12px;padding:10px 16px;margin:0 16px 8px;border-radius:8px;background:rgba(88,101,242,0.18);border:1px solid rgba(88,101,242,0.45);color:var(--text-normal,#f2f3f5);font-size:13px;";
    banner.textContent = "";

    const label = document.createElement("span");
    label.textContent =
      "Вы просматриваете этот сервер в режиме предпросмотра. Вступите на сервер, чтобы писать в чат и заходить в голосовые каналы.";
    const actions = document.createElement("div");
    actions.style.cssText = "display:flex;gap:8px;flex-shrink:0;";

    const joinBtn = document.createElement("button");
    joinBtn.type = "button";
    joinBtn.className = "btn-primary";
    joinBtn.style.cssText =
      "padding:6px 12px;border-radius:4px;border:none;background:#5865f2;color:#fff;font-weight:600;cursor:pointer;";
    joinBtn.textContent = "Присоединиться к серверу";
    joinBtn.addEventListener("click", () => {
      joinBtn.disabled = true;
      joinBtn.textContent = "Вход на сервер…";
      void ctx
        .invoke("join_guild", { guildId: curGid })
        .then(() =>
          ctx.invoke("voice_set_audio_device", {
            kind: "dm_friends_sync",
            deviceId: "",
          }),
        )
        .then((/** @type {any} */ syncSnap) => {
          const gAfter = Array.isArray(syncSnap?.guilds)
            ? syncSnap.guilds.find((/** @type {any} */ g) => String(g?.id ?? "") === curGid)
            : null;
          if (gAfter && gAfter.lurker !== true && gAfter.isMember !== false) {
            guildObjNow.lurker = false;
            guildObjNow.isMember = true;
            guildObjNow.is_member = true;
            if (Array.isArray(gAfter.myRoles)) {
              guildObjNow.myRoles = gAfter.myRoles;
              guildObjNow.my_roles = gAfter.myRoles;
            }
            banner.remove();
            ctx.showVoiceToastNotice("Вы успешно вступили на сервер!");
            void ctx.openChatChannel(rawTarget);
          } else {
            joinBtn.disabled = false;
            joinBtn.textContent = "Присоединиться к серверу";
            const addSrvBtn = /** @type {HTMLElement | null} */ (
              document.querySelector('#guild-rail [data-action="add-server"]')
            );
            addSrvBtn?.click();
            ctx.showVoiceToastNotice(
              "Discord требует подтверждение входа по коду приглашения (или капчу в браузере)",
            );
          }
        })
        .catch((/** @type {any} */ err) => {
          joinBtn.disabled = false;
          joinBtn.textContent = "Присоединиться к серверу";
          ctx.showVoiceToastNotice(
            String(err?.message || "Не удалось вступить на сервер — попробуйте по коду приглашения"),
          );
        });
    });

    actions.appendChild(joinBtn);
    banner.append(label, actions);
    if (!existingBanner) {
      compParent.insertBefore(banner, compForm);
    }
  }

  /**
   * Render or update the pending rules screening banner above `#composer`.
   * @param {any} existingPendingBanner
   * @param {HTMLElement} compForm
   * @param {any} compParent
   * @param {string} curGid
   * @param {any} guildObjNow
   */
  function renderPendingRulesBanner(
    existingPendingBanner,
    compForm,
    compParent,
    curGid,
    guildObjNow,
  ) {
    let pendingBanner = existingPendingBanner;
    if (!pendingBanner) {
      pendingBanner = document.createElement("div");
      pendingBanner.id = "guild-pending-screening-banner";
      pendingBanner.className = "rules-screening-banner";
      pendingBanner.setAttribute("role", "region");
      pendingBanner.setAttribute("aria-label", "Требуется подтверждение правил");
      pendingBanner.setAttribute("data-access-banner", "pending");
      pendingBanner.style.cssText =
        "display:flex;align-items:center;justify-content:space-between;gap:8px;padding:12px 16px;margin:0 16px 8px;border-radius:8px;background:#2b2d31;border:1px solid rgba(255,255,255,0.08);color:#ffffff;font-size:14px;line-height:1.4;";
      compParent.insertBefore(pendingBanner, compForm);
    }
    pendingBanner.replaceChildren();

    const msgSpan = document.createElement("span");
    msgSpan.className = "rules-screening-banner-text";
    msgSpan.textContent = ctx.rulesScreeningText.bannerNotice;

    const completeBtn = document.createElement("button");
    completeBtn.type = "button";
    completeBtn.className = "rules-screening-complete-btn";
    completeBtn.setAttribute("data-action", "open-rules-modal");
    completeBtn.style.cssText =
      "display:inline-flex;align-items:center;justify-content:center;padding:8px 16px;background-color:#23a55a;color:#ffffff;border:none;border-radius:4px;font-size:14px;font-weight:600;cursor:pointer;flex-shrink:0;";
    completeBtn.textContent = ctx.rulesScreeningText.bannerButton;
    completeBtn.addEventListener("click", () => {
      ctx.openRulesScreeningModal({
        guildId: curGid,
        guildName: guildObjNow?.name || ctx.getCurrentGuildInfo()?.name || "Сервер",
        rules: ctx.extractRulesFromGuild(guildObjNow),
      });
    });

    pendingBanner.append(msgSpan, completeBtn);
  }

  /**
   * Reactively recompute permissions in the open channel and channel tree without reload
   * on `GUILD_MEMBER_UPDATE`, `GUILD_ROLE_*`, or `CHANNEL_UPDATE`.
   * @param {string} [targetGuildId]
   */
  function syncActiveChannelPermissionsInDom(targetGuildId) {
    if (!ctx.isLiveSession()) return;
    const curGid = ctx.cleanDiscordStr(String(ctx.getSelectedGuildId() || ""));
    if (targetGuildId && curGid && ctx.cleanDiscordStr(targetGuildId) !== curGid) {
      return;
    }
    ctx.refreshTreeView();
    const chatChannel = ctx.getChatChannel();
    const chatChannelId = ctx.getChatChannelId();
    const rawTarget =
      ctx.getActiveOpenChannelTarget() ??
      (chatChannel ? { ...chatChannel } : chatChannelId ? { id: chatChannelId } : null);

    if (rawTarget && typeof rawTarget === "object") {
      const resolvedTarget =
        ctx.resolveChannelObject(rawTarget, ctx.getTreeChannels(), curGid) ?? rawTarget;
      const targetType = Number(resolvedTarget?.type ?? 0);
      const isDm = targetType === 1 || targetType === 3;
      const channelForPolicy = resolveChannelPolicyTarget(resolvedTarget, curGid, isDm);

      const selfUid = ctx.cleanDiscordStr(String(ctx.getSelfUserId() || ""));
      const policy = ctx.evaluateClientChannelPolicy({
        channel: channelForPolicy,
        userId: selfUid,
      });
      ctx.applyComposerChannelPolicy(policy);

      const activeVoice = ctx.getActiveVoiceConnection();
      const compInput = /** @type {HTMLTextAreaElement | null} */ (byId("composer-input"));
      if (compInput) {
        if (compInput.disabled && channelForPolicy?.permissionDeniedReason) {
          compInput.placeholder = channelForPolicy.permissionDeniedReason;
        } else if (
          (resolvedTarget?.type === 2 || resolvedTarget?.type === 13) &&
          (!activeVoice || String(activeVoice.channelId) !== String(resolvedTarget.id))
        ) {
          compInput.placeholder = "Присоединитесь к каналу, чтобы отправлять сообщения";
        } else if (!compInput.disabled && resolvedTarget?.name) {
          compInput.placeholder = isDm
            ? `Написать @${resolvedTarget.name}`
            : `Написать в #${resolvedTarget.name}`;
        }
      }

      const existingBanner = byId("guild-preview-join-banner");
      const existingPendingBanner = byId("guild-pending-screening-banner");
      const existingReadOnlyBanner = byId("channel-readonly-access-banner");
      const guildObjNow = !isDm && curGid && curGid !== "@me" ? ctx.getGuildById(curGid) : null;
      const showPreviewBanner =
        Boolean(guildObjNow) &&
        (guildObjNow.lurker === true ||
          guildObjNow.isMember === false ||
          guildObjNow.is_member === false);
      const showPendingBanner = !showPreviewBanner && Boolean(guildObjNow?.pending === true);
      const showReadOnlyBanner =
        !showPreviewBanner &&
        !showPendingBanner &&
        Boolean(guildObjNow) &&
        channelForPolicy?.canSendMessages === false;

      const compForm = byId("composer");
      const compParent = compForm?.parentElement || compForm?.parentNode;

      if (!showPreviewBanner) {
        existingBanner?.remove();
      } else if (compForm && compParent) {
        existingPendingBanner?.remove();
        existingReadOnlyBanner?.remove();
        renderLurkerPreviewBanner(
          existingBanner,
          compForm,
          compParent,
          curGid,
          guildObjNow,
          rawTarget,
        );
      }

      if (!showPendingBanner) {
        existingPendingBanner?.remove();
        if (ctx.isRulesScreeningModalOpen()) {
          ctx.closeRulesScreeningModal();
        }
      } else if (compForm && compParent) {
        existingReadOnlyBanner?.remove();
        renderPendingRulesBanner(
          existingPendingBanner,
          compForm,
          compParent,
          curGid,
          guildObjNow,
        );
      }

      if (!showReadOnlyBanner) {
        existingReadOnlyBanner?.remove();
      } else if (compForm && compParent) {
        const roBanner = existingReadOnlyBanner || document.createElement("div");
        roBanner.id = "channel-readonly-access-banner";
        roBanner.setAttribute("data-access-banner", "missing-send-messages");
        roBanner.style.cssText =
          "display:flex;align-items:center;justify-content:space-between;gap:12px;padding:8px 16px;margin:0 16px 8px;border-radius:8px;background:rgba(242,63,67,0.14);border:1px solid rgba(242,63,67,0.4);color:var(--text-normal,#f2f3f5);font-size:12px;";
        roBanner.textContent = "У вас нет прав на отправку сообщений в этом канале.";
        if (!existingReadOnlyBanner) {
          compParent.insertBefore(roBanner, compForm);
        }
      }

      const compInputEl = /** @type {HTMLTextAreaElement | HTMLInputElement | null} */ (
        document.getElementById("composer-input")
      );
      if (compInputEl) {
        if (showPreviewBanner) {
          compInputEl.disabled = true;
          compInputEl.placeholder =
            "Режим предпросмотра (только чтение). Вступите на сервер, чтобы писать в чат.";
        } else if (showPendingBanner) {
          compInputEl.disabled = true;
          compInputEl.placeholder = "Примите правила сервера в Discord для отправки сообщений.";
        } else if (showReadOnlyBanner) {
          compInputEl.disabled = true;
          compInputEl.placeholder = "У вас нет прав на отправку сообщений в этом канале.";
        } else if (!channelForPolicy?.permissionDeniedReason) {
          compInputEl.disabled = false;
          compInputEl.placeholder = isDm
            ? `Написать @${resolvedTarget?.name || ""}`
            : `Написать в #${resolvedTarget?.name || ""}`;
        }
      }
    }

    ctx.refreshLiveChatAuthorsInDom();
    const activeVoiceAfter = ctx.getActiveVoiceConnection();
    if (activeVoiceAfter && (!targetGuildId || activeVoiceAfter.guildId === curGid)) {
      ctx.renderLiveVoicePanel();
    }
  }

  /**
   * Minimize `#live-stream-stage-viewer` into the floating PiP preview (TASK-806).
   * @returns {boolean} `true` if the stage viewer was active and minimized.
   */
  function minimizeStageViewerToPip() {
    const stageViewer = document.getElementById("live-stream-stage-viewer");
    if (!stageViewer) return false;
    ctx.clearFloatingPipPosition();
    const minimizeBtn = /** @type {HTMLElement | null} */ (
      document.getElementById("stage-minimize-viewer-btn")
    );
    if (minimizeBtn) {
      minimizeBtn.click();
      return true;
    }
    const closeBtn = /** @type {HTMLElement | null} */ (
      document.getElementById("stage-close-viewer-btn")
    );
    if (closeBtn) {
      closeBtn.click();
      return true;
    }
    stageViewer.remove();
    ctx.syncFloatingStreamPipPreview();
    return true;
  }

  /**
   * Channel switch handler (TASK-806): minimizes active Stage Viewer to PiP on text channel switch.
   * @param {any} [target]
   */
  function onChannelSwitch(target) {
    const isTextChannel =
      !target ||
      target.type === undefined ||
      target.type === null ||
      target.type === 0 ||
      target.type === 1 ||
      target.type === 3 ||
      target.type === 5;
    if (isTextChannel && document.getElementById("live-stream-stage-viewer")) {
      minimizeStageViewerToPip();
    }
  }

  /**
   * Build the DM header call action bar (`📞`, `📹`, `🖥`, `👥+`, `🔗`).
   * @param {string} targetChannelId
   * @param {string} targetChannelName
   * @returns {HTMLDivElement}
   */
  function buildDmCallHeaderBar(targetChannelId, targetChannelName) {
    const dmCallBar = document.createElement("div");
    dmCallBar.id = "dm-header-call-actions";
    dmCallBar.style.cssText =
      "display:inline-flex;align-items:center;gap:6px;margin-right:8px;padding-right:8px;border-right:1px solid rgba(255,255,255,0.08);";

    /**
     * @param {string} icon
     * @param {string} title
     * @param {string} actionId
     * @param {() => void} onClick
     */
    const makeDmHdrBtn = (icon, title, actionId, onClick) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "icon-btn";
      btn.title = title;
      btn.setAttribute("aria-label", title);
      btn.setAttribute("data-action", actionId);
      btn.textContent = icon;
      btn.style.cssText =
        "width:30px;height:30px;border-radius:6px;border:none;background:transparent;color:#dbdee1;font-size:15px;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;";
      btn.addEventListener("mouseenter", () => {
        btn.style.background = "rgba(255,255,255,0.08)";
        btn.style.color = "#f2f3f5";
      });
      btn.addEventListener("mouseleave", () => {
        btn.style.background = "transparent";
        btn.style.color = "#dbdee1";
      });
      btn.addEventListener("click", onClick);
      return btn;
    };

    const isInThisDmCall = () => {
      const activeVoice = ctx.getActiveVoiceConnection();
      return Boolean(
        activeVoice &&
          ctx.normalizeVoiceGuildKey(activeVoice.guildId) === ctx.homeGuildId &&
          String(activeVoice.channelId || "") === targetChannelId,
      );
    };

    const startDmCallAndRing = () => {
      const prefs = ctx.getSelfVoicePrefs();
      if (!prefs.deafened) prefs.muted = false;
      ctx.connectToLiveVoiceChannel("", targetChannelName, {
        id: targetChannelId,
        name: targetChannelName,
      });
      void ctx
        .sendDmRest({
          method: "POST",
          url: `/channels/${targetChannelId}/call/ring`,
          body: { recipients: null },
        })
        .catch(() => {});
    };

    dmCallBar.append(
      makeDmHdrBtn("📞", "Начать голосовой звонок", "dm-call-voice", () => {
        if (!targetChannelId) return;
        if (isInThisDmCall()) {
          ctx.renderLiveDmCallStage();
          return;
        }
        startDmCallAndRing();
      }),
      makeDmHdrBtn("📹", "Начать видеозвонок", "dm-call-video", () => {
        if (!targetChannelId) return;
        const alreadyIn = isInThisDmCall();
        if (!alreadyIn) startDmCallAndRing();
        const activeVoice = ctx.getActiveVoiceConnection();
        if (activeVoice) {
          activeVoice.cameraOn = alreadyIn ? !activeVoice.cameraOn : true;
          ctx.syncVoiceMuteStateWithBackend();
          ctx.renderLiveVoicePanel();
        }
      }),
      makeDmHdrBtn("🖥", "Демонстрация экрана", "dm-call-stream", () => {
        if (!targetChannelId) return;
        if (!isInThisDmCall()) startDmCallAndRing();
        ctx.openScreenShareModal(true);
      }),
      makeDmHdrBtn("👥+", "Добавить друзей в беседу", "dm-add-recipients", () => {
        if (targetChannelId) {
          ctx.openVoiceInviteModal({
            id: targetChannelId,
            name: targetChannelName,
            type: 3,
          });
        }
        ctx.openGroupDmModal();
      }),
      makeDmHdrBtn("🔗", "Создать приглашение в группу", "dm-create-invite", () => {
        if (!targetChannelId) return;
        ctx.openVoiceInviteModal({
          id: targetChannelId,
          name: targetChannelName,
          type: 3,
        });
      }),
    );
    return dmCallBar;
  }

  /**
   * Wrap `chat.openChannel` to coordinate channel switch PiP, DM call header bar,
   * permission sync, and notifications auto-ack.
   * @param {any} chat
   */
  function installChatOpenChannelWrapper(chat) {
    const originalOpenChannel = chat.openChannel.bind(chat);
    chat.openChannel = async (/** @type {any} */ target, /** @type {any} */ controls) => {
      ctx.setActiveOpenChannelTarget(target);
      onChannelSwitch(target);
      if (ctx.isLiveSession()) {
        clearErrorState(ctx.getContentElement());
      }
      await originalOpenChannel(target, controls);
      if (ctx.isLiveSession()) {
        document.querySelector(".chat-notice")?.setAttribute("hidden", "");
        ctx.clearTyping();
        const targetType = /** @type {any} */ (target)?.type;
        const isDm = targetType === 1 || targetType === 3;
        const isVoice = targetType === 2 || targetType === 13;
        const hashNode = document.querySelector(".chat-header .hash");
        if (hashNode) {
          hashNode.textContent = isDm ? "@" : isVoice ? "🔊" : "#";
        }
        const screenTitleNode = document.querySelector(".screen-title");
        if (screenTitleNode && target?.name) {
          screenTitleNode.textContent = isDm
            ? `Личные сообщения · @${target.name}`
            : isVoice
              ? `Голосовой канал · 🔊 ${target.name}`
              : `Канал · #${target.name}`;
        }
        document.getElementById("dm-header-call-actions")?.remove();
        if (isDm) {
          const chatActionsEl = document.querySelector(".chat-header .chat-actions");
          const targetChannelId = String(target?.id ?? "").trim();
          const targetChannelName = String(target?.name || "Личные сообщения");
          if (chatActionsEl) {
            chatActionsEl.prepend(buildDmCallHeaderBar(targetChannelId, targetChannelName));
            if (targetChannelId) {
              void ctx
                .sendDmRest({
                  method: "GET",
                  url: `/channels/${targetChannelId}/call`,
                })
                .then((/** @type {any} */ res) => {
                  if (res && res.ok && res.data && typeof res.data === "object") {
                    const callData = res.data;
                    const cid = String(callData.channel_id || targetChannelId);
                    if (cid) {
                      ctx.setActiveDmCall(cid, callData);
                      if (Array.isArray(callData.voice_states) && callData.voice_states.length > 0) {
                        const currentMe = ctx.getLiveVoiceStatesForGuild("@me") ?? [];
                        const otherStates = currentMe.filter(
                          (/** @type {any} */ vs) => String(vs?.channel_id || "") !== cid,
                        );
                        for (const vs of callData.voice_states) {
                          otherStates.push({
                            ...vs,
                            guild_id: "@me",
                            channel_id: cid,
                            confirmed: true,
                          });
                        }
                        ctx.setLiveVoiceStatesForGuild("@me", otherStates);
                        ctx.setLiveVoiceStatesForGuild(ctx.homeGuildId, otherStates);
                      }
                      ctx.renderLiveDmCallStage();
                      ctx.refreshTreeView();
                    }
                  }
                })
                .catch(() => {});
            }
          }
          const chatListEl = document.getElementById("chat-list");
          if (chatListEl && !chatListEl.dataset.dmCallClickBound) {
            chatListEl.dataset.dmCallClickBound = "1";
            chatListEl.addEventListener("click", (ev) => {
              const curTarget = /** @type {any} */ (chat).getActiveTarget?.();
              const curType = Number(curTarget?.type ?? 0);
              if (curType !== 1 && curType !== 3) return;
              const curCid = String(curTarget?.id ?? "").trim();
              const curName = String(curTarget?.name || "Личные сообщения");
              if (!curCid) return;
              const row = /** @type {HTMLElement | null} */ (
                /** @type {HTMLElement | null} */ (ev.target)?.closest?.(
                  ".msg-row, .msg-body, .msg-text",
                )
              );
              if (!row) return;
              const txt = String(row.textContent || "");
              if (txt.includes("начал(а) звонок") || txt.includes("📞")) {
                const prefs = ctx.getSelfVoicePrefs();
                if (!prefs.deafened) prefs.muted = false;
                ctx.connectToLiveVoiceChannel("", curName, {
                  id: curCid,
                  name: curName,
                });
              }
            });
          }
        }
        if (targetType !== 2 && targetType !== 13) {
          ctx.clearPreviewedVoiceChannel();
        }
        ctx.renderLiveDmCallStage();
        syncActiveChannelPermissionsInDom();
      }
      const channelId = String(target?.id ?? "").trim();
      const bell = /** @type {HTMLElement | null} */ (document.querySelector('[data-action="inbox"]'));
      if (bell && channelId) {
        bell.removeAttribute("aria-disabled");
        bell.removeAttribute("title");
        bell.setAttribute("aria-haspopup", "menu");
        bell.setAttribute("aria-controls", "channel-notif-menu");
      }
      if (channelId && ctx.shouldAutoAck(channelId)) {
        const lastMsgId = ctx.getLastMessageId(channelId);
        void ctx.ackChannel(channelId, lastMsgId).catch(() => {});
      }
    };
  }

  return {
    syncActiveChannelPermissionsInDom,
    minimizeStageViewerToPip,
    onChannelSwitch,
    onSelectChannel: onChannelSwitch,
    installChatOpenChannelWrapper,
  };
}
