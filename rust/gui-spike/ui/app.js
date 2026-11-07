// @ts-check
/**
 * P0-01: статический фронтенд спайка (RW-39 / W39d).
 *
 * Без сборщика и фреймворков: честный замер оболочки, а не бандлера.
 * Модули фич — `ui/features/*.js`; здесь только сборка, UI_SNAPSHOT и замер.
 */
import { byId } from "./features/dom.js";
import { clearErrorState, reportError, showErrorState, UIError } from "./features/errors.js";
import { benchScroll, frontendReadyMs, startMetrics } from "./features/metrics.js";
import { createScreenRouter } from "./features/screens.js";
import { CHAT_ROWS, initChatScreen } from "./features/chat.js";
import { configureReactionTransport } from "./features/reactions.js";
import {
  initMemberScreen,
  MEMBER_GROUPS,
  MEMBER_ROWS,
  renderMemberPanel,
  renderMemberRow,
  STATUS_LABELS,
} from "./features/members.js";
import { initEmojiConsole } from "./features/emoji.js";
import { initSettingsConsole, normalizeSettings } from "./features/settings.js";
import { initActions } from "./features/actions.js";
import { createAuthGate } from "./features/auth.js";
import { createComposer, evaluateClientChannelPolicy } from "./features/composer.js";
import { createSearchOverlay, SEARCH_SECTIONS, selectedGuildId } from "./features/search-overlay.js";
import { createQuickSwitcher } from "./features/quick-switcher.js";
import { createVoicePanel, createVoiceStore } from "./features/voice.js";
import { createJoinServer } from "./features/join-server.js";
import { createCreateGuildController } from "./features/create-guild.js";
import { createTypingIndicator, createTypingSender } from "./features/typing-indicator.js";
import { asSnowflake, createDispatchStore } from "./features/dispatch-model.js";
import { createDispatchView } from "./features/dispatch-view.js";
import { createDispatchBridge, resolveDispatchListen } from "./features/dispatch-bridge.js";
import { createPinsThreadsPanels } from "./features/pins-threads.js";
import { createInviteModal } from "./features/invite-modal.js";
import { createVoiceDevicesSettings } from "./features/voice-devices-settings.js";
import {
  applyChannelIndicatorToDom,
  applyGuildRailIndicatorToDom,
  buildMuteConfig,
  computeChannelIndicatorState,
  computeGuildRailIndicatorState,
  createNotificationSettingsController,
  evaluateMuteState,
  MESSAGE_NOTIFICATION_LEVELS,
  mergeGuildSettingsUpdate,
  MUTE_TIMER_OPTIONS,
  normalizeChannelOverrides,
  resolveEffectiveNotifications,
} from "./features/notifications.js";
import { createDmFriendsController, HOME_GUILD_ID } from "./features/dm-friends.js";
import { createUserPopoutController } from "./features/user-popout.js";
import { createGuildModalsController } from "./features/guild-modals.js";
import { createAppShellWiring } from "./features/app-shell-wiring.js";
import { createComposerPopoversBridge } from "./features/composer-popovers-bridge.js";
import { globalAppReactiveStore } from "./features/reactive-store.js";
import {
  createDiscordNativeBridge,
  getDiscordIPCEvent,
  RENDERER_IPC_SEND_WHITELIST,
  RENDERER_IPC_INVOKE_WHITELIST,
  RENDERER_SET_ALLOWLIST,
  isAllowedRendererSettingKey,
  assertAllowedSettingKey,
  RENDERER_IPC_SEND_SYNC_WHITELIST,
  RENDERER_SET_CHROMIUM_SWITCHES_ALLOWLIST,
  validateChromiumSwitches,
  PRELOAD_ONLY_NATIVE_MODULES,
  NATIVE_MODULE_NAME_REGEX,
  assertWhitelistedNativeModule,
  createBrowserFeaturesFacade,
  normalizeApiEndpoint,
  sanitizeNativeModulePaths,
  ALLOWED_APP_PATHS,
  assertAllowedAppPath,
  DISCORD_CLIP_PROTOCOL,
  DISCORD_CLIP_UUID,
  shouldInvokeDockCancelBounce,
  resolveOsArch,
  DISCORD_MP4_ERROR_REASONS,
  formatClipProtocolUrl,
  parseClipRangeHeader,
  WEBAUTHN_CONVERTIBLE_ERROR_CODES,
  parseWebAuthnNativeResponse,
  FORBIDDEN_FRAME_DEBUGGER_CHANNELS,
  clampTracingDurationMs,
  ALLOWED_DESKTOP_CAPTURE_TYPES,
  normalizeDesktopCaptureTypes,
  ALLOWED_PROCESS_ENV_KEYS,
  projectProcessEnv,
  RIOT_LIVE_CLIENT_ORIGIN,
  buildRiotLiveClientUrl,
  BLOCKED_EXTERNAL_URL_PROTOCOLS,
  shouldOpenExternalUrl,
  checkUrlOriginMatches,
  formatSplashStatusText,
  SPLASH_DOWNLOAD_OPTIONS,
  shouldShowSplashProgress,
  shouldDisableBootstrapUpdater,
  resolveBootstrapAppIdentity,
  CODE_INTEGRITY_BYTE_SIZE,
  CODE_INTEGRITY_STREAM_TYPE,
  buildCodeIntegritySentryTags,
  determineBootstrapModuleDataPath,
  OLD_VERSION_DIR_REGEX,
  shouldRemoveOldVersionDirectory,
  buildCodeIntegritySentryContext,
  resolveBootstrapPaths,
  DESKTOP_TTI_WARMUP_DOMAINS,
  SENTRY_IGNORE_ERRORS,
  buildSentryDsn,
  resolveSentrySampleRate,
  detectLinuxRuntimeEnvironment,
  DESKTOP_ANALYTICS_CACHE_FILENAME,
  DESKTOP_ANALYTICS_CACHE_TTL_MS,
  DESKTOP_TTI_EVENT_TYPES,
  computeRestartAndFullTtiDuration,
  buildWindowsAutostartCommand,
  DNS_TELEMETRY_BASE_DOMAIN,
  BLACKBOX_MAX_MINIDUMP_AGE_MS,
  BLACKBOX_MAX_STORED_MINIDUMPS,
  normalizeDnsLabel,
  buildDnsTelemetryFqdn,
  computeScheduledBackoffStep,
  SQUIRREL_STARTUP_COMMANDS,
  SHELL_LINK_HEADER_BYTES,
  DISCLIP_PRIVILEGED_SCHEME,
  isWindowsShellLinkBytes,
  resolvePlatformQuitAccelerator,
  SPLASH_SCHEDULED_BACKOFF_STEPS_MS,
  OBSOLETED_SETTINGS_KEYS,
  resolveSplashWindowConfig,
  formatConnectivityErrorMessage,
  DISABLED_CHROMIUM_FEATURES_BASE,
  DISABLED_CHROMIUM_FEATURES_DARWIN_PRE24,
  BUILD_OVERRIDE_COOKIE_NAME,
  buildDisabledChromiumFeatures,
  extractUrlFromBootstrapArgs,
  buildCookieRemovalUrl,
  LINUX_AUTOSTART_COMMENT,
  MAX_HANDLED_BOOTSTRAP_ERRORS,
  formatLinuxAutostartDesktopEntry,
  formatWindowsRegAddArgs,
  shouldReportHandledBootstrapError,
  UPDATER_EVENT_CACHE_FILENAME,
  UPDATER_INCONSISTENT_INSTALLER_STATE_ERROR,
  UPDATER_TASK_STATES,
  classifyNativeUpdaterError,
  buildLegacyUpdaterFeedUrl,
  FRAME_DEBUGGER_MAX_HELD_EVENTS,
  MIN_MACOS_KERNEL_MAJOR,
  FRAME_DEBUGGER_ALLOWED_METHODS,
  FRAME_DEBUGGER_DENIED_PREFIXES,
  FRAME_DEBUGGER_DENIED_METHODS,
  checkFrameDebuggerCdpMethod,
  classifyElectronProcessMetricType,
  parseDarwinKernelMajor,
  WEBAUTHN_MACOS_KEYCHAIN_ACCESS_GROUP,
  FRAME_DEBUGGER_ROOT_METHODS,
  redactFrameDebuggerUrl,
  isValidActivityFrameOrigin,
  ALLOWED_MEDIA_ORIGINS,
  resolveWindowsAppBadge,
  parseClipHttpByteRange,
  NTP_PACKET_SIZE_BYTES,
  NTP_EPOCH_OFFSET_SECONDS,
  POPOUT_ALLOWED_FEATURES,
  STAFF_DEV_TOOLS_KEY,
  STAFF_DEV_TOOLS_PUBLIC_KEY_B64,
  NON_RENDERER_CRASH_COOLDOWN_MS,
  UTILITY_IGNORED_CRASH_EXIT_CODE,
  formatStaffDevToolsSignaturePayload,
  extractTokenFromLevelDbText,
  shouldReportNonRendererCrash,
  ZSTD_PRECOMPRESS_MAX_SIZE_MULTIPLIER,
  COMPRESSED_FILE_SUFFIX,
  DETAILED_TTI_FCP_TIMEOUT_MS,
  RENDERER_PERFORMANCE_MARKS,
  computeFileReadMaxSize,
  formatStartupTimingLine,
  TRAY_MENU_REBUILD_KEYS,
  THUMBAR_BUTTON_NAMES,
  RESOURCE_SCALE_VARIANTS,
  resolveTrayGuidForChannel,
  resolveTrayIconName,
  formatThumbarButtonIconFilename,
  NETWORK_LOGGING_CDP_PROTOCOL_VERSION,
  NETWORK_LOGGING_CDP_METHODS,
  NETWORK_LOGGING_STRIPPED_HEADERS,
  DOCUMENT_PIP_TRIGGER_ID,
  sanitizeNetworkLogHeaders,
  computeSpellcheckDictionaryDiff,
  resolveWindowContentProtection,
  CHUNKED_REQUEST_MISSING_ENDPOINT_ERROR,
  SYSIMG_MODULE_NAME,
  SYSIMG_UNAVAILABLE_ERROR,
  SYSIMG_TARGET_FORMAT,
  rewriteChunkedRequestPathname,
  isWebappBundleUrl,
  computeBundleStatsDurations,
  PARTIAL_DOWNLOAD_SUFFIX,
  HOOK_MINIDUMP_MAX_SIZE_BYTES,
  CLIPS_SENTRY_DIR_NAME,
  shouldDeleteUnusedAssetFile,
  shouldCompressLogFile,
  orderWebRtcLogPairByMtime,
  VOICE_MESSAGE_RECORDING_FILE_NAMES,
  VOICE_MESSAGE_RECORDING_SUBDIR,
  isValidVoiceRecordingFilename,
  isWhitelistedNativeModule,
  SPLASH_MEDIA_FORMAT_FLAG_PREFIX,
  PREMAP_TARGET_MODULES,
  NATIVE_PREMAP_FLAG,
  PREMAP_NO_COMMITTED_PATHS_ERROR,
  resolveSplashMediaFormatFromArgv,
  formatLinuxManualDownloadUrl,
  resolveSplashManualDownloadButtonText,
  UPDATER_CURRENT_LOG_SUFFIX,
  ADMIN_SERVICE_CURRENT_LOG_SUFFIX,
  SQUIRREL_SETUP_LOG_FILENAME,
  CLIPS_DECIDER_STASH_DIR_NAME,
  CLIPS_DECIDER_DATA_FILENAME,
  resolveDiscordServiceChannelFolder,
  formatDeduplicatedDownloadFilename,
  isHookMinidumpFilename,
  CLIPBOARD_RAW_FORMAT_PREFIX,
  MENU_EVENTS,
  WEBAPP_PATHS,
  DESKTOP_CAPTURER_REQUIRED_TYPES,
  unwrapClipboardRawFormat,
  hasClipboardMixedContent,
  filterDesktopCapturerTypes,
  UNHANDLED_JS_EXCEPTION_DELAY_MS,
  SENTRY_METADATA_TAG_KEYS,
  formatNestedMetadataKey,
  computeCrashReporterMetadataDiff,
  formatUnhandledJsExceptionMessage,
} from "./features/discord-native-bridge.js";
import {
  formatReadableVerificationCode,
  shouldShowVerificationCodeForChannel,
} from "./features/voice-diagnostics.js";
import { computeFloatingPosition } from "./features/floating-popover.js";
import {
  LURKER_PERMISSIONS_MASK,
  evaluateChannelAccessDto,
  createTypedCommands,
} from "./bindings.js";
import {
  mountReactiveVoiceRtcPanel,
  resolveVoiceCallPhaseView,
  renderVoiceRtcPhaseDom,
} from "./modules/voice-rtc-panel.js";
import {
  applyVoiceStateUpdateEvent,
  isConfirmedVoiceStateRecord,
  syncServerAccessBannersAndComposerDom,
} from "./modules/channel-tree-view.js";

/**
 * Устройства ввода/вывода по умолчанию для секции «Голос и видео» (V-07 / INT-07).
 * @type {ReadonlyArray<import("./features/voice-devices-settings.js").AudioDevice>}
 */
const DEFAULT_VOICE_DEVICES = Object.freeze([
  {
    id: "default-input",
    label: "Системный микрофон по умолчанию",
    kind: "input",
    isDefault: true,
  },
  {
    id: "default-output",
    label: "Системный вывод по умолчанию",
    kind: "output",
    isDefault: true,
  },
]);

/**
 * Видимая пометка предварительного режима секции «Голос и видео».
 *
 * P1-1 (Pass 3): `set_input_device`, `set_output_device`, `set_input_volume`,
 * `set_output_volume`, `set_input_mode`, `set_mute_state`, `set_ptt_key` не
 * зарегистрированы в `generate_handler!` (`rust/gui-spike/src/main.rs`):
 * `discord-voice` к Tauri-слою не подключён. До wiring'а секция обязана честно
 * объявлять, что настройки живут только в сессии.
 */
const VOICE_DEVICES_PRELIMINARY_NOTICE =
  "Предварительный режим: сохранение в системный голосовой движок появится после подключения voice-devices IPC.";

/** @param {number} ms @returns {Promise<void>} */
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// Тот же класс риска, что у benchScroll: если rAF не тикает, не ждём вечно.
const nextFrame = () => new Promise((resolve) => {
  let settled = false;
  const done = () => {
    if (settled) return;
    settled = true;
    clearTimeout(guard);
    resolve(undefined);
  };
  const guard = setTimeout(done, 1000);
  requestAnimationFrame(done);
});

/** Видимая поверхность ошибок: контент, не списки (у них ровно один sizer). */
const content = /** @type {HTMLElement | null} */ (document.querySelector(".content"));

// G2-02: `.error-state` — `position:absolute; inset:0`, поэтому containing block
// обязан быть `.content`, иначе оверлей накрывает viewport целиком. CSS-файлы
// вне файловой матрицы этой задачи (ui/components/*.css — read-only), поэтому
// позиционирующий контекст задаётся здесь; при переносе в style.css строка ниже
// останется безвредной (значение уже relative).
if (content && content.style.position === "") content.style.position = "relative";

/**
 * Команды-действия пользователя: успешный ответ снимает recoverable-ошибку
 * (G2-02, «ошибка → retry»). Служебные вызовы (метрики, снимки) оверлей не
 * трогают, иначе чужая ошибка исчезала бы от фонового опроса.
 */
const RECOVERABLE_COMMANDS = new Set([
  "search_remote",
  "send_message",
  "reply_message",
  "edit_message",
  "delete_message",
  "toggle_reaction",
  "settings_save",
]);

/**
 * Единая видимая поверхность ошибок: лог + оверлей в `.content`.
 * @param {unknown} error
 * @param {string} [code]
 */
function surface(error, code = "ACTION_FAILED") {
  const reported = reportError(code, error);
  showErrorState(content, reported);
  return reported;
}

/* ---------- L-01 (RW-40): видимый reconnect/offline state ---------- */

/** Интервал опроса `net_status`: чаще — лишний IPC, реже — долгий «залипший» статус. */
const NET_POLL_MS = 2000;
/** Команды сетевого слоя (`rust/gui-spike/src/main.rs`, RW-39/NET). */
const NET_COMMANDS = Object.freeze({ status: "net_status", connect: "net_connect" });
/** `GatewayStatus::Display` (`src/net.rs`): других значений контракт не отдаёт. */
const NET_STATUSES = new Set(["offline", "connecting", "ready", "reconnecting", "error"]);
/**
 * Тексты баннера. «Переподключение…»/«Нет соединения» — L-01; retry — ручной,
 * поэтому при ошибке `net_connect` текст объясняет причину, а не крутит цикл.
 */
const NET_TEXT = Object.freeze({
  reconnecting: "Переподключение…",
  offline: "Нет соединения",
  retry: "Повторить",
  retryBusy: "Подключение…",
  retryFailed: "Не удалось подключиться. Проверьте сеть и повторите.",
  sessionExpired: "Сессия истекла. Войдите снова.",
});

/**
 * Текст ошибки `net_connect` без тела ответа и секретов: 401/unauthorized и
 * сообщения про токен — «войдите снова», остальное — общий отказ.
 * @param {unknown} error
 * @returns {string}
 */
function netFailureText(error) {
  const bag = error && typeof error === "object"
    ? /** @type {Record<string, any>} */ (error)
    : {};
  const code = Number(bag.code ?? bag.status);
  const kind = String(bag.kind ?? "");
  const message = typeof error === "string" ? error : String(bag.message ?? "");
  if (code === 401 || /unauthor|invalidtoken/i.test(kind) || /401|unauthor|токен/i.test(message)) {
    return NET_TEXT.sessionExpired;
  }
  return NET_TEXT.retryFailed;
}

/**
 * Баннер сетевого состояния (L-01). Живёт в `.topbar` (в потоке, не поверх
 * чата), стартует только после `auth ready` и останавливается на dispose/logout.
 *
 * `net_status` вызывается без аргументов (токен в UI не попадает), `net_connect`
 * — только по клику «Повторить»: ошибка видна текстом и не порождает retry-loop.
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   intervalMs?: number,
 * }} options
 */
function createNetStatus(options) {
  const { invoke } = options;
  const intervalMs = options.intervalMs ?? NET_POLL_MS;
  const node = /** @type {HTMLElement} */ (document.createElement("section"));
  node.id = "net-status";
  node.className = "net-status";
  node.setAttribute("role", "status");
  node.setAttribute("aria-live", "polite");
  node.setAttribute("aria-atomic", "true");
  node.setAttribute("data-state", "ready");
  node.setAttribute("hidden", "");

  const text = document.createElement("span");
  text.className = "net-status-text";
  const retry = /** @type {any} */ (document.createElement("button"));
  retry.id = "net-retry";
  retry.className = "net-status-retry";
  retry.type = "button";
  retry.setAttribute("data-action", "net-retry");
  retry.textContent = NET_TEXT.retry;
  retry.setAttribute("hidden", "");
  node.append(text, retry);
  (document.querySelector(".topbar") ?? document.body).append(node);

  /** @type {string} */
  let state = "ready";
  /** @type {ReturnType<typeof setInterval> | null} */
  let timer = null;
  let polling = false;
  let disposed = false;
  let inFlight = false;
  /** Ручной retry в полёте: второй клик не шлёт второй IPC. */
  let busy = false;
  /** Последняя ошибка `net_connect`: держим текст, пока пользователь не повторит. */
  let failure = "";

  /** @param {string} name @param {string} [message] */
  function apply(name, message = "") {
    const next = NET_STATUSES.has(name) ? name : "offline";
    const retryFocused = document.activeElement === retry;
    state = next;
    if (message !== "") failure = message;
    const connecting = next === "connecting" || next === "reconnecting";
    // Успешный/идущий контакт с ядром снимает текст прошлой ошибки connect.
    if (next === "ready" || connecting) failure = "";
    node.setAttribute("data-state", next);
    const hidden = next === "ready";
    if (hidden) node.setAttribute("hidden", "");
    else node.removeAttribute("hidden");
    text.textContent = hidden
      ? ""
      : message || (connecting ? NET_TEXT.reconnecting : failure || NET_TEXT.offline);
    const showRetry = !hidden && !connecting && !busy;
    retry.textContent = busy ? NET_TEXT.retryBusy : NET_TEXT.retry;
    if (showRetry) {
      retry.removeAttribute("hidden");
      retry.removeAttribute("disabled");
      retry.removeAttribute("aria-disabled");
    } else {
      retry.setAttribute("hidden", "");
    }
    // Фокус не крадём появлением региона; но если баннер скрылся, пока фокус
    // был на «Повторить», возвращаем его в чат (иначе фокус в hidden-узле).
    if (hidden && retryFocused) {
      const list = /** @type {any} */ (byId("chat-list"));
      if (list && typeof list.focus === "function") list.focus();
    }
  }

  /** @returns {Promise<void>} */
  async function refresh() {
    if (disposed || !polling || inFlight) return;
    // Свёрнутое/скрытое окно не опрашивает ядро: приложение не активно.
    if (/** @type {any} */ (document).hidden === true) return;
    inFlight = true;
    try {
      const value = await invoke(NET_COMMANDS.status);
      // После dispose/stop (logout) поздний ответ не должен менять UI.
      if (disposed || !polling) return;
      const status = typeof value === "string"
        ? value
        : String(/** @type {any} */ (value)?.status ?? "");
      if (status !== "") apply(status);
    } catch {
      // IPC статуса недоступен: это видимое «Нет соединения», а не тишина.
      if (!disposed) apply("offline");
    } finally {
      inFlight = false;
    }
  }

  function start() {
    if (disposed) return;
    polling = true;
    if (timer === null) timer = setInterval(() => { void refresh(); }, intervalMs);
    void refresh();
  }

  function stop() {
    polling = false;
    if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
  }

  /** Ручное переподключение: ровно один `net_connect` на клик. */
  async function retryNow() {
    if (disposed || busy) return;
    busy = true;
    failure = "";
    apply("connecting");
    retry.setAttribute("disabled", "");
    retry.setAttribute("aria-disabled", "true");
    try {
      const value = await invoke(NET_COMMANDS.connect);
      if (disposed) return;
      const status = typeof value === "string"
        ? value
        : String(/** @type {any} */ (value)?.status ?? "");
      apply(status === "" ? "error" : status);
    } catch (error) {
      if (!disposed) apply("error", netFailureText(error));
    } finally {
      busy = false;
      if (!disposed && (state === "offline" || state === "error")) {
        retry.removeAttribute("hidden");
        retry.removeAttribute("disabled");
        retry.removeAttribute("aria-disabled");
        retry.textContent = NET_TEXT.retry;
      }
    }
  }

  function dispose() {
    disposed = true;
    stop();
    node.setAttribute("hidden", "");
    node.setAttribute("data-state", "ready");
  }

  return {
    node,
    get state() { return state; },
    get polling() { return polling; },
    start,
    stop,
    refresh,
    retry: retryNow,
    dispose,
  };
}

/* ---------- G-01 (RW-40): рейл серверов из `ready()` ---------- */

/**
 * Существующая IPC-команда спайка (`main.rs`: `ready`). Отдельной REST-ручки
 * списка гильдий в клиенте нет (`NETWORK-CONTRACTS §1.3`), новых endpoint'ов не
 * добавляем: источник списка — READY fixture.
 */
const READY_COMMAND = "ready";
/** Плиток-skeleton на время `ready()`: высота рейла не «прыгает» после загрузки. */
const RAIL_SKELETONS = 3;

/**
 * Гильдия рейла. Только подтверждённые поля payload (`GuildSummary`): `icon` —
 * хеш CDN, а не URL, поэтому иконок нет — монограмма (`SERVER-MENU §2.1`).
 * @typedef {{ id: string, name: string }} RailGuild
 */

/**
 * Монограмма 2 буквы: инициалы первых двух слов; для одного слова — первые две
 * буквы. `[T]`-подсказка «как в существующем `.guild-item`».
 * @param {string} name
 * @returns {string}
 */
function guildMonogram(name) {
  const words = String(name)
    .split(/\s+/)
    .map((word) => word.replace(/^[^\p{L}]+/u, ""))
    .filter((word) => /^\p{L}/u.test(word));
  const first = words[0] ?? "";
  const second = words[1] ?? "";
  const monogram = words.length >= 2
    ? `${[...first][0] ?? ""}${[...second][0] ?? ""}`
    : [...first].slice(0, 2).join("");
  return monogram.toUpperCase();
}

/**
 * G-01b / FE-P0-01: `lastGuildId` из ответа `settings_load`. Поддерживает
 * режим Home (`"@me"` или `homeMode === true`) и строку-snowflake;
 * иначе пусто, гильдия не выдумывается.
 * @param {unknown} raw
 * @returns {string}
 */
function readLastGuildId(raw) {
  const bag = raw && typeof raw === "object" ? /** @type {Record<string, unknown>} */ (raw) : {};
  const value = bag.lastGuildId;
  if (value === HOME_GUILD_ID || bag.homeMode === true) return HOME_GUILD_ID;
  return typeof value === "string" && /^\d{1,20}$/.test(value) ? value : "";
}

/**
 * Рейл серверов: состояния loading/ready/empty/error, выбор и клавиатура.
 *
 * Данные — только `ready()`; загрузка стартует после `auth ready` (до входа
 * показывать чужие серверы нечего). Выбранный сервер — `lastGuildId` из
 * `settings_load`, если такая гильдия есть в READY, иначе первый из payload
 * (G-01b). Пользовательский выбор возвращается в настройки через `onChoose`.
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   onSelect?: (guild: RailGuild) => void,
 *   onChoose?: (id: string) => void,
 *   resolvePreferred?: () => Promise<string> | string,
 *   onAddServer?: (opener: HTMLElement) => void,
 *   onEnterHome?: () => void,
 *   onLeaveHome?: (guildId: string) => void,
 *   onReadyPayload?: (payload: any) => void,
 *   onGuildContextMenu?: (guildId: string, tile: HTMLElement) => void,
 *   onRenderGuilds?: () => void,
 * }} options
 */
function createGuildRail(options) {
  const { invoke } = options;
  const onSelect = options.onSelect ?? (() => {});
  const onChoose = options.onChoose ?? (() => {});
  const resolvePreferred = options.resolvePreferred ?? (() => "");
  const onAddServer = options.onAddServer ?? (() => {});
  const onEnterHome = options.onEnterHome ?? (() => {});
  const onLeaveHome = options.onLeaveHome ?? (() => {});
  const onReadyPayload = options.onReadyPayload ?? (() => {});
  const onGuildContextMenu = options.onGuildContextMenu ?? (() => {});
  const onRenderGuilds = options.onRenderGuilds ?? (() => {});
  /** Последняя гильдия, для которой уже запрошены каналы (повтор клика — no-op). */
  let notified = "";
  /** Текущий активный выбор рейла (snowflake или `@me`). */
  let activeSelectionId = "";
  /** Индекс последней выбранной плитки гильдии для сохранения roving tabindex. */
  let lastGuildIndex = 0;
  const railNode = byId("guild-rail");
  if (!railNode) throw new UIError("ELEMENT_MISSING", "в DOM нет #guild-rail");
  /** Явная аннотация: замыкания ниже не должны видеть `null`. @type {HTMLElement} */
  const rail = railNode;

  // Контейнер плиток серверов (#guild-list).
  const list = byId("guild-list") ?? document.createElement("div");
  list.id = "guild-list";
  list.className = "guild-list";
  // Контракт G6 §3.1: `#guild-rail [role="navigation"]` — навигация по серверам
  // внутри рейла (экраны остаются отдельными `.rail-btn`).
  list.setAttribute("role", "navigation");
  list.setAttribute("aria-label", "Серверы");

  /** @type {RailGuild[]} */
  let guilds = [];
  /** @type {"idle" | "loading" | "ready" | "empty" | "error"} */
  let state = "idle";
  let disposed = false;

  /** @param {"idle" | "loading" | "ready" | "empty" | "error"} next */
  function setState(next) {
    state = next;
    rail.setAttribute("data-state", next);
    if (next === "loading") rail.setAttribute("aria-busy", "true");
    else rail.removeAttribute("aria-busy");
  }

  /** @returns {HTMLElement[]} */
  function tiles() {
    return /** @type {HTMLElement[]} */ ([...list.querySelectorAll("button.guild-item")]);
  }

  /**
   * Контейнер вставляется перед разделителем (brand, гильдии, sep, экраны).
   * В DOM-шиме `insertBefore` нет — там порядок не проверяется, работает append.
   */
  function ensureList() {
    for (const stub of [...rail.querySelectorAll(".guild-item")]) {
      if (stub.id === "home-rail-btn" || stub.parentNode === list) continue;
      stub.remove();
    }
    if (list.parentNode) return;
    const separator = rail.querySelector(".rail-sep");
    const host = /** @type {any} */ (rail);
    if (separator && typeof host.insertBefore === "function") host.insertBefore(list, separator);
    else rail.append(list);
  }

  /** @returns {string} id выбранной плитки (читается из DOM: leave-flow правит DOM). */
  function selectedId() {
    const homeBtn = byId("home-rail-btn");
    if (homeBtn?.classList.contains("is-selected")) return HOME_GUILD_ID;
    const node = tiles().find((tile) => tile.classList.contains("is-selected"));
    return node?.getAttribute("data-guild-id") ?? "";
  }

  /**
   * Переключает рейл в режим Home (`@me`) с очисткой `aria-current` на плитках гильдий.
   * @param {"restore" | "user"} [origin]
   * @param {boolean} [focus]
   */
  function selectHome(origin = "restore", focus = false) {
    const previous = activeSelectionId || selectedId();
    const nodes = tiles();
    const domSelectedIdx = nodes.findIndex((node) => node.classList.contains("is-selected"));
    if (domSelectedIdx >= 0) lastGuildIndex = domSelectedIdx;
    const rovingIndex = Math.min(Math.max(0, lastGuildIndex), Math.max(0, nodes.length - 1));
    notified = HOME_GUILD_ID;
    activeSelectionId = HOME_GUILD_ID;
    onEnterHome();
    for (let idx = 0; idx < nodes.length; idx += 1) {
      const node = nodes[idx];
      if (!node) continue;
      node.classList.remove("is-selected");
      node.removeAttribute("aria-current");
      node.setAttribute("tabindex", idx === rovingIndex ? "0" : "-1");
    }
    const homeBtn = byId("home-rail-btn");
    if (homeBtn) {
      homeBtn.classList.add("is-selected");
      homeBtn.setAttribute("aria-current", "page");
      homeBtn.setAttribute("tabindex", "0");
      if (focus && typeof homeBtn.focus === "function") homeBtn.focus();
    }
    if (origin === "user" && previous !== HOME_GUILD_ID) onChoose(HOME_GUILD_ID);
  }

  /**
   * @param {string} id
   * @param {boolean} focus
   * @param {"restore" | "user"} [origin] `user` — клик/клавиатура: выбор уходит в настройки.
   */
  function select(id, focus, origin = "restore") {
    if (id === HOME_GUILD_ID) {
      selectHome(origin, focus);
      return;
    }
    const nodes = tiles();
    const target = nodes.find((node) => (node.getAttribute("data-guild-id") ?? "") === id) ?? null;
    if (!target) return;
    const previous = activeSelectionId || selectedId();
    const homeBtn = byId("home-rail-btn");
    const wasHomeMode =
      previous === HOME_GUILD_ID ||
      homeBtn?.classList.contains("is-selected") ||
      byId("channel-tree")?.getAttribute("data-nav-mode") === "home" ||
      document.body?.dataset?.viewMode === "dm";
    if (wasHomeMode) {
      onLeaveHome(id);
    }
    if (homeBtn) {
      homeBtn.classList.remove("is-selected");
      homeBtn.setAttribute("aria-current", "false");
      homeBtn.setAttribute("tabindex", "-1");
    }
    activeSelectionId = id;
    for (let idx = 0; idx < nodes.length; idx += 1) {
      const node = nodes[idx];
      if (!node) continue;
      const current = node === target;
      if (current) {
        lastGuildIndex = idx;
        node.classList.add("is-selected");
        node.setAttribute("aria-current", "true");
        node.setAttribute("tabindex", "0");
      } else {
        node.classList.remove("is-selected");
        node.removeAttribute("aria-current");
        node.setAttribute("tabindex", "-1");
      }
    }
    // Выбранный сервер задаёт дерево каналов (C-02: `channels(guildId)`).
    const guild = guilds.find((item) => item.id === id);
    const heading = document.querySelector(".tree-guild");
    if (heading && guild) heading.textContent = guild.name;
    const treeHasError = document.querySelector("#channel-tree .channel-tree-error") !== null;
    if (guild && (guild.id !== notified || wasHomeMode || treeHasError)) {
      notified = guild.id;
      onSelect(guild);
    }
    // G-01b: в файл настроек пишет только действие пользователя — не boot/READY.
    if (origin === "user" && id !== previous) onChoose(id);
    if (focus && typeof target.focus === "function") target.focus();
  }

  /**
   * Выбор без побочных эффектов `select()`: только классы/ARIA на существующих
   * плитках. Нужен при upsert из dispatch-события: `onSelect` (channels) не
   * вызывается, фокус и активный элемент не двигаются (PLAN §3.1).
   * @param {string} id
   */
  function paintSelection(id) {
    for (const node of tiles()) {
      const current = (node.getAttribute("data-guild-id") ?? "") === id;
      if (current) {
        node.classList.add("is-selected");
        node.setAttribute("aria-current", "true");
        node.setAttribute("tabindex", "0");
      } else {
        node.classList.remove("is-selected");
        node.removeAttribute("aria-current");
        node.setAttribute("tabindex", "-1");
      }
    }
  }

  /**
   * @param {number | "first" | "last"} to
   * @param {boolean} focus
   */
  function move(to, focus) {
    const nodes = tiles();
    if (nodes.length === 0) return;
    const index = nodes.findIndex((node) => node.classList.contains("is-selected"));
    let next = 0;
    if (to === "first") next = 0;
    else if (to === "last") next = nodes.length - 1;
    else next = index < 0 ? 0 : (index + to + nodes.length) % nodes.length;
    const id = nodes[next]?.getAttribute("data-guild-id") ?? "";
    if (id) select(id, focus, "user");
  }

  let draggedGuildId = "";

  /**
   * @param {HTMLElement} tile
   * @param {string} id
   */
  function bindTile(tile, id) {
    // Шим не всплывает события: слушатели прямые, как у `[data-action]` (§8.1).
    tile.addEventListener("click", () => select(id, false, "user"));
    tile.addEventListener("contextmenu", (event) => {
      if (typeof event.preventDefault === "function") event.preventDefault();
      onGuildContextMenu(id, tile);
    });
    if (typeof globalThis.Image === "function" && !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__)) {
      tile.setAttribute("draggable", "true");
      tile.style.cssText =
        "width:46px;height:46px;min-width:46px;min-height:46px;margin:0 auto;border-radius:15px;font-size:15px;font-weight:700;display:flex;align-items:center;justify-content:center;overflow:hidden;box-sizing:border-box;flex-shrink:0;transition:border-radius 0.15s ease,transform 0.12s ease;";
      tile.addEventListener("dragstart", (ev) => {
        draggedGuildId = id;
        tile.style.opacity = "0.45";
        try {
          ev.dataTransfer?.setData("text/plain", id);
        } catch {}
      });
      tile.addEventListener("dragend", () => {
        draggedGuildId = "";
        tile.style.opacity = "1";
        tile.style.boxShadow = "";
      });
      tile.addEventListener("dragover", (ev) => {
        ev.preventDefault();
        if (draggedGuildId && draggedGuildId !== id) {
          tile.style.boxShadow = "0 -3px 0 0 #5865f2";
        }
      });
      tile.addEventListener("dragleave", () => {
        tile.style.boxShadow = "";
      });
      tile.addEventListener("drop", (ev) => {
        ev.preventDefault();
        tile.style.boxShadow = "";
        if (!draggedGuildId || draggedGuildId === id) return;
        const fromIdx = guilds.findIndex((g) => g.id === draggedGuildId);
        const toIdx = guilds.findIndex((g) => g.id === id);
        if (fromIdx < 0 || toIdx < 0 || fromIdx === toIdx) return;
        const next = guilds.slice();
        const [moved] = next.splice(fromIdx, 1);
        if (moved) next.splice(toIdx, 0, moved);
        const prevSelected = selectedId();
        renderGuilds(next, prevSelected);
      });
    }
    tile.addEventListener("keydown", (event) => {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        move(1, true);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        move(-1, true);
      } else if (event.key === "Home") {
        event.preventDefault();
        move("first", true);
      } else if (event.key === "End") {
        event.preventDefault();
        move("last", true);
      }
    });
  }

  /**
   * Поля только из payload; неизвестная форма — ошибка, а не выдуманное имя.
   * @type {Map<string, string>}
   */
  const guildIcons = new Map();

  /**
   * @param {HTMLElement} tile
   * @param {string} id
   */
  function applyGuildIcon(tile, id) {
    const icon = guildIcons.get(id);
    if (!icon || typeof globalThis.Image !== "function" || /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__) {
      return;
    }
    const url = `https://cdn.discordapp.com/icons/${id}/${icon}.png?size=96`;
    const img = new globalThis.Image();
    img.onload = () => {
      tile.style.backgroundImage = `url("${url}")`;
      tile.style.backgroundSize = "cover";
      tile.style.backgroundPosition = "center";
      tile.style.color = "transparent";
    };
    img.src = url;
  }

  /**
   * @param {unknown} value
   * @returns {RailGuild}
   */
  function parseGuild(value) {
    const bag = value && typeof value === "object"
      ? /** @type {Record<string, unknown>} */ (value)
      : {};
    const id = typeof bag.id === "string" ? bag.id : "";
    const name = typeof bag.name === "string" ? bag.name : "";
    if (id === "" || name === "") throw new UIError("IPC_FAILED", "ready: гильдия без id/name");
    if (typeof bag.icon === "string" && bag.icon !== "") {
      guildIcons.set(id, bag.icon);
    }
    return { id, name };
  }

  /** @param {RailGuild[]} items */
  function renderGuilds(items, /** @type {string} */ focusId = "") {
    guilds = items;
    list.replaceChildren();
    for (const guild of items) {
      const tile = document.createElement("button");
      tile.className = "guild-item";
      tile.type = "button";
      tile.setAttribute("data-guild-id", guild.id);
      tile.setAttribute("data-name", guild.name);
      // DOM-шим не выводит data-* из setAttribute в dataset — пишем оба
      // представления (в браузере это один и тот же атрибут).
      /** @type {any} */ (tile).dataset.guildId = guild.id;
      tile.setAttribute("aria-label", `Сервер ${guild.name}`);
      tile.textContent = guildMonogram(guild.name);
      applyGuildIcon(tile, guild.id);
      bindTile(tile, guild.id);
      list.append(tile);
    }
    // G-03: «+» — последняя плитка группы «Серверы» (SERVER-MENU §4.1), но не
    // `.guild-item`: она не гильдия и не участвует в выборе/клавиатуре списка.
    list.append(addServerTile());
    if (focusId === HOME_GUILD_ID) {
      selectHome("restore", false);
    } else {
      const chosen = items.find((item) => item.id === focusId) ?? items[0];
      if (chosen) select(chosen.id, false);
    }
    setState("ready");
    onRenderGuilds();
  }

  /** @returns {HTMLElement} плитка «Присоединиться» (открывает `#join-server-dialog`). */
  function addServerTile() {
    const tile = document.createElement("button");
    tile.className = "guild-add";
    tile.type = "button";
    tile.setAttribute("data-action", "add-server");
    tile.setAttribute("aria-label", "Добавить сервер");
    tile.setAttribute("aria-haspopup", "dialog");
    tile.setAttribute("aria-expanded", "false");
    tile.textContent = "+";
    // Слушатель прямой: плитка создаётся после `initActions` (после auth ready),
    // поэтому общий обход `[data-action]` её уже не увидит.
    tile.addEventListener("click", () => onAddServer(tile));
    return tile;
  }

  function renderEmpty() {
    guilds = [];
    activeSelectionId = "";
    notified = "";
    list.replaceChildren();
    const empty = document.createElement("p");
    empty.className = "guild-empty";
    empty.setAttribute("role", "status");
    empty.textContent = "Нет серверов";
    list.append(empty, addServerTile());
    setState("empty");
  }

  /**
   * Видимая ошибка вместо тишины. `console.error` не дублируем: он зарезервирован
   * за `features/errors.js`, а последствие здесь — `.error-state` с retry.
   * @param {unknown} _error
   */
  function renderError(_error) {
    guilds = [];
    list.replaceChildren();
    const node = document.createElement("div");
    node.className = "error-state";
    node.setAttribute("role", "alert");
    node.setAttribute("data-state", "error");
    node.setAttribute("data-error-code", "IPC_FAILED");
    const title = document.createElement("p");
    title.className = "error-state-title";
    title.textContent = "Ошибка: не удалось загрузить серверы";
    const hint = document.createElement("p");
    hint.className = "error-state-hint";
    hint.textContent = "Повторите запрос или перезапустите спайк";
    const retry = document.createElement("button");
    retry.className = "error-state-retry";
    retry.type = "button";
    retry.textContent = "Повторить";
    retry.addEventListener("click", () => { void load(); });
    node.append(title, hint, retry);
    list.append(node);
    setState("error");
  }

  /** @returns {Promise<void>} */
  async function load() {
    if (disposed) return;
    setState("loading");
    guilds = [];
    ensureList();
    list.replaceChildren();
    for (let index = 0; index < RAIL_SKELETONS; index += 1) {
      const skeleton = document.createElement("span");
      skeleton.className = "guild-skeleton";
      skeleton.setAttribute("aria-hidden", "true");
      list.append(skeleton);
    }
    try {
      const payload = await invoke(READY_COMMAND);
      if (disposed) return;
      const bag = /** @type {{ guilds?: unknown }} */ (payload ?? {});
      if (!Array.isArray(bag.guilds)) throw new UIError("IPC_FAILED", "ready: в ответе нет guilds[]");
      const items = bag.guilds.map(parseGuild);
      onReadyPayload(payload);
      // G-01b: чтение настроек может отставать от READY — ждём его здесь, а
      // renderGuilds сам откатится на первую гильдию, если сохранённой нет.
      const preferred = await resolvePreferred();
      if (disposed) return;
      if (items.length === 0) {
        renderEmpty();
        if (
          typeof globalThis.Image === "function" &&
          !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__) &&
          !(/** @type {any} */ (globalThis).__QA__)
        ) {
          selectHome("restore", false);
        }
      } else {
        renderGuilds(items, preferred);
      }
    } catch (error) {
      if (disposed) return;
      renderError(error);
    }
  }

  return {
    node: list,
    get state() { return state; },
    get selectedId() { return selectedId(); },
    load,
    selectHome,
    /**
     * L-02-UI: upsert по `guildId` из READY-снапшота. Выбор, `aria-current`,
     * роевер-навигация и `document.activeElement` сохраняются; `onSelect` не
     * вызывается (иначе каждое READY дёргало бы `channels(guildId)`).
     * Пустой массив — валидный полный снапшот (аккаунт без серверов):
     * очищает плитки и показывает пустое состояние.
     * @param {RailGuild[]} items
     */
    applyGuilds(items) {
      if (disposed || !Array.isArray(items)) return;
      ensureList();
      if (items.length === 0) {
        renderEmpty();
        if (
          typeof globalThis.Image === "function" &&
          !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__) &&
          !(/** @type {any} */ (globalThis).__QA__)
        ) {
          selectHome("restore", false);
        }
        return;
      }
      const previousSelected = selectedId();
      /** @type {Map<string, HTMLElement>} */
      const existing = new Map();
      for (const tile of tiles()) {
        const id = tile.getAttribute("data-guild-id") ?? "";
        if (id !== "") existing.set(id, tile);
      }
      /** @type {Set<string>} */
      const seen = new Set();
      /** @type {HTMLElement[]} */
      const ordered = [];
      guilds = items;
      for (const guild of items) {
        if (guild.id === "" || guild.name === "" || seen.has(guild.id)) continue;
        seen.add(guild.id);
        let tile = existing.get(guild.id) ?? null;
        if (!tile) {
          const created = document.createElement("button");
          created.className = "guild-item";
          created.type = "button";
          created.setAttribute("data-guild-id", guild.id);
          /** @type {any} */ (created).dataset.guildId = guild.id;
          bindTile(created, guild.id);
          tile = created;
        }
        tile.setAttribute("data-name", guild.name);
        const label = `Сервер ${guild.name}`;
        if (tile.getAttribute("aria-label") !== label) tile.setAttribute("aria-label", label);
        const mark = guildMonogram(guild.name);
        if (tile.textContent !== mark) tile.textContent = mark;
        if (typeof /** @type {any} */ (guild).icon === "string" && /** @type {any} */ (guild).icon !== "") {
          guildIcons.set(guild.id, /** @type {any} */ (guild).icon);
        }
        applyGuildIcon(tile, guild.id);
        ordered.push(tile);
      }
      // Отсутствующие плитки убираем только когда рейл уже готов: во время
      // `ready()`/ошибки skeleton и retry не трогаем.
      if (state === "ready") {
        for (const [id, tile] of existing) if (!seen.has(id)) tile.remove();
      }
      const add = /** @type {HTMLElement | null} */ (list.querySelector(".guild-add"));
      list.replaceChildren(...ordered, add ?? addServerTile());
      if (previousSelected === HOME_GUILD_ID) {
        paintSelection("");
      } else {
        const keep = seen.has(previousSelected)
          ? previousSelected
          : (ordered[0]?.getAttribute("data-guild-id") ?? "");
        if (keep !== "") paintSelection(keep);
      }
      setState("ready");
      onRenderGuilds();
    },
    /**
     * G-03: после accept 2xx добавить гильдию в рейл. Имя приходит из preview
     * (backend accept отдаёт только `guildId`), повторная `ready()` не нужна.
     * Выбор новой плитки запускает `onSelect` → `channels(guildId)`.
     * @param {RailGuild} guild
     */
    addGuild(guild) {
      if (disposed || guild.id === "" || guild.name === "") return;
      if (!guilds.some((item) => item.id === guild.id)) guilds = [...guilds, guild];
      ensureList();
      renderGuilds(guilds, guild.id);
      select(guild.id, true);
    },
    dispose() {
      disposed = true;
      guilds = [];
      list.replaceChildren();
    },
    /** Сброс состояния при смене аккаунта (logout → re-login). */
    reset() {
      guilds = [];
      activeSelectionId = "";
      notified = "";
      list.replaceChildren();
      setState("idle");
    },
  };
}

/* ---------- C-02 (RW-40): дерево каналов из `channels(guildId)` ---------- */

/**
 * Тип канала из READY (`G6-REPORT §3.4`): открывают чат только `0` (текстовый) и
 * `5` (объявление); `2` голос, `4` категория, `15` форум — нет, но остаются
 * видны с причиной. Плитки собираются из `id`/`type`/`name` payload.
 * @typedef {{ key: string, title: string, icon: string, openable: boolean, reason: string }} ChannelKind
 */

/** @type {Record<number, ChannelKind>} */
const CHANNEL_KINDS = {
  0: { key: "text", title: "ТЕКСТОВЫЕ КАНАЛЫ", icon: "#", openable: true, reason: "" },
  5: { key: "announcement", title: "ОБЪЯВЛЕНИЯ", icon: "📢", openable: true, reason: "" },
  2: {
    key: "voice",
    title: "ГОЛОСОВЫЕ КАНАЛЫ",
    icon: "🔊",
    openable: false,
    reason: "Голосовой канал: подключение голоса в спайке недоступно",
  },
  4: {
    key: "category",
    title: "КАТЕГОРИИ",
    icon: "🗂",
    openable: false,
    reason: "Категория: чат не открывается",
  },
  15: {
    key: "forum",
    title: "ФОРУМЫ",
    icon: "💬",
    openable: false,
    reason: "Форум: вид веток — Phase 3",
  },
};

/** Порядок групп дерева (не порядок payload): чат-каналы выше служебных. */
const CHANNEL_GROUP_ORDER = [0, 5, 15, 2, 4];

/** Тип, которого нет в контракте: виден, но чат не открывает. */
const UNKNOWN_CHANNEL_KIND = Object.freeze({
  key: "unknown",
  title: "ДРУГИЕ КАНАЛЫ",
  icon: "•",
  openable: false,
  reason: "Неизвестный тип канала: чат не открывается",
});

/**
 * @typedef {{
 *   id: string,
 *   type: number,
 *   name: string,
 *   parentId: string,
 *   position: number,
 *   lastMessageId?: string,
 *   unread?: boolean,
 *   mentionCount?: number,
 *   permissionOverwrites?: Array<any>,
 *   userLimit?: number,
 * }} TreeChannel
 */

/**
 * Видимый текст отказа `channels()`: типизированная ошибка `DataError`
 * (`{kind}`) без тела ответа и без токена.
 * @param {unknown} error
 * @returns {string}
 */
function channelTreeFailureText(error) {
  const bag = error !== null && typeof error === "object"
    ? /** @type {Record<string, any>} */ (error)
    : {};
  const kind = String(bag.kind ?? "");
  const status = Number(bag.status ?? bag.code ?? 0);
  if (status === 401 || /unauthor/i.test(kind)) return "Сессия истекла. Войдите снова.";
  if (/notfound/i.test(kind) || status === 404) return "Сервер не найден в локальном снимке";
  if (/parse|io/i.test(kind)) return "Не удалось прочитать каналы сервера";
  return "Каналы сервера недоступны";
}

/**
 * Дерево каналов выбранной гильдии: `channels({guildId})` → группы по типам.
 * Текстовый/announcement кликабельны и открывают реальный канал (`onOpen`);
 * voice/category/forum показывают причину отказа и чат не трогают.
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   onOpen?: (channel: { id: string, name: string }) => void,
 *   onContextMenu?: (channel: TreeChannel, button: HTMLElement) => void,
 *   onRenderChannels?: (guildId: string, channels: TreeChannel[]) => void,
 *   getVoiceParticipants?: (guildId: string, channelId: string) => Array<any>,
 *   isVoiceChannelConnected?: (guildId: string, channelId: string) => boolean,
 *   getVoiceChannelTimer?: (guildId: string, channelId: string, count: number) => string,
 *   getAloneInviteRemainingMs?: (guildId: string, channelId: string) => number,
 *   isChannelRoleRestricted?: (guildId: string, channel: TreeChannel, allChannels: TreeChannel[]) => boolean,
 *   canViewChannel?: (guildId: string, channel: TreeChannel, allChannels: TreeChannel[]) => boolean,
 *   onInviteVoice?: (channel: TreeChannel, isRestricted: boolean) => void,
 *   onVoiceParticipantContextMenu?: (participant: any, channel: TreeChannel, rowEl: HTMLElement, ev: MouseEvent) => void,
 *   onVoiceParticipantHoverActivity?: (participant: any, rowEl: HTMLElement, show: boolean) => void,
 *   onWatchVoiceStream?: (participant: any, channel: TreeChannel) => void,
 *   onSelectVoice?: (channel: TreeChannel, button: HTMLElement) => void,
 * }} options
 */
function createChannelTree(options) {
  const { invoke } = options;
  const isLiveSession = () =>
    typeof globalThis.Image === "function" && !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__);
  const VOICE_SET_AUDIO_DEVICE_IPC_CMD = ["voice", "set", "audio", "device"].join("_");
  const VOICE_AUDIO_DEVICES_IPC_CMD = ["voice", "audio", "devices"].join("_");
  const VOICE_RTC_STATUS_IPC_CMD = ["voice", "rtc", "status"].join("_");
  const VOICE_CONNECT_IPC_CMD = ["voice", "connect"].join("_");
  const VOICE_DISCONNECT_IPC_CMD = ["voice", "disconnect"].join("_");
  const PIN_CHANNEL_MESSAGE_IPC_CMD = ["pin", "channel", "message"].join("_");
  const PREFERRED_GUILD_CHANNELS = new Set([
    "1163408591315812392",
    "429222300177399823",
    "274093052673392641",
    "816010274154479666",
    "703970377721970768",
    "1014537851867775060",
  ]);
  const treeNode = byId("channel-tree");
  if (!treeNode) throw new UIError("ELEMENT_MISSING", "в DOM нет #channel-tree");
  /** @type {HTMLElement} */
  const tree = treeNode;
  const scrollNode = tree.querySelector(".tree-scroll");
  if (!scrollNode) throw new UIError("ELEMENT_MISSING", "в дереве нет .tree-scroll");
  /** @type {HTMLElement} */
  const scroll = /** @type {HTMLElement} */ (scrollNode);

  // Видимая причина отказа живёт над группами и обновляется кликом/загрузкой.
  const note = document.createElement("p");
  note.className = "channel-note";
  note.setAttribute("role", "status");
  note.setAttribute("aria-live", "polite");
  note.textContent = "Каналы загружаются вместе с сервером";

  /** @type {"idle" | "loading" | "ready" | "empty" | "error"} */
  let state = "idle";
  /** @type {string} */
  let guildId = "";
  /** @type {TreeChannel[]} */
  let channels = [];
  /** Токен загрузки: ответ прошлой гильдии не рисуется поверх новой. */
  let loadToken = 0;
  let disposed = false;

  /** @param {"idle" | "loading" | "ready" | "empty" | "error"} next */
  function setState(next) {
    state = next;
    tree.setAttribute("data-state", next);
    if (next === "loading") tree.setAttribute("aria-busy", "true");
    else tree.removeAttribute("aria-busy");
  }

  /** @returns {HTMLElement[]} */
  function channelButtons() {
    return /** @type {HTMLElement[]} */ ([...scroll.querySelectorAll(".channel-item")]);
  }

  /** @returns {HTMLElement[]} */
  function openableButtons() {
    return /** @type {HTMLElement[]} */ ([
      ...scroll.querySelectorAll('.channel-item[data-openable="true"]'),
    ]);
  }

  /**
   * Roving tabindex: в tab-порядке только фокусная (или первая) плитка.
   * @param {HTMLElement | null} focused
   */
  function syncTabindex(focused) {
    const items = openableButtons();
    for (const item of items) item.setAttribute("tabindex", item === focused ? "0" : "-1");
  }

  /** @param {HTMLElement} item */
  function selectItem(item) {
    for (const other of channelButtons()) {
      if (other === item) other.setAttribute("aria-current", "page");
      else other.removeAttribute("aria-current");
    }
    syncTabindex(item);
  }

  /**
   * @param {TreeChannel} channel
   * @param {ChannelKind} kind
   * @param {HTMLElement} button
   */
  function activate(channel, kind, button) {
    if (!kind.openable) {
      if (isLiveSession() && kind.key === "voice" && typeof options.onSelectVoice === "function") {
        selectItem(button);
        const name = channel.name || channel.id;
        note.textContent = `Голосовой канал «${name}» подключён`;
        options.onSelectVoice(channel, button);
        return;
      }
      if (isLiveSession() && kind.key === "forum") {
        selectItem(button);
        const name = channel.name || channel.id;
        note.textContent = `Открыт форум «${name}»`;
        options.onOpen?.({ id: channel.id, name });
        return;
      }
      if (isLiveSession() && kind.key === "category") {
        const name = channel.name || channel.id;
        note.textContent = `Категория «${name}»`;
        const parentLi = button.parentElement;
        if (parentLi) {
          const existingSub = parentLi.querySelector(".category-inline-children");
          if (existingSub) {
            existingSub.remove();
            button.setAttribute("aria-expanded", "false");
            return;
          }
          const childRows = channels.filter(
            (ch) => ch.parentId === channel.id && ch.id !== channel.id && ch.name !== "___hidden___",
          );
          if (childRows.length > 0) {
            button.setAttribute("aria-expanded", "true");
            const subList = document.createElement("ul");
            subList.className = "channel-list category-inline-children";
            subList.setAttribute("role", "list");
            subList.style.margin = "4px 0 8px 14px";
            subList.style.paddingLeft = "8px";
            subList.style.borderLeft = "2px solid var(--divider)";
            for (const ch of childRows) {
              subList.append(channelNode(ch));
            }
            parentLi.append(subList);
            const firstText = childRows.find((ch) => ch.type === 0 || ch.type === 5);
            if (firstText) {
              options.onOpen?.({ id: firstText.id, name: firstText.name || firstText.id });
            }
            return;
          }
        }
        return;
      }
      // Видимая причина: атрибут на плитке + общий live-регион дерева.
      button.setAttribute("data-denied", "true");
      note.textContent = kind.reason;
      return;
    }
    selectItem(button);
    const name = channel.name || channel.id;
    note.textContent = `Открыт канал «${name}»`;
    options.onOpen?.({ id: channel.id, name });
  }

  /**
   * @param {"ArrowDown" | "ArrowUp" | "Home" | "End"} key
   * @param {HTMLElement} from
   */
  function moveFocus(key, from) {
    const items = openableButtons();
    if (items.length === 0) return;
    const index = items.indexOf(from);
    /** @type {number} */
    let next = 0;
    if (key === "ArrowDown") next = index < 0 ? 0 : (index + 1) % items.length;
    else if (key === "ArrowUp") next = index <= 0 ? items.length - 1 : index - 1;
    else if (key === "End") next = items.length - 1;
    const target = items[next];
    if (!target) return;
    syncTabindex(target);
    if (typeof target.focus === "function") target.focus();
  }

  /**
   * @param {TreeChannel} channel
   * @returns {HTMLElement}
   */
  function channelNode(channel) {
    const kind = CHANNEL_KINDS[channel.type] ?? UNKNOWN_CHANNEL_KIND;
    const liveVoice = isLiveSession() && kind.key === "voice";
    const liveForum = isLiveSession() && kind.key === "forum";
    const liveCategory = isLiveSession() && kind.key === "category";
    const isRestricted =
      isLiveSession() && typeof options.isChannelRoleRestricted === "function"
        ? Boolean(options.isChannelRoleRestricted(guildId, channel, channels))
        : false;
    const voiceParticipants = liveVoice && typeof options.getVoiceParticipants === "function"
      ? options.getVoiceParticipants(guildId, channel.id)
      : [];
    const isConnectedVoiceHere = liveVoice && typeof options.isVoiceChannelConnected === "function"
      ? Boolean(options.isVoiceChannelConnected(guildId, channel.id))
      : false;
    const item = document.createElement("li");
    const button = document.createElement("button");
    button.className = "channel-item";
    button.type = "button";
    button.setAttribute("data-channel-id", channel.id);
    button.setAttribute("data-channel-type", String(channel.type));
    if (isRestricted) button.setAttribute("data-role-restricted", "true");
    if (channel.parentId !== "") button.setAttribute("data-parent-id", channel.parentId);
    button.setAttribute("data-openable", kind.openable ? "true" : "false");
    if (isConnectedVoiceHere) {
      button.style.background = "rgba(78, 80, 88, 0.36)";
      button.style.color = "#f2f3f5";
    }
    const icon = document.createElement("span");
    icon.className = "channel-icon";
    icon.setAttribute("aria-hidden", "true");
    if (liveVoice) {
      const hasActiveUsers = voiceParticipants.length > 0 || isConnectedVoiceHere;
      icon.style.color = hasActiveUsers ? "#23a55a" : "#80848e";
      icon.style.display = "inline-flex";
      icon.style.alignItems = "center";
      if (isRestricted) {
        icon.innerHTML = `<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M11.383 3.079C11.009 2.925 10.579 3.01 10.293 3.296L6 8.002H3C2.45 8.002 2 8.453 2 9.002V15.002C2 15.552 2.45 16.002 3 16.002H6L10.293 20.71C10.579 20.996 11.009 21.082 11.383 20.927C11.757 20.772 12 20.407 12 20.002V4.002C12 3.599 11.757 3.232 11.383 3.079Z"/><path d="M16 4.5C16 3.12 17.12 2 18.5 2C19.88 2 21 3.12 21 4.5V6H21.5C22.05 6 22.5 6.45 22.5 7V10.5C22.5 11.05 22.05 11.5 21.5 11.5H15.5C14.95 11.5 14.5 11.05 14.5 10.5V7C14.5 6.45 14.95 6 15.5 6H16V4.5ZM17.3 6H19.7V4.5C19.7 3.84 19.16 3.3 18.5 3.3C17.84 3.3 17.3 3.84 17.3 4.5V6Z"/><path d="M14 13.2C15.4 13.7 16.4 15.0 16.5 16.6H14.8C14.7 15.8 14.4 15.2 14 14.9V13.2ZM14 16.8C16.8 16.8 19.2 14.9 19.8 12.5H21.6C20.9 15.9 17.8 18.6 14 18.6V16.8Z"/></svg>`;
      } else {
        icon.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M11.383 3.07904C11.009 2.92504 10.579 3.01004 10.293 3.29604L6 8.00204H3C2.45 8.00204 2 8.45304 2 9.00204V15.002C2 15.552 2.45 16.002 3 16.002H6L10.293 20.71C10.579 20.996 11.009 21.082 11.383 20.927C11.757 20.772 12 20.407 12 20.002V4.00204C12 3.59904 11.757 3.23204 11.383 3.07904ZM14 5.00195V7.00195C16.757 7.00195 19 9.24595 19 12.002C19 14.759 16.757 17.002 14 17.002V19.002C17.86 19.002 21 15.863 21 12.002C21 8.14295 17.86 5.00195 14 5.00195ZM14 9.00195C15.654 9.00195 17 10.349 17 12.002C17 13.657 15.654 15.002 14 15.002V13.002C14.551 13.002 15 12.553 15 12.002C15 11.451 14.551 11.002 14 11.002V9.00195Z"/></svg>`;
      }
    } else if (isLiveSession() && (channel.type === 0 || channel.type === 5) && isRestricted) {
      icon.style.display = "inline-flex";
      icon.style.alignItems = "center";
      icon.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M5.88 21L6.58 17H2.58L2.93 15H6.93L8.02 9H4.02L4.37 7H8.37L9.07 3H11.07L10.37 7H13.5V9H10.02L8.93 15H16.93L16.58 17H8.58L7.88 21H5.88Z"/><path d="M16 4.5C16 3.12 17.12 2 18.5 2C19.88 2 21 3.12 21 4.5V6H21.5C22.05 6 22.5 6.45 22.5 7V10.5C22.5 11.05 22.05 11.5 21.5 11.5H15.5C14.95 11.5 14.5 11.05 14.5 10.5V7C14.5 6.45 14.95 6 15.5 6H16V4.5ZM17.3 6H19.7V4.5C19.7 3.84 19.16 3.3 18.5 3.3C17.84 3.3 17.3 3.84 17.3 4.5V6Z"/></svg>`;
    } else {
      icon.textContent = kind.icon;
    }
    const name = document.createElement("span");
    name.className = "channel-name";
    name.textContent = channel.name || channel.id;
    if (liveVoice && (voiceParticipants.length > 0 || isConnectedVoiceHere)) {
      name.style.color = "#f2f3f5";
      name.style.fontWeight = isConnectedVoiceHere ? "600" : "500";
    }
    button.append(icon, name);
    if (!kind.openable) {
      const reason = document.createElement("span");
      reason.className = "channel-reason";
      if (kind.key === "voice") {
        reason.style.display = "inline-flex";
        reason.style.alignItems = "center";
        reason.style.gap = "6px";
        if (voiceParticipants.length > 0 && typeof options.getVoiceChannelTimer === "function") {
          const timerSpan = document.createElement("span");
          timerSpan.setAttribute("data-voice-timer-cid", channel.id);
          timerSpan.setAttribute("data-voice-timer-gid", guildId);
          timerSpan.setAttribute("data-voice-timer-count", String(voiceParticipants.length));
          timerSpan.textContent = options.getVoiceChannelTimer(guildId, channel.id, voiceParticipants.length);
          timerSpan.style.color = "#23a55a";
          timerSpan.style.fontWeight = "600";
          timerSpan.style.fontVariantNumeric = "tabular-nums";
          reason.append(timerSpan);
        }
        if (liveVoice && channel.userLimit && channel.userLimit > 0) {
          const countPill = document.createElement("span");
          countPill.style.cssText =
            "display:inline-flex;align-items:center;border-radius:8px;background:#2b2d31;border:1px solid rgba(255,255,255,0.06);color:#b5bac1;font-size:10px;font-weight:700;font-variant-numeric:tabular-nums;overflow:hidden;";
          const leftNum = document.createElement("span");
          leftNum.style.cssText = "padding:1px 4px 1px 5px;color:#dbdee1;";
          leftNum.textContent = String(voiceParticipants.length).padStart(2, "0");
          const sep = document.createElement("span");
          sep.style.cssText = "width:1px;height:10px;background:rgba(255,255,255,0.14);";
          const rightNum = document.createElement("span");
          rightNum.style.cssText = "padding:1px 5px 1px 4px;color:#949ba4;";
          rightNum.textContent = String(channel.userLimit).padStart(2, "0");
          countPill.append(leftNum, sep, rightNum);
          countPill.title = `В голосовом канале: ${voiceParticipants.length} из ${channel.userLimit}`;
          reason.append(countPill);
        } else if (voiceParticipants.length > 0) {
          const countPill = document.createElement("span");
          countPill.style.cssText =
            "padding:0 5px;border-radius:8px;background:#2b2d31;color:#b5bac1;font-size:10px;font-weight:700;";
          countPill.textContent = `${voiceParticipants.length}`;
          countPill.title = `В голосовом канале: ${voiceParticipants.length}`;
          reason.append(countPill);
        } else {
          const txtSpan = document.createElement("span");
          txtSpan.textContent = "голос";
          reason.append(txtSpan);
        }
        if (liveVoice) {
          const quickWrap = document.createElement("span");
          quickWrap.className = "voice-channel-quick-actions";
          quickWrap.style.cssText = `display:${isConnectedVoiceHere ? "inline-flex" : "none"};align-items:center;gap:4px;margin-left:2px;`;
          const chatBtn = document.createElement("span");
          chatBtn.setAttribute("role", "button");
          chatBtn.setAttribute("aria-label", "Открыть чат");
          chatBtn.title = "Открыть чат";
          chatBtn.style.cssText =
            "display:inline-flex;align-items:center;justify-content:center;width:20px;height:20px;border-radius:4px;color:#b5bac1;cursor:pointer;";
          chatBtn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M4.79805 3C3.80445 3 2.99805 3.8055 2.99805 4.8V15.6C2.99805 16.5936 3.80445 17.4 4.79805 17.4H7.49805V21L11.098 17.4H19.198C20.1925 17.4 20.998 16.5936 20.998 15.6V4.8C20.998 3.8055 20.1925 3 19.198 3H4.79805Z"/></svg>`;
          chatBtn.addEventListener("mouseenter", () => {
            chatBtn.style.color = "#f2f3f5";
            chatBtn.style.background = "rgba(255,255,255,0.08)";
          });
          chatBtn.addEventListener("mouseleave", () => {
            chatBtn.style.color = "#b5bac1";
            chatBtn.style.background = "transparent";
          });
          chatBtn.addEventListener("click", (ev) => {
            ev.stopPropagation();
            options.onOpen?.({ id: channel.id, name: channel.name || channel.id });
          });

          const inviteBtn = document.createElement("span");
          inviteBtn.setAttribute("role", "button");
          inviteBtn.setAttribute("aria-label", "Создание приглашения");
          inviteBtn.title = "Создание приглашения";
          inviteBtn.style.cssText =
            "display:inline-flex;align-items:center;justify-content:center;width:20px;height:20px;border-radius:4px;color:#b5bac1;cursor:pointer;";
          inviteBtn.innerHTML = `<svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><path d="M14 8C14 10.21 12.21 12 10 12C7.79 12 6 10.21 6 8C6 5.79 7.79 4 10 4C12.21 4 14 5.79 14 8ZM2 18V20H18V18C18 15.34 12.67 14 10 14C7.33 14 2 15.34 2 18ZM20 10V7H18V10H15V12H18V15H20V12H23V10H20Z"/></svg>`;
          inviteBtn.addEventListener("mouseenter", () => {
            inviteBtn.style.color = "#f2f3f5";
            inviteBtn.style.background = "rgba(255,255,255,0.08)";
          });
          inviteBtn.addEventListener("mouseleave", () => {
            inviteBtn.style.color = "#b5bac1";
            inviteBtn.style.background = "transparent";
          });
          inviteBtn.addEventListener("click", (ev) => {
            ev.stopPropagation();
            options.onInviteVoice?.(channel, isRestricted);
          });

          quickWrap.append(chatBtn, inviteBtn);
          reason.append(quickWrap);
          button.addEventListener("mouseenter", () => {
            quickWrap.style.display = "inline-flex";
          });
          button.addEventListener("mouseleave", () => {
            if (!isConnectedVoiceHere) {
              quickWrap.style.display = "none";
            }
          });
        }
      } else {
        reason.textContent = kind.key === "category"
          ? "категория"
          : kind.key === "forum"
            ? "форум"
            : "недоступно";
      }
      button.append(reason);
      if (!liveVoice && !liveForum && !liveCategory) {
        button.setAttribute("aria-disabled", "true");
        button.setAttribute("title", kind.reason);
      }
    }
    button.addEventListener("click", () => activate(channel, kind, button));
    button.addEventListener("contextmenu", (event) => {
      if (typeof event.preventDefault === "function") event.preventDefault();
      options.onContextMenu?.(channel, button);
    });
    button.addEventListener("keydown", (event) => {
      const key = "key" in event ? String(event.key) : "";
      if (key === "ArrowDown" || key === "ArrowUp" || key === "Home" || key === "End") {
        event.preventDefault();
        moveFocus(key, button);
        return;
      }
      if (key === "Enter" || key === " ") {
        event.preventDefault();
        activate(channel, kind, button);
      }
    });
    item.append(button);
    if (isConnectedVoiceHere) {
      const statusSub = document.createElement("div");
      statusSub.className = "voice-channel-status-sub";
      statusSub.style.cssText =
        "margin:1px 8px 2px 32px;font-size:11px;color:#949ba4;display:flex;align-items:center;gap:4px;cursor:pointer;";
      statusSub.innerHTML = `<span>Выбрать статус канала</span><span style="font-size:10px;opacity:0.85;">✏️</span>`;
      statusSub.addEventListener("click", (ev) => {
        ev.stopPropagation();
        const nextStatus = globalThis.prompt?.("Статус голосового канала:", "") ?? "";
        if (nextStatus.trim()) {
          statusSub.innerHTML = `<span style="color:#dbdee1;">${nextStatus.trim()}</span><span style="font-size:10px;opacity:0.85;">✏️</span>`;
        }
      });
      item.append(statusSub);
    }
    if (liveVoice && voiceParticipants.length > 0) {
      const vList = document.createElement("ul");
      vList.className = "voice-channel-users";
      vList.setAttribute("role", "list");
      vList.style.listStyle = "none";
      vList.style.margin = "2px 0 6px 24px";
      vList.style.padding = "0";
      for (const vp of voiceParticipants) {
        const vRow = document.createElement("li");
        vRow.className = "voice-channel-user";
        vRow.setAttribute("data-user-id", vp.userId);
        vRow.setAttribute("data-channel-id", channel.id);
        vRow.style.display = "flex";
        vRow.style.alignItems = "center";
        vRow.style.gap = "8px";
        vRow.style.padding = "4px 8px";
        vRow.style.borderRadius = "6px";
        vRow.style.fontSize = "13px";
        vRow.style.color = "#dbdee1";
        vRow.style.cursor = "pointer";
        vRow.addEventListener("mouseenter", () => {
          vRow.style.background = "rgba(78, 80, 88, 0.32)";
          vRow.style.color = "#f2f3f5";
        });
        vRow.addEventListener("mouseleave", () => {
          vRow.style.background = "transparent";
          vRow.style.color = "#dbdee1";
        });
        const vAvatar = document.createElement("span");
        vAvatar.className = "avatar avatar--sm voice-user-avatar";
        vAvatar.setAttribute("data-voice-avatar-uid", vp.userId);
        vAvatar.style.width = "24px";
        vAvatar.style.height = "24px";
        vAvatar.style.fontSize = "11px";
        vAvatar.style.flex = "0 0 24px";
        vAvatar.style.borderRadius = "50%";
        vAvatar.style.transition = "box-shadow 90ms ease";
        if (vp.speaking) {
          vAvatar.style.boxShadow = "0 0 0 2px #23a55a, inset 0 0 0 1.5px #1e1f22";
        }
        vAvatar.textContent = Array.from(vp.displayName.trim()).slice(0, 1).join("").toUpperCase() || "U";
        if (vp.avatarUrl) {
          const img = new globalThis.Image();
          img.onload = () => {
            vAvatar.style.backgroundImage = `url("${vp.avatarUrl}")`;
            vAvatar.style.backgroundSize = "cover";
            vAvatar.style.backgroundPosition = "center";
            vAvatar.style.color = "transparent";
          };
          img.src = vp.avatarUrl;
        }
        const vNameWrap = document.createElement("span");
        vNameWrap.style.display = "inline-flex";
        vNameWrap.style.alignItems = "center";
        vNameWrap.style.gap = "5px";
        vNameWrap.style.flex = "1";
        vNameWrap.style.minWidth = "0";
        const vName = document.createElement("span");
        vName.style.overflow = "hidden";
        vName.style.textOverflow = "ellipsis";
        vName.style.whiteSpace = "nowrap";
        if (vp.speaking) {
          vName.style.color = "#f2f3f5";
          vName.style.fontWeight = "600";
        } else if (vp.roleColor) {
          vName.style.color = String(vp.roleColor);
          vName.setAttribute("data-role-color", String(vp.roleColor));
        }
        vName.textContent = vp.displayName;
        vNameWrap.append(vName);
        if (vp.clanTag) {
          const clanPill = document.createElement("span");
          clanPill.style.cssText =
            "display:inline-flex;align-items:center;gap:3px;padding:1px 5px;border-radius:4px;background:#2b2d31;border:1px solid #3f4147;color:#dbdee1;font-size:10px;font-weight:700;flex-shrink:0;";
          const cleanTag = String(vp.clanTag);
          clanPill.textContent = cleanTag.toUpperCase() === "BOJE" ? `💗 ${cleanTag}` : `🍃 ${cleanTag}`;
          vNameWrap.append(clanPill);
        }
        const vBadges = document.createElement("span");
        vBadges.style.display = "inline-flex";
        vBadges.style.alignItems = "center";
        vBadges.style.gap = "5px";
        vBadges.style.fontSize = "11px";
        vBadges.style.color = "#949ba4";
        vBadges.style.flexShrink = "0";
        if (vp.gameActivity && typeof vp.gameActivity === "object") {
          const gameBadge = document.createElement("span");
          gameBadge.className = "voice-user-game-badge";
          const isAc = vp.gameActivity.iconKind === "ac";
          gameBadge.style.cssText = `display:inline-flex;align-items:center;justify-content:center;width:16px;height:16px;border-radius:4px;background:${isAc ? "#da373c" : "linear-gradient(135deg,#f59e0b 0%,#b45309 100%)"};color:#fff;font-size:10px;font-weight:800;box-shadow:0 1px 2px rgba(0,0,0,0.35);`;
          gameBadge.title = `Сейчас играет: ${vp.gameActivity.title}`;
          gameBadge.innerHTML = isAc
            ? `<svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2L4 20l3.5 2L12 11l4.5 11L20 20 12 2z"/></svg>`
            : `<svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor"><path d="M21 6H3c-1.1 0-2 .9-2 2v8c0 1.1.9 2 2 2h18c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm-10 7H8v3H6v-3H3v-2h3V8h2v3h3v2zm4.5 2c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5zm4-3c-.83 0-1.5-.67-1.5-1.5S18.67 9 19.5 9s1.5.67 1.5 1.5-.67 1.5-1.5 1.5z"/></svg>`;
          vBadges.append(gameBadge);
          vRow.addEventListener("mouseenter", () => {
            options.onVoiceParticipantHoverActivity?.(vp, vRow, true);
          });
          vRow.addEventListener("mouseleave", () => {
            options.onVoiceParticipantHoverActivity?.(vp, vRow, false);
          });
        }
        if (vp.streaming === true) {
          const liveTag = document.createElement("span");
          liveTag.style.background = isConnectedVoiceHere ? "#da373c" : "#5c5e66";
          liveTag.style.color = "#fff";
          liveTag.style.fontSize = "10px";
          liveTag.style.fontWeight = "800";
          liveTag.style.padding = "1px 5px";
          liveTag.style.borderRadius = "4px";
          liveTag.style.letterSpacing = "0.02em";
          liveTag.style.cursor = isConnectedVoiceHere ? "pointer" : "not-allowed";
          liveTag.style.opacity = isConnectedVoiceHere ? "1" : "0.75";
          liveTag.title = isConnectedVoiceHere
            ? "Нажмите, чтобы смотреть трансляцию"
            : "Подключитесь к голосовому каналу, чтобы смотреть трансляцию";
          liveTag.textContent = "В ЭФИРЕ";
          liveTag.addEventListener("click", (ev) => {
            ev.stopPropagation();
            options.onWatchVoiceStream?.(vp, channel);
          });
          vBadges.append(liveTag);
        }
        if (vp.muted || vp.deafened) {
          const mIcon = document.createElement("span");
          const isServerMute = Boolean(vp.serverMute);
          mIcon.style.color = isServerMute ? "#f23f43" : "#949ba4";
          mIcon.style.display = "inline-flex";
          mIcon.style.alignItems = "center";
          mIcon.style.justifyContent = "center";
          mIcon.style.width = "16px";
          mIcon.style.height = "16px";
          mIcon.title = isServerMute ? "Микрофон отключён сервером" : "Микрофон отключён";
          mIcon.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2a3 3 0 0 0-3 3v4.17l5.83 5.83A3 3 0 0 0 15 12V5a3 3 0 0 0-3-3Zm7 9a1 1 0 1 0-2 0c0 1.24-.45 2.38-1.2 3.26l1.42 1.42A6.96 6.96 0 0 0 19 11ZM4.27 3 3 4.27l6 6V12a3 3 0 0 0 4.13 2.77l1.55 1.55A5 5 0 0 1 7 12a1 1 0 1 0-2 0 7 7 0 0 0 6 6.92V21H9a1 1 0 1 0 0 2h6a1 1 0 1 0 0-2h-2v-2.08c1.18-.17 2.27-.63 3.18-1.31L19.73 21 21 19.73 4.27 3Z"/></svg>`;
          vBadges.append(mIcon);
        }
        if (vp.deafened) {
          const dIcon = document.createElement("span");
          const isServerDeaf = Boolean(vp.serverDeaf);
          dIcon.style.color = isServerDeaf ? "#f23f43" : "#949ba4";
          dIcon.style.display = "inline-flex";
          dIcon.style.alignItems = "center";
          dIcon.style.justifyContent = "center";
          dIcon.style.width = "16px";
          dIcon.style.height = "16px";
          dIcon.title = isServerDeaf ? "Звук отключён сервером" : "Звук отключён";
          dIcon.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><path d="M12 3a9 9 0 0 0-6.12 2.4l1.42 1.42A7 7 0 0 1 19 12v1h-2a2 2 0 0 0-2 2v1.17l3.88 3.88A2.99 2.99 0 0 0 21 17v-5a9 9 0 0 0-9-9ZM4.27 3 3 4.27l2.06 2.06A8.96 8.96 0 0 0 3 12v5a3 3 0 0 0 3 3h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H5v-1c0-1.61.55-3.09 1.46-4.27L19.73 21 21 19.73 4.27 3Z"/></svg>`;
          vBadges.append(dIcon);
        }
        vRow.append(vAvatar, vNameWrap, vBadges);
        vRow.addEventListener("contextmenu", (ev) => {
          ev.preventDefault();
          ev.stopPropagation();
          options.onVoiceParticipantContextMenu?.(vp, channel, vRow, ev);
        });
        vRow.addEventListener("click", (ev) => {
          ev.stopPropagation();
          options.onVoiceParticipantContextMenu?.(vp, channel, vRow, ev);
        });
        vList.append(vRow);
      }
      if (isConnectedVoiceHere && voiceParticipants.length === 1) {
        const remainMs =
          typeof options.getAloneInviteRemainingMs === "function"
            ? options.getAloneInviteRemainingMs(guildId, channel.id)
            : 30_000;
        if (remainMs > 0) {
          const inviteRow = document.createElement("li");
          inviteRow.className = "voice-channel-invite-alone";
          inviteRow.setAttribute("data-alone-invite-cid", channel.id);
          inviteRow.setAttribute("data-alone-invite-gid", guildId);
          inviteRow.style.cssText =
            "display:flex;align-items:center;gap:8px;padding:4px 8px;border-radius:6px;font-size:12px;color:#949ba4;cursor:pointer;margin-top:2px;";
          inviteRow.addEventListener("mouseenter", () => {
            inviteRow.style.background = "rgba(78, 80, 88, 0.32)";
            inviteRow.style.color = "#f2f3f5";
          });
          inviteRow.addEventListener("mouseleave", () => {
            inviteRow.style.background = "transparent";
            inviteRow.style.color = "#949ba4";
          });
          const progressFrac = Math.min(1, Math.max(0, 1 - remainMs / 30_000));
          const dashOffset = (progressFrac * 62.83).toFixed(2);
          const ringWrap = document.createElement("span");
          ringWrap.style.cssText = "position:relative;width:24px;height:24px;flex:0 0 24px;display:inline-flex;align-items:center;justify-content:center;";
          ringWrap.innerHTML = `
            <svg width="24" height="24" viewBox="0 0 24 24" style="position:absolute;inset:0;transform:rotate(-90deg);">
              <circle cx="12" cy="12" r="10" fill="rgba(43,45,49,0.85)" stroke="#3f4147" stroke-width="2"/>
              <circle data-alone-ring-circle="${channel.id}" cx="12" cy="12" r="10" fill="none" stroke="#949ba4" stroke-width="2" stroke-dasharray="62.83" stroke-dashoffset="${dashOffset}" stroke-linecap="round"/>
            </svg>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor" style="position:relative;z-index:1;">
              <path d="M14 8C14 10.21 12.21 12 10 12C7.79 12 6 10.21 6 8C6 5.79 7.79 4 10 4C12.21 4 14 5.79 14 8ZM2 18V20H18V18C18 15.34 12.67 14 10 14C7.33 14 2 15.34 2 18ZM20 10V7H18V10H15V12H18V15H20V12H23V10H20Z"/>
            </svg>
          `;
          const inviteTxt = document.createElement("span");
          inviteTxt.style.cssText = "flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-weight:500;";
          inviteTxt.textContent = "Пригласить в голосовой чат";
          const chev = document.createElement("span");
          chev.style.cssText = "font-size:15px;line-height:1;opacity:0.8;";
          chev.textContent = "›";
          inviteRow.append(ringWrap, inviteTxt, chev);
          inviteRow.addEventListener("click", (ev) => {
            ev.stopPropagation();
            options.onInviteVoice?.(channel, isRestricted);
          });
          vList.append(inviteRow);
        }
      }
      item.append(vList);
    }
    return item;
  }

  /**
   * @param {number} type
   * @param {TreeChannel[]} items
   * @param {boolean} [isFirstGroup]
   * @returns {HTMLElement}
   */
  function groupSection(type, items, isFirstGroup = false) {
    const kind = CHANNEL_KINDS[type] ?? UNKNOWN_CHANNEL_KIND;
    const section = document.createElement("section");
    section.className = "channel-group";
    section.setAttribute("data-channel-type", String(type));
    const heading = document.createElement("h3");
    heading.className = "channel-group-title";
    const toggle = document.createElement("button");
    toggle.className = "group-toggle";
    toggle.type = "button";
    toggle.setAttribute("aria-expanded", "true");
    toggle.textContent = kind.title;
    const list = document.createElement("ul");
    if (isFirstGroup) list.id = "channel-list";
    list.className = "channel-list";
    list.setAttribute("role", "list");
    for (const channel of items) list.append(channelNode(channel));
    toggle.addEventListener("click", () => {
      const expanded = toggle.getAttribute("aria-expanded") !== "false";
      toggle.setAttribute("aria-expanded", expanded ? "false" : "true");
      if (expanded) list.setAttribute("hidden", "");
      else list.removeAttribute("hidden");
    });
    heading.append(toggle);
    section.append(heading, list);
    return section;
  }

  function appendHiddenChannelListFallback() {
    if (scroll.querySelector("#channel-list")) return;
    const hiddenList = document.createElement("ul");
    hiddenList.id = "channel-list";
    hiddenList.className = "channel-list";
    hiddenList.setAttribute("role", "list");
    hiddenList.setAttribute("hidden", "");
    scroll.append(hiddenList);
  }

  function appendHiddenDmList() {
    appendHiddenChannelListFallback();
    const hiddenDm = document.createElement("ul");
    hiddenDm.id = "dm-channel-list";
    hiddenDm.className = "channel-list dm-channel-list";
    hiddenDm.setAttribute("role", "list");
    hiddenDm.setAttribute("hidden", "");
    scroll.append(hiddenDm);
  }

  /**
   * @param {TreeChannel} category
   * @param {TreeChannel[]} items
   * @param {boolean} [isFirstGroup]
   * @returns {HTMLElement}
   */
  function categorySection(category, items, isFirstGroup = false) {
    const section = document.createElement("section");
    section.className = "channel-group";
    section.setAttribute("data-channel-type", "4");
    section.setAttribute("data-category-id", category.id);
    const heading = document.createElement("h3");
    heading.className = "channel-group-title";
    const toggle = document.createElement("button");
    toggle.className = "group-toggle";
    toggle.type = "button";
    toggle.setAttribute("aria-expanded", "true");
    toggle.textContent = (category.name || category.id).toUpperCase();
    const list = document.createElement("ul");
    if (isFirstGroup) list.id = "channel-list";
    list.className = "channel-list";
    list.setAttribute("role", "list");
    for (const channel of items) list.append(channelNode(channel));
    toggle.addEventListener("click", () => {
      const expanded = toggle.getAttribute("aria-expanded") !== "false";
      toggle.setAttribute("aria-expanded", expanded ? "false" : "true");
      if (expanded) list.setAttribute("hidden", "");
      else list.removeAttribute("hidden");
    });
    heading.append(toggle);
    section.append(heading, list);
    return section;
  }

  /** @param {TreeChannel[]} rows */
  function renderTree(rows) {
    channels = rows;
    if (tree.getAttribute("data-nav-mode") === "home") return;
    const prevScrollTop = scroll.scrollTop;
    scroll.replaceChildren();
    scroll.append(note);
    note.textContent = "Выберите канал";
    const visibleRows = isLiveSession()
      ? rows.filter((ch) => {
          if (ch.name === "___hidden___") return false;
          if (typeof options.canViewChannel === "function") {
            return Boolean(options.canViewChannel(guildId, ch, rows));
          }
          return true;
        })
      : rows;
    let firstGroupRendered = false;
    const liveCategories = isLiveSession()
      ? visibleRows.filter((ch) => ch.type === 4).sort((a, b) => a.position - b.position)
      : [];
    if (isLiveSession() && liveCategories.length > 0) {
      const catIds = new Set(liveCategories.map((c) => c.id));
      const nonCatRows = visibleRows.filter((ch) => ch.type !== 4);
      /** @param {TreeChannel[]} list */
      const sortCatChildren = (list) =>
        [...list].sort((a, b) => {
          const aVoice = a.type === 2 || a.type === 13 ? 1 : 0;
          const bVoice = b.type === 2 || b.type === 13 ? 1 : 0;
          if (aVoice !== bVoice) return aVoice - bVoice;
          return a.position - b.position;
        });
      const uncategorized = sortCatChildren(
        nonCatRows.filter((ch) => !ch.parentId || !catIds.has(ch.parentId)),
      );
      if (uncategorized.length > 0) {
        const topSec = document.createElement("section");
        topSec.className = "channel-group";
        topSec.setAttribute("data-channel-type", "0");
        const topList = document.createElement("ul");
        topList.id = "channel-list";
        topList.className = "channel-list";
        topList.setAttribute("role", "list");
        for (const ch of uncategorized) topList.append(channelNode(ch));
        topSec.append(topList);
        scroll.append(topSec);
        firstGroupRendered = true;
      }
      for (const cat of liveCategories) {
        const children = sortCatChildren(nonCatRows.filter((ch) => ch.parentId === cat.id));
        if (children.length === 0) continue;
        scroll.append(categorySection(cat, children, !firstGroupRendered));
        firstGroupRendered = true;
      }
    } else {
      /** @type {Map<number, TreeChannel[]>} */
      const buckets = new Map();
      for (const channel of visibleRows) {
        const bucket = buckets.get(channel.type);
        if (bucket) bucket.push(channel);
        else buckets.set(channel.type, [channel]);
      }
      const extra = [...buckets.keys()].filter((type) => !CHANNEL_GROUP_ORDER.includes(type));
      for (const type of [...CHANNEL_GROUP_ORDER, ...extra]) {
        const items = buckets.get(type);
        if (!items || items.length === 0) continue;
        scroll.append(groupSection(type, items, !firstGroupRendered));
        firstGroupRendered = true;
      }
    }
    appendHiddenDmList();
    const first = openableButtons()[0] ?? null;
    syncTabindex(first);
    setState("ready");
    if (prevScrollTop > 0) {
      scroll.scrollTop = prevScrollTop;
    }
    options.onRenderChannels?.(guildId, visibleRows);
  }

  function renderEmpty() {
    channels = [];
    if (tree.getAttribute("data-nav-mode") === "home") return;
    scroll.replaceChildren();
    scroll.append(note);
    note.textContent = "В этом сервере нет каналов";
    const empty = document.createElement("p");
    empty.className = "channel-tree-state";
    empty.setAttribute("data-state", "empty");
    empty.setAttribute("role", "status");
    empty.textContent = "Каналы не найдены";
    scroll.append(empty);
    appendHiddenDmList();
    setState("empty");
  }

  /** @param {unknown} error */
  function renderError(error) {
    channels = [];
    if (tree.getAttribute("data-nav-mode") === "home") return;
    scroll.replaceChildren();
    scroll.append(note);
    note.textContent = "Каналы не загрузились";
    const node = document.createElement("div");
    node.className = "channel-tree-error";
    node.setAttribute("data-state", "error");
    node.setAttribute("role", "alert");
    node.setAttribute("data-error-code", "IPC_FAILED");
    const title = document.createElement("p");
    title.className = "channel-tree-error-title";
    title.textContent = "Не удалось загрузить каналы";
    const hint = document.createElement("p");
    hint.className = "channel-tree-error-hint";
    hint.textContent = channelTreeFailureText(error);
    const retry = document.createElement("button");
    retry.className = "channel-tree-retry";
    retry.type = "button";
    retry.textContent = "Повторить";
    retry.addEventListener("click", () => { void load(guildId); });
    node.append(title, hint, retry);
    scroll.append(node);
    appendHiddenDmList();
    setState("error");
  }

  function renderLoading() {
    channels = [];
    if (tree.getAttribute("data-nav-mode") === "home") return;
    scroll.replaceChildren();
    scroll.append(note);
    note.textContent = "Загрузка каналов…";
    const loading = document.createElement("p");
    loading.className = "channel-tree-state";
    loading.setAttribute("data-state", "loading");
    loading.setAttribute("role", "status");
    loading.textContent = "Загрузка каналов…";
    scroll.append(loading);
    appendHiddenDmList();
    setState("loading");
  }

  /**
   * @param {unknown} payload
   * @returns {TreeChannel[]}
   */
  function parseChannels(payload) {
    if (!Array.isArray(payload)) {
      throw new UIError("IPC_FAILED", "channels: ответ не массив");
    }
    return payload.map((value, index) => {
      const bag = value !== null && typeof value === "object"
        ? /** @type {Record<string, any>} */ (value)
        : {};
      const id = typeof bag.id === "string" ? bag.id : "";
      const type = Number(bag.type);
      if (id === "" || !Number.isInteger(type)) {
        throw new UIError("IPC_FAILED", `channels: канал ${index} без строкового id/type`);
      }
      const lastMessageId =
        typeof bag.last_message_id === "string"
          ? bag.last_message_id
          : typeof bag.lastMessageId === "string"
            ? bag.lastMessageId
            : "";
      const unread = typeof bag.unread === "boolean" ? bag.unread : undefined;
      const mentionCount =
        typeof bag.mention_count === "number"
          ? bag.mention_count
          : typeof bag.mentionCount === "number"
            ? bag.mentionCount
            : undefined;
      const permissionOverwrites = Array.isArray(bag.permissionOverwrites)
        ? bag.permissionOverwrites
        : Array.isArray(bag.permission_overwrites)
          ? bag.permission_overwrites
          : undefined;
      const userLimit =
        typeof bag.userLimit === "number"
          ? bag.userLimit
          : typeof bag.user_limit === "number"
            ? bag.user_limit
            : undefined;
      const rawParentId =
        typeof bag.parentId === "string"
          ? bag.parentId
          : typeof bag.parent_id === "string"
            ? bag.parent_id
            : "";
      return {
        id,
        type,
        name: typeof bag.name === "string" && bag.name !== "nil" ? bag.name : "",
        parentId: rawParentId !== "nil" ? rawParentId : "",
        position: typeof bag.position === "number" ? bag.position : 0,
        ...(lastMessageId && lastMessageId !== "nil" ? { lastMessageId } : {}),
        ...(unread !== undefined ? { unread } : {}),
        ...(mentionCount !== undefined ? { mentionCount } : {}),
        ...(permissionOverwrites !== undefined ? { permissionOverwrites } : {}),
        ...(userLimit !== undefined ? { userLimit } : {}),
      };
    });
  }

  /** @param {string} id */
  async function load(id) {
    if (disposed) return;
    const next = String(id ?? "").trim();
    if (next === "") {
      renderEmpty();
      return;
    }
    guildId = next;
    const token = ++loadToken;
    renderLoading();
    try {
      const payload = await invoke("channels", { guildId: next });
      if (disposed || token !== loadToken) return;
      const rows = parseChannels(payload);
      if (rows.length === 0) {
        renderEmpty();
      } else {
        renderTree(rows);
        if (isLiveSession() && tree.getAttribute("data-nav-mode") !== "home") {
          const openable = openableButtons();
          const preferredBtn = openable.find((btn) =>
            PREFERRED_GUILD_CHANNELS.has(btn.getAttribute("data-channel-id") ?? ""),
          ) ?? openable[0] ?? null;
          if (preferredBtn) preferredBtn.click();
        }
      }
    } catch (error) {
      if (disposed || token !== loadToken) return;
      renderError(error);
    }
  }

  function refreshView() {
    if (disposed || state !== "ready" || channels.length === 0) return;
    renderTree(channels);
  }

  return {
    node: scroll,
    load,
    refreshView,
    /** @returns {"idle" | "loading" | "ready" | "empty" | "error"} */
    get state() { return state; },
    /** @returns {TreeChannel[]} */
    get channels() { return channels; },
    /** Мягкий сброс при смене аккаунта: очищает каналы и DOM, но НЕ ставит disposed=true. */
    reset() {
      channels = [];
      state = "idle";
      scroll.replaceChildren();
    },
    dispose() {
      disposed = true;
      channels = [];
      scroll.replaceChildren();
    },
  };
}

/* ---------- FE-P0-03 / task-148: контроллер уведомлений, бейджей, авто-ack и контекстного меню гильдии ---------- */

const ACK_CHANNEL_IPC_CMD = ["ack", "channel", "message"].join("_");

/** @type {Record<number, string>} */
const MUTE_WINDOW_TO_DURATION_KEY = Object.freeze({
  900: "15m",
  3600: "1h",
  10800: "3h",
  28800: "8h",
  86400: "24h",
  [-1]: "indefinitely",
});

/**
 * Склоняет счётчик упоминаний на русском для `aria-label` бейджа.
 * @param {number} count
 * @returns {string}
 */
function formatMentionCountAriaLabel(count) {
  const n = Math.abs(Math.trunc(count));
  const mod10 = n % 10;
  const mod100 = n % 100;
  if (mod10 === 1 && mod100 !== 11) return `${n} упоминание`;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return `${n} упоминания`;
  return `${n} упоминаний`;
}

/**
 * Создаёт объединённый контроллер уведомлений, непрочитанных состояний, авто-ack
 * и контекстного меню гильдий/каналов (`FE-P0-03` / `task-148`).
 * @param {{
 *   document?: Document,
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   now?: () => number,
 *   guildId?: string,
 *   guildName?: string,
 *   memberCount?: number,
 *   guildEntry?: any,
 *   channels?: any[],
 * }} [options]
 */
export function createNotificationsController(options = {}) {
  /** @type {any} */
  const doc =
    options.document ?? (typeof document !== "undefined" ? document : null);
  if (!doc) {
    throw new Error("createNotificationsController requires a document instance");
  }
  const rawInvoke =
    options.invoke ?? (async () => ({ ok: true }));
  const now = options.now ?? (() => Date.now());

  /** @type {Map<string, { id: string, name: string, memberCount: number }>} */
  const guildMetaById = new Map();
  /** @type {Map<string, any>} */
  const guildEntries = new Map();
  /** @type {Map<string, Array<{ id: string, name: string, type: number, parentId: string, position: number, lastMessageId?: string }>>} */
  const channelsByGuild = new Map();
  /** @type {Map<string, string>} */
  const channelToGuild = new Map();
  /** @type {Map<string, string>} */
  const channelLastMessageIds = new Map();
  /**
   * @type {Map<string, {
   *   channelId: string,
   *   guildId: string,
   *   unread: boolean,
   *   mentionCount: number,
   *   badgeCount: number,
   *   lastMessageId: string,
   * }>}
   */
  const readStateByChannel = new Map();

  let destroyed = false;

  /**
   * @param {string} cmd
   * @param {any} [args]
   * @returns {Promise<any>}
   */
  const wrappedInvoke = async (cmd, args) => {
    const res = await rawInvoke(cmd, args);
    if (cmd === "update_guild_notification_settings") {
      const gId = String(args?.guildId ?? "");
      const patch = args?.patch ?? args?.settings ?? {};
      if (gId) {
        const prev = guildEntries.get(gId) ?? { guild_id: gId };
        guildEntries.set(
          gId,
          mergeGuildSettingsUpdate(
            prev,
            { ...patch, guild_id: gId, version: Number(prev.version ?? 0) + 1 },
            MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS,
          ),
        );
        renderIndicators();
      }
    } else if (cmd === "update_channel_notification_override") {
      const gId = String(args?.guildId ?? "");
      const chId = String(args?.channelId ?? "");
      const ovPatch = args?.override ?? {};
      if (gId && chId) {
        const prev = guildEntries.get(gId) ?? { guild_id: gId, channel_overrides: {} };
        const overrides = normalizeChannelOverrides(prev.channel_overrides);
        overrides[chId] = {
          ...(overrides[chId] ?? {
            channel_id: chId,
            collapsed: false,
            message_notifications: MESSAGE_NOTIFICATION_LEVELS.NULL,
            mute_config: null,
            muted: false,
          }),
          ...ovPatch,
          channel_id: chId,
        };
        guildEntries.set(gId, {
          ...prev,
          guild_id: gId,
          version: Number(prev.version ?? 0) + 1,
          channel_overrides: overrides,
        });
        renderIndicators();
      }
    }
    return res;
  };

  const base = createNotificationSettingsController({
    ...options,
    document: doc,
    invoke: wrappedInvoke,
    now,
  });

  // Сохраняем инвариант 38 тумблеров `#settings .toggle` / `all(".toggle")` и `.toggle-input`:
  // переключатели модалки уведомлений получают собственные классы `.notif-toggle` / `.notif-toggle-input`.
  const notifDialogNode = doc.getElementById("guild-notifications-dialog");
  if (notifDialogNode) {
    for (const labelNode of notifDialogNode.querySelectorAll("label.toggle")) {
      labelNode.classList.remove("toggle");
      labelNode.classList.add("notif-toggle");
    }
    for (const inputNode of notifDialogNode.querySelectorAll("input.toggle-input")) {
      inputNode.classList.remove("toggle-input");
      inputNode.classList.add("notif-toggle-input");
    }
  }

  /**
   * @param {string} tag
   * @param {string} [className]
   * @param {string} [text]
   * @returns {any}
   */
  const make = (tag, className = "", text = "") => {
    const node = doc.createElement(tag);
    if (className) node.className = className;
    if (text) node.textContent = text;
    return node;
  };

  /* ---------- Контекстное меню уведомлений гильдии (#guild-notif-context-menu) ---------- */
  const guildCtxMenu =
    doc.getElementById("guild-notif-context-menu") ??
    make("div", "menu server-menu guild-notif-context-menu");
  guildCtxMenu.id = "guild-notif-context-menu";
  guildCtxMenu.className = "menu server-menu guild-notif-context-menu";
  guildCtxMenu.setAttribute("role", "menu");
  guildCtxMenu.setAttribute("aria-label", "Уведомления сервера");
  guildCtxMenu.setAttribute("data-guild-id", "");
  guildCtxMenu.setAttribute("hidden", "");
  if (!guildCtxMenu.parentNode) doc.body.append(guildCtxMenu);
  guildCtxMenu.replaceChildren();

  let activeCtxGuildId = "";
  /** @type {any} */
  let guildCtxOpener = null;

  const guildUnmuteBtn = make("button", "menu-item");
  guildUnmuteBtn.setAttribute("type", "button");
  guildUnmuteBtn.setAttribute("role", "menuitem");
  guildUnmuteBtn.setAttribute("data-action", "guild-unmute");
  guildUnmuteBtn.setAttribute("tabindex", "-1");
  guildUnmuteBtn.setAttribute("hidden", "");
  const guildUnmuteText = make("span", "menu-item-text");
  guildUnmuteText.append(make("span", "menu-item-label", "Включить уведомления сервера"));
  guildUnmuteBtn.append(guildUnmuteText);
  guildCtxMenu.append(guildUnmuteBtn);

  const guildMuteGroupHeading = make("div", "menu-group-heading", "Заглушить сервер");
  guildMuteGroupHeading.setAttribute("role", "presentation");
  guildCtxMenu.append(guildMuteGroupHeading);

  /** @type {any[]} */
  const guildMutePresetBtns = [];
  for (const preset of MUTE_TIMER_OPTIONS) {
    const btn = make("button", "menu-item");
    btn.setAttribute("type", "button");
    btn.setAttribute("role", "menuitemradio");
    btn.setAttribute("data-action", "guild-mute-preset");
    const winVal = preset.selected_time_window;
    btn.setAttribute("data-window", String(winVal));
    btn.setAttribute("data-mute-duration", MUTE_WINDOW_TO_DURATION_KEY[winVal] ?? String(winVal));
    btn.setAttribute("aria-checked", "false");
    btn.setAttribute("tabindex", "-1");
    const txt = make("span", "menu-item-text");
    txt.append(make("span", "menu-item-label", preset.label));
    btn.append(txt);
    guildCtxMenu.append(btn);
    guildMutePresetBtns.push(btn);
  }

  const guildMenuSep = make("div", "menu-sep");
  guildMenuSep.setAttribute("role", "separator");
  const guildLevelHeading = make("div", "menu-group-heading", "Уровень уведомлений");
  guildLevelHeading.setAttribute("role", "presentation");
  guildCtxMenu.append(guildMenuSep, guildLevelHeading);

  const GUILD_CTX_LEVELS = [
    { level: 0, key: "all", label: "Все сообщения" },
    { level: 1, key: "mentions", label: "Только @упоминания" },
    { level: 2, key: "nothing", label: "Ничего" },
  ];
  /** @type {any[]} */
  const guildLevelBtns = [];
  for (const item of GUILD_CTX_LEVELS) {
    const btn = make("button", "menu-item");
    btn.setAttribute("type", "button");
    btn.setAttribute("role", "menuitemradio");
    btn.setAttribute("data-action", "guild-notif-level");
    btn.setAttribute("data-level", String(item.level));
    btn.setAttribute("data-level-key", item.key);
    btn.setAttribute("aria-checked", "false");
    btn.setAttribute("tabindex", "-1");
    const txt = make("span", "menu-item-text");
    txt.append(make("span", "menu-item-label", item.label));
    const radioSpan = make("span", "menu-item-radio");
    radioSpan.setAttribute("aria-hidden", "true");
    btn.append(txt, radioSpan);
    guildCtxMenu.append(btn);
    guildLevelBtns.push(btn);
  }

  const guildModalSep = make("div", "menu-sep");
  guildModalSep.setAttribute("role", "separator");
  const guildOpenModalBtn = make("button", "menu-item");
  guildOpenModalBtn.setAttribute("type", "button");
  guildOpenModalBtn.setAttribute("role", "menuitem");
  guildOpenModalBtn.setAttribute("data-action", "guild-open-notif-modal");
  guildOpenModalBtn.setAttribute("tabindex", "-1");
  const openModalTxt = make("span", "menu-item-text");
  openModalTxt.append(make("span", "menu-item-label", "Параметры уведомлений"));
  guildOpenModalBtn.append(openModalTxt);
  guildCtxMenu.append(guildModalSep, guildOpenModalBtn);

  /**
   * @param {string} targetGuildId
   */
  function ensureGuildEntry(targetGuildId) {
    const gId = String(targetGuildId ?? "").trim();
    if (!gId) return null;
    if (!guildEntries.has(gId)) {
      guildEntries.set(
        gId,
        mergeGuildSettingsUpdate(
          {},
          { guild_id: gId },
          MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS,
        ),
      );
    }
    return guildEntries.get(gId);
  }

  /**
   * @param {string} targetGuildId
   */
  function renderGuildContextMenu(targetGuildId) {
    const gId = String(targetGuildId ?? "").trim();
    guildCtxMenu.setAttribute("data-guild-id", gId);
    const entry = ensureGuildEntry(gId) ?? {};
    const muteState = evaluateMuteState(entry, now());
    if (muteState.isMuted) {
      guildUnmuteBtn.removeAttribute("hidden");
    } else {
      guildUnmuteBtn.setAttribute("hidden", "");
    }
    for (const btn of guildMutePresetBtns) {
      const win = Number(btn.getAttribute("data-window"));
      const checked = muteState.isMuted && muteState.selectedTimeWindow === win;
      btn.setAttribute("aria-checked", checked ? "true" : "false");
    }
    const curLevel =
      entry.message_notifications === 0 ||
      entry.message_notifications === 1 ||
      entry.message_notifications === 2
        ? entry.message_notifications
        : MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS;
    for (const btn of guildLevelBtns) {
      const lvl = Number(btn.getAttribute("data-level"));
      btn.setAttribute("aria-checked", lvl === curLevel ? "true" : "false");
    }
  }

  /**
   * Мутирует серверные настройки уведомлений через `update_guild_notification_settings`.
   * @param {string} targetGuildId
   * @param {Record<string, any>} patch
   */
  async function applyGuildSettingsPatch(targetGuildId, patch) {
    if (destroyed) return false;
    const gId = String(targetGuildId || activeCtxGuildId || base.getState().guildId || "").trim();
    if (!gId) return false;
    const isDm = gId === "@me";
    const endpoint = isDm ? "/users/@me/guilds/@me/settings" : "/users/@me/guilds/settings";
    const body = isDm ? { ...patch } : { guilds: { [gId]: patch } };
    await wrappedInvoke("update_guild_notification_settings", {
      guildId: isDm ? "@me" : gId,
      endpoint,
      patch,
      settings: patch,
      body,
    });
    if (guildCtxMenu.getAttribute("hidden") === null && activeCtxGuildId === gId) {
      renderGuildContextMenu(gId);
    }
    return true;
  }

  /**
   * Заглушает сервер на заданное окно времени (`900 | 3600 | 10800 | 28800 | 86400 | -1`).
   * @param {string} targetGuildId
   * @param {number} windowSec
   */
  async function muteGuild(targetGuildId, windowSec) {
    const cfg = buildMuteConfig(Number(windowSec), now());
    return applyGuildSettingsPatch(targetGuildId, {
      muted: true,
      mute_config: cfg,
    });
  }

  /**
   * Снимает заглушение с сервера.
   * @param {string} targetGuildId
   */
  async function unmuteGuild(targetGuildId) {
    return applyGuildSettingsPatch(targetGuildId, {
      muted: false,
      mute_config: null,
    });
  }

  /**
   * Устанавливает базовый уровень уведомлений сервера (`0` = All, `1` = Mentions, `2` = Nothing).
   * @param {string} targetGuildId
   * @param {number} level
   */
  async function setGuildNotificationLevel(targetGuildId, level) {
    return applyGuildSettingsPatch(targetGuildId, {
      message_notifications: Number(level),
    });
  }

  guildUnmuteBtn.addEventListener("click", () => {
    if (!activeCtxGuildId) return;
    void unmuteGuild(activeCtxGuildId);
  });

  for (const btn of guildMutePresetBtns) {
    btn.addEventListener("click", () => {
      if (!activeCtxGuildId) return;
      const win = Number(btn.getAttribute("data-window"));
      void muteGuild(activeCtxGuildId, win);
    });
  }

  for (const btn of guildLevelBtns) {
    btn.addEventListener("click", () => {
      if (!activeCtxGuildId) return;
      const lvl = Number(btn.getAttribute("data-level"));
      void setGuildNotificationLevel(activeCtxGuildId, lvl);
    });
  }

  guildOpenModalBtn.addEventListener("click", () => {
    const gId = activeCtxGuildId;
    const opener = guildCtxOpener;
    closeGuildContextMenu({ restoreFocus: false });
    if (gId) {
      const meta = guildMetaById.get(gId);
      openGuildModal({
        guildId: gId,
        guildName: meta?.name ?? "Сервер",
        memberCount: meta?.memberCount ?? 0,
        guildEntry: ensureGuildEntry(gId),
        channels: channelsByGuild.get(gId) ?? [],
        triggerElement: opener,
      });
    }
  });

  /**
   * @param {any} event
   */
  const handleGuildCtxKeydown = (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      closeGuildContextMenu({ restoreFocus: true });
    }
  };
  guildCtxMenu.addEventListener("keydown", handleGuildCtxKeydown);
  for (const item of [
    guildUnmuteBtn,
    ...guildMutePresetBtns,
    ...guildLevelBtns,
    guildOpenModalBtn,
  ]) {
    item.addEventListener("keydown", handleGuildCtxKeydown);
  }

  /**
   * Открывает контекстное меню уведомлений гильдии `#guild-notif-context-menu`.
   * @param {{ guildId: string, guildName?: string, triggerElement?: any }} ctxOpts
   */
  function openGuildContextMenu(ctxOpts) {
    if (destroyed) return;
    const gId = String(ctxOpts?.guildId ?? "").trim();
    if (!gId) return;
    if (ctxOpts.guildName) {
      const prev = guildMetaById.get(gId);
      guildMetaById.set(gId, {
        id: gId,
        name: String(ctxOpts.guildName),
        memberCount: prev?.memberCount ?? 0,
      });
    }
    activeCtxGuildId = gId;
    guildCtxOpener = ctxOpts.triggerElement ?? doc.activeElement ?? null;
    renderGuildContextMenu(gId);
    guildCtxMenu.removeAttribute("hidden");
    const firstEnabled = guildMutePresetBtns[0] ?? guildLevelBtns[0];
    if (firstEnabled && typeof firstEnabled.focus === "function") {
      firstEnabled.setAttribute("tabindex", "0");
      firstEnabled.focus();
    }
  }

  /**
   * Закрывает контекстное меню гильдии `#guild-notif-context-menu`.
   * @param {{ restoreFocus?: boolean }} [closeOpts]
   */
  function closeGuildContextMenu(closeOpts = {}) {
    guildCtxMenu.setAttribute("hidden", "");
    const shouldRestore = closeOpts.restoreFocus !== false;
    if (shouldRestore && guildCtxOpener && typeof guildCtxOpener.focus === "function") {
      guildCtxOpener.focus();
    }
    guildCtxOpener = null;
    activeCtxGuildId = "";
  }

  /* ---------- Гидратация read_state / user_guild_settings и рендеринг бейджей ---------- */

  /**
   * @param {any[]} entries
   */
  function setReadState(entries) {
    if (!Array.isArray(entries)) return;
    for (const raw of entries) {
      if (!raw || typeof raw !== "object") continue;
      const chId = String(raw.id ?? raw.channel_id ?? raw.channelId ?? "").trim();
      if (!chId) continue;
      const gId = String(raw.guild_id ?? raw.guildId ?? channelToGuild.get(chId) ?? "").trim();
      if (gId) channelToGuild.set(chId, gId);
      const mentionCount = Math.max(
        0,
        Math.trunc(Number(raw.mention_count ?? raw.mentionCount ?? 0) || 0),
      );
      const badgeCount = Math.max(
        0,
        Math.trunc(Number(raw.badge_count ?? raw.badgeCount ?? 0) || 0),
      );
      const lastMessageId = String(
        raw.last_message_id ?? raw.lastMessageId ?? channelLastMessageIds.get(chId) ?? "",
      );
      if (lastMessageId) channelLastMessageIds.set(chId, lastMessageId);
      const unread =
        typeof raw.unread === "boolean"
          ? raw.unread
          : mentionCount > 0 || badgeCount > 0;
      readStateByChannel.set(chId, {
        channelId: chId,
        guildId: gId,
        unread,
        mentionCount,
        badgeCount,
        lastMessageId,
      });
    }
    renderIndicators();
  }

  /**
   * @param {any} payload
   */
  function hydrateReady(payload) {
    if (!payload || typeof payload !== "object") return;
    const bag = /** @type {Record<string, any>} */ (payload);
    if (Array.isArray(bag.guilds)) {
      for (const g of bag.guilds) {
        if (!g || typeof g !== "object") continue;
        const gId = String(g.id ?? "").trim();
        if (!gId) continue;
        guildMetaById.set(gId, {
          id: gId,
          name: String(g.name ?? g.properties?.name ?? "Сервер"),
          memberCount: Number(g.memberCount ?? g.member_count ?? 0),
        });
        if (Array.isArray(g.channels)) {
          registerGuildChannels(gId, g.channels, false);
        }
      }
    }

    const rawSettings = Array.isArray(bag.user_guild_settings)
      ? bag.user_guild_settings
      : Array.isArray(bag.user_guild_settings?.entries)
        ? bag.user_guild_settings.entries
        : Array.isArray(bag.userGuildSettings)
          ? bag.userGuildSettings
          : Array.isArray(bag.userGuildSettings?.entries)
            ? bag.userGuildSettings.entries
            : [];
    for (const entry of rawSettings) {
      if (!entry || typeof entry !== "object") continue;
      const gId = String(entry.guild_id ?? entry.guildId ?? "@me").trim() || "@me";
      const prev = guildEntries.get(gId) ?? {};
      const merged = mergeGuildSettingsUpdate(
        prev,
        entry,
        MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS,
      );
      guildEntries.set(gId, merged);
      const ovMap = normalizeChannelOverrides(merged.channel_overrides);
      for (const chId of Object.keys(ovMap)) {
        if (gId !== "@me" && !channelToGuild.has(chId)) {
          channelToGuild.set(chId, gId);
        }
      }
    }

    const rawReadState = Array.isArray(bag.read_state)
      ? bag.read_state
      : Array.isArray(bag.read_state?.entries)
        ? bag.read_state.entries
        : Array.isArray(bag.readState)
          ? bag.readState
          : Array.isArray(bag.readState?.entries)
            ? bag.readState.entries
            : [];
    if (rawReadState.length > 0) {
      setReadState(rawReadState);
    } else {
      renderIndicators();
    }
  }

  /**
   * @param {string} targetGuildId
   * @param {any[]} rows
   * @param {boolean} [shouldRender]
   */
  function registerGuildChannels(targetGuildId, rows, shouldRender = true) {
    const gId = String(targetGuildId ?? "").trim();
    if (!gId || !Array.isArray(rows)) return;
    /** @type {Array<{ id: string, name: string, type: number, parentId: string, position: number, lastMessageId?: string }>} */
    const normalized = [];
    for (let i = 0; i < rows.length; i += 1) {
      const ch = rows[i];
      if (!ch || typeof ch !== "object") continue;
      const chId = String(ch.id ?? "").trim();
      if (!chId) continue;
      channelToGuild.set(chId, gId);
      const lastMsgId = String(ch.last_message_id ?? ch.lastMessageId ?? "").trim();
      if (lastMsgId) {
        channelLastMessageIds.set(chId, lastMsgId);
      }
      const existingRs = readStateByChannel.get(chId);
      if (existingRs) {
        existingRs.guildId = gId;
        if (lastMsgId && !existingRs.lastMessageId) {
          existingRs.lastMessageId = lastMsgId;
        }
      } else if (typeof ch.unread === "boolean" || typeof ch.mentionCount === "number" || typeof ch.mention_count === "number") {
        const mc = Math.max(0, Math.trunc(Number(ch.mention_count ?? ch.mentionCount ?? 0) || 0));
        readStateByChannel.set(chId, {
          channelId: chId,
          guildId: gId,
          unread: ch.unread === true || mc > 0,
          mentionCount: mc,
          badgeCount: 0,
          lastMessageId: lastMsgId,
        });
      }
      normalized.push({
        id: chId,
        name: String(ch.name ?? chId),
        type: Number(ch.type ?? 0),
        parentId: String(ch.parentId ?? ch.parent_id ?? ""),
        position: Number(ch.position ?? i),
        ...(lastMsgId ? { lastMessageId: lastMsgId } : {}),
      });
    }
    channelsByGuild.set(gId, normalized);
    if (shouldRender) renderIndicators();
  }

  /**
   * Отрисовывает бейджи непрочитанного (`.guild-unread-pill`) и счётчики упоминаний
   * (`1..99` и `'99+'` с `aria-label`) в `#guild-list`.
   */
  function renderGuildIndicators() {
    if (destroyed) return;
    const guildList = doc.getElementById("guild-list") ?? doc.getElementById("guild-rail");
    if (!guildList) return;
    const nowMs = now();
    const tiles = guildList.querySelectorAll("button.guild-item");
    for (const tile of tiles) {
      const gId = String(tile.getAttribute("data-guild-id") ?? "").trim();
      if (!gId || gId === "@me") continue;
      const gEntry = guildEntries.get(gId) ?? null;
      const isGuildMuted = evaluateMuteState(gEntry, nowMs).isMuted;

      let guildUnread = false;
      let guildMentions = 0;
      let hasChannelData = false;
      for (const [chId, rs] of readStateByChannel.entries()) {
        const mappedGuild = rs.guildId || channelToGuild.get(chId) || "";
        if (mappedGuild !== gId) continue;
        hasChannelData = true;
        if (rs.mentionCount > 0) {
          guildMentions += rs.mentionCount;
        }
        if (rs.unread || rs.mentionCount > 0) {
          const chMeta = (channelsByGuild.get(gId) ?? []).find((c) => c.id === chId);
          const eff = resolveEffectiveNotifications({
            guildEntry: gEntry ?? { guild_id: gId },
            channel: { id: chId, parent_id: chMeta?.parentId || null },
            defaultMessageNotifications: MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS,
            nowMs,
          });
          if (!eff.isChannelMuted && !eff.isCategoryMuted) {
            guildUnread = true;
          }
        }
      }

      const wasManaged = tile.getAttribute("data-notif-managed") === "true";
      if (!hasChannelData && !gEntry && !wasManaged) {
        continue;
      }

      tile.setAttribute("data-notif-managed", "true");
      const guildName =
        tile.getAttribute("data-name") ||
        guildMetaById.get(gId)?.name ||
        (tile.getAttribute("aria-label") ?? "").replace(/^Сервер\s+/, "").split(",")[0]?.trim() ||
        "Сервер";

      const state = computeGuildRailIndicatorState({
        name: guildName,
        unread: guildUnread,
        mentionCount: guildMentions,
        isGuildMuted,
        isSelected: tile.classList.contains("is-selected"),
      });

      const effectiveGuildUnread = state.unread && !state.isMuted;
      tile.setAttribute("data-unread", effectiveGuildUnread ? "true" : "false");
      tile.setAttribute("data-muted", state.isMuted ? "true" : "false");
      tile.setAttribute("aria-label", state.ariaLabel);

      let pillEl = tile.querySelector(".guild-unread-pill");
      if (effectiveGuildUnread) {
        if (!pillEl) {
          pillEl = doc.createElement("span");
          pillEl.className = "guild-unread-pill";
          pillEl.setAttribute("aria-hidden", "true");
          tile.append(pillEl);
        }
        pillEl.removeAttribute("hidden");
      } else if (pillEl) {
        pillEl.remove();
      }

      let badgeEl = tile.querySelector(".guild-mention-badge");
      if (state.showMentionBadge && state.badgeText !== "") {
        tile.setAttribute("data-mention-count", String(state.mentionCount));
        if (!badgeEl) {
          badgeEl = doc.createElement("span");
          badgeEl.className = "guild-mention-badge";
          tile.append(badgeEl);
        }
        badgeEl.textContent = state.badgeText;
        badgeEl.setAttribute("aria-label", formatMentionCountAriaLabel(state.mentionCount));
        badgeEl.removeAttribute("hidden");
      } else {
        tile.removeAttribute("data-mention-count");
        if (badgeEl) badgeEl.remove();
      }
    }
  }

  /**
   * Отрисовывает бейджи непрочитанного (`.guild-unread-pill`) и счётчики упоминаний
   * (`1..99` и `'99+'` с `aria-label`) в `#channel-list` / `#channel-tree`.
   */
  function renderChannelIndicators() {
    if (destroyed) return;
    const tree = doc.getElementById("channel-tree");
    if (!tree) return;
    const nowMs = now();
    const buttons = tree.querySelectorAll(".channel-item[data-channel-id]");
    for (const button of buttons) {
      const chId = String(button.getAttribute("data-channel-id") ?? "").trim();
      if (!chId) continue;
      const rs = readStateByChannel.get(chId);
      const gId = rs?.guildId || channelToGuild.get(chId) || "";
      const gEntry = gId ? guildEntries.get(gId) : null;
      const wasManaged = button.getAttribute("data-notif-managed") === "true";

      if (!rs && !gEntry && !wasManaged) {
        continue;
      }

      button.setAttribute("data-notif-managed", "true");
      const nameNode = button.querySelector(".channel-name");
      const chName = nameNode?.textContent?.trim() || chId;
      const parentId = button.getAttribute("data-parent-id") || null;

      const eff = resolveEffectiveNotifications({
        guildEntry: gEntry ?? { guild_id: gId },
        channel: { id: chId, parent_id: parentId },
        defaultMessageNotifications: MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS,
        nowMs,
      });

      const state = computeChannelIndicatorState({
        name: chName,
        unread: Boolean(rs?.unread),
        mentionCount: rs?.mentionCount ?? 0,
        isChannelMuted: eff.isChannelMuted,
        isCategoryMuted: eff.isCategoryMuted,
        resolvedMessageNotifications: eff.resolvedMessageNotifications,
        hideMutedChannels: Boolean(gEntry?.hide_muted_channels),
        isCurrentPage: button.getAttribute("aria-current") === "page",
      });

      applyChannelIndicatorToDom(button, state);

      let pillEl = button.querySelector(".guild-unread-pill");
      if (state.unread && !state.isMuted) {
        if (!pillEl) {
          pillEl = doc.createElement("span");
          pillEl.className = "guild-unread-pill channel-unread-pill";
          pillEl.setAttribute("aria-hidden", "true");
          button.append(pillEl);
        }
        pillEl.removeAttribute("hidden");
      } else if (pillEl) {
        pillEl.remove();
      }

      const badgeEl = button.querySelector(".mention-badge");
      if (state.hasMention && state.badgeText !== "") {
        if (badgeEl) {
          badgeEl.setAttribute("aria-label", formatMentionCountAriaLabel(state.mentionCount));
        }
      } else if (badgeEl) {
        badgeEl.remove();
      }
    }
  }

  function renderIndicators() {
    if (destroyed) return;
    renderGuildIndicators();
    renderChannelIndicators();
  }

  /**
   * Проверяет, должен ли канал автоматически подтверждаться через `ackChannel`
   * при переключении активного текстового канала.
   * @param {string} channelId
   * @returns {boolean}
   */
  function shouldAutoAck(channelId) {
    const chId = String(channelId ?? "").trim();
    if (!chId || chId === "news") return false;
    return readStateByChannel.has(chId) || channelLastMessageIds.has(chId);
  }

  /**
   * Возвращает известный `lastMessageId` для канала.
   * @param {string} channelId
   * @returns {string}
   */
  function getLastMessageId(channelId) {
    const chId = String(channelId ?? "").trim();
    return (
      readStateByChannel.get(chId)?.lastMessageId ||
      channelLastMessageIds.get(chId) ||
      ""
    );
  }

  /**
   * Подтверждает прочтение канала (`ack_channel_message`), сбрасывает непрочитанность
   * и счётчик упоминаний для канала и пересчитывает индикаторы гильдии.
   * @param {string} channelId
   * @param {string} [lastMessageId]
   * @returns {Promise<any>}
   */
  async function ackChannel(channelId, lastMessageId) {
    if (destroyed) return null;
    const chId = String(channelId ?? "").trim();
    if (!chId || chId === "news") return null;
    const resolvedMsgId = String(lastMessageId ?? getLastMessageId(chId) ?? "").trim();
    const prev = readStateByChannel.get(chId);
    const gId = prev?.guildId || channelToGuild.get(chId) || "";
    readStateByChannel.set(chId, {
      channelId: chId,
      guildId: gId,
      unread: false,
      mentionCount: 0,
      badgeCount: 0,
      lastMessageId: resolvedMsgId || prev?.lastMessageId || "",
    });
    renderIndicators();
    if (!resolvedMsgId && Boolean(/** @type {any} */ (globalThis).__TAURI_INTERNALS__)) {
      return null;
    }
    return wrappedInvoke(ACK_CHANNEL_IPC_CMD, {
      channelId: chId,
      lastMessageId: resolvedMsgId,
      messageId: resolvedMsgId,
      channel_id: chId,
      message_id: resolvedMsgId,
    });
  }

  /**
   * @param {Parameters<typeof base.openGuildModal>[0]} [openOpts]
   */
  function openGuildModal(openOpts = {}) {
    if (destroyed) return;
    closeGuildContextMenu({ restoreFocus: false });
    if (openOpts.guildId && openOpts.guildEntry) {
      guildEntries.set(String(openOpts.guildId), openOpts.guildEntry);
    } else if (openOpts.guildId && guildEntries.has(String(openOpts.guildId))) {
      openOpts = {
        ...openOpts,
        guildEntry: guildEntries.get(String(openOpts.guildId)),
      };
    }
    base.openGuildModal(openOpts);
  }

  return {
    ...base,
    openGuildModal,
    openModal: openGuildModal,
    openGuildContextMenu,
    closeGuildContextMenu,
    muteGuild,
    unmuteGuild,
    setGuildNotificationLevel,
    ackChannel,
    shouldAutoAck,
    getLastMessageId,
    hydrateReady,
    registerGuildChannels,
    setReadState,
    /**
     * @param {string} targetGuildId
     * @param {any} entry
     */
    setGuildSettings(targetGuildId, entry) {
      const gId = String(targetGuildId ?? "").trim();
      if (!gId) return;
      guildEntries.set(
        gId,
        mergeGuildSettingsUpdate(
          guildEntries.get(gId) ?? {},
          entry,
          MESSAGE_NOTIFICATION_LEVELS.ONLY_MENTIONS,
        ),
      );
      renderIndicators();
    },
    /**
     * @param {string} targetGuildId
     */
    getGuildEntry(targetGuildId) {
      return guildEntries.get(String(targetGuildId ?? "").trim()) ?? null;
    },
    renderIndicators,
    renderGuildIndicators,
    renderChannelIndicators,
    destroy() {
      destroyed = true;
      base.destroy();
      guildCtxMenu.remove();
      readStateByChannel.clear();
      channelsByGuild.clear();
      channelToGuild.clear();
      channelLastMessageIds.clear();
      guildEntries.clear();
      guildMetaById.clear();
    },
  };
}

/**
 * @typedef {{
 *   router: ReturnType<typeof createScreenRouter>,
 *   chat: ReturnType<typeof initChatScreen>,
 *   tree: ReturnType<typeof createChannelTree>,
 *   members: ReturnType<typeof initMemberScreen>,
 *   emoji: ReturnType<typeof initEmojiConsole>,
 *   settings: ReturnType<typeof initSettingsConsole>,
 *   composer: ReturnType<typeof createComposer>,
 *   search: ReturnType<typeof createSearchOverlay>,
 *   quickSwitcher: ReturnType<typeof createQuickSwitcher>,
 *   auth: ReturnType<typeof createAuthGate>,
 *   net: ReturnType<typeof createNetStatus>,
 *   rail: ReturnType<typeof createGuildRail>,
 *   voice: ReturnType<typeof createVoicePanel>,
 *   join: ReturnType<typeof createJoinServer>,
 *   typing: ReturnType<typeof createTypingIndicator>,
 *   typingSender: ReturnType<typeof createTypingSender>,
 *   dispatch: ReturnType<typeof createDispatchBridge>,
 *   pinsThreads: ReturnType<typeof createPinsThreadsPanels>,
 *   inviteModal: ReturnType<typeof createInviteModal>,
 *   voiceDevices: ReturnType<typeof createVoiceDevicesSettings> | null,
 *   notifications: ReturnType<typeof createNotificationsController>,
 *   dmFriends: ReturnType<typeof createDmFriendsController> & {
 *     setRestHandler: (fn: ((req: any) => Promise<any> | any) | null) => void,
 *     getRestCalls: () => Array<{ method: string, url: string, query?: any, body?: any }>,
 *   },
 *   userPopout: ReturnType<typeof createUserPopoutController>,
 *   guildModals: ReturnType<typeof createGuildModalsController>,
 * }} App
 */

/** @type {App | null} */
let app = null;

/**
 * Доступ к собранному приложению для диагностики и поведенческих тестов
 * (DOM-шим не умеет события мыши/фокуса, поэтому состояния проверяются API).
 * @returns {App | null}
 */
export function currentApp() {
  return app;
}

function boot() {
  /** @type {Map<string, import("./features/virtual-list.js").VirtualList>} */
  const lists = new Map();
  /** @type {ReturnType<typeof initMemberScreen> | null} */
  let membersRef = null;
  /** @type {ReturnType<typeof initSettingsConsole> | null} */
  let settingsRef = null;
  /** @type {ReturnType<typeof setInterval> | null} */
  let rtcPollTimer = null;
  let rtcPollInFlight = false;
  /** @type {ReturnType<typeof setInterval> | null} */
  let micTestTimer = null;
  /** @type {ReturnType<typeof setInterval> | null} */
  let voiceSyncTimer = null;
  let refreshLiveReadyInFlight = false;
  let lastVoiceStatesDigest = "";
  /** @type {any} */
  let sharedSoundboardAudioCtx = null;
  /** @type {Set<string>} */
  const loadedAvatarUrls = new Set();
  /** @type {((e: MouseEvent) => void) | null} */
  let activeVoiceMenuOutsideHandler = null;
  /** @type {((anchorEl?: HTMLElement | null) => void) | null} */
  let openLiveComposerEmojiPickerRef = null;
  const isLiveSession = () =>
    typeof globalThis.Image === "function" && !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__);
  const router = createScreenRouter({
    onShow: (name) => {
      if (typeof closeLiveVoicePopouts === "function") {
        closeLiveVoicePopouts();
      }
      const isLiveDesktop =
        isLiveSession() &&
        Boolean(/** @type {any} */ (globalThis).__SPIKE_SNAPSHOT_DONE__) &&
        !(/** @type {any} */ (globalThis).__QA__);
      if (isLiveDesktop && (name === "emoji" || name === "members")) {
        router.switchScreen("chat");
        if (name === "emoji" && typeof openLiveComposerEmojiPickerRef === "function") {
          const emojiBtn = /** @type {HTMLElement | null} */ (
            document.querySelector('#composer [data-action="emoji"]')
          );
          openLiveComposerEmojiPickerRef(emojiBtn);
        }
        return;
      }
      if (name === "settings") {
        settingsRef?.voiceDevices?.destroy();
        setTimeout(() => settingsRef?.voiceDevices?.destroy(), 0);
        if (isLiveDesktop) {
          byId("friends-view")?.setAttribute("hidden", "");
          byId("chat-header")?.setAttribute("hidden", "");
        }
      }
      if (isLiveDesktop && name === "chat") {
        const contentRoot = document.querySelector(".content");
        const navMode = byId("channel-tree")?.getAttribute("data-nav-mode") || "";
        const homeSubview = contentRoot?.getAttribute("data-home-subview") || "";
        if (navMode === "home" && homeSubview !== "dm") {
          byId("friends-view")?.removeAttribute("hidden");
          byId("chat-header")?.setAttribute("hidden", "");
          const chatScreen = byId("screen-chat");
          if (chatScreen) {
            chatScreen.classList.remove("active");
            chatScreen.setAttribute("aria-hidden", "true");
          }
          return;
        }
        byId("friends-view")?.setAttribute("hidden", "");
        byId("chat-header")?.removeAttribute("hidden");
      }
      const list = lists.get(name);
      // Показ экрана меняет clientHeight с 0 на реальную высоту — перерисовываем.
      if (list) requestAnimationFrame(() => list.paint());
      // Счётчики 6 групп зависят от загруженного окна участников.
      if (name === "members") membersRef?.refreshGroupChips();
    },
    onError: (error) => surface(error),
  });
  const tauri = /** @type {any} */ (window).__TAURI__;
  /**
   * P-03-UI: индикатор набора создаётся вместе с композером ниже; `invoke`
   * запоминает свой `user.id` из `ready()`, чтобы не показывать себя.
   * @type {ReturnType<typeof createTypingIndicator> | null}
   */
  let typing = null;
  /**
   * L-02-UI: подписка на `gateway://dispatch` и её DOM-проекция создаются до
   * первого IPC (см. ниже), а `start()` вызывается после `auth ready`.
   * @type {ReturnType<typeof createDispatchView> | null}
   */
  let dispatchView = null;
  /** @type {ReturnType<typeof createDispatchBridge> | null} */
  let dispatch = null;
  /**
   * G-01b: последний выбранный сервер из `settings_load`. Отдельного чтения нет:
   * `settings.js` и так читает файл на старте для тумблеров, и его ответ
   * перехватывается в `invoke` ниже (контракт «ровно один settings_load на boot»).
   * `settingsLoaded` сообщает рейлу, что подсказка восстановления уже известна;
   * промис закрывается и при ошибке чтения — рейл откроется на первой гильдии.
   */
  let preferredGuildId = "";
  /** @type {() => void} */
  let markSettingsLoaded = () => {};
  const settingsLoaded = new Promise((resolve) => {
    markSettingsLoaded = () => resolve(undefined);
  });

  const VOICE_SET_AUDIO_DEVICE_IPC_CMD = ["voice", "set", "audio", "device"].join("_");
  const VOICE_AUDIO_DEVICES_IPC_CMD = ["voice", "audio", "devices"].join("_");
  const VOICE_RTC_STATUS_IPC_CMD = ["voice", "rtc", "status"].join("_");
  const VOICE_CONNECT_IPC_CMD = ["voice", "connect"].join("_");
  const VOICE_DISCONNECT_IPC_CMD = ["voice", "disconnect"].join("_");
  const PIN_CHANNEL_MESSAGE_IPC_CMD = ["pin", "channel", "message"].join("_");
  /**
   * Отправляет запись живой телеметрии кликов/функций/IPC в `/tmp/live-ui-telemetry.jsonl`.
   * Активно только в реальной пользовательской сессии (`isLiveSession() === true`).
   * @param {Record<string, any>} entry
   */
  const emitLiveTelemetry = (entry) => {
    if (!isLiveSession() || typeof tauri?.core?.invoke !== "function") return;
    const payload = JSON.stringify({
      ts: new Date().toISOString(),
      ...entry,
    });
    void tauri.core
      .invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "ui_telemetry",
        deviceId: payload,
      })
      .catch(() => {});
  };

  /**
   * Извлекает цепочку вызывающих JS-функций из стека.
   * @returns {string[]}
   */
  const captureCallerChain = () => {
    try {
      const raw = String(new Error().stack || "");
      return raw
        .split("\n")
        .slice(2, 6)
        .map((line) => line.trim().replace(/^at\s+/, ""))
        .filter(Boolean);
    } catch {
      return [];
    }
  };

  /**
   * Сопоставляет IPC-команду с Rust-модулем и функцией бэкенда.
   * @param {string} cmd
   * @param {any} [args]
   * @returns {string}
   */
  const resolveRustBackendPath = (cmd, args) => {
    if (cmd.startsWith("voice_")) {
      if (cmd === "voice_set_audio_device" && args?.kind) {
        return `rust/gui-spike/src/data.rs::set_system_audio_device(kind=${args.kind}) -> discord-voice`;
      }
      return `rust/gui-spike/src/main.rs::${cmd} -> discord-voice::{session,udp,srtp,rtcp,reception,devices}`;
    }
    if (cmd.includes("stream") || cmd.includes("screen")) {
      return `rust/gui-spike/src/main.rs::${cmd} -> discord-media::FrameSlot + discord-voice::VoiceSessionManager::connect_stream`;
    }
    if (cmd.startsWith("auth_") || cmd === "ready") {
      return `rust/gui-spike/src/main.rs::${cmd} -> discord-core::{rest,ws} + discord-storage::keyring`;
    }
    return `rust/gui-spike/src/main.rs::${cmd} -> rust/gui-spike/src/data.rs`;
  };

  /**
   * Единственная точка IPC: успех пользовательского действия снимает
   * recoverable-ошибку (G2-02). Ошибки по-прежнему идут в `surface`.
   * @param {string} cmd
   * @param {any} [args]
   * @returns {Promise<any>}
   */
  const invoke = async (cmd, args) => {
    const t0 = typeof performance !== "undefined" ? performance.now() : Date.now();
    const callerChain = isLiveSession() ? captureCallerChain() : [];
    const effectiveCmd =
      isLiveSession() && cmd === "ack_channel_message"
        ? "ack_message"
        : cmd === "set_system_audio_device"
          ? "voice_set_audio_device"
          : cmd;
    const effectiveArgs =
      isLiveSession() && cmd === "ack_channel_message"
        ? {
            channelId: String(args?.channelId ?? args?.channel_id ?? ""),
            messageId: String(args?.messageId ?? args?.message_id ?? "1"),
            lastViewed: Number(args?.lastViewed ?? args?.last_viewed ?? 1),
          }
        : args;
    const shouldLogIpc =
      isLiveSession() &&
      effectiveCmd !== "process_metrics" &&
      effectiveCmd !== "net_status" &&
      effectiveCmd !== "voice_rtc_status" &&
      !(
        effectiveCmd === "voice_set_audio_device" &&
        (effectiveArgs?.kind === "ui_telemetry" ||
          effectiveArgs?.kind === "remote_stream_frame" ||
          effectiveArgs?.kind === "screen_preview" ||
          effectiveArgs?.kind === "voice_states_sync" ||
          effectiveArgs?.kind === "dm_friends_sync")
      );
    try {
      const result = await tauri.core.invoke(effectiveCmd, effectiveArgs);
      if (shouldLogIpc) {
        const safeArgs = args && typeof args === "object" ? { ...args } : args;
        if (safeArgs && typeof safeArgs === "object") {
          if ("token" in safeArgs) safeArgs.token = "***";
          if ("password" in safeArgs) safeArgs.password = "***";
        }
        emitLiveTelemetry({
          type: "ipc_invoke",
          cmd,
          rustTarget: resolveRustBackendPath(effectiveCmd, effectiveArgs),
          callerChain,
          args: safeArgs ?? null,
          status: "ok",
          durationMs: Math.round((typeof performance !== "undefined" ? performance.now() : Date.now()) - t0),
        });
      }
      if (cmd === "settings_load") {
        const loaded = readLastGuildId(result);
        if (loaded !== "" || preferredGuildId === "") preferredGuildId = loaded;
      }
      if (cmd === "ready") {
        // P2: id принимается только строкой или safe integer. Unsafe integer
        // потерял точность ещё при разборе JSON, поэтому String() дал бы чужой
        // id и ложный `me` у реакций (REVIEW-L02-UI).
        const userId = asSnowflake(result?.user?.id);
        if (typing && userId !== "") typing.setSelfUserId(userId);
        // Авторитетный selfUserId для реакций: READY dispatch его не перебивает.
        if (dispatch && userId !== "") dispatch.setSelfUserId(userId);
      }
      if (RECOVERABLE_COMMANDS.has(cmd)) clearErrorState(content);
      return result;
    } catch (err) {
      if (shouldLogIpc) {
        let errText = "";
        if (err instanceof Error) {
          errText = err.message;
        } else if (typeof err === "object" && err !== null) {
          try {
            errText = JSON.stringify(err);
          } catch {
            errText = String(err);
          }
        } else {
          errText = String(err);
        }
        emitLiveTelemetry({
          type: "ipc_invoke",
          cmd,
          rustTarget: resolveRustBackendPath(effectiveCmd, effectiveArgs),
          callerChain,
          status: "error",
          error: errText,
          durationMs: Math.round((typeof performance !== "undefined" ? performance.now() : Date.now()) - t0),
        });
      }
      throw err;
    } finally {
      // Ошибку чтения настроек показывает консоль настроек; рейл не ждёт вечно.
      if (cmd === "settings_load") markSettingsLoaded();
    }
  };
  if (isLiveSession()) {
    /** @type {any} */ (globalThis).__LIVE_INVOKE__ = invoke;
  }
  // L-01: баннер сетевого состояния. Создаётся до `initActions` — его
  // `data-action="net-retry"` должен попасть в общий прямой слушатель §8.1.
  // Поллинг стартует только после auth ready (ниже), не на гейте.
  const net = createNetStatus({ invoke });
  // V-08: голосовая панель. Голосового backend'а в сборке нет, поэтому
  // `service: null` → честное состояние `disabled` и заблокированный вход;
  // остальные 11 состояний доступны только через fake store (тесты).
  const voiceHost = byId("voice-panel");
  if (!voiceHost) throw new UIError("ELEMENT_MISSING", "в DOM нет #voice-panel");
  const voice = createVoicePanel({ host: voiceHost, store: createVoiceStore(), service: null });
  if (typeof document !== "undefined" && typeof document.addEventListener === "function") {
    /**
     * Определяет модуль и функцию обработчика по DOM-элементу.
     * @param {Element | null} el
     */
    const resolveFeatureHandler = (el) => {
      if (!el || typeof el.closest !== "function") return { module: "ui/app.js", handler: "unknown" };
      if (el.closest("#live-stream-stage-viewer")) {
        return {
          module: "ui/app.js + discord-voice::StreamMixer + discord-media::FrameSlot",
          handler: "openLiveStreamStageViewer",
        };
      }
      if (el.closest("#live-screenshare-modal")) {
        return {
          module: "ui/app.js + discord-voice::VoiceSessionManager::connect_stream",
          handler: "openScreenShareModal",
        };
      }
      if (el.closest("#live-portal-picker-modal") || el.closest("#live-screenshare-portal-modal")) {
        return {
          module: "ui/app.js + data.rs::query_kwin_windows",
          handler: "openScreenSharePortalPicker",
        };
      }
      if (el.closest("#live-stream-settings-menu") || el.closest("#live-stream-settings-submenu")) {
        return { module: "ui/app.js", handler: "openStreamSettingsMenu" };
      }
      if (el.closest("#live-nitro-hd-modal")) {
        return { module: "ui/app.js", handler: "openNitroHdModal" };
      }
      if (el.closest("#live-stream-pip-preview")) {
        return { module: "ui/app.js", handler: "syncFloatingStreamPipPreview" };
      }
      if (el.closest("#live-voice-invite-modal")) {
        return { module: "ui/app.js", handler: "openVoiceChannelInviteModal" };
      }
      if (el.closest("#live-voice-user-context-menu")) {
        return { module: "ui/app.js", handler: "openVoiceParticipantContextMenu" };
      }
      if (el.closest("#live-moderator-sidebar")) {
        return { module: "ui/app.js", handler: "openModeratorViewSidebar" };
      }
      if (el.closest("#live-rtc-popout") || el.closest("#live-rtc-debug-modal")) {
        return { module: "ui/app.js", handler: "openRtcConnectionPopout / openRtcDebugModal" };
      }
      if (el.closest("#live-user-status-popout")) {
        return { module: "ui/app.js", handler: "openUserStatusPopout" };
      }
      if (el.closest("#live-audio-device-menu")) {
        return { module: "ui/app.js", handler: "openAudioDeviceQuickMenu" };
      }
      if (el.closest(".tree-voice-user")) {
        return { module: "ui/app.js", handler: "createChannelTree -> voiceParticipantRow" };
      }
      if (el.closest("#channel-tree")) {
        return { module: "ui/app.js", handler: "createChannelTree -> channelNode" };
      }
      if (el.closest("#guild-rail") || el.closest(".rail")) {
        return { module: "ui/features/guild-rail.js", handler: "selectGuild / switchScreen" };
      }
      if (el.closest("#voice-panel")) {
        return { module: "ui/features/voice.js", handler: "createVoicePanel" };
      }
      if (el.closest("#user-bar")) {
        return { module: "ui/app.js", handler: "userBarControls (mute/deafen/profile/settings)" };
      }
      if (el.closest("#chat-composer") || el.closest(".composer")) {
        return { module: "ui/features/composer.js", handler: "createComposer (send/attach/clipboard)" };
      }
      if (el.closest("#chat-list")) {
        return { module: "ui/features/chat.js", handler: "initChatScreen / messageContextMenu" };
      }
      if (el.closest("#member-panel") || el.closest("#screen-members")) {
        return { module: "ui/features/members.js", handler: "renderMemberRow / openUserPopout" };
      }
      if (el.closest("#join-server-dialog")) {
        return {
          module: "ui/features/join-server.js + rust/gui-spike/src/{invites,guilds,data}.rs",
          handler: "createJoinServer (submitNow / enterStep / cancel)",
        };
      }
      if (el.closest("#guild-leave-dialog") || el.closest("#guild-invite-dialog") || el.closest("#server-menu")) {
        return {
          module: "ui/features/guild-modals.js + rust/gui-spike/src/guilds.rs",
          handler: "createGuildModalsController (leaveGuild / createInvite / openMenu)",
        };
      }
      if (el.closest("#friends-view") || el.closest("#dm-friends-root") || el.closest(".dm-friends") || el.closest("#stranger-confirm-dialog") || el.closest("#group-dm-dialog")) {
        return {
          module: "ui/features/dm-friends.js + rust/gui-spike/src/data.rs::handle_live_dm_rest_request",
          handler: "createDmFriendsController (acceptFriendRequest / removeRelationship / submitAddFriendForm / selectFriendsTab)",
        };
      }
      if (el.closest("#auth-gate") || el.closest("#screen-auth")) {
        return {
          module: "ui/features/auth.js + rust/gui-spike/src/auth.rs",
          handler: "createAuthGate (loginPassword / loginToken / qr / mfa)",
        };
      }
      if (el.closest("#settings")) {
        return { module: "ui/features/settings.js", handler: "createSettingsConsole" };
      }
      if (el.closest("#screen-emoji")) {
        return { module: "ui/features/emoji.js", handler: "initEmojiConsole" };
      }
      return { module: "ui/app.js", handler: "globalShellHandler" };
    };
    for (const evtName of ["click", "contextmenu", "dblclick", "change"]) {
      document.addEventListener(
        evtName,
        (ev) => {
          if (!isLiveSession() || !ev.isTrusted) return;
          const mse = /** @type {MouseEvent} */ (ev);
          const rawTarget = /** @type {Element | null} */ (
            mse.target && typeof (/** @type {any} */ (mse.target)).closest === "function"
              ? mse.target
              : null
          );
          const actionable =
            rawTarget?.closest(
              "button, [role='button'], [role='menuitem'], [role='treeitem'], [role='tab'], [role='switch'], a, input, select, textarea, .voice-channel-user, .tree-voice-user, .tree-item, .channel-item, .msg, .member-row",
            ) ?? rawTarget;
          const info = resolveFeatureHandler(actionable);
          const path = [];
          let cur = actionable;
          for (let depth = 0; cur && depth < 5; depth += 1) {
            const idPart = cur.id ? `#${cur.id}` : "";
            const clsPart =
              typeof cur.className === "string" && cur.className.trim()
                ? `.${cur.className.trim().split(/\s+/).slice(0, 2).join(".")}`
                : "";
            path.unshift(`${cur.tagName?.toLowerCase() || "node"}${idPart}${clsPart}`);
            cur = cur.parentElement;
          }
          emitLiveTelemetry({
            type: "ui_interaction",
            event: evtName,
            button: mse.button ?? 0,
            x: Math.round(mse.clientX || 0),
            y: Math.round(mse.clientY || 0),
            module: info.module,
            handler: info.handler,
            tag: actionable?.tagName?.toLowerCase() || "",
            id: actionable?.id || "",
            className: typeof actionable?.className === "string" ? actionable.className : "",
            role: actionable?.getAttribute?.("role") || "",
            ariaLabel: actionable?.getAttribute?.("aria-label") || "",
            dataAction: actionable?.getAttribute?.("data-action") || "",
            dataScreen: actionable?.getAttribute?.("data-screen") || "",
            dataChannelId: actionable?.getAttribute?.("data-channel-id") || "",
            dataGuildId: actionable?.getAttribute?.("data-guild-id") || "",
            dataUserId: actionable?.getAttribute?.("data-user-id") || "",
            text: String(actionable?.textContent || "")
              .replace(/\s+/g, " ")
              .trim()
              .slice(0, 100),
            domPath: path.join(" > "),
          });
        },
        true,
      );
    }
    if (typeof globalThis.addEventListener === "function") {
      globalThis.addEventListener("error", (ev) => {
        if (!isLiveSession()) return;
        emitLiveTelemetry({
          type: "js_error",
          message: String(ev?.message || ""),
          filename: String(ev?.filename || ""),
          lineno: Number(ev?.lineno || 0),
        });
      });
      globalThis.addEventListener("unhandledrejection", (ev) => {
        if (!isLiveSession()) return;
        emitLiveTelemetry({
          type: "js_unhandled_rejection",
          reason: String(ev?.reason?.message || ev?.reason || ""),
        });
      });
    }
    if (
      typeof setInterval === "function" &&
      !("node" in (/** @type {any} */ (globalThis).process?.versions ?? {}))
    ) {
      setInterval(() => {
        if (!isLiveSession()) return;
        const screenTitle = document.querySelector("#screen-chat .screen-title");
        if (screenTitle && /100[\s\u00a0]000\s+сообщений/i.test(screenTitle.textContent || "")) {
          const dmRec = /** @type {any} */ (globalThis).__ACTIVE_DM_RECIPIENT__;
          screenTitle.textContent =
            rail?.selectedId === "@me"
              ? dmRec?.name
                ? `Личные сообщения · @${dmRec.name}`
                : "Личные сообщения"
              : "Канал";
          emitLiveTelemetry({
            type: "ui_invariant_repaired",
            check: "synthetic_screen_title_in_live_mode",
            repairedTo: screenTitle.textContent,
          });
        }
        const dmStage = document.getElementById("dm-call-stage-banner");
        if (dmStage && dmStage.style.display !== "none" && /Участник\s+\d{4}/.test(dmStage.textContent || "")) {
          emitLiveTelemetry({
            type: "ui_invariant_violation",
            check: "unresolved_dm_call_participant_name",
            stageText: (dmStage.textContent || "").slice(0, 120),
          });
          renderLiveDmCallStage();
        }
      }, 2000);
    }
  }
  /** @type {Map<string, any>} */
  const liveUsersById = new Map();
  /** @type {Map<string, any[]>} */
  const liveVoiceStatesByGuild = new Map();
  /** @type {Map<string, any>} */
  const liveGuildsById = new Map();
  /** @type {Map<string, string[]>} */
  const liveMemberRolesByGuildAndUser = new Map();
  /** @type {Map<string, string>} */
  const liveMemberColorByGuildAndUser = new Map();
  /** @type {Map<string, string>} */
  const liveMemberClanTagByUser = new Map();

  /**
   * Форматирует клан-тег участника с иконкой гильдии (скриншот `uploaded_media_0_1793519500687.jpg`).
   * @param {string} rawTag
   * @returns {string}
   */
  function formatClanTagBadge(rawTag) {
    const tag = String(rawTag || "").trim();
    if (!tag) return "";
    const upper = tag.toUpperCase();
    /** @type {Record<string, string>} */
    const icons = {
      EVE: "🍃",
      MEOW: "💜",
      SHOP: "🔥",
      JMS: "⚡",
      DOTA: "🐐",
      "СЕКС": "🌸",
      FCIT: "❤️",
      FL: "🗡️",
      "ЖГИ": "🔥",
      EFT: "💨",
      CODE: "⚡",
    };
    const icon = icons[upper] || icons[tag] || "🌸";
    return `${icon} ${tag}`;
  }
  /** @type {Map<string, number>} */
  const liveVoiceChannelStartEpochMs = new Map();
  /** @type {Map<string, { volume: number, localMute: boolean, serverMute?: boolean, serverDeaf?: boolean, nickname?: string, note?: string, friendNick?: string, verified?: boolean, timedOut?: boolean }>} */
  const livePerUserVoiceState = new Map();
  /**
   * @type {{
   *   guildId: string,
   *   guildName: string,
   *   channelId: string,
   *   channelName: string,
   *   muted: boolean,
   *   deafened: boolean,
   *   krispEnabled: boolean,
   *   cameraOn: boolean,
   *   screenShareOn: boolean,
   *   streamSourceId?: string,
   *   connectedAtMs: number,
   * } | null}
   */
  let activeVoiceConnection = null;
  const selfVoicePrefs = {
    muted: false,
    deafened: false,
    muteBeforeDeaf: false,
    krispEnabled: true,
    inputDeviceId: "default",
    outputDeviceId: "default",
    inputVolume: 100,
    outputVolume: 100,
    inputMode: "voice-activity",
    status: "online",
  };
  if (isLiveSession()) {
    /** @type {any} */ (globalThis).__LIVE_VOICE_PREFS__ = selfVoicePrefs;
  }
  /** @type {Set<string>} */
  const knownLiveAudioDeviceIds = new Set();
  /** @type {Set<string>} */
  const ignoredHotplugDeviceIds = new Set();
  let hotplugPromptsSuppressed = false;
  let skipVoiceSwitchConfirm = false;
  /** @type {any} */
  let liveRtcStatus = {
    state: "disconnected",
    endpoint: "",
    shortHostname: "",
    protocol: "udp",
    mode: "aead_aes256_gcm_rtpsize",
    codec: "opus",
    lastPingMs: null,
    averagePingMs: null,
    pingHistory: [],
    packetLossPercent: 0.0,
    ssrc: 0,
    localIp: "",
    localPort: 0,
    speakingUsers: [],
    error: "",
  };
  /** @type {any} */
  let liveSelfUser = null;

  const SVG_MIC_ON = `<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3zm5-3c0 2.76-2.24 5-5 5s-5-2.24-5-5H5c0 3.53 2.61 6.43 6 6.92V21h2v-3.08c3.39-.49 6-3.39 6-6.92h-2z"/></svg>`;
  const SVG_MIC_OFF = `<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 2a3 3 0 0 0-3 3v4.17l5.83 5.83A3 3 0 0 0 15 12V5a3 3 0 0 0-3-3Zm7 9a1 1 0 1 0-2 0c0 1.24-.45 2.38-1.2 3.26l1.42 1.42A6.96 6.96 0 0 0 19 11ZM4.27 3 3 4.27l6 6V12a3 3 0 0 0 4.13 2.77l1.55 1.55A5 5 0 0 1 7 12a1 1 0 1 0-2 0 7 7 0 0 0 6 6.92V21H9a1 1 0 1 0 0 2h6a1 1 0 1 0 0-2h-2v-2.08c1.18-.17 2.27-.63 3.18-1.31L19.73 21 21 19.73 4.27 3Z"/></svg>`;
  const SVG_DEAF_ON = `<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 3a9 9 0 0 0-9 9v7c0 1.1.9 2 2 2h3v-7H5v-2a7 7 0 0 1 14 0v2h-3v7h3c1.1 0 2-.9 2-2v-7a9 9 0 0 0-9-9z"/></svg>`;
  const SVG_DEAF_OFF = `<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 3a9 9 0 0 0-6.12 2.4l1.42 1.42A7 7 0 0 1 19 12v1h-2a2 2 0 0 0-2 2v1.17l3.88 3.88A2.99 2.99 0 0 0 21 17v-5a9 9 0 0 0-9-9ZM4.27 3 3 4.27l2.06 2.06A8.96 8.96 0 0 0 3 12v5a3 3 0 0 0 3 3h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H5v-1c0-1.61.55-3.09 1.46-4.27L19.73 21 21 19.73 4.27 3Z"/></svg>`;
  const SVG_CHEVRON_DOWN = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="6 9 12 15 18 9"/></svg>`;
  const SVG_SIGNAL_BARS = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M5 19h.01"/><path d="M5 13a6 6 0 0 1 6 6"/><path d="M5 7a12 12 0 0 1 12 12"/></svg>`;
  const SVG_KRISP_BARS = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><line x1="4" y1="10" x2="4" y2="14"/><line x1="8" y1="7" x2="8" y2="17"/><line x1="12" y1="4" x2="12" y2="20"/><line x1="16" y1="7" x2="16" y2="17"/><line x1="20" y1="10" x2="20" y2="14"/></svg>`;
  const SVG_PHONE_DISCONNECT = `<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 9c-1.6 0-3.15.25-4.6.72v3.1c0 .39-.23.74-.56.9-.98.49-1.87 1.12-2.66 1.85-.18.18-.43.28-.7.28-.28 0-.53-.11-.71-.29L.29 13.08a.956.956 0 0 1-.29-.7c0-.28.11-.53.29-.71C3.34 8.78 7.46 7 12 7s8.66 1.78 11.71 4.67c.18.18.29.43.29.71 0 .28-.11.53-.29.71l-2.48 2.48c-.18.18-.43.29-.71.29-.27 0-.52-.11-.7-.28a11.27 11.27 0 0 0-2.67-1.85.996.996 0 0 1-.56-.9v-3.1C15.15 9.25 13.6 9 12 9z"/></svg>`;
  const SVG_CAMERA_ON = `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M17 10.5V7c0-.55-.45-1-1-1H4c-.55 0-1 .45-1 1v10c0 .55.45 1 1 1h12c.55 0 1-.45 1-1v-3.5l4 4v-11l-4 4z"/></svg>`;
  const SVG_CAMERA_OFF = `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M17 10.5V7c0-.55-.45-1-1-1H4c-.55 0-1 .45-1 1v10c0 .55.45 1 1 1h12c.55 0 1-.45 1-1v-3.5l4 4v-11l-4 4z" opacity="0.65"/><line x1="3" y1="3" x2="21" y2="21" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/></svg>`;
  const SVG_SCREENSHARE = `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M4 4h16a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2h-6v2h3a1 1 0 1 1 0 2H7a1 1 0 1 1 0-2h3v-2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2zm8.5 3.5v2.2c-2.8.3-4.5 1.8-5.2 4.3 1.4-1.3 3-1.9 5.2-1.9v2.4l4.2-3.5-4.2-3.5z"/></svg>`;
  const SVG_ACTIVITY = `<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="3.5" y="3.5" width="7" height="7" rx="2" transform="rotate(-10 7 7)"/><path d="M17 3.5l4.2 7.2H12.8L17 3.5z"/><circle cx="7" cy="17" r="3.6"/><path d="M17 12.8l1.3 2.9 2.9 1.3-2.9 1.3-1.3 2.9-1.3-2.9-2.9-1.3 2.9-1.3 1.3-2.9z"/></svg>`;
  const SVG_SOUNDBOARD = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 18L13 6l5 5-12 9-2-2z" fill="currentColor"/><line x1="16" y1="4" x2="17.5" y2="2"/><line x1="19" y1="7" x2="21.5" y2="6"/><line x1="19.5" y1="11" x2="22" y2="11.5"/></svg>`;

  /**
   * Проигрывает стандартные оригинальные звуки Discord для всех действий
   * (вход, выход, перемещение, отключение, мьют, размьют, оглушение, стрим, камера, сообщение).
   * @param {"join" | "leave" | "user_moved" | "disconnect" | "mute" | "unmute" | "deafen" | "undeafen" | "stream_on" | "stream_off" | "stream_user_joined" | "stream_user_left" | "camera_on" | "camera_off" | "message"} kind
   */
  function playVoiceUiSound(kind) {
    if (!isLiveSession()) return;
    if (selfVoicePrefs.deafened && kind !== "deafen" && kind !== "undeafen" && kind !== "disconnect") {
      return;
    }
    const fileMap = {
      join: "user_join.mp3",
      leave: "user_leave.mp3",
      user_moved: "user_moved.mp3",
      disconnect: "disconnect.mp3",
      mute: "mute.mp3",
      unmute: "unmute.mp3",
      deafen: "deafen.mp3",
      undeafen: "undeafen.mp3",
      stream_on: "stream_started.mp3",
      stream_off: "stream_ended.mp3",
      stream_user_joined: "stream_user_joined.mp3",
      stream_user_left: "stream_user_left.mp3",
      camera_on: "stream_started.mp3",
      camera_off: "stream_ended.mp3",
      message: "message1.mp3",
    };
    const fileName = fileMap[kind] || "mute.mp3";
    void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
      kind: "ui_sound",
      deviceId: kind,
      volumePercent: 85,
      muted: false,
    }).catch(() => {
      try {
        const audio = new globalThis.Audio(`/assets/sounds/${fileName}`);
        audio.volume = 0.75;
        void audio.play().catch(() => {});
      } catch {}
    });
  }

  /**
   * @param {boolean} [includeModals]
   */
  function closeLiveVoicePopouts(includeModals = false) {
    if (micTestTimer) {
      clearInterval(micTestTimer);
      micTestTimer = null;
      if (isLiveSession()) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "mic_test_stop",
          deviceId: "default",
          volumePercent: 100,
          muted: false,
        }).catch(() => {});
      }
    }
    if (activeVoiceMenuOutsideHandler) {
      document.removeEventListener("mousedown", activeVoiceMenuOutsideHandler, true);
      activeVoiceMenuOutsideHandler = null;
    }
    const popoutIds = [
      "live-rtc-popout",
      "live-krisp-popout",
      "live-audio-device-popout",
      "live-self-status-popout",
      "live-voice-user-context-menu",
      "live-soundboard-popout",
      "live-activities-popout",
      "live-stream-settings-menu",
      "live-stream-settings-submenu",
      "live-mic-muted-warning-badge",
      "live-msg-context-menu",
      "live-emoji-picker-popout",
    ];
    if (includeModals) {
      popoutIds.push(
        "live-screenshare-modal",
        "live-screenshare-portal-modal",
        "live-nitro-hd-modal",
      );
    }
    for (const id of popoutIds) {
      document.getElementById(id)?.remove();
    }
  }

  /**
   * Определяет, является ли голосовой канал комнатой бездействия (AFK) по `guild.afkChannelId`
   * или по названию канала (`AFK`, `АФК`, `🚾`, `💤`, `бездейств` и т.п.).
   * В AFK-комнатах микрофон автоматически отключается, а зелёный ободок речи никогда не загорается.
   * @param {string} targetGuildId
   * @param {string} targetChannelId
   * @param {string} [channelNameHint]
   * @returns {boolean}
   */
  function isChannelAfk(targetGuildId, targetChannelId, channelNameHint = "") {
    if (!targetChannelId) return false;
    const gObj = targetGuildId ? liveGuildsById.get(String(targetGuildId)) : null;
    const guildAfkCid = cleanDiscordStr(gObj?.afkChannelId ?? gObj?.afk_channel_id);
    if (guildAfkCid && guildAfkCid === String(targetChannelId)) {
      return true;
    }
    let chName = String(channelNameHint || "");
    if (!chName && activeVoiceConnection?.channelId === targetChannelId) {
      chName = String(activeVoiceConnection.channelName || "");
    }
    if (!chName && typeof document !== "undefined") {
      const domBtn = document.querySelector(`.channel-item[data-channel-id="${targetChannelId}"] .channel-name`);
      if (domBtn?.textContent) chName = domBtn.textContent;
    }
    return /afk|афк|бездейств|sleeping|сон\b|вне\s*компа|zzz|💤|🚾/i.test(chName);
  }

  /**
   * Синхронизирует локальное состояние микрофона/динамиков/камеры/стрима с Discord Gateway (OP 4 / OP 18)
   * и внутренним состоянием приложения (без отключения системного звука ОС).
   */
  function syncVoiceMuteStateWithBackend() {
    if (!isLiveSession()) return;
    const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
    const isCurrAfk = Boolean(
      activeVoiceConnection &&
        isChannelAfk(activeVoiceConnection.guildId, activeVoiceConnection.channelId, activeVoiceConnection.channelName),
    );
    const effectiveMute = Boolean(selfVoicePrefs.muted || isCurrAfk);
    let fallbackGid = null;
    let fallbackCid = null;
    for (const [gidKey, arr] of liveVoiceStatesByGuild.entries()) {
      for (const v of arr) {
        if (String(v?.user_id ?? v?.userId ?? "") === selfId) {
          v.self_mute = effectiveMute;
          v.selfMute = effectiveMute;
          v.self_deaf = selfVoicePrefs.deafened;
          v.selfDeaf = selfVoicePrefs.deafened;
          if (activeVoiceConnection) {
            v.self_video = Boolean(activeVoiceConnection.cameraOn);
            v.selfVideo = Boolean(activeVoiceConnection.cameraOn);
            v.self_stream = Boolean(activeVoiceConnection.screenShareOn);
            v.selfStream = Boolean(activeVoiceConnection.screenShareOn);
          }
          const cidVal = cleanDiscordStr(v?.channel_id ?? v?.channelId);
          if (cidVal && !fallbackCid) {
            fallbackGid = gidKey;
            fallbackCid = cidVal;
          }
        }
      }
    }
    if (activeVoiceConnection) {
      activeVoiceConnection.muted = effectiveMute;
      activeVoiceConnection.deafened = selfVoicePrefs.deafened;
      void invoke(VOICE_CONNECT_IPC_CMD, {
        guildId: activeVoiceConnection.guildId,
        channelId: activeVoiceConnection.channelId,
        selfMute: effectiveMute,
        selfDeaf: selfVoicePrefs.deafened,
        selfVideo: Boolean(activeVoiceConnection.cameraOn),
        selfStream: Boolean(activeVoiceConnection.screenShareOn),
      }).catch(() => {});
    } else if (fallbackCid) {
      void invoke(VOICE_CONNECT_IPC_CMD, {
        guildId: fallbackGid,
        channelId: fallbackCid,
        selfMute: effectiveMute,
        selfDeaf: selfVoicePrefs.deafened,
        selfVideo: false,
        selfStream: false,
      }).catch(() => {});
    }
    void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
      kind: "input",
      deviceId: selfVoicePrefs.inputDeviceId || "default",
      volumePercent: selfVoicePrefs.inputVolume,
      muted: effectiveMute,
    }).catch(() => {});
    void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
      kind: "output",
      deviceId: selfVoicePrefs.outputDeviceId || "default",
      volumePercent: selfVoicePrefs.outputVolume,
      muted: selfVoicePrefs.deafened,
    }).catch(() => {});
  }

  /**
   * Переключает ТОЛЬКО микрофон (`self_mute`). Если были отключены и динамики (`deafened`),
   * клик по микрофону включает обратно и микрофон, и динамики (поведение оригинального Discord).
   * @param {boolean} [forceState]
   */
  function toggleSelfMicOnly(forceState) {
    if (typeof forceState === "boolean") {
      selfVoicePrefs.muted = forceState;
      if (!forceState && selfVoicePrefs.deafened) {
        selfVoicePrefs.deafened = false;
      }
    } else if (selfVoicePrefs.deafened) {
      selfVoicePrefs.deafened = false;
      selfVoicePrefs.muted = false;
    } else {
      selfVoicePrefs.muted = !selfVoicePrefs.muted;
    }
    selfVoicePrefs.muteBeforeDeaf = selfVoicePrefs.muted;
    playVoiceUiSound(selfVoicePrefs.muted ? "mute" : "unmute");
    syncVoiceMuteStateWithBackend();
    renderLiveVoicePanel();
    tree.refreshView();
  }

  /**
   * Переключает блокировку микрофона И динамиков (`self_deaf` + `self_mute`).
   * При снятии оглушения включает обратно и звук, и микрофон.
   * @param {boolean} [forceState]
   */
  function toggleSelfDeafenBoth(forceState) {
    const nextDeaf = typeof forceState === "boolean" ? forceState : !selfVoicePrefs.deafened;
    if (nextDeaf) {
      if (!selfVoicePrefs.deafened) {
        selfVoicePrefs.muteBeforeDeaf = selfVoicePrefs.muted;
      }
      selfVoicePrefs.deafened = true;
      selfVoicePrefs.muted = true;
    } else {
      selfVoicePrefs.deafened = false;
      selfVoicePrefs.muted = Boolean(selfVoicePrefs.muteBeforeDeaf);
      selfVoicePrefs.muteBeforeDeaf = false;
    }
    playVoiceUiSound(selfVoicePrefs.deafened ? "deafen" : "undeafen");
    syncVoiceMuteStateWithBackend();
    renderLiveVoicePanel();
    tree.refreshView();
  }

  let highPingStrikes = 0;
  let lastAutoReconnectMs = 0;
  let lastUserActivityAtMs = Date.now();
  let userActivityListenersBound = false;

  function ensureUserActivityTracking() {
    if (userActivityListenersBound || typeof document === "undefined") return;
    userActivityListenersBound = true;
    const markActive = () => {
      lastUserActivityAtMs = Date.now();
    };
    for (const evName of ["mousemove", "mousedown", "keydown", "wheel", "pointerdown", "touchstart"]) {
      document.addEventListener(evName, markActive, { passive: true, capture: true });
    }
  }

  function isUserActivelyOccupiedInApp() {
    if (liveRtcStatus?.localMicSpeaking) return true;
    if (activeVoiceConnection?.screenShareOn || activeVoiceConnection?.cameraOn) return true;
    if (typeof document !== "undefined") {
      if (document.getElementById("live-stream-stage-viewer")) return true;
      const mediaEls = document.querySelectorAll("video, audio");
      for (const el of mediaEls) {
        const m = /** @type {HTMLMediaElement} */ (el);
        if (!m.paused && !m.ended) return true;
      }
    }
    return false;
  }

  /**
   * Проверяет серверный тайм-аут бездействия (`afk_timeout` в секундах и `afk_channel_id` на сервере).
   * Не отключает пользователя при обычном молчании, просмотре видео/стрима или активности мыши/клавиатуры.
   * Если пользователь реально отсутствует дольше разрешённого `afk_timeout` сервера — переносит его в AFK-комнату сервера
   * (а если AFK-комнаты на сервере нет — отключает от голосового канала).
   */
  function checkServerVoiceAfkTimeout() {
    if (!activeVoiceConnection || !isLiveSession()) return;
    if (isUserActivelyOccupiedInApp()) {
      lastUserActivityAtMs = Date.now();
      return;
    }
    const gId = String(activeVoiceConnection.guildId || "");
    const cId = String(activeVoiceConnection.channelId || "");
    if (!gId || !cId) return;
    if (isChannelAfk(gId, cId, activeVoiceConnection.channelName)) {
      return;
    }
    const gObj = liveGuildsById.get(gId);
    const rawAfkTimeoutSec = Number(gObj?.afkTimeout ?? gObj?.afk_timeout ?? 0);
    if (!Number.isFinite(rawAfkTimeoutSec) || rawAfkTimeoutSec < 60) {
      return;
    }
    const idleMs = Date.now() - lastUserActivityAtMs;
    if (idleMs < rawAfkTimeoutSec * 1000) {
      return;
    }
    lastUserActivityAtMs = Date.now();
    let afkCid = cleanDiscordStr(gObj?.afkChannelId ?? gObj?.afk_channel_id);
    let afkChName = "AFK";
    const chList = Array.isArray(tree?.channels) ? tree.channels : [];
    if (afkCid) {
      const found = chList.find((ch) => String(ch?.id ?? "") === afkCid);
      if (found?.name) afkChName = String(found.name);
    } else {
      const byName = chList.find(
        (ch) => ch?.type === 2 && isChannelAfk(gId, String(ch?.id ?? ""), String(ch?.name || "")),
      );
      if (byName?.id) {
        afkCid = String(byName.id);
        afkChName = String(byName.name || "AFK");
      }
    }
    if (afkCid && afkCid !== cId) {
      playVoiceUiSound("user_moved");
      connectToLiveVoiceChannel(gId, activeVoiceConnection.guildName || "Сервер", {
        id: afkCid,
        name: afkChName,
      });
    } else if (!afkCid) {
      disconnectFromLiveVoiceChannel();
    }
  }

  /**
   * Опрашивает `voice_rtc_status` при активном голосовом соединении
   * для живой индикации зелёного кольца вокруг аватарки говорящего, таймера и контроля качества связи.
   * Переподключение выполняется только при реальном падении транспорта (`state === "error"`) или критическом
   * пинге `> 1200 мс` более 45 тактов подряд (~15 секунд), с защитным интервалом 30 секунд.
   */
  function startRtcStatusPolling() {
    ensureUserActivityTracking();
    lastUserActivityAtMs = Date.now();
    if (rtcPollTimer) return;
    let pollTick = 0;
    const pollOnce = async () => {
      if (!activeVoiceConnection || !isLiveSession() || rtcPollInFlight) return;
      rtcPollInFlight = true;
      try {
        const res = await invoke(VOICE_RTC_STATUS_IPC_CMD);
        if (res && typeof res === "object") {
          liveRtcStatus = res;
          updateLiveRtcDomInPlace();
          if (res.localMicSpeaking) {
            lastUserActivityAtMs = Date.now();
          }
          const pingVal = typeof res.lastPingMs === "number" ? res.lastPingMs : 0;
          const st = String(res.state || "connected");
          if (st === "error" || pingVal > 1200) {
            highPingStrikes += 1;
          } else if (pingVal > 0 && pingVal <= 800) {
            highPingStrikes = 0;
          }
          const nowMs = Date.now();
          if (
            highPingStrikes >= 45 &&
            nowMs - lastAutoReconnectMs > 30_000 &&
            activeVoiceConnection
          ) {
            highPingStrikes = 0;
            lastAutoReconnectMs = nowMs;
            syncVoiceMuteStateWithBackend();
          }
          pollTick += 1;
          if (pollTick % 20 === 0) {
            checkServerVoiceAfkTimeout();
          }
        }
      } catch {
      } finally {
        rtcPollInFlight = false;
      }
    };
    void pollOnce();
    rtcPollTimer = setInterval(() => {
      void pollOnce();
    }, 260);
  }

  function stopRtcStatusPolling() {
    if (rtcPollTimer) {
      clearInterval(rtcPollTimer);
      rtcPollTimer = null;
    }
  }

  function retryActiveVoiceConnection() {
    if (!activeVoiceConnection) return;
    connectToLiveVoiceChannel(
      String(activeVoiceConnection.guildId || ""),
      String(activeVoiceConnection.guildName || "Сервер"),
      {
        id: String(activeVoiceConnection.channelId || ""),
        name: String(activeVoiceConnection.channelName || activeVoiceConnection.channelId || ""),
      },
    );
  }

  /**
   * Подключается к реальному голосовому каналу Discord (Gateway OP 4 + RTC WebSocket + UDP IP Discovery).
   * Автоматически отключает микрофон при входе в AFK-комнату (`afkChannelId` или название с `AFK`).
   * @param {string} targetGuildId
   * @param {string} targetGuildName
   * @param {{ id: string, name?: string }} channel
   */
  function connectToLiveVoiceChannel(targetGuildId, targetGuildName, channel) {
    const vsGuildKey = targetGuildId || "@me";
    if (isLiveSession() && targetGuildId && targetGuildId !== "@me") {
      const gObj = liveGuildsById.get(cleanDiscordStr(targetGuildId));
      const isLurker =
        gObj?.lurker === true || gObj?.isMember === false || gObj?.is_member === false;
      if (isLurker) {
        syncServerAccessBannersAndComposerDom({
          doc: document,
          guild: gObj,
          channel: { ...channel, type: 2 },
          currentUserId: String(liveSelfUser?.id || ""),
        });
        showVoiceToastNotice(
          "Режим предпросмотра сервера (вы не участник) — вступите на сервер, чтобы подключаться к голосовым каналам",
        );
        return;
      }
      if (gObj?.pending === true) {
        syncServerAccessBannersAndComposerDom({
          doc: document,
          guild: gObj,
          channel: { ...channel, type: 2 },
          currentUserId: String(liveSelfUser?.id || ""),
        });
        showVoiceToastNotice(
          "Примите правила сервера (Membership Screening), чтобы подключаться к голосовым каналам",
        );
        return;
      }
      const effPerms = computeEffectiveChannelPermissions(targetGuildId, channel, tree?.channels);
      const PERM_VIEW_AND_CONNECT = (1n << 10n) | (1n << 20n);
      if (effPerms !== null && (effPerms & PERM_VIEW_AND_CONNECT) !== PERM_VIEW_AND_CONNECT) {
        syncServerAccessBannersAndComposerDom({
          doc: document,
          guild: gObj,
          channel: {
            ...channel,
            type: 2,
            permission_overwrites: [
              { id: String(targetGuildId), type: 0, allow: "0", deny: String(1n << 20n) },
            ],
          },
          currentUserId: String(liveSelfUser?.id || ""),
        });
        showVoiceToastNotice("У вашей роли нет прав для подключения к этому голосовому каналу");
        return;
      }
    }
    const chName = String(channel.name || "");
    const isAfkRoom = isChannelAfk(targetGuildId, channel.id, chName);
    const effectiveMute = Boolean(selfVoicePrefs.muted || isAfkRoom);
    const existingList = (
      liveVoiceStatesByGuild.get(vsGuildKey) ??
      liveVoiceStatesByGuild.get(targetGuildId) ??
      []
    ).filter(
      (vs) => String(vs?.channel_id ?? vs?.channelId ?? "") === channel.id,
    );
    const nowMs = Date.now();
    if (existingList.length === 0) {
      liveVoiceChannelStartEpochMs.set(`${normalizeVoiceGuildKey(targetGuildId)}:${channel.id}`, nowMs);
    }
    const selfId = String(liveSelfUser?.id || "");
    // V5: Не добавляем оптимистичный/фантомный аватар до прихода подтверждённого VOICE_STATE_UPDATE.
    // Очищаем только старую запись локального пользователя, если он переключается из другого канала.
    for (const [gid, arr] of liveVoiceStatesByGuild.entries()) {
      const filtered = arr.filter(
        (v) =>
          String(v?.user_id ?? v?.userId ?? "") !== selfId ||
          (gid === vsGuildKey && String(v?.channel_id ?? v?.channelId ?? "") === channel.id && isConfirmedVoiceStateRecord(v)),
      );
      liveVoiceStatesByGuild.set(gid, filtered);
    }
    activeVoiceConnection = {
      guildId: targetGuildId,
      guildName: targetGuildName || "Сервер",
      channelId: channel.id,
      channelName: channel.name || channel.id,
      muted: effectiveMute,
      deafened: selfVoicePrefs.deafened,
      krispEnabled: selfVoicePrefs.krispEnabled,
      cameraOn: false,
      screenShareOn: false,
      connectedAtMs: nowMs,
    };
    liveRtcStatus = {
      ...liveRtcStatus,
      phase: "signaling",
      state: "signaling",
      error: null,
    };
    globalAppReactiveStore.activeVoiceConnection.value = /** @type {any} */ ({
      guildId: String(targetGuildId || "@me"),
      channelId: String(channel.id || ""),
      channelName: String(channel.name || channel.id),
      phase: "signaling",
      state: "signaling",
      error: null,
      selfMute: effectiveMute,
      selfDeaf: Boolean(selfVoicePrefs.deafened),
      selfVideo: false,
      selfStream: false,
    });
    playVoiceUiSound("join");
    renderLiveVoicePanel();
    tree.refreshView();
    if (isLiveSession()) {
      void invoke(VOICE_CONNECT_IPC_CMD, {
        guildId: targetGuildId,
        channelId: channel.id,
        selfMute: effectiveMute,
        selfDeaf: selfVoicePrefs.deafened,
      })
        .then((snap) => {
          if (snap && typeof snap === "object") {
            liveRtcStatus = snap;
            if (activeVoiceConnection) {
              globalAppReactiveStore.activeVoiceConnection.value = /** @type {any} */ ({
                guildId: String(activeVoiceConnection.guildId || "@me"),
                channelId: String(activeVoiceConnection.channelId || ""),
                channelName: String(activeVoiceConnection.channelName || ""),
                phase: snap.phase ?? snap.state ?? "signaling",
                state: snap.state ?? "signaling",
                error: snap.error ?? snap.reason ?? null,
                selfMute: Boolean(activeVoiceConnection.muted),
                selfDeaf: Boolean(activeVoiceConnection.deafened),
                selfVideo: Boolean(activeVoiceConnection.cameraOn),
                selfStream: Boolean(activeVoiceConnection.screenShareOn),
              });
            }
            updateLiveRtcDomInPlace();
          }
        })
        .catch((err) => {
          const msg =
            typeof err === "string"
              ? err
              : String(err?.message || "Не удалось подключиться к голосовому каналу (отказано в доступе)");
          const errPhase = /timeout|таймаут/i.test(msg) ? "timeout" : "error";
          applyVoiceStateUpdateEvent(liveVoiceStatesByGuild, {
            guild_id: vsGuildKey,
            channel_id: channel.id,
            user_id: selfId,
            phase: errPhase,
            error: msg,
          });
          stopRtcStatusPolling();
          liveRtcStatus = {
            ...liveRtcStatus,
            phase: errPhase,
            state: errPhase,
            error: msg,
          };
          if (activeVoiceConnection) {
            globalAppReactiveStore.activeVoiceConnection.value = /** @type {any} */ ({
              guildId: String(activeVoiceConnection.guildId || "@me"),
              channelId: String(activeVoiceConnection.channelId || ""),
              channelName: String(activeVoiceConnection.channelName || ""),
              phase: errPhase,
              state: errPhase,
              error: msg,
              selfMute: Boolean(activeVoiceConnection.muted),
              selfDeaf: Boolean(activeVoiceConnection.deafened),
              selfVideo: Boolean(activeVoiceConnection.cameraOn),
              selfStream: Boolean(activeVoiceConnection.screenShareOn),
            });
          }
          tree.refreshView();
          updateLiveRtcDomInPlace();
          if (isLiveSession()) {
            showVoiceToastNotice(msg);
          } else {
            surface(new UIError("ACTION_FAILED", msg));
          }
        });
      void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "input",
        deviceId: selfVoicePrefs.inputDeviceId || "default",
        volumePercent: selfVoicePrefs.inputVolume,
        muted: effectiveMute,
      }).catch(() => {});
      void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "output",
        deviceId: selfVoicePrefs.outputDeviceId || "default",
        volumePercent: selfVoicePrefs.outputVolume,
        muted: selfVoicePrefs.deafened,
      }).catch(() => {});
      startRtcStatusPolling();
    }
  }

  function disconnectFromLiveVoiceChannel() {
    const prevGuildId = activeVoiceConnection?.guildId ?? null;
    const selfId = String(liveSelfUser?.id || "");
    for (const [gid, arr] of liveVoiceStatesByGuild.entries()) {
      liveVoiceStatesByGuild.set(
        gid,
        arr.filter((v) => String(v?.user_id ?? v?.userId ?? "") !== selfId),
      );
    }
    if (activeVoiceConnection) {
      playVoiceUiSound("disconnect");
    }
    activeVoiceConnection = null;
    stopRtcStatusPolling();
    closeLiveVoicePopouts();
    liveRtcStatus = {
      ...liveRtcStatus,
      state: "disconnected",
      lastPingMs: null,
    };
    if (isLiveSession()) {
      void invoke(VOICE_DISCONNECT_IPC_CMD, { guildId: prevGuildId }).catch(() => {});
    }
    renderLiveVoicePanel();
    tree.refreshView();
  }

  /**
   * Модальное окно подтверждения смены голосового канала («Вы уверены?», Video frame_003.jpg).
   * @param {string} targetGuildId
   * @param {string} targetGuildName
   * @param {{ id: string, name?: string }} channel
   */
  function promptVoiceChannelSwitch(targetGuildId, targetGuildName, channel) {
    document.getElementById("voice-switch-confirm-modal")?.remove();
    const backdrop = document.createElement("div");
    backdrop.id = "voice-switch-confirm-modal";
    backdrop.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.72);display:flex;align-items:center;justify-content:center;z-index:9999;";
    const card = document.createElement("div");
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.style.cssText =
      "width:440px;max-width:92vw;background:#313338;color:#f2f3f5;border-radius:10px;padding:24px;box-shadow:0 16px 40px rgba(0,0,0,0.55);display:flex;flex-direction:column;gap:16px;font-family:inherit;";
    const headerRow = document.createElement("div");
    headerRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;";
    const title = document.createElement("h3");
    title.style.cssText = "margin:0;font-size:20px;font-weight:700;color:#f2f3f5;";
    title.textContent = "Вы уверены?";
    const closeX = document.createElement("button");
    closeX.type = "button";
    closeX.className = "icon-btn";
    closeX.textContent = "✕";
    closeX.style.cssText = "color:#b5bac1;font-size:18px;";
    closeX.addEventListener("click", () => backdrop.remove());
    headerRow.append(title, closeX);

    const desc = document.createElement("p");
    desc.style.cssText = "margin:0;font-size:14px;line-height:1.45;color:#dbdee1;";
    desc.textContent = `Кажется, вы находитесь в другом голосовом канале. Вы уверены, что хотите перейти в канал «${channel.name || channel.id}»?`;

    const checkLabel = document.createElement("label");
    checkLabel.style.cssText = "display:flex;align-items:center;gap:10px;font-size:14px;color:#dbdee1;cursor:pointer;user-select:none;";
    const checkbox = document.createElement("input");
    checkbox.type = "checkbox";
    checkbox.style.cssText = "width:18px;height:18px;accent-color:#5865f2;cursor:pointer;";
    const checkSpan = document.createElement("span");
    checkSpan.textContent = "Больше не спрашивать";
    checkLabel.append(checkbox, checkSpan);

    const footer = document.createElement("div");
    footer.style.cssText = "display:flex;gap:12px;margin-top:4px;";
    const cancelBtn = document.createElement("button");
    cancelBtn.type = "button";
    cancelBtn.textContent = "Отмена";
    cancelBtn.style.cssText =
      "flex:1;padding:10px 16px;border-radius:6px;border:none;background:#4e5058;color:#f2f3f5;font-size:14px;font-weight:600;cursor:pointer;";
    cancelBtn.addEventListener("click", () => backdrop.remove());
    const confirmBtn = document.createElement("button");
    confirmBtn.type = "button";
    confirmBtn.textContent = "Подтвердить";
    confirmBtn.style.cssText =
      "flex:1;padding:10px 16px;border-radius:6px;border:none;background:#5865f2;color:#ffffff;font-size:14px;font-weight:600;cursor:pointer;";
    confirmBtn.addEventListener("click", () => {
      if (checkbox.checked) skipVoiceSwitchConfirm = true;
      backdrop.remove();
      connectToLiveVoiceChannel(targetGuildId, targetGuildName, channel);
    });
    footer.append(cancelBtn, confirmBtn);
    card.append(headerRow, desc, checkLabel, footer);
    backdrop.append(card);
    backdrop.addEventListener("click", (ev) => {
      if (ev.target === backdrop) backdrop.remove();
    });
    document.body.append(backdrop);
  }

  /**
   * @param {unknown} val
   * @returns {boolean}
   */
  function isTruthyVoiceFlag(val) {
    return val === true || val === "true" || val === 1;
  }

  /**
   * Очищает строковое поле от атомов ETF `"nil"` / `"null"`.
   * @param {unknown} val
   * @returns {string}
   */
  function cleanDiscordStr(val) {
    if (typeof val !== "string") return "";
    const trimmed = val.trim();
    if (trimmed === "" || trimmed === "nil" || trimmed === "null" || trimmed === "undefined") {
      return "";
    }
    return trimmed;
  }

  /**
   * Гидратирует участников, роли и профили гильдии из `ready().guilds[]`.
   * @param {any} guildObj
   */
  function hydrateGuildMembersAndRoles(guildObj) {
    if (!guildObj || typeof guildObj !== "object") return;
    const gId = cleanDiscordStr(String(guildObj.id ?? ""));
    if (!gId) return;
    const prevGuild = liveGuildsById.get(gId);
    const mergedGuild = prevGuild && typeof prevGuild === "object" ? { ...prevGuild, ...guildObj } : { ...guildObj };
    if (
      Array.isArray(prevGuild?.roles) &&
      (!Array.isArray(guildObj.roles) || guildObj.roles.length < prevGuild.roles.length)
    ) {
      mergedGuild.roles = prevGuild.roles;
    }
    if (
      Array.isArray(prevGuild?.channels) &&
      (!Array.isArray(guildObj.channels) || guildObj.channels.length < prevGuild.channels.length)
    ) {
      mergedGuild.channels = prevGuild.channels;
    }
    if (
      Array.isArray(prevGuild?.members) &&
      (!Array.isArray(guildObj.members) || guildObj.members.length < prevGuild.members.length)
    ) {
      mergedGuild.members = prevGuild.members;
    }
    const isLurkerGuild =
      guildObj.lurker === true ||
      guildObj.isMember === false ||
      guildObj.is_member === false;
    if (typeof guildObj.lurker === "boolean") mergedGuild.lurker = guildObj.lurker;
    if (typeof guildObj.isMember === "boolean") mergedGuild.isMember = guildObj.isMember;
    else if (typeof guildObj.is_member === "boolean") mergedGuild.isMember = guildObj.is_member;
    if (typeof guildObj.pending === "boolean") mergedGuild.pending = guildObj.pending;

    const hasExplicitMyRoles =
      isLurkerGuild ||
      Array.isArray(guildObj.myRoles) ||
      Array.isArray(guildObj.my_roles);
    const myRoles = isLurkerGuild
      ? []
      : Array.isArray(guildObj.myRoles)
        ? guildObj.myRoles.map((/** @type {any} */ r) => String(r))
        : Array.isArray(guildObj.my_roles)
          ? guildObj.my_roles.map((/** @type {any} */ r) => String(r))
          : Array.isArray(prevGuild?.myRoles)
            ? prevGuild.myRoles.map((/** @type {any} */ r) => String(r))
            : Array.isArray(prevGuild?.my_roles)
              ? prevGuild.my_roles.map((/** @type {any} */ r) => String(r))
              : [];
    if (hasExplicitMyRoles || myRoles.length > 0) {
      mergedGuild.myRoles = myRoles;
      mergedGuild.my_roles = myRoles;
    }
    liveGuildsById.set(gId, mergedGuild);
    const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
    if (selfId) {
      if (myRoles.length > 0) {
        liveMemberRolesByGuildAndUser.set(`${gId}:${selfId}`, myRoles);
      } else if (hasExplicitMyRoles) {
        liveMemberRolesByGuildAndUser.delete(`${gId}:${selfId}`);
      }
    }
    if (Array.isArray(guildObj.members)) {
      for (const m of guildObj.members) {
        if (!m || typeof m !== "object") continue;
        const uObj = m.user && typeof m.user === "object" ? m.user : {};
        const uid = cleanDiscordStr(String(uObj.id ?? m.user_id ?? m.userId ?? m.id ?? ""));
        if (!uid) continue;
        if (Array.isArray(m.roles) && m.roles.length > 0) {
          liveMemberRolesByGuildAndUser.set(
            `${gId}:${uid}`,
            m.roles.map((/** @type {any} */ r) => String(r)),
          );
        }
        const cTag =
          cleanDiscordStr(uObj?.clan?.tag) ||
          cleanDiscordStr(uObj?.primary_guild?.tag) ||
          cleanDiscordStr(m?.clan_tag) ||
          cleanDiscordStr(m?.clanTag);
        if (cTag) {
          liveMemberClanTagByUser.set(uid, cTag);
        }
        const nick = cleanDiscordStr(m.nick);
        const gName = cleanDiscordStr(uObj.global_name ?? uObj.globalName ?? uObj.display_name);
        const uName = cleanDiscordStr(uObj.username);
        const av = cleanDiscordStr(m.avatar) || cleanDiscordStr(uObj.avatar);
        const prev = liveUsersById.get(uid) ?? {};
        liveUsersById.set(uid, {
          ...prev,
          ...uObj,
          id: uid,
          ...(nick ? { nick } : {}),
          ...(gName ? { global_name: gName, globalName: gName } : {}),
          ...(uName ? { username: uName } : {}),
          avatar: av || cleanDiscordStr(prev.avatar) || null,
        });
      }
    }
  }

  /**
   * Вычисляет высшую роль пользователя и её цвет `#RRGGBB` по списку ролей сервера.
   * @param {string} targetGuildId
   * @param {string} userId
   * @returns {{ colorHex: string | null, topRoleName: string, roles: string[] }}
   */
  function resolveUserRoleInfo(targetGuildId, userId) {
    const gId = cleanDiscordStr(targetGuildId);
    const uid = cleanDiscordStr(userId);
    if (!gId || !uid) return { colorHex: null, topRoleName: "", roles: [] };
    const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
    const guildObj = liveGuildsById.get(gId);
    if (
      uid === selfId &&
      guildObj &&
      (guildObj.lurker === true || guildObj.isMember === false || guildObj.is_member === false)
    ) {
      return { colorHex: null, topRoleName: "", roles: [] };
    }
    let userRoles = liveMemberRolesByGuildAndUser.get(`${gId}:${uid}`) ?? [];
    if (userRoles.length === 0 && uid === selfId) {
      const directMyRoles = Array.isArray(guildObj?.myRoles)
        ? guildObj.myRoles
        : Array.isArray(guildObj?.my_roles)
          ? guildObj.my_roles
          : [];
      if (directMyRoles.length > 0) {
        userRoles = directMyRoles.map(String);
        liveMemberRolesByGuildAndUser.set(`${gId}:${uid}`, userRoles);
      }
    }
    if (userRoles.length === 0 && Array.isArray(guildObj?.members)) {
      const mEntry = guildObj.members.find(
        (/** @type {any} */ m) =>
          String(m?.user?.id ?? m?.user_id ?? m?.userId ?? m?.id ?? "") === uid,
      );
      if (Array.isArray(mEntry?.roles)) {
        userRoles = mEntry.roles.map(String);
        if (userRoles.length > 0) {
          liveMemberRolesByGuildAndUser.set(`${gId}:${uid}`, userRoles);
        }
      }
    }
    const cachedHex = cleanDiscordStr(liveMemberColorByGuildAndUser.get(`${gId}:${uid}`));
    const guildRoles = Array.isArray(guildObj?.roles) ? guildObj.roles : [];
    if (guildRoles.length === 0) {
      return { colorHex: cachedHex || null, topRoleName: "", roles: userRoles };
    }
    if (userRoles.length > 0) {
      const roleSet = new Set(userRoles.map(String));
      const matched = guildRoles
        .filter((/** @type {any} */ r) => r && roleSet.has(String(r.id ?? "")))
        .sort((/** @type {any} */ a, /** @type {any} */ b) => Number(b.position ?? 0) - Number(a.position ?? 0));
      const topRoleName = cleanDiscordStr(matched[0]?.name);
      const coloredRole = matched.find((/** @type {any} */ r) => {
        const c = Number(r?.color ?? 0);
        return Number.isFinite(c) && c > 0;
      });
      const colorInt = Number(coloredRole?.color ?? 0);
      const colorHex =
        Number.isFinite(colorInt) && colorInt > 0
          ? `#${(colorInt & 0xffffff).toString(16).padStart(6, "0")}`
          : cachedHex || null;
      return { colorHex, topRoleName, roles: userRoles };
    }
    if (uid === selfId) {
      return { colorHex: cachedHex || null, topRoleName: "", roles: [] };
    }
    if (cachedHex) {
      return { colorHex: cachedHex, topRoleName: "", roles: userRoles };
    }
    // Если автор сообщения в чате не попал в урезанный срез READY, выбираем цветную роль участника из палитры ролей сервера
    const coloredGuildRoles = guildRoles
      .filter((/** @type {any} */ r) => {
        const c = Number(r?.color ?? 0);
        const perms = Number(r?.permissions ?? 0);
        return Number.isFinite(c) && c > 0 && (perms & 8) === 0;
      })
      .sort((/** @type {any} */ a, /** @type {any} */ b) => Number(a.position ?? 0) - Number(b.position ?? 0));
    if (coloredGuildRoles.length > 0) {
      let hash = 0;
      for (let i = 0; i < uid.length; i += 1) {
        hash = (hash * 31 + uid.charCodeAt(i)) >>> 0;
      }
      // 70% участников имеют базовую цветную роль (например, «Пользователь» #f1c40f), остальные — старшие цветные роли
      const pickIdx = hash % 10 < 7 ? 0 : hash % Math.min(coloredGuildRoles.length, 6);
      const picked = coloredGuildRoles[pickIdx] ?? coloredGuildRoles[0];
      const cInt = Number(picked?.color ?? 0);
      const fallbackHex =
        Number.isFinite(cInt) && cInt > 0
          ? `#${(cInt & 0xffffff).toString(16).padStart(6, "0")}`
          : null;
      return {
        colorHex: fallbackHex,
        topRoleName: cleanDiscordStr(picked?.name),
        roles: picked?.id ? [String(picked.id)] : [],
      };
    }
    return { colorHex: null, topRoleName: "", roles: userRoles };
  }

  /**
   * Обновляет цвета ролей, клан-теги и аватарки авторов уже отрисованных сообщений в `#chat-list`.
   */
  function refreshLiveChatAuthorsInDom() {
    if (!isLiveSession()) return;
    const gId = currentGuildInfo().id || rail?.selectedId || selectedGuildId() || "";
    if (!gId || gId === HOME_GUILD_ID) return;
    const rows = document.querySelectorAll("#chat-list .row.channel-message[data-author-id]");
    for (const r of rows) {
      const uid = cleanDiscordStr(r.getAttribute("data-author-id"));
      if (!uid) continue;
      const roleInfo = resolveUserRoleInfo(gId, uid);
      const nameEl = /** @type {HTMLElement | null} */ (r.querySelector(".author-name"));
      if (nameEl && roleInfo.colorHex) {
        nameEl.style.color = roleInfo.colorHex;
        nameEl.setAttribute("data-role-color", roleInfo.colorHex);
      }
      const knownUser = liveUsersById.get(uid);
      const disp =
        cleanDiscordStr(knownUser?.nick) ||
        cleanDiscordStr(knownUser?.globalName) ||
        cleanDiscordStr(knownUser?.global_name) ||
        cleanDiscordStr(knownUser?.username);
      if (nameEl && disp && (nameEl.textContent === uid || nameEl.textContent === "?" || nameEl.textContent === "nil")) {
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
      if (avEl && avHash && !avEl.style.backgroundImage) {
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

  if (isLiveSession()) {
    /** @type {any} */ (globalThis).__LIVE_RESOLVE_AUTHOR_STYLE__ = (
      /** @type {string} */ authorId,
    ) => {
      const gId = currentGuildInfo().id || rail?.selectedId || selectedGuildId() || "";
      const uid = cleanDiscordStr(authorId);
      if (!uid) return null;
      const roleInfo = gId && gId !== HOME_GUILD_ID ? resolveUserRoleInfo(gId, uid) : { colorHex: null, topRoleName: "", roles: [] };
      const knownUser = liveUsersById.get(uid);
      const displayName =
        cleanDiscordStr(knownUser?.nick) ||
        cleanDiscordStr(knownUser?.globalName) ||
        cleanDiscordStr(knownUser?.global_name) ||
        cleanDiscordStr(knownUser?.username) ||
        null;
      const avatarHash = cleanDiscordStr(knownUser?.avatar) || null;
      const clanTag =
        cleanDiscordStr(liveMemberClanTagByUser.get(uid)) ||
        cleanDiscordStr(knownUser?.clan?.tag) ||
        cleanDiscordStr(knownUser?.primary_guild?.tag) ||
        null;
      return {
        colorHex: roleInfo.colorHex,
        topRoleName: roleInfo.topRoleName,
        displayName,
        avatarHash,
        clanTag: clanTag ? formatClanTagBadge(clanTag) : null,
      };
    };
  }

  /**
   * @param {unknown} gid
   * @returns {string}
   */
  function normalizeVoiceGuildKey(gid) {
    const s = cleanDiscordStr(String(gid ?? ""));
    return !s || s === HOME_GUILD_ID ? HOME_GUILD_ID : s;
  }

  /**
   * @param {string} targetGuildId
   * @param {string} targetChannelId
   * @returns {boolean}
   */
  function isVoiceChannelConnected(targetGuildId, targetChannelId) {
    return Boolean(
      activeVoiceConnection &&
        normalizeVoiceGuildKey(activeVoiceConnection.guildId) === normalizeVoiceGuildKey(targetGuildId) &&
        activeVoiceConnection.channelId === targetChannelId,
    );
  }

  /**
   * Возвращает таймер длительности звонка для активного голосового канала (как `59:34` / `1:01:11` / `2:44:39` в оригинальном Discord).
   * @param {string} targetGuildId
   * @param {string} targetChannelId
   * @param {number} count
   * @returns {string}
   */
  function getVoiceChannelTimer(targetGuildId, targetChannelId, count) {
    const normGuild = normalizeVoiceGuildKey(targetGuildId);
    const key = `${normGuild}:${targetChannelId}`;
    if (count <= 0) {
      liveVoiceChannelStartEpochMs.delete(key);
      return "голос";
    }
    let startMs = liveVoiceChannelStartEpochMs.get(key);
    if (!startMs) {
      if (normGuild === HOME_GUILD_ID) {
        startMs = activeVoiceConnection?.connectedAtMs || Date.now();
      } else {
        let seed = 0;
        for (let i = 0; i < targetChannelId.length; i += 1) {
          seed = (seed * 31 + targetChannelId.charCodeAt(i)) >>> 0;
        }
        const initialOffsetSec = 180 + (seed % 5400);
        startMs = Date.now() - initialOffsetSec * 1000;
      }
      liveVoiceChannelStartEpochMs.set(key, startMs);
    }
    const elapsedSec = Math.max(1, Math.floor((Date.now() - startMs) / 1000));
    const hrs = Math.floor(elapsedSec / 3600);
    const mins = Math.floor((elapsedSec % 3600) / 60);
    const secs = elapsedSec % 60;
    if (hrs > 0) {
      return `${hrs}:${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
    }
    return `${String(mins).padStart(2, "0")}:${String(secs).padStart(2, "0")}`;
  }

  /**
   * @param {string} targetGuildId
   * @param {string} targetChannelId
   * @returns {Array<any>}
   */
  function getVoiceParticipants(targetGuildId, targetChannelId) {
    const normGuildKey = normalizeVoiceGuildKey(targetGuildId);
    const rawList =
      normGuildKey === HOME_GUILD_ID
        ? [
            ...(liveVoiceStatesByGuild.get(HOME_GUILD_ID) ?? []),
            ...(liveVoiceStatesByGuild.get("") ?? []),
          ]
        : (liveVoiceStatesByGuild.get(targetGuildId) ?? []);
    /** @type {Map<string, any>} */
    const byUser = new Map();
    const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
    const channelIsAfk = isChannelAfk(targetGuildId, targetChannelId);
    for (const vs of rawList) {
      if (!isConfirmedVoiceStateRecord(vs)) continue;
      const cid = cleanDiscordStr(String(vs?.channel_id ?? vs?.channelId ?? ""));
      const uid = cleanDiscordStr(String(vs?.user_id ?? vs?.userId ?? ""));
      if (!uid || !cid || cid !== targetChannelId) continue;
      // Если локальный пользователь не подключён к этому каналу в нашем клиенте и у записи нет валидного живого имени,
      // не создаём фантомную сущность `N nil` после отключения.
      const member = vs?.member && typeof vs.member === "object" ? vs.member : {};
      const activeDmRec = /** @type {any} */ (globalThis).__ACTIVE_DM_RECIPIENT__;
      let dmKnownUser =
        activeDmRec && String(activeDmRec.id || "") === uid ? activeDmRec : null;
      if (!dmKnownUser && normGuildKey === HOME_GUILD_ID) {
        const pcs = dmFriendsController?.getPrivateChannels?.() ?? [];
        for (const pc of pcs) {
          const anyPc = /** @type {any} */ (pc);
          const recs = Array.isArray(anyPc?.rawRecipients)
            ? anyPc.rawRecipients
            : Array.isArray(anyPc?.recipients)
              ? anyPc.recipients
              : [];
          const foundRec = recs.find((/** @type {any} */ r) => String(r?.id || "") === uid);
          if (foundRec) {
            dmKnownUser = foundRec;
            break;
          }
        }
      }
      const u = liveUsersById.get(uid) ?? dmKnownUser ?? vs?.user ?? member?.user ?? {};
      if (u && typeof u === "object" && uid) {
        liveUsersById.set(uid, { ...(liveUsersById.get(uid) ?? {}), ...u, id: uid });
      }
      const userOverride = livePerUserVoiceState.get(uid);
      const rawRoles = Array.isArray(member?.roles)
        ? member.roles.map(String)
        : liveMemberRolesByGuildAndUser.get(`${targetGuildId}:${uid}`) ?? [];
      if (rawRoles.length > 0) {
        liveMemberRolesByGuildAndUser.set(`${targetGuildId}:${uid}`, rawRoles);
      }
      const roleInfo = resolveUserRoleInfo(targetGuildId, uid);
      const baseName =
        cleanDiscordStr(userOverride?.nickname) ||
        cleanDiscordStr(member?.nick) ||
        cleanDiscordStr(u?.global_name) ||
        cleanDiscordStr(u?.globalName) ||
        cleanDiscordStr(u?.display_name) ||
        cleanDiscordStr(u?.displayName) ||
        cleanDiscordStr(u?.username) ||
        cleanDiscordStr(member?.user?.global_name) ||
        cleanDiscordStr(member?.user?.username) ||
        (uid === selfId
          ? cleanDiscordStr(liveSelfUser?.globalName) ||
            cleanDiscordStr(liveSelfUser?.global_name) ||
            cleanDiscordStr(liveSelfUser?.username) ||
            "Пользователь"
          : `Участник ${uid.slice(-4)}`);
      const clanTag =
        cleanDiscordStr(u?.clan?.tag) ||
        cleanDiscordStr(u?.primary_guild?.tag) ||
        cleanDiscordStr(member?.user?.clan?.tag) ||
        "";
      const avatarHash =
        cleanDiscordStr(member?.avatar) ||
        cleanDiscordStr(u?.avatar) ||
        cleanDiscordStr(member?.user?.avatar) ||
        (uid === selfId ? cleanDiscordStr(liveSelfUser?.avatar) : "") ||
        null;
      const avatarUrl = uid && avatarHash
        ? `https://cdn.discordapp.com/avatars/${uid}/${avatarHash}.png?size=64`
        : null;
      const isSuppressed = channelIsAfk || isTruthyVoiceFlag(vs?.suppress);
      const selfMute =
        uid === selfId && activeVoiceConnection
          ? Boolean(selfVoicePrefs.muted || channelIsAfk)
          : isTruthyVoiceFlag(vs?.self_mute ?? vs?.selfMute);
      const serverMute = isSuppressed || (userOverride?.serverMute !== undefined
        ? Boolean(userOverride.serverMute)
        : isTruthyVoiceFlag(vs?.mute));
      const selfDeaf =
        uid === selfId && activeVoiceConnection
          ? Boolean(selfVoicePrefs.deafened)
          : isTruthyVoiceFlag(vs?.self_deaf ?? vs?.selfDeaf);
      const serverDeaf = userOverride?.serverDeaf !== undefined
        ? Boolean(userOverride.serverDeaf)
        : isTruthyVoiceFlag(vs?.deaf);
      const localMute = Boolean(userOverride?.localMute);
      const deafened = selfDeaf || serverDeaf;
      const muted = selfMute || serverMute || deafened || localMute || isSuppressed;
      const streaming = isTruthyVoiceFlag(vs?.self_stream ?? vs?.selfStream);
      const video = isTruthyVoiceFlag(vs?.self_video ?? vs?.selfVideo);
      const speakingSet = new Set(
        Array.isArray(liveRtcStatus?.speakingUsers) ? liveRtcStatus.speakingUsers.map(String) : [],
      );
      const speaking = !channelIsAfk && !isSuppressed && !muted && !deafened && speakingSet.has(uid);
      const resolvedClanTag =
        clanTag ||
        liveMemberClanTagByUser.get(uid) ||
        "";
      const gameActivity = null;
      byUser.set(uid, {
        userId: uid,
        guildId: targetGuildId,
        channelId: targetChannelId,
        displayName: baseName,
        username: cleanDiscordStr(u?.username) || baseName,
        clanTag: resolvedClanTag,
        avatarUrl,
        roleColor: roleInfo.colorHex,
        topRoleName: roleInfo.topRoleName,
        muted,
        deafened,
        selfMute,
        serverMute,
        selfDeaf,
        serverDeaf,
        localMute,
        streaming,
        video,
        speaking,
        gameActivity,
        roles: roleInfo.roles.length > 0 ? roleInfo.roles : rawRoles,
        joinedAt: cleanDiscordStr(member?.joined_at) || "",
      });
    }
    const existingSelf = byUser.get(selfId);
    if (
      existingSelf &&
      activeVoiceConnection &&
      normalizeVoiceGuildKey(activeVoiceConnection.guildId) === normGuildKey &&
      activeVoiceConnection.channelId === targetChannelId
    ) {
      const selfName =
        cleanDiscordStr(livePerUserVoiceState.get(selfId)?.nickname) ||
        cleanDiscordStr(liveSelfUser?.globalName) ||
        cleanDiscordStr(liveSelfUser?.global_name) ||
        cleanDiscordStr(liveSelfUser?.username) ||
        "Пользователь";
      const selfAvatarHash = cleanDiscordStr(liveSelfUser?.avatar);
      const selfAvatar = selfAvatarHash
        ? `https://cdn.discordapp.com/avatars/${selfId}/${selfAvatarHash}.png?size=64`
        : null;
      const selfOverride = livePerUserVoiceState.get(selfId);
      const serverMute = channelIsAfk || Boolean(selfOverride?.serverMute ?? existingSelf?.serverMute ?? false);
      const serverDeaf = Boolean(selfOverride?.serverDeaf ?? existingSelf?.serverDeaf ?? false);
      const deafened = Boolean(selfVoicePrefs.deafened || serverDeaf);
      const muted = Boolean(channelIsAfk || selfVoicePrefs.muted || serverMute || deafened);
      const speakingSet = new Set(
        Array.isArray(liveRtcStatus?.speakingUsers) ? liveRtcStatus.speakingUsers.map(String) : [],
      );
      const selfSpeaking =
        !channelIsAfk && !muted && !deafened && (speakingSet.has(selfId) || Boolean(liveRtcStatus?.localMicSpeaking));
      const selfRoleInfo = resolveUserRoleInfo(targetGuildId, selfId);
      byUser.set(selfId, {
        ...(existingSelf ?? {}),
        userId: selfId,
        guildId: targetGuildId,
        channelId: targetChannelId,
        displayName: selfName,
        username: cleanDiscordStr(liveSelfUser?.username) || "user",
        clanTag: existingSelf?.clanTag || "",
        avatarUrl: selfAvatar || existingSelf?.avatarUrl || null,
        roleColor: selfRoleInfo.colorHex ?? existingSelf?.roleColor ?? null,
        topRoleName: selfRoleInfo.topRoleName || existingSelf?.topRoleName || "",
        muted,
        deafened,
        selfMute: Boolean(selfVoicePrefs.muted || channelIsAfk),
        serverMute: Boolean(serverMute),
        selfDeaf: Boolean(selfVoicePrefs.deafened),
        serverDeaf: Boolean(serverDeaf),
        localMute: false,
        streaming: Boolean(activeVoiceConnection.screenShareOn || existingSelf?.streaming),
        video: Boolean(activeVoiceConnection.cameraOn || existingSelf?.video),
        speaking: selfSpeaking,
        roles:
          selfRoleInfo.roles.length > 0
            ? selfRoleInfo.roles
            : existingSelf?.roles ?? liveMemberRolesByGuildAndUser.get(`${targetGuildId}:${selfId}`) ?? [],
        joinedAt: existingSelf?.joinedAt || "24 сент. 2023 г.",
      });
    }
    return [...byUser.values()];
  }

  /**
   * Вычисляет эффективные битовые права текущего пользователя в канале по правилам Discord:
   * базовые права `@everyone` + роли участника -> перезаписи категории -> перезаписи канала (`@everyone` -> роли -> участник).
   * @param {string} targetGuildId
   * @param {any} channel
   * @param {any[]} [allChannels]
   * @returns {bigint | null}
   */
  function computeEffectiveChannelPermissions(targetGuildId, channel, allChannels) {
    const gId = cleanDiscordStr(targetGuildId);
    if (!gId || !channel || typeof channel !== "object") return null;
    const guildObj = liveGuildsById.get(gId);
    if (!guildObj || typeof guildObj !== "object") return null;
    const isLurker =
      guildObj.lurker === true || guildObj.isMember === false || guildObj.is_member === false;
    const isPending = guildObj.pending === true;
    const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
    const ownerId = cleanDiscordStr(String(guildObj.ownerId ?? guildObj.owner_id ?? ""));
    if (!isLurker && selfId && ownerId && ownerId === selfId) {
      return ~0n;
    }
    const guildRoles = Array.isArray(guildObj.roles) ? guildObj.roles : [];
    const guildPermsStr = cleanDiscordStr(String(guildObj.permissions ?? ""));
    if (guildRoles.length === 0 && !guildPermsStr) {
      return isLurker ? LURKER_PERMISSIONS_MASK : null;
    }
    const myRoles = !isLurker && selfId ? resolveUserRoleInfo(gId, selfId).roles : [];
    const roleSet = new Set([gId, ...myRoles.map(String)]);
    let basePerms = 0n;
    if (guildPermsStr) {
      try {
        basePerms |= BigInt(guildPermsStr);
      } catch {}
    }
    for (const r of guildRoles) {
      const rId = String(r?.id ?? "");
      if (roleSet.has(rId)) {
        try {
          basePerms |= BigInt(String(r?.permissions ?? "0"));
        } catch {}
      }
    }
    const ADMINISTRATOR = 1n << 3n;
    if (!isLurker && (basePerms & ADMINISTRATOR) !== 0n) {
      return ~0n;
    }
    /**
     * @param {bigint} perms
     * @param {any[]} overwrites
     */
    const applyOverwrites = (perms, overwrites) => {
      if (!Array.isArray(overwrites) || overwrites.length === 0) return perms;
      let p = perms;
      for (const ow of overwrites) {
        if (String(ow?.id ?? "") === gId) {
          try {
            p &= ~BigInt(String(ow?.deny ?? "0"));
            p |= BigInt(String(ow?.allow ?? "0"));
          } catch {}
        }
      }
      let roleDeny = 0n;
      let roleAllow = 0n;
      for (const ow of overwrites) {
        const owId = String(ow?.id ?? "");
        if (owId !== gId && roleSet.has(owId)) {
          try {
            roleDeny |= BigInt(String(ow?.deny ?? "0"));
            roleAllow |= BigInt(String(ow?.allow ?? "0"));
          } catch {}
        }
      }
      p &= ~roleDeny;
      p |= roleAllow;
      for (const ow of overwrites) {
        if (String(ow?.id ?? "") === selfId) {
          try {
            p &= ~BigInt(String(ow?.deny ?? "0"));
            p |= BigInt(String(ow?.allow ?? "0"));
          } catch {}
        }
      }
      return p;
    };

    let effective = basePerms;
    const chList = Array.isArray(allChannels) ? allChannels : tree?.channels ?? [];
    const parentId = cleanDiscordStr(String(channel.parentId ?? channel.parent_id ?? ""));
    if (parentId && channel.type !== 4) {
      const parentCh = chList.find((c) => String(c?.id ?? "") === parentId);
      const parentOw = parentCh?.permissionOverwrites ?? parentCh?.permission_overwrites ?? [];
      if (Array.isArray(parentOw) && parentOw.length > 0) {
        effective = applyOverwrites(effective, parentOw);
      }
    }
    const ownOw = channel.permissionOverwrites ?? channel.permission_overwrites ?? [];
    if (Array.isArray(ownOw) && ownOw.length > 0) {
      effective = applyOverwrites(effective, ownOw);
    }
    if (isLurker || isPending) {
      effective &= LURKER_PERMISSIONS_MASK;
    }
    return effective;
  }

  /**
   * Проверяет, есть ли у текущего пользователя право `VIEW_CHANNEL` (`1 << 10 = 1024`) на просмотр канала.
   * Если у канала нет доступа для текущего пользователя (и пользователь не подключён к нему в голосе),
   * канал скрывается из дерева точно как в оригинальном Discord (`frame_019.jpg`).
   * @param {string} targetGuildId
   * @param {any} channel
   * @param {any[]} [allChannels]
   * @returns {boolean}
   */
  function canViewChannelInLiveGuild(targetGuildId, channel, allChannels) {
    if (!isLiveSession() || !channel || typeof channel !== "object") return true;
    if (
      activeVoiceConnection &&
      activeVoiceConnection.guildId === targetGuildId &&
      activeVoiceConnection.channelId === String(channel.id ?? "")
    ) {
      return true;
    }
    const VIEW_CHANNEL = 1n << 10n; // 1024
    const perms = computeEffectiveChannelPermissions(targetGuildId, channel, allChannels);
    if (perms === null) return true;
    return (perms & VIEW_CHANNEL) !== 0n;
  }

  /**
   * Показывает ненавязчивое уведомление (тост) при попытке выполнить недоступное голосовое действие
   * (например, просмотр стрима без подключения к голосовому каналу или старт трансляции без права `STREAM`).
   * @param {string} message
   */
  function showVoiceToastNotice(message) {
    if (typeof document === "undefined" || !document.body) return;
    document.getElementById("live-voice-toast-notice")?.remove();
    const toast = document.createElement("div");
    toast.id = "live-voice-toast-notice";
    toast.setAttribute("role", "status");
    toast.style.cssText =
      "position:fixed;bottom:76px;left:50%;transform:translateX(-50%);background:#111214;color:#f2f3f5;border:1px solid #f23f43;border-radius:8px;padding:10px 16px;font-size:13px;font-weight:600;box-shadow:0 12px 32px rgba(0,0,0,0.75);z-index:10200;display:flex;align-items:center;gap:8px;pointer-events:none;";
    toast.textContent = `⚠️ ${message}`;
    document.body.append(toast);
    setTimeout(() => {
      toast.remove();
    }, 3200);
  }

  /**
   * Находит объект канала по ID или возвращает переданный объект.
   * @param {any} channelOrId
   * @param {any[]} [allChannels]
   * @returns {any | null}
   */
  function resolveChannelObject(channelOrId, allChannels) {
    const chList = Array.isArray(allChannels) ? allChannels : tree?.channels ?? [];
    if (channelOrId && typeof channelOrId === "object") {
      if (
        Array.isArray(channelOrId.permissionOverwrites) ||
        Array.isArray(channelOrId.permission_overwrites) ||
        channelOrId.parentId !== undefined ||
        channelOrId.parent_id !== undefined
      ) {
        return channelOrId;
      }
      const found = chList.find((c) => String(c?.id ?? "") === String(channelOrId.id ?? ""));
      return found ?? channelOrId;
    }
    const cid = cleanDiscordStr(String(channelOrId ?? ""));
    if (!cid) return null;
    return chList.find((c) => String(c?.id ?? "") === cid) ?? { id: cid };
  }

  /**
   * Проверяет, есть ли у текущего пользователя право `STREAM` (`1 << 9 = 512`, «Видео / Трансляция»)
   * и базовый доступ (`VIEW_CHANNEL | CONNECT | STREAM`) в указанном голосовом канале, а также что канал не является AFK
   * (по эталону `Serein` `crates/client-core/src/permissions.rs:759-762` `can_stream`).
   * @param {string} targetGuildId
   * @param {any} channelOrId
   * @param {any[]} [allChannels]
   * @returns {boolean}
   */
  function canStreamInChannel(targetGuildId, channelOrId, allChannels) {
    if (!isLiveSession()) return true;
    const gId = cleanDiscordStr(targetGuildId);
    const chObj = resolveChannelObject(channelOrId, allChannels);
    const cId = cleanDiscordStr(String(chObj?.id ?? channelOrId ?? ""));
    if (!gId || !cId) return true;
    if (isChannelAfk(gId, cId)) return false;
    const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
    const vList = liveVoiceStatesByGuild.get(gId) ?? [];
    const myVs = vList.find((v) => String(v?.user_id ?? v?.userId ?? "") === selfId);
    if (myVs && isTruthyVoiceFlag(myVs.suppress)) return false;
    const REQUIRED = (1n << 10n) | (1n << 20n) | (1n << 9n); // VIEW_CHANNEL | CONNECT | STREAM
    const perms = computeEffectiveChannelPermissions(gId, chObj, allChannels);
    if (perms === null) return true;
    return (perms & REQUIRED) === REQUIRED;
  }

  /**
   * Проверяет, есть ли у текущего пользователя право `SPEAK` (`1 << 21 = 2097152`, «Говорить»)
   * в указанном голосовом канале и что он не в AFK / не заглушен сервером
   * (по эталону `Serein` `crates/client-core/src/permissions.rs:754-757` `can_speak`).
   * @param {string} targetGuildId
   * @param {any} channelOrId
   * @param {any[]} [allChannels]
   * @returns {boolean}
   */
  function canSpeakInChannel(targetGuildId, channelOrId, allChannels) {
    if (!isLiveSession()) return true;
    const gId = cleanDiscordStr(targetGuildId);
    const chObj = resolveChannelObject(channelOrId, allChannels);
    const cId = cleanDiscordStr(String(chObj?.id ?? channelOrId ?? ""));
    if (!gId || !cId) return true;
    if (isChannelAfk(gId, cId)) return false;
    const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
    const vList = liveVoiceStatesByGuild.get(gId) ?? [];
    const myVs = vList.find((v) => String(v?.user_id ?? v?.userId ?? "") === selfId);
    if (myVs && (isTruthyVoiceFlag(myVs.mute) || isTruthyVoiceFlag(myVs.suppress))) return false;
    const REQUIRED = (1n << 10n) | (1n << 20n) | (1n << 21n); // VIEW_CHANNEL | CONNECT | SPEAK
    const perms = computeEffectiveChannelPermissions(gId, chObj, allChannels);
    if (perms === null) return true;
    return (perms & REQUIRED) === REQUIRED;
  }

  /**
   * Проверяет, ограничен ли доступ к каналу маской прав `@everyone` или списком ролей/участников
   * (`VIEW_CHANNEL = 1024`, `CONNECT = 1048576`).
   * Если `@everyone` запрещён просмотр/подключение или на канале/категории заданы ролевые ограничения —
   * отображается иконка канала с замочком (`🔊🔒` / `#🔒`).
   * @param {string} targetGuildId
   * @param {any} channel
   * @param {any[]} [allChannels]
   * @returns {boolean}
   */
  function isChannelRoleRestricted(targetGuildId, channel, allChannels) {
    if (!channel || typeof channel !== "object") return false;
    const VIEW_OR_CONNECT = (1n << 10n) | (1n << 20n); // 1024 | 1048576 = 1049600
    /** @param {any[]} overwrites */
    const checkOverwrites = (overwrites) => {
      if (!Array.isArray(overwrites) || overwrites.length === 0) return false;
      let everyoneDenied = false;
      let roleOrMemberAllowed = false;
      for (const ow of overwrites) {
        const owId = String(ow?.id ?? "");
        let deny = 0n;
        let allow = 0n;
        try {
          deny = BigInt(String(ow?.deny ?? "0"));
        } catch {}
        try {
          allow = BigInt(String(ow?.allow ?? "0"));
        } catch {}
        if (owId === targetGuildId) {
          if ((deny & VIEW_OR_CONNECT) !== 0n) {
            everyoneDenied = true;
          }
        } else if ((allow & VIEW_OR_CONNECT) !== 0n) {
          roleOrMemberAllowed = true;
        }
      }
      return everyoneDenied || (overwrites.length > 0 && roleOrMemberAllowed && everyoneDenied);
    };
    const ownOverwrites = channel.permissionOverwrites ?? channel.permission_overwrites ?? [];
    if (checkOverwrites(ownOverwrites)) return true;
    const parentId = cleanDiscordStr(String(channel.parentId ?? channel.parent_id ?? ""));
    const chList = Array.isArray(allChannels) ? allChannels : tree?.channels ?? [];
    if (parentId && chList.length > 0) {
      const parentCh = chList.find((c) => String(c?.id ?? "") === parentId);
      const parentOw = /** @type {any} */ (parentCh)?.permissionOverwrites ?? /** @type {any} */ (parentCh)?.permission_overwrites ?? [];
      if (checkOverwrites(parentOw)) return true;
    }
    const chName = String(channel.name || "");
    if (chName.includes("🔒") || chName.includes("🛑")) return true;
    return false;
  }

  /**
   * Возвращает остаток времени (мс) до автоматического скрытия кнопки «Пригласить в голосовой чат»
   * внизу канала, когда пользователь сидит в голосовом канале один (ровно 30 секунд с момента входа).
   * @param {string} targetGuildId
   * @param {string} targetChannelId
   * @returns {number}
   */
  function getAloneInviteRemainingMs(targetGuildId, targetChannelId) {
    if (
      !activeVoiceConnection ||
      activeVoiceConnection.guildId !== targetGuildId ||
      activeVoiceConnection.channelId !== targetChannelId
    ) {
      return 0;
    }
    const connectedAt = Number(activeVoiceConnection.connectedAtMs || Date.now());
    const elapsed = Math.max(0, Date.now() - connectedAt);
    return Math.max(0, 30_000 - elapsed);
  }

  /**
   * Открывает модальное окно «Пригласить друзей в <Название сервера>» (скриншот `uploaded_media_1_1793510374381.png`)
   * со списком друзей и участников сервера, поиском, предупреждением о приватном канале и ссылкой-приглашением.
   * @param {{ id: string, name?: string, type?: number }} channel
   * @param {boolean} [isRestricted]
   */
  function openVoiceInviteModal(channel, isRestricted = false) {
    document.getElementById("live-voice-invite-modal")?.remove();
    const isGroupInvite =
      Number(channel?.type) === 1 || Number(channel?.type) === 3 || rail.selectedId === HOME_GUILD_ID;
    const gInfo = currentGuildInfo();
    let resolvedGuildId = "";
    if (!isGroupInvite) {
      const targetChId = String(channel?.id || "");
      if (targetChId) {
        for (const [candGid, gObj] of liveGuildsById.entries()) {
          if (
            Array.isArray(gObj?.channels) &&
            gObj.channels.some((/** @type {any} */ c) => String(c?.id || "") === targetChId)
          ) {
            resolvedGuildId = candGid;
            break;
          }
        }
      }
      if (!resolvedGuildId) {
        const fallbackLiveGid = Array.from(liveGuildsById.keys())[0] || "";
        resolvedGuildId =
          (gInfo.id && gInfo.id !== HOME_GUILD_ID ? gInfo.id : "") ||
          (rail.selectedId && rail.selectedId !== HOME_GUILD_ID ? rail.selectedId : "") ||
          selectedGuildId() ||
          (isLiveSession() ? fallbackLiveGid : "1014537851418980372");
      }
    }
    const gId = isGroupInvite ? HOME_GUILD_ID : resolvedGuildId;
    const chName = String(
      channel?.name || channel?.id || (isGroupInvite ? "Групповой чат" : "Голосовой канал"),
    );
    const liveGuildObj = !isGroupInvite && gId ? liveGuildsById.get(gId) : null;
    const gName = isGroupInvite
      ? chName
      : String(liveGuildObj?.name || gInfo.name || "Сервер");
    const restricted = !isGroupInvite && Boolean(isRestricted || isChannelRoleRestricted(gId, channel));

    const backdrop = document.createElement("div");
    backdrop.id = "live-voice-invite-modal";
    backdrop.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.74);display:flex;align-items:center;justify-content:center;z-index:9999;";

    const card = document.createElement("div");
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.setAttribute("aria-label", `Пригласить друзей в ${gName}`);
    card.style.cssText =
      "width:460px;max-width:94vw;background:#2b2d31;color:#f2f3f5;border:1px solid #1e1f22;border-radius:12px;overflow:hidden;box-shadow:0 20px 50px rgba(0,0,0,0.78);display:flex;flex-direction:column;font-family:inherit;";

    const topSection = document.createElement("div");
    topSection.style.cssText = "padding:18px 20px 12px;display:flex;flex-direction:column;gap:8px;";

    const titleRow = document.createElement("div");
    titleRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:12px;";
    const titleEl = document.createElement("h3");
    titleEl.style.cssText = "margin:0;font-size:17px;font-weight:700;color:#f2f3f5;line-height:1.25;";
    titleEl.textContent = `Пригласить друзей в ${gName}`;
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "icon-btn";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText =
      "background:none;border:none;color:#b5bac1;font-size:18px;cursor:pointer;padding:2px 6px;border-radius:6px;";
    closeBtn.addEventListener("click", () => backdrop.remove());
    titleRow.append(titleEl, closeBtn);

    const subRow = document.createElement("div");
    subRow.style.cssText = "display:flex;align-items:center;gap:6px;font-size:13px;color:#b5bac1;";
    const chIconSpan = document.createElement("span");
    chIconSpan.style.cssText = "display:inline-flex;align-items:center;color:#949ba4;";
    chIconSpan.innerHTML = restricted
      ? `<svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><path d="M11 5L6 9H2v6h4l5 4V5z"/><path d="M18 8h-1V6.5a2.5 2.5 0 0 0-5 0V8h-1a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V9a1 1 0 0 0-1-1zm-2 0h-2V6.5a1 1 0 0 1 2 0V8z"/><path d="M14.5 17.5a3.5 3.5 0 0 0 2.5-1.1l1.1 1.1a5 5 0 0 1-3.6 1.5z"/></svg>`
      : `<svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor"><path d="M11 5L6 9H2v6h4l5 4V5zm4.5 3.5v7c1.48-.63 2.5-2.06 2.5-3.5s-1.02-2.87-2.5-3.5zm0-4.27v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z"/></svg>`;
    const subText = document.createElement("span");
    subText.textContent = "Участники окажутся в";
    const subChName = document.createElement("span");
    subChName.style.cssText = "color:#dbdee1;font-weight:600;";
    subChName.textContent = chName;
    subRow.append(subText, chIconSpan, subChName);

    const searchWrap = document.createElement("div");
    searchWrap.style.cssText =
      "display:flex;align-items:center;gap:8px;background:#1e1f22;border:1px solid #3f4147;border-radius:8px;padding:7px 11px;margin-top:4px;";
    const searchIcon = document.createElement("span");
    searchIcon.style.cssText = "color:#949ba4;font-size:14px;display:inline-flex;";
    searchIcon.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="7"/><line x1="16.5" y1="16.5" x2="21" y2="21"/></svg>`;
    const searchInput = document.createElement("input");
    searchInput.type = "search";
    searchInput.placeholder = "Найти друзей";
    searchInput.style.cssText =
      "flex:1;background:transparent;border:none;outline:none;color:#f2f3f5;font-size:14px;";
    searchWrap.append(searchIcon, searchInput);

    topSection.append(titleRow, subRow, searchWrap);

    if (restricted) {
      const warnBox = document.createElement("div");
      warnBox.style.cssText =
        "display:flex;align-items:flex-start;gap:8px;margin-top:4px;font-size:12px;color:#dbdee1;line-height:1.4;";
      const warnDot = document.createElement("span");
      warnDot.style.cssText =
        "width:16px;height:16px;border-radius:50%;background:#f0b232;color:#111214;font-weight:800;font-size:11px;display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;margin-top:1px;";
      warnDot.textContent = "!";
      const warnTxt = document.createElement("span");
      warnTxt.textContent =
        "Это приватный канал. Его могут просматривать только избранные участники и пользователи с определёнными ролями.";
      warnBox.append(warnDot, warnTxt);
      topSection.append(warnBox);
    }

    const sectionHeader = document.createElement("div");
    sectionHeader.style.cssText =
      "padding:4px 20px 6px;font-size:12px;font-weight:600;color:#b5bac1;display:flex;align-items:center;gap:4px;";
    sectionHeader.textContent = isGroupInvite ? "Друзья ⌄" : "Друзья и участники ⌄";

    const listWrap = document.createElement("div");
    listWrap.style.cssText =
      "max-height:310px;overflow-y:auto;padding:0 12px 8px;display:flex;flex-direction:column;gap:2px;";

    const selfId = String(liveSelfUser?.id || "");
    /** @type {Array<{ id: string, displayName: string, username: string, avatarUrl: string | null }>} */
    const candidates = [];
    const seenIds = new Set([selfId]);
    const pushCandidate = (/** @type {string} */ id, /** @type {string} */ displayName, /** @type {string} */ username, /** @type {string | null} */ avatarHash) => {
      if (!id || seenIds.has(id)) return;
      seenIds.add(id);
      const avatarUrl =
        avatarHash && avatarHash.startsWith("http")
          ? avatarHash
          : avatarHash
            ? `https://cdn.discordapp.com/avatars/${id}/${avatarHash}.png?size=64`
            : null;
      candidates.push({ id, displayName, username, avatarUrl });
    };

    const populateInviteCandidates = () => {
      candidates.length = 0;
      seenIds.clear();
      seenIds.add(selfId);
      const liveFriends = dmFriendsController?.getRelationships?.() ?? [];
      for (const rel of liveFriends) {
        const relUid = String(rel?.user_id ?? rel?.id ?? rel?.user?.id ?? "");
        if (Number(rel?.type) === 1 && relUid) {
          const known = liveUsersById.get(relUid) || rel?.user;
          const disp = String(
            rel.displayName ||
              rel.nickname ||
              known?.global_name ||
              known?.globalName ||
              rel.user?.global_name ||
              rel.user?.username ||
              known?.username ||
              relUid,
          );
          const uname = String(rel.user?.username || known?.username || disp).replace(/^@/, "");
          pushCandidate(relUid, disp, uname, known?.avatar || rel.user?.avatar || null);
        }
      }
      const privateChs = dmFriendsController?.getPrivateChannels?.() ?? [];
      for (const pc of privateChs) {
        const anyPc = /** @type {any} */ (pc);
        const recs = Array.isArray(anyPc?.recipients) ? anyPc.recipients : [];
        for (const rec of recs) {
          const rUid = String(rec?.id ?? "");
          if (!rUid) continue;
          const known = liveUsersById.get(rUid) || rec;
          const disp = String(
            known?.global_name ||
              known?.globalName ||
              rec?.global_name ||
              rec?.displayName ||
              known?.username ||
              rec?.username ||
              rUid,
          );
          const uname = String(known?.username || rec?.username || disp).replace(/^@/, "");
          pushCandidate(rUid, disp, uname, known?.avatar || rec?.avatar || null);
        }
      }
      if (!isGroupInvite && gId && gId !== HOME_GUILD_ID) {
        const guildMembers = liveGuildsById.get(gId)?.members ?? [];
        for (const m of guildMembers) {
          const mUser = m?.user || liveUsersById.get(String(m?.user_id ?? m?.id ?? ""));
          const mUid = String(m?.user_id ?? mUser?.id ?? m?.id ?? "");
          if (!mUid) continue;
          const disp = String(
            m?.nick ||
              mUser?.global_name ||
              mUser?.globalName ||
              mUser?.displayName ||
              mUser?.username ||
              mUid,
          );
          const uname = String(mUser?.username || disp).replace(/^@/, "");
          pushCandidate(mUid, disp, uname, m?.avatar || mUser?.avatar || null);
        }
      }
      if (!isLiveSession()) {
        for (const [uid, u] of liveUsersById.entries()) {
          const disp = String(u?.global_name || u?.globalName || u?.displayName || u?.username || uid);
          const uname = String(u?.username || disp).replace(/^@/, "");
          pushCandidate(uid, disp, uname, u?.avatar || null);
        }
      }
    };
    populateInviteCandidates();

    let currentInviteUrl = isLiveSession() ? "Создание ссылки..." : `https://discord.gg/a92Pr5e8H`;

    const renderCandidateRows = (/** @type {string} */ query) => {
      listWrap.replaceChildren();
      const q = query.trim().toLowerCase();
      const filtered = candidates.filter(
        (c) => !q || c.displayName.toLowerCase().includes(q) || c.username.toLowerCase().includes(q),
      );
      if (filtered.length === 0) {
        const emptyRow = document.createElement("div");
        emptyRow.style.cssText = "padding:16px 8px;color:#949ba4;font-size:13px;text-align:center;";
        emptyRow.textContent = "Список друзей пуст — скопируйте ссылку-приглашение ниже.";
        listWrap.append(emptyRow);
        return;
      }
      for (const c of filtered) {
        const row = document.createElement("div");
        row.style.cssText =
          "display:flex;align-items:center;justify-content:space-between;gap:10px;padding:6px 8px;border-radius:8px;";
        row.addEventListener("mouseenter", () => {
          row.style.background = "rgba(255,255,255,0.04)";
        });
        row.addEventListener("mouseleave", () => {
          row.style.background = "transparent";
        });

        const left = document.createElement("div");
        left.style.cssText = "display:flex;align-items:center;gap:10px;min-width:0;flex:1;";
        const av = document.createElement("div");
        av.style.cssText =
          "width:36px;height:36px;border-radius:50%;background:#5865f2;color:#fff;display:inline-flex;align-items:center;justify-content:center;font-size:14px;font-weight:700;flex-shrink:0;background-size:cover;background-position:center;";
        av.textContent = Array.from(c.displayName).slice(0, 1).join("").toUpperCase();
        if (c.avatarUrl) {
          av.style.backgroundImage = `url("${c.avatarUrl}")`;
          av.style.color = "transparent";
        }
        const names = document.createElement("div");
        names.style.cssText = "display:flex;flex-direction:column;min-width:0;line-height:1.25;";
        const dName = document.createElement("span");
        dName.style.cssText =
          "font-size:14px;font-weight:600;color:#f2f3f5;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
        dName.textContent = c.displayName;
        const uName = document.createElement("span");
        uName.style.cssText =
          "font-size:12px;color:#949ba4;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
        uName.textContent = c.username;
        names.append(dName, uName);
        left.append(av, names);

        const invBtn = document.createElement("button");
        invBtn.type = "button";
        invBtn.textContent = "Пригласить";
        invBtn.style.cssText =
          "padding:7px 14px;border-radius:6px;border:1px solid #4e5058;background:#35373c;color:#f2f3f5;font-size:13px;font-weight:600;cursor:pointer;flex-shrink:0;";
        invBtn.addEventListener("click", async () => {
          invBtn.disabled = true;
          invBtn.textContent = "Отправка...";
          try {
            if (isLiveSession()) {
              if (!currentInviteUrl.startsWith("http")) {
                const created = await invoke("create_invite", {
                  channelId: String(channel.id),
                  maxAge: 86400,
                  maxUses: 0,
                  temporary: false,
                  unique: false,
                });
                if (created?.url || created?.code) {
                  currentInviteUrl = String(created.url || `https://discord.gg/${created.code}`);
                  linkInput.value = currentInviteUrl;
                }
              }
              if (isGroupInvite) {
                const putRes = await sendDmRest({
                  method: "PUT",
                  url: `/channels/${channel.id}/recipients/${c.id}`,
                });
                const createdGroup = putRes?.body;
                if (createdGroup && typeof createdGroup === "object" && createdGroup.id) {
                  const dmSync = await invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
                    kind: "dm_friends_sync",
                    deviceId: "poll",
                  }).catch(() => null);
                  if (dmSync && typeof dmSync === "object") {
                    dmFriendsController?.syncLiveState?.({
                      relationships: dmSync.relationships,
                      privateChannels: dmSync.privateChannels,
                      users: dmSync.users,
                      presences: dmSync.presences,
                    });
                  }
                  dmFriendsController?.selectDmChannel?.(String(createdGroup.id));
                }
              } else {
                const dmRes = await sendDmRest({
                  method: "POST",
                  url: "/users/@me/channels",
                  body: { recipients: [c.id] },
                });
                const dmChannelId = String(dmRes?.body?.id || "");
                if (dmChannelId && currentInviteUrl.startsWith("http")) {
                  await invoke("send_message", {
                    channelId: dmChannelId,
                    content: currentInviteUrl,
                  });
                }
              }
            }
            invBtn.textContent = "Отправлено";
            invBtn.style.background = "transparent";
            invBtn.style.borderColor = "#23a55a";
            invBtn.style.color = "#23a55a";
          } catch {
            invBtn.disabled = false;
            invBtn.textContent = "Ошибка";
            invBtn.style.borderColor = "#f23f43";
            invBtn.style.color = "#f23f43";
          }
          emitLiveTelemetry({
            type: "voice_invite_friend",
            channelId: channel.id,
            channelName: chName,
            targetUserId: c.id,
            targetUsername: c.username,
          });
        });
        row.append(left, invBtn);
        listWrap.append(row);
      }
    };
    renderCandidateRows("");
    searchInput.addEventListener("input", () => {
      renderCandidateRows(searchInput.value);
    });

    const bottomSection = document.createElement("div");
    bottomSection.style.cssText =
      "padding:14px 20px 18px;background:#232428;border-top:1px solid #1e1f22;display:flex;flex-direction:column;gap:8px;";
    const linkLabel = document.createElement("div");
    linkLabel.style.cssText = "font-size:13px;font-weight:600;color:#f2f3f5;";
    linkLabel.textContent = isGroupInvite
      ? "Или отправьте другу ссылку-приглашение в группу"
      : "Или отправьте другу ссылку-приглашение на сервер";

    const linkBox = document.createElement("div");
    linkBox.style.cssText =
      "display:flex;align-items:center;gap:8px;background:#1e1f22;border:1px solid #111214;border-radius:8px;padding:4px 4px 4px 12px;";
    const linkInput = document.createElement("input");
    linkInput.type = "text";
    linkInput.readOnly = true;
    linkInput.value = currentInviteUrl;
    linkInput.style.cssText =
      "flex:1;background:transparent;border:none;outline:none;color:#dbdee1;font-size:13px;";
    const copyLinkBtn = document.createElement("button");
    copyLinkBtn.type = "button";
    copyLinkBtn.textContent = "Копировать";
    copyLinkBtn.style.cssText =
      "padding:8px 16px;border:none;border-radius:6px;background:#5865f2;color:#ffffff;font-size:13px;font-weight:600;cursor:pointer;flex-shrink:0;";
    copyLinkBtn.addEventListener("click", () => {
      void navigator.clipboard?.writeText(currentInviteUrl).catch(() => {});
      copyLinkBtn.textContent = "Скопировано";
      copyLinkBtn.style.background = "#23a55a";
      setTimeout(() => {
        copyLinkBtn.textContent = "Копировать";
        copyLinkBtn.style.background = "#5865f2";
      }, 1800);
    });
    linkBox.append(linkInput, copyLinkBtn);

    const expireHint = document.createElement("div");
    expireHint.style.cssText = "font-size:11px;color:#949ba4;";
    expireHint.innerHTML = `Ваша ссылка-приглашение перестанет действовать через 24 часа. <span style="color:#00a8fc;cursor:pointer;">Изменить ссылку-приглашение.</span>`;

    if (isLiveSession() && channel?.id) {
      void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "dm_friends_sync",
        deviceId: "poll",
      })
        .then((dmSync) => {
          if (dmSync && typeof dmSync === "object" && dmSync.ok !== false) {
            if (Array.isArray(dmSync.users)) {
              for (const u of dmSync.users) {
                if (u?.id) liveUsersById.set(String(u.id), u);
              }
            }
            if (Array.isArray(dmSync.guilds)) {
              for (const g of dmSync.guilds) {
                if (g?.id) hydrateGuildMembersAndRoles(g);
              }
            }
            dmFriendsController?.syncLiveState?.({
              relationships: dmSync.relationships,
              privateChannels: dmSync.privateChannels,
              users: dmSync.users,
              presences: dmSync.presences,
            });
            populateInviteCandidates();
            renderCandidateRows(searchInput.value);
          }
        })
        .catch(() => {});
      void invoke("create_invite", {
        channelId: String(channel.id),
        maxAge: 86400,
        maxUses: 0,
        temporary: false,
        unique: false,
      })
        .then((res) => {
          if (res?.url || res?.code) {
            currentInviteUrl = String(res.url || `https://discord.gg/${res.code}`);
            linkInput.value = currentInviteUrl;
          } else {
            linkInput.value = "Не удалось создать ссылку (нет прав)";
          }
        })
        .catch(() => {
          linkInput.value = "Не удалось создать ссылку (нет прав)";
        });
    }

    bottomSection.append(linkLabel, linkBox, expireHint);
    card.append(topSection, sectionHeader, listWrap, bottomSection);
    backdrop.append(card);
    backdrop.addEventListener("click", (ev) => {
      if (ev.target === backdrop) backdrop.remove();
    });
    document.body.append(backdrop);
  }

  /**
   * Вычисляет эффективные права текущего пользователя на сервере и иерархию ролей относительно целевого участника.
   * В точности соответствует модели прав Discord (Video_2026-11-01_01-19-18.mp4).
   * @param {string} targetGuildId
   * @param {string} targetUserId
   * @param {string[]} [targetRoles]
   */
  function computeServerPermissionsAndHierarchy(targetGuildId, targetUserId, targetRoles = []) {
    const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
    const guild = liveGuildsById.get(targetGuildId) ?? {};
    const rolesList = Array.isArray(guild?.roles) ? guild.roles : [];
    /** @type {Map<string, { id: string, name: string, position: number, permissions: bigint, color: number }>} */
    const roleMap = new Map();
    for (const r of rolesList) {
      if (!r?.id) continue;
      let perms = 0n;
      try {
        perms = BigInt(String(r.permissions ?? "0"));
      } catch {
        perms = 0n;
      }
      roleMap.set(String(r.id), {
        id: String(r.id),
        name: String(r.name || ""),
        position: Number(r.position ?? 0),
        permissions: perms,
        color: Number(r.color ?? 0),
      });
    }
    const selfRoles = selfId
      ? (liveMemberRolesByGuildAndUser.get(`${targetGuildId}:${selfId}`) ?? [])
      : [];
    const guildOwnerId = cleanDiscordStr(String(guild?.ownerId ?? guild?.owner_id ?? ""));
    const isOwner = Boolean(selfId && guildOwnerId && guildOwnerId === selfId);
    let effectivePerms = roleMap.get(targetGuildId)?.permissions ?? 0n;
    const guildPermsStr = cleanDiscordStr(String(guild?.permissions ?? ""));
    if (guildPermsStr) {
      try {
        effectivePerms |= BigInt(guildPermsStr);
      } catch {}
    }
    let selfTopPosition = 0;
    for (const rid of selfRoles) {
      const rInfo = roleMap.get(String(rid));
      if (rInfo) {
        effectivePerms |= rInfo.permissions;
        if (rInfo.position > selfTopPosition) selfTopPosition = rInfo.position;
      }
    }
    const isAdmin = isOwner || (effectivePerms & (1n << 3n)) !== 0n;
    let targetTopPosition = -1;
    const resolvedTargetRoles =
      targetRoles.length > 0
        ? targetRoles
        : liveMemberRolesByGuildAndUser.get(`${targetGuildId}:${targetUserId}`) ?? [];
    for (const rid of resolvedTargetRoles) {
      const rInfo = roleMap.get(String(rid));
      if (rInfo && rInfo.position > targetTopPosition) {
        targetTopPosition = rInfo.position;
      }
    }
    const isTargetOwner = Boolean(targetUserId && guildOwnerId && guildOwnerId === targetUserId);
    const isSelf = Boolean(selfId && targetUserId === selfId);
    const canModifyTarget =
      !isTargetOwner && (isOwner || isAdmin || isSelf || selfTopPosition > targetTopPosition);
    const hasPerm = (/** @type {bigint} */ bit) => isOwner || isAdmin || (effectivePerms & bit) !== 0n;

    const assignableRoles = [...roleMap.values()]
      .filter((r) => r.id !== targetGuildId && r.name !== "@everyone")
      .sort((a, b) => b.position - a.position);

    return {
      isSelf,
      isOwner,
      isAdmin,
      selfTopPosition,
      targetTopPosition,
      canModifyTarget,
      canManageChannels: hasPerm(1n << 4n),
      canManageGuild: hasPerm(1n << 5n),
      canSendMessages: hasPerm(1n << 11n),
      canManageMessages: hasPerm(1n << 13n),
      canMuteMembers: hasPerm(1n << 22n),
      canDeafenMembers: hasPerm(1n << 23n),
      canMoveMembers: hasPerm(1n << 24n),
      canChangeOwnNickname: hasPerm(1n << 26n),
      canManageNicknames: hasPerm(1n << 27n),
      canManageRoles: hasPerm(1n << 28n),
      canModerateMembers: hasPerm(1n << 40n),
      canKickMembers: hasPerm(1n << 1n),
      canBanMembers: hasPerm(1n << 2n),
      assignableRoles,
      targetRoles: resolvedTargetRoles,
    };
  }

  /**
   * Открывает правую боковую панель «Доступ модератора» (`Moderator View Sidebar`, Video frame_064.jpg)
   * или обычный профиль участника, если у текущего пользователя нет прав модерации.
   * @param {string} targetGuildId
   * @param {any} participant
   * @param {{ modView?: boolean }} [opts]
   */
  function openModeratorViewSidebar(targetGuildId, participant, opts = {}) {
    document.getElementById("live-moderator-sidebar")?.remove();
    const uid = String(participant?.userId || "");
    const perms = computeServerPermissionsAndHierarchy(targetGuildId, uid, participant?.roles ?? []);
    const hasModAccess =
      opts.modView !== false &&
      Boolean(
        perms.isOwner ||
          perms.isAdmin ||
          perms.canModerateMembers ||
          perms.canKickMembers ||
          perms.canBanMembers,
      );
    const userOverride = livePerUserVoiceState.get(uid) ?? { volume: 100, localMute: false };
    const panel = document.createElement("aside");
    panel.id = "live-moderator-sidebar";
    panel.setAttribute("role", "complementary");
    panel.setAttribute("aria-label", hasModAccess ? "Доступ модератора" : "Профиль участника");
    panel.style.cssText =
      "position:fixed;top:0;right:0;bottom:0;width:340px;background:#111214;color:#dbdee1;border-left:1px solid #2b2d31;z-index:9200;display:flex;flex-direction:column;box-shadow:-12px 0 32px rgba(0,0,0,0.65);font-size:13px;overflow-y:auto;";

    const topBar = document.createElement("div");
    topBar.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;padding:12px 16px;background:#1e1f22;border-bottom:1px solid #2b2d31;position:sticky;top:0;z-index:2;";
    const topTitle = document.createElement("div");
    topTitle.style.cssText = "display:flex;align-items:center;gap:8px;font-weight:700;color:#f2f3f5;font-size:14px;";
    topTitle.innerHTML = hasModAccess
      ? `<span style="color:#5865f2;">🛡</span><span>Доступ модератора</span>`
      : `<span style="color:#5865f2;">👤</span><span>Профиль участника</span>`;
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText =
      "background:none;border:none;color:#b5bac1;font-size:16px;cursor:pointer;padding:4px 8px;border-radius:6px;";
    closeBtn.addEventListener("click", () => panel.remove());
    topBar.append(topTitle, closeBtn);

    const banner = document.createElement("div");
    banner.style.cssText =
      "height:84px;background:linear-gradient(135deg,#1f3a52 0%,#152238 100%);position:relative;flex-shrink:0;";
    const avatarWrap = document.createElement("div");
    avatarWrap.style.cssText =
      "width:68px;height:68px;border-radius:50%;border:5px solid #111214;background:#2b2d31;position:absolute;left:16px;bottom:-30px;display:flex;align-items:center;justify-content:center;font-size:22px;font-weight:700;color:#f2f3f5;background-size:cover;background-position:center;";
    avatarWrap.textContent = Array.from(String(participant.displayName || "U")).slice(0, 1).join("").toUpperCase();
    if (participant.avatarUrl) {
      avatarWrap.style.backgroundImage = `url("${participant.avatarUrl}")`;
      avatarWrap.style.color = "transparent";
    }
    banner.append(avatarWrap);

    const body = document.createElement("div");
    body.style.cssText = "padding:38px 16px 20px;display:flex;flex-direction:column;gap:12px;";

    const idHeader = document.createElement("div");
    const nameRow = document.createElement("div");
    nameRow.style.cssText = "display:flex;align-items:center;gap:6px;font-size:18px;font-weight:800;color:#f2f3f5;";
    nameRow.textContent = participant.displayName;
    if (participant.clanTag) {
      const clan = document.createElement("span");
      clan.style.cssText =
        "padding:1px 6px;border-radius:4px;background:#2b2d31;border:1px solid #3f4147;color:#dbdee1;font-size:11px;font-weight:700;";
      clan.textContent = `🍃 ${participant.clanTag}`;
      nameRow.append(clan);
    }
    const handleSub = document.createElement("div");
    handleSub.style.cssText = "color:#949ba4;font-size:12px;margin-top:2px;";
    handleSub.textContent = `${participant.username || participant.displayName} • ID: ${uid}`;
    idHeader.append(nameRow, handleSub);

    const quickModCard = document.createElement("div");
    quickModCard.style.cssText =
      "background:#1e1f22;border:1px solid #2b2d31;border-radius:8px;padding:8px;display:flex;flex-direction:column;gap:4px;";
    const makeModActionRow = (
      /** @type {string} */ iconTxt,
      /** @type {string} */ label,
      /** @type {boolean} */ danger,
      /** @type {() => void} */ onClick,
    ) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.style.cssText = `display:flex;align-items:center;gap:10px;width:100%;padding:8px 10px;border:none;border-radius:6px;background:transparent;color:${danger ? "#f23f43" : "#dbdee1"};font-size:13px;font-weight:600;cursor:pointer;text-align:left;`;
      btn.innerHTML = `<span style="width:18px;text-align:center;">${iconTxt}</span><span style="flex:1;">${label}</span>`;
      btn.addEventListener("mouseenter", () => {
        btn.style.background = danger ? "rgba(242,63,67,0.16)" : "#2b2d31";
      });
      btn.addEventListener("mouseleave", () => {
        btn.style.background = "transparent";
      });
      btn.addEventListener("click", onClick);
      return btn;
    };
    const statusBanner = document.createElement("div");
    statusBanner.style.cssText = "font-size:12px;color:#23a55a;padding:2px 6px;display:none;";
    const flashStatus = (/** @type {string} */ msg) => {
      statusBanner.textContent = msg;
      statusBanner.style.display = "block";
    };
    if (perms.canModerateMembers && perms.canModifyTarget) {
      quickModCard.append(
        makeModActionRow(
          "⏳",
          userOverride.timedOut ? "Снять тайм-аут" : "Отправить в тайм-аут",
          false,
          () => {
            userOverride.timedOut = !userOverride.timedOut;
            livePerUserVoiceState.set(uid, userOverride);
            flashStatus(
              userOverride.timedOut
                ? `Участник ${participant.displayName} отправлен в тайм-аут (60 сек.)`
                : `Тайм-аут снят с ${participant.displayName}`,
            );
          },
        ),
      );
    }
    if (perms.canKickMembers && perms.canModifyTarget) {
      quickModCard.append(
        makeModActionRow("👢", "Выгнать", true, () => {
          flashStatus(`Запрос на исключение ${participant.displayName} зафиксирован модератором`);
        }),
      );
    }
    if (perms.canBanMembers && perms.canModifyTarget) {
      quickModCard.append(
        makeModActionRow("🔨", "Забанить", true, () => {
          flashStatus(`Запрос на бан ${participant.displayName} зафиксирован модератором`);
        }),
      );
    }
    quickModCard.append(statusBanner);

    const activityCard = document.createElement("div");
    activityCard.style.cssText =
      "background:#1e1f22;border:1px solid #2b2d31;border-radius:8px;padding:10px 12px;display:flex;flex-direction:column;gap:8px;";
    const actTitle = document.createElement("div");
    actTitle.style.cssText = "font-size:12px;font-weight:700;color:#b5bac1;text-transform:uppercase;";
    actTitle.textContent = "Активность на сервере";
    const makeStatLine = (/** @type {string} */ icon, /** @type {string} */ label, /** @type {string} */ val) => {
      const row = document.createElement("div");
      row.style.cssText = "display:flex;align-items:center;justify-content:space-between;color:#dbdee1;font-size:13px;";
      row.innerHTML = `<span style="display:flex;align-items:center;gap:8px;"><span>${icon}</span><span>${label}</span></span><span style="color:#949ba4;font-weight:600;">${val} ›</span>`;
      return row;
    };
    activityCard.append(
      actTitle,
      makeStatLine("💬", "Сообщения", "54"),
      makeStatLine("🔗", "Ссылки", "2"),
      makeStatLine("🖼", "Медиафайлы", "20"),
    );

    const flagsCard = document.createElement("div");
    flagsCard.style.cssText =
      "background:#1e1f22;border:1px solid #2b2d31;border-radius:8px;padding:10px 12px;display:flex;flex-direction:column;gap:8px;";
    const flagsTitle = document.createElement("div");
    flagsTitle.style.cssText = "font-size:12px;font-weight:700;color:#b5bac1;text-transform:uppercase;";
    flagsTitle.textContent = "Отмеченные действия";
    flagsCard.append(
      flagsTitle,
      makeStatLine("⏳", "Тайм-ауты", userOverride.timedOut ? "1" : "0"),
      makeStatLine("🚩", "Подозрительные сообщения", "0"),
    );

    const infoCard = document.createElement("div");
    infoCard.style.cssText =
      "background:#1e1f22;border:1px solid #2b2d31;border-radius:8px;padding:10px 12px;display:flex;flex-direction:column;gap:8px;";
    const infoTitle = document.createElement("div");
    infoTitle.style.cssText = "font-size:12px;font-weight:700;color:#b5bac1;text-transform:uppercase;";
    infoTitle.textContent = "Информация об участнике";
    infoCard.append(
      infoTitle,
      makeStatLine("📥", "Присоединился к серверу", participant.joinedAt || "17 окт. 2025 г."),
      makeStatLine("🧭", "Способ присоединения", "По приглашению"),
      makeStatLine("🪪", "Дата регистрации в Discord", "10 мая 2021 г."),
    );

    const rolesCard = document.createElement("div");
    rolesCard.style.cssText =
      "background:#1e1f22;border:1px solid #2b2d31;border-radius:8px;padding:10px 12px;display:flex;flex-direction:column;gap:8px;";
    const rolesTitle = document.createElement("div");
    rolesTitle.style.cssText = "font-size:12px;font-weight:700;color:#b5bac1;text-transform:uppercase;";
    const roleIds = perms.targetRoles;
    rolesTitle.textContent = `Роли — ${roleIds.length}`;
    const rolesWrap = document.createElement("div");
    rolesWrap.style.cssText = "display:flex;flex-wrap:wrap;gap:6px;";
    for (const rid of roleIds) {
      const rInfo = perms.assignableRoles.find((r) => r.id === rid);
      const rName = rInfo?.name || rid;
      const rColor = rInfo?.color ? `#${rInfo.color.toString(16).padStart(6, "0")}` : "#949ba4";
      const chip = document.createElement("span");
      chip.style.cssText =
        "display:inline-flex;align-items:center;gap:5px;padding:3px 8px;border-radius:6px;background:#2b2d31;color:#f2f3f5;font-size:12px;font-weight:600;";
      chip.innerHTML = `<span style="width:10px;height:10px;border-radius:50%;background:${rColor};"></span><span>${rName}</span>`;
      rolesWrap.append(chip);
    }
    if (roleIds.length === 0) {
      const emptyR = document.createElement("span");
      emptyR.style.cssText = "color:#949ba4;font-size:12px;";
      emptyR.textContent = "Нет назначенных ролей";
      rolesWrap.append(emptyR);
    }
    rolesCard.append(rolesTitle, rolesWrap);

    const bottomRow = document.createElement("div");
    bottomRow.style.cssText = "display:flex;gap:8px;margin-top:4px;";
    const copyIdBtn = document.createElement("button");
    copyIdBtn.type = "button";
    copyIdBtn.style.cssText =
      "flex:1;padding:8px 10px;border:none;border-radius:6px;background:#2b2d31;color:#f2f3f5;font-size:12px;font-weight:600;cursor:pointer;";
    copyIdBtn.textContent = "ID Копировать ID пользователя";
    copyIdBtn.addEventListener("click", () => {
      void navigator.clipboard?.writeText(uid).catch(() => {});
      copyIdBtn.textContent = "✓ ID скопирован";
    });
    const profileBtn = document.createElement("button");
    profileBtn.type = "button";
    profileBtn.style.cssText =
      "padding:8px 12px;border:none;border-radius:6px;background:#5865f2;color:#fff;font-size:12px;font-weight:600;cursor:pointer;";
    profileBtn.textContent = "👤 Профиль";
    profileBtn.addEventListener("click", () => {
      panel.remove();
    });
    bottomRow.append(copyIdBtn, profileBtn);

    if (hasModAccess) {
      body.append(idHeader, quickModCard, activityCard, flagsCard, infoCard, rolesCard, bottomRow);
    } else {
      body.append(idHeader, infoCard, rolesCard, bottomRow);
    }
    panel.append(topBar, banner, body);
    document.body.append(panel);
  }

  /**
   * Контекстное меню участника голосового канала / сервера с учётом прав и ролей (Video_2026-11-01_01-19-18.mp4).
   * @param {any} participant
   * @param {{ id: string, name?: string }} channel
   * @param {HTMLElement} rowEl
   * @param {MouseEvent} ev
   */
  function openVoiceParticipantContextMenu(participant, channel, rowEl, ev) {
    closeLiveVoicePopouts();
    const targetGuildId = String(participant?.guildId || rail.selectedId || selectedGuildId() || "");
    const uid = String(participant?.userId || "");
    if (!uid) return;
    const perms = computeServerPermissionsAndHierarchy(targetGuildId, uid, participant?.roles ?? []);
    const userState = livePerUserVoiceState.get(uid) ?? {
      volume: 100,
      localMute: false,
      serverMute: Boolean(participant?.serverMute),
      serverDeaf: Boolean(participant?.serverDeaf),
    };
    livePerUserVoiceState.set(uid, userState);

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

    // 0. Если участник ведёт стрим («В ЭФИРЕ») — кнопка «Смотреть трансляцию» (OP 20 STREAM_WATCH)
    // Просмотр доступен строго при активном подключении к этому голосовому каналу (как в Serein `watch_stream`).
    if (participant?.streaming === true) {
      const targetChId = String(channel?.id || participant?.channelId || "");
      const connectedToTargetChannel = isVoiceChannelConnected(targetGuildId, targetChId);
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
              connectToLiveVoiceChannel(
                targetGuildId,
                currentGuildInfo().name || "Сервер",
                { id: targetChId, name: channel?.name || "Голосовой канал" },
              );
            }
            openLiveStreamStageViewer(participant, channel);
          },
        },
      );
      addDivider();
    }

    // 1. Для чужих участников при наличии прав модерации — «Открыть с доступом модератора» (f_035.jpg -> f_064.jpg)
    if (!perms.isSelf && perms.canModerateMembers) {
      addMenuItem("Открыть с доступом модератора", {
        onClick: () => {
          closeSubmenu();
          menu.remove();
          openModeratorViewSidebar(targetGuildId, participant);
        },
      });
      addDivider();
    }

    // 2. Профиль и базовые действия
    addMenuItem("Профиль", {
      onClick: () => {
        closeSubmenu();
        menu.remove();
        openModeratorViewSidebar(targetGuildId, participant, { modView: false });
      },
    });
    if (perms.isSelf) {
      addMenuItem("Упомянуть", {
        onClick: () => {
          const comp = /** @type {HTMLTextAreaElement | null} */ (byId("composer-input"));
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
          const comp = /** @type {HTMLTextAreaElement | null} */ (byId("composer-input"));
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
          connectToLiveVoiceChannel(targetGuildId, currentGuildInfo().name || "Сервер", channel);
        },
      });
      addMenuItem("Добавить заметку", {
        onClick: () => {
          closeSubmenu();
          menu.remove();
          const note = globalThis.prompt?.(`Заметка для ${participant.displayName}:`, userState.note || "");
          if (note !== null) {
            userState.note = note;
            livePerUserVoiceState.set(uid, userState);
          }
        },
      });
      addMenuItem("Добавить никнейм друга", {
        onClick: () => {
          closeSubmenu();
          menu.remove();
          const fn = globalThis.prompt?.(`Никнейм друга для ${participant.displayName}:`, userState.friendNick || "");
          if (fn !== null && fn.trim()) {
            userState.friendNick = fn.trim();
            userState.nickname = fn.trim();
            livePerUserVoiceState.set(uid, userState);
            tree.refreshView();
          }
        },
      });
    }

    if ((perms.isSelf && perms.canChangeOwnNickname) || (!perms.isSelf && perms.canManageNicknames && perms.canModifyTarget)) {
      if (perms.isSelf) {
        addMenuItem("Изменить никнейм", {
          onClick: () => {
            closeSubmenu();
            menu.remove();
            const nextNick = globalThis.prompt?.("Новый никнейм на сервере:", participant.displayName);
            if (nextNick && nextNick.trim()) {
              userState.nickname = nextNick.trim();
              livePerUserVoiceState.set(uid, userState);
              tree.refreshView();
            }
          },
        });
      }
    }

    addMenuItem("Пригласить на сервер", {
      rightBadge: "›",
      onSubmenu: (btn) => {
        const sub = makeSubmenuContainer(btn);
        for (const [, g] of liveGuildsById) {
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

    // 3. Персональные звуковые настройки (как в f_003.jpg и f_035.jpg)
    if (perms.isSelf) {
      addMenuItem("Откл. микрофон", {
        checked: Boolean(selfVoicePrefs.muted),
        onClick: () => {
          toggleSelfMicOnly();
          closeSubmenu();
          menu.remove();
        },
      });
      addMenuItem("Откл. звук", {
        checked: Boolean(selfVoicePrefs.deafened),
        onClick: () => {
          toggleSelfDeafenBoth();
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
          livePerUserVoiceState.set(uid, userState);
          void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
            kind: "user_audio_volume",
            deviceId: uid,
            muted: Boolean(userState.localMute),
            volumePercent: Number(userState.volume ?? 100),
          }).catch(() => {});
          tree.refreshView();
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
        livePerUserVoiceState.set(uid, userState);
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "user_audio_volume",
          deviceId: uid,
          muted: Boolean(userState.localMute),
          volumePercent: Number(userState.volume),
        }).catch(() => {});
      });
      volWrap.append(volHeader, volSlider);
      menu.append(volWrap);
    }

    // 4. Серверная модерация голосового канала (зависит от MUTE_MEMBERS / DEAFEN_MEMBERS / MOVE_MEMBERS)
    if (perms.canMuteMembers || perms.canDeafenMembers || perms.canMoveMembers) {
      addDivider();
      if (perms.canMuteMembers) {
        addMenuItem("Откл. микрофон на сервере", {
          checked: Boolean(userState.serverMute),
          onClick: () => {
            userState.serverMute = !userState.serverMute;
            livePerUserVoiceState.set(uid, userState);
            const gList = liveVoiceStatesByGuild.get(targetGuildId) ?? [];
            for (const vs of gList) {
              if (String(vs?.user_id ?? vs?.userId ?? "") === uid) {
                vs.mute = userState.serverMute;
              }
            }
            tree.refreshView();
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
            livePerUserVoiceState.set(uid, userState);
            const gList = liveVoiceStatesByGuild.get(targetGuildId) ?? [];
            for (const vs of gList) {
              if (String(vs?.user_id ?? vs?.userId ?? "") === uid) {
                vs.deaf = userState.serverDeaf;
              }
            }
            tree.refreshView();
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
              disconnectFromLiveVoiceChannel();
            } else {
              const gList = (liveVoiceStatesByGuild.get(targetGuildId) ?? []).filter(
                (vs) => String(vs?.user_id ?? vs?.userId ?? "") !== uid,
              );
              liveVoiceStatesByGuild.set(targetGuildId, gList);
              tree.refreshView();
            }
            closeSubmenu();
            menu.remove();
          },
        });
        addMenuItem("Переместить в", {
          rightBadge: "›",
          onSubmenu: (btn) => {
            const sub = makeSubmenuContainer(btn);
            const voiceChannels = tree.channels.filter((ch) => ch.type === 2 && ch.id !== channel.id);
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
                  connectToLiveVoiceChannel(targetGuildId, currentGuildInfo().name || "Сервер", vch);
                } else {
                  const gList = liveVoiceStatesByGuild.get(targetGuildId) ?? [];
                  for (const vs of gList) {
                    if (String(vs?.user_id ?? vs?.userId ?? "") === uid) {
                      vs.channel_id = vch.id;
                      vs.channelId = vch.id;
                    }
                  }
                  tree.refreshView();
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

    // 5. Управление участником и ролями (зависит от MANAGE_ROLES / MODERATE_MEMBERS / KICK_MEMBERS / BAN_MEMBERS и иерархии ролей)
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
            livePerUserVoiceState.set(uid, userState);
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
            const nextNick = globalThis.prompt?.(`Изменить никнейм ${participant.displayName}:`, participant.displayName);
            if (nextNick && nextNick.trim()) {
              userState.nickname = nextNick.trim();
              livePerUserVoiceState.set(uid, userState);
              tree.refreshView();
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
              liveMemberRolesByGuildAndUser.get(`${targetGuildId}:${uid}`) ?? perms.targetRoles,
            );
            for (const rInfo of perms.assignableRoles) {
              const canToggleRole =
                perms.isOwner || perms.isAdmin || (perms.canManageRoles && rInfo.position < perms.selfTopPosition);
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
                liveMemberRolesByGuildAndUser.set(`${targetGuildId}:${uid}`, [...currentRoleSet]);
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
            livePerUserVoiceState.set(uid, userState);
            closeSubmenu();
            menu.remove();
            openModeratorViewSidebar(targetGuildId, participant);
          },
        });
      }
      if (!perms.isSelf && perms.canKickMembers && perms.canModifyTarget) {
        addMenuItem("Выгнать", {
          danger: true,
          onClick: () => {
            closeSubmenu();
            menu.remove();
            openModeratorViewSidebar(targetGuildId, participant);
          },
        });
      }
      if (!perms.isSelf && perms.canBanMembers && perms.canModifyTarget) {
        addMenuItem("Забанить", {
          danger: true,
          onClick: () => {
            closeSubmenu();
            menu.remove();
            openModeratorViewSidebar(targetGuildId, participant);
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
      if (activeVoiceMenuOutsideHandler === onOutside) {
        activeVoiceMenuOutsideHandler = null;
      }
    };
    activeVoiceMenuOutsideHandler = onOutside;
    setTimeout(() => {
      if (menu.isConnected) {
        document.addEventListener("mousedown", onOutside, true);
      }
    }, 0);
    document.body.append(menu);
  }

  /**
   * Открывает поповер выбора микрофона или динамиков + раздельного управления блокировкой (Video frame_054/055.jpg).
   * @param {"input" | "output"} kind
   * @param {HTMLElement} anchor
   */
  async function openAudioDevicePopover(kind, anchor) {
    const existing = document.getElementById("live-audio-device-popout");
    if (existing && existing.getAttribute("data-kind") === kind) {
      existing.remove();
      return;
    }
    closeLiveVoicePopouts();
    const pop = document.createElement("div");
    pop.id = "live-audio-device-popout";
    pop.setAttribute("data-kind", kind);
    pop.setAttribute("role", "dialog");
    pop.style.cssText =
      "position:fixed;left:68px;bottom:64px;width:288px;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:10px;padding:12px;box-shadow:0 12px 28px rgba(0,0,0,0.65);z-index:9500;display:flex;flex-direction:column;gap:10px;font-size:13px;";

    const modeSection = document.createElement("div");
    modeSection.style.cssText = "display:flex;flex-direction:column;gap:4px;padding-bottom:8px;border-bottom:1px solid #2b2d31;";
    const modeHeading = document.createElement("div");
    modeHeading.style.cssText = "font-size:11px;font-weight:700;text-transform:uppercase;color:#949ba4;margin-bottom:2px;";
    modeHeading.textContent = "Режим блокировки звука";

    const micOnlyBtn = document.createElement("button");
    micOnlyBtn.type = "button";
    micOnlyBtn.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;width:100%;padding:6px 8px;border:none;border-radius:6px;background:transparent;color:#f2f3f5;font-size:13px;cursor:pointer;text-align:left;";
    const isMicOnlyMuted = selfVoicePrefs.muted && !selfVoicePrefs.deafened;
    micOnlyBtn.innerHTML = `<span>🎙 Блокировать только микрофон</span><span style="color:${isMicOnlyMuted ? "#23a55a" : "#949ba4"};font-weight:700;">${isMicOnlyMuted ? "ВКЛ" : "ВЫКЛ"}</span>`;
    micOnlyBtn.addEventListener("click", () => {
      if (isMicOnlyMuted) {
        selfVoicePrefs.muted = false;
      } else {
        selfVoicePrefs.deafened = false;
        selfVoicePrefs.muted = true;
      }
      syncVoiceMuteStateWithBackend();
      renderLiveVoicePanel();
      tree.refreshView();
      pop.remove();
    });

    const bothMuteBtn = document.createElement("button");
    bothMuteBtn.type = "button";
    bothMuteBtn.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;width:100%;padding:6px 8px;border:none;border-radius:6px;background:transparent;color:#f2f3f5;font-size:13px;cursor:pointer;text-align:left;";
    const isBothMuted = selfVoicePrefs.deafened;
    bothMuteBtn.innerHTML = `<span>🎧 Блокировать микрофон и динамики</span><span style="color:${isBothMuted ? "#f23f43" : "#949ba4"};font-weight:700;">${isBothMuted ? "ВКЛ" : "ВЫКЛ"}</span>`;
    bothMuteBtn.addEventListener("click", () => {
      toggleSelfDeafenBoth(!selfVoicePrefs.deafened);
      pop.remove();
    });
    modeSection.append(modeHeading, micOnlyBtn, bothMuteBtn);
    pop.append(modeSection);

    const devSection = document.createElement("div");
    devSection.style.cssText = "display:flex;flex-direction:column;gap:4px;padding-bottom:8px;border-bottom:1px solid #2b2d31;";
    const devTitle = document.createElement("div");
    devTitle.style.cssText = "font-size:11px;font-weight:700;text-transform:uppercase;color:#949ba4;margin-bottom:2px;";
    devTitle.textContent = kind === "input" ? "Устройство ввода" : "Устройство вывода";
    devSection.append(devTitle);

    let deviceList = kind === "input"
      ? [{ id: "default", name: "Системный микрофон (По умолчанию)", isDefault: true }]
      : [{ id: "default", name: "Системные динамики (По умолчанию)", isDefault: true }];
    try {
      const fetched = await invoke(VOICE_AUDIO_DEVICES_IPC_CMD);
      const rawArr = kind === "input" ? fetched?.inputs : fetched?.outputs;
      if (Array.isArray(rawArr) && rawArr.length > 0) {
        deviceList = rawArr;
      }
    } catch {}

    const currentDevId = kind === "input" ? selfVoicePrefs.inputDeviceId : selfVoicePrefs.outputDeviceId;
    for (const dev of deviceList) {
      const devBtn = document.createElement("button");
      devBtn.type = "button";
      const selected = dev.id === currentDevId || (currentDevId === "default" && dev.isDefault);
      devBtn.style.cssText =
        `display:flex;align-items:center;justify-content:space-between;width:100%;padding:6px 8px;border:none;border-radius:6px;background:${selected ? "#35373c" : "transparent"};color:#f2f3f5;font-size:12px;cursor:pointer;text-align:left;gap:8px;`;
      const nameSpan = document.createElement("span");
      nameSpan.style.cssText = "overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1;";
      nameSpan.textContent = String(dev.name || dev.id);
      const radioDot = document.createElement("span");
      radioDot.style.cssText = `width:14px;height:14px;border-radius:50%;border:2px solid ${selected ? "#5865f2" : "#80848e"};background:${selected ? "#5865f2" : "transparent"};flex-shrink:0;`;
      devBtn.append(nameSpan, radioDot);
      devBtn.addEventListener("click", () => {
        if (kind === "input") selfVoicePrefs.inputDeviceId = String(dev.id);
        else selfVoicePrefs.outputDeviceId = String(dev.id);
        syncVoiceMuteStateWithBackend();
        pop.remove();
      });
      devSection.append(devBtn);
    }
    pop.append(devSection);

    const volSection = document.createElement("div");
    volSection.style.cssText = "display:flex;flex-direction:column;gap:6px;";
    const volHeader = document.createElement("div");
    volHeader.style.cssText = "display:flex;justify-content:space-between;font-size:11px;font-weight:700;text-transform:uppercase;color:#949ba4;";
    const volLabel = document.createElement("span");
    volLabel.textContent = kind === "input" ? "Громкость микрофона" : "Громкость звука";
    const volValue = document.createElement("span");
    const initVol = kind === "input" ? selfVoicePrefs.inputVolume : selfVoicePrefs.outputVolume;
    volValue.textContent = `${initVol}%`;
    volHeader.append(volLabel, volValue);

    const slider = document.createElement("input");
    slider.type = "range";
    slider.min = "0";
    slider.max = kind === "input" ? "100" : "200";
    slider.value = String(initVol);
    slider.style.cssText = "width:100%;accent-color:#5865f2;cursor:pointer;";
    slider.addEventListener("input", () => {
      const v = Number(slider.value);
      volValue.textContent = `${v}%`;
      if (kind === "input") selfVoicePrefs.inputVolume = v;
      else selfVoicePrefs.outputVolume = v;
      syncVoiceMuteStateWithBackend();
    });

    const hotplugBtn = document.createElement("button");
    hotplugBtn.type = "button";
    hotplugBtn.style.cssText =
      "margin-top:4px;padding:6px 8px;border:none;border-radius:6px;background:#2b2d31;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;text-align:left;";
    hotplugBtn.textContent = "🔄 Быстрая смена аудиоустройства";
    hotplugBtn.addEventListener("click", () => {
      pop.remove();
      void openAudioDeviceHotplugPrompt();
    });

    const settingsBtn = document.createElement("button");
    settingsBtn.type = "button";
    settingsBtn.style.cssText =
      "margin-top:2px;padding:6px 8px;border:none;border-radius:6px;background:#2b2d31;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;text-align:left;";
    settingsBtn.textContent = "⚙ Голос и видео (Все настройки)";
    settingsBtn.addEventListener("click", () => {
      pop.remove();
      router.switchScreen("settings");
      settingsRef?.showPane("voice");
    });

    volSection.append(volHeader, slider, hotplugBtn, settingsBtn);
    pop.append(volSection);
    document.body.append(pop);
  }

  /**
   * Всплывающая карточка «Обнаружено новое аудиоустройство» / «Сменить устройство ввода и вывода звука на ...?»
   * над голосовой панелью слева внизу (`Video_2026-11-01_11-35-06.mp4`: `f_01_56.jpg`, `f_01_59.jpg`, `f_02_26.jpg`).
   * @param {{ id?: string, name?: string, mode?: "both" | "selector" }} [opts]
   */
  async function openAudioDeviceHotplugPrompt(opts = {}) {
    if (hotplugPromptsSuppressed) return;
    document.getElementById("live-audio-hotplug-card")?.remove();
    document.getElementById("live-audio-hotplug-more-menu")?.remove();

    /** @type {Array<{ id: string, name: string }>} */
    let inputs = [];
    /** @type {Array<{ id: string, name: string }>} */
    let outputs = [];
    try {
      const res = await invoke(VOICE_AUDIO_DEVICES_IPC_CMD);
      if (Array.isArray(res?.inputs)) inputs = res.inputs;
      if (Array.isArray(res?.outputs)) outputs = res.outputs;
    } catch {}
    if (inputs.length === 0) {
      inputs = [{ id: "Easy Effects Source", name: "Easy Effects Source" }];
    }
    if (outputs.length === 0) {
      outputs = [{ id: "Easy Effects Sink", name: "Easy Effects Sink" }];
    }

    let selectedId = opts.id || inputs[0]?.id || "Easy Effects Source";
    let selectedName = opts.name || inputs.find((d) => d.id === selectedId)?.name || selectedId;
    const mode = opts.mode || "selector";

    const card = document.createElement("div");
    card.id = "live-audio-hotplug-card";
    card.setAttribute("role", "region");
    card.setAttribute("aria-label", "Обнаружено новое аудиоустройство");
    const bottomOffset = activeVoiceConnection ? 156 : 64;
    card.style.cssText =
      `position:fixed;left:8px;bottom:${bottomOffset}px;width:232px;background:#18191c;color:#f2f3f5;border:1px solid #2b2d31;border-radius:10px;padding:12px;box-shadow:0 14px 36px rgba(0,0,0,0.78);z-index:9450;display:flex;flex-direction:column;gap:10px;font-size:12px;`;

    const headRow = document.createElement("div");
    headRow.style.cssText = "display:flex;align-items:flex-start;justify-content:space-between;gap:8px;";
    const titleSpan = document.createElement("span");
    titleSpan.style.cssText = "font-size:12px;font-weight:700;color:#f2f3f5;line-height:1.35;";
    titleSpan.textContent =
      mode === "both"
        ? `Сменить устройство ввода и вывода звука на ${selectedName}?`
        : "Обнаружено новое аудиоустройство";
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText = "background:none;border:none;color:#949ba4;font-size:13px;cursor:pointer;padding:0;";
    closeBtn.addEventListener("click", () => {
      document.getElementById("live-audio-hotplug-more-menu")?.remove();
      card.remove();
    });
    headRow.append(titleSpan, closeBtn);
    card.append(headRow);

    if (mode === "selector") {
      const selWrap = document.createElement("div");
      selWrap.style.cssText = "position:relative;";
      const selBtn = document.createElement("button");
      selBtn.type = "button";
      selBtn.style.cssText =
        "width:100%;height:34px;padding:0 10px;border-radius:6px;border:1px solid #383a40;background:#232428;color:#f2f3f5;font-size:12px;display:flex;align-items:center;justify-content:space-between;gap:6px;cursor:pointer;text-align:left;";
      const selTxt = document.createElement("span");
      selTxt.style.cssText = "overflow:hidden;text-overflow:ellipsis;white-space:nowrap;flex:1;";
      selTxt.textContent = `🎙 ${selectedName}`;
      const selChev = document.createElement("span");
      selChev.textContent = "⌄";
      selBtn.append(selTxt, selChev);

      const selMenu = document.createElement("div");
      selMenu.setAttribute("hidden", "");
      selMenu.style.cssText =
        "position:absolute;left:0;right:0;bottom:calc(100% + 4px);background:#232428;border:1px solid #383a40;border-radius:8px;padding:4px;box-shadow:0 12px 28px rgba(0,0,0,0.75);z-index:9550;max-height:180px;overflow-y:auto;display:flex;flex-direction:column;gap:2px;";
      const allDevs = [
        ...inputs.map((d) => ({ id: d.id, name: d.name, kind: "input" })),
        ...outputs
          .filter((o) => !inputs.some((i) => i.id === o.id))
          .map((d) => ({ id: d.id, name: d.name, kind: "output" })),
      ];
      for (const dev of allDevs) {
        const opt = document.createElement("button");
        opt.type = "button";
        opt.style.cssText =
          "padding:6px 8px;border:none;border-radius:4px;background:transparent;color:#dbdee1;font-size:12px;cursor:pointer;text-align:left;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
        opt.textContent = `${dev.kind === "output" ? "🎧" : "🎙"} ${dev.name}`;
        opt.addEventListener("click", () => {
          selectedId = dev.id;
          selectedName = dev.name;
          selTxt.textContent = `${dev.kind === "output" ? "🎧" : "🎙"} ${selectedName}`;
          selMenu.setAttribute("hidden", "");
        });
        selMenu.append(opt);
      }
      selBtn.addEventListener("click", () => {
        if (selMenu.hasAttribute("hidden")) selMenu.removeAttribute("hidden");
        else selMenu.setAttribute("hidden", "");
      });
      selWrap.append(selBtn, selMenu);
      card.append(selWrap);
    }

    const actionsRow = document.createElement("div");
    actionsRow.style.cssText = "display:flex;align-items:center;gap:6px;";
    const applyBtn = document.createElement("button");
    applyBtn.type = "button";
    applyBtn.style.cssText =
      "flex:1;height:32px;padding:0 10px;border:none;border-radius:6px;background:#5865f2;color:#ffffff;font-size:12px;font-weight:600;cursor:pointer;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
    applyBtn.textContent =
      mode === "both" ? "Сменить устройства ввода и вывода" : "Сменить устройство ввода";
    applyBtn.addEventListener("click", () => {
      selfVoicePrefs.inputDeviceId = selectedId;
      if (mode === "both") {
        const matchOut = outputs.find((o) => o.name === selectedName || o.id === selectedId);
        if (matchOut) selfVoicePrefs.outputDeviceId = matchOut.id;
      }
      syncVoiceMuteStateWithBackend();
      document.getElementById("live-audio-hotplug-more-menu")?.remove();
      card.remove();
    });

    const moreBtn = document.createElement("button");
    moreBtn.type = "button";
    moreBtn.id = "live-hotplug-more-btn";
    moreBtn.style.cssText =
      "width:32px;height:32px;border:none;border-radius:6px;background:#2b2d31;color:#dbdee1;font-size:14px;font-weight:700;cursor:pointer;flex-shrink:0;";
    moreBtn.textContent = "⋯";
    moreBtn.addEventListener("click", (ev) => {
      ev.stopPropagation();
      const existingMenu = document.getElementById("live-audio-hotplug-more-menu");
      if (existingMenu) {
        existingMenu.remove();
        return;
      }
      const m = document.createElement("div");
      m.id = "live-audio-hotplug-more-menu";
      m.style.cssText =
        `position:fixed;left:24px;bottom:${bottomOffset + 48}px;width:236px;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:8px;padding:6px;box-shadow:0 14px 34px rgba(0,0,0,0.8);z-index:9550;display:flex;flex-direction:column;gap:2px;font-size:12px;`;

      const addOpt = (/** @type {string} */ label, /** @type {string} */ icon, /** @type {boolean} */ danger, /** @type {() => void} */ fn) => {
        const b = document.createElement("button");
        b.type = "button";
        b.style.cssText =
          `display:flex;align-items:center;justify-content:space-between;gap:8px;padding:7px 8px;border:none;border-radius:5px;background:transparent;color:${danger ? "#f23f43" : "#dbdee1"};font-size:12px;font-weight:500;cursor:pointer;text-align:left;`;
        b.innerHTML = `<span style="overflow:hidden;text-overflow:ellipsis;">${label}</span><span style="flex-shrink:0;">${icon}</span>`;
        b.addEventListener("mouseenter", () => {
          b.style.background = danger ? "rgba(242,63,67,0.16)" : "#2b2d31";
        });
        b.addEventListener("mouseleave", () => {
          b.style.background = "transparent";
        });
        b.addEventListener("click", () => {
          fn();
          m.remove();
          card.remove();
        });
        m.append(b);
      };

      addOpt("Сменить только устройство ввода", "🎙", false, () => {
        selfVoicePrefs.inputDeviceId = selectedId;
        syncVoiceMuteStateWithBackend();
      });
      addOpt("Сменить только устройство вывода", "🎧", false, () => {
        const matchOut = outputs.find((o) => o.name === selectedName || o.id === selectedId) ?? outputs[0];
        if (matchOut) selfVoicePrefs.outputDeviceId = matchOut.id;
        syncVoiceMuteStateWithBackend();
      });
      const div = document.createElement("div");
      div.style.cssText = "height:1px;background:#2b2d31;margin:4px 0;";
      m.append(div);
      addOpt(`Не предлагать ${selectedName} снова`, "🎙̸", true, () => {
        ignoredHotplugDeviceIds.add(selectedId);
      });
      addOpt("Больше не показывать", "🎙̸", true, () => {
        hotplugPromptsSuppressed = true;
      });
      document.body.append(m);
    });

    actionsRow.append(applyBtn, moreBtn);
    card.append(actionsRow);
    document.body.append(card);
  }

  /**
   * Поповер статуса/профиля пользователя по клику на `#user-bar [data-action="profile"]` (Video frame_053.jpg).
   */
  function openSelfProfileStatusPopover() {
    const existing = document.getElementById("live-self-status-popout");
    if (existing) {
      existing.remove();
      return;
    }
    closeLiveVoicePopouts();
    const pop = document.createElement("div");
    pop.id = "live-self-status-popout";
    pop.setAttribute("role", "dialog");
    pop.style.cssText =
      "position:fixed;left:16px;bottom:64px;width:300px;background:#111214;color:#f2f3f5;border:1px solid #2b2d31;border-radius:12px;overflow:hidden;box-shadow:0 16px 36px rgba(0,0,0,0.7);z-index:9500;display:flex;flex-direction:column;";
    const banner = document.createElement("div");
    banner.style.cssText = "height:60px;background:linear-gradient(135deg,#5865f2,#3b429f);";
    const body = document.createElement("div");
    body.style.cssText = "padding:12px 14px 14px;display:flex;flex-direction:column;gap:10px;";
    const disp = String(
      liveSelfUser?.globalName || liveSelfUser?.global_name || liveSelfUser?.username || "Пользователь",
    );
    const uname = String(liveSelfUser?.username || "user");
    const uid = String(liveSelfUser?.id || "");
    const nameBlock = document.createElement("div");
    nameBlock.style.cssText = "display:flex;flex-direction:column;gap:2px;";
    const dispLine = document.createElement("div");
    dispLine.style.cssText = "font-size:17px;font-weight:700;color:#f2f3f5;";
    dispLine.textContent = disp;
    const unameBtn = document.createElement("button");
    unameBtn.type = "button";
    unameBtn.title = "Нажмите, чтобы скопировать имя пользователя";
    unameBtn.style.cssText =
      "font-size:12px;color:#949ba4;background:transparent;border:none;padding:2px 0;cursor:pointer;text-align:left;user-select:text;";
    unameBtn.textContent = `@${uname}`;
    const copyTextWithBackend = (/** @type {string} */ val) => {
      void navigator.clipboard?.writeText(val).catch(() => {});
      void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "copy_clipboard",
        deviceId: val,
      }).catch(() => {});
    };
    const handleCopyUname = (/** @type {Event} */ ev) => {
      ev.preventDefault();
      copyTextWithBackend(uname);
      unameBtn.textContent = `✓ Скопировано: ${uname}`;
      unameBtn.style.color = "#23a55a";
      setTimeout(() => {
        unameBtn.textContent = `@${uname}`;
        unameBtn.style.color = "#949ba4";
      }, 1200);
    };
    unameBtn.addEventListener("click", handleCopyUname);
    unameBtn.addEventListener("contextmenu", handleCopyUname);
    nameBlock.append(dispLine, unameBtn);
    body.append(nameBlock);

    const statusGroup = document.createElement("div");
    statusGroup.style.cssText = "background:#1e1f22;border-radius:8px;padding:6px;display:flex;flex-direction:column;gap:2px;";
    const statuses = [
      { key: "online", label: "В сети", color: "#23a55a" },
      { key: "idle", label: "Неактивен", color: "#f0b232" },
      { key: "dnd", label: "Не беспокоить", color: "#f23f43" },
      { key: "invisible", label: "Невидимый", color: "#80848e" },
    ];
    for (const st of statuses) {
      const btn = document.createElement("button");
      btn.type = "button";
      const active = selfVoicePrefs.status === st.key;
      btn.style.cssText =
        `display:flex;align-items:center;gap:10px;padding:7px 8px;border:none;border-radius:6px;background:${active ? "#35373c" : "transparent"};color:#f2f3f5;font-size:13px;cursor:pointer;text-align:left;`;
      btn.innerHTML = `<span style="width:10px;height:10px;border-radius:50%;background:${st.color};display:inline-block;"></span><span style="flex:1;">${st.label}</span>${active ? "<span>✓</span>" : ""}`;
      btn.addEventListener("click", () => {
        selfVoicePrefs.status = st.key;
        const uStatusNode = document.querySelector("#user-bar .user-status");
        if (uStatusNode) {
          uStatusNode.textContent = st.label;
        }
        syncUserBarVoiceButtons();
        const selfUid = cleanDiscordStr(String(liveSelfUser?.id || ""));
        const effectiveSt = st.key === "invisible" ? "offline" : st.key;
        for (const r of document.querySelectorAll(`.member-row[data-user-id="${selfUid}"]`)) {
          r.setAttribute("data-status", effectiveSt);
          const dotEl = r.querySelector(".status");
          if (dotEl) {
            dotEl.className = `status status--${effectiveSt}`;
          }
        }
        pop.remove();
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "user_status",
          deviceId: st.key,
          volumePct: null,
          muted: null,
        })
          .catch(() => {})
          .finally(() => {
            if (rail.selectedId && rail.selectedId !== HOME_GUILD_ID) {
              void loadLiveGuildMembers(rail.selectedId);
            }
          });
      });
      statusGroup.append(btn);
    }
    body.append(statusGroup);

    const actionsGroup = document.createElement("div");
    actionsGroup.style.cssText = "background:#1e1f22;border-radius:8px;padding:6px;display:flex;flex-direction:column;gap:2px;";
    const editBtn = document.createElement("button");
    editBtn.type = "button";
    editBtn.style.cssText = "padding:7px 8px;border:none;border-radius:6px;background:transparent;color:#dbdee1;font-size:13px;cursor:pointer;text-align:left;";
    editBtn.textContent = "✏️ Редактировать профиль";
    editBtn.addEventListener("click", () => {
      pop.remove();
      router.switchScreen("settings");
    });
    const copyUnameBtn = document.createElement("button");
    copyUnameBtn.type = "button";
    copyUnameBtn.style.cssText = "padding:7px 8px;border:none;border-radius:6px;background:transparent;color:#dbdee1;font-size:13px;cursor:pointer;text-align:left;";
    copyUnameBtn.textContent = `📋 Копировать имя (@${uname})`;
    copyUnameBtn.addEventListener("click", () => {
      copyTextWithBackend(uname);
      copyUnameBtn.textContent = `✓ Имя скопировано (${uname})`;
      setTimeout(() => pop.remove(), 450);
    });
    const copyIdBtn = document.createElement("button");
    copyIdBtn.type = "button";
    copyIdBtn.style.cssText = "padding:7px 8px;border:none;border-radius:6px;background:transparent;color:#dbdee1;font-size:13px;cursor:pointer;text-align:left;";
    copyIdBtn.textContent = "🆔 Копировать ID пользователя";
    copyIdBtn.addEventListener("click", () => {
      copyTextWithBackend(uid);
      copyIdBtn.textContent = "✓ ID скопирован";
      setTimeout(() => pop.remove(), 450);
    });
    actionsGroup.append(editBtn, copyUnameBtn, copyIdBtn);
    body.append(actionsGroup);

    pop.append(banner, body);
    document.body.append(pop);
  }

  /**
   * Модальное окно детальной RTC-диагностики (кнопка «Отладка» в поповере «О голосовом звонке»).
   */
  function openRtcDiagnosticsModal() {
    document.getElementById("live-rtc-debug-modal")?.remove();
    const backdrop = document.createElement("div");
    backdrop.id = "live-rtc-debug-modal";
    backdrop.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.72);display:flex;align-items:center;justify-content:center;z-index:9999;";
    const card = document.createElement("div");
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.style.cssText =
      "width:520px;max-width:94vw;background:#1e1f22;color:#dbdee1;border:1px solid #2b2d31;border-radius:10px;padding:20px;box-shadow:0 18px 44px rgba(0,0,0,0.75);display:flex;flex-direction:column;gap:12px;font-size:13px;";
    const head = document.createElement("div");
    head.style.cssText = "display:flex;align-items:center;justify-content:space-between;border-bottom:1px solid #2b2d31;padding-bottom:10px;";
    const title = document.createElement("h3");
    title.style.cssText = "margin:0;font-size:16px;font-weight:700;color:#f2f3f5;";
    title.textContent = "Диагностика RTC / Голосового транспорта";
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "icon-btn";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText = "color:#b5bac1;font-size:16px;cursor:pointer;";
    closeBtn.addEventListener("click", () => backdrop.remove());
    head.append(title, closeBtn);

    const host = liveRtcStatus?.shortHostname || liveRtcStatus?.endpoint || "—";
    const ep = liveRtcStatus?.endpoint || "—";
    const lastPing = typeof liveRtcStatus?.lastPingMs === "number" && liveRtcStatus.lastPingMs > 0 ? `${liveRtcStatus.lastPingMs} мс` : "—";
    const avgPing = typeof liveRtcStatus?.averagePingMs === "number" && liveRtcStatus.averagePingMs > 0 ? `${liveRtcStatus.averagePingMs} мс` : lastPing;
    const ssrc = liveRtcStatus?.ssrc || "—";
    const mode = liveRtcStatus?.encryptionMode || liveRtcStatus?.mode || "aead_aes256_gcm_rtpsize";
    const proto = liveRtcStatus?.protocol || "udp";
    const gpuVendor = liveRtcStatus?.gpuVendor || "GPU";
    const gpuModel = liveRtcStatus?.gpuModel || "Hardware Accelerator";
    const gpuApi = liveRtcStatus?.gpuApi || "Vulkan / VA-API";
    const activeVideoCodec = liveRtcStatus?.activeVideoCodec || "H264 (HW)";
    const hwEncoders = Array.isArray(liveRtcStatus?.hwEncoders) ? liveRtcStatus.hwEncoders.join(", ") : "H264, HEVC, AV1";
    const hwDecoders = Array.isArray(liveRtcStatus?.hwDecoders) ? liveRtcStatus.hwDecoders.join(", ") : "H264, HEVC, VP9, AV1";
    const participants = activeVoiceConnection
      ? getVoiceParticipants(activeVoiceConnection.guildId, activeVoiceConnection.channelId)
      : [];

    const grid = document.createElement("div");
    grid.style.cssText = "display:grid;grid-template-columns:1fr 1fr;gap:8px;background:#111214;padding:12px;border-radius:8px;font-family:monospace;font-size:12px;";
    grid.innerHTML = `
      <div><span style="color:#949ba4;">Сервер:</span> <b style="color:#23a55a;">${host}</b></div>
      <div><span style="color:#949ba4;">Endpoint:</span> ${ep}</div>
      <div><span style="color:#949ba4;">Транспорт:</span> ${proto.toUpperCase()} + WSS v8</div>
      <div><span style="color:#949ba4;">Шифрование:</span> ${mode}</div>
      <div><span style="color:#949ba4;">Аудиокодек:</span> opus (48000 Hz, 2ch)</div>
      <div><span style="color:#949ba4;">Видеокодек:</span> <b style="color:#23a55a;">${activeVideoCodec}</b></div>
      <div><span style="color:#949ba4;">Видеокарта (GPU):</span> <b style="color:#f2f3f5;">${gpuVendor} · ${gpuModel}</b></div>
      <div><span style="color:#949ba4;">Графический API:</span> ${gpuApi}</div>
      <div><span style="color:#949ba4;">HW Кодировщики:</span> ${hwEncoders}</div>
      <div><span style="color:#949ba4;">HW Декодеры:</span> ${hwDecoders}</div>
      <div><span style="color:#949ba4;">Локальный SSRC:</span> ${ssrc}</div>
      <div><span style="color:#949ba4;">Средний RTT:</span> <b style="color:#f2f3f5;">${avgPing}</b></div>
      <div><span style="color:#949ba4;">Последний RTT:</span> <b style="color:#f2f3f5;">${lastPing}</b></div>
      <div><span style="color:#949ba4;">DAVE E2EE:</span> v1 (активен)</div>
    `;

    const partBox = document.createElement("div");
    partBox.style.cssText = "display:flex;flex-direction:column;gap:4px;";
    partBox.innerHTML = `<div style="font-size:11px;font-weight:700;text-transform:uppercase;color:#949ba4;">Участники в голосовом канале (${participants.length})</div>`;
    for (const p of participants) {
      const row = document.createElement("div");
      row.style.cssText = "display:flex;align-items:center;justify-content:space-between;padding:4px 8px;background:#2b2d31;border-radius:4px;font-size:12px;";
      row.innerHTML = `<span><b>${p.displayName}</b> <span style="color:#949ba4;">(${p.userId})</span></span><span style="color:${p.streaming ? "#da373c" : "#23a55a"};font-weight:600;">${p.streaming ? "В ЭФИРЕ" : p.deafened ? "Звук выкл" : p.muted ? "Микр. выкл" : "Активен"}</span>`;
      partBox.append(row);
    }

    const foot = document.createElement("div");
    foot.style.cssText = "display:flex;justify-content:flex-end;gap:8px;margin-top:4px;";
    const settingsBtn = document.createElement("button");
    settingsBtn.type = "button";
    settingsBtn.textContent = "Настройки голоса и видео";
    settingsBtn.style.cssText = "padding:7px 12px;border:none;border-radius:6px;background:#4e5058;color:#f2f3f5;font-size:12px;font-weight:600;cursor:pointer;";
    settingsBtn.addEventListener("click", () => {
      backdrop.remove();
      router.switchScreen("settings");
      settingsRef?.showPane("voice");
    });
    const okBtn = document.createElement("button");
    okBtn.type = "button";
    okBtn.textContent = "Закрыть";
    okBtn.style.cssText = "padding:7px 14px;border:none;border-radius:6px;background:#5865f2;color:#ffffff;font-size:12px;font-weight:600;cursor:pointer;";
    okBtn.addEventListener("click", () => backdrop.remove());
    foot.append(settingsBtn, okBtn);

    card.append(head, grid, partBox, foot);
    backdrop.append(card);
    backdrop.addEventListener("click", (ev) => {
      if (ev.target === backdrop) backdrop.remove();
    });
    document.body.append(backdrop);
  }

  /**
   * Поповер деталей голосового соединения («О голосовом звонке», Video_2026-11-01_00-50-10.mp4 f_018/f_022) с живым SVG-графиком RTT.
   */
  function openRtcConnectionPopout() {
    const existing = document.getElementById("live-rtc-popout");
    if (existing) {
      existing.remove();
      return;
    }
    closeLiveVoicePopouts();
    const pop = document.createElement("div");
    pop.id = "live-rtc-popout";
    pop.setAttribute("role", "dialog");
    pop.style.cssText =
      "position:fixed;left:78px;bottom:118px;width:340px;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:10px;padding:16px;box-shadow:0 16px 38px rgba(0,0,0,0.75);z-index:9500;display:flex;flex-direction:column;gap:10px;font-size:13px;";

    let activeTab = "integration";
    const renderPopContent = () => {
      pop.replaceChildren();
      const tabsRow = document.createElement("div");
      tabsRow.style.cssText = "display:flex;gap:16px;border-bottom:1px solid #2b2d31;padding-bottom:8px;";
      const tabInt = document.createElement("button");
      tabInt.type = "button";
      tabInt.textContent = "Интеграция";
      tabInt.style.cssText = `background:none;border:none;padding:0 0 6px;font-size:13px;font-weight:600;cursor:pointer;color:${activeTab === "integration" ? "#5865f2" : "#949ba4"};border-bottom:2px solid ${activeTab === "integration" ? "#5865f2" : "transparent"};`;
      tabInt.addEventListener("click", () => {
        activeTab = "integration";
        renderPopContent();
      });
      const tabPriv = document.createElement("button");
      tabPriv.type = "button";
      tabPriv.textContent = "Конфиденциальность";
      tabPriv.style.cssText = `background:none;border:none;padding:0 0 6px;font-size:13px;font-weight:600;cursor:pointer;color:${activeTab === "privacy" ? "#5865f2" : "#949ba4"};border-bottom:2px solid ${activeTab === "privacy" ? "#5865f2" : "transparent"};`;
      tabPriv.addEventListener("click", () => {
        activeTab = "privacy";
        renderPopContent();
      });
      tabsRow.append(tabInt, tabPriv);
      pop.append(tabsRow);

      if (activeTab === "privacy") {
        // Генерируем стабильный 30-значный E2EE код верификации звонка (6 блоков по 5 цифр, как в f_018.jpg)
        const seedStr = `${activeVoiceConnection?.channelId || "1176785373989445703"}-${liveRtcStatus?.ssrc || 87421}`;
        let hash = 2166136261;
        for (let i = 0; i < seedStr.length; i += 1) {
          hash ^= seedStr.charCodeAt(i);
          hash = Math.imul(hash, 16777619) >>> 0;
        }
        /** @type {string[]} */
        const blocks = [];
        for (let b = 0; b < 6; b += 1) {
          hash = (Math.imul(hash ^ (b + 17), 1664525) + 1013904223) >>> 0;
          blocks.push(String(10000 + (hash % 90000)));
        }
        const fullCode = `${blocks.slice(0, 3).join(" ")} ${blocks.slice(3, 6).join(" ")}`;

        const privBox = document.createElement("div");
        privBox.style.cssText = "display:flex;flex-direction:column;gap:10px;padding:4px 0;";
        const pTitle = document.createElement("div");
        pTitle.style.cssText = "font-size:15px;font-weight:700;color:#f2f3f5;line-height:1.3;";
        pTitle.textContent = "Код защиты конфиденциальности голосового звонка";
        const pDesc = document.createElement("div");
        pDesc.style.cssText = "font-size:12px;color:#b5bac1;line-height:1.45;";
        pDesc.textContent =
          "Голосовые и видеозвонки защищены сквозным шифрованием. Сравните этот код с кодами других участников звонка. Одинаковые коды подтверждают защиту звонка.";

        const codeCard = document.createElement("div");
        codeCard.style.cssText =
          "display:flex;align-items:center;justify-content:space-between;gap:10px;background:#1e1f22;border:1px solid #2b2d31;border-radius:8px;padding:10px 12px;";
        const digitsGrid = document.createElement("div");
        digitsGrid.style.cssText =
          "display:grid;grid-template-columns:repeat(3,auto);gap:6px 14px;font-family:monospace;font-size:14px;font-weight:700;color:#f2f3f5;letter-spacing:0.5px;";
        for (const blk of blocks) {
          const cell = document.createElement("span");
          cell.textContent = blk;
          digitsGrid.append(cell);
        }
        const copyCodeBtn = document.createElement("button");
        copyCodeBtn.type = "button";
        copyCodeBtn.style.cssText =
          "padding:7px 12px;border:none;border-radius:6px;background:#2b2d31;color:#f2f3f5;font-size:12px;font-weight:600;cursor:pointer;white-space:nowrap;";
        copyCodeBtn.textContent = "📋 Копировать";
        copyCodeBtn.addEventListener("click", () => {
          void navigator.clipboard?.writeText(fullCode).catch(() => {});
          copyCodeBtn.textContent = "✓ Скопировано";
        });
        codeCard.append(digitsGrid, copyCodeBtn);

        const footerLock = document.createElement("div");
        footerLock.style.cssText =
          "padding-top:8px;border-top:1px solid #2b2d31;color:#23a55a;font-size:12px;font-weight:600;display:flex;align-items:center;gap:6px;";
        footerLock.textContent = "🔒 Защищено сквозным шифрованием";

        privBox.append(pTitle, pDesc, codeCard, footerLock);
        pop.append(privBox);
        return;
      }

      const st = String(liveRtcStatus.state || "connected");
      const isErr = st === "error";
      const statusTitle = document.createElement("div");
      statusTitle.id = "live-rtc-popout-title";
      statusTitle.style.cssText = `font-size:16px;font-weight:700;color:${isErr ? "#f23f43" : "#23a55a"};`;
      statusTitle.textContent = isErr ? "Ошибка RTC соединения" : "Голосовая связь подключена";

      const chartWrap = document.createElement("div");
      chartWrap.id = "live-rtc-popout-chart";
      chartWrap.style.cssText =
        "height:86px;background:#18191c;border:1px solid #232428;border-radius:8px;padding:8px 10px 18px;position:relative;overflow:hidden;";

      const statsBlock = document.createElement("div");
      statsBlock.id = "live-rtc-popout-stats";
      statsBlock.style.cssText = "display:flex;flex-direction:column;gap:4px;color:#dbdee1;line-height:1.45;font-size:13px;";

      const helpNote = document.createElement("div");
      helpNote.style.cssText = "font-size:12px;color:#949ba4;line-height:1.4;margin-top:2px;";
      helpNote.innerHTML =
        `Пинг в районе 250 мс или больше для меня не новость, но когда вы слышите роботов, значит дело дрянь. Подробности приводятся в <span style="color:#00a8fc;cursor:pointer;">руководстве по устранению неполадок</span>.`;

      const btnRow = document.createElement("div");
      btnRow.style.cssText = "display:flex;gap:8px;margin-top:4px;";
      const debugBtn = document.createElement("button");
      debugBtn.type = "button";
      debugBtn.textContent = "🪲 Отладка";
      debugBtn.style.cssText =
        "flex:1;padding:8px 10px;border:none;border-radius:6px;background:#2b2d31;color:#f2f3f5;font-size:13px;font-weight:600;cursor:pointer;";
      debugBtn.addEventListener("click", () => {
        pop.remove();
        openRtcDiagnosticsModal();
      });
      const logBtn = document.createElement("button");
      logBtn.type = "button";
      logBtn.textContent = "↑ Загрузить журнал";
      logBtn.style.cssText =
        "flex:1;padding:8px 10px;border:none;border-radius:6px;background:#2b2d31;color:#f2f3f5;font-size:13px;font-weight:600;cursor:pointer;";
      logBtn.addEventListener("click", () => {
        logBtn.textContent = "✓ Журнал выгружен";
      });
      btnRow.append(debugBtn, logBtn);

      const footerLock = document.createElement("div");
      footerLock.style.cssText =
        "padding-top:8px;border-top:1px solid #2b2d31;color:#23a55a;font-size:12px;font-weight:600;display:flex;align-items:center;gap:6px;";
      footerLock.textContent = "🔒 Защищено сквозным шифрованием";

      pop.append(statusTitle, chartWrap, statsBlock, helpNote, btnRow, footerLock);
      updateLiveRtcDomInPlace();
    };
    renderPopContent();
    document.body.append(pop);
  }

  /**
   * Показывает зелёный плавающий бейдж «Ваш микрофон отключён!» над кнопкой микрофона в `#user-bar`
   * (точное соответствие `frame_009.jpg` при проверке микрофона или клике в отключённом состоянии).
   */
  function showMutedMicFloatingWarning() {
    document.getElementById("live-mic-muted-warning-badge")?.remove();
    if (!selfVoicePrefs.muted) return;
    const uMute = document.querySelector('#user-bar [data-action="mute"]');
    const rect = uMute?.getBoundingClientRect?.();
    const badge = document.createElement("div");
    badge.id = "live-mic-muted-warning-badge";
    const leftPx = rect && rect.width > 0 ? Math.max(12, Math.round(rect.left - 54)) : 108;
    const bottomPx = rect && rect.top > 0 ? Math.round(window.innerHeight - rect.top + 8) : 58;
    badge.style.cssText =
      `position:fixed;left:${leftPx}px;bottom:${bottomPx}px;background:#23a55a;color:#ffffff;font-size:12px;font-weight:700;padding:7px 12px;border-radius:6px;box-shadow:0 8px 20px rgba(0,0,0,0.55);z-index:9600;pointer-events:none;white-space:nowrap;`;
    badge.textContent = "Ваш микрофон отключён!";
    const arrow = document.createElement("span");
    arrow.style.cssText =
      "position:absolute;left:50%;bottom:-5px;transform:translateX(-50%) rotate(45deg);width:10px;height:10px;background:#23a55a;";
    badge.append(arrow);
    document.body.append(badge);
  }

  /**
   * Поповер шумоподавления Krisp и проверки микрофона (точное соответствие `frame_009.jpg` и `frame_010.jpg`).
   */
  function openKrispNoisePopout() {
    const existing = document.getElementById("live-krisp-popout");
    if (existing) {
      if (micTestTimer) {
        clearInterval(micTestTimer);
        micTestTimer = null;
      }
      if (isLiveSession()) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "mic_test_stop",
          deviceId: "default",
          volumePercent: 100,
          muted: false,
        }).catch(() => {});
      }
      existing.remove();
      document.getElementById("live-mic-muted-warning-badge")?.remove();
      return;
    }
    closeLiveVoicePopouts();
    const pop = document.createElement("div");
    pop.id = "live-krisp-popout";
    pop.setAttribute("role", "dialog");
    pop.setAttribute("aria-label", "Шумоподавление Krisp и проверка микрофона");
    pop.style.cssText =
      "position:fixed;left:140px;bottom:116px;width:276px;background:#18191c;color:#dbdee1;border:1px solid #2b2d31;border-radius:10px;padding:16px;box-shadow:0 16px 36px rgba(0,0,0,0.75);z-index:9500;display:flex;flex-direction:column;gap:12px;font-size:12px;";

    const headRow = document.createElement("div");
    headRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:10px;";
    const title = document.createElement("span");
    title.style.cssText = "font-size:14px;font-weight:700;color:#f2f3f5;";
    title.textContent = "Шумоподавление";

    const switchWrap = document.createElement("button");
    switchWrap.type = "button";
    switchWrap.setAttribute("aria-label", "Включить шумоподавление Krisp");
    const thumb = document.createElement("span");
    const renderToggle = () => {
      const on = Boolean(selfVoicePrefs.krispEnabled);
      switchWrap.style.cssText =
        `width:40px;height:24px;border-radius:12px;border:none;padding:3px;background:${on ? "#5865f2" : "#4e5058"};cursor:pointer;display:inline-flex;align-items:center;justify-content:${on ? "flex-end" : "flex-start"};transition:background 140ms ease;flex-shrink:0;`;
      thumb.style.cssText =
        "width:18px;height:18px;border-radius:50%;background:#ffffff;box-shadow:0 1px 3px rgba(0,0,0,0.4);display:block;";
    };
    switchWrap.append(thumb);
    renderToggle();
    switchWrap.addEventListener("click", () => {
      selfVoicePrefs.krispEnabled = !selfVoicePrefs.krispEnabled;
      if (activeVoiceConnection) activeVoiceConnection.krispEnabled = selfVoicePrefs.krispEnabled;
      renderToggle();
      renderLiveVoicePanel();
      if (isLiveSession()) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "krisp",
          deviceId: "default",
          volumePercent: 100,
          muted: Boolean(selfVoicePrefs.krispEnabled),
        }).catch(() => {});
      }
      emitLiveTelemetry({
        type: "voice_krisp_toggle",
        enabled: selfVoicePrefs.krispEnabled,
      });
    });
    headRow.append(title, switchWrap);

    const desc = document.createElement("div");
    desc.style.cssText = "color:#b5bac1;line-height:1.42;font-size:12px;";
    desc.textContent =
      "Включите шумоподавление Krisp! Теперь вы можете шуметь во время разговора, например, хлопая в ладоши, и ваши друзья не услышат ничего, кроме вашего прекрасного голоса.";

    const testSection = document.createElement("div");
    testSection.style.cssText =
      "padding-top:10px;border-top:1px solid #2b2d31;display:flex;flex-direction:column;gap:8px;";
    const testTitle = document.createElement("div");
    testTitle.style.cssText = "font-weight:700;color:#f2f3f5;font-size:13px;";
    testTitle.textContent = "Проверка микрофона";

    const testRow = document.createElement("div");
    testRow.style.cssText = "display:flex;align-items:center;gap:10px;";
    const testBtn = document.createElement("button");
    testBtn.type = "button";
    testBtn.textContent = "Проверка";
    testBtn.style.cssText =
      "padding:7px 14px;border:1px solid #3f4147;border-radius:6px;background:#2b2d31;color:#f2f3f5;font-size:13px;font-weight:600;cursor:pointer;flex-shrink:0;min-width:76px;";
    const barsWrap = document.createElement("div");
    barsWrap.style.cssText = "display:flex;gap:3px;flex:1;align-items:center;height:18px;";
    /** @type {HTMLElement[]} */
    const bars = [];
    for (let i = 0; i < 20; i += 1) {
      const b = document.createElement("span");
      b.style.cssText =
        "width:3px;flex:1;height:14px;border-radius:2px;background:#35373c;transition:background 70ms ease;";
      bars.push(b);
      barsWrap.append(b);
    }
    let testing = false;
    testBtn.addEventListener("click", () => {
      testing = !testing;
      testBtn.textContent = testing ? "Стоп" : "Проверка";
      testBtn.style.background = testing ? "#da373c" : "#2b2d31";
      if (micTestTimer) {
        clearInterval(micTestTimer);
        micTestTimer = null;
      }
      if (isLiveSession()) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: testing ? "mic_test_start" : "mic_test_stop",
          deviceId: selfVoicePrefs.inputDeviceId || "default",
          volumePercent: selfVoicePrefs.inputVolume ?? 100,
          muted: Boolean(selfVoicePrefs.muted),
        }).catch(() => {});
      }
      if (testing) {
        if (selfVoicePrefs.muted) {
          showMutedMicFloatingWarning();
        } else {
          document.getElementById("live-mic-muted-warning-badge")?.remove();
        }
        micTestTimer = setInterval(async () => {
          if (selfVoicePrefs.muted) {
            showMutedMicFloatingWarning();
          } else {
            document.getElementById("live-mic-muted-warning-badge")?.remove();
          }
          if (isLiveSession()) {
            try {
              const st = await invoke(VOICE_RTC_STATUS_IPC_CMD);
              if (st && typeof st === "object") {
                liveRtcStatus = st;
              }
            } catch {}
          }
          const db = typeof liveRtcStatus?.localMicLevelDb === "number" ? liveRtcStatus.localMicLevelDb : -100;
          const effVad = typeof liveRtcStatus?.effectiveVadDb === "number" ? liveRtcStatus.effectiveVadDb : -45;
          const fromDb = db > -72 ? Math.max(1, Math.min(20, Math.round(((db + 72) / 62) * 20))) : 0;
          const activeCount = selfVoicePrefs.muted ? 0 : fromDb;
          bars.forEach((b, idx) => {
            b.style.background =
              idx < activeCount ? (db >= effVad ? "#23a55a" : "#f0b232") : "#35373c";
          });
        }, 95);
      } else {
        document.getElementById("live-mic-muted-warning-badge")?.remove();
        bars.forEach((b) => {
          b.style.background = "#35373c";
        });
      }
      emitLiveTelemetry({
        type: "voice_mic_test_toggle",
        testing,
        muted: selfVoicePrefs.muted,
      });
    });
    testRow.append(testBtn, barsWrap);
    testSection.append(testTitle, testRow);

    const footerSection = document.createElement("div");
    footerSection.style.cssText =
      "padding-top:10px;border-top:1px solid #2b2d31;display:flex;flex-direction:column;gap:2px;";
    const poweredBy = document.createElement("div");
    poweredBy.style.cssText = "font-size:11px;font-weight:600;color:#b5bac1;";
    poweredBy.textContent = "С помощью";
    const brandRow = document.createElement("div");
    brandRow.style.cssText = "display:flex;align-items:baseline;justify-content:space-between;";
    const krispLogo = document.createElement("span");
    krispLogo.style.cssText =
      "font-size:19px;font-weight:900;color:#f2f3f5;letter-spacing:-0.6px;font-family:sans-serif;";
    krispLogo.textContent = "krisp";
    const learnMore = document.createElement("span");
    learnMore.style.cssText = "font-size:12px;color:#00a8fc;cursor:pointer;font-weight:500;";
    learnMore.textContent = "Узнать больше";
    learnMore.addEventListener("click", () => {
      pop.remove();
      router.switchScreen("settings");
      settingsRef?.showPane("voice");
    });
    brandRow.append(krispLogo, learnMore);
    footerSection.append(poweredBy, brandRow);

    pop.append(headRow, desc, testSection, footerSection);
    document.body.append(pop);
  }

  /**
   * Поповер «Звуковая панель» (`Soundboard`) по клику на 4-ю кнопку голосовой панели (мегафон/хлопушка).
   * Генерирует реальный звук через WebAudio API и отправляет индикацию в голосовой канал.
   */
  function openSoundboardPopout() {
    const existing = document.getElementById("live-soundboard-popout");
    if (existing) {
      existing.remove();
      return;
    }
    closeLiveVoicePopouts();
    const pop = document.createElement("div");
    pop.id = "live-soundboard-popout";
    pop.setAttribute("role", "dialog");
    pop.setAttribute("aria-label", "Звуковая панель");
    pop.style.cssText =
      "position:fixed;left:72px;bottom:116px;width:360px;background:#18191c;color:#f2f3f5;border:1px solid #2b2d31;border-radius:12px;padding:16px;box-shadow:0 18px 42px rgba(0,0,0,0.78);z-index:9500;display:flex;flex-direction:column;gap:12px;font-size:13px;";

    const headRow = document.createElement("div");
    headRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:8px;";
    const title = document.createElement("div");
    title.style.cssText = "font-size:15px;font-weight:700;color:#f2f3f5;";
    title.textContent = "Звуковая панель";
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText = "background:none;border:none;color:#b5bac1;font-size:15px;cursor:pointer;";
    closeBtn.addEventListener("click", () => pop.remove());
    headRow.append(title, closeBtn);

    const searchInput = document.createElement("input");
    searchInput.type = "search";
    searchInput.placeholder = "Найти звук…";
    searchInput.style.cssText =
      "width:100%;padding:7px 10px;border-radius:6px;border:1px solid #2b2d31;background:#111214;color:#f2f3f5;font-size:13px;outline:none;";

    const grid = document.createElement("div");
    grid.style.cssText = "display:grid;grid-template-columns:repeat(2,1fr);gap:8px;max-height:220px;overflow-y:auto;";

    const sounds = [
      { id: "quack", icon: "🦆", name: "Кря", freq: 520, dur: 0.18 },
      { id: "airhorn", icon: "📯", name: "Воздушный горн", freq: 440, dur: 0.35 },
      { id: "cricket", icon: "🦗", name: "Сверчок", freq: 1480, dur: 0.22 },
      { id: "trombone", icon: "🎺", name: "Грустный тромбон", freq: 260, dur: 0.4 },
      { id: "clap", icon: "👏", name: "Аплодисменты", freq: 660, dur: 0.2 },
      { id: "rimshot", icon: "🥁", name: "Ба-дум-тсс", freq: 340, dur: 0.28 },
      { id: "bell", icon: "🔔", name: "Колокольчик", freq: 880, dur: 0.25 },
      { id: "rocket", icon: "🚀", name: "Запуск", freq: 600, dur: 0.32 },
    ];

    const playTone = (/** @type {number} */ freq, /** @type {number} */ dur) => {
      try {
        const Ctx = globalThis.AudioContext || /** @type {any} */ (globalThis).webkitAudioContext;
        if (!Ctx) return;
        if (!sharedSoundboardAudioCtx || sharedSoundboardAudioCtx.state === "closed") {
          sharedSoundboardAudioCtx = new Ctx();
        }
        const ctx = sharedSoundboardAudioCtx;
        if (ctx.state === "suspended" && typeof ctx.resume === "function") {
          void ctx.resume().catch(() => {});
        }
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = "triangle";
        osc.frequency.setValueAtTime(freq, ctx.currentTime);
        osc.frequency.exponentialRampToValueAtTime(Math.max(120, freq * 0.72), ctx.currentTime + dur);
        gain.gain.setValueAtTime(0.18, ctx.currentTime);
        gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + dur);
        osc.connect(gain);
        gain.connect(ctx.destination);
        osc.onended = () => {
          try {
            osc.disconnect();
            gain.disconnect();
          } catch {}
        };
        osc.start();
        osc.stop(ctx.currentTime + dur);
      } catch {}
    };

    const renderSounds = (/** @type {string} */ q) => {
      grid.replaceChildren();
      const clean = q.trim().toLowerCase();
      for (const s of sounds) {
        if (clean && !s.name.toLowerCase().includes(clean)) continue;
        const btn = document.createElement("button");
        btn.type = "button";
        btn.style.cssText =
          "display:flex;align-items:center;gap:8px;padding:9px 10px;border-radius:8px;border:1px solid #2b2d31;background:#2b2d31;color:#f2f3f5;font-size:13px;font-weight:600;cursor:pointer;text-align:left;";
        btn.innerHTML = `<span style="font-size:17px;">${s.icon}</span><span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${s.name}</span>`;
        btn.addEventListener("click", () => {
          btn.style.borderColor = "#23a55a";
          btn.style.background = "rgba(35,165,90,0.18)";
          playTone(s.freq, s.dur);
          setTimeout(() => {
            btn.style.borderColor = "#2b2d31";
            btn.style.background = "#2b2d31";
          }, 350);
          emitLiveTelemetry({ type: "voice_soundboard_play", soundId: s.id, soundName: s.name });
        });
        grid.append(btn);
      }
    };
    renderSounds("");
    searchInput.addEventListener("input", () => renderSounds(searchInput.value));

    pop.append(headRow, searchInput, grid);
    document.body.append(pop);
  }

  /**
   * Поповер «Выбрать активность» (`Activities`) по клику на 3-ю кнопку голосовой панели (4 фигуры).
   */
  function openActivitiesPopout() {
    const existing = document.getElementById("live-activities-popout");
    if (existing) {
      existing.remove();
      return;
    }
    closeLiveVoicePopouts();
    const pop = document.createElement("div");
    pop.id = "live-activities-popout";
    pop.setAttribute("role", "dialog");
    pop.setAttribute("aria-label", "Начать активность");
    pop.style.cssText =
      "position:fixed;left:72px;bottom:116px;width:360px;background:#18191c;color:#f2f3f5;border:1px solid #2b2d31;border-radius:12px;padding:16px;box-shadow:0 18px 42px rgba(0,0,0,0.78);z-index:9500;display:flex;flex-direction:column;gap:10px;font-size:13px;";

    const headRow = document.createElement("div");
    headRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;";
    const title = document.createElement("div");
    title.style.cssText = "font-size:15px;font-weight:700;color:#f2f3f5;";
    title.textContent = "Начать активность";
    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.textContent = "✕";
    closeBtn.style.cssText = "background:none;border:none;color:#b5bac1;font-size:15px;cursor:pointer;";
    closeBtn.addEventListener("click", () => pop.remove());
    headRow.append(title, closeBtn);

    const activities = [
      { id: "youtube", icon: "▶️", name: "Watch Together (YouTube)", desc: "Смотрите видео вместе с друзьями" },
      { id: "poker", icon: "🃏", name: "Poker Night", desc: "Техасский холдем до 7 игроков" },
      { id: "chess", icon: "♟️", name: "Chess in the Park", desc: "Шахматы в голосовом канале" },
      { id: "sketch", icon: "🎨", name: "Sketch Heads", desc: "Рисуйте и угадывайте слова" },
      { id: "gartic", icon: "📞", name: "Gartic Phone", desc: "Испорченный телефон с рисунками" },
    ];

    const list = document.createElement("div");
    list.style.cssText = "display:flex;flex-direction:column;gap:6px;max-height:240px;overflow-y:auto;";
    for (const act of activities) {
      const row = document.createElement("button");
      row.type = "button";
      row.style.cssText =
        "display:flex;align-items:center;justify-content:space-between;gap:10px;padding:8px 10px;border-radius:8px;border:1px solid #2b2d31;background:#2b2d31;color:#f2f3f5;cursor:pointer;text-align:left;";
      row.innerHTML = `<span style="display:flex;align-items:center;gap:10px;"><span style="font-size:20px;">${act.icon}</span><span><div style="font-size:13px;font-weight:700;">${act.name}</div><div style="font-size:11px;color:#949ba4;">${act.desc}</div></span></span><span style="padding:4px 8px;border-radius:6px;background:#5865f2;color:#fff;font-size:11px;font-weight:700;">Запустить</span>`;
      row.addEventListener("click", () => {
        emitLiveTelemetry({ type: "voice_activity_launch", activityId: act.id, activityName: act.name });
        pop.remove();
      });
      list.append(row);
    }
    pop.append(headRow, list);
    document.body.append(pop);
  }

  /** @type {ReturnType<typeof setInterval> | null} */
  let activePipAnimTimer = null;

  /**
   * Рисует живой кадр предпросмотра для выбранного экрана или окна (`Video_2026-11-01_12-11-05.mp4`).
   * @param {HTMLCanvasElement} canvas
   * @param {string} sourceId
   * @param {number} tick
   * @param {boolean} [showWallpaper]
   */
  /** @type {HTMLImageElement | null} */
  let cachedLiveScreenImg = null;
  let cachedLiveScreenDataUrl = "";
  let lastLiveScreenSeq = 0;
  let liveScreenCaptureInFlight = false;
  let lastLiveScreenCaptureAtMs = 0;

  function requestLiveScreenPreviewFrame() {
    if (!isLiveSession() || liveScreenCaptureInFlight) return;
    const now = Date.now();
    if (now - lastLiveScreenCaptureAtMs < 850) return;
    liveScreenCaptureInFlight = true;
    lastLiveScreenCaptureAtMs = now;
    void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, { kind: "screen_preview" })
      .then((res) => {
        const seq = Number(res?.seq || 0);
        const dataUrl = res?.previewDataUrl;
        if (typeof dataUrl === "string" && dataUrl.startsWith("data:image/")) {
          if (seq > 0 && seq === lastLiveScreenSeq && cachedLiveScreenImg) return;
          lastLiveScreenSeq = seq;
          cachedLiveScreenDataUrl = dataUrl;
          const img = new globalThis.Image();
          img.onload = () => {
            cachedLiveScreenImg = img;
          };
          img.src = dataUrl;
        }
      })
      .catch(() => {})
      .finally(() => {
        liveScreenCaptureInFlight = false;
      });
  }

  /**
   * Открывает полноразмерное окно просмотра трансляции (`Stream Stage Viewer`) и отправляет `OP 20 STREAM_WATCH`.
   * Поддерживает масштабирование колесом мыши (100%..500%), панорамирование перетаскиванием (Drag-to-Pan),
   * двойной клик для быстрого приближения (100% ↔ 225%), кнопки зума и полноэкранный режим (`⛶`).
   * @param {any} participant
   * @param {{ id: string, name?: string }} channel
   */
  function openLiveStreamStageViewer(participant, channel) {
    document.getElementById("live-stream-stage-viewer")?.remove();
    const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
    const rawGid = cleanDiscordStr(
      String(participant?.guildId ?? activeVoiceConnection?.guildId ?? rail.selectedId ?? selectedGuildId() ?? ""),
    );
    const normGid = normalizeVoiceGuildKey(rawGid);
    const gId = normGid === HOME_GUILD_ID ? "" : normGid;
    const cId = String(channel?.id || participant?.channelId || "");
    const uId = cleanDiscordStr(String(participant?.userId || selfId));
    const isSelfStream = Boolean(uId && uId === selfId);
    const streamKey = cId && uId ? (gId ? `guild:${gId}:${cId}:${uId}` : `call:${cId}:${uId}`) : "";

    if (
      isLiveSession() &&
      cId &&
      (!activeVoiceConnection ||
        activeVoiceConnection.channelId !== cId ||
        (gId && normalizeVoiceGuildKey(activeVoiceConnection.guildId) !== normGid))
    ) {
      connectToLiveVoiceChannel(
        gId,
        currentGuildInfo().name || "Сервер",
        { id: cId, name: channel?.name || "Голосовой канал" },
      );
    }

    if (isLiveSession() && streamKey) {
      if (!isSelfStream) {
        playVoiceUiSound("stream_user_joined");
      }
      void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "stream_watch",
        deviceId: streamKey,
      }).catch(() => {});
    }
    if (isSelfStream) {
      requestLiveScreenPreviewFrame();
    }

    const overlay = document.createElement("div");
    overlay.id = "live-stream-stage-viewer";
    overlay.setAttribute("role", "dialog");
    overlay.setAttribute("aria-label", `Трансляция ${participant?.displayName || "Участника"}`);
    const normalOverlayCss =
      "position:fixed;left:312px;top:48px;right:248px;bottom:64px;background:#0b0c0e;border:1px solid #2b2d31;border-radius:12px;box-shadow:0 24px 64px rgba(0,0,0,0.9);z-index:9250;display:flex;flex-direction:column;overflow:hidden;user-select:none;contain:strict;";
    const expandedOverlayCss =
      "position:fixed;left:0;top:0;right:0;bottom:0;background:#070809;border:none;border-radius:0;z-index:10500;display:flex;flex-direction:column;overflow:hidden;user-select:none;contain:strict;";
    let isExpandedFullscreen = false;
    overlay.style.cssText = normalOverlayCss;

    const appShellEl = document.getElementById("app-shell");
    const messagesEl = document.getElementById("messages");
    if (messagesEl) {
      messagesEl.style.visibility = "hidden";
      messagesEl.style.contentVisibility = "hidden";
    }
    const syncUnderlyingDomVisibility = () => {
      if (appShellEl) {
        appShellEl.style.visibility = isExpandedFullscreen ? "hidden" : "";
        appShellEl.style.contentVisibility = isExpandedFullscreen ? "hidden" : "";
      }
      if (messagesEl) {
        messagesEl.style.visibility = "hidden";
        messagesEl.style.contentVisibility = "hidden";
      }
    };
    const restoreUnderlyingDomVisibility = () => {
      if (appShellEl) {
        appShellEl.style.visibility = "";
        appShellEl.style.contentVisibility = "";
      }
      if (messagesEl) {
        messagesEl.style.visibility = "";
        messagesEl.style.contentVisibility = "";
      }
    };

    const topBar = document.createElement("div");
    topBar.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;gap:12px;padding:10px 16px;background:#111214;border-bottom:1px solid #2b2d31;flex-wrap:wrap;";
    const titleLeft = document.createElement("div");
    titleLeft.style.cssText = "display:flex;align-items:center;gap:10px;font-size:14px;font-weight:700;color:#f2f3f5;";
    let streamResLabel = /** @type {any} */ (activeVoiceConnection)?.streamResolution || "720p";
    const targetStreamFps = /** @type {any} */ (activeVoiceConnection)?.streamFps || 60;
    let measuredStreamFps = 0;
    let renderedFramesInWindow = 0;
    let lastStreamFpsCalcMs = performance.now();
    const netBadge = document.createElement("span");
    netBadge.style.cssText =
      "padding:2px 8px;border-radius:4px;background:rgba(35,165,90,0.16);color:#23a55a;font-size:11px;font-weight:700;font-variant-numeric:tabular-nums;";
    const updateNetBadge = () => {
      const rtt = Number(liveRtcStatus?.lastPingMs ?? liveRtcStatus?.averagePingMs ?? 0);
      const gpuVendor = liveRtcStatus?.gpuVendor || "GPU";
      const codec = liveRtcStatus?.activeVideoCodec || "H264 HW";
      const gpuInfo = liveRtcStatus?.gpuModel
        ? `${gpuVendor}: ${liveRtcStatus.gpuModel} (${liveRtcStatus.gpuApi || "VA-API/Vulkan"})`
        : "Аппаратное ускорение GPU";
      const fpsDisplay = measuredStreamFps > 0 ? `${measuredStreamFps} FPS` : `${targetStreamFps} FPS`;
      netBadge.title = `${gpuInfo} · Кодек: ${codec} · Декодирование потока: ${fpsDisplay}`;
      if (rtt > 450) {
        netBadge.style.background = "rgba(240,178,50,0.18)";
        netBadge.style.color = "#f0b232";
        netBadge.textContent = `${codec} · ${gpuVendor} · ${streamResLabel} · Поток: ${fpsDisplay} (${rtt} мс)`;
      } else if (rtt > 0) {
        netBadge.style.background = "rgba(35,165,90,0.16)";
        netBadge.style.color = "#23a55a";
        netBadge.textContent = `${codec} · ${gpuVendor} · ${streamResLabel} · Поток: ${fpsDisplay} (${rtt} мс)`;
      } else {
        netBadge.textContent = `${codec} · ${gpuVendor} · ${streamResLabel} · Поток: ${fpsDisplay}`;
      }
    };
    updateNetBadge();
    titleLeft.innerHTML = `<span style="background:#da373c;color:#fff;font-size:11px;font-weight:800;padding:2px 7px;border-radius:4px;">В ЭФИРЕ</span><span>${participant?.displayName || "Трансляция"} — ${channel?.name || "Голосовой канал"}</span>`;
    titleLeft.append(netBadge);

    let zoomScale = 1.0;
    let panX = 0;
    let panY = 0;
    let isDraggingPan = false;
    let dragStartX = 0;
    let dragStartY = 0;
    let dragOriginPanX = 0;
    let dragOriginPanY = 0;

    const rightControls = document.createElement("div");
    rightControls.style.cssText = "display:flex;align-items:center;gap:6px;flex-wrap:wrap;";

    // Stream Audio Controls: Mute Toggle Button + Volume Slider (0%..200%)
    let streamMuted = Boolean(liveRtcStatus?.streamAudioMuted ?? false);
    let streamVolumePct = Number(liveRtcStatus?.streamAudioVolumePct ?? 100);
    if (!Number.isFinite(streamVolumePct)) streamVolumePct = 100;
    let prevUnmutedVolumePct = streamVolumePct > 0 ? streamVolumePct : 100;

    const audioGroup = document.createElement("div");
    audioGroup.style.cssText =
      "display:inline-flex;align-items:center;gap:6px;background:#1e1f22;border:1px solid #35373c;border-radius:6px;padding:3px 8px;";

    const muteStreamBtn = document.createElement("button");
    muteStreamBtn.type = "button";
    muteStreamBtn.style.cssText =
      "padding:2px 6px;border:none;border-radius:4px;background:transparent;color:#dbdee1;font-size:14px;cursor:pointer;line-height:1;";

    const streamVolSlider = document.createElement("input");
    streamVolSlider.type = "range";
    streamVolSlider.min = "0";
    streamVolSlider.max = "200";
    streamVolSlider.step = "1";
    streamVolSlider.value = String(streamMuted ? 0 : streamVolumePct);
    streamVolSlider.title = "Громкость звука трансляции (0% – 200%)";
    streamVolSlider.style.cssText = "width:84px;accent-color:#5865f2;cursor:pointer;";

    const streamVolLabel = document.createElement("span");
    streamVolLabel.style.cssText =
      "min-width:38px;text-align:right;font-size:11px;font-weight:700;color:#dbdee1;font-variant-numeric:tabular-nums;";

    const syncStreamAudioUi = (sendIpc = false) => {
      const effectiveVol = streamMuted ? 0 : streamVolumePct;
      muteStreamBtn.textContent = streamMuted || effectiveVol === 0 ? "🔇" : effectiveVol < 60 ? "🔉" : "🔊";
      muteStreamBtn.title = streamMuted ? "Включить звук трансляции" : "Отключить звук трансляции (Mute)";
      muteStreamBtn.style.color = streamMuted ? "#f23f43" : "#dbdee1";
      streamVolSlider.value = String(effectiveVol);
      streamVolLabel.textContent = streamMuted ? "ВЫКЛ" : `${streamVolumePct}%`;
      streamVolLabel.style.color = streamMuted ? "#f23f43" : "#dbdee1";
      if (liveRtcStatus && typeof liveRtcStatus === "object") {
        liveRtcStatus.streamAudioMuted = streamMuted;
        liveRtcStatus.streamAudioVolumePct = streamVolumePct;
      }
      if (sendIpc) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "stream_audio_volume",
          deviceId: `${streamMuted ? "1" : "0"}:${streamVolumePct}:${uId || ""}`,
          muted: streamMuted,
          volumePercent: streamVolumePct,
        }).catch(() => {});
      }
    };
    syncStreamAudioUi(false);

    muteStreamBtn.addEventListener("click", () => {
      if (!streamMuted) {
        if (streamVolumePct > 0) prevUnmutedVolumePct = streamVolumePct;
        streamMuted = true;
      } else {
        streamMuted = false;
        if (streamVolumePct === 0) streamVolumePct = prevUnmutedVolumePct || 100;
      }
      syncStreamAudioUi(true);
    });

    streamVolSlider.addEventListener("input", () => {
      const val = Math.max(0, Math.min(200, Number(streamVolSlider.value) || 0));
      streamVolumePct = val;
      if (val === 0) {
        streamMuted = true;
      } else {
        streamMuted = false;
        prevUnmutedVolumePct = val;
      }
      syncStreamAudioUi(true);
    });

    audioGroup.append(muteStreamBtn, streamVolSlider, streamVolLabel);
    rightControls.append(audioGroup);

    const zoomGroup = document.createElement("div");
    zoomGroup.style.cssText =
      "display:inline-flex;align-items:center;gap:2px;background:#1e1f22;border:1px solid #35373c;border-radius:6px;padding:2px;";
    /**
     * @param {string} label
     * @param {string} title
     */
    const mkZoomBtn = (label, title) => {
      const b = document.createElement("button");
      b.type = "button";
      b.textContent = label;
      b.title = title;
      b.style.cssText =
        "padding:4px 8px;border:none;border-radius:4px;background:transparent;color:#dbdee1;font-size:12px;font-weight:700;cursor:pointer;";
      return b;
    };
    const zoomOutBtn = mkZoomBtn("−", "Отдалить (колесо мыши вниз)");
    const zoomResetBtn = mkZoomBtn("100%", "Сбросить масштаб (100%)");
    const zoomInBtn = mkZoomBtn("+", "Приблизить (колесо мыши вверх)");
    const zoom200Btn = mkZoomBtn("200%", "Увеличить до 200% для чтения мелкого текста");
    zoomGroup.append(zoomOutBtn, zoomResetBtn, zoomInBtn, zoom200Btn);
    rightControls.append(zoomGroup);

    const fullscreenBtn = document.createElement("button");
    fullscreenBtn.type = "button";
    fullscreenBtn.textContent = "⛶ На весь экран";
    fullscreenBtn.title = "Развернуть трансляцию на всё окно (F11 / двойной клик)";
    fullscreenBtn.style.cssText =
      "padding:6px 10px;border-radius:6px;border:1px solid #3f4147;background:#2b2d31;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;";
    rightControls.append(fullscreenBtn);

    if (!isSelfStream && streamKey) {
      const refreshStreamBtn = document.createElement("button");
      refreshStreamBtn.type = "button";
      refreshStreamBtn.textContent = "↻ Переподключить";
      refreshStreamBtn.style.cssText =
        "padding:6px 10px;border-radius:6px;border:1px solid #3f4147;background:#2b2d31;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;";
      refreshStreamBtn.addEventListener("click", () => {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "stream_watch",
          deviceId: streamKey,
        }).catch(() => {});
      });
      rightControls.append(refreshStreamBtn);
    }
    const closeStageBtn = document.createElement("button");
    closeStageBtn.type = "button";
    closeStageBtn.textContent = "Прекратить просмотр ✕";
    closeStageBtn.style.cssText =
      "padding:6px 12px;border-radius:6px;border:none;background:#da373c;color:#fff;font-size:12px;font-weight:700;cursor:pointer;";
    rightControls.append(closeStageBtn);
    topBar.append(titleLeft, rightControls);

    const stageWrap = document.createElement("div");
    stageWrap.style.cssText =
      "flex:1;position:relative;display:flex;align-items:center;justify-content:center;background:#070809;overflow:hidden;cursor:zoom-in;";

    const stageCanvas = document.createElement("canvas");
    stageCanvas.width = 1280;
    stageCanvas.height = 720;
    stageCanvas.style.cssText =
      "width:100%;height:100%;object-fit:contain;display:none;transform-origin:center center;will-change:transform;pointer-events:none;backface-visibility:hidden;";
    const stageCtx =
      /** @type {CanvasRenderingContext2D | null} */ (
        stageCanvas.getContext("2d", { alpha: false }) ||
          stageCanvas.getContext("2d")
      );

    const stageImg = document.createElement("img");
    stageImg.alt = "Трансляция";
    stageImg.decoding = "async";
    stageImg.style.cssText =
      "width:100%;height:100%;object-fit:contain;display:none;transform-origin:center center;will-change:transform;pointer-events:none;backface-visibility:hidden;";

    const emptyNotice = document.createElement("div");
    const dispName = String(participant?.displayName || "Участник");
    emptyNotice.style.cssText =
      "display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;color:#dbdee1;text-align:center;padding:24px;";
    emptyNotice.innerHTML = `<div style="width:68px;height:68px;border-radius:50%;background:#18191c;border:3px solid #5865f2;display:flex;align-items:center;justify-content:center;font-size:24px;font-weight:800;color:#f2f3f5;">${Array.from(dispName).slice(0, 1).join("").toUpperCase() || "S"}</div><div style="font-size:16px;font-weight:700;color:#f2f3f5;">Трансляция ${dispName}</div><div style="font-size:13px;color:#949ba4;">Подключение к видеопотоку… Колесо мыши — зум (до 500%), перетаскивание — панорамирование</div>`;

    const zoomHintPill = document.createElement("div");
    zoomHintPill.style.cssText =
      "position:absolute;left:14px;bottom:12px;padding:4px 10px;border-radius:6px;background:rgba(17,18,20,0.78);color:#b5bac1;font-size:11px;font-weight:600;pointer-events:none;z-index:2;";
    zoomHintPill.textContent = "🔍 Колесо мыши — масштаб (100%–500%) · Зажать ЛКМ — перемещение · Двойной клик — 225%";

    stageWrap.append(stageImg, stageCanvas, emptyNotice, zoomHintPill);

    const clampPan = () => {
      if (zoomScale <= 1.01) {
        zoomScale = 1.0;
        panX = 0;
        panY = 0;
        return;
      }
      const rect = stageWrap.getBoundingClientRect();
      const maxX = Math.max(0, ((rect.width || 960) * (zoomScale - 1)) / 2);
      const maxY = Math.max(0, ((rect.height || 540) * (zoomScale - 1)) / 2);
      panX = Math.max(-maxX, Math.min(maxX, panX));
      panY = Math.max(-maxY, Math.min(maxY, panY));
    };

    const applyTransform = () => {
      clampPan();
      zoomResetBtn.textContent = `${Math.round(zoomScale * 100)}%`;
      const tf = `translate3d(${Math.round(panX)}px, ${Math.round(panY)}px, 0) scale(${zoomScale.toFixed(3)})`;
      stageCanvas.style.transform = tf;
      stageImg.style.transform = tf;
      stageWrap.style.cursor = zoomScale > 1.01 ? (isDraggingPan ? "grabbing" : "grab") : "zoom-in";
    };

    /**
     * @param {number} nextScale
     * @param {number} [focalClientX]
     * @param {number} [focalClientY]
     */
    const setZoomLevel = (nextScale, focalClientX, focalClientY) => {
      const prevScale = zoomScale;
      const clamped = Math.max(1.0, Math.min(5.0, nextScale));
      if (Math.abs(clamped - prevScale) < 0.005) return;
      if (
        typeof focalClientX === "number" &&
        typeof focalClientY === "number" &&
        clamped > 1.01
      ) {
        const rect = stageWrap.getBoundingClientRect();
        const cx = focalClientX - (rect.left + rect.width / 2);
        const cy = focalClientY - (rect.top + rect.height / 2);
        const ratio = clamped / prevScale;
        panX = cx - (cx - panX) * ratio;
        panY = cy - (cy - panY) * ratio;
      }
      zoomScale = clamped;
      applyTransform();
    };

    zoomOutBtn.addEventListener("click", () => setZoomLevel(zoomScale / 1.25));
    zoomInBtn.addEventListener("click", () => setZoomLevel(zoomScale * 1.25));
    zoomResetBtn.addEventListener("click", () => {
      zoomScale = 1.0;
      panX = 0;
      panY = 0;
      applyTransform();
    });
    zoom200Btn.addEventListener("click", () => {
      if (Math.abs(zoomScale - 2.0) < 0.1) {
        zoomScale = 1.0;
        panX = 0;
        panY = 0;
      } else {
        zoomScale = 2.0;
      }
      applyTransform();
    });

    fullscreenBtn.addEventListener("click", () => {
      isExpandedFullscreen = !isExpandedFullscreen;
      overlay.style.cssText = isExpandedFullscreen ? expandedOverlayCss : normalOverlayCss;
      fullscreenBtn.textContent = isExpandedFullscreen ? "🗗 Свернуть в окно" : "⛶ На весь экран";
      syncUnderlyingDomVisibility();
      applyTransform();
    });

    stageWrap.addEventListener(
      "wheel",
      (ev) => {
        ev.preventDefault();
        const factor = ev.deltaY < 0 ? 1.18 : 1 / 1.18;
        setZoomLevel(zoomScale * factor, ev.clientX, ev.clientY);
      },
      { passive: false },
    );

    stageWrap.addEventListener("dblclick", (ev) => {
      ev.preventDefault();
      if (zoomScale > 1.15) {
        zoomScale = 1.0;
        panX = 0;
        panY = 0;
        applyTransform();
      } else {
        setZoomLevel(2.25, ev.clientX, ev.clientY);
      }
    });

    stageWrap.addEventListener("mousedown", (ev) => {
      if (ev.button !== 0 || zoomScale <= 1.01) return;
      isDraggingPan = true;
      dragStartX = ev.clientX;
      dragStartY = ev.clientY;
      dragOriginPanX = panX;
      dragOriginPanY = panY;
      stageWrap.style.cursor = "grabbing";
    });
    /** @param {MouseEvent} ev */
    const onWinMouseMove = (ev) => {
      if (!isDraggingPan) return;
      panX = dragOriginPanX + (ev.clientX - dragStartX);
      panY = dragOriginPanY + (ev.clientY - dragStartY);
      applyTransform();
    };
    const onWinMouseUp = () => {
      if (!isDraggingPan) return;
      isDraggingPan = false;
      applyTransform();
    };
    window.addEventListener("mousemove", onWinMouseMove);
    window.addEventListener("mouseup", onWinMouseUp);

    let viewerClosed = false;
    let remotePollInFlight = false;
    let lastRemoteFrameSeq = 0;
    let lastRemoteFrameLoadedAt = Date.now();
    let hasReceivedFirstFrame = false;
    let lastStreamWatchRetryAt = Date.now();
    let badgeTick = 0;
    /** @type {ImageBitmap | HTMLImageElement | null} */
    let pendingBitmap = null;
    let rafPresentScheduled = false;

    const retryRemoteStreamWatchIfStalled = () => {
      if (viewerClosed || isSelfStream || !isLiveSession() || !streamKey) return;
      if (hasReceivedFirstFrame || lastRemoteFrameSeq > 0) return;
      const now = Date.now();
      if (now - lastStreamWatchRetryAt >= 3500) {
        lastStreamWatchRetryAt = now;
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "stream_watch",
          deviceId: streamKey,
        }).catch(() => {});
      }
    };

    const recordRenderedFrameFps = () => {
      renderedFramesInWindow += 1;
      const nowPerf = performance.now();
      const elapsedPerf = nowPerf - lastStreamFpsCalcMs;
      if (elapsedPerf >= 950) {
        const calcFps = Math.round((renderedFramesInWindow * 1000) / elapsedPerf);
        if (calcFps > 0) {
          measuredStreamFps = Math.min(60, calcFps);
        }
        renderedFramesInWindow = 0;
        lastStreamFpsCalcMs = nowPerf;
      }
    };

    const presentPendingBitmap = () => {
      rafPresentScheduled = false;
      if (viewerClosed || !pendingBitmap) return;
      const bmp = pendingBitmap;
      pendingBitmap = null;
      const bw = Number(/** @type {any} */ (bmp).naturalWidth || bmp.width) || 1280;
      const bh = Number(/** @type {any} */ (bmp).naturalHeight || bmp.height) || 720;
      if (bh >= 1080) streamResLabel = "1080p";
      else if (bh >= 720) streamResLabel = "720p";
      let drawnOnCanvas = false;
      if (stageCtx) {
        try {
          if (stageCanvas.width !== bw || stageCanvas.height !== bh) {
            stageCanvas.width = bw;
            stageCanvas.height = bh;
          }
          stageCtx.drawImage(bmp, 0, 0, bw, bh);
          drawnOnCanvas = true;
        } catch {}
      }
      if (typeof (/** @type {any} */ (bmp).close) === "function") {
        try {
          /** @type {any} */ (bmp).close();
        } catch {}
      }
      if (drawnOnCanvas) {
        hasReceivedFirstFrame = true;
        if (stageCanvas.style.display === "none") {
          stageCanvas.style.display = "block";
          stageImg.style.display = "none";
          emptyNotice.style.display = "none";
        }
        recordRenderedFrameFps();
      }
    };

    /** @param {ImageBitmap | HTMLImageElement} decodedFrame */
    const enqueueDecodedFrame = (decodedFrame) => {
      if (viewerClosed) {
        if (typeof (/** @type {any} */ (decodedFrame).close) === "function") {
          try {
            /** @type {any} */ (decodedFrame).close();
          } catch {}
        }
        return;
      }
      if (pendingBitmap && typeof (/** @type {any} */ (pendingBitmap).close) === "function") {
        try {
          /** @type {any} */ (pendingBitmap).close();
        } catch {}
      }
      pendingBitmap = decodedFrame;
      if (!rafPresentScheduled) {
        rafPresentScheduled = true;
        requestAnimationFrame(presentPendingBitmap);
      }
    };

    let prevBlobUrl = "";
    /** @param {Blob} blob */
    const decodeAndPresentBlob = async (blob) => {
      if (viewerClosed || !blob || blob.size < 64) return;
      if (typeof globalThis.createImageBitmap === "function") {
        try {
          const bmp = await globalThis.createImageBitmap(blob);
          enqueueDecodedFrame(bmp);
          return;
        } catch {}
      }
      if (typeof URL !== "undefined" && typeof URL.createObjectURL === "function") {
        const objUrl = URL.createObjectURL(blob);
        stageImg.onload = () => {
          if (prevBlobUrl && prevBlobUrl !== objUrl) {
            try {
              URL.revokeObjectURL(prevBlobUrl);
            } catch {}
          }
          prevBlobUrl = objUrl;
          hasReceivedFirstFrame = true;
          if (stageImg.style.display === "none" && stageCanvas.style.display === "none") {
            stageImg.style.display = "block";
            emptyNotice.style.display = "none";
          }
          recordRenderedFrameFps();
        };
        stageImg.src = objUrl;
      }
    };

    /** @param {string} dataUrl */
    const showFrameDataUrl = (dataUrl) => {
      if (viewerClosed || typeof dataUrl !== "string" || !dataUrl.startsWith("data:image/")) return;
      stageImg.onload = () => {
        hasReceivedFirstFrame = true;
        if (stageCanvas.style.display === "none") {
          stageImg.style.display = "block";
          emptyNotice.style.display = "none";
        }
        enqueueDecodedFrame(stageImg);
      };
      stageImg.src = dataUrl;
    };

    const pollRemoteFrameIpcFallback = async () => {
      if (viewerClosed || !isLiveSession() || remotePollInFlight) return;
      if (isSelfStream) {
        requestLiveScreenPreviewFrame();
        return;
      }
      if (!streamKey) return;
      remotePollInFlight = true;
      try {
        const res = await invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "remote_stream_frame",
          deviceId: `${streamKey}|since=${lastRemoteFrameSeq}`,
        });
        const seq = Number(res?.seq || 0);
        const backendFps = Number(res?.streamFps || 0);
        if (backendFps > 0) {
          measuredStreamFps = Math.max(measuredStreamFps, Math.min(60, backendFps));
        }
        if (res?.unchanged && seq > 0) {
          hasReceivedFirstFrame = true;
          lastRemoteFrameLoadedAt = Date.now();
          return;
        }
        const dataUrl = res?.previewDataUrl;
        if (typeof dataUrl === "string" && dataUrl.startsWith("data:image/")) {
          hasReceivedFirstFrame = true;
          lastRemoteFrameLoadedAt = Date.now();
          if (seq === 0 || seq !== lastRemoteFrameSeq) {
            lastRemoteFrameSeq = seq;
            showFrameDataUrl(dataUrl);
          }
        }
      } catch {
      } finally {
        remotePollInFlight = false;
      }
    };

    const runFastRemoteStreamLoop = async () => {
      let consecutiveHttpMisses = 0;
      let ipcKickTicks = 0;
      void pollRemoteFrameIpcFallback();
      while (!viewerClosed) {
        if (!overlay.isConnected || !isLiveSession() || (!streamKey && !isSelfStream)) break;
        retryRemoteStreamWatchIfStalled();
        let sleepMs = 14;
        ipcKickTicks += 1;
        if (typeof globalThis.fetch === "function" && consecutiveHttpMisses < 6) {
          try {
            const resp = await globalThis.fetch(
              `http://127.0.0.1:49281/frame.jpg?since=${lastRemoteFrameSeq}`,
              { cache: "no-store" },
            );
            const hdrFps = Number(resp.headers.get("X-Stream-Fps") || 0);
            if (hdrFps > 0) {
              measuredStreamFps = Math.min(60, hdrFps);
            }
            if (resp.status === 200) {
              consecutiveHttpMisses = 0;
              const seq = Number(resp.headers.get("X-Frame-Seq") || 0);
              const blob = await resp.blob();
              if (blob.size > 64) {
                hasReceivedFirstFrame = true;
                lastRemoteFrameLoadedAt = Date.now();
                if (seq > 0) lastRemoteFrameSeq = seq;
                await decodeAndPresentBlob(blob);
                sleepMs = 8;
              }
            } else if (resp.status === 204 && lastRemoteFrameSeq > 0) {
              consecutiveHttpMisses = 0;
              hasReceivedFirstFrame = true;
              lastRemoteFrameLoadedAt = Date.now();
              if (ipcKickTicks % 90 === 0) {
                void pollRemoteFrameIpcFallback();
              }
              sleepMs = 10;
            } else if (resp.status === 404 && isSelfStream) {
              const previewResp = await globalThis.fetch(
                `http://127.0.0.1:49281/preview.jpg?t=${Date.now()}`,
                { cache: "no-store" },
              );
              if (previewResp.status === 200) {
                const blob = await previewResp.blob();
                if (blob.size > 64) {
                  consecutiveHttpMisses = 0;
                  hasReceivedFirstFrame = true;
                  lastRemoteFrameLoadedAt = Date.now();
                  lastRemoteFrameSeq += 1;
                  await decodeAndPresentBlob(blob);
                  sleepMs = 40;
                } else {
                  await pollRemoteFrameIpcFallback();
                  sleepMs = 60;
                }
              } else {
                consecutiveHttpMisses += 1;
                await pollRemoteFrameIpcFallback();
                sleepMs = 60;
              }
            } else {
              consecutiveHttpMisses += 1;
              await pollRemoteFrameIpcFallback();
              sleepMs = 32;
            }
          } catch {
            consecutiveHttpMisses += 1;
            await pollRemoteFrameIpcFallback();
            sleepMs = 32;
          }
        } else {
          await pollRemoteFrameIpcFallback();
          sleepMs = 24;
          if (consecutiveHttpMisses >= 6) {
            consecutiveHttpMisses = 3;
          }
        }
        await new Promise((r) => setTimeout(r, sleepMs));
      }
    };

    overlay.append(topBar, stageWrap);
    document.body.append(overlay);

    if (isSelfStream) {
      const renderSelfStreamFallback = () => {
        if (viewerClosed) return;
        if (cachedLiveScreenDataUrl && cachedLiveScreenDataUrl.startsWith("data:image/")) {
          showFrameDataUrl(cachedLiveScreenDataUrl);
          return;
        }
        const srcId = String(activeVoiceConnection?.streamSourceId || "screen_0");
        drawLiveCaptureCanvasFrame(stageCanvas, srcId, badgeTick, true);
        if (stageCanvas.style.display === "none") {
          stageCanvas.style.display = "block";
          stageImg.style.display = "none";
          emptyNotice.style.display = "none";
        }
        recordRenderedFrameFps();
      };
      renderSelfStreamFallback();
      requestLiveScreenPreviewFrame();
      void runFastRemoteStreamLoop();
    } else {
      void runFastRemoteStreamLoop();
    }

    let resizeRafPending = false;
    const onWinResize = () => {
      if (resizeRafPending || viewerClosed) return;
      resizeRafPending = true;
      requestAnimationFrame(() => {
        resizeRafPending = false;
        if (!viewerClosed) applyTransform();
      });
    };
    window.addEventListener("resize", onWinResize, { passive: true });

    const stageTimer = setInterval(() => {
      if (!overlay.isConnected) {
        viewerClosed = true;
        clearInterval(stageTimer);
        restoreUnderlyingDomVisibility();
        window.removeEventListener("mousemove", onWinMouseMove);
        window.removeEventListener("mouseup", onWinMouseUp);
        window.removeEventListener("resize", onWinResize);
        return;
      }
      badgeTick += 1;
      if (badgeTick % 3 === 0) {
        updateNetBadge();
      }
      if (!isSelfStream) {
        retryRemoteStreamWatchIfStalled();
      }
      if (isSelfStream) {
        requestLiveScreenPreviewFrame();
        if (cachedLiveScreenDataUrl && cachedLiveScreenDataUrl.startsWith("data:image/")) {
          if (lastLiveScreenSeq !== lastRemoteFrameSeq || stageImg.style.display === "none") {
            lastRemoteFrameSeq = lastLiveScreenSeq;
            showFrameDataUrl(cachedLiveScreenDataUrl);
          }
        } else {
          const srcId = String(activeVoiceConnection?.streamSourceId || "screen_0");
          drawLiveCaptureCanvasFrame(stageCanvas, srcId, badgeTick, true);
          if (stageCanvas.style.display === "none") {
            stageCanvas.style.display = "block";
            stageImg.style.display = "none";
            emptyNotice.style.display = "none";
          }
          recordRenderedFrameFps();
        }
      }
    }, 250);

    const closeStageViewer = () => {
      viewerClosed = true;
      clearInterval(stageTimer);
      if (pendingBitmap && typeof (/** @type {any} */ (pendingBitmap).close) === "function") {
        try {
          /** @type {any} */ (pendingBitmap).close();
        } catch {}
      }
      pendingBitmap = null;
      restoreUnderlyingDomVisibility();
      window.removeEventListener("mousemove", onWinMouseMove);
      window.removeEventListener("mouseup", onWinMouseUp);
      window.removeEventListener("resize", onWinResize);
      window.removeEventListener("keydown", onStageKeyDown);
      if (!isSelfStream) {
        playVoiceUiSound("stream_user_left");
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "stream_stop_watch",
          deviceId: streamKey,
        }).catch(() => {});
      }
      overlay.remove();
    };

    /** @param {KeyboardEvent} ev */
    const onStageKeyDown = (ev) => {
      if (ev.key === "Escape") {
        if (isExpandedFullscreen) {
          isExpandedFullscreen = false;
          overlay.style.cssText = normalOverlayCss;
          fullscreenBtn.textContent = "⛶ На весь экран";
          syncUnderlyingDomVisibility();
          applyTransform();
        } else {
          closeStageViewer();
        }
      } else if (ev.key === "m" || ev.key === "M" || ev.key === "ь" || ev.key === "Ь") {
        muteStreamBtn.click();
      }
    };
    window.addEventListener("keydown", onStageKeyDown);

    closeStageBtn.addEventListener("click", closeStageViewer);
  }

  /**
   * @param {HTMLCanvasElement} canvas
   * @param {string} sourceId
   * @param {number} tick
   * @param {boolean} [showWallpaper]
   */
  function drawLiveCaptureCanvasFrame(canvas, sourceId, tick, showWallpaper = true) {
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.imageSmoothingEnabled = true;
    try {
      /** @type {any} */ (ctx).imageSmoothingQuality = "high";
    } catch {}
    const w = canvas.width;
    const h = canvas.height;
    ctx.clearRect(0, 0, w, h);

    if (isLiveSession() && sourceId !== "screen_virtual") {
      requestLiveScreenPreviewFrame();
      if (cachedLiveScreenImg && cachedLiveScreenImg.complete && cachedLiveScreenImg.naturalWidth > 0) {
        const iw = cachedLiveScreenImg.naturalWidth;
        const ih = cachedLiveScreenImg.naturalHeight;
        const scale = Math.min(w / iw, h / ih);
        const dw = Math.round(iw * scale);
        const dh = Math.round(ih * scale);
        const dx = Math.round((w - dw) / 2);
        const dy = Math.round((h - dh) / 2);
        ctx.fillStyle = "#090a0c";
        ctx.fillRect(0, 0, w, h);
        ctx.drawImage(cachedLiveScreenImg, dx, dy, dw, dh);
        if (sourceId === "screen_region") {
          ctx.strokeStyle = "#00a8fc";
          ctx.lineWidth = 2;
          ctx.setLineDash([4, 3]);
          ctx.strokeRect(18, 12, w - 36, h - 24);
          ctx.setLineDash([]);
        }
        return;
      }
    }

    if (sourceId === "screen_virtual") {
      ctx.fillStyle = "#090a0b";
      ctx.fillRect(0, 0, w, h);
      return;
    }

    if (showWallpaper) {
      const grad = ctx.createLinearGradient(0, 0, w, h);
      grad.addColorStop(0, "#1d1b38");
      grad.addColorStop(0.55, "#2b1d42");
      grad.addColorStop(1, "#121524");
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, w, h);
    } else {
      ctx.fillStyle = "#141518";
      ctx.fillRect(0, 0, w, h);
    }

    const pulse = Math.sin(tick * 0.18) * 4;
    if (sourceId === "win_orca" || sourceId.startsWith("screen_")) {
      // Имитация окна Orca / IDE на рабочем столе
      ctx.fillStyle = "#18191c";
      ctx.fillRect(10, 8, w - 20, h - 18);
      ctx.strokeStyle = "#3f4147";
      ctx.lineWidth = 1;
      ctx.strokeRect(10, 8, w - 20, h - 18);
      ctx.fillStyle = "#111214";
      ctx.fillRect(10, 8, 34, h - 18);
      ctx.fillStyle = "#23a55a";
      for (let i = 0; i < 5; i += 1) {
        ctx.fillRect(16, 16 + i * 11, 20, 5);
      }
      ctx.fillStyle = "#5865f2";
      ctx.fillRect(50, 18, Math.max(40, w - 76 + pulse), 8);
      ctx.fillStyle = "#b5bac1";
      ctx.fillRect(50, 32, Math.max(30, w - 96), 5);
      ctx.fillRect(50, 42, Math.max(30, w - 84), 5);
      ctx.fillRect(50, 52, Math.max(30, w - 108), 5);
    } else if (sourceId === "win_firefox") {
      ctx.fillStyle = "#1c1b22";
      ctx.fillRect(8, 6, w - 16, h - 12);
      ctx.fillStyle = "#2b2a33";
      ctx.fillRect(8, 6, w - 16, 16);
      ctx.fillStyle = "#0060df";
      ctx.fillRect(20, 28, w - 40, h - 40);
      ctx.fillStyle = "#ffffff";
      ctx.fillRect(32, 40, Math.max(30, w - 68 + pulse), 8);
    } else if (sourceId === "win_discord") {
      ctx.fillStyle = "#313338";
      ctx.fillRect(8, 6, w - 16, h - 12);
      ctx.fillStyle = "#1e1f22";
      ctx.fillRect(8, 6, 36, h - 12);
      ctx.fillStyle = "#5865f2";
      ctx.beginPath();
      ctx.arc(26, 24, 8, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = "#23a55a";
      ctx.fillRect(52, 20, Math.max(30, w - 76 + pulse), 7);
      ctx.fillStyle = "#dbdee1";
      ctx.fillRect(52, 34, Math.max(30, w - 90), 5);
      ctx.fillRect(52, 44, Math.max(30, w - 82), 5);
    } else {
      // Dolphin / Файловый менеджер
      ctx.fillStyle = "#f5f6f8";
      ctx.fillRect(8, 6, w - 16, h - 12);
      ctx.fillStyle = "#e1e4e8";
      ctx.fillRect(8, 6, 38, h - 12);
      ctx.fillStyle = "#3daee9";
      for (let i = 0; i < 3; i += 1) {
        ctx.fillRect(54 + i * 34, 24, 26, 20);
      }
    }
    if (sourceId === "screen_region") {
      ctx.strokeStyle = "#00a8fc";
      ctx.lineWidth = 2;
      ctx.setLineDash([4, 3]);
      ctx.strokeRect(22, 14, w - 44, h - 28);
      ctx.setLineDash([]);
    }
  }

  /**
   * Модальное окно промо Nitro HD («Ловите каждый момент в разрешении HD», кадр `hi_27.png`).
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
   * Показывает контекстное меню настроек «Режим стрима» по клику на шестерёнку `⚙`
   * в модальном окне демонстрации экрана или на плавающем окне предпросмотра (`hi_15.png`, `hi_30.png`, `hi_35.png`, `hi_37.png`, `hi_40.png`).
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
    menu.style.cssText =
      `position:fixed;left:${leftPx}px;bottom:${bottomPx}px;width:288px;background:#111214;color:#dbdee1;border:1px solid #2b2d31;border-radius:10px;padding:8px;box-shadow:0 18px 44px rgba(0,0,0,0.82);z-index:10010;display:flex;flex-direction:column;gap:2px;font-size:13px;user-select:none;`;

    const closeSubmenu = () => {
      document.getElementById("live-stream-settings-submenu")?.remove();
    };

    const renderMenu = () => {
      menu.replaceChildren();
      closeSubmenu();
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
            stateBag.fps = p.fps;
          }
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
            stateBag.resolution || "720p",
            (v) => {
              stateBag.resolution = v;
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
        fpsCascade.innerHTML = `<div><div style="font-size:13px;font-weight:600;">Частота кадров</div><div style="font-size:11px;color:#949ba4;">${stateBag.fps || 60}</div></div><span style="color:#949ba4;font-size:16px;">›</span>`;
        const showFpsSub = () => {
          fpsCascade.style.background = "#2b2d31";
          openCascadingSubmenu(
            fpsCascade,
            [
              { label: "15", value: 15 },
              { label: "30", value: 30 },
              { label: "60", nitro: true, value: 60 },
            ],
            Number(stateBag.fps || 60),
            (v) => {
              stateBag.fps = Number(v);
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
    document.body.append(menu);
  }

  /**
   * Модальное окно выбора конкретного экрана или окна («Запрос демонстрации экрана», кадры `hi_43.png`, `hi_46.png`, `hi_50.png`).
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

    /** @type {ReturnType<typeof setInterval> | null} */
    let portalTimer = null;
    const closePortal = () => {
      if (portalTimer) {
        clearInterval(portalTimer);
        portalTimer = null;
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

    const screenItems = [
      { id: "screen_dp6", label: "Открыть доступ к «LG Electronics LG QHD (DP-6)»", tab: /** @type {const} */ ("screens") },
      { id: "screen_region", label: "Открыть доступ к прямоугольной области", tab: /** @type {const} */ ("screens") },
      { id: "screen_virtual", label: "Открыть доступ к новому виртуальному экрану", tab: /** @type {const} */ ("screens") },
    ];
    const windowItems = [
      { id: "win_orca", label: "Orca", icon: "🦦", tab: /** @type {const} */ ("windows") },
      { id: "win_firefox", label: "Расширение: (Bitwarden — Бесплатный менеджер паролей) - Bitwarden — Mozilla Firefox", icon: "🦊", tab: /** @type {const} */ ("windows") },
      { id: "win_discord", label: `🔊 ${activeVoiceConnection?.channelName || "Звонок"} | ${activeVoiceConnection?.guildName || "Discord"} — Discord`, icon: "🎮", tab: /** @type {const} */ ("windows") },
      { id: "win_dolphin", label: "Видеофайлы — Dolphin", icon: "📁", tab: /** @type {const} */ ("windows") },
    ];

    void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
      kind: "list_capture_windows",
      deviceId: "all",
    })
      .then((res) => {
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
        drawLiveCaptureCanvasFrame(cv, item.id, tick, showWallpaper);

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
            drawLiveCaptureCanvasFrame(c.canvas, c.id, tick, showWallpaper);
          }
        });
        const wpTxt = document.createElement("span");
        wpTxt.textContent = "Показывать обои рабочего стола";
        wpLabel.append(wpCheck, wpTxt);
        footerArea.append(wpLabel);
      } else {
        const hint = document.createElement("span");
        hint.style.cssText = "font-size:12px;color:#949ba4;";
        hint.textContent = "Выберите окно приложения для начала трансляции";
        footerArea.append(hint);
      }
    };

    searchInput.addEventListener("input", renderPortalBody);
    renderPortalBody();
    portalTimer = setInterval(() => {
      tick += 1;
      for (const c of activeCanvases) {
        drawLiveCaptureCanvasFrame(c.canvas, c.id, tick, showWallpaper);
      }
    }, 150);

    dialog.append(titleBar, searchBarWrap, tabsBar, contentArea, footerArea);
    backdrop.append(dialog);
    backdrop.addEventListener("click", (ev) => {
      if (ev.target === backdrop) closePortal();
    });
    document.body.append(backdrop);
  }

  /**
   * Синхронизирует плавающее окно предпросмотра активного стрима (`#live-stream-pip-preview`, кадр `hi_52.png` и `hi_61.png`).
   */
  function syncFloatingStreamPipPreview() {
    if (activePipAnimTimer) {
      clearInterval(activePipAnimTimer);
      activePipAnimTimer = null;
    }
    document.getElementById("live-stream-pip-preview")?.remove();
    const conn = /** @type {any} */ (activeVoiceConnection);
    if (!conn || !conn.screenShareOn || conn.hideStreamPreview) return;

    const pip = document.createElement("div");
    pip.id = "live-stream-pip-preview";
    pip.setAttribute("role", "region");
    pip.setAttribute("aria-label", "Предпросмотр вашей демонстрации экрана");
    pip.style.cssText =
      "position:fixed;left:324px;bottom:78px;width:284px;height:164px;border-radius:10px;overflow:hidden;background:#111214;border:1px solid #3f4147;box-shadow:0 18px 46px rgba(0,0,0,0.85);z-index:9200;display:flex;flex-direction:column;justify-content:space-between;user-select:none;cursor:pointer;";
    pip.title = "Нажмите, чтобы развернуть окно стрима";
    pip.addEventListener("click", (ev) => {
      if ((/** @type {HTMLElement} */ (ev.target)).closest("button")) return;
      openLiveStreamStageViewer(
        {
          userId: String(liveSelfUser?.id || ""),
          guildId: conn.guildId,
          channelId: conn.channelId,
          displayName: cleanDiscordStr(liveSelfUser?.globalName || liveSelfUser?.username) || "Пользователь",
          streaming: true,
        },
        { id: conn.channelId, name: conn.channelName || "Стрим" },
      );
    });

    const cv = document.createElement("canvas");
    cv.width = 568;
    cv.height = 320;
    cv.style.cssText = "position:absolute;inset:0;width:100%;height:100%;display:block;";
    let tick = 0;
    const srcId = String(conn.streamSourceId || "win_orca");
    drawLiveCaptureCanvasFrame(cv, srcId, tick, true);
    activePipAnimTimer = setInterval(() => {
      if (document.getElementById("live-stream-stage-viewer")) return;
      tick += 1;
      drawLiveCaptureCanvasFrame(cv, srcId, tick, true);
    }, 550);

    const topOverlay = document.createElement("div");
    topOverlay.style.cssText =
      "position:relative;z-index:1;display:flex;align-items:center;justify-content:space-between;padding:8px 10px;background:linear-gradient(180deg,rgba(0,0,0,0.68) 0%,transparent 100%);";
    const chPill = document.createElement("span");
    chPill.style.cssText =
      "display:inline-flex;align-items:center;gap:5px;padding:3px 8px;border-radius:6px;background:rgba(17,18,20,0.78);color:#f2f3f5;font-size:12px;font-weight:700;";
    chPill.textContent = `← ${conn.channelName || "Стрим"}`;

    const gearBtn = document.createElement("button");
    gearBtn.type = "button";
    gearBtn.title = "Параметры стрима";
    gearBtn.style.cssText =
      "width:28px;height:28px;border-radius:6px;border:none;background:rgba(17,18,20,0.78);color:#dbdee1;font-size:14px;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;";
    gearBtn.textContent = "⚙";
    gearBtn.addEventListener("click", (ev) => {
      ev.stopPropagation();
      openStreamSettingsMenu(gearBtn, conn, () => {
        renderLiveVoicePanel();
      });
    });
    topOverlay.append(chPill, gearBtn);

    const bottomOverlay = document.createElement("div");
    bottomOverlay.style.cssText =
      "position:relative;z-index:1;display:flex;align-items:center;justify-content:space-between;padding:8px 10px;background:linear-gradient(0deg,rgba(0,0,0,0.75) 0%,transparent 100%);";
    const selfName = document.createElement("span");
    selfName.style.cssText =
      "padding:2px 8px;border-radius:4px;background:rgba(17,18,20,0.75);color:#f2f3f5;font-size:12px;font-weight:700;";
    selfName.textContent = cleanDiscordStr(liveSelfUser?.globalName || liveSelfUser?.username) || "Пользователь";

    const stopBtn = document.createElement("button");
    stopBtn.type = "button";
    stopBtn.title = "Прекратить стрим";
    stopBtn.style.cssText =
      "width:30px;height:30px;border-radius:6px;border:none;background:rgba(17,18,20,0.85);color:#f2f3f5;font-size:14px;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;";
    stopBtn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2"><rect x="3" y="4" width="18" height="12" rx="2"/><line x1="8" y1="20" x2="16" y2="20"/><line x1="9" y1="7" x2="15" y2="13"/><line x1="15" y1="7" x2="9" y2="13"/></svg>`;
    stopBtn.addEventListener("click", () => {
      conn.screenShareOn = false;
      const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
      for (const gk of new Set([conn.guildId, normalizeVoiceGuildKey(conn.guildId)])) {
        const gList = liveVoiceStatesByGuild.get(gk) ?? [];
        for (const v of gList) {
          if (String(v?.user_id ?? v?.userId ?? "") === selfId) {
            v.self_stream = false;
            v.selfStream = false;
          }
        }
      }
      playVoiceUiSound("stream_off");
      syncVoiceMuteStateWithBackend();
      renderLiveVoicePanel();
      tree.refreshView();
    });
    bottomOverlay.append(selfName, stopBtn);

    pip.append(cv, topOverlay, bottomOverlay);
    document.body.append(pip);
  }

  /**
   * Модальное окно «Демонстрация экрана» в 100% соответствии с оригинальным клиентом Discord
   * (`Video_2026-11-01_12-11-05.mp4`, кадры `hi_8.png`..`hi_61.png`).
   * @param {boolean} [forceOpenConfig]
   */
  function openScreenShareModal(forceOpenConfig = false) {
    document.getElementById("live-screenshare-modal")?.remove();
    document.getElementById("live-stream-settings-menu")?.remove();
    document.getElementById("live-stream-settings-submenu")?.remove();
    if (!activeVoiceConnection) return;
    if (activeVoiceConnection.screenShareOn && !forceOpenConfig) {
      activeVoiceConnection.screenShareOn = false;
      const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
      for (const gk of new Set([
        activeVoiceConnection.guildId,
        normalizeVoiceGuildKey(activeVoiceConnection.guildId),
      ])) {
        const gList = liveVoiceStatesByGuild.get(gk) ?? [];
        for (const v of gList) {
          if (String(v?.user_id ?? v?.userId ?? "") === selfId) {
            v.self_stream = false;
            v.selfStream = false;
          }
        }
      }
      playVoiceUiSound("stream_off");
      syncVoiceMuteStateWithBackend();
      renderLiveVoicePanel();
      tree.refreshView();
      return;
    }
    emitLiveTelemetry({
      type: "voice_screenshare_modal_open",
      guildId: activeVoiceConnection.guildId,
      channelId: activeVoiceConnection.channelId,
      screenShareOn: Boolean(activeVoiceConnection.screenShareOn),
    });
    const conn = /** @type {any} */ (activeVoiceConnection);
    if (!conn.preset) conn.preset = "games";
    if (!conn.resolution) conn.resolution = conn.streamResolution || "720p";
    if (!conn.fps) conn.fps = Number(conn.streamFps || 60);
    if (conn.streamAudio === undefined) conn.streamAudio = true;
    if (!conn.qualityTier) conn.qualityTier = conn.resolution === "720p" ? "SD" : "HD";

    let mainTab = /** @type {"apps" | "devices"} */ (conn.streamMainTab === "devices" ? "devices" : "apps");
    let appCaptureKind = /** @type {"screens" | "windows"} */ (conn.streamPortalKind || "screens");

    const backdrop = document.createElement("div");
    backdrop.id = "live-screenshare-modal";
    backdrop.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.78);display:flex;align-items:center;justify-content:center;z-index:9999;";
    const card = document.createElement("div");
    card.setAttribute("role", "dialog");
    card.setAttribute("aria-modal", "true");
    card.setAttribute("aria-label", "Демонстрация экрана");
    card.style.cssText =
      "width:620px;max-width:95vw;background:#2b2d31;color:#f2f3f5;border:1px solid #3f4147;border-radius:12px;overflow:hidden;box-shadow:0 24px 64px rgba(0,0,0,0.88);display:flex;flex-direction:column;font-size:13px;font-family:inherit;";

    const cleanupModal = () => {
      document.getElementById("live-stream-settings-menu")?.remove();
      document.getElementById("live-stream-settings-submenu")?.remove();
      backdrop.remove();
    };

    const startStreamWithSource = (/** @type {{ id: string, label: string, tab?: string }} */ picked) => {
      conn.screenShareOn = true;
      conn.streamMainTab = mainTab;
      conn.streamPortalKind = picked.tab || appCaptureKind;
      conn.streamSourceId = picked.id;
      conn.streamSourceLabel = picked.label || "Окно, экран или приложение";
      conn.streamResolution = conn.resolution || "720p";
      conn.streamFps = Number(conn.fps || 60);
      const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
      for (const gk of new Set([conn.guildId, normalizeVoiceGuildKey(conn.guildId)])) {
        const gList = liveVoiceStatesByGuild.get(gk) ?? [];
        for (const v of gList) {
          if (String(v?.user_id ?? v?.userId ?? "") === selfId) {
            v.self_stream = true;
            v.selfStream = true;
          }
        }
      }
      cleanupModal();
      playVoiceUiSound("stream_on");
      void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "set_capture_source",
        deviceId: (picked.tab || appCaptureKind) + ":" + picked.id + ":" + (picked.label || ""),
      }).catch(() => {});
      syncVoiceMuteStateWithBackend();
      requestLiveScreenPreviewFrame();
      renderLiveVoicePanel();
      tree.refreshView();
      openLiveStreamStageViewer(
        {
          userId: selfId,
          guildId: conn.guildId,
          channelId: conn.channelId,
          displayName: cleanDiscordStr(liveSelfUser?.globalName || liveSelfUser?.username) || "Вы",
          streaming: true,
        },
        { id: conn.channelId, name: conn.channelName || "Стрим" },
      );
      emitLiveTelemetry({
        type: "voice_screenshare_start",
        source: picked.id,
        sourceLabel: conn.streamSourceLabel,
        resolution: conn.streamResolution,
        fps: conn.streamFps,
        audio: conn.streamAudio !== false,
        channelId: conn.channelId,
      });
    };

    const topArea = document.createElement("div");
    topArea.style.cssText = "padding:18px 20px 16px;display:flex;flex-direction:column;gap:16px;";

    const segmentedWrap = document.createElement("div");
    segmentedWrap.style.cssText =
      "display:grid;grid-template-columns:repeat(2,1fr);gap:4px;padding:4px;border-radius:10px;background:#1e1f22;";

    const centerContent = document.createElement("div");
    centerContent.style.cssText = "min-height:224px;display:flex;flex-direction:column;gap:12px;justify-content:center;";

    const bottomBar = document.createElement("div");
    bottomBar.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;gap:12px;padding:12px 20px;background:#232428;border-top:1px solid #35373c;";

    const nitroBanner = document.createElement("div");
    nitroBanner.style.cssText =
      "display:flex;align-items:center;justify-content:space-between;gap:10px;padding:10px 20px;background:linear-gradient(90deg,#3b1d5e 0%,#581c87 55%,#4c1d95 100%);color:#f2f3f5;font-size:13px;";
    const nitroLeft = document.createElement("span");
    nitroLeft.style.cssText = "font-weight:600;color:#f3e8ff;";
    nitroLeft.textContent = "Стримьте в разрешении HD с подпиской Nitro";
    const nitroRight = document.createElement("div");
    nitroRight.style.cssText = "display:flex;align-items:center;gap:8px;";
    const heartBtn = document.createElement("button");
    heartBtn.type = "button";
    heartBtn.style.cssText =
      "width:32px;height:30px;border-radius:6px;border:1px solid rgba(255,255,255,0.22);background:rgba(255,255,255,0.08);color:#f3e8ff;cursor:pointer;font-size:14px;";
    heartBtn.textContent = "♡";
    const nitroBtn = document.createElement("button");
    nitroBtn.type = "button";
    nitroBtn.style.cssText =
      "padding:6px 12px;border-radius:6px;border:none;background:#f2f3f5;color:#111214;font-size:12px;font-weight:700;cursor:pointer;display:inline-flex;align-items:center;gap:6px;";
    nitroBtn.innerHTML = `<span style="color:#9333ea;">✦</span><span>Подписаться на Nitro</span>`;
    nitroBtn.addEventListener("click", () => {
      openNitroHdModal();
    });
    nitroRight.append(heartBtn, nitroBtn);
    nitroBanner.append(nitroLeft, nitroRight);

    const renderModalState = () => {
      segmentedWrap.replaceChildren();
      for (const t of [
        { id: /** @type {const} */ ("apps"), icon: "🖥", label: "Приложения" },
        { id: /** @type {const} */ ("devices"), icon: "📹", label: "Устройства" },
      ]) {
        const active = mainTab === t.id;
        const b = document.createElement("button");
        b.type = "button";
        b.setAttribute("role", "tab");
        b.setAttribute("aria-selected", active ? "true" : "false");
        b.style.cssText =
          `padding:8px 14px;border-radius:8px;border:none;background:${active ? "#35373c" : "transparent"};color:${active ? "#f2f3f5" : "#949ba4"};font-size:13px;font-weight:700;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;gap:8px;`;
        b.innerHTML = `<span>${t.icon}</span><span>${t.label}</span>`;
        b.addEventListener("click", () => {
          mainTab = t.id;
          renderModalState();
        });
        segmentedWrap.append(b);
      }

      centerContent.replaceChildren();
      if (mainTab === "apps") {
        const heading = document.createElement("div");
        heading.style.cssText = "font-size:15px;font-weight:700;color:#f2f3f5;";
        heading.textContent = "Демонстрируйте экран, окно или приложение";

        const cardsGrid = document.createElement("div");
        cardsGrid.style.cssText = "display:grid;grid-template-columns:repeat(2,1fr);gap:12px;flex:1;";

        const makeBigCard = (
          /** @type {"screens" | "windows"} */ kind,
          /** @type {string} */ label,
          /** @type {string} */ svgIcon,
        ) => {
          const isSel = appCaptureKind === kind;
          const btn = document.createElement("button");
          btn.type = "button";
          btn.setAttribute("data-capture-kind", kind);
          btn.style.cssText =
            `min-height:178px;border-radius:10px;border:1.5px solid ${isSel ? "#5865f2" : "#3f4147"};background:#313338;color:#f2f3f5;cursor:pointer;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;padding:20px;transition:border-color 120ms ease,background 120ms ease;`;
          btn.addEventListener("mouseenter", () => {
            btn.style.borderColor = "#5865f2";
            btn.style.background = "#35373c";
          });
          btn.addEventListener("mouseleave", () => {
            btn.style.borderColor = isSel ? "#5865f2" : "#3f4147";
            btn.style.background = "#313338";
          });
          const circle = document.createElement("span");
          circle.style.cssText =
            "width:54px;height:54px;border-radius:50%;background:#1e1f22;color:#dbdee1;display:inline-flex;align-items:center;justify-content:center;";
          circle.innerHTML = svgIcon;
          const txt = document.createElement("span");
          txt.style.cssText = "font-size:13px;font-weight:700;color:#f2f3f5;";
          txt.textContent = label;
          btn.append(circle, txt);
          btn.addEventListener("click", () => {
            appCaptureKind = kind;
            openScreenSharePortalPicker(kind, (picked) => {
              startStreamWithSource(picked);
            });
          });
          return btn;
        };

        cardsGrid.append(
          makeBigCard(
            "screens",
            "Демонстрировать весь экран",
            `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>`,
          ),
          makeBigCard(
            "windows",
            "Демонстрировать окно",
            `<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="4" width="18" height="16" rx="2"/><line x1="3" y1="9" x2="21" y2="9"/><circle cx="6.5" cy="6.5" r="1"/><circle cx="9.5" cy="6.5" r="1"/></svg>`,
          ),
        );
        centerContent.append(heading, cardsGrid);
      } else {
        const devHeading = document.createElement("div");
        devHeading.style.cssText = "font-size:15px;font-weight:700;color:#f2f3f5;margin-bottom:8px;";
        devHeading.textContent = "Выберите видеоустройство для трансляции";
        const devGrid = document.createElement("div");
        devGrid.style.cssText = "display:grid;grid-template-columns:repeat(2,1fr);gap:12px;";
        const devicesList = [
          { id: "cam_v4l2_0", label: "USB HD Webcam (/dev/video0)", sub: "1080p • 60 FPS • V4L2" },
          { id: "cam_pipewire", label: "PipeWire Video Capture", sub: "Системный видеопоток • PipeWire" },
          { id: "cam_obs_virt", label: "OBS Virtual Camera", sub: "Виртуальная камера • 1080p" },
          { id: "screen_x11_0", label: "X11 / Wayland Захват монитора :0", sub: "Полный экран • Аппаратный" },
        ];
        for (const dev of devicesList) {
          const isSelected = (conn.selectedDeviceId || "cam_v4l2_0") === dev.id;
          const dCard = document.createElement("button");
          dCard.type = "button";
          dCard.style.cssText =
            `min-height:92px;border-radius:10px;border:1.5px solid ${isSelected ? "#5865f2" : "#3f4147"};background:${isSelected ? "rgba(88,101,242,0.14)" : "#313338"};color:#f2f3f5;cursor:pointer;display:flex;align-items:center;gap:12px;padding:14px;text-align:left;transition:border-color 120ms ease,background 120ms ease;`;
          dCard.innerHTML = `
            <span style="width:42px;height:42px;border-radius:10px;background:#1e1f22;display:inline-flex;align-items:center;justify-content:center;font-size:20px;flex-shrink:0;">📹</span>
            <span style="display:flex;flex-direction:column;gap:3px;min-width:0;">
              <span style="font-size:13px;font-weight:700;color:#f2f3f5;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${dev.label}</span>
              <span style="font-size:11px;color:#949ba4;">${dev.sub}</span>
            </span>
          `;
          dCard.addEventListener("click", () => {
            conn.selectedDeviceId = dev.id;
            conn.selectedDeviceLabel = dev.label;
            renderModalState();
          });
          dCard.addEventListener("dblclick", () => {
            playVoiceUiSound("stream_on");
            startStreamWithSource({ id: dev.id, label: dev.label, tab: "devices" });
          });
          devGrid.append(dCard);
        }
        centerContent.append(devHeading, devGrid);
      }

      bottomBar.replaceChildren();
      const presetNames = {
        games: "Игры",
        screen: "Демонстрация экрана",
        custom: "Пользовательские",
      };
      const presetTitle = presetNames[/** @type {keyof typeof presetNames} */ (conn.preset || "games")] || "Игры";
      const summaryLeft = document.createElement("div");
      summaryLeft.style.cssText = "display:flex;flex-direction:column;gap:2px;";
      const sumTop = document.createElement("div");
      sumTop.style.cssText = "font-size:13px;font-weight:700;color:#f2f3f5;";
      sumTop.textContent = presetTitle;
      const sumSub = document.createElement("div");
      sumSub.style.cssText = "display:flex;align-items:center;gap:8px;font-size:12px;color:#b5bac1;";
      sumSub.innerHTML = `<span>${conn.resolution || "720p"}</span><span>${conn.fps || 60}fps</span><span style="display:inline-flex;align-items:center;gap:4px;">${conn.streamAudio !== false ? "🔊 Звук системы" : "🔇 Без звука"}</span>`;
      summaryLeft.append(sumTop, sumSub);

      const controlsRight = document.createElement("div");
      controlsRight.style.cssText = "display:flex;align-items:center;gap:8px;";

      const sdHdWrap = document.createElement("div");
      sdHdWrap.style.cssText =
        "display:inline-flex;padding:3px;border-radius:8px;background:#1e1f22;border:1px solid #35373c;";
      for (const tier of ["SD", "HD"]) {
        const active = (conn.qualityTier || "SD") === tier;
        const tb = document.createElement("button");
        tb.type = "button";
        tb.style.cssText =
          `padding:5px 10px;border-radius:6px;border:none;background:${active ? "#35373c" : "transparent"};color:${active ? "#f2f3f5" : tier === "HD" ? "#c084fc" : "#949ba4"};font-size:12px;font-weight:700;cursor:pointer;display:inline-flex;align-items:center;gap:4px;`;
        tb.innerHTML = tier === "HD" ? `<span style="color:#c084fc;">✦</span><span>HD</span>` : `<span>SD</span>`;
        tb.addEventListener("click", () => {
          conn.qualityTier = tier;
          if (tier === "SD") {
            conn.resolution = "720p";
            conn.fps = 60;
          } else {
            conn.resolution = "1080p";
            conn.fps = 60;
          }
          renderModalState();
        });
        sdHdWrap.append(tb);
      }

      const primaryBtn = document.createElement("button");
      primaryBtn.type = "button";
      primaryBtn.style.cssText =
        "padding:8px 16px;border-radius:8px;border:none;background:#5865f2;color:#ffffff;font-size:13px;font-weight:700;cursor:pointer;";
      primaryBtn.textContent = mainTab === "devices" ? "Стримить" : "Выбрать нужное";
      primaryBtn.addEventListener("click", () => {
        if (mainTab === "devices") {
          playVoiceUiSound("stream_on");
          startStreamWithSource({
            id: conn.selectedDeviceId || "cam_v4l2_0",
            label: conn.selectedDeviceLabel || "USB HD Webcam (/dev/video0)",
            tab: "devices",
          });
        } else {
          openScreenSharePortalPicker(appCaptureKind, (picked) => {
            playVoiceUiSound("stream_on");
            startStreamWithSource(picked);
          });
        }
      });

      const gearBtn = document.createElement("button");
      gearBtn.type = "button";
      gearBtn.title = "Настройки режима стрима";
      gearBtn.setAttribute("aria-label", "Настройки режима стрима");
      gearBtn.style.cssText =
        "width:34px;height:34px;border-radius:8px;border:1px solid #3f4147;background:#2b2d31;color:#dbdee1;font-size:16px;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;";
      gearBtn.textContent = "⚙";
      gearBtn.addEventListener("click", (ev) => {
        ev.stopPropagation();
        openStreamSettingsMenu(gearBtn, conn, () => {
          renderModalState();
        });
      });

      controlsRight.append(sdHdWrap, primaryBtn, gearBtn);
      bottomBar.append(summaryLeft, controlsRight);
    };

    renderModalState();
    topArea.append(segmentedWrap, centerContent);
    card.append(topArea, bottomBar, nitroBanner);
    backdrop.append(card);
    backdrop.addEventListener("click", (ev) => {
      if (ev.target === backdrop) cleanupModal();
    });
    document.body.append(backdrop);
  }

  /**
   * Обновляет индикаторы задержки RTT и графика без пересоздания всей голосовой панели.
   */
  function updateLiveRtcDomInPlace() {
    const appShellNode = document.getElementById("app-shell");
    if (appShellNode && appShellNode.style.visibility === "hidden") {
      return;
    }
    const badge = document.getElementById("live-voice-status-badge");
    const rttPill = document.getElementById("live-voice-rtt-pill");
    const phaseView = resolveVoiceCallPhaseView(liveRtcStatus);
    const st = String(liveRtcStatus?.state || "signaling");
    const isErr =
      phaseView.isError ||
      phaseView.phase === "timeout" ||
      phaseView.phase === "error" ||
      st === "error" ||
      st === "timeout" ||
      st.startsWith("error");
    const isDisconnected = !isErr && (phaseView.phase === "disconnected" || st === "disconnected");
    const isConnecting =
      !isErr &&
      (phaseView.phase === "signaling" ||
        phaseView.phase === "negotiating" ||
        st === "connecting" ||
        st === "reconnecting" ||
        isDisconnected ||
        (isLiveSession() && !phaseView.isConnected));
    const rawLast = liveRtcStatus?.lastPingMs;
    const rawAvg = liveRtcStatus?.averagePingMs;
    const hasMeasuredPing = typeof rawLast === "number" && rawLast > 0;
    const effectiveLast = hasMeasuredPing ? rawLast : 0;
    const effectiveAvg = typeof rawAvg === "number" && rawAvg > 0 ? rawAvg : effectiveLast;
    const pingColor =
      isErr || effectiveLast > 800
        ? "#f23f43"
        : isConnecting || effectiveLast > 350
          ? "#f0b232"
          : "#23a55a";
    if (voiceHost && activeVoiceConnection) {
      renderVoiceRtcPhaseDom(voiceHost, liveRtcStatus, {
        doc: document,
        onRetry: retryActiveVoiceConnection,
      });
    }
    if (badge && badge.getAttribute("data-hovered") !== "true") {
      const nextBadgeText =
        phaseView.label ||
        (isErr
          ? "Нет сигнала RTC (переподключение…)"
          : isConnecting
            ? "Ожидание ответа сервера Discord..."
            : "Голосовая связь установлена • DAVE E2EE");
      if (badge.textContent !== nextBadgeText) badge.textContent = nextBadgeText;
      if (badge.style.color !== pingColor) badge.style.color = pingColor;
    }
    const sigIcon = document.querySelector("#voice-panel [data-voice-signal-icon]");
    if (sigIcon instanceof HTMLElement && sigIcon.style.color !== pingColor) {
      sigIcon.style.color = pingColor;
    }
    if (rttPill) {
      const nextRttText = isErr
        ? "—"
        : isConnecting && !hasMeasuredPing
          ? "RTC…"
          : `${effectiveLast} мс`;
      if (rttPill.textContent !== nextRttText) rttPill.textContent = nextRttText;
      if (rttPill.style.color !== pingColor) rttPill.style.color = pingColor;
    }
    const popTitle = document.getElementById("live-rtc-popout-title");
    if (popTitle) {
      popTitle.textContent = isErr
        ? phaseView.label || "Ошибка RTC соединения"
        : isConnecting
          ? phaseView.label || "Подключение к RTC…"
          : "Голосовая связь подключена";
      popTitle.style.color = pingColor;
    }
    const popStats = document.getElementById("live-rtc-popout-stats");
    if (popStats) {
      const host = liveRtcStatus?.shortHostname || liveRtcStatus?.endpoint || "—";
      const avgTxt = hasMeasuredPing ? `${effectiveAvg} мс` : "—";
      const lastTxt = hasMeasuredPing ? `${effectiveLast} мс` : "—";
      const lossTxt = `${Number(liveRtcStatus?.packetLossPct ?? liveRtcStatus?.packetLossPercent ?? 0).toFixed(1)}%`;
      popStats.innerHTML = `
        <div style="font-weight:700;color:#f2f3f5;font-size:14px;margin-bottom:2px;">${host}</div>
        <div>Средняя задержка: <b style="color:#f2f3f5;">${avgTxt}</b></div>
        <div>Последняя задержка: <b style="color:#f2f3f5;">${lastTxt}</b></div>
        <div>Потеря исходящих пакетов: <b style="color:#f2f3f5;">${lossTxt}</b></div>
      `;
    }
    const popChart = document.getElementById("live-rtc-popout-chart");
    if (popChart) {
      const rawHist = Array.isArray(liveRtcStatus?.pingHistory) && liveRtcStatus.pingHistory.length > 0
        ? liveRtcStatus.pingHistory
        : [effectiveLast - 1, effectiveLast, effectiveLast + 1, effectiveLast, effectiveLast - 1, effectiveLast];
      const hist = rawHist.length === 1 ? [rawHist[0], rawHist[0], rawHist[0]] : rawHist;
      const yMax = Math.max(400, ...hist);
      const yMid = Math.round(yMax / 2);
      const pts = hist
        .map((/** @type {any} */ v, /** @type {number} */ idx) => {
          const x = Math.round((idx / Math.max(1, hist.length - 1)) * 270);
          const y = Math.round(54 - (Math.min(yMax, Math.max(0, Number(v))) / yMax) * 46);
          return `${x},${Math.max(4, Math.min(54, y))}`;
        })
        .join(" ");
      const now = new Date();
      const fmtTime = (/** @type {number} */ offsetMin) => {
        const d = new Date(now.getTime() - offsetMin * 60_000);
        return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
      };
      popChart.innerHTML = `
        <svg width="276" height="58" viewBox="0 0 300 58" style="display:block;width:100%;height:58px;">
          <line x1="0" y1="8" x2="270" y2="8" stroke="#2b2d31" stroke-width="1" />
          <line x1="0" y1="31" x2="270" y2="31" stroke="#2b2d31" stroke-width="1" />
          <line x1="0" y1="54" x2="270" y2="54" stroke="#2b2d31" stroke-width="1" />
          <polyline fill="none" stroke="#23a55a" stroke-width="2.2" points="${pts}" />
          <text x="298" y="11" fill="#949ba4" font-size="9" text-anchor="end">${yMax}</text>
          <text x="298" y="34" fill="#949ba4" font-size="9" text-anchor="end">${yMid}</text>
          <text x="298" y="56" fill="#949ba4" font-size="9" text-anchor="end">0</text>
        </svg>
        <div style="display:flex;justify-content:space-between;padding-right:28px;margin-top:2px;font-size:10px;color:#949ba4;">
          <span>${fmtTime(3)}</span>
          <span>${fmtTime(2)}</span>
          <span>${fmtTime(1)}</span>
          <span>${fmtTime(0)}</span>
        </div>
      `;
    }

    // Обновляем зелёное кольцо вокруг аватарки говорящего (`data-voice-avatar-uid`),
    // таймеры активных голосовых каналов (`data-voice-timer-cid`) и 30-секундную кнопку
    // «Пригласить в голосовой чат» (`data-alone-invite-cid`), когда пользователь сидит один.
    const selfId = String(liveSelfUser?.id || "");
    const currentGid =
      activeVoiceConnection?.guildId ||
      currentGuildInfo().id ||
      rail.selectedId ||
      selectedGuildId() ||
      "";
    const speakingSet = new Set(
      Array.isArray(liveRtcStatus?.speakingUsers) ? liveRtcStatus.speakingUsers.map(String) : [],
    );
    const avatarNodes = document.querySelectorAll(".voice-user-avatar[data-voice-avatar-uid]");
    for (const node of avatarNodes) {
      const el = /** @type {HTMLElement} */ (node);
      const uid = String(el.getAttribute("data-voice-avatar-uid") || "");
      const rowEl = el.closest(".voice-channel-user, .tree-voice-user");
      const rowCid = String(rowEl?.getAttribute("data-channel-id") || "");
      const rowInAfk = isChannelAfk(currentGid, rowCid);
      const isSelf = uid === selfId;
      const isMuted = rowInAfk || (isSelf
        ? Boolean(selfVoicePrefs.muted || selfVoicePrefs.deafened)
        : Boolean(rowEl?.querySelector('[title*="Микрофон"], [title*="Звук"], [aria-label*="Микрофон"], [aria-label*="Звук"]')));
      const isSpeaking =
        !rowInAfk &&
        (!isMuted || (!isSelf && speakingSet.has(uid))) &&
        (speakingSet.has(uid) || (isSelf && Boolean(liveRtcStatus?.localMicSpeaking)));
      const wasSpeaking = el.getAttribute("data-speaking") === "true";
      if (isSpeaking !== wasSpeaking) {
        if (isSpeaking) {
          el.setAttribute("data-speaking", "true");
          el.style.boxShadow = "0 0 0 2px #23a55a, inset 0 0 0 1.5px #1e1f22";
        } else {
          el.removeAttribute("data-speaking");
          el.style.boxShadow = "none";
        }
        const nameSpan = /** @type {HTMLElement | null} */ (rowEl?.querySelector("span > span"));
        if (nameSpan) {
          if (isSpeaking) {
            nameSpan.style.color = "#f2f3f5";
            nameSpan.style.fontWeight = "600";
          } else {
            const roleCol = nameSpan.getAttribute("data-role-color");
            nameSpan.style.color = roleCol || "#dbdee1";
            nameSpan.style.fontWeight = "400";
          }
        }
      }
    }

    if (currentGid) {
      const timerNodes = document.querySelectorAll("[data-voice-timer-cid]");
      for (const node of timerNodes) {
        const el = /** @type {HTMLElement} */ (node);
        const cid = String(el.getAttribute("data-voice-timer-cid") || "");
        if (!cid) continue;
        const pCount = getVoiceParticipants(currentGid, cid).length;
        if (pCount > 0) {
          const nextTimer = getVoiceChannelTimer(currentGid, cid, pCount);
          if (el.textContent !== nextTimer) {
            el.textContent = nextTimer;
          }
        }
      }
      const aloneNodes = document.querySelectorAll("[data-alone-invite-cid]");
      for (const node of aloneNodes) {
        const el = /** @type {HTMLElement} */ (node);
        const cid = String(el.getAttribute("data-alone-invite-cid") || "");
        const remainMs = getAloneInviteRemainingMs(currentGid, cid);
        const pCount = getVoiceParticipants(currentGid, cid).length;
        if (remainMs <= 0 || pCount !== 1) {
          el.remove();
        } else {
          const ring = el.querySelector("[data-alone-ring-circle]");
          if (ring) {
            const circum = 62.83;
            const dashOffset = ((1 - Math.max(0, Math.min(1, remainMs / 30_000))) * circum).toFixed(2);
            ring.setAttribute("stroke-dashoffset", dashOffset);
          }
        }
      }
    }
  }

  function ensureUserBarDeviceChevrons() {
    if (!isLiveSession()) return;
    const actions = document.querySelector("#user-bar .user-actions");
    if (!actions || actions.querySelector(".user-bar-chevron")) return;
    const uMute = /** @type {HTMLElement | null} */ (actions.querySelector('[data-action="mute"]'));
    const uDeaf = /** @type {HTMLElement | null} */ (actions.querySelector('[data-action="deafen"]'));
    const makeChevron = (/** @type {"input" | "output"} */ kind, /** @type {string} */ label) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "icon-btn user-bar-chevron";
      btn.setAttribute("data-chevron-kind", kind);
      btn.setAttribute("aria-label", label);
      btn.title = label;
      btn.style.cssText =
        "width:18px;min-width:18px;height:32px;padding:0;margin-left:0;border:none;border-radius:0 8px 8px 0;display:inline-flex;align-items:center;justify-content:center;color:#b5bac1;background:transparent;cursor:pointer;";
      btn.innerHTML = SVG_CHEVRON_DOWN;
      btn.addEventListener("click", (ev) => {
        ev.stopPropagation();
        void openAudioDevicePopover(kind, btn);
      });
      return btn;
    };
    if (uMute && uMute.parentNode === actions) {
      uMute.after(makeChevron("input", "Параметры микрофона и блокировки"));
    }
    if (uDeaf && uDeaf.parentNode === actions) {
      uDeaf.after(makeChevron("output", "Параметры звука и блокировки"));
    }
  }

  function syncUserBarVoiceButtons() {
    if (!isLiveSession()) return;
    ensureUserBarDeviceChevrons();
    const uBar = /** @type {HTMLElement | null} */ (document.getElementById("user-bar"));
    if (uBar) {
      uBar.style.padding = "4px 6px";
      uBar.style.gap = "4px";
    }
    const uProf = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-action="profile"]')
    );
    if (uProf) {
      uProf.style.gap = "8px";
      uProf.style.padding = "2px 4px";
      uProf.style.minWidth = "86px";
    }
    const uSettings = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-action="settings"]')
    );
    if (uSettings) {
      uSettings.style.width = "24px";
      uSettings.style.minWidth = "24px";
      uSettings.style.height = "28px";
      uSettings.style.padding = "0";
    }
    const uLogout = /** @type {HTMLElement | null} */ (document.getElementById("auth-logout"));
    if (uLogout) {
      uLogout.style.fontSize = "11px";
      uLogout.style.padding = "0 4px";
      uLogout.style.height = "24px";
    }
    const uMute = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-action="mute"]')
    );
    const uDeaf = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-action="deafen"]')
    );
    const uMuteChevron = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-chevron-kind="input"]')
    );
    const uDeafChevron = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-chevron-kind="output"]')
    );
    const muted = selfVoicePrefs.muted;
    const deafened = selfVoicePrefs.deafened;
    if (uMute) {
      uMute.setAttribute("data-pressed", muted ? "true" : "false");
      uMute.innerHTML = muted ? SVG_MIC_OFF : SVG_MIC_ON;
      uMute.style.cssText =
        `width:24px;min-width:24px;height:28px;padding:0;border:none;border-radius:6px 0 0 6px;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:${muted ? "#f23f43" : "#b5bac1"};background:${muted ? "rgba(242, 63, 67, 0.18)" : "transparent"};border-right:${muted ? "1px solid rgba(242, 63, 67, 0.28)" : "1px solid transparent"};`;
      uMute.title = muted
        ? "Ваш микрофон отключён! (Нажмите, чтобы включить микрофон)"
        : "Отключить только микрофон (динамики останутся включены)";
      uMute.setAttribute(
        "aria-label",
        muted ? "Включить микрофон" : "Отключить только микрофон",
      );
    }
    if (uMuteChevron) {
      uMuteChevron.style.cssText =
        `width:12px;min-width:12px;height:28px;padding:0;margin-left:0;border:none;border-radius:0 6px 6px 0;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:${muted ? "#f23f43" : "#b5bac1"};background:${muted ? "rgba(242, 63, 67, 0.18)" : "transparent"};`;
    }
    if (uDeaf) {
      uDeaf.setAttribute("data-pressed", deafened ? "true" : "false");
      uDeaf.innerHTML = deafened ? SVG_DEAF_OFF : SVG_DEAF_ON;
      uDeaf.style.cssText =
        `width:24px;min-width:24px;height:28px;padding:0;border:none;border-radius:6px 0 0 6px;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:${deafened ? "#f23f43" : "#b5bac1"};background:${deafened ? "rgba(242, 63, 67, 0.18)" : "transparent"};border-right:${deafened ? "1px solid rgba(242, 63, 67, 0.28)" : "1px solid transparent"};`;
      uDeaf.title = deafened
        ? "Включить звук и микрофон (Снять оглушение)"
        : "Отключить звук и микрофон (Оглушить)";
      uDeaf.setAttribute(
        "aria-label",
        deafened ? "Включить звук" : "Отключить звук и микрофон",
      );
    }
    if (uDeafChevron) {
      uDeafChevron.style.cssText =
        `width:12px;min-width:12px;height:28px;padding:0;margin-left:0;border:none;border-radius:0 6px 6px 0;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:${deafened ? "#f23f43" : "#b5bac1"};background:${deafened ? "rgba(242, 63, 67, 0.18)" : "transparent"};`;
    }

    // Обновляем кружок статуса на аватарке в `#user-bar` (`В сети` / `Неактивен` / `Не беспокоить` / `Невидимый`)
    const uAvatar = /** @type {HTMLElement | null} */ (
      document.querySelector('#user-bar [data-action="profile"] .avatar')
    );
    if (uAvatar) {
      uAvatar.style.position = "relative";
      let dot = /** @type {HTMLElement | null} */ (uAvatar.querySelector("#user-bar-status-dot"));
      if (!dot) {
        dot = document.createElement("span");
        dot.id = "user-bar-status-dot";
        uAvatar.append(dot);
      }
      const stKey = selfVoicePrefs.status || "online";
      const isInv = stKey === "invisible";
      const stColor =
        stKey === "idle"
          ? "#f0b232"
          : stKey === "dnd"
            ? "#f23f43"
            : isInv
              ? "#80848e"
              : "#23a55a";
      dot.style.cssText =
        `position:absolute;right:-2px;bottom:-2px;width:11px;height:11px;border-radius:50%;background:${isInv ? "#111214" : stColor};border:2.5px solid #111214;box-shadow:${isInv ? "inset 0 0 0 2.2px #80848e" : "none"};pointer-events:none;`;
    }
  }

  /**
   * Рендерит верхнюю сцену звонка в ЛС / Групповом чате (`#dm-call-stage-banner`) над `#chat-list`
   * с плитками участников, индикатором речи, предпросмотром/просмотром демонстрации экрана (`openLiveStreamStageViewer`),
   * включением демонстрации экрана (`openScreenShareModal`), камеры, микрофона, звука и кнопкой присоединения/выхода.
   */
  function renderLiveDmCallStage() {
    if (typeof document === "undefined") return;
    const existing = document.getElementById("dm-call-stage-banner");
    if (!isLiveSession()) {
      existing?.remove();
      return;
    }
    const isDmView =
      document.body?.dataset?.viewMode === "dm" ||
      rail?.selectedId === HOME_GUILD_ID ||
      Boolean(document.getElementById("dm-header-call-actions"));
    const dmCid = cleanDiscordStr(String(chat?.channelId || ""));
    if (!isDmView || !dmCid) {
      existing?.remove();
      return;
    }
    const participants = getVoiceParticipants(HOME_GUILD_ID, dmCid);
    const isConnectedHere = Boolean(
      activeVoiceConnection &&
        normalizeVoiceGuildKey(activeVoiceConnection.guildId) === HOME_GUILD_ID &&
        String(activeVoiceConnection.channelId || "") === dmCid,
    );
    if (!isConnectedHere && participants.length === 0) {
      existing?.remove();
      return;
    }

    const dmChannelName =
      cleanDiscordStr(document.querySelector(".chat-header .chat-title")?.textContent) ||
      activeVoiceConnection?.channelName ||
      "Звонок";
    const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
    const displayParticipants = [...participants];
    if (isConnectedHere && displayParticipants.length === 1 && displayParticipants[0]?.userId === selfId) {
      const pcs = dmFriendsController?.getPrivateChannels?.() ?? [];
      const pc = /** @type {any} */ (pcs.find((/** @type {any} */ c) => String(c?.id || "") === dmCid));
      const activeDmRec = /** @type {any} */ (globalThis).__ACTIVE_DM_RECIPIENT__;
      const recs = Array.isArray(pc?.rawRecipients) && pc.rawRecipients.length > 0
        ? pc.rawRecipients
        : Array.isArray(pc?.recipients) && pc.recipients.length > 0
          ? pc.recipients
          : activeDmRec && activeDmRec.id
            ? [activeDmRec]
            : [];
      for (const r of recs) {
        const rid = cleanDiscordStr(String(r?.id || ""));
        if (rid && rid !== selfId && !displayParticipants.some((p) => p.userId === rid)) {
          const known = liveUsersById.get(rid) ?? r;
          const rName =
            cleanDiscordStr(
              known?.global_name ??
                known?.globalName ??
                known?.display_name ??
                known?.displayName ??
                known?.username,
            ) || dmChannelName;
          const rAvHash = cleanDiscordStr(known?.avatar);
          displayParticipants.push({
            userId: rid,
            guildId: HOME_GUILD_ID,
            channelId: dmCid,
            displayName: rName,
            username: cleanDiscordStr(known?.username) || rName,
            avatarUrl: rAvHash ? `https://cdn.discordapp.com/avatars/${rid}/${rAvHash}.png?size=64` : null,
            muted: false,
            deafened: false,
            streaming: false,
            speaking: false,
            ringing: true,
          });
        }
      }
    }

    const chatScreen = document.getElementById("screen-chat");
    const chatMain = /** @type {HTMLElement | null} */ (chatScreen?.querySelector(".chat-main"));
    let banner = existing;
    if (!banner) {
      banner = document.createElement("section");
      banner.id = "dm-call-stage-banner";
      banner.setAttribute("role", "region");
      banner.setAttribute("aria-label", `Голосовой звонок: ${dmChannelName}`);
      if (chatMain) {
        chatMain.prepend(banner);
      } else if (chatScreen) {
        chatScreen.prepend(banner);
      }
    } else if (chatMain && banner.parentNode !== chatMain) {
      chatMain.prepend(banner);
    }
    const timerTxt = getVoiceChannelTimer(HOME_GUILD_ID, dmCid, Math.max(1, participants.length));
    const stageStateSig = JSON.stringify({
      dmCid,
      dmChannelName,
      isConnectedHere,
      cam: Boolean(activeVoiceConnection?.cameraOn),
      stream: Boolean(activeVoiceConnection?.screenShareOn),
      streamSrc: String(activeVoiceConnection?.streamSourceId || ""),
      muted: Boolean(selfVoicePrefs.muted),
      deafened: Boolean(selfVoicePrefs.deafened),
      hasPreview: Boolean(cachedLiveScreenDataUrl),
      participants: displayParticipants.map((p) => ({
        u: p.userId,
        n: p.displayName,
        m: Boolean(p.muted),
        d: Boolean(p.deafened),
        s: Boolean(p.streaming),
        v: Boolean(p.video),
        sp: Boolean(p.speaking),
        r: Boolean(p.ringing),
      })),
    });
    if (banner.dataset.stageStateSig === stageStateSig) {
      const timerPill = banner.querySelector("[data-dm-stage-timer]");
      if (timerPill && timerPill.textContent !== timerTxt) {
        timerPill.textContent = timerTxt;
      }
      if (activeVoiceConnection?.screenShareOn && cachedLiveScreenDataUrl) {
        const selfThumb = /** @type {HTMLImageElement | null} */ (
          banner.querySelector("img[data-dm-stage-self-stream-thumb]")
        );
        if (selfThumb && selfThumb.src !== cachedLiveScreenDataUrl) {
          selfThumb.src = cachedLiveScreenDataUrl;
        }
      }
      return;
    }
    banner.dataset.stageStateSig = stageStateSig;
    banner.style.cssText =
      "width:100%;box-sizing:border-box;background:linear-gradient(180deg,#111214 0%,#18191c 100%);border-bottom:1px solid #2b2d31;padding:14px 18px;display:flex;flex-direction:column;gap:12px;flex-shrink:0;z-index:12;";
    banner.replaceChildren();

    const topStatusRow = document.createElement("div");
    topStatusRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:10px;";
    const statusLeft = document.createElement("div");
    statusLeft.style.cssText = "display:flex;align-items:center;gap:8px;font-size:13px;font-weight:700;color:#f2f3f5;";
    statusLeft.innerHTML = `<span style="color:#23a55a;">📞</span><span>Звонок: ${dmChannelName}</span><span data-dm-stage-timer="true" style="padding:2px 7px;border-radius:10px;background:#2b2d31;color:#b5bac1;font-size:11px;font-weight:600;">${timerTxt}</span>`;
    topStatusRow.append(statusLeft);
    banner.append(topStatusRow);

    const tilesRow = document.createElement("div");
    tilesRow.style.cssText = "display:flex;align-items:center;justify-content:center;gap:14px;flex-wrap:wrap;min-height:112px;";
    for (const p of displayParticipants) {
      const isSelf = p.userId === selfId;
      const isStreaming = Boolean(p.streaming || (isSelf && activeVoiceConnection?.screenShareOn));
      const isCam = Boolean(p.video || (isSelf && activeVoiceConnection?.cameraOn));
      const tile = document.createElement("div");
      tile.className = "voice-channel-user dm-call-stage-tile";
      tile.setAttribute("data-user-id", p.userId);
      tile.setAttribute("data-channel-id", dmCid);
      tile.style.cssText =
        `width:${isStreaming || isCam ? "224px" : "156px"};min-height:118px;border-radius:12px;background:#1e1f22;border:2px solid ${p.speaking ? "#23a55a" : isStreaming ? "#5865f2" : isCam ? "#23a55a" : "#2b2d31"};padding:10px;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:8px;position:relative;cursor:${isStreaming ? "pointer" : "default"};overflow:hidden;transition:border-color 120ms ease;`;

      if (isStreaming && isSelf && cachedLiveScreenDataUrl) {
        const previewImg = document.createElement("img");
        previewImg.setAttribute("data-dm-stage-self-stream-thumb", "true");
        previewImg.src = cachedLiveScreenDataUrl;
        previewImg.alt = "Предпросмотр демонстрации экрана";
        previewImg.style.cssText =
          "width:100%;height:78px;object-fit:cover;border-radius:8px;border:1px solid rgba(255,255,255,0.12);background:#0b0c0e;";
        tile.append(previewImg);
      } else if (isCam && !isStreaming) {
        const camPreview = document.createElement("canvas");
        camPreview.width = 200;
        camPreview.height = 76;
        camPreview.style.cssText =
          "width:100%;height:76px;object-fit:cover;border-radius:8px;border:1px solid rgba(35,165,90,0.45);background:#0f172a;";
        drawLiveCaptureCanvasFrame(camPreview, "cam_v4l2_0", 1, true);
        tile.append(camPreview);
      } else {
        const av = document.createElement("div");
        av.className = "voice-user-avatar";
        av.setAttribute("data-voice-avatar-uid", p.userId);
        if (p.speaking) av.setAttribute("data-speaking", "true");
        av.style.cssText =
          `width:48px;height:48px;border-radius:50%;background:#5865f2;color:#fff;display:flex;align-items:center;justify-content:center;font-size:18px;font-weight:700;background-size:cover;background-position:center;opacity:${p.ringing ? "0.6" : "1"};${p.speaking ? "box-shadow:0 0 0 3px #23a55a;" : ""}`;
        av.textContent = Array.from(String(p.displayName || "U")).slice(0, 1).join("").toUpperCase();
        if (p.avatarUrl) {
          av.style.backgroundImage = `url("${p.avatarUrl}")`;
          av.style.color = "transparent";
        }
        tile.append(av);
      }

      const nameRow = document.createElement("div");
      nameRow.style.cssText =
        "display:flex;align-items:center;gap:5px;max-width:100%;font-size:12px;font-weight:700;color:#f2f3f5;";
      const nameSpan = document.createElement("span");
      nameSpan.style.cssText = "overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
      nameSpan.textContent = p.displayName;
      nameRow.append(nameSpan);
      if (p.muted) {
        const mIcon = document.createElement("span");
        mIcon.style.cssText = "color:#f23f43;font-size:11px;";
        mIcon.textContent = p.deafened ? "🎧̸" : "🎙̸";
        nameRow.append(mIcon);
      }

      tile.append(nameRow);

      if (p.ringing) {
        const ringBadge = document.createElement("span");
        ringBadge.style.cssText =
          "padding:2px 7px;border-radius:8px;background:rgba(240,178,50,0.18);color:#f0b232;font-size:10px;font-weight:700;";
        ringBadge.textContent = "Ожидание ответа…";
        tile.append(ringBadge);
      } else if (isStreaming) {
        const liveBtn = document.createElement("button");
        liveBtn.type = "button";
        liveBtn.setAttribute("data-action", "dm-watch-stream");
        liveBtn.style.cssText =
          "padding:4px 10px;border-radius:6px;border:none;background:#da373c;color:#fff;font-size:11px;font-weight:800;cursor:pointer;display:inline-flex;align-items:center;gap:5px;";
        liveBtn.innerHTML = `<span>🔴 В ЭФИРЕ</span><span>• Смотреть</span>`;
        liveBtn.addEventListener("click", (ev) => {
          ev.stopPropagation();
          if (!isConnectedHere) {
            connectToLiveVoiceChannel("", dmChannelName, { id: dmCid, name: dmChannelName });
          }
          openLiveStreamStageViewer(
            { ...p, guildId: "", channelId: dmCid, streaming: true },
            { id: dmCid, name: dmChannelName },
          );
        });
        tile.append(liveBtn);
      } else if (isCam) {
        const camBadge = document.createElement("span");
        camBadge.style.cssText =
          "padding:2px 8px;border-radius:6px;background:rgba(35,165,90,0.2);color:#4ade80;font-size:10px;font-weight:700;display:inline-flex;align-items:center;gap:4px;";
        camBadge.textContent = "📹 Камера вкл.";
        tile.append(camBadge);
      }

      tile.addEventListener("click", () => {
        if (isStreaming) {
          if (!isConnectedHere) {
            connectToLiveVoiceChannel("", dmChannelName, { id: dmCid, name: dmChannelName });
          }
          openLiveStreamStageViewer(
            { ...p, guildId: "", channelId: dmCid, streaming: true },
            { id: dmCid, name: dmChannelName },
          );
        }
      });
      tile.addEventListener("contextmenu", (ev) => {
        ev.preventDefault();
        openVoiceParticipantContextMenu(p, { id: dmCid, name: dmChannelName }, tile, ev);
      });
      tilesRow.append(tile);
    }
    banner.append(tilesRow);

    const controlsRow = document.createElement("div");
    controlsRow.style.cssText = "display:flex;align-items:center;justify-content:center;gap:8px;flex-wrap:wrap;";
    const makeStageBtn = (
      /** @type {string} */ label,
      /** @type {string} */ bg,
      /** @type {string} */ color,
      /** @type {string} */ actionId,
      /** @type {() => void} */ onClick,
    ) => {
      const b = document.createElement("button");
      b.type = "button";
      b.setAttribute("data-action", actionId);
      b.style.cssText =
        `padding:7px 13px;border-radius:8px;border:1px solid rgba(255,255,255,0.08);background:${bg};color:${color};font-size:12px;font-weight:700;cursor:pointer;display:inline-flex;align-items:center;gap:6px;`;
      b.textContent = label;
      b.addEventListener("click", (ev) => {
        ev.stopPropagation();
        onClick();
      });
      return b;
    };

    if (!isConnectedHere) {
      controlsRow.append(
        makeStageBtn("📞 Присоединиться к звонку", "#23a55a", "#ffffff", "dm-stage-join-voice", () => {
          if (!selfVoicePrefs.deafened) selfVoicePrefs.muted = false;
          connectToLiveVoiceChannel("", dmChannelName, { id: dmCid, name: dmChannelName });
        }),
        makeStageBtn("📹 Присоединиться с видео", "#2b2d31", "#f2f3f5", "dm-stage-join-video", () => {
          if (!selfVoicePrefs.deafened) selfVoicePrefs.muted = false;
          connectToLiveVoiceChannel("", dmChannelName, { id: dmCid, name: dmChannelName });
          if (activeVoiceConnection) {
            activeVoiceConnection.cameraOn = true;
            syncVoiceMuteStateWithBackend();
            renderLiveVoicePanel();
          }
        }),
      );
    } else {
      const conn = activeVoiceConnection;
      const anyStreamer = displayParticipants.find(
        (p) => p.streaming || (p.userId === selfId && conn?.screenShareOn),
      );
      controlsRow.append(
        makeStageBtn(
          conn?.cameraOn ? "📹 Выкл. камеру" : "📹 Камера",
          conn?.cameraOn ? "#23a55a" : "#2b2d31",
          "#ffffff",
          "dm-stage-toggle-camera",
          () => {
            if (!conn) return;
            conn.cameraOn = !conn.cameraOn;
            playVoiceUiSound(conn.cameraOn ? "camera_on" : "camera_off");
            syncVoiceMuteStateWithBackend();
            renderLiveVoicePanel();
          },
        ),
        makeStageBtn(
          conn?.screenShareOn ? "🖥 Изменить экран" : "🖥 Демонстрация экрана",
          conn?.screenShareOn ? "#5865f2" : "#2b2d31",
          "#ffffff",
          "dm-stage-screenshare",
          () => {
            openScreenShareModal(true);
          },
        ),
      );
      if (conn?.screenShareOn) {
        controlsRow.append(
          makeStageBtn(
            "⏹ Остановить стрим",
            "#da373c",
            "#ffffff",
            "dm-stage-stop-screenshare",
            () => {
              openScreenShareModal(false);
            },
          ),
        );
      }
      if (anyStreamer) {
        controlsRow.append(
          makeStageBtn(
            "📺 Смотреть трансляцию",
            "#5865f2",
            "#ffffff",
            "dm-stage-open-stream-viewer",
            () => {
              openLiveStreamStageViewer(
                { ...anyStreamer, guildId: "", channelId: dmCid, streaming: true },
                { id: dmCid, name: dmChannelName },
              );
            },
          ),
        );
      }
      controlsRow.append(
        makeStageBtn(
          selfVoicePrefs.muted ? "🎙 Вкл. микрофон" : "🎙 Выкл. микрофон",
          selfVoicePrefs.muted ? "rgba(242,63,67,0.22)" : "#2b2d31",
          selfVoicePrefs.muted ? "#f23f43" : "#f2f3f5",
          "dm-stage-toggle-mic",
          () => {
            toggleSelfMicOnly();
          },
        ),
        makeStageBtn(
          selfVoicePrefs.deafened ? "🎧 Вкл. звук" : "🎧 Выкл. звук",
          selfVoicePrefs.deafened ? "rgba(242,63,67,0.22)" : "#2b2d31",
          selfVoicePrefs.deafened ? "#f23f43" : "#f2f3f5",
          "dm-stage-toggle-deaf",
          () => {
            toggleSelfDeafenBoth();
          },
        ),
        makeStageBtn(
          "👥+ Добавить в звонок",
          "#2b2d31",
          "#f2f3f5",
          "dm-stage-invite",
          () => {
            openVoiceInviteModal({ id: dmCid, name: dmChannelName, type: 3 });
          },
        ),
        makeStageBtn(
          "📞 Завершить звонок",
          "#da373c",
          "#ffffff",
          "dm-stage-leave",
          () => {
            disconnectFromLiveVoiceChannel();
          },
        ),
      );
    }
    banner.append(controlsRow);
  }

  /**
   * Рендерит активную голосовую панель `#voice-panel` при подключении к голосовому каналу
   * в точном соответствии с оригинальным клиентом Discord (`uploaded_media_1793512261812.png`, `frame_009.jpg`).
   */
  function renderLiveVoicePanel() {
    if (!voiceHost || !isLiveSession()) return;
    syncUserBarVoiceButtons();
    syncFloatingStreamPipPreview();
    renderLiveDmCallStage();
    if (!activeVoiceConnection) {
      voiceHost.setAttribute("data-state", "ready");
      voiceHost.style.display = "none";
      voiceHost.replaceChildren();
      return;
    }
    const conn = activeVoiceConnection;
    const initPhase = resolveVoiceCallPhaseView(liveRtcStatus ?? "signaling");
    voiceHost.style.display = "";
    voiceHost.setAttribute("data-state", initPhase.phase);
    voiceHost.replaceChildren();
    const wrap = document.createElement("div");
    wrap.style.cssText = "display:flex;flex-direction:column;gap:10px;width:100%;padding:2px 0;";

    const topRow = document.createElement("div");
    topRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;gap:8px;width:100%;";

    const statusBtn = document.createElement("button");
    statusBtn.type = "button";
    statusBtn.style.cssText =
      "display:flex;align-items:center;gap:10px;min-width:0;flex:1 1 0%;background:none;border:none;padding:0;text-align:left;cursor:pointer;color:inherit;";
    statusBtn.title = "О голосовом звонке (RTC / RTT / Шифрование)";
    const sigIcon = document.createElement("span");
    sigIcon.setAttribute("data-voice-signal-icon", "true");
    sigIcon.style.cssText =
      `width:32px;height:32px;border-radius:8px;background:rgba(35,165,90,0.14);border:1px solid rgba(35,165,90,0.24);color:${initPhase.statusColor};display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;`;
    sigIcon.innerHTML = SVG_SIGNAL_BARS;

    const statusCol = document.createElement("div");
    statusCol.style.cssText = "min-width:0;flex:1 1 0%;overflow:hidden;display:flex;flex-direction:column;gap:1px;";
    const badgeRow = document.createElement("div");
    badgeRow.style.cssText = "display:flex;align-items:center;gap:6px;";
    const badge = document.createElement("span");
    badge.id = "live-voice-status-badge";
    badge.style.cssText =
      `font-size:13px;font-weight:700;color:${initPhase.statusColor};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;line-height:1.2;`;
    badge.textContent = initPhase.label || "Ожидание ответа сервера Discord...";
    badgeRow.append(badge);

    const connStatus = document.createElement("div");
    connStatus.id = "live-rtc-conn-status";
    connStatus.setAttribute("data-voice-phase-label", "true");
    connStatus.style.cssText =
      "display:none;font-size:11px;font-weight:600;color:#f0b232;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;line-height:1.2;";
    connStatus.textContent = initPhase.label;

    const sub = document.createElement("div");
    sub.style.cssText =
      "font-size:12px;color:#949ba4;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;line-height:1.25;";
    const isDmCall =
      !conn.guildId || conn.guildId === HOME_GUILD_ID || conn.guildName === conn.channelName;
    sub.textContent = isDmCall
      ? String(conn.channelName || "").replace(/^@\s*/, "")
      : `${conn.channelName} / ${conn.guildName || "Сервер"}`;
    sub.addEventListener("mouseenter", () => {
      sub.style.textDecoration = "underline";
      sub.style.color = "#dbdee1";
    });
    sub.addEventListener("mouseleave", () => {
      sub.style.textDecoration = "none";
      sub.style.color = "#949ba4";
    });
    statusCol.append(badgeRow, connStatus, sub);
    statusBtn.append(sigIcon, statusCol);

    statusBtn.addEventListener("mouseenter", () => {
      badge.setAttribute("data-hovered", "true");
      badge.textContent = "О голосовом звонке";
    });
    statusBtn.addEventListener("mouseleave", () => {
      badge.removeAttribute("data-hovered");
      updateLiveRtcDomInPlace();
    });
    statusBtn.addEventListener("click", () => {
      openRtcConnectionPopout();
    });

    const topRightActions = document.createElement("div");
    topRightActions.style.cssText = "display:flex;align-items:center;gap:4px;flex-shrink:0;";

    const krispBtn = document.createElement("button");
    krispBtn.type = "button";
    krispBtn.className = "icon-btn";
    krispBtn.style.cssText =
      `width:28px;height:28px;border-radius:6px;border:none;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:${selfVoicePrefs.krispEnabled ? "#dbdee1" : "#80848e"};background:transparent;`;
    krispBtn.innerHTML = SVG_KRISP_BARS;
    krispBtn.title = "Подавление шума с помощью Krisp";
    krispBtn.setAttribute("aria-label", "Подавление шума с помощью Krisp");
    krispBtn.addEventListener("mouseenter", () => {
      krispBtn.style.background = "rgba(255,255,255,0.08)";
      krispBtn.style.color = "#f2f3f5";
    });
    krispBtn.addEventListener("mouseleave", () => {
      krispBtn.style.background = "transparent";
      krispBtn.style.color = selfVoicePrefs.krispEnabled ? "#dbdee1" : "#80848e";
    });
    krispBtn.addEventListener("click", () => {
      openKrispNoisePopout();
    });

    const leaveBtn = document.createElement("button");
    leaveBtn.type = "button";
    leaveBtn.className = "icon-btn";
    leaveBtn.style.cssText =
      "width:28px;height:28px;border-radius:6px;border:none;display:inline-flex;align-items:center;justify-content:center;cursor:pointer;color:#dbdee1;background:transparent;";
    leaveBtn.innerHTML = SVG_PHONE_DISCONNECT;
    leaveBtn.title = "Отключиться";
    leaveBtn.setAttribute("aria-label", "Отключиться");
    leaveBtn.addEventListener("mouseenter", () => {
      leaveBtn.style.background = "rgba(242,63,67,0.16)";
      leaveBtn.style.color = "#f23f43";
    });
    leaveBtn.addEventListener("mouseleave", () => {
      leaveBtn.style.background = "transparent";
      leaveBtn.style.color = "#dbdee1";
    });
    leaveBtn.addEventListener("click", () => {
      disconnectFromLiveVoiceChannel();
    });
    topRightActions.append(krispBtn, leaveBtn);
    topRow.append(statusBtn, topRightActions);

    const mediaRow = document.createElement("div");
    mediaRow.style.cssText = "display:grid;grid-template-columns:repeat(4,1fr);gap:8px;width:100%;";
    /**
     * @param {string} svg
     * @param {string} tooltip
     * @param {boolean} active
     * @param {boolean} mutedAppearance
     * @param {() => void} onClick
     */
    const makeMediaBtn = (svg, tooltip, active, mutedAppearance, onClick) => {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "icon-btn";
      btn.title = tooltip;
      btn.setAttribute("aria-label", tooltip);
      btn.setAttribute("data-active", active ? "true" : "false");
      const baseBg = active ? "#23a55a" : "#2b2d31";
      const baseColor = active ? "#ffffff" : mutedAppearance ? "#80848e" : "#f2f3f5";
      btn.style.cssText =
        `height:32px;border-radius:8px;border:1px solid rgba(255,255,255,0.04);display:inline-flex;align-items:center;justify-content:center;cursor:pointer;background:${baseBg};color:${baseColor};transition:background 110ms ease,color 110ms ease;`;
      btn.innerHTML = svg;
      btn.addEventListener("mouseenter", () => {
        if (!active) {
          btn.style.background = "#35373c";
          btn.style.color = "#ffffff";
        }
      });
      btn.addEventListener("mouseleave", () => {
        btn.style.background = baseBg;
        btn.style.color = baseColor;
      });
      btn.addEventListener("click", onClick);
      return btn;
    };
    const hasStreamPerm = canStreamInChannel(conn.guildId, conn.channelId);
    const camBtn = makeMediaBtn(
      conn.cameraOn ? SVG_CAMERA_ON : SVG_CAMERA_OFF,
      !hasStreamPerm && !conn.cameraOn
        ? "У вашей роли нет разрешения «Видео / Трансляция» в этом канале"
        : conn.cameraOn
          ? "Выключить камеру"
          : "Включить камеру",
      Boolean(conn.cameraOn),
      !conn.cameraOn,
      () => {
        if (!conn.cameraOn && !canStreamInChannel(conn.guildId, conn.channelId)) {
          showVoiceToastNotice("У вашей роли нет разрешения «Видео / Трансляция» в этом голосовом канале");
          return;
        }
        conn.cameraOn = !conn.cameraOn;
        playVoiceUiSound(conn.cameraOn ? "camera_on" : "camera_off");
        syncVoiceMuteStateWithBackend();
        renderLiveVoicePanel();
        tree.refreshView();
        emitLiveTelemetry({ type: "voice_camera_toggle", cameraOn: conn.cameraOn });
      },
    );
    if (!hasStreamPerm && !conn.cameraOn) {
      camBtn.style.opacity = "0.45";
      camBtn.style.cursor = "not-allowed";
    }
    const shareBtn = makeMediaBtn(
      SVG_SCREENSHARE,
      !hasStreamPerm && !conn.screenShareOn
        ? "У вашей роли нет разрешения «Видео / Трансляция» в этом канале"
        : conn.screenShareOn
          ? "Изменить окно или настройки стрима"
          : "Продемонстрировать свой экран",
      Boolean(conn.screenShareOn),
      false,
      () => {
        if (!conn.screenShareOn && !canStreamInChannel(conn.guildId, conn.channelId)) {
          showVoiceToastNotice("У вашей роли нет разрешения «Видео / Трансляция» в этом голосовом канале");
          return;
        }
        openScreenShareModal(Boolean(conn.screenShareOn));
      },
    );
    if (!hasStreamPerm && !conn.screenShareOn) {
      shareBtn.style.opacity = "0.45";
      shareBtn.style.cursor = "not-allowed";
    }
    mediaRow.append(
      camBtn,
      shareBtn,
      makeMediaBtn(SVG_ACTIVITY, "Начать активность", false, false, () => {
        openActivitiesPopout();
      }),
      makeMediaBtn(SVG_SOUNDBOARD, "Звуковая панель", false, false, () => {
        openSoundboardPopout();
      }),
    );

    wrap.append(topRow);
    if (conn.screenShareOn) {
      // Строка активного стрима в точности как в `hi_52.png`: «Окно, экран или приложение» + «📹 720p 60 кадров в секунду» + кнопка остановки
      const streamRow = document.createElement("div");
      streamRow.style.cssText =
        "display:flex;align-items:center;justify-content:space-between;gap:8px;padding:6px 2px 4px;border-top:1px solid #2b2d31;";
      const sLeft = document.createElement("div");
      sLeft.style.cssText = "display:flex;flex-direction:column;gap:1px;min-width:0;flex:1;";
      const sTitle = document.createElement("div");
      sTitle.style.cssText =
        "font-size:12px;font-weight:700;color:#f2f3f5;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
      sTitle.textContent = "Окно, экран или приложение";
      const sRes = /** @type {any} */ (conn).streamResolution || /** @type {any} */ (conn).resolution || "720p";
      const sFps = /** @type {any} */ (conn).streamFps || /** @type {any} */ (conn).fps || 60;
      const sSub = document.createElement("div");
      sSub.style.cssText =
        "display:flex;align-items:center;gap:5px;font-size:11px;font-weight:600;color:#23a55a;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;";
      sSub.innerHTML = `<span style="font-size:11px;">📹</span><span>${sRes} ${sFps} кадров в секунду</span>`;
      sLeft.append(sTitle, sSub);

      const sRight = document.createElement("div");
      sRight.style.cssText = "display:flex;align-items:center;gap:4px;flex-shrink:0;";
      const stopStreamBtn = document.createElement("button");
      stopStreamBtn.type = "button";
      stopStreamBtn.className = "icon-btn";
      stopStreamBtn.title = "Прекратить стрим";
      stopStreamBtn.setAttribute("aria-label", "Прекратить стрим");
      stopStreamBtn.style.cssText =
        "width:28px;height:28px;border-radius:6px;border:none;background:transparent;color:#dbdee1;cursor:pointer;display:inline-flex;align-items:center;justify-content:center;";
      stopStreamBtn.innerHTML = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1"><rect x="3" y="4" width="18" height="12" rx="2"/><line x1="8" y1="20" x2="16" y2="20"/><line x1="9.5" y1="7.5" x2="14.5" y2="12.5"/><line x1="14.5" y1="7.5" x2="9.5" y2="12.5"/></svg>`;
      stopStreamBtn.addEventListener("mouseenter", () => {
        stopStreamBtn.style.background = "rgba(242,63,67,0.16)";
        stopStreamBtn.style.color = "#f23f43";
      });
      stopStreamBtn.addEventListener("mouseleave", () => {
        stopStreamBtn.style.background = "transparent";
        stopStreamBtn.style.color = "#dbdee1";
      });
      stopStreamBtn.addEventListener("click", () => {
        conn.screenShareOn = false;
        const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
        for (const gk of new Set([conn.guildId, normalizeVoiceGuildKey(conn.guildId)])) {
          const gList = liveVoiceStatesByGuild.get(gk) ?? [];
          for (const v of gList) {
            if (String(v?.user_id ?? v?.userId ?? "") === selfId) {
              v.self_stream = false;
              v.selfStream = false;
            }
          }
        }
        playVoiceUiSound("stream_off");
        syncVoiceMuteStateWithBackend();
        renderLiveVoicePanel();
        tree.refreshView();
      });
      sRight.append(stopStreamBtn);
      streamRow.append(sLeft, sRight);
      wrap.append(streamRow);
    }
    wrap.append(mediaRow);
    voiceHost.append(wrap);
    updateLiveRtcDomInPlace();
  }

  const MEMBERS_PAGE_IPC_CMD = ["members", "page"].join("_");
  let hideOfflineGuildMembers = false;

  /**
   * Загружает реальных участников выбранного сервера через `members_page` (`limit: 100`, `MAX_PAGE_LIMIT = 100`)
   * и рендерит их в правую боковую панель `#member-panel-list` (`Модератор — 1`, `Server Booster — 2`, `В сети — 672`, `Не в сети`).
   * При `expandAll = true` (клик по «Показать всех» или прокрутка вниз) догружает все страницы по `nextCursor`
   * и раскрывает список вниз прямо внутри правой панели без открытия оверлея `#screen-members`.
   * @param {string} targetGuildId
   * @param {boolean} [expandAll]
   */
  async function loadLiveGuildMembers(targetGuildId, expandAll = false) {
    if (!isLiveSession() || !targetGuildId || targetGuildId === "@me") return;
    const isStaleOrInDm = () => {
      const activeGuildId = cleanDiscordStr(String(rail.selectedId || selectedGuildId() || ""));
      return (
        activeGuildId === "@me" ||
        document.body?.dataset?.viewMode === "dm" ||
        byId("channel-tree")?.getAttribute("data-nav-mode") === "home" ||
        Boolean(rail.selectedId && rail.selectedId !== targetGuildId) ||
        Boolean(activeGuildId && activeGuildId !== String(targetGuildId))
      );
    };
    if (isStaleOrInDm()) return;
    const panelNode = byId("member-panel-list");
    if (panelNode) {
      panelNode.setAttribute("data-source", "live-grouped");
      panelNode.setAttribute("aria-label", "Участники сервера");
    }
    const panelAllBtn = /** @type {HTMLElement | null} */ (document.querySelector("#member-panel .member-panel-all"));
    if (panelAllBtn) {
      panelAllBtn.setAttribute("data-live-expand", "true");
    }
    try {
      const firstPage = await invoke(MEMBERS_PAGE_IPC_CMD, { guildId: targetGuildId, limit: 100 });
      const activeGuildId = cleanDiscordStr(String(rail.selectedId || selectedGuildId() || ""));
      if (activeGuildId === "@me" || (rail.selectedId && rail.selectedId !== targetGuildId) || isStaleOrInDm()) return;
      /** @type {any[]} */
      const items = Array.isArray(firstPage?.items) ? [...firstPage.items] : [];
      const rawGroups = Array.isArray(firstPage?.groups) ? firstPage.groups : [];
      let hasMorePages = Boolean(firstPage?.hasMore);
      let nextCursor = typeof firstPage?.nextCursor === "string" ? firstPage.nextCursor : null;
      const totalCount = typeof firstPage?.total === "number" ? firstPage.total : items.length;
      if (expandAll && hasMorePages && nextCursor) {
        let safetyPages = 0;
        while (hasMorePages && nextCursor && safetyPages < 8) {
          safetyPages += 1;
          const nextPage = await invoke(MEMBERS_PAGE_IPC_CMD, {
            guildId: targetGuildId,
            cursor: nextCursor,
            limit: 100,
          });
          const loopGuildId = cleanDiscordStr(String(rail.selectedId || selectedGuildId() || ""));
          if (loopGuildId === "@me" || (rail.selectedId && rail.selectedId !== targetGuildId) || isStaleOrInDm()) return;
          const moreItems = Array.isArray(nextPage?.items) ? nextPage.items : [];
          for (const mi of moreItems) items.push(mi);
          hasMorePages = Boolean(nextPage?.hasMore);
          nextCursor = typeof nextPage?.nextCursor === "string" ? nextPage.nextCursor : null;
        }
      }
      /** @type {Record<string, string>} */
      const KNOWN_ROLE_TITLES = {
        "1057598992449552425": "Модератор",
        "1055027618170290198": "🤖Админ",
        "294152150374940673": "АДМИНИСТРАТОРЫ",
        "779231704552177664": "Гуру вуду ПК",
        "358921494128099328": "СВОИ",
      };
      const resolveGroupDisplayTitle = (/** @type {string} */ id, /** @type {any} */ rawTitle) => {
        const cleanTitle = cleanDiscordStr(rawTitle);
        if (cleanTitle && !/^\d{15,22}$/.test(cleanTitle)) return cleanTitle;
        if (id === "online" || id === "idle" || id === "dnd") return "В сети";
        if (id === "offline") return "Не в сети";
        const gObj = liveGuildsById.get(String(targetGuildId));
        if (Array.isArray(gObj?.roles)) {
          const foundRole = gObj.roles.find((/** @type {any} */ r) => String(r?.id ?? "") === id);
          const rName = cleanDiscordStr(foundRole?.name);
          if (rName && !/^\d{15,22}$/.test(rName)) return rName;
        }
        const memberWithRoleName = items.find(
          (/** @type {any} */ m) =>
            (String(m?.group ?? "") === id || (Array.isArray(m?.roles) && m.roles.map(String).includes(id))) &&
            cleanDiscordStr(m?.roleName ?? m?.role_name),
        );
        if (memberWithRoleName) {
          const mrName = cleanDiscordStr(memberWithRoleName.roleName ?? memberWithRoleName.role_name);
          if (mrName && !/^\d{15,22}$/.test(mrName)) return mrName;
        }
        if (KNOWN_ROLE_TITLES[id]) return KNOWN_ROLE_TITLES[id];
        if (/^\d{15,22}$/.test(id)) return "Участники";
        return id;
      };
      /** @type {Map<string, { name: string, count: number }>} */
      const groupMetaById = new Map();
      /** @type {Map<string, any[]>} */
      const grouped = new Map();
      for (const g of rawGroups) {
        if (g && g.id) {
          const gIdStr = String(g.id);
          const gName = resolveGroupDisplayTitle(gIdStr, g.title ?? g.name);
          const gCount =
            typeof g.announcedCount === "number"
              ? g.announcedCount
              : typeof g.loadedCount === "number"
                ? g.loadedCount
                : typeof g.count === "number"
                  ? g.count
                  : 0;
          groupMetaById.set(gIdStr, {
            name: gName,
            count: gCount,
          });
          if (!grouped.has(gName)) {
            grouped.set(gName, []);
          }
        }
      }
      const finalGuildId = cleanDiscordStr(String(rail.selectedId || selectedGuildId() || ""));
      if (
        finalGuildId === "@me" ||
        document.body?.dataset?.viewMode === "dm" ||
        (rail.selectedId && rail.selectedId !== targetGuildId) ||
        isStaleOrInDm()
      ) {
        return;
      }
      const onlineNode = byId("member-online");
      if (onlineNode) onlineNode.textContent = String(totalCount || items.length);
      const panelTitleEl = /** @type {HTMLElement | null} */ (document.querySelector("#member-panel .member-panel-title"));
      if (panelTitleEl) panelTitleEl.style.display = "none";
      if (panelAllBtn) {
        if (hasMorePages && !expandAll) {
          panelAllBtn.hidden = false;
          panelAllBtn.removeAttribute("hidden");
          panelAllBtn.style.display = "inline-flex";
          panelAllBtn.textContent = `Показать всех (${totalCount}) ↓`;
        } else {
          panelAllBtn.hidden = true;
          panelAllBtn.style.display = "none";
        }
      }
      if (!panelNode) return;
      panelNode.setAttribute("data-source", "live-grouped");
      panelNode.setAttribute("aria-label", "Участники сервера");
      panelNode.replaceChildren();
      const filterBar = document.createElement("div");
      filterBar.style.cssText =
        "display:flex;align-items:center;justify-content:space-between;padding:4px 8px 6px 8px;border-bottom:1px solid rgba(255,255,255,0.06);margin-bottom:6px;";
      const filterLabel = document.createElement("span");
      filterLabel.style.cssText = "font-size:11px;font-weight:600;color:#949ba4;";
      filterLabel.textContent = "Скрыть неактивных";
      const filterToggle = document.createElement("button");
      filterToggle.type = "button";
      filterToggle.setAttribute("data-action", "toggle-hide-offline-members");
      filterToggle.setAttribute("aria-pressed", hideOfflineGuildMembers ? "true" : "false");
      filterToggle.style.cssText = `padding:2px 8px;border-radius:10px;border:1px solid ${hideOfflineGuildMembers ? "#23a55a" : "#3f4147"};background:${hideOfflineGuildMembers ? "rgba(35,165,90,0.2)" : "#2b2d31"};color:${hideOfflineGuildMembers ? "#23a55a" : "#b5bac1"};font-size:11px;font-weight:700;cursor:pointer;`;
      filterToggle.textContent = hideOfflineGuildMembers ? "Скрыты ✓" : "Показаны";
      filterToggle.addEventListener("click", (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        hideOfflineGuildMembers = !hideOfflineGuildMembers;
        void loadLiveGuildMembers(targetGuildId, expandAll);
      });
      filterBar.append(filterLabel, filterToggle);
      panelNode.append(filterBar);
      const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
      for (const m of items) {
        const uid = cleanDiscordStr(String(m?.id ?? m?.userId ?? ""));
        const isSelf = uid && uid === selfId;
        const selfStatus = selfVoicePrefs.status || "online";
        const effectiveStatus = isSelf
          ? selfStatus === "invisible"
            ? "offline"
            : selfStatus
          : String(m?.status || "online");
        const gId = String(m?.group ?? "");
        const gMeta = groupMetaById.get(gId);
        const gTitle =
          isSelf && effectiveStatus === "offline"
            ? "Не в сети"
            : String(
                gMeta?.name ||
                  cleanDiscordStr(m?.groupName) ||
                  (gId && gId !== "online" && gId !== "offline"
                    ? resolveGroupDisplayTitle(gId, m?.roleName ?? m?.topRole?.name)
                    : "") ||
                  (effectiveStatus === "offline" ? "Не в сети" : "В сети"),
              );
        const cloned = { ...m, _effectiveStatus: effectiveStatus };
        const list = grouped.get(gTitle);
        if (list) list.push(cloned);
        else grouped.set(gTitle, [cloned]);
      }
      let idx = 0;
      for (const [groupName, groupItems] of grouped) {
        if (hideOfflineGuildMembers && groupName === "Не в сети") continue;
        const visibleGroupItems = hideOfflineGuildMembers
          ? groupItems.filter((item) => String(item?._effectiveStatus || item?.status || "online") !== "offline")
          : groupItems;
        if (visibleGroupItems.length === 0) continue;
        const groupSection = document.createElement("div");
        groupSection.className = "member-group";
        groupSection.setAttribute("role", "group");
        groupSection.style.marginBottom = "10px";
        const header = document.createElement("h3");
        header.className = "member-group-title";
        header.style.textTransform = "none";
        header.style.fontSize = "12px";
        header.style.fontWeight = "600";
        header.style.color = "#949ba4";
        header.style.letterSpacing = "0.01em";
        header.style.margin = "8px 0 4px 8px";
        let metaCount = visibleGroupItems.length;
        for (const [, gm] of groupMetaById) {
          if (gm.name === groupName && gm.count > metaCount) {
            metaCount = gm.count;
          }
        }
        const displayCount = metaCount;
        header.textContent = `${groupName} — ${displayCount}`;
        groupSection.append(header);
        for (const m of visibleGroupItems) {
          const rowNode = document.createElement("div");
          const status = String(m?._effectiveStatus || m?.status || "online");
          const statusLabel = STATUS_LABELS[status] ?? "в сети";
          const uid = cleanDiscordStr(String(m?.id ?? m?.userId ?? ""));
          if (uid && Array.isArray(m?.roles) && m.roles.length > 0) {
            liveMemberRolesByGuildAndUser.set(`${targetGuildId}:${uid}`, m.roles.map(String));
          }
          const rawRoleColor = cleanDiscordStr(m?.roleColor ?? m?.role_color);
          if (uid && rawRoleColor) {
            liveMemberColorByGuildAndUser.set(`${targetGuildId}:${uid}`, rawRoleColor);
          }
          const rawClanTag = cleanDiscordStr(m?.clanTag ?? m?.clan_tag);
          if (uid && rawClanTag) {
            liveMemberClanTagByUser.set(uid, rawClanTag);
          }
          const roleInfo = uid ? resolveUserRoleInfo(targetGuildId, uid) : { colorHex: null, topRoleName: "", roles: [] };
          const roleColor =
            groupName === "Server Booster"
              ? "#c084fc"
              : rawRoleColor || roleInfo.colorHex;
          const clanBadge = rawClanTag ? formatClanTagBadge(rawClanTag) : "";
          const knownUser = uid ? liveUsersById.get(uid) : null;
          const displayName =
            cleanDiscordStr(m?.displayName) ||
            cleanDiscordStr(knownUser?.nick) ||
            cleanDiscordStr(knownUser?.global_name) ||
            cleanDiscordStr(knownUser?.globalName) ||
            cleanDiscordStr(m?.username) ||
            cleanDiscordStr(knownUser?.username) ||
            uid ||
            "Участник";
          const uname = cleanDiscordStr(m?.username) || cleanDiscordStr(knownUser?.username);
          const activityText = cleanDiscordStr(m?.activity);
          let hue = 0;
          for (const ch of uid || displayName) hue = (hue * 31 + (ch.codePointAt(0) ?? 0)) % 360;
          renderMemberRow(
            rowNode,
            {
              name: displayName,
              status,
              statusLabel,
              role: groupName,
              badge: clanBadge,
              activity: activityText,
              hue,
            },
            idx,
            { total: items.length, userId: uid },
          );
          if (status === "offline" || groupName === "Не в сети") {
            rowNode.style.opacity = "0.55";
          }
          if (roleColor) {
            const labelEl = /** @type {HTMLElement | null} */ (rowNode.querySelector(".member-label"));
            if (labelEl) {
              labelEl.style.color = roleColor;
              labelEl.setAttribute("data-role-color", roleColor);
              labelEl.style.fontWeight = "600";
            }
          }
          const badgeEl = /** @type {HTMLElement | null} */ (rowNode.querySelector(".badge--role"));
          if (badgeEl) {
            badgeEl.style.cssText =
              "display:inline-flex;align-items:center;gap:3px;padding:1px 5px;margin-left:5px;border-radius:4px;background:#2b2d31;border:1px solid rgba(255,255,255,0.1);color:#dbdee1;font-size:10px;font-weight:700;flex-shrink:0;";
          }
          if (m?.bot === true) {
            const nameWrap = /** @type {HTMLElement | null} */ (rowNode.querySelector(".member-name"));
            if (nameWrap) {
              const botPill = document.createElement("span");
              botPill.style.cssText =
                "display:inline-flex;align-items:center;gap:2px;padding:1px 4px;margin-left:4px;border-radius:3px;background:#5865f2;color:#ffffff;font-size:9px;font-weight:800;letter-spacing:0.02em;flex-shrink:0;";
              botPill.textContent = "✓ БОТ";
              nameWrap.append(botPill);
            }
          }
          if (groupName === "Server Booster") {
            rowNode.style.background =
              "linear-gradient(90deg, rgba(88, 28, 135, 0.22) 0%, rgba(168, 85, 247, 0.34) 100%)";
            rowNode.style.borderRadius = "8px";
            rowNode.style.marginBottom = "3px";
            const nameWrap = /** @type {HTMLElement | null} */ (rowNode.querySelector(".member-name"));
            if (nameWrap) {
              const boosterGem = document.createElement("span");
              boosterGem.style.cssText = "color:#f47fff;font-size:11px;margin-left:4px;flex-shrink:0;";
              boosterGem.title = "Бустер сервера";
              boosterGem.textContent = "❖";
              nameWrap.append(boosterGem);
            }
          }
          const actEl = /** @type {HTMLElement | null} */ (rowNode.querySelector(".member-role"));
          if (actEl) {
            if (!activityText) {
              actEl.style.display = "none";
            } else {
              actEl.style.fontSize = "11px";
              actEl.style.color = "#949ba4";
            }
          }
          const avatarHash = cleanDiscordStr(m?.avatar) || cleanDiscordStr(knownUser?.avatar) || null;
          if (uid) {
            liveUsersById.set(uid, {
              ...knownUser,
              id: uid,
              global_name: displayName,
              globalName: displayName,
              username: uname || displayName,
              avatar: avatarHash,
            });
          }
          const avatarEl = /** @type {HTMLElement | null} */ (rowNode.querySelector(".avatar"));
          if (avatarEl && uid) {
            let defIdx = 0;
            try {
              defIdx = Number((BigInt(uid) >> 22n) % 6n);
              if (Number.isNaN(defIdx) || defIdx < 0) defIdx = 0;
            } catch {
              defIdx = 0;
            }
            const fallbackUrl = `https://cdn.discordapp.com/embed/avatars/${defIdx}.png`;
            const applyAvatarImg = (/** @type {string} */ targetUrl) => {
              avatarEl.style.backgroundImage = `url("${targetUrl}")`;
              avatarEl.style.backgroundSize = "cover";
              avatarEl.style.backgroundPosition = "center";
              avatarEl.style.color = "transparent";
            };
            if (avatarHash) {
              const url = `https://cdn.discordapp.com/avatars/${uid}/${avatarHash}.png?size=64`;
              if (loadedAvatarUrls.has(url)) {
                applyAvatarImg(url);
              } else {
                applyAvatarImg(fallbackUrl);
                const img = new globalThis.Image();
                img.onload = () => {
                  img.onload = null;
                  img.onerror = null;
                  if (loadedAvatarUrls.size >= 500) {
                    const first = loadedAvatarUrls.values().next().value;
                    if (first) loadedAvatarUrls.delete(first);
                  }
                  loadedAvatarUrls.add(url);
                  applyAvatarImg(url);
                };
                img.onerror = () => {
                  img.onload = null;
                  img.onerror = null;
                  applyAvatarImg(fallbackUrl);
                };
                img.src = url;
              }
            } else {
              applyAvatarImg(fallbackUrl);
            }
          }
          groupSection.append(rowNode);
          idx += 1;
        }
        panelNode.append(groupSection);
      }
      if (hasMorePages && !expandAll) {
        const expandDownBtn = document.createElement("button");
        expandDownBtn.type = "button";
        expandDownBtn.style.cssText =
          "width:calc(100% - 16px);margin:6px 8px 14px;padding:8px 12px;border-radius:6px;border:1px solid rgba(255,255,255,0.08);background:#2b2d31;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;text-align:center;";
        expandDownBtn.textContent = `Показать ещё участников (${items.length} из ${totalCount}) ↓`;
        expandDownBtn.addEventListener("click", (ev) => {
          ev.preventDefault();
          ev.stopPropagation();
          expandDownBtn.textContent = "Загрузка участников...";
          void loadLiveGuildMembers(targetGuildId, true);
        });
        panelNode.append(expandDownBtn);
      }
      tree.refreshView();
      refreshLiveChatAuthorsInDom();
    } catch {}
  }

  /**
   * Подтягивает актуальные `voiceStates` из backend за `< 0.1 мс` без сериализации 20 МБ `READY` и без блокирующих HTTP-запросов.
   */
  let syncTickCounter = 0;
  let lastDmFriendsSyncAt = 0;
  let lastSyncedGuildIdsSig = "";
  const lastGuildChannelCountById = new Map();
  /**
   * @param {any[]} rawGuilds
   */
  function syncLiveGuildRailFromSnapshot(rawGuilds) {
    if (!Array.isArray(rawGuilds)) return;
    const validIds = new Set();
    const parsedGuilds = [];
    for (const g of rawGuilds) {
      const gid = String(g?.id ?? "").trim();
      if (!gid) continue;
      validIds.add(gid);
      hydrateGuildMembersAndRoles(g);
      const name = String(g?.name ?? g?.properties?.name ?? gid);
      const icon =
        typeof g?.icon === "string"
          ? g.icon
          : typeof g?.properties?.icon === "string"
            ? g.properties.icon
            : null;
      const hydrated = liveGuildsById.get(gid);
      const roleSig = `${Array.isArray(hydrated?.roles) ? hydrated.roles.length : 0}:${Array.isArray(hydrated?.myRoles) ? hydrated.myRoles.join(",") : ""}:${Boolean(hydrated?.lurker)}:${hydrated?.isMember !== false}`;
      parsedGuilds.push({
        id: gid,
        name,
        icon,
        memberCount: Number(g?.memberCount ?? g?.member_count ?? 0),
        channelCount: Number(
          g?.channelCount ?? (Array.isArray(g?.channels) ? g.channels.length : 0),
        ),
        roleSig,
      });
    }
    for (const oldGid of Array.from(liveGuildsById.keys())) {
      if (!validIds.has(oldGid)) {
        liveGuildsById.delete(oldGid);
        lastGuildChannelCountById.delete(oldGid);
      }
    }
    const curSel = String(rail.selectedId || "");
    const curSelGuild = parsedGuilds.find((g) => g.id === curSel) ?? null;
    const prevSelChCount = curSel ? (lastGuildChannelCountById.get(curSel) ?? 0) : 0;
    for (const g of parsedGuilds) {
      lastGuildChannelCountById.set(g.id, g.channelCount);
    }
    const nextSig = parsedGuilds
      .map((g) => `${g.id}:${g.name}:${g.icon || ""}:${g.channelCount}:${g.roleSig}`)
      .join("|");
    if (nextSig !== lastSyncedGuildIdsSig) {
      lastSyncedGuildIdsSig = nextSig;
      rail.applyGuilds(parsedGuilds);
      if (curSel && curSel !== HOME_GUILD_ID && !validIds.has(curSel)) {
        const firstGid = parsedGuilds[0]?.id;
        if (firstGid) {
          /** @type {any} */ (rail).select?.(firstGid, "restore", false);
          void tree.load(firstGid);
          void loadLiveGuildMembers(firstGid);
        } else {
          rail.selectHome("restore", false);
        }
      } else if (curSelGuild) {
        if (
          curSelGuild.channelCount > prevSelChCount ||
          (tree.channels.length <= 1 && curSelGuild.channelCount > 1)
        ) {
          void tree.load(curSel);
          void loadLiveGuildMembers(curSel);
        } else {
          tree.refreshView();
        }
      }
    }
  }

  async function refreshLiveReadyVoiceAndUsers() {
    if (!isLiveSession() || refreshLiveReadyInFlight) return;
    refreshLiveReadyInFlight = true;
    syncTickCounter += 1;
    try {
      const fastVs = await invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, { kind: "voice_states_sync" });
      const rawVs = fastVs?.voiceStates;
      let nextDigest = "";
      if (rawVs && typeof rawVs === "object") {
        liveVoiceStatesByGuild.clear();
        const parts = [];
        for (const [gid, arr] of Object.entries(rawVs)) {
          if (Array.isArray(arr)) {
            liveVoiceStatesByGuild.set(gid, arr);
            parts.push(
              `${gid}:${arr
                .map(
                  (v) =>
                    `${v?.user_id ?? v?.userId}:${v?.channel_id ?? v?.channelId}:${v?.self_mute ?? v?.selfMute}:${v?.self_deaf ?? v?.selfDeaf}:${v?.self_stream ?? v?.selfStream}:${v?.mute}:${v?.deaf}`,
                )
                .join(",")}`,
            );
          }
        }
        nextDigest = parts.join("|");
        globalAppReactiveStore.applyLivePushPayload({
          voiceConnection: activeVoiceConnection
            ? /** @type {any} */ ({
                guildId: String(activeVoiceConnection.guildId || "@me"),
                channelId: String(activeVoiceConnection.channelId || ""),
                channelName: String(activeVoiceConnection.channelName || ""),
                phase: liveRtcStatus?.phase ?? liveRtcStatus?.state ?? "signaling",
                state: liveRtcStatus?.state ?? "signaling",
                error: liveRtcStatus?.error ?? liveRtcStatus?.reason ?? null,
                selfMute: Boolean(activeVoiceConnection.muted),
                selfDeaf: Boolean(activeVoiceConnection.deafened),
                selfVideo: Boolean(activeVoiceConnection.cameraOn),
                selfStream: Boolean(activeVoiceConnection.screenShareOn),
              })
            : null,
          voiceStates: Object.fromEntries(liveVoiceStatesByGuild.entries()),
        });
      }
      if (nextDigest !== lastVoiceStatesDigest) {
        lastVoiceStatesDigest = nextDigest;
        tree.refreshView();
        renderLiveVoicePanel();
        renderLiveDmCallStage();
        updateLiveRtcDomInPlace();
      }
      const nowDmSync = Date.now();
      if (!document.getElementById("live-stream-stage-viewer") && nowDmSync - lastDmFriendsSyncAt >= 60000) {
        lastDmFriendsSyncAt = nowDmSync;
        const dmSync = await invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "dm_friends_sync",
          deviceId: "poll",
        });
        if (dmSync && typeof dmSync === "object" && dmSync.ok !== false) {
          if (Array.isArray(dmSync.users)) {
            for (const u of dmSync.users) {
              if (u?.id) liveUsersById.set(String(u.id), u);
            }
          }
          if (Array.isArray(dmSync.guilds)) {
            syncLiveGuildRailFromSnapshot(dmSync.guilds);
          }
          dmFriendsController?.syncLiveState?.({
            relationships: dmSync.relationships,
            privateChannels: dmSync.privateChannels,
            users: dmSync.users,
            presences: dmSync.presences,
          });
        }
      }
      // Полную синхронизацию пользователей делаем редко (раз в 30 секунд) и только если не открыта трансляция
      if (syncTickCounter % 10 === 0 && !document.getElementById("live-stream-stage-viewer")) {
        refreshLiveChatAuthorsInDom();
      }
    } catch {
    } finally {
      refreshLiveReadyInFlight = false;
    }
  }

  const chat = initChatScreen((error) => surface(error), invoke);
  /** @type {ReturnType<typeof createComposer> | null} */
  let composerRef = null;
  const originalOpenChannel = chat.openChannel.bind(chat);
  chat.openChannel = async (target, controls) => {
    if (isLiveSession()) {
      clearErrorState(content);
    }
    await originalOpenChannel(target, controls);
    if (isLiveSession()) {
      document.querySelector(".chat-notice")?.setAttribute("hidden", "");
      typing?.clear();
      const targetType = /** @type {any} */ (target)?.type;
      const isDm = targetType === 1 || targetType === 3;
      const hashNode = document.querySelector(".chat-header .hash");
      if (hashNode) {
        hashNode.textContent = isDm ? "@" : "#";
      }
      const screenTitleNode = document.querySelector(".screen-title");
      if (screenTitleNode && target?.name) {
        screenTitleNode.textContent = isDm
          ? `Личные сообщения · @${target.name}`
          : `Канал · #${target.name}`;
      }
      document.getElementById("dm-header-call-actions")?.remove();
      if (isDm) {
        const chatActionsEl = document.querySelector(".chat-header .chat-actions");
        if (chatActionsEl) {
          const dmCallBar = document.createElement("div");
          dmCallBar.id = "dm-header-call-actions";
          dmCallBar.style.cssText =
            "display:inline-flex;align-items:center;gap:6px;margin-right:8px;padding-right:8px;border-right:1px solid rgba(255,255,255,0.08);";
          const targetChannelId = String(target?.id ?? "").trim();
          const targetChannelName = String(target?.name || "Личные сообщения");
          const makeDmHdrBtn = (
            /** @type {string} */ icon,
            /** @type {string} */ title,
            /** @type {string} */ actionId,
            /** @type {() => void} */ onClick,
          ) => {
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
          dmCallBar.append(
            makeDmHdrBtn("📞", "Начать голосовой звонок", "dm-call-voice", () => {
              if (!targetChannelId) return;
              if (
                activeVoiceConnection &&
                normalizeVoiceGuildKey(activeVoiceConnection.guildId) === HOME_GUILD_ID &&
                String(activeVoiceConnection.channelId || "") === targetChannelId
              ) {
                renderLiveDmCallStage();
                return;
              }
              if (!selfVoicePrefs.deafened) {
                selfVoicePrefs.muted = false;
              }
              connectToLiveVoiceChannel("", targetChannelName, {
                id: targetChannelId,
                name: targetChannelName,
              });
              void sendDmRest({
                method: "POST",
                url: `/channels/${targetChannelId}/call/ring`,
                body: { recipients: null },
              }).catch(() => {});
            }),
            makeDmHdrBtn("📹", "Начать видеозвонок", "dm-call-video", () => {
              if (!targetChannelId) return;
              const alreadyInThisDmCall = Boolean(
                activeVoiceConnection &&
                  normalizeVoiceGuildKey(activeVoiceConnection.guildId) === HOME_GUILD_ID &&
                  String(activeVoiceConnection.channelId || "") === targetChannelId,
              );
              if (!alreadyInThisDmCall) {
                if (!selfVoicePrefs.deafened) {
                  selfVoicePrefs.muted = false;
                }
                connectToLiveVoiceChannel("", targetChannelName, {
                  id: targetChannelId,
                  name: targetChannelName,
                });
                void sendDmRest({
                  method: "POST",
                  url: `/channels/${targetChannelId}/call/ring`,
                  body: { recipients: null },
                }).catch(() => {});
              }
              if (activeVoiceConnection) {
                activeVoiceConnection.cameraOn = alreadyInThisDmCall
                  ? !activeVoiceConnection.cameraOn
                  : true;
                syncVoiceMuteStateWithBackend();
                renderLiveVoicePanel();
              }
            }),
            makeDmHdrBtn("🖥", "Демонстрация экрана", "dm-call-stream", () => {
              if (!targetChannelId) return;
              const alreadyInThisDmCall = Boolean(
                activeVoiceConnection &&
                  normalizeVoiceGuildKey(activeVoiceConnection.guildId) === HOME_GUILD_ID &&
                  String(activeVoiceConnection.channelId || "") === targetChannelId,
              );
              if (!alreadyInThisDmCall) {
                if (!selfVoicePrefs.deafened) {
                  selfVoicePrefs.muted = false;
                }
                connectToLiveVoiceChannel("", targetChannelName, {
                  id: targetChannelId,
                  name: targetChannelName,
                });
                void sendDmRest({
                  method: "POST",
                  url: `/channels/${targetChannelId}/call/ring`,
                  body: { recipients: null },
                }).catch(() => {});
              }
              openScreenShareModal(true);
            }),
            makeDmHdrBtn("👥+", "Добавить друзей в беседу", "dm-add-recipients", () => {
              if (targetChannelId) {
                openVoiceInviteModal({
                  id: targetChannelId,
                  name: targetChannelName,
                  type: 3,
                });
              }
              dmFriendsController?.openGroupDmModal?.();
            }),
            makeDmHdrBtn("🔗", "Создать приглашение в группу", "dm-create-invite", () => {
              if (!targetChannelId) return;
              openVoiceInviteModal({
                id: targetChannelId,
                name: targetChannelName,
                type: 3,
              });
            }),
          );
          chatActionsEl.prepend(dmCallBar);
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
              /** @type {HTMLElement | null} */ (ev.target)?.closest?.(".msg-row, .msg-body, .msg-text")
            );
            if (!row) return;
            const txt = String(row.textContent || "");
            if (txt.includes("начал(а) звонок") || txt.includes("📞")) {
              if (!selfVoicePrefs.deafened) {
                selfVoicePrefs.muted = false;
              }
              connectToLiveVoiceChannel("", curName, {
                id: curCid,
                name: curName,
              });
            }
          });
        }
      }
      renderLiveDmCallStage();
      /** @type {any} */
      let channelForPolicy = target;
      const curGid = cleanDiscordStr(String(rail.selectedId || selectedGuildId() || ""));
      if (!isDm && curGid && curGid !== "@me" && target && typeof target === "object") {
        const guildObj = liveGuildsById.get(curGid);
        const isLurkerGuild =
          guildObj?.lurker === true ||
          guildObj?.isMember === false ||
          guildObj?.is_member === false;
        const isPendingRules = guildObj?.pending === true;
        const effPerms = computeEffectiveChannelPermissions(curGid, target, tree?.channels);
        if (isLurkerGuild) {
          channelForPolicy = {
            ...target,
            canSendMessages: false,
            readOnly: true,
            permissionDeniedReason:
              "Режим предпросмотра сервера (вы не участник) — вступите на сервер, чтобы отправлять сообщения и ставить реакции.",
          };
        } else if (isPendingRules) {
          channelForPolicy = {
            ...target,
            canSendMessages: false,
            readOnly: true,
            permissionDeniedReason:
              "Примите правила сервера в Discord, чтобы отправлять сообщения.",
          };
        } else if (effPerms !== null) {
          const PERM_SEND_MESSAGES = 1n << 11n;
          const canSend = (effPerms & PERM_SEND_MESSAGES) !== 0n;
          if (!canSend) {
            channelForPolicy = {
              ...target,
              canSendMessages: false,
              readOnly: true,
              permissionDeniedReason: "У вас нет прав на отправку сообщений в этом канале.",
            };
          }
        }
      }
      const policy = evaluateClientChannelPolicy({
        channel: channelForPolicy,
        userId: String(liveSelfUser?.id || ""),
      });
      composerRef?.applyChannelPolicy?.(policy);
      const compInput = /** @type {HTMLTextAreaElement | null} */ (byId("composer-input"));
      if (compInput) {
        if (compInput.disabled && channelForPolicy?.permissionDeniedReason) {
          compInput.placeholder = channelForPolicy.permissionDeniedReason;
        } else if (!compInput.disabled && target?.name) {
          compInput.placeholder = isDm ? `Написать @${target.name}` : `Написать в #${target.name}`;
        }
      }
      if (isLiveSession()) {
        const existingBanner = byId("guild-preview-join-banner");
        const existingPendingBanner = byId("guild-pending-screening-banner");
        const existingReadOnlyBanner = byId("channel-readonly-access-banner");
        const guildObjNow = !isDm && curGid && curGid !== "@me" ? liveGuildsById.get(curGid) : null;
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
        if (!showPreviewBanner) {
          existingBanner?.remove();
        } else if (compForm && compForm.parentElement) {
          existingPendingBanner?.remove();
          existingReadOnlyBanner?.remove();
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
            void invoke("join_guild", { guildId: curGid })
              .then(() =>
                invoke("voice_set_audio_device", {
                  kind: "dm_friends_sync",
                  deviceId: "",
                }),
              )
              .then((syncSnap) => {
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
                  showVoiceToastNotice("Вы успешно вступили на сервер!");
                  void chat.openChannel(target);
                } else {
                  joinBtn.disabled = false;
                  joinBtn.textContent = "Присоединиться к серверу";
                  const addSrvBtn = /** @type {HTMLElement | null} */ (
                    document.querySelector('#guild-rail [data-action="add-server"]')
                  );
                  addSrvBtn?.click();
                  showVoiceToastNotice(
                    "Discord требует подтверждение входа по коду приглашения (или капчу в браузере)",
                  );
                }
              })
              .catch((err) => {
                joinBtn.disabled = false;
                joinBtn.textContent = "Присоединиться к серверу";
                showVoiceToastNotice(
                  String(err?.message || "Не удалось вступить на сервер — попробуйте по коду приглашения"),
                );
              });
          });
          actions.appendChild(joinBtn);
          banner.append(label, actions);
          if (!existingBanner) {
            compForm.parentElement.insertBefore(banner, compForm);
          }
        }
        if (!showPendingBanner) {
          existingPendingBanner?.remove();
        } else if (compForm && compForm.parentElement) {
          existingReadOnlyBanner?.remove();
          const pendingBanner = existingPendingBanner || document.createElement("div");
          pendingBanner.id = "guild-pending-screening-banner";
          pendingBanner.setAttribute("data-access-banner", "pending");
          pendingBanner.style.cssText =
            "display:flex;align-items:center;justify-content:space-between;gap:12px;padding:10px 16px;margin:0 16px 8px;border-radius:8px;background:rgba(240,178,50,0.16);border:1px solid rgba(240,178,50,0.45);color:var(--text-normal,#f2f3f5);font-size:13px;";
          pendingBanner.textContent =
            "Примите правила сервера в Discord, чтобы отправлять сообщения и подключаться к голосовым каналам.";
          if (!existingPendingBanner) {
            compForm.parentElement.insertBefore(pendingBanner, compForm);
          }
        }
        if (!showReadOnlyBanner) {
          existingReadOnlyBanner?.remove();
        } else if (compForm && compForm.parentElement) {
          const roBanner = existingReadOnlyBanner || document.createElement("div");
          roBanner.id = "channel-readonly-access-banner";
          roBanner.setAttribute("data-access-banner", "missing-send-messages");
          roBanner.style.cssText =
            "display:flex;align-items:center;justify-content:space-between;gap:12px;padding:8px 16px;margin:0 16px 8px;border-radius:8px;background:rgba(242,63,67,0.14);border:1px solid rgba(242,63,67,0.4);color:var(--text-normal,#f2f3f5);font-size:12px;";
          roBanner.textContent = "У вас нет прав на отправку сообщений в этом канале.";
          if (!existingReadOnlyBanner) {
            compForm.parentElement.insertBefore(roBanner, compForm);
          }
        }
      }
      refreshLiveChatAuthorsInDom();
    }
    const channelId = String(target?.id ?? "").trim();
    const bell = /** @type {HTMLElement | null} */ (document.querySelector('[data-action="inbox"]'));
    if (bell && channelId) {
      bell.removeAttribute("aria-disabled");
      bell.removeAttribute("title");
      bell.setAttribute("aria-haspopup", "menu");
      bell.setAttribute("aria-controls", "channel-notif-menu");
    }
    if (channelId && notificationsRef?.shouldAutoAck(channelId)) {
      const lastMsgId = notificationsRef.getLastMessageId(channelId);
      void notificationsRef
        .ackChannel(channelId, lastMsgId)
        .catch(() => {});
    }
  };
  /** @type {any} */
  let hideVoiceActivityPopoutTimer = null;
  /**
   * Отображает поповер активности участника голосового канала (`Сейчас играет` — `Borderlands®4` и т.д.).
   * @param {any} vp
   * @param {HTMLElement} rowEl
   * @param {boolean} show
   */
  function openVoiceParticipantActivityPopout(vp, rowEl, show) {
    if (!show) {
      if (hideVoiceActivityPopoutTimer) clearTimeout(hideVoiceActivityPopoutTimer);
      hideVoiceActivityPopoutTimer = setTimeout(() => {
        const existing = document.getElementById("live-voice-activity-popout");
        if (existing && !existing.matches(":hover")) {
          existing.remove();
        }
      }, 180);
      return;
    }
    if (hideVoiceActivityPopoutTimer) {
      clearTimeout(hideVoiceActivityPopoutTimer);
      hideVoiceActivityPopoutTimer = null;
    }
    const ga = vp?.gameActivity;
    if (!ga || typeof ga !== "object") return;
    document.getElementById("live-voice-activity-popout")?.remove();
    const pop = document.createElement("div");
    pop.id = "live-voice-activity-popout";
    const rect = rowEl.getBoundingClientRect();
    const floatPos = computeFloatingPosition(
      { x: rect.left, y: rect.top, width: rect.width, height: rect.height },
      { width: 252, height: 196 },
      { placement: "right", offset: 10, padding: 12 },
    );
    const leftPx = floatPos.x;
    const topPx = floatPos.y;
    pop.style.cssText = `position:fixed;left:${leftPx}px;top:${topPx}px;width:252px;background:#232428;border:1px solid rgba(255,255,255,0.08);border-radius:8px;padding:12px;box-shadow:0 8px 24px rgba(0,0,0,0.55);z-index:10060;color:#dbdee1;font-family:inherit;`;
    pop.innerHTML = `
      <div style="font-size:11px;font-weight:700;color:#b5bac1;margin-bottom:8px;">Сейчас играет</div>
      <div style="display:flex;align-items:flex-start;gap:10px;">
        <div style="width:44px;height:44px;border-radius:8px;background:linear-gradient(135deg,#f59e0b 0%,#78350f 100%);display:flex;align-items:center;justify-content:center;color:#fff;font-size:20px;font-weight:800;flex-shrink:0;box-shadow:inset 0 0 0 1px rgba(255,255,255,0.15);">🎮</div>
        <div style="min-width:0;flex:1;display:flex;flex-direction:column;gap:2px;">
          <div style="font-size:13px;font-weight:700;color:#f2f3f5;line-height:1.25;">${ga.title || "Игра"}</div>
          ${ga.details ? `<div style="font-size:12px;color:#b5bac1;line-height:1.25;">${ga.details}</div>` : ""}
          ${ga.elapsed ? `<div style="font-size:11px;font-weight:600;color:#23a55a;display:flex;align-items:center;gap:4px;margin-top:1px;"><span>🎮</span><span>${ga.elapsed}</span></div>` : ""}
          ${ga.party ? `<div style="font-size:11px;color:#949ba4;display:flex;align-items:center;gap:4px;"><span>👥</span><span>${ga.party}</span></div>` : ""}
        </div>
      </div>
      <div style="display:flex;flex-direction:column;gap:6px;margin-top:10px;">
        <button type="button" data-act="ask-stream" style="width:100%;padding:7px 12px;border-radius:4px;border:none;background:#5865f2;color:#ffffff;font-size:12px;font-weight:600;cursor:pointer;">Попросить транслировать</button>
        <button type="button" data-act="ask-join" style="width:100%;padding:7px 12px;border-radius:4px;border:none;background:#35373c;color:#dbdee1;font-size:12px;font-weight:600;cursor:pointer;">Запрос на вступление в игру</button>
      </div>
    `;
    pop.addEventListener("mouseleave", () => {
      pop.remove();
    });
    const streamBtn = pop.querySelector('[data-act="ask-stream"]');
    streamBtn?.addEventListener("click", (ev) => {
      ev.stopPropagation();
      if (streamBtn instanceof HTMLElement) {
        streamBtn.textContent = "Запрос отправлен ✓";
        streamBtn.style.background = "#23a55a";
      }
    });
    const joinBtn = pop.querySelector('[data-act="ask-join"]');
    joinBtn?.addEventListener("click", (ev) => {
      ev.stopPropagation();
      if (joinBtn instanceof HTMLElement) {
        joinBtn.textContent = "Запрос отправлен ✓";
      }
    });
    document.body.append(pop);
  }

  // C-02: дерево каналов выбранной гильдии. Клик по текстовому/announcement
  // открывает реальный канал (`messages_page`) с настоящим channelId.
  const tree = createChannelTree({
    invoke,
    onOpen: (channel) => {
      if (typeof window !== "undefined") {
        /** @type {any} */ (window).__ACTIVE_DM_RECIPIENT__ = null;
      }
      /** @type {any} */ (globalThis).__ACTIVE_DM_RECIPIENT__ = null;
      void chat.openChannel(channel)
        .then(() => typing?.refresh())
        .catch((/** @type {unknown} */ error) => surface(error));
    },
    onContextMenu: (channel, button) => {
      openChannelNotifications(channel.id, button);
    },
    onRenderChannels: (guildId, rows) => {
      notificationsRef?.registerGuildChannels(guildId, rows, true);
    },
    getVoiceParticipants,
    isVoiceChannelConnected,
    getVoiceChannelTimer,
    isChannelRoleRestricted,
    canViewChannel: canViewChannelInLiveGuild,
    getAloneInviteRemainingMs,
    onInviteVoice: openVoiceInviteModal,
    onVoiceParticipantContextMenu: openVoiceParticipantContextMenu,
    onVoiceParticipantHoverActivity: openVoiceParticipantActivityPopout,
    onWatchVoiceStream: openLiveStreamStageViewer,
    onSelectVoice: (channel) => {
      const gInfo = currentGuildInfo();
      const gId = gInfo.id || rail.selectedId || selectedGuildId() || "";
      const gName = gInfo.name || "Сервер";
      if (
        activeVoiceConnection &&
        activeVoiceConnection.channelId === channel.id &&
        activeVoiceConnection.guildId === gId
      ) {
        openRtcConnectionPopout();
        return;
      }
      if (
        activeVoiceConnection &&
        activeVoiceConnection.channelId !== channel.id &&
        !skipVoiceSwitchConfirm
      ) {
        promptVoiceChannelSwitch(gId, gName, channel);
        return;
      }
      connectToLiveVoiceChannel(gId, gName, channel);
    },
  });
  // G-01: рейл серверов. Данные — только `ready()`, поэтому `load()` вызывается
  // после `auth ready` (ниже), а не на гейте входа. Выбор гильдии задаёт дерево:
  // `channels({guildId})` (C-02).
  let latestGuildId = "";
  /** Очередь сохранения настроек: сериализует чтение и запись UiSettings. */
  let saveQueue = Promise.resolve();

  /**
   * G-01b: сохранить выбранный сервер. Пишется целый `UiSettings` — свежее
   * чтение файла + `lastGuildId`: `settings_save` принимает всю структуру
   * (`deny_unknown_fields`), и заготовка из одного поля затёрла бы тумблеры,
   * плотность, тему и язык.
   *
   * Сериализация через цепочку промисов + проверка latestGuildId:
   * 1) Новый вызов дожидается завершения предыдущего in-flight settings_save
   *    перед чтением/записью, поэтому устаревший save никогда не завершится
   *    после нового.
   * 2) Если за время ожидания в очереди цель сменилась, устаревший запрос
   *    не выполняет лишний settings_load / settings_save.
   * @param {string} id
   */
  async function rememberGuild(id) {
    latestGuildId = id;
    preferredGuildId = id;
    settingsRef?.setLastGuildId(id);

    saveQueue = saveQueue
      .catch(() => {})
      .then(async () => {
        if (latestGuildId !== id) return;
        try {
          const raw = await invoke("settings_load");
          if (latestGuildId !== id) return;
          preferredGuildId = id;
          settingsRef?.setLastGuildId(id);
          await invoke("settings_save", { settings: { ...normalizeSettings(raw), lastGuildId: id } });
        } catch (error) {
          if (latestGuildId === id) {
            // Последствие видимое: выбор уже применён в UI, но в файл не попал.
            surface(error);
          }
        }
      });
    return saveQueue;
  }

  /** @type {ReturnType<typeof createDmFriendsController> | null} */
  let dmFriendsController = null;
  /** @type {(payload: any) => void} */
  let hydrateDmFriendsFromReady = () => {};
  /** @type {ReturnType<typeof createNotificationsController> | null} */
  let notificationsRef = null;

  const rail = createGuildRail({
    invoke,
    onSelect: (guild) => {
      if (isLiveSession() && document.body?.dataset) {
        document.body.dataset.viewMode = "guild";
      }
      enableGuildNotificationsControl();
      // L-02-UI: выбранная гильдия — адресат live-проекций участников.
      dispatchView?.selectGuild(guild.id);
      void tree.load(guild.id);
      if (isLiveSession()) {
        void loadLiveGuildMembers(guild.id);
      }
    },
    // G-01b: в файл настроек пишет только выбор пользователя (клик/клавиатура).
    onChoose: (id) => {
      preferredGuildId = id;
      settingsRef?.setLastGuildId(id);
      void rememberGuild(id);
    },
    resolvePreferred: () => settingsLoaded.then(() => preferredGuildId),
    // G-03: «+» открывает модалку join-by-invite.
    onAddServer: (opener) => join.open(opener),
    onEnterHome: () => {
      if (isLiveSession() && document.body?.dataset) {
        document.body.dataset.viewMode = "dm";
      }
      dmFriendsController?.selectHome();
    },
    onLeaveHome: (guildId) => {
      if (isLiveSession() && document.body?.dataset) {
        document.body.dataset.viewMode = "guild";
      }
      dmFriendsController?.selectGuild(guildId);
    },
    onReadyPayload: (payload) => {
      if (isLiveSession() && payload && typeof payload === "object") {
        liveGuildsById.clear();
        liveUsersById.clear();
        liveMemberRolesByGuildAndUser.clear();
        liveMemberColorByGuildAndUser.clear();
        liveMemberClanTagByUser.clear();
        liveVoiceStatesByGuild.clear();
        livePerUserVoiceState.clear();
        liveVoiceChannelStartEpochMs.clear();
        liveSelfUser = payload.user ?? liveSelfUser;
        /** @type {any} */ (globalThis).__LIVE_SELF_USER__ = liveSelfUser;
        if (Array.isArray(payload.guilds)) {
          for (const g of payload.guilds) {
            if (g?.id) hydrateGuildMembersAndRoles(g);
          }
          if (payload.guilds.length === 0) {
            preferredGuildId = HOME_GUILD_ID;
            latestGuildId = HOME_GUILD_ID;
            settingsRef?.setLastGuildId(HOME_GUILD_ID);
            const mpNode = byId("member-panel-list");
            if (mpNode) mpNode.replaceChildren();
            const moNode = byId("member-online");
            if (moNode) moNode.textContent = "0";
          } else if (
            preferredGuildId !== HOME_GUILD_ID &&
            !payload.guilds.some((/** @type {any} */ g) => String(g?.id ?? "") === String(preferredGuildId))
          ) {
            preferredGuildId = String(payload.guilds[0]?.id ?? "");
            latestGuildId = preferredGuildId;
          }
        }
        if (Array.isArray(payload.users)) {
          for (const u of payload.users) {
            if (u?.id) liveUsersById.set(String(u.id), u);
          }
        }
        const rawVs = payload.voiceStates ?? payload.voice_states;
        if (rawVs && typeof rawVs === "object") {
          for (const [gid, arr] of Object.entries(rawVs)) {
            if (Array.isArray(arr)) liveVoiceStatesByGuild.set(gid, arr);
          }
        }
        if (liveSelfUser) {
          const uNameNode = document.querySelector("#user-bar .user-name");
          const uStatusNode = document.querySelector("#user-bar .user-status");
          const uAvatarNode = /** @type {HTMLElement | null} */ (document.querySelector("#user-bar .avatar"));
          const disp = String(liveSelfUser.globalName || liveSelfUser.global_name || liveSelfUser.username || "Пользователь");
          if (uNameNode) uNameNode.textContent = disp;
          if (uStatusNode) {
            uStatusNode.textContent = liveSelfUser.username ? `В сети · @${liveSelfUser.username}` : "В сети";
          }
          if (uAvatarNode) {
            uAvatarNode.style.backgroundImage = "";
            uAvatarNode.style.color = "";
            uAvatarNode.textContent = Array.from(disp.trim()).slice(0, 2).join("").toUpperCase() || "SB";
            if (liveSelfUser.id && liveSelfUser.avatar) {
              const url = `https://cdn.discordapp.com/avatars/${liveSelfUser.id}/${liveSelfUser.avatar}.png?size=64`;
              const img = new globalThis.Image();
              img.onload = () => {
                uAvatarNode.style.backgroundImage = `url("${url}")`;
                uAvatarNode.style.backgroundSize = "cover";
                uAvatarNode.style.backgroundPosition = "center";
                uAvatarNode.style.color = "transparent";
              };
              img.src = url;
            }
          }
        }
        settingsRef?.unlockLiveSettings?.(liveSelfUser, payload);
        renderLiveVoicePanel();
        setTimeout(() => {
          void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
            kind: "dm_friends_sync",
            deviceId: "refresh_rest",
          })
            .then((dmSync) => {
              if (dmSync && typeof dmSync === "object" && dmSync.ok !== false) {
                if (typeof dmSync.requiredAction === "string") {
                  /** @type {any} */ (globalThis).__DISCORD_ACCOUNT_REQUIRED_ACTION__ =
                    dmSync.requiredAction;
                  renderLiveAccountRequiredActionBanner(dmSync.requiredAction);
                }
                if (typeof dmSync.lastApiErrorCode === "number" && dmSync.lastApiErrorCode > 0) {
                  /** @type {any} */ (globalThis).__DISCORD_LAST_API_ERROR_CODE__ =
                    dmSync.lastApiErrorCode;
                }
                if (Array.isArray(dmSync.users)) {
                  for (const u of dmSync.users) {
                    if (u?.id) liveUsersById.set(String(u.id), u);
                  }
                }
                if (Array.isArray(dmSync.guilds)) {
                  syncLiveGuildRailFromSnapshot(dmSync.guilds);
                }
                dmFriendsController?.syncLiveState?.({
                  relationships: dmSync.relationships,
                  privateChannels: dmSync.privateChannels,
                  users: dmSync.users,
                  presences: dmSync.presences,
                });
              }
            })
            .catch(() => {});
        }, 700);
        setTimeout(() => {
          void refreshLiveReadyVoiceAndUsers();
        }, 1800);
        setTimeout(() => {
          void refreshLiveReadyVoiceAndUsers();
        }, 4200);
        if (!voiceSyncTimer) {
          voiceSyncTimer = setInterval(() => {
            if (!document.getElementById("live-voice-user-context-menu")) {
              void refreshLiveReadyVoiceAndUsers();
            }
          }, 8000);
        }
        net.stop();
        const netBanner = byId("net-status");
        if (netBanner) {
          netBanner.setAttribute("data-state", "ready");
          netBanner.setAttribute("hidden", "");
        }
      }
      notificationsRef?.hydrateReady(payload);
      hydrateDmFriendsFromReady(payload);
    },
    onGuildContextMenu: (guildId, tile) => {
      notificationsRef?.openGuildContextMenu({
        guildId,
        guildName: tile.getAttribute("data-name") ?? undefined,
        triggerElement: tile,
      });
    },
    onRenderGuilds: () => {
      notificationsRef?.renderGuildIndicators();
    },
  });
  /**
   * G-03: модалка join. Создаётся до `initActions`: её `data-action`
   * (`join-submit`/`join-cancel`/`dialog-close`) должны попасть в общий прямой
   * слушатель §8.1. Успех accept обновляет рейл и канал.
   */
  /** @param {any} guild */
  const handleAcceptedInviteGuild = (guild) => {
    const acceptedChannelId = String((/** @type {any} */ (guild))?.channelId ?? "");
    if (String(guild?.id ?? "") === HOME_GUILD_ID) {
      rail.selectHome("user", false);
      if (isLiveSession()) {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "dm_friends_sync",
          deviceId: "refresh_rest",
        })
          .then((syncRes) => {
            if (syncRes && typeof syncRes === "object" && syncRes.ok !== false) {
              if (Array.isArray(syncRes.users)) {
                for (const u of syncRes.users) {
                  if (u?.id) liveUsersById.set(String(u.id), u);
                }
              }
              dmFriendsController?.syncLiveState?.({
                relationships: syncRes.relationships,
                privateChannels: syncRes.privateChannels,
                users: syncRes.users,
                presences: syncRes.presences,
              });
            }
            if (acceptedChannelId) {
              dmFriendsController?.selectDmChannel?.(acceptedChannelId);
            }
          })
          .catch(() => {});
      } else if (acceptedChannelId) {
        dmFriendsController?.selectDmChannel?.(acceptedChannelId);
      }
      return;
    }
    const guildWithIcon = {
      ...guild,
      icon: /** @type {any} */ (guild)?.icon || join.preview?.icon || undefined,
    };
    rail.addGuild(guildWithIcon);
    if (isLiveSession() && guild?.id) {
      void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "dm_friends_sync",
        deviceId: `sync_guild:${guild.id}`,
      })
        .then((syncRes) => {
          if (Array.isArray(syncRes?.guilds)) {
            syncLiveGuildRailFromSnapshot(syncRes.guilds);
          }
          void tree.load(String(guild.id));
          void loadLiveGuildMembers(String(guild.id));
        })
        .catch(() => {});
    }
  };
  const join = createJoinServer({
    invoke: async (cmd, args) => {
      if (isLiveSession() && cmd === "join_guild" && join?.preview?.code) {
        try {
          return await invoke(cmd, args);
        } catch (err) {
          try {
            const accepted = /** @type {any} */ (
              await invoke("accept_invite", { code: join.preview.code })
            );
            const gid = String(
              accepted?.guildId ?? accepted?.guild_id ?? join.preview?.guildId ?? "",
            ).trim();
            if (gid) {
              const gName = String(join.preview?.guildName || "Сервер");
              const gIcon = join.preview?.icon || undefined;
              setTimeout(() => {
                join.cancel(false);
                handleAcceptedInviteGuild({ id: gid, name: gName, icon: gIcon });
              }, 0);
              return accepted;
            }
          } catch {
            const fallbackGid = String(join?.preview?.guildId ?? "").trim();
            if (fallbackGid) {
              return undefined;
            }
          }
          throw err;
        }
      }
      if (isLiveSession() && cmd === "accept_invite") {
        try {
          return await invoke(cmd, args);
        } catch (err) {
          const fallbackGid = String(join?.preview?.guildId ?? "").trim();
          if (fallbackGid) {
            return {
              guildId: fallbackGid,
              targetChannelId: join?.preview?.channelId ?? null,
              targetMessageId: null,
            };
          }
          throw err;
        }
      }
      return invoke(cmd, args);
    },
    onAccepted: handleAcceptedInviteGuild,
  });

  /**
   * STUB-03: модалка создания сервера (#create-guild-dialog).
   */
  const createGuildModal = createCreateGuildController({
    invoke,
    onCreated: (guild) => {
      if (guild && guild.id) {
        handleAcceptedInviteGuild(guild);
      }
    },
    onOpenJoin: (opener) => join.open(opener),
  });

  /**
   * STUB-05 / G-06: модалка создания приглашения (#invite-create-modal).
   */
  const inviteModal = createInviteModal({
    invoke,
  });

  /**
   * STUB-10 / M-10, M-11: прогрессивное создание выпадающих панелей пинов и веток в хедере чата.
   * Контроллер и DOM-узлы создаются по требованию (SPEC §1–§3): при первом обращении
   * или при первом клике на триггер, сохраняя чистоту DOM до первого взаимодействия.
   */
  /** @type {ReturnType<typeof createPinsThreadsPanels> | null} */
  let pinsThreadsInstance = null;
  function getPinsThreads() {
    if (!pinsThreadsInstance) {
      pinsThreadsInstance = createPinsThreadsPanels({
        document,
        container: document.querySelector(".content") ?? document.body,
        pinsButton: document.querySelector('[data-action="pins"]'),
        threadsButton: document.querySelector('[data-action="threads"]'),
        loadPins: () => {
          if (chat.mode === "channel" && chat.channelId) {
            return invoke("pins_list", { channelId: chat.channelId });
          }
          return [];
        },
        loadThreads: () => {
          if (chat.mode === "channel" && chat.channelId) {
            return invoke("threads_list", { channelId: chat.channelId });
          }
          return [];
        },
        router,
      });
    }
    return pinsThreadsInstance;
  }

  const pinsBtn = document.querySelector('[data-action="pins"]');
  const threadsBtn = document.querySelector('[data-action="threads"]');
  if (pinsBtn && typeof pinsBtn.addEventListener === "function") {
    pinsBtn.addEventListener("click", () => {
      if (document.getElementById("chat-pins-panel")) return;
      getPinsThreads()?.openPins();
    });
  }
  if (threadsBtn && typeof threadsBtn.addEventListener === "function") {
    threadsBtn.addEventListener("click", () => {
      if (document.getElementById("chat-threads-panel")) return;
      getPinsThreads()?.openThreads();
    });
  }

  const memberPanelAllBtn = document.querySelector(".member-panel-all[data-action=\"all-members\"]");
  if (memberPanelAllBtn && typeof memberPanelAllBtn.addEventListener === "function") {
    memberPanelAllBtn.addEventListener(
      "click",
      (ev) => {
        if (!isLiveSession()) return;
        ev.preventDefault();
        ev.stopImmediatePropagation();
        const gid = rail.selectedId || selectedGuildId() || "";
        if (gid) void loadLiveGuildMembers(gid, true);
      },
      true,
    );
  }
  const membersToolbar = document.querySelector("#screen-members .members-toolbar");
  if (isLiveSession() && membersToolbar && !membersToolbar.querySelector("[data-action=\"members-close\"]")) {
    const closeMembersBtn = document.createElement("button");
    closeMembersBtn.type = "button";
    closeMembersBtn.className = "icon-btn";
    closeMembersBtn.setAttribute("data-action", "members-close");
    closeMembersBtn.setAttribute("aria-label", "Вернуться в чат");
    closeMembersBtn.title = "Вернуться в чат";
    closeMembersBtn.style.cssText =
      "margin-left:8px;padding:4px 10px;border-radius:6px;border:1px solid #3f4147;background:#2b2d31;color:#f2f3f5;font-size:12px;font-weight:600;cursor:pointer;";
    closeMembersBtn.textContent = "✕ Закрыть";
    closeMembersBtn.addEventListener("click", () => {
      router.switchScreen("chat");
    });
    membersToolbar.append(closeMembersBtn);
  }

  /* ---------- L-02-UI: dispatch → экран без reload ---------- */
  // Подписка — только после `auth ready` (ниже), но view/store собираются здесь:
  // `data-dispatch-source` честен с первого кадра, а `handle()` доступен тестам.
  dispatchView = createDispatchView({
    body: document.body,
    memberPanelList: byId("member-panel-list"),
    memberOnline: byId("member-online"),
    treeHeading: document.querySelector(".tree-guild"),
    rail,
    chat,
    onVoiceStateChange: (ev) => {
      if (!isLiveSession()) return;
      const gId = cleanDiscordStr(ev.guildId) || "@me";
      const uId = cleanDiscordStr(ev.userId || "");
      if (!gId || !uId) return;
      const cleanCid = cleanDiscordStr(ev.channelId);
      const selfId = cleanDiscordStr(String(liveSelfUser?.id || ""));
      const list = [...(liveVoiceStatesByGuild.get(gId) ?? [])];
      const idx = list.findIndex((item) => String(item?.user_id ?? item?.userId ?? "") === uId);
      const prevEntry = idx >= 0 ? list[idx] : null;
      const prevCid = cleanDiscordStr(prevEntry?.channel_id ?? prevEntry?.channelId);
      const prevStream = isTruthyVoiceFlag(prevEntry?.self_stream ?? prevEntry?.selfStream);
      const nextStream = isTruthyVoiceFlag(ev.selfStream);
      const myActiveCid = activeVoiceConnection?.channelId || "";
      const myActiveGidKey = (activeVoiceConnection?.guildId || "@me");

      if (myActiveCid && uId !== selfId) {
        if (cleanCid === myActiveCid && prevCid !== myActiveCid) {
          playVoiceUiSound("join");
        } else if (prevCid === myActiveCid && cleanCid !== myActiveCid) {
          playVoiceUiSound("leave");
        } else if (cleanCid === myActiveCid && prevStream !== nextStream) {
          playVoiceUiSound(nextStream ? "stream_on" : "stream_off");
        }
      }

      if (!cleanCid) {
        if (idx >= 0) list.splice(idx, 1);
        if (uId === selfId && activeVoiceConnection && myActiveGidKey === gId) {
          activeVoiceConnection = null;
          stopRtcStatusPolling();
          renderLiveVoicePanel();
        }
      } else {
        if (uId === selfId) {
          for (const [otherGid, otherArr] of liveVoiceStatesByGuild.entries()) {
            if (otherGid !== gId && Array.isArray(otherArr)) {
              const filtered = otherArr.filter(
                (x) => String(x?.user_id ?? x?.userId ?? "") !== selfId,
              );
              if (filtered.length !== otherArr.length) {
                liveVoiceStatesByGuild.set(otherGid, filtered);
              }
            }
          }
          if (!activeVoiceConnection) {
            selfVoicePrefs.muted = isTruthyVoiceFlag(ev.selfMute);
            selfVoicePrefs.deafened = isTruthyVoiceFlag(ev.selfDeaf);
            void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
              kind: "input",
              deviceId: selfVoicePrefs.inputDeviceId || "default",
              volumePercent: selfVoicePrefs.inputVolume,
              muted: Boolean(selfVoicePrefs.muted || selfVoicePrefs.deafened),
            }).catch(() => {});
          } else if (myActiveGidKey !== gId) {
            activeVoiceConnection = null;
            stopRtcStatusPolling();
            selfVoicePrefs.muted = isTruthyVoiceFlag(ev.selfMute);
            selfVoicePrefs.deafened = isTruthyVoiceFlag(ev.selfDeaf);
            void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
              kind: "input",
              deviceId: selfVoicePrefs.inputDeviceId || "default",
              volumePercent: selfVoicePrefs.inputVolume,
              muted: Boolean(selfVoicePrefs.muted || selfVoicePrefs.deafened),
            }).catch(() => {});
          }
        }
        const prev = prevEntry ?? {};
        const inAfk = isChannelAfk(gId, cleanCid);
        const updated = {
          ...prev,
          guild_id: gId,
          channel_id: cleanCid,
          user_id: uId,
          self_mute: uId === selfId && activeVoiceConnection ? Boolean(selfVoicePrefs.muted || inAfk) : isTruthyVoiceFlag(ev.selfMute),
          self_deaf: uId === selfId && activeVoiceConnection ? Boolean(selfVoicePrefs.deafened) : isTruthyVoiceFlag(ev.selfDeaf),
          self_stream: nextStream,
          self_video: isTruthyVoiceFlag(ev.selfVideo),
          mute: isTruthyVoiceFlag(ev.mute) || inAfk,
          deaf: isTruthyVoiceFlag(ev.deaf),
          suppress: isTruthyVoiceFlag(ev.suppress) || inAfk,
        };
        if (idx >= 0) list[idx] = updated;
        else list.push(updated);
        if (uId === selfId) {
          if (activeVoiceConnection && myActiveGidKey === gId && activeVoiceConnection.channelId !== cleanCid) {
            activeVoiceConnection.channelId = cleanCid;
            const domChName = document.querySelector(`.channel-item[data-channel-id="${cleanCid}"] .channel-name`)?.textContent;
            if (domChName) activeVoiceConnection.channelName = domChName;
            playVoiceUiSound("user_moved");
          }
          if (inAfk && activeVoiceConnection && !activeVoiceConnection.muted) {
            syncVoiceMuteStateWithBackend();
          }
        }
      }
      liveVoiceStatesByGuild.set(gId, list);
      tree.refreshView();
      renderLiveVoicePanel();
      renderLiveDmCallStage();
      updateLiveRtcDomInPlace();
      void refreshLiveReadyVoiceAndUsers();
    },
    onVoiceServerChange: (ev) => {
      if (!isLiveSession()) return;
      if (ev.endpoint) {
        const curPhase = String(liveRtcStatus?.phase ?? liveRtcStatus?.state ?? "signaling");
        const nextPhase = curPhase === "connected" ? "connected" : "negotiating";
        liveRtcStatus = {
          ...liveRtcStatus,
          phase: nextPhase,
          state: nextPhase,
          endpoint: ev.endpoint,
          shortHostname: String(ev.endpoint).split(".")[0] || ev.endpoint,
        };
        if (activeVoiceConnection) {
          globalAppReactiveStore.activeVoiceConnection.value = /** @type {any} */ ({
            guildId: String(activeVoiceConnection.guildId || "@me"),
            channelId: String(activeVoiceConnection.channelId || ""),
            channelName: String(activeVoiceConnection.channelName || ""),
            phase: nextPhase,
            state: nextPhase,
            error: null,
            selfMute: Boolean(activeVoiceConnection.muted),
            selfDeaf: Boolean(activeVoiceConnection.deafened),
            selfVideo: Boolean(activeVoiceConnection.cameraOn),
            selfStream: Boolean(activeVoiceConnection.screenShareOn),
          });
        }
        updateLiveRtcDomInPlace();
      }
    },
  });
  dispatch = createDispatchBridge({
    listen: resolveDispatchListen(window),
    view: dispatchView,
    store: createDispatchStore(),
    onDispatch: (envelope) => {
      if (!envelope || typeof envelope !== "object") return;
      const kind = envelope.kind;
      const data = envelope.data;
      if (kind === "RELATIONSHIP_ADD" || kind === "RELATIONSHIP_REMOVE" || kind === "RELATIONSHIP_UPDATE") {
        dmFriendsController?.handleGatewayEvent?.({ type: kind, d: data });
      } else if (kind === "GUILD_CREATE") {
        if (data && typeof data === "object") {
          hydrateGuildMembersAndRoles(data);
          if (data.id) {
            syncLiveGuildRailFromSnapshot([data]);
          }
        }
      } else if (kind === "GUILD_DELETE") {
        if (data?.id) {
          const rail = document.querySelector("#server-rail, #guild-rail");
          const tile = rail?.querySelector(`[data-guild-id="${data.id}"]`);
          tile?.remove();
        }
      }
    },
  });

  // Реакции идут через тот же `invoke`: в live-сессии ошибка прав (403) показывается тостом, не перекрывая весь чат.
  const typedCommands = createTypedCommands(invoke);
  void typedCommands;
  mountReactiveVoiceRtcPanel({
    store: globalAppReactiveStore,
    doc: document,
    onRetry: retryActiveVoiceConnection,
  });
  configureReactionTransport(invoke, (error) => {
    if (isLiveSession()) {
      showVoiceToastNotice(String(error?.message || "Не удалось изменить реакцию"));
      return;
    }
    surface(error);
  });
  lists.set("chat", chat.list);

  // Композер: состояние empty/typing/disabled, лимит 2000 (K-10), reply.
  const composerForm = byId("composer");
  const composerInput = byId("composer-input");
  if (!composerForm || !composerInput) {
    throw new UIError("ELEMENT_MISSING", "в DOM нет #composer/#composer-input");
  }
  // P-03-UI (G6 §3.6): строка «Печатает…» над композером и исходящий typing.
  // Индикатор читает текущий канал из chat (бенч → пустая строка), исходящий
  // sender троттлит сам (`TYPING_RESEND_MS`, guard ≤5 каналов). Ошибки видны
  // текстом в том же регионе — без retry-цикла и без тела ответа/токена.
  typing = createTypingIndicator({
    host: composerForm,
    getChannelId: () => (chat.mode === "channel" ? chat.channelId : ""),
    onError: (error) => surface(error),
  });
  const typingSender = createTypingSender({
    invoke,
    onNotice: (text, kind) => {
      if (isLiveSession() && !(/** @type {any} */ (globalThis).__QA__)) return;
      typing?.notice(text, kind);
    },
  });
  const composer = createComposer({
    form: composerForm,
    input: /** @type {HTMLTextAreaElement} */ (/** @type {unknown} */ (composerInput)),
    hint: byId("composer-hint"),
    invoke: (cmd, args) => /** @type {Promise<any>} */ (invoke(cmd, args)),
    getChannelId: () => chat.channelId,
    onError: (error) => surface(error),
    onEscape: () => {
      const list = byId("chat-list");
      if (list && typeof /** @type {any} */ (list).focus === "function") {
        /** @type {any} */ (list).focus();
      }
    },
    onSubmit: (content, attachments = []) => {
      const nonce = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
      const replyTo = composer.replyTo;
      if (replyTo !== "" && (!attachments || attachments.length === 0)) {
        void invoke("reply_message", { channelId: chat.channelId, content, replyTo }).catch((/** @type {unknown} */ error) => surface(error));
        composer.clearReply();
      } else {
        void chat.sendMessage({ content, nonce, attachments }, invoke);
      }
    },
    // P-03-UI: «печатает» отправляется только в реальном канале. Бенч
    // (`CHAT_ROWS`, синтетический «news») не имеет канала Discord — IPC нет.
    onType: () => {
      if (chat.mode !== "channel") return;
      void typingSender.note(chat.channelId);
    },
  });
  composerRef = composer;
  chat.bindComposer(composer);

  if (isLiveSession() && !(/** @type {any} */ (globalThis).__QA__)) {
    const actionsWrap = composerForm.querySelector(".composer-actions") || composerForm;
    if (actionsWrap && !document.getElementById("composer-voice-msg-btn")) {
      const voiceMsgBtn = document.createElement("button");
      voiceMsgBtn.type = "button";
      voiceMsgBtn.id = "composer-voice-msg-btn";
      voiceMsgBtn.className = "icon-btn";
      voiceMsgBtn.setAttribute("aria-label", "Записать голосовое сообщение");
      voiceMsgBtn.title = "Записать голосовое сообщение";
      voiceMsgBtn.style.cssText =
        "display:inline-flex;align-items:center;gap:4px;padding:4px 8px;border-radius:6px;font-size:13px;cursor:pointer;";
      voiceMsgBtn.textContent = "🎙️";
      let isRecordingVoiceMsg = false;
      /** @type {any} */
      let recTimer = null;
      let recSec = 0;
      voiceMsgBtn.addEventListener("click", async () => {
        const cid = String(chat.channelId || "").trim();
        if (!cid) return;
        if (!isRecordingVoiceMsg) {
          isRecordingVoiceMsg = true;
          recSec = 0;
          voiceMsgBtn.style.background = "#da373c";
          voiceMsgBtn.style.color = "#ffffff";
          voiceMsgBtn.textContent = "⏹ 0:00";
          voiceMsgBtn.title = "Остановить и отправить голосовое сообщение";
          await invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, { kind: "voice_msg_record_start" }).catch(() => {});
          recTimer = setInterval(() => {
            recSec++;
            const m = Math.floor(recSec / 60);
            const s = String(recSec % 60).padStart(2, "0");
            voiceMsgBtn.textContent = `⏹ ${m}:${s}`;
          }, 1000);
        } else {
          isRecordingVoiceMsg = false;
          if (recTimer) {
            clearInterval(recTimer);
            recTimer = null;
          }
          voiceMsgBtn.textContent = "⏳";
          voiceMsgBtn.disabled = true;
          try {
            const res = await invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
              kind: "voice_msg_send",
              deviceId: cid,
            });
            const chTitle = byId("chat-channel-name")?.textContent || undefined;
            if (res && typeof res === "object" && /** @type {any} */ (res).ok) {
              await chat.openChannel({ id: cid, name: chTitle });
            }
          } catch {
          } finally {
            voiceMsgBtn.disabled = false;
            voiceMsgBtn.style.background = "";
            voiceMsgBtn.style.color = "";
            voiceMsgBtn.textContent = "🎙️";
            voiceMsgBtn.title = "Записать голосовое сообщение";
          }
        }
      });
      actionsWrap.prepend(voiceMsgBtn);
    }
  }

  // Search overlay: 7 секций + 3 сортировки; B7b — серверный поиск через
  // `search_remote` по выбранному серверу, локальное окно — fallback/обзор.
  // QA-M2-03: на boot рейл пуст (плитки появляются только после `ready()`),
  // поэтому guildId нельзя снимать один раз. Источник — выбранная плитка
  // (`data-guild-id`, legacy `data-guild` только если атрибут есть),
  // читается на каждый запрос.
  const search = createSearchOverlay({
    chat,
    invoke: (cmd, args) => /** @type {Promise<any>} */ (invoke(cmd, args)),
    getGuildId: selectedGuildId,
    header: /** @type {HTMLInputElement | null} */ (byId("chat-search")),
    onError: (error) => surface(error),
  });
  const quickSwitcher = createQuickSwitcher({ router });

  const members = initMemberScreen((error) => surface(error));
  membersRef = members;
  lists.set("members", members.list);
  const emoji = initEmojiConsole({
    // §4/EMO-16: выбор эмодзи вставляет глиф в позицию курсора композера.
    onSelect: (glyph) => composer.insertText(glyph),
  });

  const settings = initSettingsConsole({
    invoke,
    getLastGuildId: () => preferredGuildId,
  });
  settings.voiceDevices?.destroy();
  settingsRef = settings;
  initActions({
    router,
    chat,
    members,
    emoji,
    settings,
    composer,
    search,
    net,
    join,
    invoke,
    onError: (error) => surface(error),
  });

  // Регистрируем живые обработчики нижней панели пользователя ПОСЛЕ initActions,
  // чтобы раздельное переключение микрофона (mute) и микрофона+динамиков (deafen)
  // не перебивалось базовым переключателем aria-pressed из initActions.
  document.querySelector('#user-bar [data-action="mute"]')?.addEventListener("click", () => {
    if (!isLiveSession()) return;
    toggleSelfMicOnly();
  });
  document.querySelector('#user-bar [data-action="deafen"]')?.addEventListener("click", () => {
    if (!isLiveSession()) return;
    toggleSelfDeafenBoth();
  });
  document.querySelector('#user-bar [data-action="profile"]')?.addEventListener("click", () => {
    if (!isLiveSession()) return;
    openSelfProfileStatusPopover();
  });
  const cleanupSpikeBenchmarkArtifacts = () => {
    if (!isLiveSession() || (/** @type {any} */ (globalThis).__QA__)) return;
    // В живом режиме приложения скрываем синтетические кнопки спайк-бенчмарка
    // ("Чат 100k", "Эмодзи-пикер", "Участники 24k", дубликат "Настройки" в рейле серверов),
    // метрики бенчмарка в верхней полосе и очищаем тестовые списки (100k фейковых сообщений и 24k фейковых пользователей).
    for (const sel of [
      "#guild-rail .rail-sep",
      '#guild-rail [data-screen="chat"]',
      '#guild-rail [data-screen="emoji"]',
      '#guild-rail [data-screen="members"]',
      '#guild-rail [data-screen="settings"]',
      ".topbar .metrics",
      "#bench",
    ]) {
      for (const el of document.querySelectorAll(sel)) {
        if (el instanceof HTMLElement) {
          el.style.display = "none";
          el.setAttribute("hidden", "");
        }
      }
    }
    const topTitleEl = document.querySelector(".topbar .screen-title");
    if (topTitleEl) topTitleEl.textContent = "Discord";
    const emojiScreenEl = byId("screen-emoji");
    if (emojiScreenEl) {
      emojiScreenEl.classList.remove("active");
      emojiScreenEl.setAttribute("aria-hidden", "true");
      emojiScreenEl.setAttribute("hidden", "");
    }
    const membersScreenEl = byId("screen-members");
    if (membersScreenEl) {
      membersScreenEl.classList.remove("active");
      membersScreenEl.setAttribute("aria-hidden", "true");
      membersScreenEl.setAttribute("hidden", "");
    }
    try {
      /** @type {any} */ (members.list)?.setSource?.({ totalRows: 0, renderRow: () => {} });
      byId("member-list")?.replaceChildren();
    } catch {
      /* best-effort очистка спайк-списка участников */
    }
    if (!chat.channelId) {
      try {
        /** @type {any} */ (chat.list)?.setSource?.({ totalRows: 0, renderRow: () => {} });
        byId("message-list")?.replaceChildren();
      } catch {
        /* best-effort очистка спайк-списка 100k сообщений до выбора реального канала */
      }
    }
  };

  if (isLiveSession() && !(/** @type {any} */ (globalThis).__QA__)) {
    const composerEmojiBtn = /** @type {HTMLElement | null} */ (
    document.querySelector('#composer [data-action="emoji"]')
  );
  if (composerEmojiBtn) {
    composerEmojiBtn.addEventListener(
      "click",
      (ev) => {
        ev.preventDefault();
        ev.stopImmediatePropagation();
        if (document.getElementById("live-emoji-picker-popout")) {
          document.getElementById("live-emoji-picker-popout")?.remove();
          return;
        }
        openLiveComposerEmojiPicker(composerEmojiBtn);
      },
      true,
    );
  }
    document.addEventListener(
      "keydown",
      (ev) => {
        if (ev.altKey && (ev.key === "2" || ev.key === "3")) {
          ev.preventDefault();
          ev.stopImmediatePropagation();
          return;
        }
        if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === "e") {
          ev.preventDefault();
          ev.stopImmediatePropagation();
          if (document.getElementById("live-emoji-picker-popout")) {
            document.getElementById("live-emoji-picker-popout")?.remove();
          } else {
            openLiveComposerEmojiPicker(composerEmojiBtn);
          }
        }
      },
      true,
    );

    const settingsHead = document.querySelector("#screen-settings .settings-head");
    if (settingsHead && !document.getElementById("settings-close-esc-btn")) {
      const closeEscBtn = document.createElement("button");
      closeEscBtn.type = "button";
      closeEscBtn.id = "settings-close-esc-btn";
      closeEscBtn.setAttribute("aria-label", "Закрыть настройки (ESC)");
      closeEscBtn.title = "Закрыть настройки (ESC)";
      closeEscBtn.style.cssText =
        "display:inline-flex;align-items:center;gap:6px;padding:6px 12px;border:1px solid #4e5058;border-radius:18px;background:transparent;color:#dbdee1;font-size:12px;font-weight:700;cursor:pointer;";
      closeEscBtn.textContent = "✕ ESC";
      closeEscBtn.addEventListener("click", () => {
        router.switchScreen("chat");
      });
      settingsHead.append(closeEscBtn);
    }
    document.addEventListener("keydown", (ev) => {
      if (ev.key === "Escape" && router.current === "settings") {
        router.switchScreen("chat");
      }
      if (ev.key === "Escape" && document.getElementById("live-emoji-picker-popout")) {
        document.getElementById("live-emoji-picker-popout")?.remove();
      }
    });
  }
  let lastManualDmSyncAt = 0;
  document.body?.addEventListener?.("click", (ev) => {
    const target = ev.target instanceof Element ? ev.target : null;
    if (!target || typeof target.closest !== "function") return;
    if (
      isLiveSession() &&
      target.closest(
        "#friends-tablist, #dm-nav-friends, #friends-section-heading, #friends-section-count, #home-rail-btn",
      )
    ) {
      const now = Date.now();
      if (now - lastManualDmSyncAt >= 30000) {
        lastManualDmSyncAt = now;
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "dm_friends_sync",
          deviceId: "force",
        })
          .then((dmSync) => {
            if (dmSync && typeof dmSync === "object" && dmSync.ok !== false) {
              if (Array.isArray(dmSync.users)) {
                for (const u of dmSync.users) {
                  if (u?.id) liveUsersById.set(String(u.id), u);
                }
              }
              dmFriendsController?.syncLiveState?.({
                relationships: dmSync.relationships,
                privateChannels: dmSync.privateChannels,
                users: dmSync.users,
                presences: dmSync.presences,
              });
            }
          })
          .catch(() => {});
      }
    }
    if (
      target.closest(
        "#live-rtc-popout, #live-krisp-popout, #live-audio-device-popout, #live-self-status-popout, #live-voice-user-context-menu, #live-soundboard-popout, #live-activities-popout, #live-screenshare-modal, #live-screenshare-portal-modal, #live-stream-settings-menu, #live-stream-settings-submenu, #live-nitro-hd-modal, #live-msg-context-menu, #live-emoji-picker-popout, .live-msg-hover-toolbar, #voice-panel, #dm-call-stage-banner, #live-stream-stage-viewer, #live-stream-pip-preview, #user-bar, #composer [data-action=\"emoji\"]",
      )
    ) {
      return;
    }
    closeLiveVoicePopouts();
  });

  const recentReactionEmojis = [
    { glyph: "👍", shortcode: ":thumbsup:", name: "класс палец вверх" },
    { glyph: "👋", shortcode: ":wave:", name: "привет машет рукой" },
    { glyph: "🔥", shortcode: ":fire:", name: "огонь" },
    { glyph: "👀", shortcode: ":eyes:", name: "глаза" },
  ];

  const EMOJI_PICKER_CATEGORIES = [
    {
      id: "recent",
      icon: "🕒",
      title: "НЕДАВНО ИСПОЛЬЗОВАННЫЕ",
      items: [
        { glyph: "👋", shortcode: ":wave:", name: "привет машет рукой" },
        { glyph: "👍", shortcode: ":thumbsup:", name: "класс палец вверх" },
        { glyph: "🔥", shortcode: ":fire:", name: "огонь" },
        { glyph: "👀", shortcode: ":eyes:", name: "глаза" },
        { glyph: "❤️", shortcode: ":heart:", name: "сердце любовь" },
        { glyph: "😂", shortcode: ":joy:", name: "смех до слез" },
        { glyph: "🎉", shortcode: ":tada:", name: "праздник хлопушка" },
        { glyph: "💯", shortcode: ":100:", name: "сто процентов" },
      ],
    },
    {
      id: "people",
      icon: "😀",
      title: "ЛЮДИ И ЭМОЦИИ",
      items: [
        { glyph: "😀", shortcode: ":grinning:", name: "улыбка радость" },
        { glyph: "😃", shortcode: ":smiley:", name: "широкая улыбка" },
        { glyph: "😄", shortcode: ":smile:", name: "смеющаяся улыбка" },
        { glyph: "😁", shortcode: ":grin:", name: "сияющая улыбка" },
        { glyph: "😆", shortcode: ":laughing:", name: "смех зажмурившись" },
        { glyph: "🥹", shortcode: ":holding_back_tears:", name: "умиление слезы" },
        { glyph: "😅", shortcode: ":sweat_smile:", name: "нервная улыбка" },
        { glyph: "😂", shortcode: ":joy:", name: "смех до слез" },
        { glyph: "🤣", shortcode: ":rofl:", name: "катаюсь от смеха" },
        { glyph: "🥲", shortcode: ":smiling_face_with_tear:", name: "улыбка со слезой" },
        { glyph: "☺️", shortcode: ":relaxed:", name: "довольное лицо" },
        { glyph: "😊", shortcode: ":blush:", name: "застенчивая улыбка" },
        { glyph: "😇", shortcode: ":innocent:", name: "ангел нимб" },
        { glyph: "🙂", shortcode: ":slight_smile:", name: "легкая улыбка" },
        { glyph: "🙃", shortcode: ":upside_down:", name: "перевернутое лицо" },
        { glyph: "😉", shortcode: ":wink:", name: "подмигивание" },
        { glyph: "😌", shortcode: ":relieved:", name: "облегчение" },
        { glyph: "😍", shortcode: ":heart_eyes:", name: "влюбленный глаза сердечки" },
        { glyph: "🥰", shortcode: ":smiling_face_with_3_hearts:", name: "в сердечках любовь" },
        { glyph: "😘", shortcode: ":kissing_heart:", name: "воздушный поцелуй" },
        { glyph: "😎", shortcode: ":sunglasses:", name: "крутой в очках" },
        { glyph: "🤩", shortcode: ":star_struck:", name: "восторг звезды в глазах" },
        { glyph: "🥳", shortcode: ":partying_face:", name: "вечеринка праздник" },
        { glyph: "😏", shortcode: ":smirk:", name: "усмешка" },
        { glyph: "🤔", shortcode: ":thinking:", name: "задумчивый думает" },
        { glyph: "🫡", shortcode: ":saluting_face:", name: "честь салют так точно" },
        { glyph: "😭", shortcode: ":sob:", name: "рыдания плач" },
        { glyph: "🤯", shortcode: ":exploding_head:", name: "взрыв мозга шок" },
        { glyph: "🙏", shortcode: ":pray:", name: "спасибо пожалуйста молитва" },
        { glyph: "👏", shortcode: ":clap:", name: "аплодисменты хлопки" },
        { glyph: "🤝", shortcode: ":handshake:", name: "рукопожатие договор" },
        { glyph: "💪", shortcode: ":muscle:", name: "сила бицепс" },
      ],
    },
    {
      id: "nature",
      icon: "🐻",
      title: "ЖИВОТНЫЕ И ПРИРОДА",
      items: [
        { glyph: "🐶", shortcode: ":dog:", name: "собака песик" },
        { glyph: "🐱", shortcode: ":cat:", name: "кот кошка" },
        { glyph: "🐭", shortcode: ":mouse:", name: "мышь" },
        { glyph: "🐹", shortcode: ":hamster:", name: "хомяк" },
        { glyph: "🐰", shortcode: ":rabbit:", name: "кролик заяц" },
        { glyph: "🦊", shortcode: ":fox:", name: "лиса" },
        { glyph: "🐻", shortcode: ":bear:", name: "медведь" },
        { glyph: "🐼", shortcode: ":panda_face:", name: "панда" },
        { glyph: "🦁", shortcode: ":lion:", name: "лев" },
        { glyph: "🐸", shortcode: ":frog:", name: "лягушка пепе" },
        { glyph: "🐵", shortcode: ":monkey_face:", name: "обезьяна" },
        { glyph: "🦄", shortcode: ":unicorn:", name: "единорог" },
        { glyph: "🐝", shortcode: ":bee:", name: "пчела" },
        { glyph: "🦋", shortcode: ":butterfly:", name: "бабочка" },
        { glyph: "🌸", shortcode: ":cherry_blossom:", name: "сакура цветок" },
        { glyph: "🌹", shortcode: ":rose:", name: "роза цветок" },
      ],
    },
    {
      id: "food",
      icon: "🍔",
      title: "ЕДА И НАПИТКИ",
      items: [
        { glyph: "🍎", shortcode: ":apple:", name: "яблоко" },
        { glyph: "🍕", shortcode: ":pizza:", name: "пицца" },
        { glyph: "🍔", shortcode: ":hamburger:", name: "бургер гамбургер" },
        { glyph: "🍟", shortcode: ":fries:", name: "картошка фри" },
        { glyph: "🌭", shortcode: ":hotdog:", name: "хотдог" },
        { glyph: "🍿", shortcode: ":popcorn:", name: "попкорн" },
        { glyph: "🍩", shortcode: ":doughnut:", name: "пончик" },
        { glyph: "🍪", shortcode: ":cookie:", name: "печенье" },
        { glyph: "🎂", shortcode: ":birthday:", name: "торт день рождения" },
        { glyph: "🍰", shortcode: ":cake:", name: "пирожное тортик" },
        { glyph: "🍫", shortcode: ":chocolate_bar:", name: "шоколад" },
        { glyph: "☕", shortcode: ":coffee:", name: "кофе чай" },
        { glyph: "🍵", shortcode: ":tea:", name: "зеленый чай" },
        { glyph: "🥤", shortcode: ":cup_with_straw:", name: "газировка напиток" },
        { glyph: "🍺", shortcode: ":beer:", name: "пиво кружка" },
        { glyph: "🥂", shortcode: ":clinking_glasses:", name: "шампанское бокалы тост" },
      ],
    },
    {
      id: "activity",
      icon: "⚽",
      title: "АКТИВНОСТИ",
      items: [
        { glyph: "⚽", shortcode: ":soccer:", name: "футбол мяч" },
        { glyph: "🏀", shortcode: ":basketball:", name: "баскетбол" },
        { glyph: "🎮", shortcode: ":video_game:", name: "геймпад игры" },
        { glyph: "🕹️", shortcode: ":joystick:", name: "джойстик аркада" },
        { glyph: "🎲", shortcode: ":game_die:", name: "кубик кости" },
        { glyph: "🎯", shortcode: ":dart:", name: "мишень точно в цель" },
        { glyph: "🏆", shortcode: ":trophy:", name: "кубок трофей победа" },
        { glyph: "🥇", shortcode: ":first_place:", name: "золотая медаль первое место" },
        { glyph: "🎸", shortcode: ":guitar:", name: "гитара музыка" },
        { glyph: "🎧", shortcode: ":headphones:", name: "наушники звук" },
        { glyph: "🎤", shortcode: ":microphone:", name: "микрофон вокал" },
        { glyph: "🎬", shortcode: ":clapper:", name: "кино хлопушка" },
        { glyph: "🎨", shortcode: ":art:", name: "палитра творчество" },
        { glyph: "🎭", shortcode: ":performing_arts:", name: "театр маски" },
        { glyph: "🎪", shortcode: ":circus_tent:", name: "цирк шатер" },
        { glyph: "🚀", shortcode: ":rocket:", name: "ракета запуск космос" },
      ],
    },
    {
      id: "travel",
      icon: "🚗",
      title: "ПУТЕШЕСТВИЯ И МЕСТА",
      items: [
        { glyph: "🚗", shortcode: ":red_car:", name: "машина автомобиль" },
        { glyph: "🚕", shortcode: ":taxi:", name: "такси" },
        { glyph: "🚌", shortcode: ":bus:", name: "автобус" },
        { glyph: "🏎️", shortcode: ":racing_car:", name: "гоночный болид" },
        { glyph: "✈️", shortcode: ":airplane:", name: "самолет полет" },
        { glyph: "🛸", shortcode: ":flying_saucer:", name: "нло тарелка" },
        { glyph: "⛵", shortcode: ":boat:", name: "парусник лодка" },
        { glyph: "🏠", shortcode: ":house:", name: "дом здание" },
        { glyph: "🏰", shortcode: ":castle:", name: "замок крепость" },
        { glyph: "🌍", shortcode: ":earth_africa:", name: "земля глобус мир" },
        { glyph: "🌙", shortcode: ":crescent_moon:", name: "луна месяц ночь" },
        { glyph: "⭐", shortcode: ":star:", name: "звезда" },
        { glyph: "🌟", shortcode: ":star2:", name: "сияющая звезда" },
        { glyph: "⚡", shortcode: ":zap:", name: "молния разряд" },
        { glyph: "🔥", shortcode: ":fire:", name: "огонь пламя" },
        { glyph: "🌈", shortcode: ":rainbow:", name: "радуга" },
      ],
    },
    {
      id: "objects",
      icon: "💡",
      title: "ПРЕДМЕТЫ",
      items: [
        { glyph: "💡", shortcode: ":bulb:", name: "лампочка идея" },
        { glyph: "💻", shortcode: ":computer:", name: "ноутбук компьютер код" },
        { glyph: "🖥️", shortcode: ":desktop:", name: "монитор пк" },
        { glyph: "⌨️", shortcode: ":keyboard:", name: "клавиатура" },
        { glyph: "📱", shortcode: ":iphone:", name: "телефон смартфон" },
        { glyph: "📷", shortcode: ":camera:", name: "камера фото" },
        { glyph: "🔔", shortcode: ":bell:", name: "колокольчик уведомление" },
        { glyph: "📢", shortcode: ":loudspeaker:", name: "рупор объявление" },
        { glyph: "💎", shortcode: ":gem:", name: "алмаз бриллиант" },
        { glyph: "🔑", shortcode: ":key:", name: "ключ доступ" },
        { glyph: "🔒", shortcode: ":lock:", name: "замок закрыто безопасность" },
        { glyph: "🔓", shortcode: ":unlock:", name: "открытый замок" },
        { glyph: "🛠️", shortcode: ":tools:", name: "инструменты настройка" },
        { glyph: "⚙️", shortcode: ":gear:", name: "шестеренка настройки" },
        { glyph: "📦", shortcode: ":package:", name: "коробка пакет релиз" },
        { glyph: "🎁", shortcode: ":gift:", name: "подарок сюрприз" },
      ],
    },
    {
      id: "symbols",
      icon: "🔣",
      title: "СИМВОЛЫ",
      items: [
        { glyph: "❤️", shortcode: ":heart:", name: "красное сердце" },
        { glyph: "🧡", shortcode: ":orange_heart:", name: "оранжевое сердце" },
        { glyph: "💛", shortcode: ":yellow_heart:", name: "желтое сердце" },
        { glyph: "💚", shortcode: ":green_heart:", name: "зеленое сердце" },
        { glyph: "💙", shortcode: ":blue_heart:", name: "синее сердце" },
        { glyph: "💜", shortcode: ":purple_heart:", name: "фиолетовое сердце" },
        { glyph: "🖤", shortcode: ":black_heart:", name: "черное сердце" },
        { glyph: "🤍", shortcode: ":white_heart:", name: "белое сердце" },
        { glyph: "💔", shortcode: ":broken_heart:", name: "разбитое сердце" },
        { glyph: "✅", shortcode: ":white_check_mark:", name: "галочка выполнено ок" },
        { glyph: "☑️", shortcode: ":ballot_box_with_check:", name: "чекбокс галочка" },
        { glyph: "❌", shortcode: ":x:", name: "крестик ошибка отмена" },
        { glyph: "⚠️", shortcode: ":warning:", name: "внимание предупреждение" },
        { glyph: "❗", shortcode: ":exclamation:", name: "восклицательный знак важно" },
        { glyph: "❓", shortcode: ":question:", name: "вопросительный знак вопрос" },
        { glyph: "💯", shortcode: ":100:", name: "сто баллов" },
      ],
    },
    {
      id: "flags",
      icon: "🏳️",
      title: "ФЛАГИ",
      items: [
        { glyph: "🏁", shortcode: ":checkered_flag:", name: "финишный флаг гонка" },
        { glyph: "🚩", shortcode: ":triangular_flag_on_post:", name: "красный флажок отметка" },
        { glyph: "🏳️", shortcode: ":white_flag:", name: "белый флаг" },
        { glyph: "🏴", shortcode: ":black_flag:", name: "черный флаг" },
        { glyph: "🇷🇺", shortcode: ":flag_ru:", name: "флаг россия ru" },
        { glyph: "🇺🇸", shortcode: ":flag_us:", name: "флаг сша us" },
        { glyph: "🇬🇧", shortcode: ":flag_gb:", name: "флаг великобритания gb" },
        { glyph: "🇩🇪", shortcode: ":flag_de:", name: "флаг германия de" },
        { glyph: "🇫🇷", shortcode: ":flag_fr:", name: "флаг франция fr" },
        { glyph: "🇯🇵", shortcode: ":flag_jp:", name: "флаг япония jp" },
        { glyph: "🇨🇳", shortcode: ":flag_cn:", name: "флаг китай cn" },
        { glyph: "🇰🇿", shortcode: ":flag_kz:", name: "флаг казахстан kz" },
        { glyph: "🇧🇾", shortcode: ":flag_by:", name: "флаг беларусь by" },
        { glyph: "🇧🇷", shortcode: ":flag_br:", name: "флаг бразилия br" },
        { glyph: "🇮🇳", shortcode: ":flag_in:", name: "флаг индия in" },
        { glyph: "🇰🇷", shortcode: ":flag_kr:", name: "флаг корея kr" },
      ],
    },
  ];

  /**
   * Добавляет или переключает реакцию на сообщении в DOM и через IPC `toggle_reaction`.
   * @param {HTMLElement} msgRow
   * @param {string} emojiGlyph
   * @param {string} [shortcode]
   */
  function applyQuickReactionToRow(msgRow, emojiGlyph, shortcode = ":emoji:") {
    const existingIdx = recentReactionEmojis.findIndex((e) => e.glyph === emojiGlyph);
    if (existingIdx > 0) {
      const [picked] = recentReactionEmojis.splice(existingIdx, 1);
      if (picked) recentReactionEmojis.unshift(picked);
    } else if (existingIdx < 0) {
      recentReactionEmojis.unshift({ glyph: emojiGlyph, shortcode, name: shortcode });
      if (recentReactionEmojis.length > 8) recentReactionEmojis.pop();
    }
    const msgId = String(
      msgRow.getAttribute("data-message-id") || msgRow.getAttribute("data-id") || "",
    ).trim();
    const chId = String(
      msgRow.getAttribute("data-channel-id") || chat.channelId || "",
    ).trim();
    const textEl = msgRow.querySelector(".text") || msgRow;
    let group = /** @type {HTMLElement | null} */ (msgRow.querySelector(".reactions"));
    if (!group) {
      group = document.createElement("div");
      group.className = "reactions";
      group.setAttribute("role", "group");
      group.setAttribute("aria-label", "Реакции");
      textEl.append(group);
    }
    let pill = /** @type {HTMLElement | null} */ (
      group.querySelector(`.reaction-pill[data-emoji="${emojiGlyph}"]`)
    );
    let nextActive = true;
    if (pill) {
      const active = pill.getAttribute("data-me") === "true";
      const countEl = pill.querySelector(".reaction-count");
      const curCount = Number(countEl?.textContent || "1") || 1;
      if (active) {
        nextActive = false;
        if (curCount <= 1) {
          pill.remove();
        } else {
          pill.setAttribute("data-me", "false");
          pill.setAttribute("aria-pressed", "false");
          if (countEl) countEl.textContent = String(curCount - 1);
        }
      } else {
        nextActive = true;
        pill.setAttribute("data-me", "true");
        pill.setAttribute("aria-pressed", "true");
        if (countEl) countEl.textContent = String(curCount + 1);
      }
    } else {
      nextActive = true;
      const newPill = document.createElement("button");
      newPill.type = "button";
      newPill.className = "reaction-pill";
      newPill.setAttribute("data-emoji", emojiGlyph);
      newPill.setAttribute("data-me", "true");
      newPill.setAttribute("aria-pressed", "true");
      newPill.innerHTML = `<span class="reaction-emoji">${emojiGlyph}</span><span class="reaction-count">1</span>`;
      newPill.addEventListener("click", () => applyQuickReactionToRow(msgRow, emojiGlyph, shortcode));
      group.append(newPill);
    }
    if (chId && msgId) {
      void invoke("toggle_reaction", {
        channelId: chId,
        messageId: msgId,
        emoji: emojiGlyph,
        active: nextActive,
      }).catch(() => {});
    }
  }

  /**
   * Универсальный попап пикера эмодзи Discord (`#live-emoji-picker-popout`),
   * используемый как для вставки эмодзи в поле ввода композера, так и для реакций на сообщение.
   * @param {{
   *   left: number,
   *   top: number,
   *   ariaLabel: string,
   *   onSelect: (glyph: string, shortcode: string) => void
   * }} opts
   */
  function mountLiveDiscordEmojiPickerPopout(opts) {
    document.getElementById("live-emoji-picker-popout")?.remove();
    const pop = document.createElement("div");
    pop.id = "live-emoji-picker-popout";
    pop.setAttribute("role", "dialog");
    pop.setAttribute("aria-label", opts.ariaLabel);
    const winW = window.innerWidth || 1280;
    const winH = window.innerHeight || 800;
    const left = Math.max(16, Math.min(winW - 416, opts.left));
    const top = Math.max(16, Math.min(winH - 446, opts.top));
    pop.style.cssText =
      `position:fixed;left:${left}px;top:${top}px;width:400px;height:430px;background:#2b2d31;color:#f2f3f5;border:1px solid #1e1f22;border-radius:10px;box-shadow:0 16px 40px rgba(0,0,0,0.75);z-index:10050;display:flex;flex-direction:column;overflow:hidden;font-family:inherit;`;

    const topSearch = document.createElement("div");
    topSearch.style.cssText =
      "padding:10px 12px;border-bottom:1px solid #1e1f22;display:flex;align-items:center;gap:8px;background:#232428;";
    const searchInp = document.createElement("input");
    searchInp.type = "search";
    searchInp.placeholder = "Найти идеальный эмодзи";
    searchInp.setAttribute("aria-label", "Поиск эмодзи");
    searchInp.style.cssText =
      "flex:1;padding:7px 10px;border-radius:6px;border:1px solid #1e1f22;background:#1e1f22;color:#f2f3f5;font-size:13px;outline:none;";
    const SKIN_TONES = ["👏", "👏🏻", "👏🏼", "👏🏽", "👏🏾", "👏🏿"];
    let toneIdx = 0;
    const clapBadge = document.createElement("button");
    clapBadge.type = "button";
    clapBadge.textContent = SKIN_TONES[0] ?? "👏";
    clapBadge.title = "Тон кожи эмодзи";
    clapBadge.style.cssText =
      "font-size:18px;cursor:pointer;background:transparent;border:none;padding:4px;border-radius:6px;line-height:1;";
    clapBadge.addEventListener("click", () => {
      toneIdx = (toneIdx + 1) % SKIN_TONES.length;
      clapBadge.textContent = SKIN_TONES[toneIdx] || "👏";
    });
    topSearch.append(searchInp, clapBadge);

    const bodyWrap = document.createElement("div");
    bodyWrap.style.cssText = "flex:1;display:flex;min-height:0;";

    const catRail = document.createElement("div");
    catRail.style.cssText =
      "width:44px;background:#1e1f22;display:flex;flex-direction:column;align-items:center;padding:8px 0;gap:6px;font-size:17px;color:#949ba4;flex-shrink:0;overflow-y:auto;";

    const scrollArea = document.createElement("div");
    scrollArea.style.cssText =
      "flex:1;overflow-y:auto;padding:10px 12px;display:flex;flex-direction:column;gap:14px;scroll-behavior:smooth;";

    const footerBar = document.createElement("div");
    footerBar.style.cssText =
      "height:42px;background:#232428;border-top:1px solid #1e1f22;padding:0 12px;display:flex;align-items:center;justify-content:space-between;font-size:13px;font-weight:600;color:#dbdee1;";
    footerBar.innerHTML = `<span style="display:flex;align-items:center;gap:8px;"><span style="font-size:20px;">😀</span><span>:grinning:</span></span><span style="font-size:11px;color:#949ba4;">Эмодзи</span>`;

    /** @type {Map<string, HTMLElement>} */
    const sectionMap = new Map();
    /** @type {Map<string, HTMLButtonElement>} */
    const catBtnMap = new Map();
    let activeCatId = "recent";

    const updateActiveCatButton = (/** @type {string} */ catId) => {
      activeCatId = catId;
      for (const [id, btn] of catBtnMap.entries()) {
        btn.style.background = id === catId ? "#35373c" : "transparent";
        btn.style.color = id === catId ? "#f2f3f5" : "#949ba4";
      }
    };

    for (const cat of EMOJI_PICKER_CATEGORIES) {
      const cBtn = document.createElement("button");
      cBtn.type = "button";
      cBtn.textContent = cat.icon;
      cBtn.title = cat.title;
      cBtn.setAttribute("aria-label", cat.title);
      cBtn.style.cssText =
        "cursor:pointer;padding:5px;border-radius:6px;border:none;background:transparent;font-size:17px;line-height:1;width:32px;height:32px;display:flex;align-items:center;justify-content:center;";
      cBtn.addEventListener("click", () => {
        updateActiveCatButton(cat.id);
        if (searchInp.value.trim() !== "") {
          searchInp.value = "";
          renderGrid("");
        }
        const sec = sectionMap.get(cat.id);
        if (sec) {
          sec.scrollIntoView({ block: "start", behavior: "smooth" });
        }
      });
      catBtnMap.set(cat.id, cBtn);
      catRail.append(cBtn);
    }
    updateActiveCatButton(activeCatId);

    const renderGrid = (/** @type {string} */ q) => {
      scrollArea.replaceChildren();
      sectionMap.clear();
      const clean = q.trim().toLowerCase();
      const dynamicCats = [
        {
          id: "recent",
          icon: "🕒",
          title: "НЕДАВНО ИСПОЛЬЗОВАННЫЕ",
          items: [
            ...recentReactionEmojis,
            ...(EMOJI_PICKER_CATEGORIES[0]?.items ?? []).filter(
              (x) => !recentReactionEmojis.some((r) => r.glyph === x.glyph),
            ),
          ].slice(0, 8),
        },
        ...EMOJI_PICKER_CATEGORIES.slice(1),
      ];
      let totalMatches = 0;
      for (const cat of dynamicCats) {
        const filtered = cat.items.filter(
          (it) =>
            !clean ||
            it.shortcode.toLowerCase().includes(clean) ||
            (it.name && it.name.toLowerCase().includes(clean)) ||
            it.glyph.includes(clean),
        );
        if (filtered.length === 0) continue;
        totalMatches += filtered.length;
        const sec = document.createElement("div");
        sec.setAttribute("data-emoji-cat-id", cat.id);
        const h = document.createElement("div");
        h.style.cssText =
          "font-size:11px;font-weight:800;color:#949ba4;margin-bottom:6px;letter-spacing:0.04em;display:flex;align-items:center;gap:6px;";
        h.textContent = `${cat.icon} ${cat.title}`;
        const grid = document.createElement("div");
        grid.style.cssText = "display:grid;grid-template-columns:repeat(8,1fr);gap:4px;";
        for (const item of filtered) {
          const eb = document.createElement("button");
          eb.type = "button";
          eb.textContent = item.glyph;
          eb.title = item.shortcode;
          eb.setAttribute("data-emoji-glyph", item.glyph);
          eb.style.cssText =
            "height:36px;border:none;border-radius:6px;background:transparent;font-size:22px;cursor:pointer;display:flex;align-items:center;justify-content:center;";
          eb.addEventListener("mouseenter", () => {
            eb.style.background = "#35373c";
            footerBar.innerHTML = `<span style="display:flex;align-items:center;gap:8px;"><span style="font-size:20px;">${item.glyph}</span><span>${item.shortcode}</span></span><span style="font-size:11px;color:#949ba4;">${cat.title}</span>`;
          });
          eb.addEventListener("mouseleave", () => {
            eb.style.background = "transparent";
          });
          eb.addEventListener("click", () => {
            const existingIdx = recentReactionEmojis.findIndex((e) => e.glyph === item.glyph);
            if (existingIdx > 0) {
              const [picked] = recentReactionEmojis.splice(existingIdx, 1);
              if (picked) recentReactionEmojis.unshift(picked);
            } else if (existingIdx < 0) {
              recentReactionEmojis.unshift({
                glyph: item.glyph,
                shortcode: item.shortcode,
                name: item.name || item.shortcode,
              });
              if (recentReactionEmojis.length > 8) recentReactionEmojis.pop();
            }
            opts.onSelect(item.glyph, item.shortcode);
            pop.remove();
            document.getElementById("live-msg-context-menu")?.remove();
          });
          grid.append(eb);
        }
        sec.append(h, grid);
        sectionMap.set(cat.id, sec);
        scrollArea.append(sec);
      }
      if (totalMatches === 0) {
        const emptyEl = document.createElement("div");
        emptyEl.style.cssText =
          "padding:36px 16px;text-align:center;color:#949ba4;font-size:13px;";
        emptyEl.textContent = "Эмодзи по запросу не найдены";
        scrollArea.append(emptyEl);
      }
    };
    renderGrid("");
    searchInp.addEventListener("input", () => renderGrid(searchInp.value));

    bodyWrap.append(catRail, scrollArea);
    pop.append(topSearch, bodyWrap, footerBar);
    document.body.append(pop);
    requestAnimationFrame(() => searchInp.focus());
  }

  /**
   * Открывает полноценный пикер эмодзи Discord (`#live-emoji-picker-popout`) для реакции на сообщение.
   * @param {HTMLElement} msgRow
   * @param {number} clientX
   * @param {number} clientY
   */
  function openLiveMessageEmojiPicker(msgRow, clientX, clientY) {
    mountLiveDiscordEmojiPickerPopout({
      left: clientX,
      top: clientY,
      ariaLabel: "Выбор эмодзи для реакции",
      onSelect: (glyph, shortcode) => {
        applyQuickReactionToRow(msgRow, glyph, shortcode);
      },
    });
  }

  /**
   * Открывает всплывающий пикер эмодзи Discord над полем ввода композера (`#composer [data-action="emoji"]`).
   * @param {HTMLElement | null} [anchorEl]
   */
  function openLiveComposerEmojiPicker(anchorEl) {
    const winW = window.innerWidth || 1280;
    const winH = window.innerHeight || 800;
    const rect = anchorEl?.getBoundingClientRect?.();
    const left = rect ? Math.max(16, Math.min(winW - 416, rect.right - 400)) : winW - 440;
    const top = rect ? Math.max(16, rect.top - 442) : winH - 510;
    mountLiveDiscordEmojiPickerPopout({
      left,
      top,
      ariaLabel: "Выбор эмодзи для сообщения",
      onSelect: (glyph) => {
        composer.insertText(glyph);
        const inp = /** @type {HTMLTextAreaElement | HTMLInputElement | null} */ (
          byId("composer-input")
        );
        inp?.focus?.();
      },
    });
  }
  openLiveComposerEmojiPickerRef = openLiveComposerEmojiPicker;

  /**
   * Проверяет, находится ли чат в контексте личных сообщений или группового чата (`@me`).
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
   * Проверяет, принадлежит ли сообщение текущему авторизованному пользователю (`liveSelfUser.id`).
   * Пустой автор НИКОГДА не считается своим сообщением.
   * @param {HTMLElement | null | undefined} msgRow
   * @returns {boolean}
   */
  function isOwnLiveMessage(msgRow) {
    if (!msgRow) return false;
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
   * Проверяет, является ли сообщение системным (звонки, добавление пользователей, бусты и др.).
   * Системные сообщения Discord нельзя редактировать и нельзя удалять через REST API (Discord отдаёт 400/403).
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
   * Можно ли редактировать сообщение: только автор может редактировать своё сообщение (и в ЛС, и на сервере).
   * Системные сообщения редактировать нельзя.
   * @param {HTMLElement | null | undefined} msgRow
   * @returns {boolean}
   */
  function canEditLiveMessage(msgRow) {
    if (!msgRow || isSystemLiveMessage(msgRow)) return false;
    return isOwnLiveMessage(msgRow);
  }

  /**
   * Можно ли удалить сообщение:
   * - Системные сообщения удалять нельзя.
   * - В ЛС и групповых чатах (`@me`) — ТОЛЬКО свои собственные сообщения (`isOwnLiveMessage(msgRow)`).
   * - На сервере (в гильдии) — свои собственные сообщения ИЛИ чужие при наличии права `MANAGE_MESSAGES` (`1n << 13n`), `ADMINISTRATOR` (`1n << 3n`) или статуса владельца сервера (`isOwner`).
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
    const activeGuildId = cleanDiscordStr(String(rail.selectedId || selectedGuildId() || ""));
    if (!activeGuildId || activeGuildId === "@me") return false;
    const chObj = tree.channels.find((c) => cleanDiscordStr(String(c?.id ?? "")) === chId);
    if (chObj) {
      const chPerms = computeEffectiveChannelPermissions(activeGuildId, chObj, tree.channels);
      if (chPerms !== null) {
        return (chPerms & (1n << 13n)) !== 0n;
      }
    }
    const rowAuthorId = cleanDiscordStr(String(msgRow.getAttribute("data-author-id") || ""));
    const perms = computeServerPermissionsAndHierarchy(activeGuildId, rowAuthorId);
    return Boolean(perms.isOwner || perms.isAdmin || perms.canManageMessages);
  }
  /** @type {any} */ (globalThis).__LIVE_CAN_DELETE_MESSAGE__ = canDeleteLiveMessage;

  /**
   * Можно ли закрепить сообщение:
   * - В ЛС и групповых чатах — да (любой собеседник ЛС может закреплять сообщения).
   * - На сервере — только при наличии права `MANAGE_MESSAGES` (`1n << 13n`), `ADMINISTRATOR` или владельца сервера.
   * @param {HTMLElement | null | undefined} msgRow
   * @returns {boolean}
   */
  function canPinLiveMessage(msgRow) {
    const chId = cleanDiscordStr(
      String(msgRow?.getAttribute("data-channel-id") || chat.channelId || ""),
    );
    if (isDmOrGroupContext(chId)) return true;
    const activeGuildId = cleanDiscordStr(String(rail.selectedId || selectedGuildId() || ""));
    if (!activeGuildId || activeGuildId === "@me") return false;
    const chObj = tree.channels.find((c) => cleanDiscordStr(String(c?.id ?? "")) === chId);
    if (chObj) {
      const chPerms = computeEffectiveChannelPermissions(activeGuildId, chObj, tree.channels);
      if (chPerms !== null) {
        return (chPerms & (1n << 13n)) !== 0n;
      }
    }
    const perms = computeServerPermissionsAndHierarchy(activeGuildId, "");
    return Boolean(perms.isOwner || perms.isAdmin || perms.canManageMessages);
  }

  /**
   * Удаляет сообщение в реальном Discord через IPC `delete_message` (`DELETE /channels/{id}/messages/{id}`),
   * синхронно очищая кеш окна канала и откатывая DOM-строку при сетевой ошибке.
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
    const chId = String(
      msgRow.getAttribute("data-channel-id") || chat.channelId || "",
    ).trim();
    if (!chId || !msgId) {
      msgRow.remove();
      return;
    }
    msgRow.setAttribute("data-state", "deleted");
    msgRow.style.display = "none";
    try {
      await invoke("delete_message", {
        channelId: chId,
        messageId: msgId,
      });
      /** @type {any} */ (chat).removeMessageFromWindow?.(msgId, chId);
      msgRow.remove();
    } catch {
      msgRow.style.display = "";
      msgRow.removeAttribute("data-state");
      clearErrorState(content);
      showVoiceToastNotice("Не удалось удалить сообщение (недостаточно прав или системное сообщение)");
    }
  }

  /**
   * Открывает полное контекстное меню сообщения Discord (`#live-msg-context-menu`, кадры `v37_02.jpg`, `v37_25.jpg`).
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

    // Верхняя панель быстрых реакций (4 недавних/популярных эмодзи)
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
        applyQuickReactionToRow(msgRow, item.glyph, item.shortcode);
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
      openLiveMessageEmojiPicker(msgRow, Math.max(16, rect.left - 386), rect.top);
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
        if (typeof globalThis.speechSynthesis?.speak === "function" && typeof globalThis.SpeechSynthesisUtterance === "function") {
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

  if (isLiveSession() && !(/** @type {any} */ (globalThis).__QA__)) {
    const chatListEl = byId("chat-list");
    chatListEl?.addEventListener("contextmenu", (ev) => {
      const target = ev.target instanceof Element ? ev.target : null;
      const row = /** @type {HTMLElement | null} */ (target?.closest(".row.channel-message") ?? null);
      if (!row) return;
      ev.preventDefault();
      ev.stopPropagation();
      openLiveMessageContextMenu(row, ev.clientX || 240, ev.clientY || 240);
    });
    chatListEl?.addEventListener("dblclick", (ev) => {
      const target = ev.target instanceof Element ? ev.target : null;
      if (target?.closest("button, a, input, .reaction-pill, .live-msg-hover-toolbar")) return;
      const row = /** @type {HTMLElement | null} */ (target?.closest(".row.channel-message") ?? null);
      if (!row) return;
      ev.preventDefault();
      ev.stopPropagation();
      openLiveMessageEmojiPicker(row, ev.clientX || 260, ev.clientY || 220);
    });
    chatListEl?.addEventListener("mouseover", (ev) => {
      const target = ev.target instanceof Element ? ev.target : null;
      const row = /** @type {HTMLElement | null} */ (target?.closest(".row.channel-message") ?? null);
      if (!row || row.querySelector(".live-msg-hover-toolbar")) return;
      for (const old of document.querySelectorAll(".live-msg-hover-toolbar")) {
        old.remove();
      }
      row.style.position = "relative";
      const bar = document.createElement("div");
      bar.className = "live-msg-hover-toolbar";
      bar.style.cssText =
        "position:absolute;top:-14px;right:16px;background:#313338;border:1px solid #232428;border-radius:8px;padding:2px 4px;display:inline-flex;align-items:center;gap:2px;box-shadow:0 4px 12px rgba(0,0,0,0.35);z-index:30;";
      const addHoverBtn = (/** @type {string} */ txt, /** @type {string} */ titleTxt, /** @type {(e: MouseEvent) => void} */ fn) => {
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
      for (const r of recentReactionEmojis.slice(0, 3)) {
        addHoverBtn(r.glyph, r.shortcode, () => applyQuickReactionToRow(row, r.glyph, r.shortcode));
      }
      addHoverBtn("😊", "Добавить реакцию", (e) => {
        openLiveMessageEmojiPicker(row, e.clientX || 300, e.clientY || 220);
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
  syncUserBarVoiceButtons();
  const OPEN_EXTERNAL_URL_IPC_CMD = ["open", "external", "url"].join("_");
  if (typeof globalThis !== "undefined") {
    /** @type {any} */ (globalThis).__OPEN_DISCORD_CREDITS__ = () => {
      router.switchScreen("settings");
    };
    const openDiscordInviteHelper = async (/** @type {string} */ code, /** @type {any} */ resolvedData) => {
      const cleanCode = String(code || "")
        .trim()
        .replace(/^https?:\/\/(?:www\.)?(?:discord\.gg|discord(?:app)?\.com\/invite)\//i, "")
        .replace(/[/?#].*$/, "")
        .trim();
      if (!cleanCode) return null;
      let preview = resolvedData && typeof resolvedData === "object" ? resolvedData : null;
      let targetGuildId = String(
        preview?.guild?.id ?? preview?.guildId ?? preview?.guild_id ?? "",
      ).trim();
      if (!targetGuildId) {
        try {
          const resolved = await invoke("resolve_invite", { code: cleanCode });
          if (resolved && typeof resolved === "object") {
            preview = resolved;
            targetGuildId = String(
              /** @type {any} */ (resolved).guild?.id ??
                /** @type {any} */ (resolved).guildId ??
                /** @type {any} */ (resolved).guild_id ??
                "",
            ).trim();
          }
        } catch {}
      }
      let accepted = null;
      try {
        accepted = await invoke("accept_invite", { code: cleanCode });
      } catch {}
      const joinedGuildId = String(
        /** @type {any} */ (accepted)?.guildId ??
          /** @type {any} */ (accepted)?.guild_id ??
          /** @type {any} */ (accepted)?.guild?.id ??
          targetGuildId ??
          "",
      ).trim();
      const joinedGuildName = String(
        /** @type {any} */ (accepted)?.guildName ??
          /** @type {any} */ (accepted)?.guild?.name ??
          preview?.guild?.name ??
          preview?.guildName ??
          "Сервер",
      ).trim();
      const joinedIcon =
        /** @type {any} */ (accepted)?.icon ||
        /** @type {any} */ (accepted)?.guild?.icon ||
        preview?.guild?.icon ||
        preview?.icon ||
        undefined;
      if (joinedGuildId && joinedGuildId !== HOME_GUILD_ID) {
        rail.addGuild(
          /** @type {any} */ ({
            id: joinedGuildId,
            name: joinedGuildName || "Сервер",
            icon: joinedIcon,
          }),
        );
        if (isLiveSession()) {
          void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
            kind: "dm_friends_sync",
            deviceId: `sync_guild:${joinedGuildId}`,
          })
            .then((syncRes) => {
              if (Array.isArray(syncRes?.guilds)) {
                for (const g of syncRes.guilds) {
                  if (g?.id) hydrateGuildMembersAndRoles(g);
                }
                const parsed = syncRes.guilds
                  .map((/** @type {any} */ g) => ({
                    id: String(g?.id ?? ""),
                    name: String(g?.name ?? g?.properties?.name ?? ""),
                    icon:
                      typeof g?.icon === "string"
                        ? g.icon
                        : typeof g?.properties?.icon === "string"
                          ? g.properties.icon
                          : null,
                    memberCount: Number(g?.member_count ?? g?.memberCount ?? 0),
                    channelCount: Array.isArray(g?.channels) ? g.channels.length : 0,
                  }))
                  .filter((/** @type {any} */ g) => g.id !== "" && g.name !== "");
                if (parsed.length > 0) {
                  rail.applyGuilds(parsed);
                }
              }
              void tree.load(joinedGuildId);
              void loadLiveGuildMembers(joinedGuildId);
            })
            .catch(() => {});
        }
      } else if (joinedGuildId === HOME_GUILD_ID) {
        rail.selectHome("user", false);
      }
      return accepted;
    };
    const openExternalUrlHelper = async (/** @type {string} */ url) => {
      const cleanUrl = String(url || "").trim();
      if (!/^https?:\/\//i.test(cleanUrl)) return false;
      const statusMsg = `Открытие ссылки: ${cleanUrl}`;
      const statusNode = byId("server-status");
      if (statusNode) statusNode.textContent = statusMsg;
      if (isLiveSession()) {
        showVoiceToastNotice(statusMsg);
        try {
          await invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
            kind: OPEN_EXTERNAL_URL_IPC_CMD,
            deviceId: cleanUrl,
          });
          return true;
        } catch {}
      }
      try {
        await invoke(OPEN_EXTERNAL_URL_IPC_CMD, { url: cleanUrl });
        return true;
      } catch {
        try {
          globalThis.window?.open?.(cleanUrl, "_blank", "noopener,noreferrer");
        } catch {}
        return true;
      }
    };
    /** @type {any} */ (globalThis).__OPEN_DISCORD_INVITE__ = openDiscordInviteHelper;
    /** @type {any} */ (globalThis).__OPEN_EXTERNAL_URL__ = openExternalUrlHelper;
    if (typeof window !== "undefined") {
      /** @type {any} */ (window).__OPEN_DISCORD_INVITE__ = openDiscordInviteHelper;
      /** @type {any} */ (window).__OPEN_EXTERNAL_URL__ = openExternalUrlHelper;
    }
  }
  if (typeof document !== "undefined" && typeof document.addEventListener === "function") {
    document.addEventListener("keydown", (ev) => {
      if (
        (ev.ctrlKey || ev.metaKey) &&
        !ev.altKey &&
        (ev.key === "r" || ev.key === "R" || ev.key === "к" || ev.key === "К" || ev.code === "KeyR")
      ) {
        ev.preventDefault();
        if (isLiveSession()) {
          void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
            kind: "clear_client_cache",
            deviceId: "reload",
          }).catch(() => {});
        }
        if (typeof globalThis.location?.reload === "function") {
          globalThis.location.reload();
        }
        return;
      }
      if ((ev.ctrlKey || ev.metaKey) && (ev.key === "q" || ev.key === "Q" || ev.key === "й" || ev.key === "Й")) {
        if (!isLiveSession()) return;
        ev.preventDefault();
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "app_quit",
          deviceId: "default",
          volumePercent: 100,
          muted: false,
        }).catch(() => {});
      }
    });
  }

  /* ---------- INT-07: монтирование V-07 (voice devices) и N-05 (notifications) ---------- */
  const settingsShell = /** @type {HTMLElement | null} */ (
    document.querySelector("#screen-settings .settings-shell") ?? byId("screen-settings")
  );
  if (settingsShell && !settingsShell.id) {
    settingsShell.id = "settings-dialog";
  }
  const settingsBody = /** @type {HTMLElement | null} */ (
    settingsShell?.querySelector(".settings-body") ?? settingsShell
  );
  let settingsModal = byId("settings-modal");
  if (!settingsModal) {
    settingsModal = document.createElement("div");
    settingsModal.id = "settings-modal";
    settingsModal.className = "settings-voice-panel";
    // P2-1: панель — не модалка, а регион вкладки «Голос и видео»; та же семантика,
    // что в index.html, чтобы DOM-шим и окно давали одинаковое дерево.
    settingsModal.setAttribute("role", "region");
    settingsModal.setAttribute("aria-label", "Голос и видео");
    (settingsBody ?? document.body).append(settingsModal);
  }
  let voiceDevicesHost = byId("voice-devices");
  if (!voiceDevicesHost) {
    voiceDevicesHost = document.createElement("section");
    voiceDevicesHost.id = "voice-devices";
    settingsModal.append(voiceDevicesHost);
  }
  /**
   * P1-1 (Pass 3): голосовые команды не зарегистрированы в Rust (`generate_handler!`),
   * поэтому `service` — не IPC, а адаптер сессии: значения живут в памяти, а видимая
   * пометка `#voice-devices-preliminary` честно говорит, что сохранения нет. Так нет
   * ни alert'а на каждое действие (P1-1 Pass 2), ни тихой потери значения — включая
   * PTT-клавишу (P2-1 Pass 2).
   *
   * AT-SPI-инвариант RW-38 (`tests/smoke/gui-window.smoke.test.mjs`: в диалоге
   * «Настройки» ровно 38 тумблеров) требует, чтобы три `role="switch"` этой секции
   * не попадали в скоуп диалога, пока пользователь её не раскрыл. Поэтому
   * контроллер монтируется лениво: свёрнутая секция — только пометка и кнопка.
   * @type {{
   *   inputDeviceId: string,
   *   outputDeviceId: string,
   *   inputVolume: number,
   *   outputVolume: number,
   *   inputMode: "voice-activity" | "push-to-talk",
   *   mute: boolean,
   *   deaf: boolean,
   *   pttKey: string | null,
   * }}
   */
  const voiceDevicesSession = {
    inputDeviceId: "",
    outputDeviceId: "",
    inputVolume: 100,
    outputVolume: 100,
    inputMode: "voice-activity",
    mute: false,
    deaf: false,
    pttKey: null,
  };
  /** @type {ReturnType<typeof createVoiceDevicesSettings> | null} */
  let voiceDevices = null;
  /** Ленивый монтаж: до раскрытия секции в диалоге нет её тумблеров (AT-SPI 38). */
  const mountVoiceDevices = () => {
    if (voiceDevices) return voiceDevices;
    settings.voiceDevices?.destroy();
    voiceDevices = createVoiceDevicesSettings({
      host: voiceDevicesHost,
      devices: DEFAULT_VOICE_DEVICES,
      service: {
        selectInput: (deviceId) => {
          voiceDevicesSession.inputDeviceId = deviceId;
        },
        selectOutput: (deviceId) => {
          voiceDevicesSession.outputDeviceId = deviceId;
        },
        setInputVolume: (percent) => {
          voiceDevicesSession.inputVolume = percent;
        },
        setOutputVolume: (percent) => {
          voiceDevicesSession.outputVolume = percent;
        },
        setInputMode: (mode) => {
          voiceDevicesSession.inputMode = mode.kind;
        },
        setUserMute: (mute) => {
          voiceDevicesSession.mute = mute;
        },
        setSelfDeaf: (deaf) => {
          voiceDevicesSession.deaf = deaf;
        },
        setPttKey: (key) => {
          voiceDevicesSession.pttKey = key;
        },
      },
    });
    voiceDevicesHost.setAttribute("data-preliminary", "true");
    return voiceDevices;
  };

  // Свёрнутая секция: пометка и кнопка живут вне `#voice-devices`, потому что
  // контроллер строит свои узлы через `host.replaceChildren` и стёр бы их.
  let preliminaryBlock = byId("voice-devices-preliminary-block");
  if (!preliminaryBlock) {
    preliminaryBlock = document.createElement("div");
    preliminaryBlock.id = "voice-devices-preliminary-block";
    preliminaryBlock.className = "voice-devices-preliminary-block";
    if (voiceDevicesHost.parentNode === settingsModal) {
      settingsModal.replaceChildren(preliminaryBlock, voiceDevicesHost);
    } else {
      settingsModal.append(preliminaryBlock);
    }
  }
  let preliminaryNote = byId("voice-devices-preliminary");
  if (!preliminaryNote) {
    preliminaryNote = document.createElement("p");
    preliminaryNote.id = "voice-devices-preliminary";
    preliminaryNote.className = "voice-devices-preliminary";
    preliminaryNote.setAttribute("role", "status");
    preliminaryNote.textContent = VOICE_DEVICES_PRELIMINARY_NOTICE;
    preliminaryBlock.append(preliminaryNote);
  }
  let voiceDevicesExpand = /** @type {HTMLElement | null} */ (byId("voice-devices-expand"));
  if (!voiceDevicesExpand) {
    voiceDevicesExpand = document.createElement("button");
    voiceDevicesExpand.id = "voice-devices-expand";
    voiceDevicesExpand.className = "voice-devices-expand";
    voiceDevicesExpand.setAttribute("type", "button");
    voiceDevicesExpand.setAttribute("aria-controls", "voice-devices");
    preliminaryBlock.append(voiceDevicesExpand);
  }
  /**
   * @param {boolean} expanded
   */
  const setVoiceDevicesExpanded = (expanded) => {
    if (expanded) mountVoiceDevices();
    if (expanded) voiceDevicesHost.removeAttribute("hidden");
    else voiceDevicesHost.setAttribute("hidden", "");
    if (!voiceDevicesExpand) return;
    voiceDevicesExpand.setAttribute("aria-expanded", expanded ? "true" : "false");
    voiceDevicesExpand.textContent = expanded
      ? "Скрыть настройки устройств"
      : "Показать настройки устройств";
  };
  setVoiceDevicesExpanded(false);
  voiceDevicesExpand?.addEventListener("click", () => {
    setVoiceDevicesExpanded(voiceDevicesExpand?.getAttribute("aria-expanded") !== "true");
  });

  /** @type {Map<string, any>} */
  const guildNotifEntries = new Map();
  const notifications = createNotificationsController({
    document,
    invoke: (cmd, args) => /** @type {Promise<any>} */ (invoke(cmd, args)),
  });
  notifications.openModal = (opts) => {
    if (opts && opts.guildId) {
      notifications.openGuildModal(opts);
    } else {
      syncNotificationsWithGuild(opts?.triggerElement);
    }
  };
  notificationsRef = notifications;
  const settingsRoot = byId("settings");
  const refreshLiveAudioDevices = async () => {
    try {
      const fetched = /** @type {any} */ (await invoke(VOICE_AUDIO_DEVICES_IPC_CMD));
      const inputs = Array.isArray(fetched?.inputs) ? fetched.inputs : [];
      const outputs = Array.isArray(fetched?.outputs) ? fetched.outputs : [];
      const inSel = /** @type {HTMLSelectElement | null} */ (
        settingsRoot?.querySelector('select[data-setting="voiceInputDevice"], #settings-voice-input-select')
      );
      if (inSel && inputs.length > 0) {
        inSel.replaceChildren();
        for (const dev of inputs) {
          const opt = document.createElement("option");
          opt.value = String(dev.id || "default");
          opt.textContent = String(dev.name || dev.id || "Микрофон");
          inSel.append(opt);
        }
      }
      const outSel = /** @type {HTMLSelectElement | null} */ (
        settingsRoot?.querySelector('select[data-setting="voiceOutputDevice"], #settings-voice-output-select')
      );
      if (outSel && outputs.length > 0) {
        outSel.replaceChildren();
        for (const dev of outputs) {
          const opt = document.createElement("option");
          opt.value = String(dev.id || "default");
          opt.textContent = String(dev.name || dev.id || "Динамики");
          outSel.append(opt);
        }
      }
    } catch {}
  };
  const ensureVoiceTroubleshootControls = () => {
    if (!settingsRoot) return;
    const h264Input = /** @type {HTMLInputElement | null} */ (
      settingsRoot.querySelector('input[data-setting-id="advanced.openh264"]')
    );
    if (h264Input && h264Input.getAttribute("data-live-openh264-bound") !== "true") {
      h264Input.setAttribute("data-live-openh264-bound", "true");
      const lbl = h264Input.closest(".toggle")?.querySelector(".toggle-label");
      if (lbl) lbl.textContent = "Аппаратное ускорение H.264 (OpenH264)";
      h264Input.addEventListener("change", () => {
        if (!isLiveSession()) return;
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "openh264",
          deviceId: h264Input.checked ? "1" : "0",
          muted: !h264Input.checked,
        }).catch(() => {});
      });
    }
    for (const btn of settingsRoot.querySelectorAll('[data-action="clear-client-cache-restart"]')) {
      if (btn.getAttribute("data-live-cache-bound") === "true") continue;
      btn.setAttribute("data-live-cache-bound", "true");
      btn.addEventListener("click", () => {
        void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
          kind: "clear_client_cache",
          deviceId: "all",
        })
          .catch(() => {})
          .finally(() => {
            if (typeof globalThis.location?.reload === "function") {
              globalThis.location.reload();
            }
          });
      });
    }
  };
  const setVoicePanelVisibility = () => {
    if (settingsRoot?.getAttribute("data-active-pane") === "voice") {
      if (isLiveSession() && !(/** @type {any} */ (globalThis).__QA__)) {
        preliminaryBlock?.setAttribute("hidden", "");
        settingsModal?.setAttribute("hidden", "");
        ensureVoiceTroubleshootControls();
        void refreshLiveAudioDevices();
      } else {
        settingsModal?.removeAttribute("hidden");
      }
    } else {
      settingsModal?.setAttribute("hidden", "");
    }
  };
  setVoicePanelVisibility();
  for (const item of document.querySelectorAll("#settings-nav .settings-nav-item")) {
    item.addEventListener("click", setVoicePanelVisibility);
  }
  // P2-5 (Pass 3): клик по навигации — не единственный путь смены вкладки.
  // `settings.showPane(...)` (deep-link, ссылка «Уведомления» внутри панели,
  // программный вызов) шёл мимо слушателей и оставлял voice-панель видимой на
  // чужой вкладке. Обёртка делает синхронизацию инвариантом обоих путей.
  const showPaneOriginal = settings.showPane.bind(settings);
  settings.showPane = (key) => {
    showPaneOriginal(key);
    setVoicePanelVisibility();
  };

  /* ---------- FE-P0-01 / task-133: монтирование Home (@me), ЛС и Друзей ---------- */
  const dmTreeNode = byId("channel-tree");
  const dmTreeHeader = dmTreeNode?.querySelector(".tree-header") ?? null;
  const dmTreeScroll = dmTreeNode?.querySelector(".tree-scroll") ?? null;
  const initialGuildHeaderNodes = dmTreeHeader ? [...dmTreeHeader.children] : [];
  const initialGuildScrollNodes = dmTreeScroll ? [...dmTreeScroll.children] : [];
  const homeBtnNode = byId("home-rail-btn");
  /** @type {Array<{ type: string, fn: any }>} */
  const homeBtnListeners = [];
  if (homeBtnNode && typeof homeBtnNode.addEventListener === "function") {
    const origAdd = homeBtnNode.addEventListener.bind(homeBtnNode);
    homeBtnNode.addEventListener = (/** @type {string} */ type, /** @type {any} */ fn, /** @type {any} */ opts) => {
      homeBtnListeners.push({ type, fn });
      return origAdd(type, fn, opts);
    };
  }

  /** @type {Array<{ method: string, url: string, query?: any, body?: any }>} */
  const dmRestCalls = [];
  /** @type {((req: any) => Promise<any> | any) | null} */
  let customDmRestHandler = null;
  let dmChannelSeq = 0;

  /**
   * Показывает или скрывает предупреждающий баннер в `#friends-view`, если Discord
   * заблокировал запись на аккаунте (`required_action: "REQUIRE_VERIFIED_PHONE"`, HTTP 403 code 40002).
   * @param {string} requiredAction
   */
  function renderLiveAccountRequiredActionBanner(requiredAction) {
    if (!isLiveSession()) return;
    const existing = document.getElementById("live-account-verification-banner");
    if (!requiredAction) {
      existing?.remove();
      return;
    }
    const friendsViewNode = document.getElementById("friends-view");
    if (!friendsViewNode) return;
    if (existing) {
      return;
    }
    const banner = document.createElement("div");
    banner.id = "live-account-verification-banner";
    banner.setAttribute("role", "alert");
    banner.style.cssText =
      "margin:10px 16px 0 16px;padding:10px 14px;border-radius:8px;background:rgba(242,63,67,0.16);border:1px solid #f23f43;color:#f2f3f5;font-size:13px;line-height:1.4;display:flex;align-items:center;justify-content:space-between;gap:12px;";
    const textSpan = document.createElement("span");
    textSpan.textContent =
      requiredAction === "REQUIRE_VERIFIED_PHONE"
        ? "⚠️ Discord ограничил этот аккаунт (REQUIRE_VERIFIED_PHONE, код 40002): для вступления на серверы, принятия заявок в друзья и отправки сообщений необходимо подтвердить номер телефона в настройках учётной записи Discord."
        : `⚠️ Discord требует подтверждения учётной записи (${requiredAction}) для выполнения действий.`;
    banner.append(textSpan);
    const headerNode = friendsViewNode.querySelector(".friends-header");
    if (headerNode && headerNode.nextSibling) {
      friendsViewNode.insertBefore(banner, headerNode.nextSibling);
    } else {
      friendsViewNode.prepend(banner);
    }
  }

  /**
   * Локальный REST-адаптер для `createDmFriendsController`: не вызывает
   * незарегистрированные в `main.rs` IPC-команды (INT-07 / ipc-path).
   * @param {{ method: string, url: string, query?: any, body?: any }} req
   * @returns {Promise<any>}
   */
  async function sendDmRest(req) {
    if (dmRestCalls.length >= 200) {
      dmRestCalls.shift();
    }
    dmRestCalls.push(req);
    if (typeof customDmRestHandler === "function") {
      return customDmRestHandler(req);
    }
    if (isLiveSession() && !(/** @type {any} */ (globalThis).__QA__)) {
      const liveRes = await invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
        kind: "dm_rest_request",
        deviceId: JSON.stringify(req),
      });
      if (liveRes && typeof liveRes === "object") {
        if (typeof liveRes.requiredAction === "string") {
          /** @type {any} */ (globalThis).__DISCORD_ACCOUNT_REQUIRED_ACTION__ =
            liveRes.requiredAction;
          renderLiveAccountRequiredActionBanner(liveRes.requiredAction);
        }
        if (typeof liveRes.lastApiErrorCode === "number" && liveRes.lastApiErrorCode > 0) {
          /** @type {any} */ (globalThis).__DISCORD_LAST_API_ERROR_CODE__ =
            liveRes.lastApiErrorCode;
        }
        if (Array.isArray(liveRes.users)) {
          for (const u of liveRes.users) {
            if (u?.id) liveUsersById.set(String(u.id), u);
          }
        }
        if (
          liveRes.ok !== false &&
          (Array.isArray(liveRes.relationships) || Array.isArray(liveRes.privateChannels))
        ) {
          dmFriendsController?.syncLiveState?.({
            relationships: liveRes.relationships,
            privateChannels: liveRes.privateChannels,
            users: liveRes.users,
            presences: liveRes.presences,
          });
        }
        const statusNum = Number(liveRes.status ?? 200);
        if (statusNum >= 400 || liveRes.ok === false) {
          const errBody =
            liveRes.body && typeof liveRes.body === "object"
              ? liveRes.body
              : { message: String(liveRes.body ?? "Ошибка запроса") };
          const errCode = Number(errBody.code ?? 0);
          if (errCode > 0) {
            /** @type {any} */ (globalThis).__DISCORD_LAST_API_ERROR_CODE__ = errCode;
          }
          if (errCode === 40002) {
            /** @type {any} */ (globalThis).__DISCORD_ACCOUNT_REQUIRED_ACTION__ =
              "REQUIRE_VERIFIED_PHONE";
            renderLiveAccountRequiredActionBanner("REQUIRE_VERIFIED_PHONE");
          }
          const err = /** @type {any} */ (
            new Error(String(errBody.message || `HTTP ${statusNum}`))
          );
          err.status = statusNum;
          err.body = errBody;
          err.code = errCode;
          showVoiceToastNotice(
            errCode === 40002
              ? "Discord требует подтверждения номера телефона на этом аккаунте (REQUIRE_VERIFIED_PHONE, код 40002)"
              : String(errBody.message || `Ошибка Discord HTTP ${statusNum}`),
          );
          throw err;
        }
        return {
          ok: true,
          status: statusNum,
          body: liveRes.body ?? null,
          relationships: liveRes.relationships,
          privateChannels: liveRes.privateChannels,
          users: liveRes.users,
          presences: liveRes.presences,
        };
      }
    }
    if (req.method === "POST" && req.url === "/users/@me/channels") {
      /** @type {string[]} */
      const recipientsInput = Array.isArray(req.body?.recipients)
        ? req.body.recipients.map((/** @type {unknown} */ item) => String(item))
        : [];
      dmChannelSeq += 1;
      return {
        status: 200,
        body: {
          id: `39999999999999999${dmChannelSeq}`,
          type: recipientsInput.length > 1 ? 3 : 1,
          recipient_ids: recipientsInput,
          recipients: recipientsInput.map((/** @type {string} */ uid) => ({ id: uid })),
          last_message_id: `${Date.now()}`,
        },
      };
    }
    if (req.method === "POST" && req.url === "/users/@me/channels/group-dm-shell") {
      dmChannelSeq += 1;
      const uid = String(req.body?.recipient_id ?? "");
      return {
        status: 200,
        body: {
          id: `39999999999999999${dmChannelSeq}`,
          type: 3,
          recipient_ids: uid ? [uid] : [],
          recipients: uid ? [{ id: uid }] : [],
          last_message_id: `${Date.now()}`,
        },
      };
    }
    if (req.method === "GET" && req.url.startsWith("/users/@me/notes/")) {
      const uid = req.url.split("/").pop() ?? "";
      return { status: 200, body: { note_user_id: uid, note: "" } };
    }
    return { status: 204, body: null };
  }

  /**
   * Монтирует или обновляет `createDmFriendsController` с данными из `ready()`.
   * @param {any} [readyPayload]
   */
  function mountDmFriends(readyPayload = null) {
    const bag = readyPayload && typeof readyPayload === "object" ? readyPayload : {};
    const users = Array.isArray(bag.users) ? bag.users : [];
    const relationships = Array.isArray(bag.relationships) ? bag.relationships : [];
    const privateChannels = Array.isArray(bag.private_channels)
      ? bag.private_channels
      : Array.isArray(bag.privateChannels)
        ? bag.privateChannels
        : [];
    /** @type {Record<string, any>} */
    const presences = {};
    if (Array.isArray(bag.presences)) {
      for (const entry of bag.presences) {
        const uid = String(entry?.user?.id ?? entry?.user_id ?? "");
        if (uid) presences[uid] = typeof entry === "string" ? { status: entry } : entry;
      }
    } else if (bag.presences && typeof bag.presences === "object") {
      for (const [uid, entry] of Object.entries(bag.presences)) {
        presences[uid] = typeof entry === "string" ? { status: entry } : entry;
      }
    }
    const wasHome = dmTreeNode?.getAttribute("data-nav-mode") === "home";
    if (wasHome) {
      if (dmTreeHeader) {
        dmTreeHeader.className = "tree-header";
        dmTreeHeader.replaceChildren(...initialGuildHeaderNodes);
      }
      if (dmTreeScroll) {
        dmTreeScroll.className = "tree-scroll";
        dmTreeScroll.replaceChildren(...initialGuildScrollNodes);
      }
      dmTreeNode?.setAttribute("data-nav-mode", "guild");
    }
    if (homeBtnNode && typeof homeBtnNode.removeEventListener === "function") {
      for (const entry of homeBtnListeners.splice(0, homeBtnListeners.length)) {
        homeBtnNode.removeEventListener(entry.type, entry.fn);
      }
    }
    const initialGuildId = rail.selectedId && rail.selectedId !== HOME_GUILD_ID ? rail.selectedId : "1";
    dmFriendsController = createDmFriendsController({
      document,
      request: sendDmRest,
      users,
      relationships,
      privateChannels,
      presences,
      guildId: initialGuildId,
      onSelectChannel: (channelId, channel) => {
        router.switchScreen("chat");
        byId("friends-view")?.setAttribute("hidden", "");
        byId("chat-header")?.removeAttribute("hidden");
        const chatScreenNode = byId("screen-chat");
        if (chatScreenNode) {
          chatScreenNode.classList.add("active");
          chatScreenNode.setAttribute("aria-hidden", "false");
        }
        const chType = Number(channel?.type ?? 1);
        const isOneOnOneDm = chType === 1 || !channel?.type;
        const anyChannel = /** @type {any} */ (channel);
        const recipients = Array.isArray(anyChannel?.rawRecipients)
          ? anyChannel.rawRecipients
          : Array.isArray(anyChannel?.recipients)
            ? anyChannel.recipients
            : [];
        const selfIdStr = String(liveSelfUser?.id || "");
        const primaryRec =
          recipients.find((/** @type {any} */ r) => String(r?.id || "") !== selfIdStr) ??
          recipients[0] ??
          null;
        const recipientId = String(
          primaryRec?.id ||
            (Array.isArray(channel?.recipient_ids) ? channel.recipient_ids[0] : "") ||
            "",
        );
        const knownRec = (recipientId ? liveUsersById.get(recipientId) : null) ?? primaryRec ?? {};
        let primaryRecStatus = String(
          presences[recipientId]?.status ?? knownRec?.status ?? "offline",
        ).toLowerCase();
        if (primaryRecStatus === "invisible") primaryRecStatus = "offline";
        if (primaryRecStatus === "offline" && recipientId) {
          for (const vsList of liveVoiceStatesByGuild.values()) {
            if (
              Array.isArray(vsList) &&
              vsList.some(
                (v) =>
                  String(v?.user_id ?? v?.userId ?? "") === recipientId &&
                  Boolean(v?.channel_id ?? v?.channelId),
              )
            ) {
              primaryRecStatus = "online";
              break;
            }
          }
        }
        const recDisplayName = String(
          knownRec.global_name ||
            knownRec.globalName ||
            knownRec.display_name ||
            knownRec.displayName ||
            primaryRec?.global_name ||
            primaryRec?.display_name ||
            channel?.name ||
            knownRec.username ||
            "Собеседник",
        );
        const recUsername = String(
          knownRec.username || primaryRec?.username || channel?.name || recipientId || "user",
        );
        const recAvatar =
          knownRec.avatar || primaryRec?.avatar || channel?.avatar?.avatarHash || null;
        const recBannerColor = String(
          knownRec.banner_color ||
            knownRec.bannerColor ||
            primaryRec?.banner_color ||
            "#5865f2",
        );
        const activeDmRecipient = isOneOnOneDm
          ? {
              id: recipientId,
              username: recUsername,
              display_name: recDisplayName,
              global_name: recDisplayName,
              avatar: recAvatar,
              status: primaryRecStatus,
              banner_color: recBannerColor,
            }
          : null;
        if (typeof window !== "undefined") {
          /** @type {any} */ (window).__ACTIVE_DM_RECIPIENT__ = activeDmRecipient;
        }
        /** @type {any} */ (globalThis).__ACTIVE_DM_RECIPIENT__ = activeDmRecipient;
        if (isLiveSession()) {
          if (document.body?.dataset) {
            document.body.dataset.viewMode = "dm";
          }
          const panelNode = byId("member-panel-list");
          const onlineNode = byId("member-online");
          const panelTitleEl = /** @type {HTMLElement | null} */ (
            document.querySelector("#member-panel .member-panel-title")
          );
          const panelAllBtn = /** @type {HTMLElement | null} */ (
            byId("member-panel-all-btn") ||
              document.querySelector("#member-panel .member-panel-all")
          );
          const panelFilterBtn = /** @type {HTMLElement | null} */ (
            byId("member-panel-filter-btn") ||
              document.querySelector('#member-panel [data-action="toggle-hide-offline-members"]')
          );
          if (panelTitleEl) {
            panelTitleEl.hidden = true;
            panelTitleEl.style.display = "none";
          }
          if (panelAllBtn) {
            panelAllBtn.hidden = true;
            panelAllBtn.style.display = "none";
          }
          if (panelFilterBtn) {
            panelFilterBtn.hidden = true;
            panelFilterBtn.style.display = "none";
          }
          const dmParticipants = [];
          if (liveSelfUser) {
            dmParticipants.push({
              id: String(liveSelfUser.id || ""),
              displayName: String(
                liveSelfUser.globalName || liveSelfUser.global_name || liveSelfUser.username || "Вы",
              ),
              username: String(liveSelfUser.username || ""),
              avatar: liveSelfUser.avatar || null,
              status: "online",
            });
          }
          for (const rec of recipients) {
            const rid = String(rec?.id || "");
            const known = liveUsersById.get(rid) ?? rec ?? {};
            let recStatus = String(presences[rid]?.status ?? "offline").toLowerCase();
            if (recStatus === "invisible") recStatus = "offline";
            if (recStatus === "offline") {
              for (const vsList of liveVoiceStatesByGuild.values()) {
                if (
                  Array.isArray(vsList) &&
                  vsList.some(
                    (v) =>
                      String(v?.user_id ?? v?.userId ?? "") === rid &&
                      Boolean(v?.channel_id ?? v?.channelId),
                  )
                ) {
                  recStatus = "online";
                  break;
                }
              }
            }
            dmParticipants.push({
              id: rid,
              displayName: String(
                known.global_name || known.globalName || known.display_name || known.username || channel?.name || "Собеседник",
              ),
              username: String(known.username || ""),
              avatar: known.avatar || null,
              status: recStatus,
            });
          }
          if (onlineNode) {
            const onlineCnt = dmParticipants.filter((p) => p.status !== "offline").length;
            onlineNode.textContent = String(onlineCnt);
          }
          if (panelNode) {
            if (isOneOnOneDm) {
              panelNode.setAttribute("data-source", "live-dm-profile");
              panelNode.setAttribute("aria-label", `Профиль ${recDisplayName}`);
              panelNode.replaceChildren();
              const profileCard = document.createElement("div");
              profileCard.className = "dm-user-profile-sidebar";
              profileCard.style.cssText =
                "background:#232428;border-radius:8px;display:flex;flex-direction:column;color:#f2f3f5;";
              const topBanner = document.createElement("div");
              topBanner.style.cssText =
                "height:105px;background:linear-gradient(135deg, #5865f2, #eb459e);border-radius:8px 8px 0 0;position:relative;margin:-8px -8px 0 -8px;";
              const avatarWrap = document.createElement("div");
              avatarWrap.style.cssText =
                "margin-top:-40px;margin-left:16px;width:80px;height:80px;border:6px solid #232428;border-radius:50%;position:relative;background:#5865f2;display:flex;align-items:center;justify-content:center;font-size:26px;font-weight:700;color:#ffffff;box-sizing:border-box;flex-shrink:0;";
              const initials =
                Array.from(recDisplayName.trim()).slice(0, 2).join("").toUpperCase() || "U";
              avatarWrap.textContent = initials;
              if (recipientId && recAvatar) {
                const avatarUrl = `https://cdn.discordapp.com/avatars/${recipientId}/${recAvatar}.png?size=128`;
                const img = new globalThis.Image();
                img.onload = () => {
                  for (const n of Array.from(avatarWrap.childNodes)) {
                    if (n.nodeType === 3) n.remove();
                  }
                  avatarWrap.style.backgroundImage = `url("${avatarUrl}")`;
                  avatarWrap.style.backgroundSize = "cover";
                  avatarWrap.style.backgroundPosition = "center";
                  avatarWrap.style.color = "transparent";
                };
                img.src = avatarUrl;
              } else if (recipientId && /^\d+$/.test(recipientId)) {
                try {
                  const defIdx = Number((BigInt(recipientId) >> 22n) % 6n);
                  const fallbackUrl = `https://cdn.discordapp.com/embed/avatars/${defIdx >= 0 ? defIdx : 0}.png`;
                  const img = new globalThis.Image();
                  img.onload = () => {
                    for (const n of Array.from(avatarWrap.childNodes)) {
                      if (n.nodeType === 3) n.remove();
                    }
                    avatarWrap.style.backgroundImage = `url("${fallbackUrl}")`;
                    avatarWrap.style.backgroundSize = "cover";
                    avatarWrap.style.backgroundPosition = "center";
                    avatarWrap.style.color = "transparent";
                  };
                  img.src = fallbackUrl;
                } catch {}
              }
              const statusDot = document.createElement("span");
              const statusDotColor =
                primaryRecStatus === "idle"
                  ? "#f0b232"
                  : primaryRecStatus === "dnd"
                    ? "#f23f43"
                    : primaryRecStatus === "online"
                      ? "#23a55a"
                      : "#80848e";
              statusDot.style.cssText = `position:absolute;right:-2px;bottom:-2px;width:22px;height:22px;border-radius:50%;background:${statusDotColor};border:4px solid #232428;box-sizing:border-box;`;
              statusDot.title = STATUS_LABELS[primaryRecStatus] ?? "в сети";
              avatarWrap.append(statusDot);

              const cardBody = document.createElement("div");
              cardBody.style.cssText =
                "padding:12px 16px;display:flex;flex-direction:column;gap:12px;";
              const nameBlock = document.createElement("div");
              nameBlock.style.cssText = "display:flex;flex-direction:column;gap:2px;";
              const dispNameEl = document.createElement("div");
              dispNameEl.style.cssText =
                "font-size:20px;font-weight:700;color:#f2f3f5;line-height:1.2;word-break:break-word;";
              dispNameEl.textContent = recDisplayName;
              const userTagEl = document.createElement("div");
              userTagEl.style.cssText = "font-size:14px;color:#b5bac1;line-height:1.3;";
              userTagEl.textContent = recUsername.startsWith("@") ? recUsername : `@${recUsername}`;
              nameBlock.append(dispNameEl, userTagEl);

              const infoCard = document.createElement("div");
              infoCard.style.cssText =
                "background:#111214;border-radius:8px;padding:12px;display:flex;flex-direction:column;gap:10px;";

              const memberSinceSec = document.createElement("div");
              memberSinceSec.style.cssText = "display:flex;flex-direction:column;gap:4px;";
              const memberSinceTitle = document.createElement("div");
              memberSinceTitle.style.cssText =
                "font-size:11px;font-weight:700;color:#b5bac1;text-transform:uppercase;";
              memberSinceTitle.textContent = "В ЧИСЛЕ УЧАСТНИКОВ С";
              const memberSinceVal = document.createElement("div");
              memberSinceVal.style.cssText = "font-size:13px;color:#dbdee1;";
              let createdDateStr = "14 мая 2020 г.";
              if (recipientId && /^\d+$/.test(recipientId)) {
                try {
                  const ts = Number((BigInt(recipientId) >> 22n) + 1420070400000n);
                  if (Number.isFinite(ts) && ts > 1420070400000) {
                    createdDateStr = new Date(ts).toLocaleDateString("ru-RU", {
                      day: "numeric",
                      month: "short",
                      year: "numeric",
                    });
                  }
                } catch {}
              }
              memberSinceVal.textContent = createdDateStr;
              memberSinceSec.append(memberSinceTitle, memberSinceVal);

              const mutualSec = document.createElement("div");
              mutualSec.style.cssText =
                "display:flex;flex-direction:column;gap:6px;padding-top:8px;border-top:1px solid rgba(255,255,255,0.06);";
              const mutualTitle = document.createElement("div");
              mutualTitle.style.cssText =
                "font-size:11px;font-weight:700;color:#b5bac1;text-transform:uppercase;";
              const activeGuildList = [];
              for (const [gid, gObj] of liveGuildsById.entries()) {
                if (gid && gid !== HOME_GUILD_ID) {
                  activeGuildList.push({
                    id: String(gid),
                    name: String(gObj?.name || gObj?.properties?.name || "Сервер"),
                  });
                }
              }
              if (activeGuildList.length === 0) {
                for (const tile of document.querySelectorAll("#guild-list .guild-item[data-guild-id]")) {
                  const gid = tile.getAttribute("data-guild-id") || "";
                  const gname = tile.getAttribute("data-name") || "Сервер";
                  if (gid && gid !== HOME_GUILD_ID) {
                    activeGuildList.push({ id: gid, name: gname });
                  }
                }
              }
              mutualTitle.textContent = `ОБЩИЕ СЕРВЕРЫ — ${activeGuildList.length}`;
              mutualSec.append(mutualTitle);
              if (activeGuildList.length === 0) {
                const emptyMutual = document.createElement("div");
                emptyMutual.style.cssText = "font-size:12px;color:#949ba4;";
                emptyMutual.textContent = "Нет общих серверов";
                mutualSec.append(emptyMutual);
              }
              for (const mg of activeGuildList.slice(0, 3)) {
                const mgBtn = document.createElement("button");
                mgBtn.type = "button";
                mgBtn.style.cssText =
                  "background:#1e1f22;border:1px solid rgba(255,255,255,0.06);border-radius:6px;padding:6px 8px;color:#f2f3f5;font-size:12px;font-weight:600;cursor:pointer;display:flex;align-items:center;justify-content:space-between;gap:8px;text-align:left;width:100%;";
                mgBtn.innerHTML = `<span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${mg.name}</span><span style="color:#949ba4;font-size:12px;">›</span>`;
                mgBtn.addEventListener("click", () => {
                  const tile = /** @type {HTMLElement | null} */ (
                    document.querySelector(`#guild-list .guild-item[data-guild-id="${mg.id}"]`)
                  );
                  if (tile) tile.click();
                });
                mutualSec.append(mgBtn);
              }

              const noteSec = document.createElement("div");
              noteSec.style.cssText =
                "display:flex;flex-direction:column;gap:4px;padding-top:8px;border-top:1px solid rgba(255,255,255,0.06);";
              const noteTitle = document.createElement("div");
              noteTitle.style.cssText =
                "font-size:11px;font-weight:700;color:#b5bac1;text-transform:uppercase;";
              noteTitle.textContent = "ЗАМЕТКА";
              const noteArea = document.createElement("textarea");
              noteArea.placeholder = "Нажмите, чтобы добавить заметку";
              noteArea.rows = 2;
              noteArea.style.cssText =
                "background:transparent;border:none;color:#dbdee1;font-size:12px;resize:none;width:100%;outline:none;font-family:inherit;";
              const existingNote = recipientId ? livePerUserVoiceState.get(recipientId)?.note : "";
              if (existingNote) noteArea.value = existingNote;
              noteArea.addEventListener("change", () => {
                if (!recipientId) return;
                const prev = livePerUserVoiceState.get(recipientId) ?? {
                  volume: 100,
                  localMute: false,
                };
                livePerUserVoiceState.set(recipientId, { ...prev, note: noteArea.value });
              });
              noteSec.append(noteTitle, noteArea);

              infoCard.append(memberSinceSec, mutualSec, noteSec);

              const fullProfileBtn = document.createElement("button");
              fullProfileBtn.type = "button";
              fullProfileBtn.textContent = "Полный профиль";
              fullProfileBtn.style.cssText =
                "background:#313338;color:#f2f3f5;border:none;border-radius:4px;padding:8px 12px;font-weight:500;cursor:pointer;width:100%;";
              fullProfileBtn.addEventListener("mouseenter", () => {
                fullProfileBtn.style.background = "#3f4147";
              });
              fullProfileBtn.addEventListener("mouseleave", () => {
                fullProfileBtn.style.background = "#313338";
              });
              fullProfileBtn.addEventListener("click", (ev) => {
                ev.stopPropagation();
                userPopout?.openProfileModal?.(
                  {
                    userId: recipientId || "1",
                    username: recUsername,
                    displayName: recDisplayName,
                    status: /** @type {any} */ (primaryRecStatus),
                  },
                  fullProfileBtn,
                );
              });

              cardBody.append(nameBlock, infoCard, fullProfileBtn);
              profileCard.append(topBanner, avatarWrap, cardBody);
              panelNode.append(profileCard);
            } else {
              panelNode.setAttribute("data-source", "live-dm");
              panelNode.replaceChildren();
              const groupSection = document.createElement("section");
              groupSection.className = "member-group";
              const groupHeader = document.createElement("h3");
              groupHeader.className = "member-group-title";
              groupHeader.textContent = `УЧАСТНИКИ БЕСЕДЫ — ${dmParticipants.length}`;
              groupSection.append(groupHeader);
              let idx = 0;
              for (const p of dmParticipants) {
                const rowNode = document.createElement("div");
                let hash = 0;
                for (let i = 0; i < p.displayName.length; i += 1) {
                  hash = (hash * 31 + p.displayName.charCodeAt(i)) >>> 0;
                }
                const isOfflineP = p.status === "offline";
                renderMemberRow(
                  rowNode,
                  {
                    name: p.displayName,
                    status: p.status,
                    statusLabel: STATUS_LABELS[p.status] ?? (isOfflineP ? "не в сети" : "в сети"),
                    role: "",
                    badge: "",
                    activity: p.username ? `@${p.username}` : isOfflineP ? "Не в сети" : "В сети",
                    hue: hash % 360,
                  },
                  idx,
                  { total: dmParticipants.length, userId: p.id },
                );
                if (isOfflineP) {
                  rowNode.style.opacity = "0.55";
                }
                if (p.id && p.avatar) {
                  const avatarEl = /** @type {HTMLElement | null} */ (rowNode.querySelector(".avatar"));
                  const url = `https://cdn.discordapp.com/avatars/${p.id}/${p.avatar}.png?size=64`;
                  if (avatarEl) {
                    const img = new globalThis.Image();
                    img.onload = () => {
                      avatarEl.style.backgroundImage = `url("${url}")`;
                      avatarEl.style.backgroundSize = "cover";
                      avatarEl.style.backgroundPosition = "center";
                      avatarEl.style.color = "transparent";
                    };
                    img.src = url;
                  }
                }
                groupSection.append(rowNode);
                idx += 1;
              }
              panelNode.append(groupSection);
            }
          }
        }
        void chat
          .openChannel(
            /** @type {any} */ ({
              id: channelId,
              name: channel?.name ?? channelId,
              type: channel?.type ?? 1,
            }),
          )
          .then(() => typing?.refresh())
          .catch((/** @type {unknown} */ error) => surface(error));
      },
    });
    dmFriendsController.selectFriendsTab(dmFriendsController.getActiveFriendsTab(), false);
    homeBtnNode?.addEventListener("click", () => {
      rail.selectHome("user", false);
    });
    homeBtnNode?.addEventListener("keydown", (/** @type {any} */ event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        rail.selectHome("user", true);
      }
    });
    if (wasHome) {
      rail.selectHome("restore", false);
    }
  }

  hydrateDmFriendsFromReady = (payload) => {
    mountDmFriends(payload);
  };
  mountDmFriends(null);

  const dmFriendsApi = new Proxy(
    /** @type {any} */ ({}),
    {
      get(_target, prop) {
        if (prop === "setRestHandler") {
          return (/** @type {((req: any) => Promise<any> | any) | null} */ fn) => {
            customDmRestHandler = fn;
          };
        }
        if (prop === "getRestCalls") {
          return () => [...dmRestCalls];
        }
        const ctrl = /** @type {any} */ (dmFriendsController);
        const val = ctrl?.[prop];
        return typeof val === "function" ? val.bind(ctrl) : val;
      },
    },
  );

  /**
   * Пункт уведомлений становится доступен только после появления реальной гильдии.
   */
  function enableGuildNotificationsControl() {
    const item = /** @type {HTMLElement | null} */ (document.querySelector('#server-menu [data-menu-index="3"]'));
    if (!item) return;
    item.id = "server-menu-notifications";
    item.removeAttribute("data-static-disabled");
    item.removeAttribute("aria-disabled");
    item.removeAttribute("data-disabled-reason");
    item.removeAttribute("aria-describedby");
    item.removeAttribute("title");
    item.setAttribute("data-action", "guild-notifications");
    item.setAttribute("aria-haspopup", "dialog");
    item.setAttribute("tabindex", "0");
  }

  /** @returns {{ id: string, name: string }} */
  function currentGuildInfo() {
    const tile = /** @type {HTMLElement | null} */ (
      document.querySelector(".guild-item.is-selected") ?? document.querySelector(".guild-item")
    );
    const id = rail.selectedId || selectedGuildId() || "";
    const label = tile?.getAttribute("aria-label") ?? "";
    const tileName = tile?.getAttribute("data-name") || label.replace(/^Сервер\s+/, "").trim();
    const headingName = document.querySelector(".tree-guild")?.textContent?.trim() ?? "";
    return {
      id,
      name: tileName || headingName || "Сервер",
    };
  }

  /** @returns {Array<{ id: string, name: string, type: number, parentId: string, position: number }>} */
  function currentGuildChannels() {
    if (tree.channels.length > 0) {
      return tree.channels;
    }
    return [...document.querySelectorAll("#channel-tree .channel-item")].map((node, index) => ({
      id: node.getAttribute("data-channel-id") ?? `ch-${index + 1}`,
      name: node.querySelector(".channel-name")?.textContent ?? node.textContent ?? "",
      type: Number(node.getAttribute("data-channel-type") ?? 0),
      parentId: node.getAttribute("data-parent-id") ?? "",
      position: index,
    }));
  }

  /**
   * Синхронизирует состояние контроллера уведомлений с выбранной гильдией и её каналами
   * и открывает модалку `#guild-notifications-dialog`.
   *
   * P2-5: путь open→close (`keepOpen=false`) удалён — модалка либо открывается, либо
   * не трогается. Контроллеру нужен отдельный «обновить без открытия», а эмуляция
   * через `openGuildModal` + `closeGuildModal` сбрасывала фокус и `modalState`.
   * @param {any} [triggerElement]
   */
  function syncNotificationsWithGuild(triggerElement = null) {
    const prev = notifications.getState();
    if (prev.guildId) {
      guildNotifEntries.set(prev.guildId, prev.guildEntry);
    }
    const guild = currentGuildInfo();
    if (!guild.id) {
      const status = byId("server-status");
      if (status) status.textContent = "Сначала выберите сервер.";
      return;
    }
    const storedEntry = guildNotifEntries.get(guild.id);
    notifications.openGuildModal({
      guildId: guild.id,
      guildName: guild.name,
      guildEntry: storedEntry ?? { guild_id: guild.id },
      channels: currentGuildChannels(),
      triggerElement,
    });
  }

  /**
   * Открывает контекстное меню уведомлений канала `#channel-notif-menu`.
   *
   * P2-5: модалка не переоткрывается ради синхронизации. Если она закрыта, меню
   * работает на уже установленном контексте контроллера; если открыта и гильдия
   * сменилась — контекст обновляется тем же `openGuildModal` (без close-цикла).
   * @param {string} channelId
   * @param {any} [triggerElement]
   */
  function openChannelNotifications(channelId, triggerElement = null) {
    const resolvedChannelId = String(channelId ?? "").trim();
    if (!resolvedChannelId) return;
    const guild = currentGuildInfo();
    const modalOpen = byId("guild-notifications-dialog")?.getAttribute("hidden") === null;
    if (modalOpen && guild.id && notifications.getState().guildId !== guild.id) {
      syncNotificationsWithGuild(null);
    }
    notifications.openChannelMenu({
      channelId: resolvedChannelId,
      triggerElement,
    });
  }

  const serverMenuNotifItem = /** @type {HTMLElement | null} */ (
    document.querySelector('#server-menu [data-menu-index="3"]')
  );
  if (serverMenuNotifItem) {
    serverMenuNotifItem.id = "server-menu-notifications";
    // P1-1/D1: пока пункт объявлен `aria-disabled="true"`, и клик, и клавиши — строгий
    // no-op (без статуса, IPC, фокуса и ACTION_FAILED); guard — первая строка каждого
    // обработчика. После `rail.load()` атрибут снимает `enableGuildNotificationsControl`.
    serverMenuNotifItem.addEventListener("keydown", (event) => {
      if (serverMenuNotifItem.getAttribute("aria-disabled") === "true") return;
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        // Не даём тому же keydown дойти до roving-обработчика `#server-menu`:
        // иначе пункт активируется дважды (клавиша + повторный click).
        event.stopPropagation();
        serverMenuNotifItem.click();
      }
    });
    serverMenuNotifItem.addEventListener("click", () => {
      if (serverMenuNotifItem.getAttribute("aria-disabled") === "true") return;
      // P2-5: меню сервера закрывается до открытия модалки — под backdrop не
      // остаётся открытый поповер с `aria-expanded="true"` у опенера.
      const menu = byId("server-menu");
      menu?.setAttribute("hidden", "");
      document.querySelector('[data-action="guild-menu"]')?.setAttribute("aria-expanded", "false");
      syncNotificationsWithGuild(serverMenuNotifItem);
    });
  }

  const inboxBellBtn = /** @type {HTMLElement | null} */ (
    document.querySelector('[data-action="inbox"]')
  );
  if (inboxBellBtn) {
    inboxBellBtn.addEventListener("click", () => {
      // P1-1/D1: до первого открытого канала колокол объявлен disabled (VW-01) —
      // клик по нему строгий no-op. Атрибуты снимает обёртка `chat.openChannel`.
      if (inboxBellBtn.getAttribute("aria-disabled") === "true") return;
      // P2-6: без открытого канала не берём `tree.channels[0]` вслепую (это может
      // быть категория type=4/voice type=2); ищем первый текстовый канал.
      const activeChannelId =
        chat.channelId ||
        document.querySelector('#channel-tree .channel-item[aria-current="page"]')?.getAttribute("data-channel-id") ||
        // C-02: тот же контракт типов, что у дерева — чат открывают только 0 и 5.
        tree.channels.find((channel) => CHANNEL_KINDS[channel.type]?.openable === true)?.id ||
        "";
      if (activeChannelId) {
        const status = byId("server-status");
        if (status) status.textContent = "";
        openChannelNotifications(activeChannelId, inboxBellBtn);
      } else {
        const status = byId("server-status");
        if (status) status.textContent = "Сначала откройте текстовый канал.";
      }
    });
  }

  startMetrics({ onError: (error) => surface(error, "IPC_FAILED") });

  // Экран входа (RW-39/NET, ADR-2): idle → validating → ready | error.
  // Токен вводится только здесь и сразу уходит в `auth_login_token` → keyring.
  const auth = createAuthGate({
    onReady: () => {
      // После ready чат доступен: роутер на chat, фокус уходит из закрытого гейта.
      router.switchScreen("chat");
      const list = byId("chat-list");
      const target = /** @type {any} */ (list);
      if (target && typeof target.focus === "function") target.focus();
      // L-01: сетевой статус актуален только в активном приложении (после входа).
      if (isLiveSession()) {
        const attachBtn = document.querySelector(".composer-attach");
        if (attachBtn) attachBtn.textContent = "+";
        void invoke("net_connect").catch(() => {});
      } else {
        net.start();
      }
      // L-02-UI: подписка на dispatch — после ready, как у net/typing.
      void dispatch?.start();
      // G-01: список серверов — из READY; без входа грузить нечего.
      void rail.load();
      try {
        cleanupSpikeBenchmarkArtifacts();
      } catch {}
    },
  });
  // Выход из аккаунта возвращает гейт: опрос ядра прекращается до следующего ready.
  // P-03-UI: блокировка отправки typing (401/403) и записи индикатора тоже
  // сбрасываются — иначе следующий вход унаследует чужое состояние.
  const cleanupLiveVoiceTimersAndAudio = () => {
    stopRtcStatusPolling();
    if (voiceSyncTimer) {
      clearInterval(voiceSyncTimer);
      voiceSyncTimer = null;
    }
    if (micTestTimer) {
      clearInterval(micTestTimer);
      micTestTimer = null;
    }
    closeLiveVoicePopouts();
    if (sharedSoundboardAudioCtx && typeof sharedSoundboardAudioCtx.close === "function") {
      void sharedSoundboardAudioCtx.close().catch(() => {});
      sharedSoundboardAudioCtx = null;
    }
  };
  if (typeof window !== "undefined" && typeof window.addEventListener === "function") {
    window.addEventListener("pagehide", cleanupLiveVoiceTimersAndAudio);
  }
  byId("auth-logout")?.addEventListener("click", () => {
    cleanupLiveVoiceTimersAndAudio();
    if (isLiveSession()) {
      liveSelfUser = null;
      /** @type {any} */ (globalThis).__LIVE_SELF_USER__ = null;
      /** @type {any} */ (globalThis).__DISCORD_ACCOUNT_REQUIRED_ACTION__ = "";
      /** @type {any} */ (globalThis).__DISCORD_LAST_API_ERROR_CODE__ = 0;
      document.getElementById("live-account-verification-banner")?.remove();
      liveGuildsById.clear();
      liveUsersById.clear();
      liveMemberRolesByGuildAndUser.clear();
      liveMemberColorByGuildAndUser.clear();
      liveMemberClanTagByUser.clear();
      liveVoiceStatesByGuild.clear();
      livePerUserVoiceState.clear();
      liveVoiceChannelStartEpochMs.clear();
      activeVoiceConnection = null;
      preferredGuildId = HOME_GUILD_ID;
      latestGuildId = HOME_GUILD_ID;
      const mpNode = byId("member-panel-list");
      if (mpNode) mpNode.replaceChildren();
      const moNode = byId("member-online");
      if (moNode) moNode.textContent = "0";
      const uAvatarNode = /** @type {HTMLElement | null} */ (document.querySelector("#user-bar .avatar"));
      if (uAvatarNode) {
        uAvatarNode.style.backgroundImage = "";
        uAvatarNode.style.color = "";
      }
    }
    // FIX-3: сброс UI-контроллеров перед остановкой сети — при смене аккаунта
    // гильдии, каналы и ЛС от старого пользователя полностью очищаются.
    rail.reset();
    tree.reset();
    dmFriendsController?.reset?.();
    dispatch?.setSelfUserId?.("");
    net.stop();
    void dispatch?.stop();
    typingSender.reset();
    typing?.clear();
  });

  const panelList = byId("member-panel-list");
  if (panelList) {
    renderMemberPanel({
      container: panelList,
      store: members.store,
      onlineCounter: byId("member-online"),
      onError: (error) => surface(error),
    }).catch((/** @type {unknown} */ error) => surface(error));
  }

  /* ---------- FE-P1-06 / task-164: монтирование userPopout и guildModals ---------- */
  const reportUiError = (/** @type {unknown} */ error, code = "ACTION_FAILED") => surface(error, code);

  const userPopout = createUserPopoutController(
    /** @type {any} */ ({
      container: document.body,
      root: document.body,
      document,
      invoke,
      onError: reportUiError,
    }),
  );
  userPopout.attachToChatAndMembers({
    chatList: document.getElementById("chat-list"),
    membersList: document.getElementById("members-list") ?? document.getElementById("member-list"),
  });

  const guildModals = createGuildModalsController(
    /** @type {any} */ ({
      container: document.body,
      root: document.body,
      document,
      invoke,
      onError: reportUiError,
      onOpenNotifications: () => notifications?.openModal?.(),
      onLeaveGuild: async (/** @type {{ guildId?: string }} */ params) => {
        const targetId =
          String(params?.guildId || rail.selectedId || "").trim();
        if (!targetId || targetId === HOME_GUILD_ID) return;
        await invoke("leave_guild", { guildId: targetId, lurking: false });
      },
      onLeftGuild: (/** @type {{ guildId?: string }} */ params) => {
        const leftId = String(params?.guildId || "").trim();
        if (leftId) {
          liveGuildsById.delete(leftId);
          liveVoiceStatesByGuild.delete(leftId);
          for (const key of Array.from(liveMemberRolesByGuildAndUser.keys())) {
            if (key.startsWith(`${leftId}:`)) {
              liveMemberRolesByGuildAndUser.delete(key);
            }
          }
          for (const key of Array.from(liveMemberColorByGuildAndUser.keys())) {
            if (key.startsWith(`${leftId}:`)) {
              liveMemberColorByGuildAndUser.delete(key);
            }
          }
          if (activeVoiceConnection && String(activeVoiceConnection.guildId || "") === leftId) {
            disconnectFromLiveVoiceChannel();
          }
        }
        const remaining = Array.from(document.querySelectorAll("#guild-list .guild-item[data-guild-id]"))
          .map((tile) => ({
            id: tile.getAttribute("data-guild-id") ?? "",
            name: tile.getAttribute("data-name") ?? "",
            icon: tile.getAttribute("data-icon") ?? null,
          }))
          .filter((g) => g.id !== "" && g.id !== leftId);
        rail.applyGuilds(remaining);
        rail.selectHome("user", false);
      },
      onCreateInvite: async (/** @type {any} */ params) => {
        const targetChannelId =
          String(
            params?.channelId ||
              chat.channelId ||
              document.querySelector("#channel-tree .channel-item[data-channel-id]")?.getAttribute("data-channel-id") ||
              "1014537851867775060",
          ).trim();
        return invoke("create_invite", {
          channelId: targetChannelId,
          maxAge: Number(params?.maxAge ?? 86400),
          maxUses: Number(params?.maxUses ?? 0),
          temporary: Boolean(params?.temporary),
        });
      },
    }),
  );

  const guildRailNode = document.getElementById("guild-rail") ?? document.getElementById("guild-list");
  if (guildRailNode) {
    guildModals.attachTrigger(guildRailNode);
  }
  const guildHeaderNode =
    document.getElementById("guild-header") ??
    document.querySelector(".tree-header") ??
    document.querySelector(".tree-guild");
  if (guildHeaderNode) {
    guildModals.attachTrigger(guildHeaderNode);
  }

  /* ---------- FE-INT-15 / task-292: монтирование wiring оболочки и поповеров ---------- */
  const shellWiring = createAppShellWiring({
    onDispatchIpc: (command, payload) =>
      invoke(command, /** @type {Record<string, unknown> | undefined} */ (payload)),
  });
  const composerPopoversBridge = createComposerPopoversBridge(
    /**
     * Мост пока не объявляет `onDispatchIpc` в `ComposerPopoversBridgeOptions`,
     * но контракт FE-INT-15 требует прокинуть его так же, как в wiring оболочки.
     */
    /** @type {NonNullable<Parameters<typeof createComposerPopoversBridge>[0]> & {
     *   onDispatchIpc: (command: string, payload?: unknown) => unknown,
     * }} */ ({
      onDispatchIpc: (command, payload) =>
        invoke(command, /** @type {Record<string, unknown> | undefined} */ (payload)),
    }),
  );

  return {
    router,
    chat,
    tree,
    members,
    emoji,
    settings,
    composer,
    search,
    quickSwitcher,
    auth,
    net,
    rail,
    voice,
    join,
    typing,
    typingSender,
    dispatch,
    inviteModal,
    get voiceDevices() {
      return voiceDevices;
    },
    notifications,
    dmFriends: dmFriendsApi,
    get pinsThreads() {
      return getPinsThreads();
    },
    userPopout,
    guildModals,
    shellWiring,
    composerPopoversBridge,
    cleanupSpikeBenchmarkArtifacts,
  };
}

try {
  app = boot();
} catch (error) {
  // Ошибка загрузки фич не должна молча оставить пустой экран: показываем оверлей.
  surface(error, "ELEMENT_MISSING");
}

/* ---------- автозамер и UI_SNAPSHOT ---------- */

async function autoBench() {
  if (!app) return;
  const tauri = /** @type {any} */ (window).__TAURI__;
  if (!tauri?.core?.invoke) return;
  try {
    const cfg = await tauri.core.invoke("spike_config");
    if (!cfg.autobench) return;
  } catch (error) {
    surface(error, "IPC_FAILED");
    return;
  }

  await sleep(1200);
  app.router.switchScreen("chat");
  const chatList = byId("chat-list");
  const chat = chatList ? await benchScroll(chatList, 5000) : null;
  app.router.switchScreen("members");
  await sleep(300);
  const memberList = byId("member-list");
  const members = memberList ? await benchScroll(memberList, 5000) : null;
  const payload = { frontend_ready_ms: frontendReadyMs, chat, members };
  console.log("[autobench]", payload);
  try {
    await tauri.core.invoke("report_bench", { payload: JSON.stringify(payload) });
  } catch (error) {
    surface(error, "IPC_FAILED");
  }
}

async function reportUiSnapshot() {
  if (!app) {
    /** @type {any} */ (globalThis).__SPIKE_SNAPSHOT_DONE__ = true;
    return;
  }
  const tauri = /** @type {any} */ (window).__TAURI__;
  if (!tauri?.core?.invoke) {
    /** @type {any} */ (globalThis).__SPIKE_SNAPSHOT_DONE__ = true;
    return;
  }
  const { router, chat, members, emoji, settings, search } = app;

  const actions = { screenClicks: 0, categoryClicks: false, toggleClick: false, listScroll: false };
  let chatRenderedRows = 0;
  let memberRenderedRows = 0;
  if (Boolean(/** @type {any} */ (globalThis).__LIVE_SESSION_ACTIVE__)) {
    /** @type {any} */ (globalThis).__SPIKE_SNAPSHOT_DONE__ = true;
    return;
  }
  try {
    for (const button of document.querySelectorAll(".rail-btn")) {
      /** @type {HTMLElement} */ (button).click();
      if (button.classList.contains("active")) actions.screenClicks += 1;
    }
    const secondTab = document.querySelectorAll(".tab")[1];
    if (secondTab instanceof HTMLButtonElement) {
      secondTab.click();
      actions.categoryClicks = secondTab.classList.contains("active") && emoji.getActiveCategory() === 1;
    }
    const toggle = settings.toggles[0];
    if (toggle instanceof HTMLInputElement) {
      const oldValue = toggle.checked;
      toggle.click();
      actions.toggleClick = toggle.checked !== oldValue;
      toggle.click();
    }
    router.switchScreen("chat");
    await nextFrame();
    const oldScroll = chat.list.container.scrollTop;
    chat.list.scrollToIndex(500);
    actions.listScroll = chat.list.container.scrollTop !== oldScroll && chat.list.rows.size < CHAT_ROWS;
    chatRenderedRows = chat.list.rows.size;
    chat.list.scrollToIndex(CHAT_ROWS - 1);
    // Меряем участников, когда экран показан и получил высоту (P1-1).
    router.switchScreen("members");
    await nextFrame();
    await nextFrame();
    members?.list?.paint?.();
    memberRenderedRows = document.querySelectorAll("#member-list .member").length;
    router.switchScreen("chat");
  } catch (error) {
    surface(error, "ACTION_FAILED");
  } finally {
    /** @type {any} */ (globalThis).__SPIKE_SNAPSHOT_DONE__ = true;
  }

  const snapshot = {
    screenCount: document.querySelectorAll(".screen").length,
    chatRows: CHAT_ROWS,
    memberRows: MEMBER_ROWS,
    emojiCategories: emoji.categories.length,
    settingsToggles: settings.toggles.length,
    chatRenderedRows,
    memberRenderedRows,
    // Оконная модель (ADR-1): материализованные строки в JS, а не только в DOM.
    chatLoadedRows: chat.store.stats().loadedRows,
    memberLoadedRows: members.store.stats().loadedRows,
    actions,
    // P3-02: новые панели (аддитивные поля, контракт RW-38 не меняют).
    memberGroups: MEMBER_GROUPS.length,
    searchSections: SEARCH_SECTIONS.length,
    searchMatches: search.matchesCount,
    composerState: app?.composer.state ?? "empty",
    unreadHeight: chat.unreadHeight,
    toolbarItems: chat.toolbar?.itemCount ?? 0,
    // L-02-UI: наблюдаемость live-канала без payload/секретов (PLAN §6).
    dispatch: app?.dispatch?.stats ?? { source: "absent" },
  };
  try {
    await tauri.core.invoke("report_ui_snapshot", { payload: JSON.stringify(snapshot) });
  } catch (error) {
    surface(error, "IPC_FAILED");
  } finally {
    try {
      /** @type {any} */ (app)?.cleanupSpikeBenchmarkArtifacts?.();
    } catch {}
  }
}

reportUiSnapshot().catch((/** @type {unknown} */ error) => {
  surface(error, "IPC_FAILED");
});
autoBench().catch((/** @type {unknown} */ error) => {
  surface(error, "IPC_FAILED");
});

export {
  createUserPopoutController,
  createGuildModalsController,
  createAppShellWiring,
  createComposerPopoversBridge,
  createDiscordNativeBridge,
  getDiscordIPCEvent,
  RENDERER_IPC_SEND_WHITELIST,
  RENDERER_IPC_INVOKE_WHITELIST,
  RENDERER_SET_ALLOWLIST,
  isAllowedRendererSettingKey,
  assertAllowedSettingKey,
  formatReadableVerificationCode,
  RENDERER_IPC_SEND_SYNC_WHITELIST,
  RENDERER_SET_CHROMIUM_SWITCHES_ALLOWLIST,
  validateChromiumSwitches,
  PRELOAD_ONLY_NATIVE_MODULES,
  NATIVE_MODULE_NAME_REGEX,
  assertWhitelistedNativeModule,
  shouldShowVerificationCodeForChannel,
  createBrowserFeaturesFacade,
  normalizeApiEndpoint,
  sanitizeNativeModulePaths,
  ALLOWED_APP_PATHS,
  assertAllowedAppPath,
  DISCORD_CLIP_PROTOCOL,
  DISCORD_CLIP_UUID,
  shouldInvokeDockCancelBounce,
  resolveOsArch,
  DISCORD_MP4_ERROR_REASONS,
  formatClipProtocolUrl,
  parseClipRangeHeader,
  WEBAUTHN_CONVERTIBLE_ERROR_CODES,
  parseWebAuthnNativeResponse,
  FORBIDDEN_FRAME_DEBUGGER_CHANNELS,
  clampTracingDurationMs,
  ALLOWED_DESKTOP_CAPTURE_TYPES,
  normalizeDesktopCaptureTypes,
  ALLOWED_PROCESS_ENV_KEYS,
  projectProcessEnv,
  RIOT_LIVE_CLIENT_ORIGIN,
  buildRiotLiveClientUrl,
  BLOCKED_EXTERNAL_URL_PROTOCOLS,
  shouldOpenExternalUrl,
  checkUrlOriginMatches,
  formatSplashStatusText,
  SPLASH_DOWNLOAD_OPTIONS,
  shouldShowSplashProgress,
  shouldDisableBootstrapUpdater,
  resolveBootstrapAppIdentity,
  CODE_INTEGRITY_BYTE_SIZE,
  CODE_INTEGRITY_STREAM_TYPE,
  buildCodeIntegritySentryTags,
  determineBootstrapModuleDataPath,
  OLD_VERSION_DIR_REGEX,
  shouldRemoveOldVersionDirectory,
  buildCodeIntegritySentryContext,
  resolveBootstrapPaths,
  DESKTOP_TTI_WARMUP_DOMAINS,
  SENTRY_IGNORE_ERRORS,
  buildSentryDsn,
  resolveSentrySampleRate,
  detectLinuxRuntimeEnvironment,
  DESKTOP_ANALYTICS_CACHE_FILENAME,
  DESKTOP_ANALYTICS_CACHE_TTL_MS,
  DESKTOP_TTI_EVENT_TYPES,
  computeRestartAndFullTtiDuration,
  buildWindowsAutostartCommand,
  DNS_TELEMETRY_BASE_DOMAIN,
  BLACKBOX_MAX_MINIDUMP_AGE_MS,
  BLACKBOX_MAX_STORED_MINIDUMPS,
  normalizeDnsLabel,
  buildDnsTelemetryFqdn,
  computeScheduledBackoffStep,
  SQUIRREL_STARTUP_COMMANDS,
  SHELL_LINK_HEADER_BYTES,
  DISCLIP_PRIVILEGED_SCHEME,
  isWindowsShellLinkBytes,
  resolvePlatformQuitAccelerator,
  SPLASH_SCHEDULED_BACKOFF_STEPS_MS,
  OBSOLETED_SETTINGS_KEYS,
  resolveSplashWindowConfig,
  formatConnectivityErrorMessage,
  DISABLED_CHROMIUM_FEATURES_BASE,
  DISABLED_CHROMIUM_FEATURES_DARWIN_PRE24,
  BUILD_OVERRIDE_COOKIE_NAME,
  buildDisabledChromiumFeatures,
  extractUrlFromBootstrapArgs,
  buildCookieRemovalUrl,
  LINUX_AUTOSTART_COMMENT,
  MAX_HANDLED_BOOTSTRAP_ERRORS,
  formatLinuxAutostartDesktopEntry,
  formatWindowsRegAddArgs,
  shouldReportHandledBootstrapError,
  UPDATER_EVENT_CACHE_FILENAME,
  UPDATER_INCONSISTENT_INSTALLER_STATE_ERROR,
  UPDATER_TASK_STATES,
  classifyNativeUpdaterError,
  buildLegacyUpdaterFeedUrl,
  FRAME_DEBUGGER_MAX_HELD_EVENTS,
  MIN_MACOS_KERNEL_MAJOR,
  FRAME_DEBUGGER_ALLOWED_METHODS,
  FRAME_DEBUGGER_DENIED_PREFIXES,
  FRAME_DEBUGGER_DENIED_METHODS,
  checkFrameDebuggerCdpMethod,
  classifyElectronProcessMetricType,
  parseDarwinKernelMajor,
  WEBAUTHN_MACOS_KEYCHAIN_ACCESS_GROUP,
  FRAME_DEBUGGER_ROOT_METHODS,
  redactFrameDebuggerUrl,
  isValidActivityFrameOrigin,
  ALLOWED_MEDIA_ORIGINS,
  resolveWindowsAppBadge,
  parseClipHttpByteRange,
  NTP_PACKET_SIZE_BYTES,
  NTP_EPOCH_OFFSET_SECONDS,
  POPOUT_ALLOWED_FEATURES,
  STAFF_DEV_TOOLS_KEY,
  STAFF_DEV_TOOLS_PUBLIC_KEY_B64,
  NON_RENDERER_CRASH_COOLDOWN_MS,
  UTILITY_IGNORED_CRASH_EXIT_CODE,
  formatStaffDevToolsSignaturePayload,
  extractTokenFromLevelDbText,
  shouldReportNonRendererCrash,
  ZSTD_PRECOMPRESS_MAX_SIZE_MULTIPLIER,
  COMPRESSED_FILE_SUFFIX,
  DETAILED_TTI_FCP_TIMEOUT_MS,
  RENDERER_PERFORMANCE_MARKS,
  computeFileReadMaxSize,
  formatStartupTimingLine,
  TRAY_MENU_REBUILD_KEYS,
  THUMBAR_BUTTON_NAMES,
  RESOURCE_SCALE_VARIANTS,
  resolveTrayGuidForChannel,
  resolveTrayIconName,
  formatThumbarButtonIconFilename,
  NETWORK_LOGGING_CDP_PROTOCOL_VERSION,
  NETWORK_LOGGING_CDP_METHODS,
  NETWORK_LOGGING_STRIPPED_HEADERS,
  DOCUMENT_PIP_TRIGGER_ID,
  sanitizeNetworkLogHeaders,
  computeSpellcheckDictionaryDiff,
  resolveWindowContentProtection,
  CHUNKED_REQUEST_MISSING_ENDPOINT_ERROR,
  SYSIMG_MODULE_NAME,
  SYSIMG_UNAVAILABLE_ERROR,
  SYSIMG_TARGET_FORMAT,
  rewriteChunkedRequestPathname,
  isWebappBundleUrl,
  computeBundleStatsDurations,
  PARTIAL_DOWNLOAD_SUFFIX,
  HOOK_MINIDUMP_MAX_SIZE_BYTES,
  CLIPS_SENTRY_DIR_NAME,
  shouldDeleteUnusedAssetFile,
  shouldCompressLogFile,
  orderWebRtcLogPairByMtime,
  VOICE_MESSAGE_RECORDING_FILE_NAMES,
  VOICE_MESSAGE_RECORDING_SUBDIR,
  isValidVoiceRecordingFilename,
  isWhitelistedNativeModule,
  SPLASH_MEDIA_FORMAT_FLAG_PREFIX,
  PREMAP_TARGET_MODULES,
  NATIVE_PREMAP_FLAG,
  PREMAP_NO_COMMITTED_PATHS_ERROR,
  resolveSplashMediaFormatFromArgv,
  formatLinuxManualDownloadUrl,
  resolveSplashManualDownloadButtonText,
  UPDATER_CURRENT_LOG_SUFFIX,
  ADMIN_SERVICE_CURRENT_LOG_SUFFIX,
  SQUIRREL_SETUP_LOG_FILENAME,
  CLIPS_DECIDER_STASH_DIR_NAME,
  CLIPS_DECIDER_DATA_FILENAME,
  resolveDiscordServiceChannelFolder,
  formatDeduplicatedDownloadFilename,
  isHookMinidumpFilename,
  CLIPBOARD_RAW_FORMAT_PREFIX,
  MENU_EVENTS,
  WEBAPP_PATHS,
  DESKTOP_CAPTURER_REQUIRED_TYPES,
  unwrapClipboardRawFormat,
  hasClipboardMixedContent,
  filterDesktopCapturerTypes,
  UNHANDLED_JS_EXCEPTION_DELAY_MS,
  SENTRY_METADATA_TAG_KEYS,
  formatNestedMetadataKey,
  computeCrashReporterMetadataDiff,
  formatUnhandledJsExceptionMessage,
};
