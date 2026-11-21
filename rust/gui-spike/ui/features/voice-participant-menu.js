// @ts-check
/**
 * Context menu for voice channel participants (stream watch, profile/moderation, volume, mute, roles).
 * Extracted from `app.js` as part of RW51-EPIC2-T2.
 * @module ui/features/voice-participant-menu
 */

/**
 * Create the voice participant context menu controller bound to the shared voice context.
 * @param {any} ctx
 */
export function createVoiceParticipantMenu(ctx) {
  /**
   * Opens the context menu for a voice channel participant.
   * @param {any} participant
   * @param {any} channel
   * @param {HTMLElement} rowEl
   * @param {MouseEvent | any} [ev]
   */
  function openVoiceParticipantContextMenu(participant, channel, rowEl, ev) {
    ctx.closeLiveVoicePopouts();
    const targetGuildId = String(
      participant?.guildId || ctx.getRailSelectedId?.() || ctx.selectedGuildId() || "",
    );
    const uid = String(participant?.userId || "");
    if (!uid) return;
    const perms = ctx.computeServerPermissionsAndHierarchy(
      targetGuildId,
      uid,
      participant?.roles ?? [],
    );
    const userState = ctx.livePerUserVoiceState.get(uid) ?? {
      volume: 100,
      localMute: false,
      serverMute: Boolean(participant?.serverMute),
      serverDeaf: Boolean(participant?.serverDeaf),
    };
    ctx.livePerUserVoiceState.set(uid, userState);

    const menu = document.createElement("div");
    menu.id = "live-voice-user-context-menu";
    menu.setAttribute("role", "menu");
    menu.setAttribute("aria-label", `Меню участника ${participant.displayName}`);
    const rect = rowEl.getBoundingClientRect();
    const clientX = typeof ev?.clientX === "number" && ev.clientX > 0 ? ev.clientX : rect.right + 6;
    const clientY = typeof ev?.clientY === "number" && ev.clientY > 0 ? ev.clientY : rect.top;
    const leftPx = Math.min(Math.max(12, clientX), Math.max(20, window.innerWidth - 290));
    const topPx = Math.min(Math.max(12, clientY), Math.max(20, window.innerHeight - 560));
    menu.style.cssText =
      `position:fixed;left:${leftPx}px;top:${topPx}px;width:256px;max-height:calc(100vh - 24px);overflow-y:auto;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:8px;padding:6px;box-shadow:0 16px 36px rgba(0,0,0,0.75);z-index:9600;display:flex;flex-direction:column;gap:2px;font-size:13px;user-select:none;`;

    /** @type {HTMLElement | null} */
    let activeSubmenu = null;
    const closeSubmenu = () => {
      if (activeSubmenu) {
        activeSubmenu.remove();
        activeSubmenu = null;
      }
    };

    const addDivider = () => {
      const hr = document.createElement("div");
      hr.style.cssText = "height:1px;background:#2b2d31;margin:4px 4px;";
      menu.append(hr);
    };

    /**
     * @param {string} label
     * @param {{
     *   danger?: boolean,
     *   rightBadge?: string,
     *   checked?: boolean,
     *   onClick?: () => void,
     *   onSubmenu?: (itemEl: HTMLElement) => HTMLElement,
     * }} [opts]
     */
    const addMenuItem = (label, opts = {}) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.setAttribute("role", "menuitem");
      const baseColor = opts.danger ? "#f23f43" : "#dbdee1";
      btn.style.cssText =
        `display:flex;align-items:center;justify-content:space-between;gap:8px;width:100%;padding:6px 8px;border:none;border-radius:4px;background:transparent;color:${baseColor};font-size:13px;font-weight:500;cursor:pointer;text-align:left;`;
      const textSpan = document.createElement("span");
      textSpan.style.cssText = "overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1;";
      textSpan.textContent = label;
      btn.append(textSpan);

      if (typeof opts.checked === "boolean") {
        const box = document.createElement("span");
        box.style.cssText =
          `width:16px;height:16px;border-radius:4px;border:1.5px solid ${opts.checked ? "#5865f2" : "#80848e"};background:${opts.checked ? "#5865f2" : "transparent"};color:#fff;display:inline-flex;align-items:center;justify-content:center;font-size:11px;font-weight:800;flex-shrink:0;`;
        box.textContent = opts.checked ? "✓" : "";
        btn.append(box);
      } else if (opts.rightBadge) {
        const badge = document.createElement("span");
        badge.style.cssText = "font-size:11px;color:#949ba4;font-weight:700;flex-shrink:0;";
        badge.textContent = opts.rightBadge;
        btn.append(badge);
      }

      btn.addEventListener("mouseenter", () => {
        btn.style.background = opts.danger ? "#da373c" : "#5865f2";
        btn.style.color = "#ffffff";
        closeSubmenu();
        if (opts.onSubmenu) {
          activeSubmenu = opts.onSubmenu(btn);
          document.body.append(activeSubmenu);
        }
      });
      btn.addEventListener("mouseleave", () => {
        btn.style.background = "transparent";
        btn.style.color = baseColor;
      });
      btn.addEventListener("click", (e) => {
        e.stopPropagation();
        if (opts.onSubmenu && !opts.onClick) {
          closeSubmenu();
          activeSubmenu = opts.onSubmenu(btn);
          document.body.append(activeSubmenu);
          return;
        }
        opts.onClick?.();
      });
      menu.append(btn);
      return btn;
    };

    const makeSubmenuContainer = (/** @type {HTMLElement} */ anchorBtn) => {
      const r = anchorBtn.getBoundingClientRect();
      const sub = document.createElement("div");
      sub.className = "live-voice-context-submenu";
      const subLeft = r.right + 240 < window.innerWidth ? r.right + 4 : Math.max(8, r.left - 236);
      const subTop = Math.min(Math.max(12, r.top - 4), Math.max(20, window.innerHeight - 360));
      sub.style.cssText =
        `position:fixed;left:${subLeft}px;top:${subTop}px;width:232px;max-height:340px;overflow-y:auto;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:8px;padding:6px;box-shadow:0 16px 36px rgba(0,0,0,0.8);z-index:9700;display:flex;flex-direction:column;gap:2px;font-size:13px;`;
      return sub;
    };

    if (participant?.streaming === true) {
      const targetChId = String(channel?.id || participant?.channelId || "");
      const connectedToTargetChannel = ctx.isVoiceChannelConnected(targetGuildId, targetChId);
      addMenuItem(
        connectedToTargetChannel
          ? "📺 Смотреть трансляцию"
          : "📺 Подключиться и смотреть трансляцию",
        {
          rightBadge: "В ЭФИРЕ",
          onClick: () => {
            closeSubmenu();
            menu.remove();
            if (!connectedToTargetChannel && targetChId) {
              ctx.connectToLiveVoiceChannel(
                targetGuildId,
                ctx.currentGuildInfo().name || "Сервер",
                { id: targetChId, name: channel?.name || "Голосовой канал" },
              );
            }
            ctx.openLiveStreamStageViewer(participant, channel);
          },
        },
      );
      addDivider();
    }

    if (!perms.isSelf && perms.canModerateMembers) {
      addMenuItem("Открыть с доступом модератора", {
        onClick: () => {
          closeSubmenu();
          menu.remove();
          ctx.openModeratorViewSidebar(targetGuildId, participant);
        },
      });
      addDivider();
    }

    addMenuItem("Профиль", {
      onClick: () => {
        closeSubmenu();
        menu.remove();
        ctx.openModeratorViewSidebar(targetGuildId, participant, { modView: false });
      },
    });
    if (perms.isSelf) {
      addMenuItem("Упомянуть", {
        onClick: () => {
          const comp = /** @type {HTMLTextAreaElement | null} */ (ctx.byId("composer-input"));
          if (comp) {
            comp.value = `${comp.value ? `${comp.value} ` : ""}@${participant.username || participant.displayName} `;
            comp.focus();
          }
          closeSubmenu();
          menu.remove();
        },
      });
    } else {
      addMenuItem("Написать сообщение", {
        onClick: () => {
          closeSubmenu();
          menu.remove();
          const comp = /** @type {HTMLTextAreaElement | null} */ (ctx.byId("composer-input"));
          if (comp) {
            comp.value = `@${participant.username || participant.displayName} `;
            comp.focus();
          }
        },
      });
      addMenuItem("Позвонить", {
        onClick: () => {
          closeSubmenu();
          menu.remove();
          ctx.connectToLiveVoiceChannel(
            targetGuildId,
            ctx.currentGuildInfo().name || "Сервер",
            channel,
          );
        },
      });
      addMenuItem("Добавить заметку", {
        onClick: () => {
          closeSubmenu();
          menu.remove();
          const note = globalThis.prompt?.(`Заметка для ${participant.displayName}:`, userState.note || "");
          if (note !== null) {
            userState.note = note;
            ctx.livePerUserVoiceState.set(uid, userState);
          }
        },
      });
      addMenuItem("Добавить никнейм друга", {
        onClick: () => {
          closeSubmenu();
          menu.remove();
          const fn = globalThis.prompt?.(
            `Никнейм друга для ${participant.displayName}:`,
            userState.friendNick || "",
          );
          if (fn !== null && fn.trim()) {
            userState.friendNick = fn.trim();
            userState.nickname = fn.trim();
            ctx.livePerUserVoiceState.set(uid, userState);
            ctx.getTree?.()?.refreshView?.();
          }
        },
      });
    }

    if (
      (perms.isSelf && perms.canChangeOwnNickname) ||
      (!perms.isSelf && perms.canManageNicknames && perms.canModifyTarget)
    ) {
      if (perms.isSelf) {
        addMenuItem("Изменить никнейм", {
          onClick: () => {
            closeSubmenu();
            menu.remove();
            const nextNick = globalThis.prompt?.("Новый никнейм на сервере:", participant.displayName);
            if (nextNick && nextNick.trim()) {
              userState.nickname = nextNick.trim();
              ctx.livePerUserVoiceState.set(uid, userState);
              ctx.getTree?.()?.refreshView?.();
            }
          },
        });
      }
    }

    addMenuItem("Пригласить на сервер", {
      rightBadge: "›",
      onSubmenu: (btn) => {
        const sub = makeSubmenuContainer(btn);
        for (const [, g] of ctx.liveGuildsById) {
          const gBtn = document.createElement("button");
          gBtn.type = "button";
          gBtn.style.cssText =
            "display:flex;align-items:center;justify-content:space-between;width:100%;padding:6px 8px;border:none;border-radius:4px;background:transparent;color:#dbdee1;font-size:13px;cursor:pointer;text-align:left;";
          gBtn.textContent = g.name || g.id;
          gBtn.addEventListener("mouseenter", () => {
            gBtn.style.background = "#5865f2";
            gBtn.style.color = "#fff";
          });
          gBtn.addEventListener("mouseleave", () => {
            gBtn.style.background = "transparent";
            gBtn.style.color = "#dbdee1";
          });
          gBtn.addEventListener("click", () => {
            gBtn.textContent = `✓ Приглашён в ${g.name}`;
            setTimeout(() => {
              closeSubmenu();
              menu.remove();
            }, 350);
          });
          sub.append(gBtn);
        }
        return sub;
      },
    });

    if (!perms.isSelf) {
      addMenuItem("Добавить в друзья", {
        onClick: () => {
          closeSubmenu();
          menu.remove();
        },
      });
    }

    addDivider();

    if (perms.isSelf) {
      addMenuItem("Откл. микрофон", {
        checked: Boolean(ctx.selfVoicePrefs.muted),
        onClick: () => {
          ctx.toggleSelfMicOnly();
          closeSubmenu();
          menu.remove();
        },
      });
      addMenuItem("Откл. звук", {
        checked: Boolean(ctx.selfVoicePrefs.deafened),
        onClick: () => {
          ctx.toggleSelfDeafenBoth();
          closeSubmenu();
          menu.remove();
        },
      });
      addMenuItem("Настроить значок канала", {
        onClick: () => {
          closeSubmenu();
          menu.remove();
        },
      });
    } else {
      addMenuItem("Откл. микрофон", {
        checked: Boolean(userState.localMute),
        onClick: () => {
          userState.localMute = !userState.localMute;
          ctx.livePerUserVoiceState.set(uid, userState);
          void ctx
            .invoke(ctx.ipcCmds.setAudioDevice, {
              kind: "user_audio_volume",
              deviceId: uid,
              muted: Boolean(userState.localMute),
              volumePercent: Number(userState.volume ?? 100),
            })
            .catch(() => {});
          ctx.getTree?.()?.refreshView?.();
          closeSubmenu();
          menu.remove();
        },
      });
      addMenuItem("Изменить голос", {
        rightBadge: "🎧",
        onClick: () => {
          closeSubmenu();
          menu.remove();
        },
      });
      const volWrap = document.createElement("div");
      volWrap.style.cssText = "padding:6px 8px;display:flex;flex-direction:column;gap:4px;";
      const volHeader = document.createElement("div");
      volHeader.style.cssText = "display:flex;justify-content:space-between;font-size:12px;color:#b5bac1;font-weight:600;";
      const volLabel = document.createElement("span");
      volLabel.textContent = "Громкость пользователя";
      const volPct = document.createElement("span");
      volPct.textContent = `${userState.volume ?? 100}%`;
      volHeader.append(volLabel, volPct);
      const volSlider = document.createElement("input");
      volSlider.type = "range";
      volSlider.min = "0";
      volSlider.max = "200";
      volSlider.value = String(userState.volume ?? 100);
      volSlider.style.cssText = "width:100%;accent-color:#5865f2;cursor:pointer;";
      volSlider.addEventListener("input", () => {
        userState.volume = Number(volSlider.value);
        volPct.textContent = `${userState.volume}%`;
        ctx.livePerUserVoiceState.set(uid, userState);
        void ctx
          .invoke(ctx.ipcCmds.setAudioDevice, {
            kind: "user_audio_volume",
            deviceId: uid,
            muted: Boolean(userState.localMute),
            volumePercent: Number(userState.volume),
          })
          .catch(() => {});
      });
      volWrap.append(volHeader, volSlider);
      menu.append(volWrap);
    }

    if (perms.canMuteMembers || perms.canDeafenMembers || perms.canMoveMembers) {
      addDivider();
      if (perms.canMuteMembers) {
        addMenuItem("Откл. микрофон на сервере", {
          checked: Boolean(userState.serverMute),
          onClick: () => {
            userState.serverMute = !userState.serverMute;
            ctx.livePerUserVoiceState.set(uid, userState);
            const gList = ctx.liveVoiceStatesByGuild.get(targetGuildId) ?? [];
            for (const vs of gList) {
              if (String(vs?.user_id ?? vs?.userId ?? "") === uid) {
                vs.mute = userState.serverMute;
              }
            }
            ctx.getTree?.()?.refreshView?.();
            closeSubmenu();
            menu.remove();
          },
        });
      }
      if (perms.canDeafenMembers) {
        addMenuItem("Сервер: откл. звук", {
          checked: Boolean(userState.serverDeaf),
          onClick: () => {
            userState.serverDeaf = !userState.serverDeaf;
            ctx.livePerUserVoiceState.set(uid, userState);
            const gList = ctx.liveVoiceStatesByGuild.get(targetGuildId) ?? [];
            for (const vs of gList) {
              if (String(vs?.user_id ?? vs?.userId ?? "") === uid) {
                vs.deaf = userState.serverDeaf;
              }
            }
            ctx.getTree?.()?.refreshView?.();
            closeSubmenu();
            menu.remove();
          },
        });
      }
      if (perms.canMoveMembers) {
        addMenuItem("Отключить", {
          danger: true,
          onClick: () => {
            if (perms.isSelf) {
              ctx.disconnectFromLiveVoiceChannel();
            } else {
              const gList = (ctx.liveVoiceStatesByGuild.get(targetGuildId) ?? []).filter(
                (/** @type {any} */ vs) => String(vs?.user_id ?? vs?.userId ?? "") !== uid,
              );
              ctx.liveVoiceStatesByGuild.set(targetGuildId, gList);
              ctx.getTree?.()?.refreshView?.();
            }
            closeSubmenu();
            menu.remove();
          },
        });
        addMenuItem("Переместить в", {
          rightBadge: "›",
          onSubmenu: (btn) => {
            const sub = makeSubmenuContainer(btn);
            const treeChannels = ctx.getTreeChannels?.() ?? [];
            const voiceChannels = treeChannels.filter(
              (/** @type {any} */ ch) => ch.type === 2 && ch.id !== channel.id,
            );
            for (const vch of voiceChannels) {
              const chBtn = document.createElement("button");
              chBtn.type = "button";
              chBtn.style.cssText =
                "display:flex;align-items:center;gap:6px;width:100%;padding:6px 8px;border:none;border-radius:4px;background:transparent;color:#dbdee1;font-size:13px;cursor:pointer;text-align:left;";
              chBtn.textContent = vch.name || vch.id;
              chBtn.addEventListener("mouseenter", () => {
                chBtn.style.background = "#5865f2";
                chBtn.style.color = "#fff";
              });
              chBtn.addEventListener("mouseleave", () => {
                chBtn.style.background = "transparent";
                chBtn.style.color = "#dbdee1";
              });
              chBtn.addEventListener("click", () => {
                if (perms.isSelf) {
                  ctx.connectToLiveVoiceChannel(
                    targetGuildId,
                    ctx.currentGuildInfo().name || "Сервер",
                    vch,
                  );
                } else {
                  const gList = ctx.liveVoiceStatesByGuild.get(targetGuildId) ?? [];
                  for (const vs of gList) {
                    if (String(vs?.user_id ?? vs?.userId ?? "") === uid) {
                      vs.channel_id = vch.id;
                      vs.channelId = vch.id;
                    }
                  }
                  ctx.getTree?.()?.refreshView?.();
                }
                closeSubmenu();
                menu.remove();
              });
              sub.append(chBtn);
            }
            return sub;
          },
        });
      }
    }

    const showMemberModSection =
      perms.canManageRoles ||
      (!perms.isSelf &&
        perms.canModifyTarget &&
        (perms.canModerateMembers || perms.canManageNicknames || perms.canKickMembers || perms.canBanMembers));
    if (showMemberModSection) {
      addDivider();
      if (!perms.isSelf && perms.canModerateMembers && perms.canModifyTarget) {
        addMenuItem("Подтвердить участника", {
          checked: Boolean(userState.verified),
          onClick: () => {
            userState.verified = !userState.verified;
            ctx.livePerUserVoiceState.set(uid, userState);
            closeSubmenu();
            menu.remove();
          },
        });
      }
      if (!perms.isSelf && perms.canManageNicknames && perms.canModifyTarget) {
        addMenuItem("Изменить никнейм", {
          onClick: () => {
            closeSubmenu();
            menu.remove();
            const nextNick = globalThis.prompt?.(
              `Изменить никнейм ${participant.displayName}:`,
              participant.displayName,
            );
            if (nextNick && nextNick.trim()) {
              userState.nickname = nextNick.trim();
              ctx.livePerUserVoiceState.set(uid, userState);
              ctx.getTree?.()?.refreshView?.();
            }
          },
        });
      }
      if (perms.canManageRoles && perms.assignableRoles.length > 0) {
        addMenuItem("Роли", {
          rightBadge: "›",
          onSubmenu: (btn) => {
            const sub = makeSubmenuContainer(btn);
            const currentRoleSet = new Set(
              ctx.liveMemberRolesByGuildAndUser?.get(`${targetGuildId}:${uid}`) ?? perms.targetRoles,
            );
            for (const rInfo of perms.assignableRoles) {
              const canToggleRole =
                perms.isOwner ||
                perms.isAdmin ||
                (perms.canManageRoles && rInfo.position < perms.selfTopPosition);
              const rBtn = document.createElement("button");
              rBtn.type = "button";
              rBtn.style.cssText =
                `display:flex;align-items:center;justify-content:space-between;gap:8px;width:100%;padding:6px 8px;border:none;border-radius:4px;background:transparent;color:#dbdee1;font-size:13px;cursor:${canToggleRole ? "pointer" : "not-allowed"};opacity:${canToggleRole ? "1" : "0.55"};text-align:left;`;
              const rColor = rInfo.color ? `#${rInfo.color.toString(16).padStart(6, "0")}` : "#949ba4";
              const leftSpan = document.createElement("span");
              leftSpan.style.cssText = "display:inline-flex;align-items:center;gap:8px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
              leftSpan.innerHTML = `<span style="width:10px;height:10px;border-radius:50%;background:${rColor};flex-shrink:0;"></span><span>${rInfo.name}</span>`;
              const checkBox = document.createElement("span");
              const renderRoleCheck = () => {
                const hasRole = currentRoleSet.has(rInfo.id);
                checkBox.style.cssText =
                  `width:16px;height:16px;border-radius:4px;border:1.5px solid ${hasRole ? "#5865f2" : "#80848e"};background:${hasRole ? "#5865f2" : "transparent"};color:#fff;display:inline-flex;align-items:center;justify-content:center;font-size:11px;font-weight:800;flex-shrink:0;`;
                checkBox.textContent = hasRole ? "✓" : "";
              };
              renderRoleCheck();
              rBtn.append(leftSpan, checkBox);
              rBtn.addEventListener("click", (e) => {
                e.stopPropagation();
                if (!canToggleRole) return;
                if (currentRoleSet.has(rInfo.id)) currentRoleSet.delete(rInfo.id);
                else currentRoleSet.add(rInfo.id);
                ctx.liveMemberRolesByGuildAndUser?.set(`${targetGuildId}:${uid}`, [...currentRoleSet]);
                renderRoleCheck();
              });
              sub.append(rBtn);
            }
            return sub;
          },
        });
      }
      if (!perms.isSelf && perms.canModerateMembers && perms.canModifyTarget) {
        addMenuItem("Отправить в тайм-аут", {
          danger: true,
          onClick: () => {
            userState.timedOut = true;
            ctx.livePerUserVoiceState.set(uid, userState);
            closeSubmenu();
            menu.remove();
            ctx.openModeratorViewSidebar(targetGuildId, participant);
          },
        });
      }
      if (!perms.isSelf && perms.canKickMembers && perms.canModifyTarget) {
        addMenuItem("Выгнать", {
          danger: true,
          onClick: () => {
            closeSubmenu();
            menu.remove();
            ctx.openModeratorViewSidebar(targetGuildId, participant);
          },
        });
      }
      if (!perms.isSelf && perms.canBanMembers && perms.canModifyTarget) {
        addMenuItem("Забанить", {
          danger: true,
          onClick: () => {
            closeSubmenu();
            menu.remove();
            ctx.openModeratorViewSidebar(targetGuildId, participant);
          },
        });
      }
    }

    if (!perms.isSelf) {
      addDivider();
      addMenuItem("Игнорировать", {
        onClick: () => {
          closeSubmenu();
          menu.remove();
        },
      });
      addMenuItem("Заблокировать", {
        danger: true,
        onClick: () => {
          closeSubmenu();
          menu.remove();
        },
      });
      addMenuItem("Пожаловаться на профиль", {
        danger: true,
        onClick: () => {
          closeSubmenu();
          menu.remove();
        },
      });
    }

    addDivider();
    addMenuItem("Копировать ID пользователя", {
      rightBadge: "ID",
      onClick: () => {
        void navigator.clipboard?.writeText(uid).catch(() => {});
        closeSubmenu();
        menu.remove();
      },
    });

    const onOutside = (/** @type {MouseEvent} */ e) => {
      const t = /** @type {Node | null} */ (e.target);
      if (t && (menu.contains(t) || (activeSubmenu && activeSubmenu.contains(t)))) return;
      closeSubmenu();
      menu.remove();
      document.removeEventListener("mousedown", onOutside, true);
      if (ctx.activeVoiceMenuOutsideHandler === onOutside) {
        ctx.activeVoiceMenuOutsideHandler = null;
      }
    };
    ctx.activeVoiceMenuOutsideHandler = onOutside;
    setTimeout(() => {
      if (menu.isConnected) {
        document.addEventListener("mousedown", onOutside, true);
      }
    }, 0);
    document.body.append(menu);
  }

  return {
    openVoiceParticipantContextMenu,
  };
}
