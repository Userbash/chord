// @ts-check
/**
 * TASK-630 (G4 QA): Автоматизированные headless browser E2E регрессионные тесты
 * для сертифицированных функций сборок BUILD-005 (Спринты 1–10) и BUILD-006 (Спринты 11–13).
 *
 * Что поднимается: реальный `rust/gui-spike/ui/index.html` через локальный HTTP-сервер,
 * headless Chromium + CDP (`openApp` из `tests/helpers/ui-fixture.mjs`) и детерминированный
 * fake-транспорт `window.__TAURI__` (`tests/helpers/fake-tauri.mjs`).
 *
 * Покрываемые E2E-сценарии (BUILD-005 / BUILD-006):
 * 1. TASK-610: Interactive hCaptcha Solver Modal (`#captcha-modal` ↔ `join-server.js` ↔ `accept_invite`)
 * 2. TASK-611 / TASK-612: Server Creation Wizard UI (`#create-guild-dialog` ↔ `create_guild`)
 * 3. TASK-615 / TASK-616 / TASK-623: Stream Viewer & PiP Player UI + DAVE MLS E2EE badge (`stream-viewer.js`)
 * 4. TASK-617 / TASK-618: Search Popover UI with Token Suggestions & Highlights (`search.js` + `search_remote`)
 * 5. TASK-619 / TASK-620: Context Menu Actions (ПКМ), Volume Slider 0..200% & Outside Click (`context-menu.js`)
 * 6. TASK-626: Nitro Gifts & Entitlements IPC Bridge (`list_nitro_gift_plans`, `resolve_gift_code`, `create_gift_code`, `redeem_gift_code`)
 * 7. TASK-628: Advanced Settings UI: Keybinds, Soundboard & Clips (`advanced-settings.js`)
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { M2_IDS, m2Channels, m2FixtureData, openApp } from "../helpers/ui-fixture.mjs";

const data = m2FixtureData();

const SAMPLE_SITEKEY = "a9b5fb07-92ff-493f-86fe-352a2803b3df";
const SAMPLE_CAPTCHA_TOKEN = "P1_eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.build006_e2e_token";
const SAMPLE_PNG_DATA_URI =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
const CREATED_GUILD_ID = "888999000111222333";

describe("BUILD-005 / BUILD-006: Headless Browser E2E Regression Suite (реальный index.html)", () => {
  it("TASK-610: интерактивное окно #captcha-modal перехватывает HTTP 400 (code 40003) и повторяет accept_invite с captcha_key", async (t) => {
    const invitedGuildId = M2_IDS.invitedGuild;
    const ui = await openApp(t, {
      overrides: {
        resolve_invite: { ok: data.invite.preview },
        accept_invite: {
          seq: [
            {
              err: {
                kind: "captchaRequired",
                status: 400,
                code: 40003,
                captcha_service: "hcaptcha",
                captcha_sitekey: SAMPLE_SITEKEY,
                captcha_rqdata: "rqdata_build006",
                captcha_rqtoken: "rqtoken_build006",
                captcha_session_id: "sess_build006",
              },
            },
            { ok: data.invite.accepted },
          ],
        },
      },
    });

    const tilesBefore = await ui.count("#guild-rail button.guild-item");

    // 1. Открываем модалку инвайта и запрашиваем preview (resolve_invite).
    await ui.click('.guild-add[data-action="add-server"]');
    await ui.waitFor(
      "(() => { const n = document.querySelector('#join-server-dialog'); return n !== null && !n.hasAttribute('hidden'); })()",
      { label: "join-server-dialog открыт" },
    );
    await ui.type("#join-invite-input", String(data.invite.code));
    await ui.click("#join-submit");
    await ui.waitFor(
      "(() => { const n = document.querySelector('#join-preview'); return n !== null && n.getAttribute('data-state') === 'ready'; })()",
      { label: "join-preview готов" },
    );

    // 2. Первый клик по Присоединиться возвращает 40003 hCaptcha -> открывается #captcha-modal.
    await ui.click("#join-submit");
    await ui.waitFor(
      "(() => { const m = document.querySelector('#captcha-modal'); return m !== null && !m.hasAttribute('hidden') && m.getAttribute('data-state') === 'ready'; })()",
      { label: "captcha-modal открыт в состоянии ready" },
    );

    assert.equal(await ui.attr("#captcha-modal", "role"), "dialog");
    assert.equal(await ui.attr("#captcha-modal", "aria-modal"), "true");
    assert.equal(await ui.attr("#captcha-modal", "data-service"), "hcaptcha");
    assert.equal(await ui.attr("#captcha-modal", "data-sitekey"), SAMPLE_SITEKEY);
    assert.match(
      String(await ui.attr("#captcha-widget-frame", "data-widget-url")),
      new RegExp(`sitekey=${SAMPLE_SITEKEY}`),
    );
    assert.equal(await ui.callCount("accept_invite"), 1, "первая попытка accept_invite до решения капчи");
    assert.equal(await ui.count("#guild-rail button.guild-item"), tilesBefore, "плитка не должна добавляться до решения капчи");

    // 3. Пустой токен валидируется локально в #captcha-modal-error без повторного IPC-вызова.
    await ui.type("#captcha-token-input", "   ");
    await ui.eval(
      "(() => { const f = document.querySelector('#captcha-modal-form'); if (!f) throw new Error('no form'); f.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); })()",
    );
    await ui.waitFor(
      "(() => { const e = document.querySelector('#captcha-modal-error'); return e !== null && !e.hasAttribute('hidden'); })()",
      { label: "ошибка пустого токена captcha" },
    );
    assert.equal(await ui.attr("#captcha-modal", "data-state"), "error");
    assert.equal(await ui.callCount("accept_invite"), 1, "пустой токен не должен отправлять accept_invite");

    // 4. Ввод валидного токена hCaptcha -> повторный accept_invite с captcha_key и добавление гильдии в рейл.
    await ui.type("#captcha-token-input", SAMPLE_CAPTCHA_TOKEN);
    await ui.eval(
      "(() => { const f = document.querySelector('#captcha-modal-form'); if (!f) throw new Error('no form'); f.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); })()",
    );

    await ui.waitFor(
      "(() => { const m = document.querySelector('#captcha-modal'); return m !== null && m.hasAttribute('hidden'); })()",
      { label: "captcha-modal закрыт после успешного решения" },
    );
    await ui.waitFor(
      "(() => { const d = document.querySelector('#join-server-dialog'); return d !== null && d.hasAttribute('hidden'); })()",
      { label: "join-server-dialog закрыт после вступления" },
    );

    const acceptCalls = await ui.calls("accept_invite");
    assert.equal(acceptCalls.length, 2, "accept_invite вызван ровно 2 раза (до и после решения hCaptcha)");
    assert.deepEqual(acceptCalls[0]?.args, { code: String(data.invite.code) });
    assert.equal(acceptCalls[1]?.args?.code, String(data.invite.code));
    assert.equal(acceptCalls[1]?.args?.captcha_key, SAMPLE_CAPTCHA_TOKEN);
    assert.equal(acceptCalls[1]?.args?.captcha_rqtoken, "rqtoken_build006");
    assert.equal(acceptCalls[1]?.args?.captcha_session_id, "sess_build006");

    assert.equal(await ui.count("#guild-rail button.guild-item"), tilesBefore + 1);
    assert.equal(
      await ui.has(`#guild-rail button.guild-item[data-guild-id="${invitedGuildId}"]`),
      true,
      "плитка приглашённой гильдии добавлена в рейл после прохождения капчи",
    );
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });

  it("TASK-611 / TASK-612: визард создания сервера #create-guild-dialog (шаблоны, иконка base64, A11y, create_guild)", async (t) => {
    const ui = await openApp(t, {
      overrides: {
        create_guild: {
          ok: {
            id: CREATED_GUILD_ID,
            name: "Лаборатория E2E",
            icon: SAMPLE_PNG_DATA_URI,
            channels: [],
          },
        },
      },
    });

    const tilesBefore = await ui.count("#guild-rail button.guild-item");

    // 1. Переход из модалки добавления сервера в визард создания сервера (#create-guild-dialog).
    await ui.click('.guild-add[data-action="add-server"]');
    await ui.waitFor(
      "(() => { const n = document.querySelector('#join-server-dialog'); return n !== null && !n.hasAttribute('hidden'); })()",
      { label: "join-server-dialog открыт" },
    );
    await ui.click(".switch-to-create-btn");
    await ui.waitFor(
      "(() => { const d = document.querySelector('#create-guild-dialog'); return d !== null && !d.hasAttribute('hidden'); })()",
      { label: "create-guild-dialog открыт" },
    );

    // 2. Проверка 4 шаблонов по RE-оракулу и клавиатурной навигации (roving tabindex).
    assert.equal(await ui.count("#create-guild-dialog .template-card"), 4, "4 карточки шаблонов");
    const templateLabels = await ui.eval(
      "[...document.querySelectorAll('#create-guild-dialog .template-card .template-label')].map((el) => el.textContent)",
    );
    assert.deepEqual(templateLabels, [
      "Свой шаблон",
      "Для меня и друзей",
      "Учебная группа",
      "Клуб или сообщество",
    ]);

    assert.equal(
      await ui.attr('#create-guild-dialog .template-card[data-template="custom"]', "tabindex"),
      "0",
    );
    await ui.press('#create-guild-dialog .template-card[data-template="custom"]', "ArrowDown");
    assert.equal(
      await ui.attr('#create-guild-dialog .template-card[data-template="friends"]', "tabindex"),
      "0",
    );

    // 3. Выбор шаблона «Учебная группа» (study) переводит визард на шаг customize.
    await ui.click('#create-guild-dialog .template-card[data-template="study"]');
    await ui.waitFor("Boolean(document.querySelector('#create-guild-name'))", {
      label: "шаг настройки сервера (#create-guild-name)",
    });
    assert.equal(await ui.text("#create-guild-title"), "Настройте свой сервер");

    // 4. Валидация названия (1..100 символов после trim) и загрузки base64 DataURI значка.
    await ui.type("#create-guild-name", "   ");
    assert.equal(await ui.attr("#create-guild-name", "aria-invalid"), "true");
    assert.equal(await ui.eval("Boolean(document.querySelector('#create-guild-submit').disabled)"), true);

    await ui.eval(
      `(() => {
        const input = document.querySelector('#create-guild-icon-input');
        input.dataset.dataUri = ${JSON.stringify(SAMPLE_PNG_DATA_URI)};
        input.dispatchEvent(new Event('change', { bubbles: true }));
      })()`,
    );
    assert.equal(
      await ui.attr("#create-guild-icon-preview img.create-guild-icon-img", "src"),
      SAMPLE_PNG_DATA_URI,
      "превью иконки отображает загруженный base64 DataURI",
    );

    await ui.type("#create-guild-name", "  Лаборатория E2E  ");
    assert.equal(await ui.attr("#create-guild-name", "aria-invalid"), "false");
    assert.equal(await ui.eval("Boolean(document.querySelector('#create-guild-submit').disabled)"), false);

    // 5. Отправка формы вызывает create_guild с обрезанным именем, шаблоном и иконкой, добавляя сервер в рейл.
    await ui.click("#create-guild-submit");
    const call = await ui.waitForCall("create_guild");
    assert.equal(call.args?.name, "Лаборатория E2E");
    assert.equal(call.args?.template, "study");
    assert.equal(call.args?.icon, SAMPLE_PNG_DATA_URI);

    await ui.waitFor(
      "(() => { const d = document.querySelector('#create-guild-dialog'); return d !== null && d.hasAttribute('hidden'); })()",
      { label: "create-guild-dialog закрыт после создания" },
    );
    assert.equal(await ui.count("#guild-rail button.guild-item"), tilesBefore + 1);
    assert.equal(
      await ui.has(`#guild-rail button.guild-item[data-guild-id="${CREATED_GUILD_ID}"]`),
      true,
      "созданный сервер добавлен в рейл",
    );
    assert.equal(m2Channels(CREATED_GUILD_ID).length, 0);
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });

  it("TASK-616 / TASK-623: плеер стрима stream-viewer, бейдж DAVE MLS E2EE и сворачивание в PiP при смене канала", async (t) => {
    const ui = await openApp(t);

    const streamResult = await ui.eval(`(async () => {
      const mod = await import("./features/stream-viewer.js");
      const host = document.createElement("div");
      host.id = "e2e-stream-viewer-host";
      document.querySelector("#screen-chat")?.appendChild(host);

      const pauseEvents = [];
      const controller = mod.createStreamViewerController(host, {
        onPauseToggle: (paused) => pauseEvents.push(paused),
      });

      controller.openViewer({
        streamKey: "guild:100:200:300",
        channelId: "200",
        channelName: "Голосовой канал",
        userId: "300",
        displayName: "StreamerE2E",
        resolution: "1080p",
        fps: 60,
        encrypted: true,
      });

      const initialMode = controller.getMode();
      const qualityText = host.querySelector("#stream-viewer-quality-badge")?.textContent ?? "";
      const encBadge = host.querySelector("#stream-viewer-encryption-badge");
      const encAttr = encBadge?.getAttribute("data-encrypted");
      const encText = encBadge?.textContent ?? "";

      const playPauseBtn = host.querySelector("#stream-viewer-play-pause");
      playPauseBtn?.click();
      const pausedPressed = playPauseBtn?.getAttribute("aria-pressed");

      controller.onChannelSwitch("999888777");
      const afterSwitchMode = controller.getMode();
      const pipNode = document.querySelector("#stream-viewer-pip");
      const pipActive = pipNode?.getAttribute("data-pip-active");

      controller.dispose();
      host.remove();
      return {
        initialMode,
        qualityText,
        encAttr,
        encText,
        pausedPressed,
        pauseEvents,
        afterSwitchMode,
        pipActive,
      };
    })()`);

    assert.equal(streamResult.initialMode, "stage");
    assert.match(streamResult.qualityText, /1080p/);
    assert.match(streamResult.qualityText, /60 FPS/);
    assert.equal(streamResult.encAttr, "true");
    assert.match(streamResult.encText, /End-to-End Encrypted/);
    assert.equal(streamResult.pausedPressed, "true");
    assert.deepEqual(streamResult.pauseEvents, [true]);
    assert.equal(streamResult.afterSwitchMode, "pip");
    assert.equal(streamResult.pipActive, "true");
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });

  it("TASK-617 / TASK-618: поисковый попап search-popover с 7 токенами-подсказками, подсветкой и переходом к сообщению", async (t) => {
    const ui = await openApp(t);

    const searchResult = await ui.eval(`(async () => {
      const mod = await import("./features/search.js");
      const host = document.createElement("div");
      host.id = "e2e-search-popover-host";
      document.body.appendChild(host);

      const jumps = [];
      let closed = 0;
      const popover = mod.createSearchPopover(host, {
        onJumpToMessage: (messageId, channelId) => jumps.push({ messageId, channelId }),
        onClose: () => { closed += 1; },
      });

      const allTokens = [...popover.root.querySelectorAll("[data-token-suggestion]")].map((b) =>
        b.getAttribute("data-token-suggestion")
      );

      popover.setQuery("fr");
      const filteredTokens = [...popover.root.querySelectorAll("[data-token-suggestion]")].map((b) =>
        b.getAttribute("data-token-suggestion")
      );
      popover.root.querySelector("[data-token-suggestion]")?.click();
      const queryAfterClick = popover.getState().query;

      popover.setQuery("from:alex in:новости pinned:true сборка");
      const activeTokens = [...popover.root.querySelectorAll("[data-active-token]")].map((el) => ({
        prefix: el.getAttribute("data-token-prefix"),
        value: el.getAttribute("data-token-value"),
      }));

      popover.setResults({
        totalResults: 2,
        results: [
          {
            id: "1559590835220390014",
            channelId: "1014537851867775060",
            channelName: "новости",
            author: "alex",
            content: "Новая сборка BUILD-006 готова к проверке",
            timestamp: "2026-11-07 19:00",
          },
          {
            id: "1559590835220390015",
            channelId: "1014537851867775060",
            channelName: "новости",
            author: "alex",
            content: "Вторая сборка прошла все тесты",
            timestamp: "2026-11-07 19:05",
          },
        ],
      });

      const highlights = [...popover.root.querySelectorAll("mark[data-search-highlight]")].map(
        (m) => m.textContent
      );

      popover.input.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
      popover.input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
      popover.input.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
      const openAfterEsc = popover.getState().isOpen;

      host.remove();
      return {
        allTokens,
        filteredTokens,
        queryAfterClick,
        activeTokens,
        highlights,
        jumps,
        closed,
        openAfterEsc,
      };
    })()`);

    assert.deepEqual(searchResult.allTokens, [
      "from:",
      "mentions:",
      "has:",
      "in:",
      "before:",
      "after:",
      "pinned:",
    ]);
    assert.deepEqual(searchResult.filteredTokens, ["from:"]);
    assert.equal(searchResult.queryAfterClick, "from:");
    assert.deepEqual(searchResult.activeTokens, [
      { prefix: "from:", value: "alex" },
      { prefix: "in:", value: "новости" },
      { prefix: "pinned:", value: "true" },
    ]);
    assert.deepEqual(searchResult.highlights, ["сборка", "сборка"]);
    assert.deepEqual(searchResult.jumps, [
      { messageId: "1559590835220390014", channelId: "1014537851867775060" },
    ]);
    assert.equal(searchResult.closed, 1);
    assert.equal(searchResult.openAfterEsc, false);
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });

  it("TASK-620: контекстное меню context-menu по ПКМ, слайдер громкости 0..200% и закрытие кликом вне меню", async (t) => {
    const ui = await openApp(t);

    const ctxResult = await ui.eval(`(async () => {
      const mod = await import("./features/context-menu.js");
      const container = document.createElement("div");
      container.id = "e2e-context-menu-host";
      const outsideBtn = document.createElement("button");
      outsideBtn.id = "e2e-outside-btn";
      const targetEl = document.createElement("div");
      targetEl.id = "e2e-user-target";
      document.body.append(container, outsideBtn, targetEl);

      const volumeLog = [];
      const actionLog = [];
      const controller = mod.createContextMenuController({
        container,
        onVolumeChange: (v) => volumeLog.push(v),
        onSelectAction: (act) => actionLog.push(act),
      });

      const unbind = mod.bindContextMenuTrigger(targetEl, controller, () => ({
        targetType: "user",
        targetId: "user-620",
        unified: true,
        volume: 95,
      }));

      targetEl.dispatchEvent(new MouseEvent("contextmenu", {
        bubbles: true,
        cancelable: true,
        clientX: 180,
        clientY: 220,
      }));

      const opened = controller.isOpen();
      const menuEl = container.querySelector("[data-context-menu]");
      const sliderInput = menuEl?.querySelector('[data-volume-slider][role="slider"]');
      const initialVolText = menuEl?.querySelector("[data-volume-value]")?.textContent ?? "";

      if (sliderInput) {
        sliderInput.value = "165";
        sliderInput.dispatchEvent(new Event("input", { bubbles: true }));
      }
      const updatedVolText = menuEl?.querySelector("[data-volume-value]")?.textContent ?? "";
      const updatedAriaNow = sliderInput?.getAttribute("aria-valuenow") ?? "";

      controller.setVolume(250);
      const clampedHigh = controller.getVolume();

      outsideBtn.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      const openAfterOutsideClick = controller.isOpen();

      unbind();
      container.remove();
      outsideBtn.remove();
      targetEl.remove();

      return {
        opened,
        initialVolText,
        updatedVolText,
        updatedAriaNow,
        clampedHigh,
        volumeLog,
        openAfterOutsideClick,
      };
    })()`);

    assert.equal(ctxResult.opened, true);
    assert.equal(ctxResult.initialVolText, "95%");
    assert.equal(ctxResult.updatedVolText, "165%");
    assert.equal(ctxResult.updatedAriaNow, "165");
    assert.equal(ctxResult.clampedHigh, 200);
    assert.deepEqual(ctxResult.volumeLog, [165, 200]);
    assert.equal(ctxResult.openAfterOutsideClick, false);
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });

  it("TASK-626: Nitro Gifts & Entitlements IPC (list_nitro_gift_plans, resolve_gift_code, create_gift_code, redeem_gift_code)", async (t) => {
    const giftCode = "aBcDeFgHiJkLmNoP";
    const nitroStandardSku = "521847234246082599";
    const nitroMonthPlan = "511651880837840896";
    const nitroAppId = "521842831262875670";

    const ui = await openApp(t, {
      overrides: {
        list_nitro_gift_plans: {
          ok: [
            {
              skuId: nitroStandardSku,
              subscriptionPlanId: nitroMonthPlan,
              name: "Nitro Standard (1 месяц)",
              interval: 1,
              intervalCount: 1,
            },
          ],
        },
        resolve_gift_code: {
          ok: {
            code: giftCode,
            skuId: nitroStandardSku,
            applicationId: nitroAppId,
            uses: 0,
            maxUses: 1,
            redeemed: false,
            giftStyle: 2,
          },
        },
        create_gift_code: {
          ok: {
            code: giftCode,
            skuId: nitroStandardSku,
            applicationId: nitroAppId,
            uses: 0,
            maxUses: 1,
            redeemed: false,
            giftStyle: 2,
          },
        },
        redeem_gift_code: {
          seq: [
            {
              ok: {
                code: giftCode,
                entitlementId: "990011223344556677",
                skuId: nitroStandardSku,
              },
            },
            {
              err: {
                kind: "alreadyRedeemed",
                status: 400,
                code: 50050,
              },
            },
          ],
        },
      },
    });

    const giftFlow = await ui.eval(`(async () => {
      const invoke = window.__TAURI__.core.invoke;
      const plans = await invoke("list_nitro_gift_plans", {});
      const resolved = await invoke("resolve_gift_code", {
        code: ${JSON.stringify(giftCode)},
        withApplication: true,
        withSubscriptionPlan: true,
      });
      const created = await invoke("create_gift_code", {
        skuId: ${JSON.stringify(nitroStandardSku)},
        subscriptionPlanId: ${JSON.stringify(nitroMonthPlan)},
        giftStyle: 2,
        customMessage: "Подарок к релизу BUILD-006!",
      });
      const redeemed = await invoke("redeem_gift_code", {
        code: ${JSON.stringify(giftCode)},
        channelId: ${JSON.stringify(M2_IDS.channelWithMessages)},
      });
      let duplicateErrorKind = null;
      try {
        await invoke("redeem_gift_code", { code: ${JSON.stringify(giftCode)} });
      } catch (err) {
        duplicateErrorKind = err && err.kind ? err.kind : "unknown";
      }
      return { plans, resolved, created, redeemed, duplicateErrorKind };
    })()`);

    assert.equal(giftFlow.plans.length, 1);
    assert.equal(giftFlow.plans[0]?.skuId, nitroStandardSku);
    assert.equal(giftFlow.resolved.code, giftCode);
    assert.equal(giftFlow.resolved.applicationId, nitroAppId);
    assert.equal(giftFlow.created.giftStyle, 2);
    assert.equal(giftFlow.redeemed.entitlementId, "990011223344556677");
    assert.equal(giftFlow.duplicateErrorKind, "alreadyRedeemed");
    assert.equal(await ui.callCount("resolve_gift_code"), 1);
    assert.equal(await ui.callCount("create_gift_code"), 1);
    assert.equal(await ui.callCount("redeem_gift_code"), 2);
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });

  it("TASK-628: расширенные настройки advanced-settings (Keybinds, Soundboard, Clips) и сохранение в storage", async (t) => {
    const ui = await openApp(t);

    const advResult = await ui.eval(`(async () => {
      const mod = await import("./features/advanced-settings.js");
      const container = document.createElement("div");
      container.id = "e2e-advanced-settings-host";
      document.querySelector("#screen-settings")?.appendChild(container);

      const playedSounds = [];
      const uploadedSounds = [];
      const controller = mod.createAdvancedSettingsController({
        container,
        initialTab: "keybinds",
        guilds: [
          { id: "g-1", name: "Киберспорт" },
          { id: "g-2", name: "Лаборатория" },
        ],
        keybinds: [
          {
            id: "managed-clip",
            action: "SAVE_CLIP",
            shortcut: "Alt+C",
            enabled: true,
            managed: true,
          },
        ],
        soundboard: {
          volumePercent: 80,
          selectedGuildId: "all",
          entranceSounds: { all: "s-1" },
          sounds: [
            { soundId: "s-1", name: "Фанфары", emojiName: "🎺", available: true },
            { soundId: "s-2", name: "Гудок", emojiName: "📯", available: true },
          ],
        },
        clips: {
          clipsEnabled: false,
          hardwareBelowMinimum: true,
        },
        onPlaySound: (soundId, volume) => playedSounds.push({ soundId, volume }),
        onUploadSound: (payload) => uploadedSounds.push(payload),
      });

      // 1. Вкладка Keybinds: защита managed бинда, добавление нового бинда, запись и детекция конфликта с Ctrl+K.
      const managedDeleteAttempt = controller.removeKeybind("managed-clip");
      const added = controller.addKeybind({ action: "TOGGLE_MUTE", shortcut: "Ctrl+Shift+M" });
      const conflictBuiltIn = mod.detectKeybindConflicts(
        controller.getState().keybinds,
        "Ctrl+K",
        added.id,
      );

      // 2. Вкладка Soundboard: ползунок громкости, предпросмотр звука, серверный звук входа, загрузка звука.
      container.querySelector('[data-advanced-tab="soundboard"]')?.click();
      const volSlider = container.querySelector("[data-soundboard-volume-slider]");
      if (volSlider) {
        volSlider.value = "40";
        volSlider.dispatchEvent(new Event("input", { bubbles: true }));
      }
      container.querySelector('[data-soundboard-preview-btn="s-2"]')?.click();

      const guildSelect = container.querySelector("[data-entrance-guild-select]");
      if (guildSelect) {
        guildSelect.value = "g-1";
        guildSelect.dispatchEvent(new Event("change", { bubbles: true }));
      }
      container.querySelector('[data-entrance-sound-option="s-2"]')?.click();
      const entranceG1 = controller.getEntranceSoundForGuild("g-1");
      const entranceG2 = controller.getEntranceSoundForGuild("g-2");

      const nameInput = container.querySelector("[data-soundboard-name-input]");
      const uriInput = container.querySelector("[data-soundboard-datauri-input]");
      if (nameInput && uriInput) {
        nameInput.value = "Победный сигнал";
        uriInput.value = "data:audio/ogg;base64,T2dnUw==";
      }
      container.querySelector("[data-soundboard-upload-btn]")?.click();

      // 3. Вкладка Clips: предупреждение оборудования, включение клипов, длительность 60с, 1440p, 60 FPS, защита от path traversal.
      container.querySelector('[data-advanced-tab="clips"]')?.click();
      const hasHwWarning = Boolean(container.querySelector("[data-clips-hardware-warning]"));
      container.querySelector("[data-clips-enabled-switch]")?.click();

      const lengthSelect = container.querySelector("[data-clips-length-select]");
      if (lengthSelect) {
        lengthSelect.value = "60";
        lengthSelect.dispatchEvent(new Event("change", { bubbles: true }));
      }
      const resSelect = container.querySelector("[data-clips-resolution-select]");
      if (resSelect) {
        resSelect.value = "1440";
        resSelect.dispatchEvent(new Event("change", { bubbles: true }));
      }
      const fpsSelect = container.querySelector("[data-clips-fps-select]");
      if (fpsSelect) {
        fpsSelect.value = "60";
        fpsSelect.dispatchEvent(new Event("change", { bubbles: true }));
      }
      const traversalRejected = !controller.updateClipsSettings({ storageLocation: "../unsafe" });
      const finalState = controller.getState();

      controller.destroy();
      container.remove();

      return {
        managedDeleteAttempt,
        conflictBuiltIn,
        playedSounds,
        uploadedSounds,
        entranceG1,
        entranceG2,
        hasHwWarning,
        traversalRejected,
        clipsState: finalState.clips,
        soundboardVol: finalState.soundboard.volumePercent,
      };
    })()`);

    assert.equal(advResult.managedDeleteAttempt, false);
    assert.equal(advResult.conflictBuiltIn.hasBuiltInConflict, true);
    assert.equal(advResult.conflictBuiltIn.builtInAction, "QUICK_SWITCHER");
    assert.equal(advResult.soundboardVol, 40);
    assert.deepEqual(advResult.playedSounds, [{ soundId: "s-2", volume: 0.4 }]);
    assert.equal(advResult.entranceG1, "s-2");
    assert.equal(advResult.entranceG2, "s-1");
    assert.deepEqual(advResult.uploadedSounds, [
      {
        name: "Победный сигнал",
        soundDataUri: "data:audio/ogg;base64,T2dnUw==",
        volume: 0.4,
      },
    ]);
    assert.equal(advResult.hasHwWarning, true);
    assert.equal(advResult.traversalRejected, true);
    assert.equal(advResult.clipsState.clipsEnabled, true);
    assert.equal(advResult.clipsState.clipsLength, 60);
    assert.equal(advResult.clipsState.resolution, 1440);
    assert.equal(advResult.clipsState.frameRate, 60);
    assert.deepEqual(ui.pageExceptions(), [], "необработанные исключения страницы");
  });
});
