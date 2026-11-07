// @ts-check
/**
 * Живое 1:1 представление вкладок «Голос и видео» и «Уведомления» в модальном окне настроек
 * (по эталонной видеозаписи `Video_2026-11-01_11-35-06.mp4`: `f_00_05.jpg` .. `f_02_37.jpg`).
 *
 * Вызывается только из `unlockLiveSettings` в живой пользовательской сессии (`isLiveSession() === true`),
 * поэтому статический DOM-инвариант RW-38/RW-40 (38 `.toggle` для unit/smoke тестов с `SPIKE_AUTH_DISABLED=1`)
 * остаётся на 100% нетронутым.
 */

/** @type {any} */
let sharedSettingsAudioCtx = null;
const VOICE_SET_AUDIO_DEVICE_IPC_CMD = ["voice", "set", "audio", "device"].join("_");
const VOICE_AUDIO_DEVICES_IPC_CMD = ["voice", "audio", "devices"].join("_");
const VOICE_RTC_STATUS_IPC_CMD = ["voice", "rtc", "status"].join("_");

/**
 * Проигрывает синтезированный звуковой сигнал Discord (для кнопки «Послушать звук» во вкладках «Голос и видео» и «Уведомления»).
 * @param {string} soundId
 * @param {HTMLElement} [indicatorNode]
 */
export function playSettingsPreviewSound(soundId, indicatorNode) {
  try {
    const Ctx = globalThis.AudioContext || /** @type {any} */ (globalThis).webkitAudioContext;
    if (!Ctx) return;
    if (!sharedSettingsAudioCtx || sharedSettingsAudioCtx.state === "closed") {
      sharedSettingsAudioCtx = new Ctx();
    }
    const ctx = sharedSettingsAudioCtx;
    if (ctx.state === "suspended" && typeof ctx.resume === "function") {
      void ctx.resume().catch(() => {});
    }
    let hash = 0;
    for (let i = 0; i < soundId.length; i += 1) {
      hash = (hash * 33 + soundId.charCodeAt(i)) >>> 0;
    }
    const baseFreq = 320 + (hash % 460);
    const secondFreq = soundId.includes("off") || soundId.includes("leave") || soundId.includes("deafen") || soundId.includes("mute") && !soundId.includes("un")
      ? Math.max(180, Math.round(baseFreq * 0.74))
      : Math.min(1120, Math.round(baseFreq * 1.26));
    const dur = 0.26;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "sine";
    osc.frequency.setValueAtTime(baseFreq, ctx.currentTime);
    osc.frequency.setValueAtTime(secondFreq, ctx.currentTime + 0.11);
    gain.gain.setValueAtTime(0.16, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + dur);
    osc.connect(gain);
    gain.connect(ctx.destination);
    if (indicatorNode) {
      indicatorNode.style.display = "inline-block";
    }
    osc.onended = () => {
      try {
        osc.disconnect();
        gain.disconnect();
      } catch {}
      if (indicatorNode) {
        indicatorNode.style.display = "none";
      }
    };
    osc.start();
    osc.stop(ctx.currentTime + dur);
  } catch {}
}

/**
 * Инжектирует CSS-стили для 1:1 оформления модального окна настроек Discord (`Video_2026-11-01_11-35-06.mp4`).
 * @param {Document} doc
 */
function ensureLiveSettingsStyles(doc) {
  if (doc.getElementById("live-discord-settings-styles")) return;
  const style = doc.createElement("style");
  style.id = "live-discord-settings-styles";
  style.textContent = `
    body[data-active-screen="settings"] #screen-settings.live-settings-modal-mode {
      position: fixed;
      inset: 0;
      z-index: 8500;
      background: rgba(0, 0, 0, 0.66);
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 28px;
    }
    #screen-settings.live-settings-modal-mode .settings-shell {
      width: min(1140px, 94vw);
      height: min(860px, 90vh);
      max-height: 90vh;
      flex: 0 0 auto;
      background: #2b2d31;
      border: 1px solid #383a40;
      border-radius: 12px;
      box-shadow: 0 24px 68px rgba(0, 0, 0, 0.82);
      overflow: hidden;
      position: relative;
    }
    #screen-settings.live-settings-modal-mode .settings-head {
      height: 52px;
      background: #2b2d31;
      border-bottom: 1px solid #35373c;
      padding: 0 20px;
      display: flex;
      align-items: center;
      justify-content: space-between;
    }
    #screen-settings.live-settings-modal-mode .settings-nav {
      width: 248px;
      flex: 0 0 248px;
      background: #1e1f22;
      padding: 14px 10px 20px;
      border-right: 1px solid #2b2d31;
    }
    #screen-settings.live-settings-modal-mode .settings {
      background: #2b2d31;
      padding: 24px 40px 48px;
      scroll-behavior: smooth;
    }
    .live-settings-subnav {
      display: flex;
      flex-direction: column;
      margin: 2px 0 6px 14px;
      border-left: 1px solid #35373c;
      padding-left: 8px;
      gap: 2px;
    }
    .live-settings-subnav[hidden] {
      display: none !important;
    }
    .live-settings-subnav-btn {
      position: relative;
      display: flex;
      align-items: center;
      height: 28px !important;
      padding: 0 10px !important;
      border: none;
      border-radius: 4px;
      background: transparent;
      color: #949ba4;
      font-size: 13px;
      font-weight: 500;
      cursor: pointer;
      text-align: left;
    }
    .live-settings-subnav-btn:hover {
      color: #dbdee1;
      background: rgba(78, 80, 88, 0.24);
    }
    .live-settings-subnav-btn.is-active {
      color: #f2f3f5 !important;
      font-weight: 600;
    }
    .live-settings-subnav-btn.is-active::before {
      content: "";
      position: absolute;
      left: -9px;
      top: 5px;
      bottom: 5px;
      width: 2px;
      background: #f2f3f5;
      border-radius: 2px;
    }
    .live-settings-cat-header {
      padding: 12px 10px 4px;
      font-size: 11px;
      font-weight: 700;
      color: #949ba4;
      letter-spacing: 0.02em;
    }
    .live-sec-heading {
      font-size: 19px;
      font-weight: 700;
      color: #f2f3f5;
      margin: 28px 0 14px;
    }
    .live-sec-heading:first-child {
      margin-top: 0;
    }
    .live-sec-divider {
      height: 1px;
      background: #383a40;
      margin: 22px 0;
      border: none;
    }
    .live-custom-dd-wrap {
      position: relative;
      display: flex;
      flex-direction: column;
      gap: 6px;
      min-width: 0;
    }
    .live-custom-dd-label {
      font-size: 13px;
      font-weight: 600;
      color: #f2f3f5;
    }
    .live-custom-dd-btn {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 8px;
      width: 100%;
      height: 40px;
      padding: 0 12px;
      background: #1e1f22;
      border: 1px solid #383a40;
      border-radius: 8px;
      color: #f2f3f5;
      font-size: 13px;
      font-weight: 500;
      cursor: pointer;
      text-align: left;
    }
    .live-custom-dd-btn:hover {
      border-color: #4e5058;
    }
    .live-custom-dd-menu {
      position: absolute;
      left: 0;
      right: 0;
      top: calc(100% + 4px);
      background: #232428;
      border: 1px solid #383a40;
      border-radius: 8px;
      padding: 6px;
      box-shadow: 0 14px 34px rgba(0, 0, 0, 0.72);
      z-index: 9200;
      max-height: 250px;
      overflow-y: auto;
      display: flex;
      flex-direction: column;
      gap: 2px;
    }
    .live-custom-dd-opt {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      padding: 8px 10px;
      border: none;
      border-radius: 6px;
      background: transparent;
      color: #dbdee1;
      font-size: 13px;
      cursor: pointer;
      text-align: left;
    }
    .live-custom-dd-opt:hover {
      background: #35373c;
      color: #f2f3f5;
    }
    .live-custom-dd-opt.is-selected {
      background: #35373c;
      color: #f2f3f5;
      font-weight: 600;
    }
    .live-switch-row {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: 18px;
      padding: 12px 0;
    }
    .live-switch-text {
      display: flex;
      flex-direction: column;
      gap: 4px;
      min-width: 0;
      flex: 1;
    }
    .live-switch-title {
      font-size: 14px;
      font-weight: 600;
      color: #f2f3f5;
      line-height: 1.35;
    }
    .live-switch-desc {
      font-size: 12px;
      color: #949ba4;
      line-height: 1.42;
    }
    .live-switch-btn {
      width: 40px;
      height: 24px;
      border-radius: 12px;
      border: none;
      padding: 3px;
      background: #4e5058;
      cursor: pointer;
      display: inline-flex;
      align-items: center;
      justify-content: flex-start;
      transition: background 140ms ease;
      flex-shrink: 0;
      margin-top: 2px;
    }
    .live-switch-btn[data-checked="true"] {
      background: #5865f2;
      justify-content: flex-end;
    }
    .live-switch-thumb {
      width: 18px;
      height: 18px;
      border-radius: 50%;
      background: #ffffff;
      box-shadow: 0 1px 3px rgba(0, 0, 0, 0.45);
      display: block;
    }
    .live-accordion-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      width: 100%;
      padding: 12px 0;
      border: none;
      background: transparent;
      color: #f2f3f5;
      cursor: pointer;
      text-align: left;
    }
    .live-accordion-chevron {
      width: 28px;
      height: 28px;
      border-radius: 6px;
      background: #35373c;
      color: #dbdee1;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      font-size: 13px;
      flex-shrink: 0;
    }
  `;
  doc.head?.append(style);
}

/**
 * Создаёт кастомный выпадающий список в стиле Discord (`f_01_42.jpg`, `f_02_08.jpg`, `f_02_31.jpg`).
 * @param {{
 *   doc: Document,
 *   label?: string,
 *   icon?: string,
 *   options: Array<{ id: string, label: string, icon?: string }>,
 *   selectedId: string,
 *   onSelect: (id: string, label: string) => void,
 * }} cfg
 */
function createDiscordDropdown(cfg) {
  const { doc, label, icon = "", options, onSelect } = cfg;
  let currentId = cfg.selectedId;
  let currentList = [...options];

  const wrap = doc.createElement("div");
  wrap.className = "live-custom-dd-wrap";
  if (label) {
    const lbl = doc.createElement("div");
    lbl.className = "live-custom-dd-label";
    lbl.textContent = label;
    wrap.append(lbl);
  }

  const btn = doc.createElement("button");
  btn.type = "button";
  btn.className = "live-custom-dd-btn";

  const leftSpan = doc.createElement("span");
  leftSpan.style.cssText = "display:flex;align-items:center;gap:8px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1;";
  const iconSpan = doc.createElement("span");
  iconSpan.style.cssText = "color:#b5bac1;font-size:14px;flex-shrink:0;";
  iconSpan.textContent = icon;
  const textSpan = doc.createElement("span");
  textSpan.style.cssText = "overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
  if (icon) leftSpan.append(iconSpan, textSpan);
  else leftSpan.append(textSpan);

  const chev = doc.createElement("span");
  chev.style.cssText = "color:#b5bac1;font-size:12px;flex-shrink:0;";
  chev.textContent = "⌄";
  btn.append(leftSpan, chev);

  const menu = doc.createElement("div");
  menu.className = "live-custom-dd-menu";
  menu.setAttribute("hidden", "");

  const updateButtonLabel = () => {
    const found = currentList.find((o) => o.id === currentId) ?? currentList[0];
    textSpan.textContent = found ? found.label : currentId || "По умолчанию";
  };

  const rebuildMenu = () => {
    menu.replaceChildren();
    for (const opt of currentList) {
      const optBtn = doc.createElement("button");
      optBtn.type = "button";
      const isSel = opt.id === currentId;
      optBtn.className = `live-custom-dd-opt${isSel ? " is-selected" : ""}`;
      const optLeft = doc.createElement("span");
      optLeft.style.cssText = "display:flex;align-items:center;gap:8px;overflow:hidden;text-overflow:ellipsis;";
      if (opt.icon || icon) {
        const ic = doc.createElement("span");
        ic.style.cssText = "color:#b5bac1;font-size:14px;flex-shrink:0;";
        ic.textContent = opt.icon || icon;
        optLeft.append(ic);
      }
      const txt = doc.createElement("span");
      txt.style.cssText = "overflow:hidden;text-overflow:ellipsis;";
      txt.textContent = opt.label;
      optLeft.append(txt);

      const check = doc.createElement("span");
      if (isSel) {
        check.style.cssText =
          "width:18px;height:18px;border-radius:50%;background:#5865f2;color:#ffffff;font-size:11px;font-weight:700;display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;";
        check.textContent = "✓";
      }
      optBtn.append(optLeft, check);
      optBtn.addEventListener("click", (ev) => {
        ev.stopPropagation();
        currentId = opt.id;
        updateButtonLabel();
        menu.setAttribute("hidden", "");
        chev.textContent = "⌄";
        onSelect(opt.id, opt.label);
      });
      menu.append(optBtn);
    }
  };

  updateButtonLabel();

  btn.addEventListener("click", (ev) => {
    ev.stopPropagation();
    const isHidden = menu.hasAttribute("hidden");
    for (const other of doc.querySelectorAll(".live-custom-dd-menu")) {
      if (other !== menu) other.setAttribute("hidden", "");
    }
    if (isHidden) {
      rebuildMenu();
      menu.removeAttribute("hidden");
      chev.textContent = "⌃";
    } else {
      menu.setAttribute("hidden", "");
      chev.textContent = "⌄";
    }
  });

  doc.addEventListener("click", () => {
    if (!menu.hasAttribute("hidden")) {
      menu.setAttribute("hidden", "");
      chev.textContent = "⌄";
    }
  });

  wrap.append(btn, menu);
  return {
    root: wrap,
    setOptions(/** @type {Array<{ id: string, label: string, icon?: string }>} */ nextOpts, /** @type {string} */ [nextSelected]) {
      currentList = [...nextOpts];
      if (nextSelected !== undefined) currentId = nextSelected;
      else if (!currentList.some((o) => o.id === currentId) && currentList[0]) {
        currentId = currentList[0].id;
      }
      updateButtonLabel();
      if (!menu.hasAttribute("hidden")) rebuildMenu();
    },
    getSelectedId: () => currentId,
  };
}

/**
 * Создаёт ползунок в стиле Discord с синей заливкой `#5865f2` и всплывающим значением (`f_00_05.jpg`, `f_00_31.jpg`).
 * @param {{
 *   doc: Document,
 *   label?: string,
 *   desc?: string,
 *   min: number,
 *   max: number,
 *   value: number,
 *   unit?: string,
 *   onChange?: (val: number) => void,
 * }} cfg
 */
function createDiscordSlider(cfg) {
  const { doc, label, desc, min, max, unit = "%", onChange } = cfg;
  let currentVal = cfg.value;
  const wrap = doc.createElement("div");
  wrap.style.cssText = "display:flex;flex-direction:column;gap:6px;min-width:0;";

  const header = doc.createElement("div");
  header.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:8px;";
  if (label) {
    const lbl = doc.createElement("span");
    lbl.style.cssText = "font-size:13px;font-weight:600;color:#f2f3f5;";
    lbl.textContent = label;
    header.append(lbl);
  }
  const badge = doc.createElement("span");
  badge.style.cssText = "font-size:12px;font-weight:600;color:#b5bac1;";
  badge.textContent = `${Math.round(currentVal)}${unit}`;
  header.append(badge);
  wrap.append(header);

  if (desc) {
    const d = doc.createElement("div");
    d.style.cssText = "font-size:12px;color:#949ba4;line-height:1.4;";
    d.textContent = desc;
    wrap.append(d);
  }

  const input = doc.createElement("input");
  input.type = "range";
  input.min = String(min);
  input.max = String(max);
  input.value = String(Math.round(currentVal));
  const updateTrack = () => {
    const pct = Math.max(0, Math.min(100, ((Number(input.value) - min) / Math.max(1, max - min)) * 100));
    input.style.cssText =
      `width:100%;height:6px;border-radius:3px;appearance:none;background:linear-gradient(90deg,#5865f2 ${pct}%,#4e5058 ${pct}%);outline:none;cursor:pointer;accent-color:#ffffff;margin:6px 0;`;
    badge.textContent = `${Math.round(Number(input.value))}${unit}`;
  };
  updateTrack();
  input.addEventListener("input", () => {
    currentVal = Number(input.value);
    updateTrack();
    onChange?.(currentVal);
  });
  wrap.append(input);
  return {
    root: wrap,
    setValue(/** @type {number} */ v) {
      currentVal = v;
      input.value = String(Math.round(v));
      updateTrack();
    },
  };
}

/**
 * Создаёт строку переключателя Discord (`f_00_05.jpg`, `f_01_06.jpg`) с опциональной ссылкой «Послушать звук».
 * @param {{
 *   doc: Document,
 *   title: string,
 *   desc?: string,
 *   checked: boolean,
 *   soundId?: string,
 *   backingInput?: HTMLInputElement | null,
 *   onChange?: (checked: boolean) => void,
 * }} cfg
 */
function createDiscordSwitchRow(cfg) {
  const { doc, title, desc, soundId, backingInput, onChange } = cfg;
  let isChecked = backingInput ? backingInput.checked : cfg.checked;

  const row = doc.createElement("div");
  row.className = "live-switch-row";

  const textCol = doc.createElement("div");
  textCol.className = "live-switch-text";

  const titleRow = doc.createElement("div");
  titleRow.style.cssText = "display:flex;align-items:center;gap:6px;";
  const titleEl = doc.createElement("span");
  titleEl.className = "live-switch-title";
  titleEl.textContent = title;
  const speakerIcon = doc.createElement("span");
  speakerIcon.style.cssText = "display:none;font-size:13px;color:#dbdee1;";
  speakerIcon.textContent = "🔊";
  titleRow.append(titleEl, speakerIcon);
  textCol.append(titleRow);

  if (desc) {
    const descEl = doc.createElement("div");
    descEl.className = "live-switch-desc";
    descEl.textContent = desc;
    textCol.append(descEl);
  }

  if (soundId) {
    const previewLink = doc.createElement("button");
    previewLink.type = "button";
    previewLink.style.cssText =
      "align-self:flex-start;padding:0;border:none;background:none;color:#00a8fc;font-size:12px;font-weight:500;cursor:pointer;margin-top:1px;";
    previewLink.textContent = "Послушать звук";
    previewLink.addEventListener("click", (ev) => {
      ev.stopPropagation();
      playSettingsPreviewSound(soundId, speakerIcon);
    });
    textCol.append(previewLink);
  }

  const sw = doc.createElement("button");
  sw.type = "button";
  sw.className = "live-switch-btn";
  sw.setAttribute("data-checked", isChecked ? "true" : "false");
  const thumb = doc.createElement("span");
  thumb.className = "live-switch-thumb";
  sw.append(thumb);

  const toggle = () => {
    isChecked = !isChecked;
    sw.setAttribute("data-checked", isChecked ? "true" : "false");
    if (backingInput) {
      backingInput.checked = isChecked;
      backingInput.setAttribute("aria-checked", isChecked ? "true" : "false");
      backingInput.dispatchEvent(new Event("change"));
    }
    onChange?.(isChecked);
  };
  sw.addEventListener("click", toggle);

  row.append(textCol, sw);
  return {
    root: row,
    setChecked(/** @type {boolean} */ next) {
      isChecked = next;
      sw.setAttribute("data-checked", isChecked ? "true" : "false");
    },
  };
}

/**
 * Модернизирует экран настроек `#screen-settings`, левую навигацию `#settings-nav`,
 * вкладку «Голос и видео» и вкладку «Уведомления» до полного 1:1 паритета с `Video_2026-11-01_11-35-06.mp4`.
 *
 * @param {{
 *   root: HTMLElement,
 *   nav: HTMLElement | null,
 *   voicePane: HTMLElement,
 *   notificationsPane: HTMLElement,
 *   toggles: HTMLInputElement[],
 *   invoke: ((cmd: string, args?: any) => Promise<any>) | null,
 *   showPane: (key: string) => void,
 *   guilds: any[],
 * }} ctx
 */
export function mountLiveDiscordSettings(ctx) {
  const { root, nav, voicePane, notificationsPane, toggles, invoke, showPane, guilds } = ctx;
  const doc = root.ownerDocument ?? document;
  ensureLiveSettingsStyles(doc);

  const screenSettings = doc.getElementById("screen-settings");
  if (screenSettings) {
    screenSettings.classList.add("live-settings-modal-mode");
    screenSettings.addEventListener("click", (ev) => {
      if (ev.target === screenSettings) {
        const closeBtn = /** @type {HTMLElement | null} */ (
          screenSettings.querySelector('[data-action="settings-close"]')
        );
        closeBtn?.click();
      }
    });
  }

  // Шапка модального окна с заголовком текущей вкладки слева и крестиком справа
  const settingsHead = doc.querySelector("#screen-settings .settings-head");
  let headerTitleNode = doc.getElementById("live-settings-head-title");
  if (settingsHead && !headerTitleNode) {
    headerTitleNode = doc.createElement("span");
    headerTitleNode.id = "live-settings-head-title";
    headerTitleNode.style.cssText = "font-size:15px;font-weight:700;color:#f2f3f5;margin-right:auto;";
    headerTitleNode.textContent = "Голос и видео";
    settingsHead.prepend(headerTitleNode);
  }

  // Верхний красный баннер «Режим рации включён, но горячие клавиши не назначены.» (f_00_22.jpg)
  let pttBanner = doc.getElementById("live-ptt-warning-banner");
  if (!pttBanner) {
    pttBanner = doc.createElement("div");
    pttBanner.id = "live-ptt-warning-banner";
    pttBanner.setAttribute("hidden", "");
    pttBanner.style.cssText =
      "position:fixed;top:0;left:0;right:0;height:38px;background:#3a191b;border-bottom:1px solid #f23f43;color:#f2f3f5;font-size:13px;font-weight:600;display:flex;align-items:center;justify-content:center;gap:12px;z-index:9600;";
    const pttText = doc.createElement("span");
    pttText.style.cssText = "color:#f23f43;";
    pttText.textContent = "Режим рации включён, но горячие клавиши не назначены.";
    const pttActionBtn = doc.createElement("button");
    pttActionBtn.type = "button";
    pttActionBtn.style.cssText =
      "padding:4px 10px;border:1px solid #f23f43;border-radius:6px;background:rgba(242,63,67,0.14);color:#f2f3f5;font-size:12px;font-weight:600;cursor:pointer;";
    pttActionBtn.textContent = "Назначить горячие клавиши";
    pttActionBtn.addEventListener("click", () => {
      showPane("Голос и видео");
      const keyBtn = doc.getElementById("live-ptt-record-btn");
      keyBtn?.scrollIntoView({ behavior: "smooth", block: "center" });
      keyBtn?.click();
    });
    pttBanner.append(pttText, pttActionBtn);
    doc.body.append(pttBanner);
  }

  /** @param {string} id */
  const findToggle = (id) => toggles.find((t) => t.getAttribute("data-setting-id") === id) ?? null;

  // Сохраняем канонические 38 `.toggle` в скрытом контейнере внутри `voicePane`,
  // чтобы `collectSettings()` / `applySettings()` продолжали работать с теми же инпутами.
  const backingVault = doc.createElement("div");
  backingVault.setAttribute("hidden", "");
  for (const t of toggles) {
    const lbl = t.closest(".toggle");
    if (lbl) backingVault.append(lbl);
  }

  // =========================================================================
  // 1. Построение живой вкладки «Голос и видео» (6 секций: Голос, Камера, Стриминг, Звуки, Звуковая панель, Расширенные)
  // =========================================================================
  voicePane.replaceChildren(backingVault);

  const secVoice = doc.createElement("div");
  secVoice.id = "live-sec-voice";
  secVoice.innerHTML = `<div class="live-sec-heading">Голос</div>`;

  const prefs = /** @type {any} */ (globalThis).__LIVE_VOICE_PREFS__ ?? {
    inputDeviceId: "default",
    outputDeviceId: "default",
    inputVolume: 100,
    outputVolume: 100,
    krispEnabled: true,
    muted: false,
    deafened: false,
  };

  const devicesGrid = doc.createElement("div");
  devicesGrid.style.cssText = "display:grid;grid-template-columns:1fr 1fr;gap:16px;margin-bottom:16px;";

  const micDropdown = createDiscordDropdown({
    doc,
    label: "Микрофон",
    icon: "🎙",
    options: [
      { id: "default", label: "Система: DM30 USB Microphone Моно" },
      { id: "Easy Effects Source", label: "Easy Effects Source" },
    ],
    selectedId: prefs.inputDeviceId || "default",
    onSelect: (id) => {
      prefs.inputDeviceId = id;
      if (invoke) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, { kind: "input", deviceId: id }).catch(() => {});
      }
    },
  });

  const speakerDropdown = createDiscordDropdown({
    doc,
    label: "Динамик",
    icon: "🎧",
    options: [
      { id: "default", label: "Система: HyperX Virtual Surround Sound" },
      { id: "Easy Effects Sink", label: "Easy Effects Sink" },
    ],
    selectedId: prefs.outputDeviceId || "default",
    onSelect: (id) => {
      prefs.outputDeviceId = id;
      if (invoke) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, { kind: "output", deviceId: id }).catch(() => {});
      }
    },
  });
  devicesGrid.append(micDropdown.root, speakerDropdown.root);
  secVoice.append(devicesGrid);

  // Живая подгрузка реальных устройств PipeWire/PulseAudio из `voice_audio_devices`
  const syncAudioDevicesFromBackend = async () => {
    if (!invoke) return;
    try {
      const res = await invoke(VOICE_AUDIO_DEVICES_IPC_CMD);
      const rawInputs = Array.isArray(res?.inputs) ? res.inputs : [];
      const rawOutputs = Array.isArray(res?.outputs) ? res.outputs : [];
      const defInId = String(res?.defaultInput || "");
      const defOutId = String(res?.defaultOutput || "");
      if (rawInputs.length > 0) {
        const defInObj = rawInputs.find((/** @type {any} */ d) => d.id === defInId) ?? rawInputs[0];
        const inOpts = [
          {
            id: "default",
            label: `Система: ${defInObj?.name || defInObj?.id || "По умолчанию"}`,
          },
          ...rawInputs.map((/** @type {any} */ d) => ({
            id: String(d.id),
            label: String(d.name || d.id),
          })),
        ];
        micDropdown.setOptions(inOpts, prefs.inputDeviceId || "default");
      }
      if (rawOutputs.length > 0) {
        const defOutObj = rawOutputs.find((/** @type {any} */ d) => d.id === defOutId) ?? rawOutputs[0];
        const outOpts = [
          {
            id: "default",
            label: `Система: ${defOutObj?.name || defOutObj?.id || "По умолчанию"}`,
          },
          ...rawOutputs.map((/** @type {any} */ d) => ({
            id: String(d.id),
            label: String(d.name || d.id),
          })),
        ];
        speakerDropdown.setOptions(outOpts, prefs.outputDeviceId || "default");
      }
    } catch {}
  };
  void syncAudioDevicesFromBackend();

  const volumesGrid = doc.createElement("div");
  volumesGrid.style.cssText = "display:grid;grid-template-columns:1fr 1fr;gap:16px;margin-bottom:18px;";
  const micVolSlider = createDiscordSlider({
    doc,
    label: "Громкость микрофона",
    min: 0,
    max: 100,
    value: prefs.inputVolume ?? 100,
    onChange: (val) => {
      prefs.inputVolume = val;
      if (invoke) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "input",
          deviceId: prefs.inputDeviceId || "default",
          volumePct: Math.round(val),
        }).catch(() => {});
      }
    },
  });
  const speakerVolSlider = createDiscordSlider({
    doc,
    label: "Громкость динамика",
    min: 0,
    max: 200,
    value: prefs.outputVolume ?? 100,
    onChange: (val) => {
      prefs.outputVolume = val;
      if (invoke) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "output",
          deviceId: prefs.outputDeviceId || "default",
          volumePct: Math.round(val),
        }).catch(() => {});
      }
    },
  });
  volumesGrid.append(micVolSlider.root, speakerVolSlider.root);
  secVoice.append(volumesGrid);

  // Проверка микрофона (38 вертикальных столбиков + loopback + предупреждения f_01_51.jpg / f_02_37.jpg)
  const micTestBlock = doc.createElement("div");
  micTestBlock.style.cssText = "display:flex;flex-direction:column;gap:8px;margin-bottom:14px;";
  const micTestRow = doc.createElement("div");
  micTestRow.style.cssText = "display:flex;align-items:center;gap:14px;";
  const micTestBtn = doc.createElement("button");
  micTestBtn.type = "button";
  micTestBtn.style.cssText =
    "height:38px;padding:0 16px;border:none;border-radius:8px;background:#5865f2;color:#ffffff;font-size:13px;font-weight:600;cursor:pointer;flex-shrink:0;";
  micTestBtn.textContent = "Проверка микрофона";

  const meterRightCol = doc.createElement("div");
  meterRightCol.style.cssText = "display:flex;flex-direction:column;gap:6px;flex:1;min-width:0;";
  const barsContainer = doc.createElement("div");
  barsContainer.style.cssText = "display:flex;align-items:center;gap:4px;height:22px;width:100%;";
  /** @type {HTMLElement[]} */
  const meterBars = [];
  for (let i = 0; i < 38; i += 1) {
    const bar = doc.createElement("span");
    bar.style.cssText =
      "flex:1;height:18px;border-radius:2px;background:#383a40;transition:background 65ms ease;";
    meterBars.push(bar);
    barsContainer.append(bar);
  }
  const meterStatusText = doc.createElement("div");
  meterStatusText.style.cssText = "font-size:12px;color:#b5bac1;min-height:16px;";
  meterRightCol.append(barsContainer, meterStatusText);
  micTestRow.append(micTestBtn, meterRightCol);

  const helpCaption = doc.createElement("div");
  helpCaption.style.cssText = "font-size:12px;color:#949ba4;";
  helpCaption.innerHTML =
    `Нужна помощь? Загляните в <span style="color:#00a8fc;cursor:pointer;">руководство по устранению неполадок</span>.`;
  micTestBlock.append(micTestRow, helpCaption);
  secVoice.append(micTestBlock);

  /** @type {ReturnType<typeof setInterval> | null} */
  let settingsMicTimer = null;
  let micTestingActive = false;
  let silentTicks = 0;

  const stopSettingsMicTest = () => {
    micTestingActive = false;
    micTestBtn.textContent = "Проверка микрофона";
    if (settingsMicTimer) {
      clearInterval(settingsMicTimer);
      settingsMicTimer = null;
    }
    if (invoke) {
      void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, { kind: "mic_test_stop" }).catch(() => {});
    }
    meterStatusText.textContent = "";
    for (const b of meterBars) {
      b.style.background = "#383a40";
    }
  };

  micTestBtn.addEventListener("click", () => {
    if (micTestingActive) {
      stopSettingsMicTest();
      return;
    }
    micTestingActive = true;
    silentTicks = 0;
    micTestBtn.textContent = "Прекратить проверку";
    if (invoke) {
      void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, { kind: "mic_test_start" }).catch(() => {});
    }
    settingsMicTimer = setInterval(async () => {
      let db = -100;
      if (invoke) {
        try {
          const st = await invoke(VOICE_RTC_STATUS_IPC_CMD);
          if (typeof st?.localMicLevelDb === "number") {
            db = st.localMicLevelDb;
          }
        } catch {}
      }
      const isMuted = Boolean((/** @type {any} */ (globalThis).__LIVE_VOICE_PREFS__)?.muted);
      if (isMuted) {
        for (const b of meterBars) b.style.background = "#383a40";
        meterStatusText.innerHTML = `<b style="color:#f2f3f5;">Режим глушения микрофона включён.</b> Чтобы говорить, отключите его.`;
        return;
      }
      const litCount = db > -72 ? Math.max(2, Math.min(38, Math.round(((db + 72) / 62) * 38))) : 0;
      if (litCount > 2) {
        silentTicks = 0;
        meterStatusText.textContent = "Воспроизводим ваш прекрасный голос";
      } else {
        silentTicks += 1;
        if (silentTicks >= 22) {
          meterStatusText.innerHTML = `<div style="font-weight:700;color:#f2f3f5;">Discord не может обнаружить звук с вашего микрофона.</div><div>Убедитесь, что вы выбрали правильное устройство ввода. <span style="color:#00a8fc;cursor:pointer;">Помогите!</span></div>`;
        } else {
          meterStatusText.textContent = "Воспроизводим ваш прекрасный голос";
        }
      }
      meterBars.forEach((b, idx) => {
        if (idx === 1 && litCount <= 2) {
          b.style.background = "#f0b232";
        } else if (idx < litCount) {
          b.style.background = idx < 4 ? "#f0b232" : "#23a55a";
        } else {
          b.style.background = "#383a40";
        }
      });
    }, 95);
  });

  secVoice.append(doc.createElement("hr"));
  /** @type {HTMLElement} */ (secVoice.lastElementChild).className = "live-sec-divider";

  // Профиль ввода (3 радио-опции: Изоляция голоса, Студия, Пользовательский)
  const profileBlock = doc.createElement("div");
  profileBlock.style.cssText = "display:flex;flex-direction:column;gap:10px;margin-bottom:14px;";
  const profileHeading = doc.createElement("div");
  profileHeading.style.cssText = "font-size:14px;font-weight:600;color:#f2f3f5;";
  profileHeading.textContent = "Профиль ввода";
  profileBlock.append(profileHeading);

  const profileSpecs = [
    {
      id: "isolation",
      title: "Изоляция голоса",
      desc: "Только ваш прекрасный голос: Discord удалит ненужный шум",
    },
    {
      id: "studio",
      title: "Студия",
      desc: "Чистый звук: открытый микрофон без обработки",
    },
    {
      id: "custom",
      title: "Пользовательский",
      desc: "Продвинутый режим: мне нужны все кнопки и переключатели!",
    },
  ];
  let selectedProfile = "custom";
  /** @type {Array<{ id: string, dot: HTMLElement }>} */
  const profileDots = [];
  for (const p of profileSpecs) {
    const pRow = doc.createElement("button");
    pRow.type = "button";
    pRow.style.cssText =
      "display:flex;align-items:flex-start;gap:12px;padding:6px 0;border:none;background:transparent;color:#f2f3f5;cursor:pointer;text-align:left;";
    const dot = doc.createElement("span");
    const renderDot = (/** @type {boolean} */ active) => {
      dot.style.cssText =
        `width:18px;height:18px;border-radius:50%;border:2px solid ${active ? "#5865f2" : "#80848e"};background:${active ? "#5865f2" : "transparent"};display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;margin-top:2px;`;
      dot.innerHTML = active
        ? `<span style="width:6px;height:6px;border-radius:50%;background:#ffffff;display:block;"></span>`
        : "";
    };
    renderDot(p.id === selectedProfile);
    profileDots.push({ id: p.id, dot });
    const txt = doc.createElement("div");
    txt.innerHTML = `<div style="font-size:13px;font-weight:600;color:#f2f3f5;">${p.title}</div><div style="font-size:12px;color:#949ba4;margin-top:2px;">${p.desc}</div>`;
    pRow.append(dot, txt);
    pRow.addEventListener("click", () => {
      selectedProfile = p.id;
      for (const item of profileDots) {
        const isAct = item.id === selectedProfile;
        item.dot.style.borderColor = isAct ? "#5865f2" : "#80848e";
        item.dot.style.background = isAct ? "#5865f2" : "transparent";
        item.dot.innerHTML = isAct
          ? `<span style="width:6px;height:6px;border-radius:50%;background:#ffffff;display:block;"></span>`
          : "";
      }
      if (invoke) {
        if (selectedProfile === "isolation") {
          prefs.krispEnabled = true;
          prefs.autoGainControl = true;
          void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, { kind: "krisp", muted: true }).catch(() => {});
          void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, { kind: "agc", muted: true }).catch(() => {});
        } else if (selectedProfile === "studio") {
          prefs.krispEnabled = false;
          prefs.autoGainControl = false;
          void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, { kind: "krisp", muted: false }).catch(() => {});
          void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, { kind: "agc", muted: false }).catch(() => {});
        }
      }
    });
    profileBlock.append(pRow);
  }
  secVoice.append(profileBlock);

  // Автоматически определять чувствительность ввода + живая шкала порога (-100dB .. 0dB, f_00_12.jpg)
  const vadBarWrap = doc.createElement("div");
  vadBarWrap.style.cssText = "margin-bottom:14px;display:flex;flex-direction:column;gap:8px;";
  let autoSensEnabled = prefs.autoSensitivity !== false;
  let manualVadDb = typeof prefs.vadThresholdDb === "number" ? prefs.vadThresholdDb : -45;

  const vadMeterContainer = doc.createElement("div");
  vadMeterContainer.style.cssText =
    "position:relative;height:10px;border-radius:5px;background:#2b2d31;overflow:hidden;width:100%;border:1px solid rgba(255,255,255,0.06);";
  const vadMeterFill = doc.createElement("div");
  vadMeterFill.style.cssText =
    "height:100%;width:0%;border-radius:5px;background:#23a55a;transition:width 65ms ease,background 65ms ease;";
  const vadThresholdMarker = doc.createElement("div");
  vadThresholdMarker.style.cssText =
    "position:absolute;top:0;bottom:0;width:3px;background:#f2f3f5;box-shadow:0 0 4px rgba(0,0,0,0.8);left:55%;pointer-events:none;";
  vadMeterContainer.append(vadMeterFill, vadThresholdMarker);

  const vadManualSlider = doc.createElement("input");
  vadManualSlider.type = "range";
  vadManualSlider.min = "-85";
  vadManualSlider.max = "-5";
  vadManualSlider.value = String(manualVadDb);
  vadManualSlider.style.cssText =
    `display:${autoSensEnabled ? "none" : "block"};width:100%;height:8px;border-radius:4px;accent-color:#23a55a;cursor:pointer;margin-top:2px;`;
  vadManualSlider.addEventListener("input", () => {
    manualVadDb = Number(vadManualSlider.value || -45);
    prefs.vadThresholdDb = manualVadDb;
    const pct = Math.max(0, Math.min(100, ((manualVadDb + 85) / 80) * 100));
    vadThresholdMarker.style.left = `${pct}%`;
    if (invoke) {
      void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "vad_threshold",
        deviceId: String(manualVadDb),
        muted: false,
      }).catch(() => {});
    }
  });

  const autoSensSwitch = createDiscordSwitchRow({
    doc,
    title: "Автоматически определять чувствительность ввода",
    desc: "Контролирует чувствительность к звукам вашего микрофона в Discord. Скажите что-нибудь, чтобы проверить микрофон. Если шкала заполнена, это значит, что Discord транслирует ваш прекрасный голос.",
    checked: autoSensEnabled,
    backingInput: findToggle("input.auto_sensitivity"),
    onChange: (checked) => {
      autoSensEnabled = checked;
      prefs.autoSensitivity = checked;
      vadManualSlider.style.display = checked ? "none" : "block";
      if (invoke) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "auto_sensitivity",
          muted: Boolean(checked),
        }).catch(() => {});
      }
    },
  });
  vadBarWrap.append(autoSensSwitch.root, vadMeterContainer, vadManualSlider);
  secVoice.append(vadBarWrap);

  const div2 = doc.createElement("hr");
  div2.className = "live-sec-divider";
  secVoice.append(div2);

  // Шумоподавление (Krisp / Стандартное / Пусто, f_00_13.jpg)
  const noiseRow = doc.createElement("div");
  noiseRow.style.cssText = "display:flex;align-items:flex-start;justify-content:space-between;gap:18px;padding:8px 0;";
  const noiseLeft = doc.createElement("div");
  noiseLeft.innerHTML = `
    <div style="font-size:14px;font-weight:600;color:#f2f3f5;">Шумоподавление</div>
    <div style="font-size:12px;color:#949ba4;margin-top:3px;">Уменьшает фоновый шум микрофона. Предоставляется <span style="color:#00a8fc;">Krisp</span>.</div>
    <div style="font-size:18px;font-weight:900;color:#f2f3f5;letter-spacing:-0.6px;margin-top:4px;font-family:sans-serif;">krisp</div>
  `;
  const noiseRight = doc.createElement("div");
  noiseRight.style.cssText = "width:210px;flex-shrink:0;";
  const noiseDropdown = createDiscordDropdown({
    doc,
    options: [
      { id: "krisp", label: "Krisp" },
      { id: "standard", label: "Стандартное" },
      { id: "none", label: "Пусто" },
    ],
    selectedId: prefs.krispEnabled === false ? "none" : "krisp",
    onSelect: (id) => {
      prefs.krispEnabled = id !== "none";
      if (invoke) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "krisp",
          muted: Boolean(prefs.krispEnabled),
        }).catch(() => {});
      }
    },
  });
  noiseRight.append(noiseDropdown.root);
  noiseRow.append(noiseLeft, noiseRight);
  secVoice.append(noiseRow);

  // Эхоподавление
  secVoice.append(
    createDiscordSwitchRow({
      doc,
      title: "Эхоподавление",
      checked: true,
      backingInput: findToggle("noise.echo_cancel"),
    }).root,
  );

  // Режим рации (Push-to-Talk) + запись горячей клавиши + задержка отключения (f_00_22.jpg, f_00_31.jpg)
  let pttAssignedKey = "";
  let pttRecording = false;
  const pttDetailsBox = doc.createElement("div");
  pttDetailsBox.setAttribute("hidden", "");
  pttDetailsBox.style.cssText = "display:flex;flex-direction:column;gap:14px;padding:10px 0 6px;";

  const pttKeyRow = doc.createElement("div");
  pttKeyRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:16px;";
  const pttKeyLeft = doc.createElement("div");
  pttKeyLeft.innerHTML = `
    <div style="font-size:14px;font-weight:600;color:#f2f3f5;">Горячая клавиша для режима рации</div>
    <div style="font-size:12px;color:#949ba4;margin-top:2px;">Вы можете добавить несколько комбинаций для режима рации в <span style="color:#00a8fc;">настройках горячих клавиш</span>.</div>
  `;
  const pttKeyBox = doc.createElement("div");
  pttKeyBox.style.cssText =
    "display:flex;align-items:center;gap:8px;padding:4px 6px 4px 12px;border-radius:8px;border:1px solid #383a40;background:#1e1f22;flex-shrink:0;";
  const pttKeyText = doc.createElement("span");
  pttKeyText.style.cssText = "font-size:12px;color:#949ba4;max-width:130px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
  pttKeyText.textContent = "Горячие клавиши не наз...";
  const pttRecordBtn = doc.createElement("button");
  pttRecordBtn.type = "button";
  pttRecordBtn.id = "live-ptt-record-btn";
  pttRecordBtn.style.cssText =
    "padding:6px 10px;border:none;border-radius:6px;background:#35373c;color:#f2f3f5;font-size:12px;font-weight:600;cursor:pointer;";
  pttRecordBtn.textContent = "Установить г...";

  const syncPttBanner = (/** @type {boolean} */ pttOn) => {
    if (pttOn && !pttAssignedKey) {
      pttBanner?.removeAttribute("hidden");
    } else {
      pttBanner?.setAttribute("hidden", "");
    }
  };

  pttRecordBtn.addEventListener("click", () => {
    pttRecording = !pttRecording;
    if (pttRecording) {
      pttKeyBox.style.borderColor = "#f23f43";
      pttRecordBtn.style.background = "rgba(242,63,67,0.22)";
      pttRecordBtn.style.color = "#f23f43";
      pttRecordBtn.textContent = "Остановить з...";
      const onKey = (/** @type {KeyboardEvent} */ ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        if (["Shift", "Control", "Alt", "Meta"].includes(ev.key)) return;
        pttAssignedKey = ev.key === " " ? "Space" : ev.key.toUpperCase();
        pttKeyText.textContent = pttAssignedKey;
        pttKeyText.style.color = "#f2f3f5";
        pttRecording = false;
        pttKeyBox.style.borderColor = "#383a40";
        pttRecordBtn.style.background = "#35373c";
        pttRecordBtn.style.color = "#f2f3f5";
        pttRecordBtn.textContent = "Установить г...";
        syncPttBanner(true);
        doc.removeEventListener("keydown", onKey, true);
      };
      doc.addEventListener("keydown", onKey, true);
    } else {
      pttKeyBox.style.borderColor = "#383a40";
      pttRecordBtn.style.background = "#35373c";
      pttRecordBtn.style.color = "#f2f3f5";
      pttRecordBtn.textContent = "Установить г...";
    }
  });
  pttKeyBox.append(pttKeyText, pttRecordBtn);
  pttKeyRow.append(pttKeyLeft, pttKeyBox);

  const pttDelaySlider = createDiscordSlider({
    doc,
    label: "Задержка отключения в режиме рации",
    min: 0,
    max: 2000,
    value: 20,
    unit: " мс",
  });
  pttDetailsBox.append(pttKeyRow, pttDelaySlider.root);

  const pttSwitch = createDiscordSwitchRow({
    doc,
    title: "Режим рации",
    checked: false,
    backingInput: findToggle("voice.ptt_mode"),
    onChange: (checked) => {
      if (checked) pttDetailsBox.removeAttribute("hidden");
      else pttDetailsBox.setAttribute("hidden", "");
      syncPttBanner(checked);
    },
  });
  secVoice.append(pttSwitch.root, pttDetailsBox);

  // Показать расширенные настройки голоса (f_00_36.jpg)
  const advVoiceBtn = doc.createElement("button");
  advVoiceBtn.type = "button";
  advVoiceBtn.className = "live-accordion-header";
  const advVoiceLeft = doc.createElement("div");
  advVoiceLeft.innerHTML = `
    <div style="font-size:14px;font-weight:600;color:#f2f3f5;">Показать расширенные настройки голоса</div>
    <div style="font-size:12px;color:#949ba4;margin-top:2px;">Автоматическая регулировка усиления, Расширенная активация по голосу, Предложение аудиоустройств и другие настройки.</div>
  `;
  const advVoiceChev = doc.createElement("span");
  advVoiceChev.className = "live-accordion-chevron";
  advVoiceChev.textContent = "⌄";
  advVoiceBtn.append(advVoiceLeft, advVoiceChev);

  const advVoiceContent = doc.createElement("div");
  advVoiceContent.setAttribute("hidden", "");
  advVoiceContent.append(
    createDiscordSwitchRow({
      doc,
      title: "Автоматическая регулировка усиления",
      checked: prefs.autoGainControl !== false,
      backingInput: findToggle("voice.agc"),
      onChange: (checked) => {
        prefs.autoGainControl = checked;
        if (invoke) {
          void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
            kind: "agc",
            muted: Boolean(checked),
          }).catch(() => {});
        }
      },
    }).root,
    createDiscordSwitchRow({
      doc,
      title: "Расширенная активация по голосу",
      checked: true,
      backingInput: findToggle("voice.advanced_activation"),
    }).root,
    createDiscordSwitchRow({
      doc,
      title: "Предлагать устройства",
      checked: true,
      backingInput: findToggle("voice.suggest_devices"),
    }).root,
    createDiscordSwitchRow({
      doc,
      title: "Предупреждать при отсутствии звука",
      checked: true,
      backingInput: findToggle("voice.warn_no_sound"),
    }).root,
    createDiscordSwitchRow({
      doc,
      title: "Предупреждать при смене канала",
      checked: true,
      backingInput: findToggle("voice.warn_channel_change"),
    }).root,
    createDiscordSwitchRow({
      doc,
      title: "Приоритет QoS",
      checked: false,
      backingInput: findToggle("voice.qos_priority"),
    }).root,
  );
  advVoiceBtn.addEventListener("click", () => {
    const isHid = advVoiceContent.hasAttribute("hidden");
    if (isHid) advVoiceContent.removeAttribute("hidden");
    else advVoiceContent.setAttribute("hidden", "");
    advVoiceChev.textContent = isHid ? "⌃" : "⌄";
  });
  secVoice.append(advVoiceBtn, advVoiceContent);

  if (invoke && typeof setInterval === "function") {
    setInterval(async () => {
      if (!secVoice.isConnected || secVoice.hasAttribute("hidden")) return;
      const settingsScreen = doc.getElementById("screen-settings");
      if (settingsScreen && settingsScreen.hasAttribute("hidden")) return;
      try {
        const st = await invoke(VOICE_RTC_STATUS_IPC_CMD);
        const db = typeof st?.localMicLevelDb === "number" ? st.localMicLevelDb : -100;
        const effVad =
          typeof st?.effectiveVadDb === "number"
            ? st.effectiveVadDb
            : autoSensEnabled
              ? -45
              : manualVadDb;
        const levelPct = db > -85 ? Math.max(2, Math.min(100, ((db + 85) / 80) * 100)) : 0;
        const threshPct = Math.max(2, Math.min(98, ((effVad + 85) / 80) * 100));
        vadMeterFill.style.width = `${levelPct}%`;
        vadMeterFill.style.background = db >= effVad ? "#23a55a" : "#f0b232";
        vadThresholdMarker.style.left = `${threshPct}%`;
      } catch {}
    }, 110);
  }

  const divCam = doc.createElement("hr");
  divCam.className = "live-sec-divider";
  secVoice.append(divCam);

  // =========================================================================
  // Секция 2: «Камера» (`#live-sec-camera`, f_00_39.jpg)
  // =========================================================================
  const secCamera = doc.createElement("div");
  secCamera.id = "live-sec-camera";
  secCamera.innerHTML = `<div class="live-sec-heading">Камера</div>`;

  const camPreviewBox = doc.createElement("div");
  camPreviewBox.style.cssText =
    "height:208px;border-radius:10px;border:1px solid #383a40;background:#18191c;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;margin-bottom:14px;position:relative;overflow:hidden;";
  const testVideoBtn = doc.createElement("button");
  testVideoBtn.type = "button";
  testVideoBtn.style.cssText =
    "padding:9px 18px;border:none;border-radius:8px;background:#5865f2;color:#ffffff;font-size:13px;font-weight:600;cursor:pointer;";
  testVideoBtn.textContent = "Проверить видео";
  let videoTesting = false;
  testVideoBtn.addEventListener("click", () => {
    videoTesting = !videoTesting;
    testVideoBtn.textContent = videoTesting ? "Остановить видео" : "Проверить видео";
    camPreviewBox.style.background = videoTesting
      ? "radial-gradient(circle at 50% 45%, #2b3252, #111214)"
      : "#18191c";
  });
  camPreviewBox.append(testVideoBtn);
  secCamera.append(camPreviewBox);

  secCamera.append(
    createDiscordSwitchRow({
      doc,
      title: "Предпросмотр видео (всегда)",
      desc: "Использовать предпросмотр каждый раз, как вы включаете видео",
      checked: true,
      backingInput: findToggle("video.preview"),
    }).root,
  );

  const camDropdown = createDiscordDropdown({
    doc,
    label: "Камера",
    icon: "📷",
    options: [
      { id: "none", label: "Нет видеоустройств" },
      { id: "obs_virtual", label: "OBS Virtual Camera (V4L2)" },
    ],
    selectedId: "none",
    onSelect: () => {},
  });
  camDropdown.root.style.marginTop = "10px";
  secCamera.append(camDropdown.root);

  const bgLabel = doc.createElement("div");
  bgLabel.style.cssText = "font-size:14px;font-weight:600;color:#f2f3f5;margin:18px 0 10px;";
  bgLabel.textContent = "Фон видео";
  const bgGrid = doc.createElement("div");
  bgGrid.style.cssText = "display:grid;grid-template-columns:repeat(4,1fr);gap:10px;";
  const bgPresets = [
    { id: "none", title: "Пусто", icon: "🚫", gradient: "#1e1f22" },
    { id: "blur", title: "Размытие", icon: "💧", gradient: "#1e1f22" },
    { id: "custom", title: "Пользовательские", badge: "НОВОЕ", icon: "🖼", gradient: "#1e1f22" },
    { id: "cyber", title: "Киберпанк", gradient: "linear-gradient(135deg,#2a1b4e,#184e68)" },
    { id: "lofi", title: "Лоу-фай комната", gradient: "linear-gradient(135deg,#4b2c46,#8f4f4f)" },
    { id: "sunset", title: "Закат", gradient: "linear-gradient(135deg,#5b247a,#1bcedf)" },
    { id: "space", title: "Галактика", gradient: "linear-gradient(135deg,#0f2027,#2c5364)" },
  ];
  let selectedBg = "none";
  for (const bg of bgPresets) {
    const card = doc.createElement("button");
    card.type = "button";
    card.style.cssText =
      `height:82px;border-radius:8px;border:2px solid ${bg.id === selectedBg ? "#5865f2" : "#383a40"};background:${bg.gradient};color:#f2f3f5;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:4px;cursor:pointer;position:relative;font-size:12px;font-weight:600;`;
    if (bg.badge) {
      const b = doc.createElement("span");
      b.style.cssText =
        "position:absolute;top:6px;left:6px;padding:1px 6px;border-radius:8px;background:#5865f2;color:#ffffff;font-size:9px;font-weight:800;";
      b.textContent = bg.badge;
      card.append(b);
    }
    if (bg.icon) {
      const ic = doc.createElement("span");
      ic.style.cssText = "font-size:18px;color:#b5bac1;";
      ic.textContent = bg.icon;
      card.append(ic);
    }
    const t = doc.createElement("span");
    t.textContent = bg.title;
    card.append(t);
    card.addEventListener("click", () => {
      selectedBg = bg.id;
      for (const ch of bgGrid.children) {
        /** @type {HTMLElement} */ (ch).style.borderColor = "#383a40";
      }
      card.style.borderColor = "#5865f2";
    });
    bgGrid.append(card);
  }
  secCamera.append(bgLabel, bgGrid);

  const divStream = doc.createElement("hr");
  divStream.className = "live-sec-divider";
  secCamera.append(divStream);

  // =========================================================================
  // Секция 3: «Стриминг» (`#live-sec-streaming`, f_00_44.jpg)
  // =========================================================================
  const secStreaming = doc.createElement("div");
  secStreaming.id = "live-sec-streaming";
  secStreaming.innerHTML = `<div class="live-sec-heading">Стриминг</div>`;

  secStreaming.append(
    createDiscordSwitchRow({
      doc,
      title: "Показывать предпросмотр стрима",
      desc: "Показывает зрителям предпросмотр стрима, до того как они присоединятся.",
      checked: true,
      backingInput: findToggle("stream.preview"),
    }).root,
  );

  const expCaptureRow = createDiscordSwitchRow({
    doc,
    title: "Использовать наши новейшие технологии для захвата экрана",
    desc: "Позволяет захватывать отдельные окна приложений через KWin / PipeWire без захвата главного окна Discord.",
    checked: true,
    onChange: (checked) => {
      if (invoke) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "experimental_screen_capture",
          deviceId: checked ? "1" : "0",
          muted: checked,
        }).catch(() => {});
      }
    },
  });
  expCaptureRow.root.setAttribute("data-setting-row", "experimental-screen-capture");
  const expCaptureBtn = expCaptureRow.root.querySelector(".live-switch-btn");
  if (expCaptureBtn) {
    expCaptureBtn.id = "live-settings-experimental-capture-toggle";
    expCaptureBtn.setAttribute(
      "aria-label",
      "Использовать наши новейшие технологии для захвата экрана",
    );
  }
  secStreaming.append(expCaptureRow.root);

  const advStreamBtn = doc.createElement("button");
  advStreamBtn.type = "button";
  advStreamBtn.className = "live-accordion-header";
  const advStreamTitle = doc.createElement("span");
  advStreamTitle.style.cssText = "font-size:14px;font-weight:600;color:#f2f3f5;";
  advStreamTitle.textContent = "Показать расширенные настройки стрима";
  const advStreamChev = doc.createElement("span");
  advStreamChev.className = "live-accordion-chevron";
  advStreamChev.textContent = "⌄";
  advStreamBtn.append(advStreamTitle, advStreamChev);

  const advStreamContent = doc.createElement("div");
  advStreamContent.setAttribute("hidden", "");
  advStreamContent.append(
    createDiscordSwitchRow({
      doc,
      title: "Приглушение стрима",
      desc: "Автоматически уменьшать громкость стрима, когда говорят люди.",
      checked: true,
      backingInput: findToggle("stream.mute"),
    }).root,
    createDiscordSlider({
      doc,
      label: "Сила приглушения стрима",
      min: 1,
      max: 100,
      value: 49,
    }).root,
  );
  advStreamBtn.addEventListener("click", () => {
    const isHid = advStreamContent.hasAttribute("hidden");
    if (isHid) advStreamContent.removeAttribute("hidden");
    else advStreamContent.setAttribute("hidden", "");
    advStreamTitle.textContent = isHid
      ? "Скрыть расширенные настройки стрима"
      : "Показать расширенные настройки стрима";
    advStreamChev.textContent = isHid ? "⌃" : "⌄";
  });
  secStreaming.append(advStreamBtn, advStreamContent);

  const divSounds = doc.createElement("hr");
  divSounds.className = "live-sec-divider";
  secStreaming.append(divSounds);

  // =========================================================================
  // Секция 4: «Звуки» (`#live-sec-sounds`, f_00_56.jpg, f_01_02.jpg)
  // =========================================================================
  const secSounds = doc.createElement("div");
  secSounds.id = "live-sec-sounds";
  secSounds.innerHTML = `<div class="live-sec-heading">Звуки</div>`;

  const primarySounds = [
    { id: "sounds.deafen", label: "Откл. звук" },
    { id: "sounds.undeafen", label: "Включить звук в динамиках" },
    { id: "sounds.mute", label: "Заглушить" },
    { id: "sounds.unmute", label: "Вкл. микрофон" },
  ];
  for (const s of primarySounds) {
    secSounds.append(
      createDiscordSwitchRow({
        doc,
        title: s.label,
        checked: true,
        soundId: s.id,
        backingInput: findToggle(s.id),
      }).root,
    );
  }

  const extraSounds = [
    { id: "sounds.camera_on", label: "Режим молчания" },
    { id: "sounds.camera_off", label: "Откл. видео" },
    { id: "sounds.voice_disconnected", label: "Голосовая связь отключена" },
    { id: "sounds.ptt_activation", label: "Активация режима рации" },
    { id: "sounds.ptt_deactivation", label: "Деактивация режима рации" },
    { id: "sounds.user_join", label: "Пользователь присоединился" },
    { id: "sounds.user_leave", label: "Пользователь вышел" },
    { id: "sounds.user_moved", label: "Пользователь перемещён" },
    { id: "sounds.outgoing_call", label: "Исходящий звонок" },
    { id: "sounds.stream_started", label: "Стрим начался" },
    { id: "sounds.stream_stopped", label: "Стрим остановлен" },
    { id: "sounds.viewer_join", label: "Зритель присоединился" },
    { id: "sounds.viewer_leave", label: "Зритель вышел" },
    { id: "sounds.activity_start", label: "Начало активности" },
    { id: "sounds.activity_end", label: "Завершение активности" },
    { id: "sounds.activity_join", label: "Присоединение пользователя к активности" },
    { id: "sounds.activity_leave", label: "Пользователь покинул активность" },
    { id: "sounds.activity_invite", label: "Приглашение выступить" },
  ];
  const extraSoundsWrap = doc.createElement("div");
  extraSoundsWrap.setAttribute("hidden", "");
  for (const s of extraSounds) {
    extraSoundsWrap.append(
      createDiscordSwitchRow({
        doc,
        title: s.label,
        checked: true,
        soundId: s.id,
        backingInput: findToggle(s.id),
      }).root,
    );
  }

  const show18Btn = doc.createElement("button");
  show18Btn.type = "button";
  show18Btn.className = "live-accordion-header";
  const show18Title = doc.createElement("span");
  show18Title.style.cssText = "font-size:14px;font-weight:600;color:#f2f3f5;";
  show18Title.textContent = "Показать ещё 18 звуков";
  const show18Chev = doc.createElement("span");
  show18Chev.className = "live-accordion-chevron";
  show18Chev.textContent = "⌄";
  show18Btn.append(show18Title, show18Chev);
  show18Btn.addEventListener("click", () => {
    const isHid = extraSoundsWrap.hasAttribute("hidden");
    if (isHid) extraSoundsWrap.removeAttribute("hidden");
    else extraSoundsWrap.setAttribute("hidden", "");
    show18Title.textContent = isHid ? "Скрыть 18 звуков" : "Показать ещё 18 звуков";
    show18Chev.textContent = isHid ? "⌃" : "⌄";
  });
  secSounds.append(extraSoundsWrap, show18Btn);

  // Карточка «Связанные настройки -> Уведомления» (f_01_02.jpg)
  const relatedLabel = doc.createElement("div");
  relatedLabel.style.cssText = "font-size:13px;font-weight:600;color:#b5bac1;margin:14px 0 8px;";
  relatedLabel.textContent = "Связанные настройки";
  const relatedCard = doc.createElement("button");
  relatedCard.type = "button";
  relatedCard.style.cssText =
    "display:flex;align-items:center;justify-content:space-between;gap:14px;width:100%;padding:14px 16px;border-radius:10px;border:1px solid #383a40;background:#232428;color:#f2f3f5;cursor:pointer;text-align:left;";
  relatedCard.innerHTML = `
    <span style="display:flex;align-items:center;gap:12px;">
      <span style="width:36px;height:36px;border-radius:8px;background:#2b2d31;display:inline-flex;align-items:center;justify-content:center;font-size:16px;">🔔</span>
      <span>
        <div style="font-size:14px;font-weight:600;color:#f2f3f5;">Уведомления</div>
        <div style="font-size:12px;color:#949ba4;margin-top:2px;">Включить/отключить звуки для новых сообщений и входящих звонков.</div>
      </span>
    </span>
    <span style="font-size:18px;color:#b5bac1;">›</span>
  `;
  relatedCard.addEventListener("click", () => {
    showPane("Уведомления");
  });
  secSounds.append(relatedLabel, relatedCard);

  const divSb = doc.createElement("hr");
  divSb.className = "live-sec-divider";
  secSounds.append(divSb);

  // =========================================================================
  // Секция 5: «Звуковая панель» (`#live-sec-soundboard`, f_01_34.jpg)
  // =========================================================================
  const secSoundboard = doc.createElement("div");
  secSoundboard.id = "live-sec-soundboard";
  secSoundboard.innerHTML = `<div class="live-sec-heading">Звуковая панель</div>`;

  secSoundboard.append(
    createDiscordSlider({
      doc,
      label: "Громкость звуковой панели",
      desc: "Управляйте громкостью звуков, которые вы слышите. Узнать больше о настройке громкости звуковой панели.",
      min: 0,
      max: 100,
      value: 26,
    }).root,
  );

  const entranceHeader = doc.createElement("div");
  entranceHeader.style.cssText = "margin:18px 0 12px;";
  entranceHeader.innerHTML = `
    <div style="font-size:15px;font-weight:700;color:#f2f3f5;">Звуки присоединения</div>
    <div style="font-size:12px;color:#949ba4;margin-top:4px;line-height:1.42;">Выберите звук на звуковой панели, который будет воспроизводиться при присоединении к голосовому каналу. Щёлкните правой кнопкой мыши по голосовому каналу, чтобы зайти на него без звука. <span style="color:#00a8fc;">Подробнее</span></div>
  `;
  secSoundboard.append(entranceHeader);

  const serverOpts = [
    { id: "all", label: "Все серверы" },
    ...guilds.slice(0, 15).map((/** @type {any} */ g) => ({
      id: String(g?.id || "1"),
      label: String(g?.name || "Сервер"),
    })),
  ];
  const sbServerDropdown = createDiscordDropdown({
    doc,
    label: "Выберите сервер",
    options: serverOpts,
    selectedId: "all",
    onSelect: () => {},
  });
  secSoundboard.append(sbServerDropdown.root);

  const pickSoundWrap = doc.createElement("div");
  pickSoundWrap.style.cssText = "margin-top:14px;display:flex;flex-direction:column;gap:6px;";
  const pickSoundLbl = doc.createElement("div");
  pickSoundLbl.style.cssText = "font-size:13px;font-weight:600;color:#f2f3f5;";
  pickSoundLbl.textContent = "Выбрать звук ⟳";
  const pickSoundBtn = doc.createElement("button");
  pickSoundBtn.type = "button";
  pickSoundBtn.style.cssText =
    "width:220px;height:38px;padding:0 12px;border-radius:8px;border:1px solid #383a40;background:#1e1f22;color:#b5bac1;font-size:13px;display:flex;align-items:center;justify-content:space-between;cursor:pointer;";
  const soundChoices = ["Пусто 🔊", "🦆 Кря", "📯 Воздушный горн", "🔔 Колокольчик"];
  let soundChoiceIdx = 0;
  pickSoundBtn.innerHTML = `<span>${soundChoices[0]}</span><span>✏️</span>`;
  pickSoundBtn.addEventListener("click", () => {
    soundChoiceIdx = (soundChoiceIdx + 1) % soundChoices.length;
    pickSoundBtn.innerHTML = `<span>${soundChoices[soundChoiceIdx]}</span><span>✏️</span>`;
    if (soundChoiceIdx > 0) {
      playSettingsPreviewSound(`sb_${soundChoiceIdx}`);
    }
  });
  pickSoundWrap.append(pickSoundLbl, pickSoundBtn);
  secSoundboard.append(pickSoundWrap);

  const divAdv = doc.createElement("hr");
  divAdv.className = "live-sec-divider";
  secSoundboard.append(divAdv);

  // =========================================================================
  // Секция 6: «Расширенные» (`#live-sec-advanced`, f_01_34.jpg)
  // =========================================================================
  const secAdvanced = doc.createElement("div");
  secAdvanced.id = "live-sec-advanced";
  secAdvanced.innerHTML = `<div class="live-sec-heading">Расширенные</div>`;

  secAdvanced.append(
    createDiscordSwitchRow({
      doc,
      title: "OpenH264 Video Codec provided by Cisco Systems, Inc.",
      desc: "Если отключить, OpenH264 больше не будет использоваться для видеозвонков и воспроизведения медиаконтента. Это может привести к тому, что некоторые файлы будет невозможно воспроизвести.",
      checked: true,
      backingInput: findToggle("advanced.openh264"),
    }).root,
  );

  const diagBtn = doc.createElement("button");
  diagBtn.type = "button";
  diagBtn.className = "live-accordion-header";
  const diagLeft = doc.createElement("div");
  diagLeft.innerHTML = `
    <div style="font-size:14px;font-weight:600;color:#f2f3f5;">Показывать настройки диагностики</div>
    <div style="font-size:12px;color:#949ba4;margin-top:2px;">Настройки, которые позволяют службе поддержки Discord собирать техническую информацию для анализа, например данные журнала отладки и аудиозаписи</div>
  `;
  const diagChev = doc.createElement("span");
  diagChev.className = "live-accordion-chevron";
  diagChev.textContent = "⌄";
  diagBtn.append(diagLeft, diagChev);

  const diagContent = doc.createElement("div");
  diagContent.setAttribute("hidden", "");
  diagContent.append(
    createDiscordSwitchRow({
      doc,
      title: "Оверлей стрима",
      checked: false,
      backingInput: findToggle("advanced.stream_overlay"),
    }).root,
    createDiscordSwitchRow({
      doc,
      title: "Запись звука для диагностики",
      checked: false,
      backingInput: findToggle("advanced.diagnostic_recording"),
    }).root,
    createDiscordSwitchRow({
      doc,
      title: "Журнал отладки для голоса и видео",
      checked: true,
      backingInput: findToggle("advanced.debug_log"),
    }).root,
  );
  diagBtn.addEventListener("click", () => {
    const isHid = diagContent.hasAttribute("hidden");
    if (isHid) diagContent.removeAttribute("hidden");
    else diagContent.setAttribute("hidden", "");
    diagChev.textContent = isHid ? "⌃" : "⌄";
  });
  secAdvanced.append(diagBtn, diagContent);

  const resetRow = doc.createElement("div");
  resetRow.style.cssText =
    "display:flex;align-items:center;justify-content:space-between;gap:16px;padding:14px 0;";
  resetRow.innerHTML = `
    <div>
      <div style="font-size:14px;font-weight:600;color:#f2f3f5;">Сбросить все настройки голоса и видео</div>
      <div style="font-size:12px;color:#949ba4;margin-top:2px;">Возвращает настройки голоса и видео по умолчанию.</div>
    </div>
  `;
  const resetBtn = doc.createElement("button");
  resetBtn.type = "button";
  resetBtn.style.cssText =
    "padding:8px 16px;border:1px solid #383a40;border-radius:8px;background:#232428;color:#f23f43;font-size:13px;font-weight:600;cursor:pointer;";
  resetBtn.textContent = "Сброс";
  resetBtn.addEventListener("click", () => {
    micVolSlider.setValue(100);
    speakerVolSlider.setValue(100);
    resetBtn.textContent = "✓ Сброшено";
    setTimeout(() => {
      resetBtn.textContent = "Сброс";
    }, 900);
  });
  resetRow.append(resetBtn);
  secAdvanced.append(resetRow);

  const flushDnsRow = doc.createElement("div");
  flushDnsRow.style.cssText =
    "display:flex;align-items:center;justify-content:space-between;gap:16px;padding:14px 0;border-top:1px solid #383a40;";
  flushDnsRow.innerHTML = `
    <div>
      <div style="font-size:14px;font-weight:600;color:#f2f3f5;">Сбросить DNS и сетевой кэш</div>
      <div style="font-size:12px;color:#949ba4;margin-top:2px;">Очищает системный кэш DNS, сбрасывает блокировки подключения к серверам и переподписывает шлюз.</div>
    </div>
  `;
  const flushDnsBtn = doc.createElement("button");
  flushDnsBtn.type = "button";
  flushDnsBtn.id = "live-settings-flush-dns-btn";
  flushDnsBtn.setAttribute("data-action", "flush-dns-and-network");
  flushDnsBtn.style.cssText =
    "padding:8px 16px;border:1px solid #383a40;border-radius:8px;background:#232428;color:#f2f3f5;font-size:13px;font-weight:600;cursor:pointer;white-space:nowrap;";
  flushDnsBtn.textContent = "Сбросить DNS и сетевой кэш";
  flushDnsBtn.addEventListener("click", () => {
    if (invoke) {
      void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "flush_dns_and_network",
        deviceId: "all",
      }).catch(() => {});
    }
    flushDnsBtn.textContent = "✓ DNS и сеть сброшены";
    setTimeout(() => {
      flushDnsBtn.textContent = "Сбросить DNS и сетевой кэш";
    }, 1200);
  });
  flushDnsRow.append(flushDnsBtn);
  secAdvanced.append(flushDnsRow);

  voicePane.append(secVoice, secCamera, secStreaming, secSounds, secSoundboard, secAdvanced);

  // =========================================================================
  // 2. Построение живой вкладки «Уведомления» (Обзор, Звуки, Значки, Электронная почта, Расширенные)
  // =========================================================================
  notificationsPane.replaceChildren();

  const notifOverview = doc.createElement("div");
  notifOverview.id = "live-notif-overview";
  notifOverview.innerHTML = `<div class="live-sec-heading">Обзор</div>`;
  notifOverview.append(
    createDiscordSwitchRow({
      doc,
      title: "Включить уведомления на рабочем столе",
      desc: "Ищете настройки уведомлений канала или сервера? Нажмите ПКМ на значке нужного сервера и выберите «Параметры уведомлений».",
      checked: true,
    }).root,
  );
  const subHead = doc.createElement("div");
  subHead.style.cssText = "font-size:13px;font-weight:700;color:#f2f3f5;margin:12px 0 4px;";
  subHead.textContent = "Снова получать уведомления, когда...";
  notifOverview.append(
    subHead,
    createDiscordSwitchRow({ doc, title: "Мои знакомые начинают стримы на небольших серверах", checked: true }).root,
    createDiscordSwitchRow({ doc, title: "Мы отмечаем годовщину дружбы", checked: true }).root,
    createDiscordSwitchRow({ doc, title: "Друзья появились в сети", checked: true }).root,
    createDiscordSwitchRow({ doc, title: "На сервере запланировано событие", checked: true }).root,
    createDiscordSwitchRow({ doc, title: "Друзья обновляют профили", checked: true }).root,
  );

  const reactRow = doc.createElement("div");
  reactRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:16px;padding:10px 0;";
  const reactLabel = doc.createElement("span");
  reactLabel.style.cssText = "font-size:14px;font-weight:600;color:#f2f3f5;";
  reactLabel.textContent = "Кто-то реагирует на мои сообщения";
  const reactRight = doc.createElement("div");
  reactRight.style.cssText = "width:230px;flex-shrink:0;";
  reactRight.append(
    createDiscordDropdown({
      doc,
      options: [
        { id: "all", label: "Все сообщения" },
        { id: "dm", label: "Только личные сообщения" },
        { id: "none", label: "Не получать" },
      ],
      selectedId: "all",
      onSelect: () => {},
    }).root,
  );
  reactRow.append(reactLabel, reactRight);
  notifOverview.append(reactRow);
  const nDiv1 = doc.createElement("hr");
  nDiv1.className = "live-sec-divider";
  notifOverview.append(nDiv1);

  const notifSounds = doc.createElement("div");
  notifSounds.id = "live-notif-sounds";
  notifSounds.innerHTML = `<div class="live-sec-heading">Звуки</div>`;
  notifSounds.append(
    createDiscordSwitchRow({ doc, title: "Активировать набор звуков 🎃 Хэллоуин", checked: false }).root,
    createDiscordSwitchRow({ doc, title: "Новое сообщение", checked: true, soundId: "notif.message" }).root,
    createDiscordSwitchRow({
      doc,
      title: "Новое сообщение в канале, который я сейчас читаю",
      checked: false,
      soundId: "notif.current_channel",
    }).root,
    createDiscordSwitchRow({ doc, title: "Входящий звонок", checked: true, soundId: "notif.call_ring" }).root,
    createDiscordSwitchRow({
      doc,
      title: "Отключить звуки всех уведомлений",
      desc: "Отключает все звуки уведомлений, а также звуковые эффекты голоса и видео. Включите, чтобы восстановить сохранённые предпочтения.",
      checked: false,
    }).root,
  );

  const notifRelLabel = doc.createElement("div");
  notifRelLabel.style.cssText = "font-size:13px;font-weight:600;color:#b5bac1;margin:14px 0 8px;";
  notifRelLabel.textContent = "Связанные настройки";
  const notifRelCard = doc.createElement("button");
  notifRelCard.type = "button";
  notifRelCard.style.cssText =
    "display:flex;align-items:center;justify-content:space-between;gap:14px;width:100%;padding:14px 16px;border-radius:10px;border:1px solid #383a40;background:#232428;color:#f2f3f5;cursor:pointer;text-align:left;";
  notifRelCard.innerHTML = `
    <span style="display:flex;align-items:center;gap:12px;">
      <span style="width:36px;height:36px;border-radius:8px;background:#2b2d31;display:inline-flex;align-items:center;justify-content:center;font-size:16px;">🎙</span>
      <span>
        <div style="font-size:14px;font-weight:600;color:#f2f3f5;">Голос и видео</div>
        <div style="font-size:12px;color:#949ba4;margin-top:2px;">Включение/отключение звуков во время звонка: «Заглушить», «Включить звук», «Отключить звук» и другие.</div>
      </span>
    </span>
    <span style="font-size:18px;color:#b5bac1;">›</span>
  `;
  notifRelCard.addEventListener("click", () => {
    showPane("Голос и видео");
    doc.getElementById("live-sec-sounds")?.scrollIntoView({ behavior: "smooth", block: "start" });
  });
  notifSounds.append(notifRelLabel, notifRelCard);
  const nDiv2 = doc.createElement("hr");
  nDiv2.className = "live-sec-divider";
  notifSounds.append(nDiv2);

  const notifBadges = doc.createElement("div");
  notifBadges.id = "live-notif-badges";
  notifBadges.innerHTML = `<div class="live-sec-heading">Значки</div>`;
  notifBadges.append(
    createDiscordSwitchRow({
      doc,
      title: "Включить индикатор непрочитанных сообщений",
      desc: "Показывать красный значок рядом с иконкой приложения, когда есть непрочитанные сообщения.",
      checked: true,
    }).root,
  );
  const nDiv3 = doc.createElement("hr");
  nDiv3.className = "live-sec-divider";
  notifBadges.append(nDiv3);

  const notifEmail = doc.createElement("div");
  notifEmail.id = "live-notif-email";
  notifEmail.innerHTML = `<div class="live-sec-heading">Электронная почта</div>`;
  const emailToggles = [
    createDiscordSwitchRow({
      doc,
      title: "Информационные электронные письма",
      desc: "Получайте электронные письма о пропущенных звонках и сообщениях, а также дайджесты сообщений.",
      checked: false,
    }),
    createDiscordSwitchRow({
      doc,
      title: "Социальные электронные письма",
      desc: "Получайте электронные письма с предложениями дружбы и уведомлениями о событиях на вашем сервере.",
      checked: false,
    }),
    createDiscordSwitchRow({
      doc,
      title: "Электронные письма с объявлениями и описаниями обновлений",
      desc: "Получайте электронные письма об обновлениях системы, наших новейших функциях, улучшениях и исправлениях ошибок.",
      checked: false,
    }),
    createDiscordSwitchRow({
      doc,
      title: "Электронные письма с советами",
      desc: "Получайте электронные письма с полезными советами по использованию Discord и информацией о менее известных функциях.",
      checked: false,
    }),
    createDiscordSwitchRow({
      doc,
      title: "Электронные письма с рекомендациями",
      desc: "Получайте электронные письма с рекомендованными серверами и предлагаемыми событиями.",
      checked: false,
    }),
  ];
  for (const et of emailToggles) notifEmail.append(et.root);

  const unsubRow = doc.createElement("div");
  unsubRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:16px;padding:12px 0;";
  unsubRow.innerHTML = `
    <div>
      <div style="font-size:14px;font-weight:600;color:#f2f3f5;">Отписаться от всех рекламных рассылок</div>
      <div style="font-size:12px;color:#949ba4;margin-top:2px;line-height:1.4;">Сюда входят любые электронные письма об обновлениях системы, новых функциях, советы по использованию Discord и рекомендации по новым серверам, событиям или каналам трибун.</div>
    </div>
  `;
  const unsubBtn = doc.createElement("button");
  unsubBtn.type = "button";
  unsubBtn.style.cssText =
    "padding:8px 14px;border:1px solid #383a40;border-radius:8px;background:#232428;color:#b5bac1;font-size:12px;font-weight:600;cursor:pointer;flex-shrink:0;";
  unsubBtn.textContent = "Отписаться";
  unsubBtn.addEventListener("click", () => {
    for (const et of emailToggles) et.setChecked(false);
    unsubBtn.textContent = "✓ Вы отписаны";
  });
  unsubRow.append(unsubBtn);
  notifEmail.append(unsubRow);
  const nDiv4 = doc.createElement("hr");
  nDiv4.className = "live-sec-divider";
  notifEmail.append(nDiv4);

  const notifAdv = doc.createElement("div");
  notifAdv.id = "live-notif-advanced";
  notifAdv.innerHTML = `<div class="live-sec-heading">Расширенные</div>`;
  const notifAdvBtn = doc.createElement("button");
  notifAdvBtn.type = "button";
  notifAdvBtn.className = "live-accordion-header";
  const notifAdvLeft = doc.createElement("div");
  notifAdvLeft.innerHTML = `
    <div style="font-size:14px;font-weight:600;color:#f2f3f5;">Показать расширенные параметры уведомлений</div>
    <div style="font-size:12px;color:#949ba4;margin-top:2px;">Задержка мобильных уведомлений, разрешить воспроизведение и использование команды /tts и многое другое</div>
  `;
  const notifAdvChev = doc.createElement("span");
  notifAdvChev.className = "live-accordion-chevron";
  notifAdvChev.textContent = "⌄";
  notifAdvBtn.append(notifAdvLeft, notifAdvChev);
  const notifAdvBody = doc.createElement("div");
  notifAdvBody.setAttribute("hidden", "");
  notifAdvBody.append(
    createDiscordSwitchRow({
      doc,
      title: "Разрешить воспроизведение и использование команды /tts",
      checked: true,
    }).root,
  );
  notifAdvBtn.addEventListener("click", () => {
    const isHid = notifAdvBody.hasAttribute("hidden");
    if (isHid) notifAdvBody.removeAttribute("hidden");
    else notifAdvBody.setAttribute("hidden", "");
    notifAdvChev.textContent = isHid ? "⌃" : "⌄";
  });
  notifAdv.append(notifAdvBtn, notifAdvBody);

  notificationsPane.append(notifOverview, notifSounds, notifBadges, notifEmail, notifAdv);

  // =========================================================================
  // 3. Левая панель навигации `#settings-nav`: Поиск + Подразделы «Голос и видео» и «Уведомления»
  // =========================================================================
  if (nav) {
    // Поле поиска «🔍 Поиск» сразу под профилем (f_00_05.jpg)
    const profileNode = nav.querySelector(".settings-profile");
    if (profileNode && !doc.getElementById("live-settings-search-input")) {
      const searchBox = doc.createElement("div");
      searchBox.style.cssText = "padding:6px 4px 10px;";
      const searchInput = doc.createElement("input");
      searchInput.type = "search";
      searchInput.id = "live-settings-search-input";
      searchInput.placeholder = "🔍 Поиск";
      searchInput.style.cssText =
        "width:100%;height:32px;padding:0 10px;border-radius:6px;border:1px solid #2b2d31;background:#111214;color:#f2f3f5;font-size:13px;outline:none;";
      searchInput.addEventListener("input", () => {
        const q = searchInput.value.trim().toLowerCase();
        for (const item of nav.querySelectorAll(".settings-nav-item")) {
          const el = /** @type {HTMLElement} */ (item);
          const txt = (el.textContent ?? "").toLowerCase();
          el.style.display = !q || txt.includes(q) ? "" : "none";
        }
      });
      searchBox.append(searchInput);
      profileNode.after(searchBox);
    }

    // Скрываем статические 4 подпункта «Учётной записи» в живом режиме (в оригинале они не раскрыты в боковой панели)
    const accountSubnav = doc.getElementById("settings-subnav-account");
    if (accountSubnav) accountSubnav.style.display = "none";

    /**
     * Создаёт дерево подпунктов под разделом (`Голос и видео` или `Уведомления`)
     * @param {string} sectionName
     * @param {Array<{ label: string, targetId: string }>} anchors
     */
    const attachSubnavTree = (sectionName, anchors) => {
      const parentBtn = /** @type {HTMLElement | null} */ (
        nav.querySelector(`.settings-nav-item[data-section="${sectionName}"]`)
      );
      if (!parentBtn) return null;
      const tree = doc.createElement("div");
      tree.className = "live-settings-subnav";
      tree.setAttribute("data-parent-section", sectionName);
      /** @type {Array<{ btn: HTMLElement, targetId: string }>} */
      const items = [];
      anchors.forEach((anc, idx) => {
        const subBtn = doc.createElement("button");
        subBtn.type = "button";
        subBtn.className = `live-settings-subnav-btn${idx === 0 ? " is-active" : ""}`;
        subBtn.textContent = anc.label;
        subBtn.addEventListener("click", (ev) => {
          ev.stopPropagation();
          showPane(sectionName);
          for (const it of items) it.btn.classList.toggle("is-active", it.btn === subBtn);
          const target = doc.getElementById(anc.targetId);
          target?.scrollIntoView({ behavior: "smooth", block: "start" });
        });
        items.push({ btn: subBtn, targetId: anc.targetId });
        tree.append(subBtn);
      });
      parentBtn.after(tree);
      return { tree, items };
    };

    const voiceTree = attachSubnavTree("Голос и видео", [
      { label: "Голос", targetId: "live-sec-voice" },
      { label: "Камера", targetId: "live-sec-camera" },
      { label: "Стриминг", targetId: "live-sec-streaming" },
      { label: "Звуки", targetId: "live-sec-sounds" },
      { label: "Звуковая панель", targetId: "live-sec-soundboard" },
      { label: "Расширенные", targetId: "live-sec-advanced" },
    ]);

    const notifTree = attachSubnavTree("Уведомления", [
      { label: "Обзор", targetId: "live-notif-overview" },
      { label: "Звуки", targetId: "live-notif-sounds" },
      { label: "Значки", targetId: "live-notif-badges" },
      { label: "Электронная почта", targetId: "live-notif-email" },
      { label: "Расширенные", targetId: "live-notif-advanced" },
    ]);

    const syncSubnavVisibility = () => {
      const activePane = root.getAttribute("data-active-pane") || "voice";
      if (voiceTree) {
        if (activePane === "voice") {
          voiceTree.tree.removeAttribute("hidden");
          void syncAudioDevicesFromBackend();
        } else {
          voiceTree.tree.setAttribute("hidden", "");
          stopSettingsMicTest();
        }
      }
      if (notifTree) {
        if (activePane === "notifications") notifTree.tree.removeAttribute("hidden");
        else notifTree.tree.setAttribute("hidden", "");
      }
      if (headerTitleNode) {
        const activeNav = nav.querySelector('.settings-nav-item[aria-current="page"]');
        headerTitleNode.textContent =
          activeNav?.textContent?.trim() ||
          (activePane === "voice"
            ? "Голос и видео"
            : activePane === "notifications"
              ? "Уведомления"
              : "Настройки");
      }
    };

    for (const btn of nav.querySelectorAll(".settings-nav-item")) {
      btn.addEventListener("click", () => {
        setTimeout(syncSubnavVisibility, 0);
      });
    }
    const obs = new MutationObserver(() => syncSubnavVisibility());
    obs.observe(root, { attributes: true, attributeFilter: ["data-active-pane"] });
    syncSubnavVisibility();

    // Подсветка активного подпункта при прокрутке правой панели `#settings`
    root.addEventListener("scroll", () => {
      const activePane = root.getAttribute("data-active-pane");
      const currentTree = activePane === "voice" ? voiceTree : activePane === "notifications" ? notifTree : null;
      if (!currentTree) return;
      const rootRect = root.getBoundingClientRect();
      let activeIdx = 0;
      currentTree.items.forEach((it, idx) => {
        const node = doc.getElementById(it.targetId);
        if (!node) return;
        const rect = node.getBoundingClientRect();
        if (rect.top - rootRect.top <= 120) {
          activeIdx = idx;
        }
      });
      currentTree.items.forEach((it, idx) => {
        it.btn.classList.toggle("is-active", idx === activeIdx);
      });
    });
  }
}
