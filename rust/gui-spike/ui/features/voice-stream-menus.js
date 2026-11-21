// @ts-check

/**
 * Stream settings menus and portal picker: Nitro HD promo modal,
 * stream settings cascading menu, and screen/window capture portal picker.
 */

/**
 * Create the stream menus controller bound to the shared voice call context.
 * @param {any} ctx
 */
export function createVoiceStreamMenus(ctx) {
  /**
   * Nitro HD promo modal ("Catch every moment in HD").
   */
  function openNitroHdModal() {
    document.getElementById("live-nitro-hd-modal")?.remove();
    const backdrop = document.createElement("div");
    backdrop.id = "live-nitro-hd-modal";
    backdrop.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.78);display:flex;align-items:center;justify-content:center;z-index:10020;";
    const card = document.createElement("div");
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.setAttribute("aria-label", "Ловите каждый момент в разрешении HD");
    card.style.cssText =
      "width:460px;max-width:94vw;border-radius:14px;overflow:hidden;background:linear-gradient(180deg,#6d28d9 0%,#9333ea 42%,#232428 42%);color:#f2f3f5;box-shadow:0 24px 64px rgba(0,0,0,0.85);display:flex;flex-direction:column;font-family:inherit;position:relative;";

    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText =
      "position:absolute;top:12px;right:14px;width:28px;height:28px;border-radius:50%;border:none;background:rgba(0,0,0,0.35);color:#fff;font-size:15px;cursor:pointer;";
    closeBtn.addEventListener("click", () => backdrop.remove());

    const hero = document.createElement("div");
    hero.style.cssText = "padding:24px 24px 18px;text-align:center;display:flex;flex-direction:column;align-items:center;gap:8px;";
    const badge = document.createElement("div");
    badge.style.cssText =
      "padding:6px 14px;border-radius:10px;background:rgba(255,255,255,0.18);backdrop-filter:blur(6px);font-size:20px;font-weight:900;letter-spacing:0.04em;color:#fff;border:1px solid rgba(255,255,255,0.35);";
    badge.textContent = "✦ HD · 4K 60 FPS";
    const hTitle = document.createElement("h3");
    hTitle.style.cssText = "margin:8px 0 0;font-size:21px;font-weight:800;color:#ffffff;line-height:1.25;";
    hTitle.textContent = "Ловите каждый момент в разрешении HD";
    const hSub = document.createElement("div");
    hSub.style.cssText = "font-size:13px;color:rgba(255,255,255,0.88);line-height:1.4;";
    hSub.textContent = "Стримьте в разрешении до 4K и с частотой 60 кадров/с с подпиской Nitro.";
    hero.append(badge, hTitle, hSub);

    const body = document.createElement("div");
    body.style.cssText = "padding:18px 20px 20px;background:#232428;display:flex;flex-direction:column;gap:12px;";
    const bodyLabel = document.createElement("div");
    bodyLabel.style.cssText = "font-size:13px;font-weight:700;color:#dbdee1;";
    bodyLabel.textContent = "Выберите план:";

    const plans = [
      { title: "Годовая подписка", sub: "Вы сэкономите 16%", price: "$99.99 / год", selected: true },
      { title: "Месячная подписка", sub: "", price: "$9.99 / месяц", selected: false },
    ];
    for (const p of plans) {
      const row = document.createElement("label");
      row.style.cssText =
        `display:flex;align-items:center;justify-content:space-between;padding:12px 14px;border-radius:10px;border:1.5px solid ${p.selected ? "#a855f7" : "#3f4147"};background:#2b2d31;cursor:pointer;`;
      row.innerHTML = `<span style="display:flex;align-items:center;gap:10px;"><input type="radio" name="nitro-hd-plan" ${p.selected ? "checked" : ""} style="accent-color:#a855f7;width:16px;height:16px;"/><span><div style="font-size:14px;font-weight:700;color:#f2f3f5;">${p.title}</div>${p.sub ? `<div style="font-size:11px;color:#c084fc;">${p.sub}</div>` : ""}</span></span><span style="font-size:15px;font-weight:800;color:#f2f3f5;">${p.price}</span>`;
      body.append(row);
    }

    const totalRow = document.createElement("div");
    totalRow.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;padding:10px 14px;border-radius:8px;background:#1e1f22;font-size:14px;font-weight:700;";
    totalRow.innerHTML = `<span>Итого</span><span>$99.99 / год</span>`;

    const actionBtn = document.createElement("button");
    actionBtn.type = "button";
    actionBtn.style.cssText =
      "width:100%;padding:11px 16px;border-radius:8px;border:none;background:#5865f2;color:#ffffff;font-size:14px;font-weight:700;cursor:pointer;";
    actionBtn.textContent = "Разблокировать HD (1080p / 1440p 60 FPS)";
    actionBtn.addEventListener("click", () => {
      const activeVoiceConnection = ctx.getActiveVoiceConnection();
      if (activeVoiceConnection) {
        /** @type {any} */ (activeVoiceConnection).streamQualityTier = "HD";
        /** @type {any} */ (activeVoiceConnection).streamResolution = "1080p";
        /** @type {any} */ (activeVoiceConnection).streamFps = 60;
      }
      backdrop.remove();
    });

    body.prepend(bodyLabel);
    body.append(totalRow, actionBtn);
    card.append(closeBtn, hero, body);
    backdrop.append(card);
    backdrop.addEventListener("click", (ev) => {
      if (ev.target === backdrop) backdrop.remove();
    });
    document.body.append(backdrop);
  }

  /**
   * Show the "Stream Mode" settings context menu anchored to the gear button.
   * @param {HTMLElement} anchorBtn
   * @param {any} stateBag
   * @param {() => void} onStateChange
   */
  function openStreamSettingsMenu(anchorBtn, stateBag, onStateChange) {
    const existing = document.getElementById("live-stream-settings-menu");
    if (existing) {
      existing.remove();
      document.getElementById("live-stream-settings-submenu")?.remove();
      return;
    }
    const rect = anchorBtn.getBoundingClientRect();
    const menu = document.createElement("div");
    menu.id = "live-stream-settings-menu";
    menu.setAttribute("role", "menu");
    menu.setAttribute("aria-label", "Режим стрима");
    const leftPx = Math.min(Math.max(12, Math.round(rect.left - 80)), window.innerWidth - 310);
    const bottomPx = Math.max(16, Math.round(window.innerHeight - rect.top + 8));
    const verticalPosCss =
      rect.top > 0 && rect.top < 320
        ? `top:${Math.max(12, Math.round(rect.bottom + 8))}px;`
        : `bottom:${bottomPx}px;`;
    menu.style.cssText =
      `position:fixed;left:${leftPx}px;${verticalPosCss}width:288px;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:10px;padding:8px;box-shadow:0 18px 44px rgba(0,0,0,0.82);z-index:10010;display:flex;flex-direction:column;gap:2px;font-size:13px;user-select:none;`;

    const closeSubmenu = () => {
      document.getElementById("live-stream-settings-submenu")?.remove();
    };

    const syncDynamicStreamQuality = () => {
      const resStr = String(stateBag.resolution || stateBag.streamResolution || "720p");
      const fpsVal = Number(stateBag.fps || stateBag.streamFps || 60);
      const width =
        resStr === "1080p" ? 1920 : resStr === "1440p" ? 2560 : resStr === "480p" ? 854 : resStr === "Исходное" ? 1920 : 1280;
      const height =
        resStr === "1080p" ? 1080 : resStr === "1440p" ? 1440 : resStr === "480p" ? 480 : resStr === "Исходное" ? 1080 : 720;
      const frameRate = fpsVal > 0 ? fpsVal : 60;
      stateBag.width = width;
      stateBag.height = height;
      stateBag.frameRate = frameRate;
      const rawSrc = String(stateBag.streamSourceId || ctx.getActiveVoiceConnection()?.streamSourceId || "-1").replace(/^screens?:/, "");
      const isWin = rawSrc.startsWith("{") || rawSrc.startsWith("win") || stateBag.streamPortalKind === "windows";
      const kindPrefix = isWin ? "windows" : "display";
      void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "set_capture_source",
        deviceId: `${kindPrefix}:${rawSrc}:${width}x${height}@${frameRate}`,
        width,
        height,
        frameRate,
      }).catch(() => {});
    };

    const renderMenu = () => {
      menu.replaceChildren();
      closeSubmenu();

      const changeSourceRow = document.createElement("button");
      changeSourceRow.type = "button";
      changeSourceRow.setAttribute("role", "menuitem");
      changeSourceRow.style.cssText =
        "display:flex;align-items:center;gap:10px;width:100%;padding:7px 8px;border:none;border-radius:6px;background:transparent;color:#f2f3f5;cursor:pointer;text-align:left;border-bottom:1px solid #2b2d31;margin-bottom:4px;";
      changeSourceRow.innerHTML = `<span style="font-size:14px;">🖥</span><div style="font-size:13px;font-weight:600;">Сменить окно или экран…</div>`;
      changeSourceRow.addEventListener("mouseenter", () => {
        changeSourceRow.style.background = "#2b2d31";
        closeSubmenu();
      });
      changeSourceRow.addEventListener("mouseleave", () => {
        changeSourceRow.style.background = "transparent";
      });
      changeSourceRow.addEventListener("click", (ev) => {
        ev.stopPropagation();
        menu.remove();
        closeSubmenu();
        openScreenSharePortalPicker("windows", ctx.switchLiveStreamSource);
      });
      menu.append(changeSourceRow);

      const hdr = document.createElement("div");
      hdr.style.cssText = "padding:4px 8px 6px;font-size:12px;font-weight:700;color:#949ba4;";
      hdr.textContent = "Режим стрима";
      menu.append(hdr);

      const presets = [
        { id: "games", title: "Игры", sub: "До 60 кадров в секунду", res: "720p", fps: 60 },
        { id: "screen", title: "Демонстрация экрана", sub: "Оптимальная чёткость текста", res: "1080p", fps: 30 },
        { id: "custom", title: "Пользовательские", sub: "например, 720p 60fps", res: stateBag.resolution || "720p", fps: stateBag.fps || 60 },
      ];

      for (const p of presets) {
        const active = (stateBag.preset || "games") === p.id;
        const row = document.createElement("button");
        row.type = "button";
        row.setAttribute("role", "menuitemradio");
        row.setAttribute("aria-checked", active ? "true" : "false");
        row.style.cssText =
          "display:flex;align-items:center;justify-content:space-between;gap:10px;width:100%;padding:7px 8px;border:none;border-radius:6px;background:transparent;color:#f2f3f5;cursor:pointer;text-align:left;";
        row.addEventListener("mouseenter", () => {
          row.style.background = "#2b2d31";
          closeSubmenu();
        });
        row.addEventListener("mouseleave", () => {
          row.style.background = "transparent";
        });
        const txtCol = document.createElement("div");
        txtCol.innerHTML = `<div style="font-size:13px;font-weight:600;color:#f2f3f5;">${p.title}</div><div style="font-size:11px;color:#949ba4;margin-top:1px;">${p.sub}</div>`;
        const radio = document.createElement("span");
        radio.style.cssText =
          `width:18px;height:18px;border-radius:50%;border:2px solid ${active ? "#5865f2" : "#80848e"};display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;`;
        if (active) {
          radio.innerHTML = `<span style="width:10px;height:10px;border-radius:50%;background:#5865f2;display:block;"></span>`;
        }
        row.append(txtCol, radio);
        row.addEventListener("click", (ev) => {
          ev.stopPropagation();
          stateBag.preset = p.id;
          if (p.id !== "custom") {
            stateBag.resolution = p.res;
            stateBag.streamResolution = p.res;
            stateBag.fps = p.fps;
            stateBag.streamFps = p.fps;
            stateBag.qualityTier = p.res === "720p" || p.res === "480p" ? "SD" : "HD";
          }
          syncDynamicStreamQuality();
          onStateChange();
          renderMenu();
        });
        menu.append(row);
      }

      const openCascadingSubmenu = (
        /** @type {HTMLElement} */ itemEl,
        /** @type {Array<{ label: string, sub?: string, value: any, nitro?: boolean }>} */ items,
        /** @type {any} */ currentVal,
        /** @type {(val: any) => void} */ onPick,
      ) => {
        closeSubmenu();
        const itemRect = itemEl.getBoundingClientRect();
        const sub = document.createElement("div");
        sub.id = "live-stream-settings-submenu";
        sub.setAttribute("role", "menu");
        const subLeft = Math.min(itemRect.right + 6, window.innerWidth - 210);
        const subTop = Math.max(12, Math.min(itemRect.top - 8, window.innerHeight - 220));
        sub.style.cssText =
          `position:fixed;left:${subLeft}px;top:${subTop}px;width:196px;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:10px;padding:6px;box-shadow:0 16px 38px rgba(0,0,0,0.85);z-index:10015;display:flex;flex-direction:column;gap:2px;font-size:13px;`;
        for (const opt of items) {
          const checked = opt.value === currentVal;
          const b = document.createElement("button");
          b.type = "button";
          b.setAttribute("role", "menuitemradio");
          b.setAttribute("aria-checked", checked ? "true" : "false");
          b.style.cssText =
            "display:flex;align-items:center;justify-content:space-between;gap:8px;width:100%;padding:7px 8px;border:none;border-radius:6px;background:transparent;color:#f2f3f5;cursor:pointer;text-align:left;";
          b.addEventListener("mouseenter", () => {
            b.style.background = "#2b2d31";
          });
          b.addEventListener("mouseleave", () => {
            b.style.background = "transparent";
          });
          const left = document.createElement("div");
          left.innerHTML = `<div style="font-size:13px;font-weight:600;color:#f2f3f5;display:flex;align-items:center;gap:5px;"><span>${opt.label}</span>${opt.nitro ? `<span style="color:#c084fc;font-size:12px;">✦</span>` : ""}</div>${opt.sub ? `<div style="font-size:11px;color:#949ba4;">${opt.sub}</div>` : ""}`;
          const rad = document.createElement("span");
          rad.style.cssText =
            `width:18px;height:18px;border-radius:50%;border:2px solid ${checked ? "#5865f2" : "#80848e"};display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;`;
          if (checked) {
            rad.innerHTML = `<span style="width:10px;height:10px;border-radius:50%;background:#5865f2;display:block;"></span>`;
          }
          b.append(left, rad);
          b.addEventListener("click", (ev) => {
            ev.stopPropagation();
            onPick(opt.value);
            syncDynamicStreamQuality();
            onStateChange();
            renderMenu();
          });
          sub.append(b);
        }
        document.body.append(sub);
      };

      if ((stateBag.preset || "games") === "custom") {
        const resCascade = document.createElement("button");
        resCascade.type = "button";
        resCascade.style.cssText =
          "display:flex;align-items:center;justify-content:space-between;width:100%;padding:7px 8px;border:none;border-radius:6px;background:transparent;color:#f2f3f5;cursor:pointer;text-align:left;";
        resCascade.innerHTML = `<div><div style="font-size:13px;font-weight:600;">Разрешение экрана</div><div style="font-size:11px;color:#949ba4;">${stateBag.resolution || "720p"}</div></div><span style="color:#949ba4;font-size:16px;">›</span>`;
        const showResSub = () => {
          resCascade.style.background = "#2b2d31";
          openCascadingSubmenu(
            resCascade,
            [
              { label: "480p", value: "480p" },
              { label: "720p", value: "720p" },
              { label: "1080p", nitro: true, value: "1080p" },
              { label: "1440p", nitro: true, value: "1440p" },
              { label: "Исходное", sub: "Макс. до 4K", nitro: true, value: "Исходное" },
            ],
            stateBag.resolution || stateBag.streamResolution || "720p",
            (v) => {
              stateBag.resolution = v;
              stateBag.streamResolution = v;
              stateBag.qualityTier = v === "720p" || v === "480p" ? "SD" : "HD";
            },
          );
        };
        resCascade.addEventListener("mouseenter", showResSub);
        resCascade.addEventListener("click", (ev) => {
          ev.stopPropagation();
          showResSub();
        });

        const fpsCascade = document.createElement("button");
        fpsCascade.type = "button";
        fpsCascade.style.cssText =
          "display:flex;align-items:center;justify-content:space-between;width:100%;padding:7px 8px;border:none;border-radius:6px;background:transparent;color:#f2f3f5;cursor:pointer;text-align:left;";
        fpsCascade.innerHTML = `<div><div style="font-size:13px;font-weight:600;">Частота кадров</div><div style="font-size:11px;color:#949ba4;">${stateBag.fps || stateBag.streamFps || 60}</div></div><span style="color:#949ba4;font-size:16px;">›</span>`;
        const showFpsSub = () => {
          fpsCascade.style.background = "#2b2d31";
          openCascadingSubmenu(
            fpsCascade,
            [
              { label: "15", value: 15 },
              { label: "30", value: 30 },
              { label: "60", nitro: true, value: 60 },
            ],
            Number(stateBag.fps || stateBag.streamFps || 60),
            (v) => {
              stateBag.fps = Number(v);
              stateBag.streamFps = Number(v);
            },
          );
        };
        fpsCascade.addEventListener("mouseenter", showFpsSub);
        fpsCascade.addEventListener("click", (ev) => {
          ev.stopPropagation();
          showFpsSub();
        });
        menu.append(resCascade, fpsCascade);
      }

      const div1 = document.createElement("div");
      div1.style.cssText = "height:1px;background:#2b2d31;margin:4px 4px;";
      menu.append(div1);

      const muteAudioRow = document.createElement("button");
      muteAudioRow.type = "button";
      muteAudioRow.setAttribute("role", "menuitemcheckbox");
      const isAudioMuted = stateBag.streamAudio === false;
      muteAudioRow.setAttribute("aria-checked", isAudioMuted ? "true" : "false");
      muteAudioRow.style.cssText =
        "display:flex;align-items:center;justify-content:space-between;gap:10px;width:100%;padding:7px 8px;border:none;border-radius:6px;background:transparent;color:#f2f3f5;cursor:pointer;text-align:left;";
      muteAudioRow.addEventListener("mouseenter", () => {
        muteAudioRow.style.background = "#2b2d31";
        closeSubmenu();
      });
      muteAudioRow.addEventListener("mouseleave", () => {
        muteAudioRow.style.background = "transparent";
      });
      muteAudioRow.innerHTML = `<div><div style="font-size:13px;font-weight:600;">Заглушить аудио стрима</div><div style="font-size:11px;color:#949ba4;">Убирает звук из вашего стрима</div></div><span style="width:18px;height:18px;border-radius:4px;border:1.5px solid ${isAudioMuted ? "#5865f2" : "#80848e"};background:${isAudioMuted ? "#5865f2" : "transparent"};color:#fff;display:inline-flex;align-items:center;justify-content:center;font-size:12px;font-weight:800;flex-shrink:0;">${isAudioMuted ? "✓" : ""}</span>`;
      muteAudioRow.addEventListener("click", (ev) => {
        ev.stopPropagation();
        stateBag.streamAudio = !isAudioMuted;
        onStateChange();
        renderMenu();
      });
      menu.append(muteAudioRow);

      const div2 = document.createElement("div");
      div2.style.cssText = "height:1px;background:#2b2d31;margin:4px 4px;";
      menu.append(div2);

      const advRow = document.createElement("button");
      advRow.type = "button";
      advRow.style.cssText =
        "display:flex;align-items:center;justify-content:space-between;width:100%;padding:7px 8px;border:none;border-radius:6px;background:transparent;color:#f2f3f5;cursor:pointer;text-align:left;";
      advRow.innerHTML = `<span style="font-size:13px;font-weight:600;">Расширенные</span><span style="color:#949ba4;font-size:16px;">›</span>`;
      const showAdvSub = () => {
        advRow.style.background = "#2b2d31";
        closeSubmenu();
        const r = advRow.getBoundingClientRect();
        const sub = document.createElement("div");
        sub.id = "live-stream-settings-submenu";
        sub.setAttribute("role", "menu");
        const subLeft = Math.min(r.right + 6, window.innerWidth - 250);
        const subTop = Math.max(12, Math.min(r.top - 4, window.innerHeight - 90));
        sub.style.cssText =
          `position:fixed;left:${subLeft}px;top:${subTop}px;width:236px;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:10px;padding:6px;box-shadow:0 16px 38px rgba(0,0,0,0.85);z-index:10015;display:flex;flex-direction:column;gap:2px;font-size:13px;`;
        const hidePrev = Boolean(stateBag.hideStreamPreview);
        const chkBtn = document.createElement("button");
        chkBtn.type = "button";
        chkBtn.setAttribute("role", "menuitemcheckbox");
        chkBtn.setAttribute("aria-checked", hidePrev ? "true" : "false");
        chkBtn.style.cssText =
          "display:flex;align-items:center;justify-content:space-between;gap:10px;width:100%;padding:7px 8px;border:none;border-radius:6px;background:transparent;color:#f2f3f5;cursor:pointer;text-align:left;";
        chkBtn.innerHTML = `<span style="font-size:13px;font-weight:600;">Скрыть предпросмотр стрима</span><span style="width:18px;height:18px;border-radius:4px;border:1.5px solid ${hidePrev ? "#5865f2" : "#80848e"};background:${hidePrev ? "#5865f2" : "transparent"};color:#fff;display:inline-flex;align-items:center;justify-content:center;font-size:12px;font-weight:800;flex-shrink:0;">${hidePrev ? "✓" : ""}</span>`;
        chkBtn.addEventListener("click", (ev) => {
          ev.stopPropagation();
          stateBag.hideStreamPreview = !hidePrev;
          onStateChange();
          showAdvSub();
        });
        sub.append(chkBtn);
        document.body.append(sub);
      };
      advRow.addEventListener("mouseenter", showAdvSub);
      advRow.addEventListener("click", (ev) => {
        ev.stopPropagation();
        showAdvSub();
      });
      menu.append(advRow);
    };

    renderMenu();
    const onOutside = (/** @type {MouseEvent} */ e) => {
      const t = /** @type {Node | null} */ (e.target);
      const activeSubmenu = document.getElementById("live-stream-settings-submenu");
      if (t && (menu.contains(t) || anchorBtn.contains(t) || (activeSubmenu && activeSubmenu.contains(t)))) return;
      closeSubmenu();
      menu.remove();
      document.removeEventListener?.("mousedown", onOutside, true);
      if (ctx.getActiveVoiceMenuOutsideHandler() === onOutside) {
        ctx.setActiveVoiceMenuOutsideHandler(null);
      }
    };
    ctx.setActiveVoiceMenuOutsideHandler(onOutside);
    setTimeout(() => {
      if (menu.isConnected) {
        document.addEventListener?.("mousedown", onOutside, true);
      }
    }, 0);
    document.body.append(menu);
  }

  /**
   * Modal dialog for selecting a specific screen or application window ("Screen Share Request").
   * @param {"screens" | "windows"} initialPortalTab
   * @param {(picked: { id: string, label: string, tab: "screens" | "windows" }) => void} onSelectSource
   */
  function openScreenSharePortalPicker(initialPortalTab, onSelectSource) {
    document.getElementById("live-screenshare-portal-modal")?.remove();
    const backdrop = document.createElement("div");
    backdrop.id = "live-screenshare-portal-modal";
    backdrop.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.68);display:flex;align-items:center;justify-content:center;z-index:10005;";
    const dialog = document.createElement("div");
    dialog.setAttribute("role", "dialog");
    dialog.setAttribute("aria-modal", "true");
    dialog.setAttribute("aria-label", "Запрос демонстрации экрана");
    dialog.style.cssText =
      "width:480px;max-width:94vw;background:#26282e;color:#f2f3f5;border:1px solid #3f4147;border-radius:12px;overflow:hidden;box-shadow:0 24px 64px rgba(0,0,0,0.88);display:flex;flex-direction:column;font-family:inherit;";

    /** @type {number | null} */
    let portalRafId = null;
    const closePortal = () => {
      if (portalRafId !== null) {
        cancelAnimationFrame(portalRafId);
        portalRafId = null;
      }
      backdrop.remove();
    };

    const titleBar = document.createElement("div");
    titleBar.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;padding:10px 14px;background:#1e2025;border-bottom:1px solid #35373c;";
    const leftTitle = document.createElement("div");
    leftTitle.style.cssText = "display:flex;align-items:center;gap:8px;font-size:13px;font-weight:700;color:#dbdee1;";
    leftTitle.innerHTML = `<span style="width:18px;height:18px;border-radius:50%;background:#5865f2;color:#fff;display:inline-flex;align-items:center;justify-content:center;font-size:10px;">🎮</span><span>Запрос демонстрации экрана</span>`;
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText =
      "width:22px;height:22px;border-radius:50%;border:none;background:#35373c;color:#dbdee1;font-size:12px;cursor:pointer;";
    closeBtn.addEventListener("click", closePortal);
    titleBar.append(leftTitle, closeBtn);

    const searchBarWrap = document.createElement("div");
    searchBarWrap.style.cssText = "padding:10px 14px 6px;";
    const searchInput = document.createElement("input");
    searchInput.type = "search";
    searchInput.placeholder = "🔍 Поиск...";
    searchInput.style.cssText =
      "width:100%;padding:7px 11px;border-radius:6px;border:1px solid #3daee9;background:#1b1c20;color:#f2f3f5;font-size:13px;outline:none;";
    searchBarWrap.append(searchInput);

    let portalTab = initialPortalTab;
    let showWallpaper = true;
    const tabsBar = document.createElement("div");
    tabsBar.style.cssText = "display:grid;grid-template-columns:repeat(2,1fr);padding:0 14px;border-bottom:1px solid #35373c;";

    const contentArea = document.createElement("div");
    contentArea.style.cssText = "padding:14px;max-height:360px;overflow-y:auto;display:flex;flex-direction:column;gap:12px;";

    const footerArea = document.createElement("div");
    footerArea.style.cssText = "padding:10px 14px;border-top:1px solid #35373c;background:#1e2025;display:flex;align-items:center;gap:8px;";

    const activeVoiceConnection = ctx.getActiveVoiceConnection();
    const screenItems = [
      { id: "screen_dp6", label: "Открыть доступ к «LG Electronics LG QHD (DP-6)»", tab: /** @type {const} */ ("screens") },
      { id: "screen_region", label: "Открыть доступ к прямоугольной области", tab: /** @type {const} */ ("screens") },
      { id: "screen_virtual", label: "Открыть доступ к новому виртуальному экрану", tab: /** @type {const} */ ("screens") },
    ];
    const windowItems = [
      { id: "win_orca", label: "Orca", icon: "🦦", tab: /** @type {const} */ ("windows") },
      { id: "win_firefox", label: "Расширение: (Bitwarden — Бесплатный менеджер паролей) - Bitwarden — Mozilla Firefox", icon: "🦊", tab: /** @type {const} */ ("windows") },
      { id: "win_chord", label: `🔊 ${activeVoiceConnection?.channelName || "Звонок"} | ${activeVoiceConnection?.guildName || "Chord"} — Chord`, icon: "🎮", tab: /** @type {const} */ ("windows") },
      { id: "win_dolphin", label: "Видеофайлы — Dolphin", icon: "📁", tab: /** @type {const} */ ("windows") },
    ];

    void ctx.invoke("voice_desktop_sources")
      .then((/** @type {any} */ res) => {
        if (res && Array.isArray(res.sources) && res.sources.length > 0) {
          const screens = res.sources.filter(
            (/** @type {any} */ s) =>
              s.sourceType === "screen" ||
              s.source_type === "screen" ||
              String(s.id).startsWith("screen:"),
          );
          const windows = res.sources.filter(
            (/** @type {any} */ s) =>
              s.sourceType === "window" ||
              s.source_type === "window" ||
              String(s.id).startsWith("window:"),
          );
          if (screens.length > 0) {
            screenItems.length = 0;
            for (const s of screens) {
              screenItems.push({
                id: String(s.id),
                label: String(s.name || s.id),
                tab: /** @type {const} */ ("screens"),
              });
            }
          }
          if (windows.length > 0) {
            windowItems.length = 0;
            for (const w of windows) {
              const wid = String(w.id || "").trim();
              const wlabel = String(w.name || wid).trim();
              if (!wid || !wlabel) continue;
              const lower = wlabel.toLowerCase();
              const icon = lower.includes("orca")
                ? "🦦"
                : lower.includes("dolphin")
                  ? "📁"
                  : lower.includes("firefox") || lower.includes("brave") || lower.includes("chrom")
                    ? "🌐"
                    : lower.includes("discord")
                      ? "🎮"
                      : "🪟";
              windowItems.push({
                id: wid,
                label: wlabel,
                icon,
                tab: /** @type {const} */ ("windows"),
              });
            }
          }
          if (document.body.contains(backdrop)) {
            renderPortalBody();
          }
        }
      })
      .catch(() => {});

    void ctx.invoke(ctx.VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
      kind: "list_capture_windows",
      deviceId: "all",
    })
      .then((/** @type {any} */ res) => {
        if (res && Array.isArray(res.windows) && res.windows.length > 0) {
          windowItems.length = 0;
          for (const w of res.windows) {
            const wid = String(w?.id || w?.uuid || "").trim();
            const wlabel = String(w?.label || w?.title || "").trim();
            if (!wid || !wlabel) continue;
            const lower = wlabel.toLowerCase();
            const icon = lower.includes("orca")
              ? "🦦"
              : lower.includes("dolphin")
                ? "📁"
                : lower.includes("firefox") || lower.includes("brave") || lower.includes("chrom")
                  ? "🌐"
                  : lower.includes("discord")
                    ? "🎮"
                    : "🪟";
            windowItems.push({
              id: wid,
              label: wlabel,
              icon,
              tab: /** @type {const} */ ("windows"),
            });
          }
          if (document.body.contains(backdrop)) {
            renderPortalBody();
          }
        }
      })
      .catch(() => {});

    /** @type {Array<{ canvas: HTMLCanvasElement, id: string }>} */
    const activeCanvases = [];
    let tick = 0;

    const renderPortalBody = () => {
      tabsBar.replaceChildren();
      for (const t of [
        { id: /** @type {const} */ ("screens"), label: "🖥 Экраны" },
        { id: /** @type {const} */ ("windows"), label: "🪟 Окна" },
      ]) {
        const active = portalTab === t.id;
        const b = document.createElement("button");
        b.type = "button";
        b.style.cssText =
          `padding:8px 12px;border:none;border-bottom:2px solid ${active ? "#3daee9" : "transparent"};background:${active ? "rgba(61,174,233,0.12)" : "transparent"};color:${active ? "#f2f3f5" : "#949ba4"};font-size:13px;font-weight:700;cursor:pointer;`;
        b.textContent = t.label;
        b.addEventListener("click", () => {
          portalTab = t.id;
          renderPortalBody();
        });
        tabsBar.append(b);
      }

      contentArea.replaceChildren();
      activeCanvases.length = 0;
      const q = searchInput.value.trim().toLowerCase();
      const grid = document.createElement("div");
      grid.style.cssText = "display:grid;grid-template-columns:repeat(2,1fr);gap:10px;";

      const list = portalTab === "screens" ? screenItems : windowItems;
      for (const item of list) {
        if (q && !item.label.toLowerCase().includes(q)) continue;
        const cardBtn = document.createElement("button");
        cardBtn.type = "button";
        cardBtn.setAttribute("data-portal-source-id", item.id);
        cardBtn.style.cssText =
          "display:flex;flex-direction:column;gap:8px;padding:10px;border-radius:8px;border:1.5px solid #3f4147;background:#2f323a;color:#f2f3f5;cursor:pointer;text-align:center;align-items:center;";
        cardBtn.addEventListener("mouseenter", () => {
          cardBtn.style.borderColor = "#3daee9";
          cardBtn.style.background = "rgba(61,174,233,0.15)";
        });
        cardBtn.addEventListener("mouseleave", () => {
          cardBtn.style.borderColor = "#3f4147";
          cardBtn.style.background = "#2f323a";
        });
        const cv = document.createElement("canvas");
        cv.width = 188;
        cv.height = 104;
        cv.style.cssText = "width:100%;height:96px;border-radius:4px;background:#111214;display:block;";
        activeCanvases.push({ canvas: cv, id: item.id });
        ctx.drawLiveCaptureCanvasFrame(cv, item.id, tick, showWallpaper);

        const lbl = document.createElement("div");
        lbl.style.cssText =
          "font-size:12px;font-weight:600;color:#f2f3f5;line-height:1.35;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;";
        lbl.textContent = item.label;
        cardBtn.append(cv, lbl);
        cardBtn.addEventListener("click", () => {
          closePortal();
          onSelectSource(item);
        });
        grid.append(cardBtn);
      }
      contentArea.append(grid);

      footerArea.replaceChildren();
      if (portalTab === "screens") {
        const wpLabel = document.createElement("label");
        wpLabel.style.cssText = "display:flex;align-items:center;gap:8px;font-size:12px;color:#dbdee1;cursor:pointer;user-select:none;";
        const wpCheck = document.createElement("input");
        wpCheck.type = "checkbox";
        wpCheck.checked = showWallpaper;
        wpCheck.style.cssText = "width:15px;height:15px;accent-color:#3daee9;cursor:pointer;";
        wpCheck.addEventListener("change", () => {
          showWallpaper = wpCheck.checked;
          for (const c of activeCanvases) {
            ctx.drawLiveCaptureCanvasFrame(c.canvas, c.id, tick, showWallpaper);
          }
        });
        const wpTxt = document.createElement("span");
        wpTxt.textContent = "Показывать обои рабочего стола";
        wpLabel.append(wpCheck, wpTxt);
        footerArea.append(wpLabel);
      } else {
        const hint = document.createElement("span");
        hint.className = "stream-portal-hint";
        hint.textContent = "Выберите окно приложения для начала трансляции";
        footerArea.append(hint);
      }
    };

    searchInput.addEventListener("input", renderPortalBody);
    renderPortalBody();
    let lastPortalDraw = 0;
    /** @param {number} [timestamp] */
    const portalLoop = (timestamp) => {
      if (portalRafId === null || !document.body.contains(backdrop)) {
        portalRafId = null;
        return;
      }
      const now = typeof timestamp === "number" ? timestamp : (typeof performance !== "undefined" ? performance.now() : Date.now());
      if (lastPortalDraw === 0) {
        lastPortalDraw = now;
      } else if (now - lastPortalDraw >= 150) {
        lastPortalDraw = now;
        tick += 1;
        for (const c of activeCanvases) {
          ctx.drawLiveCaptureCanvasFrame(c.canvas, c.id, tick, showWallpaper);
        }
      }
      portalRafId = requestAnimationFrame(portalLoop);
    };
    portalRafId = requestAnimationFrame(portalLoop);

    dialog.append(titleBar, searchBarWrap, tabsBar, contentArea, footerArea);
    backdrop.append(dialog);
    backdrop.addEventListener("click", (ev) => {
      if (ev.target === backdrop) closePortal();
    });
    document.body.append(backdrop);
  }

  return {
    openNitroHdModal,
    openStreamSettingsMenu,
    openScreenSharePortalPicker,
  };
}
