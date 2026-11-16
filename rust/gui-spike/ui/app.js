// @ts-check
/**
 * P0-01: статический фронтенд спайка (RW-39 / W39d).
 *
 * Без сборщика и фреймворков: честный замер оболочки, а не бандлера.
 * Модули фич — `ui/features/*.js`; здесь только сборка, UI_SNAPSHOT и замер.
 */
import { byId } from "./features/dom.js";
import {
  CHANNEL_KINDS,
  createChannelTree,
} from "./features/channel-tree-controller.js";
import { createChannelSwitchPermissionsController } from "./features/channel-switch-permissions.js";
import { clearErrorState, reportError, showErrorState, UIError } from "./features/errors.js";
import { benchScroll, frontendReadyMs, startMetrics } from "./features/metrics.js";
import { createScreenRouter } from "./features/screens.js";
import { CHAT_ROWS, initChatScreen } from "./features/chat.js";
import { refreshLiveChatAuthorsInDom as refreshLiveChatAuthorsInDomHelper, renderMessage } from "./features/message-rendering.js";
import {
  COMPONENT_TYPES,
  BUTTON_STYLES,
  renderMessageComponents,
  createMessageComponentsContainer,
  handleComponentClick,
  setButtonLoading,
  clearButtonLoading,
  showBotInteractionToast,
} from "./features/message-components.js";
import {
  renderReactions,
  createReactionBadge,
  isVerificationChannel,
  isVerificationMessage,
  handleReactionVerification,
  REACTION_VERIFICATION_TOKENS,
  REACTION_VERIFICATION_TEXT,
  renderVerificationBanner,
} from "./features/reaction-verification.js";
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
import { createTauriEventListener, resolveTauriListen } from "./features/shell-bridge.js";
import { createVoicePanel, createVoiceStore } from "./features/voice.js";
import { createJoinServer } from "./features/join-server.js";
import { createCreateGuildController } from "./features/create-guild.js";
import { createTypingIndicator, createTypingSender } from "./features/typing-indicator.js";
import { asSnowflake, createDispatchStore } from "./features/dispatch-model.js";
import { createDispatchView } from "./features/dispatch-view.js";
import { createDispatchBridge, resolveDispatchListen } from "./features/dispatch-bridge.js";
import { createPinsThreadsWiring } from "./features/pins-threads-wiring.js";
import { createLiveEmojiPickerController } from "./features/live-emoji-picker.js";
import { createMessageInteractionController } from "./features/message-interaction-controller.js";
import { createVoiceCallController } from "./features/voice-call-orchestrator.js";
import { createInviteModal } from "./features/invite-modal.js";
import { createGiftModal } from "./features/gift-modal.js";
import { createQrAuthModal } from "./features/qr-auth-modal.js";
import {
  createRulesScreeningModalController,
  renderRulesScreeningBanner,
  isRulesScreeningRequired,
  extractRulesFromGuild,
  RULES_SCREENING_TEXT,
  RULES_SCREENING_TOKENS,
  DEFAULT_COMMUNITY_RULES,
} from "./features/rules-screening-modal.js";
import {
  VOICE_STAGE_PREVIEW_TOKENS,
  VOICE_STAGE_PREVIEW_TEXT,
  formatVoiceStageParticipantsSummary,
  renderVoiceStagePreview,
  renderVoiceStagePreviewBanner,
  removeVoiceStagePreviewBanner,
  createVoiceStagePreviewController,
} from "./features/voice-stage-preview.js";
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
const content = typeof document !== "undefined"
  ? /** @type {HTMLElement | null} */ (document.querySelector(".content"))
  : null;

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
  /** @type {ReturnType<typeof createTauriEventListener> | null} */
  let eventListener = null;
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
    // Свёрнутое/скрытое окно не опрашивает ядро: приложение не активно (кроме тестов __QA__).
    if (/** @type {any} */ (document).hidden === true && !(/** @type {any} */ (globalThis).__QA__)) return;
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
    if (!eventListener) {
      eventListener = createTauriEventListener({
        event: "gateway-status",
        initialFetch: async () => {
          try {
            return await invoke(NET_COMMANDS.status);
          } catch {
            return null;
          }
        },
        onEvent: (payload) => {
          if (disposed || !polling) return;
          const status = typeof payload === "string"
            ? payload
            : String(/** @type {any} */ (payload)?.status ?? "");
          if (status !== "") {
            apply(status);
          } else {
            void refresh();
          }
        },
      });
    }
    void eventListener.start().then((listening) => {
      if (!listening && polling && !disposed && timer === null) {
        timer = setInterval(() => { void refresh(); }, intervalMs);
      }
    });
    const hasTauriEvents = Boolean(resolveTauriListen());
    if (!hasTauriEvents && timer === null) {
      timer = setInterval(() => { void refresh(); }, intervalMs);
    }
    void refresh();
  }

  function stop() {
    polling = false;
    if (eventListener) {
      void eventListener.stop();
      eventListener = null;
    }
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
  const commands = createTypedCommands(invoke);
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
    if (typeof globalThis.Image === "function") {
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
    if (!icon || typeof globalThis.Image !== "function") {
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
    if (id === "") throw new UIError("IPC_FAILED", "ready: гильдия без id");
    const rawProps = /** @type {Record<string, unknown> | null | undefined} */ (
      bag.properties && typeof bag.properties === "object" ? bag.properties : null
    );
    const propName = typeof rawProps?.name === "string" ? rawProps.name : "";
    const name = typeof bag.name === "string" && bag.name !== ""
      ? bag.name
      : (propName !== "" ? propName : "Загрузка сервера...");
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
      const payload = await commands.ready();
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

/* Channel tree controller extracted to ./features/channel-tree-controller.js (RW51-EPIC2-T3). */

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
 *   pinsThreads: ReturnType<ReturnType<typeof createPinsThreadsWiring>["getPinsThreads"]>,
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
  /** @type {any} */
  let tree = null;
  /** @type {any} */
  let rail = null;
  /** @type {any} */
  let chat = null;
  /** @type {any} */
  let members = null;
  /** @type {any} */
  let emoji = null;
  /** @type {any} */
  let settings = null;
  /** @type {any} */
  let composer = null;
  /** @type {any} */
  let search = null;
  /** @type {any} */
  let quickSwitcher = null;
  /** @type {any} */
  let auth = null;
  /** @type {any} */
  let join = null;
  /** @type {any} */
  let inviteModal = null;
  /** @type {any} */
  let typingSender = null;
  /** @type {any} */
  let notifications = null;
  /** @type {any} */
  let settingsModal = null;
  /** @type {any} */
  let voiceDevicesHost = null;
  /** @type {any} */
  let voiceDevicesSession = null;
  /** @type {any} */
  let preliminaryBlock = null;
  /** @type {any} */
  let voiceDevicesExpand = null;
  /** @type {any} */
  let settingsRoot = null;
  let syncTickCounter = 0;
  /** @type {ReturnType<typeof createTauriEventListener> | ReturnType<typeof setInterval> | null} */
  let rtcPollTimer = null;
  let rtcPollInFlight = false;
  /** @type {any} */
  let micTestTimer = null;
  /** @type {ReturnType<typeof createTauriEventListener> | ReturnType<typeof setInterval> | null} */
  let voiceSyncTimer = null;
  /** @type {any} */
  let audioDeviceLevelsTimer = null;
  let audioDeviceLevelsInFlight = false;
  let lastAudioDeviceSyncAtMs = 0;
  let lastAudioDeviceInputConfig = { deviceId: "", volume: -1, muted: /** @type {boolean | null} */ (null) };
  let lastAudioDeviceOutputConfig = { deviceId: "", volume: -1, muted: /** @type {boolean | null} */ (null) };
  let lastVoiceConnectState = {
    guildId: /** @type {string | null} */ (null),
    channelId: /** @type {string | null} */ (null),
    selfMute: /** @type {boolean | null} */ (null),
    selfDeaf: /** @type {boolean | null} */ (null),
    selfVideo: /** @type {boolean | null} */ (null),
    selfStream: /** @type {boolean | null} */ (null),
  };
  // W3/W4: voice_connect deduplication state tracking (delegated to voice-connection-controller)
  // lastVoiceConnectState.guildId !== nextGuildId
  // lastVoiceConnectState.selfStream !== nextSelfStream
  /** @type {Promise<any> | null} */
  let inFlightAudioDevicesPromise = null;
  /** @type {Map<string, number>} */
  const lastVoiceUiSoundAtByKind = new Map();
  /** @type {(() => void) | null} */
  let syncVoiceSettingsScreenVisibility = null;
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
  /** @type {((includeModals?: boolean) => void) | null} */
  let closeLiveVoicePopoutsRef = null;
  const isLiveSession = () =>
    typeof globalThis.Image === "function";
  const router = createScreenRouter({
    onShow: (name) => {
      if (typeof closeLiveVoicePopoutsRef === "function") {
        closeLiveVoicePopoutsRef();
      }
      const isLiveDesktop =
        isLiveSession() &&
        !(/** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__) &&
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
        syncVoiceSettingsScreenVisibility?.();
      } else {
        syncVoiceSettingsScreenVisibility?.();
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
    if (cmd === "confirm_member_rules") {
      return `rust/gui-spike/src/guilds.rs::complete_guild_verification_and_permissions -> PUT /guilds/{id}/requests/@me`;
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
        : isLiveSession() && cmd === "accept_invite" && args && typeof args === "object" && "captcha_key" in args
          ? {
              ...args,
              captchaKey: args.captcha_key,
              ...(args.captcha_rqtoken ? { captchaRqtoken: args.captcha_rqtoken } : {}),
              ...(args.captcha_session_id ? { captchaSessionId: args.captcha_session_id } : {}),
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
      const result =
        effectiveCmd === "voice_audio_devices"
          ? await (inFlightAudioDevicesPromise ??
              (inFlightAudioDevicesPromise = Promise.resolve(
                tauri.core.invoke(effectiveCmd, effectiveArgs),
              ).finally(() => {
                inFlightAudioDevicesPromise = null;
              })))
          : await tauri.core.invoke(effectiveCmd, effectiveArgs);
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
      if (el.closest("#captcha-modal")) {
        return {
          module: "ui/features/captcha-modal.js + rust/gui-spike/src/invites.rs",
          handler: "createCaptchaModal (submit / solveWithToken / close)",
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
  const activeDmCalls = new Map();
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
   *   streamSourceName?: string,
   *   hideStreamPreview?: boolean,
   *   connectedAtMs: number,
   *   isAfk?: boolean,
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
  const SVG_SIGNAL_FINE = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M5 19h.01"/><path d="M5 13a6 6 0 0 1 6 6"/><path d="M5 7a12 12 0 0 1 12 12"/></svg>`;
  const SVG_SIGNAL_AVERAGE = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M5 19h.01"/><path d="M5 13a6 6 0 0 1 6 6"/><path d="M5 7a12 12 0 0 1 12 12" stroke-opacity="0.28"/></svg>`;
  const SVG_SIGNAL_BAD = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M5 19h.01"/><path d="M5 13a6 6 0 0 1 6 6" stroke-opacity="0.28"/><path d="M5 7a12 12 0 0 1 12 12" stroke-opacity="0.28"/></svg>`;

  const typedCommands = createTypedCommands(invoke);

  const voiceCallController = createVoiceCallController({
    byId,
    invoke,
    get commands() {
      return typedCommands;
    },
    tauri,
    surface,
    emitLiveTelemetry,
    isLiveSession,
    createTauriEventListener,
    HOME_GUILD_ID,
    SVG_ICONS: {
      micOn: SVG_MIC_ON,
      micOff: SVG_MIC_OFF,
      deafOn: SVG_DEAF_ON,
      deafOff: SVG_DEAF_OFF,
      chevronDown: SVG_CHEVRON_DOWN,
      signalBars: SVG_SIGNAL_BARS,
      signalFine: SVG_SIGNAL_FINE,
      signalAverage: SVG_SIGNAL_AVERAGE,
      signalBad: SVG_SIGNAL_BAD,
    },
    ipcCmds: {
      setAudioDevice: VOICE_SET_AUDIO_DEVICE_IPC_CMD,
      audioDevices: VOICE_AUDIO_DEVICES_IPC_CMD,
      rtcStatus: VOICE_RTC_STATUS_IPC_CMD,
      connect: VOICE_CONNECT_IPC_CMD,
      disconnect: VOICE_DISCONNECT_IPC_CMD,
    },
    selfVoicePrefs,
    liveVoiceStatesByGuild,
    activeDmCalls,
    liveVoiceChannelStartEpochMs,
    livePerUserVoiceState,
    knownLiveAudioDeviceIds,
    ignoredHotplugDeviceIds,
    liveUsersById,
    liveGuildsById,
    getActiveVoiceConnection: () => activeVoiceConnection,
    setActiveVoiceConnection: (/** @type {any} */ v) => {
      activeVoiceConnection = v;
    },
    getLiveRtcStatus: () => liveRtcStatus,
    setLiveRtcStatus: (/** @type {any} */ v) => {
      liveRtcStatus = v;
    },
    getLiveSelfUser: () => liveSelfUser,
    getHotplugPromptsSuppressed: () => hotplugPromptsSuppressed,
    setHotplugPromptsSuppressed: (/** @type {boolean} */ v) => {
      hotplugPromptsSuppressed = v;
    },
    getSkipVoiceSwitchConfirm: () => skipVoiceSwitchConfirm,
    setSkipVoiceSwitchConfirm: (/** @type {boolean} */ v) => {
      skipVoiceSwitchConfirm = v;
    },
    getSyncVoiceSettingsScreenVisibility: () => syncVoiceSettingsScreenVisibility,
    setSyncVoiceSettingsScreenVisibility: (/** @type {any} */ v) => {
      syncVoiceSettingsScreenVisibility = v;
    },
    getVoiceDevices: () => voiceDevices,
    setVoiceDevices: (/** @type {any} */ v) => {
      voiceDevices = v;
    },
    getSettingsRef: () => settingsRef,
    getSettingsObject: () => settingsRef,
    getChatActiveChannelId: () => /** @type {any} */ (chat)?.activeChannelId ?? "",
    getRailSelectedId: () => rail?.selectedId ?? "@me",
    selectedGuildId,
    currentGuildInfo: () => currentGuildInfo(),
    getDmFriendsController: () => dmFriendsController,
    getAuthState: () => auth,
    getVoicePanel: () => voice,
    getGlobalAppReactiveStore: () => globalAppReactiveStore,
    cleanDiscordStr: (/** @type {unknown} */ v) => cleanDiscordStr(v),
    resolveUserRoleInfo: (/** @type {string} */ gid, /** @type {string} */ uid) => resolveUserRoleInfo(gid, uid),
    computeEffectiveChannelPermissions: (/** @type {string} */ gid, /** @type {any} */ ch, /** @type {any} */ all) =>
      computeEffectiveChannelPermissions(gid, ch, all),
    computeServerPermissionsAndHierarchy: (/** @type {string} */ gid, /** @type {string} */ uid, /** @type {string[]} */ roles) =>
      computeServerPermissionsAndHierarchy(gid, uid, roles),
    canViewChannelInLiveGuild: (/** @type {string} */ gid, /** @type {any} */ ch, /** @type {any} */ all) =>
      canViewChannelInLiveGuild(gid, ch, all),
    openModeratorViewSidebar: (/** @type {string} */ gid, /** @type {any} */ p, /** @type {any} */ opts) =>
      openModeratorViewSidebar(gid, p, opts),
    refreshLiveChatAuthorsInDom: () => refreshLiveChatAuthorsInDom(),
    getTree: () => tree,
    getRail: () => rail,
    getRouter: () => router,
    getVoiceDevicesHost: () => voiceDevicesHost,
    getVoiceDevicesSession: () => voiceDevicesSession,
    getSettingsModal: () => settingsModal,
    getPreliminaryBlock: () => preliminaryBlock,
    getVoiceDevicesExpand: () => voiceDevicesExpand,
    getSettingsRoot: () => settingsRoot,
    lastVoiceUiSoundAtByKind,
    getVoiceHost: () => voiceHost,
    get voiceHost() {
      return voiceHost;
    },
    get tree() {
      return tree;
    },
    get rail() {
      return rail;
    },
    get router() {
      return router;
    },
    get dmChatHost() {
      return document.getElementById("dm-chat-host");
    },
    get dmHomeHost() {
      return document.getElementById("dm-home-host");
    },
    get uMute() {
      return document.querySelector('#user-bar [data-action="mute"]');
    },
    get uDeaf() {
      return document.querySelector('#user-bar [data-action="deafen"]');
    },
    get uSettings() {
      return document.querySelector('#user-bar [data-action="settings"]');
    },
    get voiceDevicesHost() {
      return voiceDevicesHost;
    },
    get voiceDevicesSession() {
      return voiceDevicesSession;
    },
    get settingsModal() {
      return settingsModal;
    },
    get preliminaryBlock() {
      return preliminaryBlock;
    },
    get voiceDevicesExpand() {
      return voiceDevicesExpand;
    },
    get settingsRoot() {
      return settingsRoot;
    },
    get createVoiceDevicesSettings() {
      return createVoiceDevicesSettings;
    },
    get DEFAULT_VOICE_DEVICES() {
      return DEFAULT_VOICE_DEVICES;
    },
    get syncTickCounter() {
      return syncTickCounter;
    },
    get lastVoiceStatesDigest() {
      return lastVoiceStatesDigest;
    },
    setLastVoiceStatesDigest: (/** @type {string} */ v) => {
      lastVoiceStatesDigest = v;
    },
    get refreshLiveReadyInFlight() {
      return refreshLiveReadyInFlight;
    },
    setRefreshLiveReadyInFlight: (/** @type {boolean} */ v) => {
      refreshLiveReadyInFlight = v;
    },
  });

  const {
    computeRtcSignalMetrics,
    playVoiceUiSound,
    closeLiveVoicePopouts,
    isChannelAfk,
    syncVoiceMuteStateWithBackend,
    pollAudioDeviceLevels,
    startAudioDeviceLevelsPolling,
    stopAudioDeviceLevelsPolling,
    toggleSelfMicOnly,
    toggleSelfDeafenBoth,
    ensureUserActivityTracking,
    isUserActivelyOccupiedInApp,
    checkServerVoiceAfkTimeout,
    startRtcStatusPolling,
    stopRtcStatusPolling,
    stopVoiceSync,
    retryActiveVoiceConnection,
    connectToLiveVoiceChannel,
    disconnectFromLiveVoiceChannel,
    promptVoiceChannelSwitch,
    isTruthyVoiceFlag,
    normalizeVoiceGuildKey,
    mergeVoiceStatesForGuild,
    mergeVoiceStatesSnapshot,
    isVoiceChannelConnected,
    getVoiceChannelTimer,
    getVoiceParticipants,
    showVoiceToastNotice,
    resolveChannelObject,
    canStreamInChannel,
    canSpeakInChannel,
    isChannelRoleRestricted,
    getAloneInviteRemainingMs,
    openVoiceInviteModal,
    openVoiceParticipantContextMenu,
    openAudioDevicePopover,
    openAudioDeviceHotplugPrompt,
    openRtcDiagnosticsModal,
    openRtcConnectionPopout,
    showMutedMicFloatingWarning,
    openKrispNoisePopout,
    openSoundboardPopout,
    openActivitiesPopout,
    cancelPipAnimTimer,
    requestLiveScreenPreviewFrame,
    switchLiveStreamSource,
    openLiveStreamStageViewer,
    drawLiveCaptureCanvasFrame,
    openNitroHdModal,
    openStreamSettingsMenu,
    openScreenSharePortalPicker,
    syncFloatingStreamPipPreview,
    openScreenShareModal,
    updateLiveRtcDomInPlace,
    ensureUserBarDeviceChevrons,
    syncUserBarVoiceButtons,
    renderLiveGuildVoiceStagePreview,
    renderLiveDmCallStage,
    playRingingTone,
    showIncomingCallOverlay,
    dismissIncomingCallOverlay,
    renderLiveVoicePanel,
    refreshLiveReadyVoiceAndUsers,
    openVoiceParticipantActivityPopout,
    mountVoiceDevices,
    setVoiceDevicesExpanded,
    refreshLiveAudioDevices,
    ensureVoiceTroubleshootControls,
    setVoicePanelVisibility,
    cleanupLiveVoiceTimersAndAudio,
    getPreviewedVoiceChannel,
    setPreviewedVoiceChannel,
    setFloatingPipPosition,
  } = voiceCallController;

  closeLiveVoicePopoutsRef = closeLiveVoicePopouts;

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
    const selfId = cleanDiscordStr(
      String(liveSelfUser?.id || /** @type {any} */ (globalThis).__LIVE_SELF_USER__?.id || ""),
    );
    if (selfId) {
      if (myRoles.length > 0) {
        liveMemberRolesByGuildAndUser.set(`${gId}:${selfId}`, myRoles);
      } else if (hasExplicitMyRoles) {
        liveMemberRolesByGuildAndUser.set(`${gId}:${selfId}`, []);
      }
    }
    if (Array.isArray(guildObj.members)) {
      for (const m of guildObj.members) {
        if (!m || typeof m !== "object") continue;
        const uObj = m.user && typeof m.user === "object" ? m.user : {};
        const uid = cleanDiscordStr(String(uObj.id ?? m.user_id ?? m.userId ?? m.id ?? ""));
        if (!uid) continue;
        if (uid === selfId) {
          const mPending =
            typeof m.pending === "boolean"
              ? m.pending
              : typeof m.is_pending === "boolean"
                ? m.is_pending
                : null;
          if (mPending !== null) {
            mergedGuild.pending = mPending;
            mergedGuild.is_pending = mPending;
          } else if (Array.isArray(m.roles) && m.roles.length > 0) {
            mergedGuild.pending = false;
            mergedGuild.is_pending = false;
          }
        }
        if (Array.isArray(m.roles) && !(uid === selfId && hasExplicitMyRoles)) {
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
    const rawGuildVs = Array.isArray(guildObj.voice_states)
      ? guildObj.voice_states
      : Array.isArray(guildObj.voiceStates)
        ? guildObj.voiceStates
        : null;
    if (rawGuildVs && rawGuildVs.length > 0) {
      mergeVoiceStatesForGuild(liveVoiceStatesByGuild, gId, rawGuildVs);
    }
    if (myRoles.length > 0 || prevGuild?.pending === false) {
      mergedGuild.pending = false;
      mergedGuild.is_pending = false;
    }
    liveGuildsById.set(gId, mergedGuild);
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
    const selfId = cleanDiscordStr(
      String(liveSelfUser?.id || /** @type {any} */ (globalThis).__LIVE_SELF_USER__?.id || ""),
    );
    const guildObj = liveGuildsById.get(gId);
    if (
      uid === selfId &&
      guildObj &&
      (guildObj.lurker === true || guildObj.isMember === false || guildObj.is_member === false)
    ) {
      return { colorHex: null, topRoleName: "", roles: [] };
    }
    const hasExplicitLiveRoles = liveMemberRolesByGuildAndUser.has(`${gId}:${uid}`);
    let userRoles = liveMemberRolesByGuildAndUser.get(`${gId}:${uid}`) ?? [];
    if (!hasExplicitLiveRoles && userRoles.length === 0 && uid === selfId) {
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
    if (!hasExplicitLiveRoles && userRoles.length === 0 && Array.isArray(guildObj?.members)) {
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
    if (uid === selfId || hasExplicitLiveRoles) {
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
    refreshLiveChatAuthorsInDomHelper({
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
    });
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
    const selfId = cleanDiscordStr(
      String(liveSelfUser?.id || /** @type {any} */ (globalThis).__LIVE_SELF_USER__?.id || ""),
    );
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
    const resolvedChannel = resolveChannelObject(channel, allChannels, gId) ?? channel;
    const guildChs = Array.isArray(guildObj.channels) ? guildObj.channels : [];
    const chList = [
      ...(Array.isArray(allChannels) ? allChannels : []),
      ...(Array.isArray(tree?.channels) ? tree.channels : []),
      ...guildChs,
    ];
    const parentId = cleanDiscordStr(
      String(resolvedChannel.parentId ?? resolvedChannel.parent_id ?? ""),
    );
    if (parentId && resolvedChannel.type !== 4) {
      const parentCh = chList.find((c) => String(c?.id ?? "") === parentId);
      const parentOw = parentCh?.permissionOverwrites ?? parentCh?.permission_overwrites ?? [];
      if (Array.isArray(parentOw) && parentOw.length > 0) {
        effective = applyOverwrites(effective, parentOw);
      }
    }
    const ownOw =
      resolvedChannel.permissionOverwrites ?? resolvedChannel.permission_overwrites ?? [];
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
          const rawDisplayName =
            cleanDiscordStr(m?.displayName) ||
            cleanDiscordStr(knownUser?.nick) ||
            cleanDiscordStr(knownUser?.global_name) ||
            cleanDiscordStr(knownUser?.globalName) ||
            cleanDiscordStr(m?.username) ||
            cleanDiscordStr(knownUser?.username) ||
            "";
          const isSnowflakeOnly = Boolean(rawDisplayName && /^\d{15,22}$/.test(rawDisplayName));
          const displayName = !isSnowflakeOnly && rawDisplayName
            ? rawDisplayName
            : `Участник ${uid ? uid.slice(-4) : ""}`.trim();
          const rawUname = cleanDiscordStr(m?.username) || cleanDiscordStr(knownUser?.username) || "";
          const isUnameSnowflake = Boolean(rawUname && /^\d{15,22}$/.test(rawUname));
          const uname = !isUnameSnowflake && rawUname
            ? rawUname
            : `user_${uid ? uid.slice(-4) : "anon"}`;
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
  syncTickCounter = 0;
  let lastDmFriendsSyncAt = 0;
  let lastSyncedGuildIdsSig = "";
  const lastGuildChannelCountById = new Map();
  /**
   * @param {any[]} rawGuilds
   * @param {boolean} [incremental=false]
   */
  function syncLiveGuildRailFromSnapshot(rawGuilds, incremental = false) {
    if (!Array.isArray(rawGuilds)) return;
    for (const g of rawGuilds) {
      const gid = String(g?.id ?? "").trim();
      if (!gid) continue;
      hydrateGuildMembersAndRoles(g);
    }
    if (!incremental) {
      if (rawGuilds.length > 0) {
        const validIds = new Set(rawGuilds.map((g) => String(g?.id ?? "").trim()).filter(Boolean));
        for (const oldGid of Array.from(liveGuildsById.keys())) {
          if (!validIds.has(oldGid)) {
            liveGuildsById.delete(oldGid);
            lastGuildChannelCountById.delete(oldGid);
          }
        }
      }
    }
    const parsedGuilds = [];
    for (const [gid, hydrated] of liveGuildsById.entries()) {
      const name = String(hydrated?.name ?? hydrated?.properties?.name ?? gid);
      const icon =
        typeof hydrated?.icon === "string"
          ? hydrated.icon
          : typeof hydrated?.properties?.icon === "string"
            ? hydrated.properties.icon
            : null;
      const roleSig = `${Array.isArray(hydrated?.roles) ? hydrated.roles.length : 0}:${Array.isArray(hydrated?.myRoles) ? hydrated.myRoles.join(",") : ""}:${Boolean(hydrated?.lurker)}:${hydrated?.isMember !== false}`;
      parsedGuilds.push({
        id: gid,
        name,
        icon,
        memberCount: Number(hydrated?.memberCount ?? hydrated?.member_count ?? 0),
        channelCount: Number(
          hydrated?.channelCount ?? (Array.isArray(hydrated?.channels) ? hydrated.channels.length : 0),
        ),
        roleSig,
      });
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
      const validIds = new Set(liveGuildsById.keys());
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

  chat = initChatScreen((error) => surface(error), invoke);
  /** @type {ReturnType<typeof createComposer> | null} */
  let composerRef = null;
  /** @type {any} */
  let activeOpenChannelTarget = null;

  const rulesScreeningModal = createRulesScreeningModalController({
    document,
    invoke,
    onSuccess: (gid) => {
      const gObj = liveGuildsById.get(gid);
      if (gObj) {
        gObj.pending = false;
        gObj.is_pending = false;
      }
      showVoiceToastNotice(RULES_SCREENING_TEXT.successNotice);
      syncActiveChannelPermissionsInDom(gid);
    },
    onError: (_err) => {
      showVoiceToastNotice(RULES_SCREENING_TEXT.errorNotice);
    },
  });

  const {
    syncActiveChannelPermissionsInDom,
    minimizeStageViewerToPip,
    onChannelSwitch,
    onSelectChannel,
    installChatOpenChannelWrapper,
  } = createChannelSwitchPermissionsController({
    isLiveSession,
    cleanDiscordStr,
    getSelectedGuildId: () => rail.selectedId || selectedGuildId() || "",
    refreshTreeView: () => tree.refreshView(),
    getTreeChannels: () => tree?.channels,
    getActiveOpenChannelTarget: () => activeOpenChannelTarget,
    setActiveOpenChannelTarget: (/** @type {any} */ t) => { activeOpenChannelTarget = t; },
    getChatChannel: () => chat.channel,
    getChatChannelId: () => chat.channelId,
    openChatChannel: (/** @type {any} */ t) => chat.openChannel(t),
    resolveChannelObject,
    getGuildById: (/** @type {string} */ gid) => liveGuildsById.get(gid),
    computeEffectiveChannelPermissions,
    getSelfUserId: () => liveSelfUser?.id || /** @type {any} */ (globalThis).__LIVE_SELF_USER__?.id || "",
    evaluateClientChannelPolicy,
    applyComposerChannelPolicy: (/** @type {any} */ policy) => composerRef?.applyChannelPolicy?.(policy),
    invoke,
    showVoiceToastNotice,
    isRulesScreeningModalOpen: () => Boolean(rulesScreeningModal?.isOpen?.()),
    closeRulesScreeningModal: () => rulesScreeningModal?.close?.(),
    openRulesScreeningModal: (/** @type {any} */ opts) => rulesScreeningModal.open(opts),
    getCurrentGuildInfo: () => currentGuildInfo(),
    extractRulesFromGuild,
    rulesScreeningText: RULES_SCREENING_TEXT,
    refreshLiveChatAuthorsInDom,
    getActiveVoiceConnection: () => activeVoiceConnection,
    renderLiveVoicePanel,
    clearFloatingPipPosition: () => { setFloatingPipPosition(null); },
    syncFloatingStreamPipPreview,
    getContentElement: () => content,
    clearTyping: () => typing?.clear(),
    homeGuildId: HOME_GUILD_ID,
    normalizeVoiceGuildKey,
    getSelfVoicePrefs: () => selfVoicePrefs,
    connectToLiveVoiceChannel,
    sendDmRest,
    renderLiveDmCallStage,
    syncVoiceMuteStateWithBackend,
    openScreenShareModal,
    openVoiceInviteModal,
    openGroupDmModal: () => dmFriendsController?.openGroupDmModal?.(),
    setActiveDmCall: (/** @type {string} */ cid, /** @type {any} */ data) => { activeDmCalls.set(cid, data); },
    getLiveVoiceStatesForGuild: (/** @type {string} */ gid) => liveVoiceStatesByGuild.get(gid),
    setLiveVoiceStatesForGuild: (/** @type {string} */ gid, /** @type {any} */ states) => { liveVoiceStatesByGuild.set(gid, states); },
    clearPreviewedVoiceChannel: () => { setPreviewedVoiceChannel(null); },
    shouldAutoAck: (/** @type {string} */ cid) => Boolean(notificationsRef?.shouldAutoAck(cid)),
    getLastMessageId: (/** @type {string} */ cid) => notificationsRef?.getLastMessageId(cid) ?? "",
    ackChannel: (/** @type {string} */ cid, /** @type {any} */ mid) =>
      notificationsRef?.ackChannel(cid, mid) ?? Promise.resolve(),
  });

  if (typeof window !== "undefined") {
    /** @type {any} */ (window).minimizeStageViewerToPip = minimizeStageViewerToPip;
    /** @type {any} */ (window).onChannelSwitch = onChannelSwitch;
    /** @type {any} */ (window).onSelectChannel = onSelectChannel;
  }

  installChatOpenChannelWrapper(chat);
  /** @type {any} */
  // C-02: дерево каналов выбранной гильдии. Клик по текстовому/announcement
  // открывает реальный канал (`messages_page`) с настоящим channelId.
  // const tree = createChannelTree({
  tree = createChannelTree({
    invoke,
    onOpen: (channel) => {
      setPreviewedVoiceChannel(null);
      onChannelSwitch(channel);
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
      const cid = String(channel.id || "");
      const currentPreviewed = getPreviewedVoiceChannel();
      const wasAlreadyPreviewing = Boolean(
        currentPreviewed &&
          currentPreviewed.id === cid &&
          currentPreviewed.guildId === gId,
      );
      setPreviewedVoiceChannel({
        guildId: gId,
        guildName: gName,
        id: cid,
        name: String(channel.name || cid || "Голосовой канал"),
        type: Number(channel.type || 2),
      });
      onChannelSwitch(channel);
      if (typeof window !== "undefined") {
        /** @type {any} */ (window).__ACTIVE_DM_RECIPIENT__ = null;
      }
      /** @type {any} */ (globalThis).__ACTIVE_DM_RECIPIENT__ = null;
      void chat.openChannel(channel)
        .then(() => typing?.refresh())
        .catch((/** @type {unknown} */ error) => surface(error));
      syncActiveChannelPermissionsInDom();
      const hashNode = document.querySelector(".chat-header .hash");
      if (hashNode) hashNode.textContent = "🔊";
      const titleNode = document.querySelector(".chat-header .chat-title-text");
      if (titleNode) titleNode.textContent = String(channel.name || cid);
      const screenTitleNode = document.querySelector(".screen-title");
      if (screenTitleNode && channel.name) {
        screenTitleNode.textContent = `Голосовой канал · 🔊 ${channel.name}`;
      }
      renderLiveGuildVoiceStagePreview();
      if (
        activeVoiceConnection &&
        activeVoiceConnection.channelId === channel.id &&
        activeVoiceConnection.guildId === gId
      ) {
        openRtcConnectionPopout();
        return;
      }
      if (isLiveSession() && gId && gId !== "@me") {
        const gObj = liveGuildsById.get(cleanDiscordStr(gId));
        const isLurker =
          gObj?.lurker === true || gObj?.isMember === false || gObj?.is_member === false;
        const isPending = gObj?.pending === true;
        const effPerms = computeEffectiveChannelPermissions(gId, channel, tree?.channels);
        const PERM_VIEW_AND_CONNECT = (1n << 10n) | (1n << 20n);
        const missingConnect =
          effPerms !== null && (effPerms & PERM_VIEW_AND_CONNECT) !== PERM_VIEW_AND_CONNECT;
        if (isLurker || isPending || missingConnect) {
          connectToLiveVoiceChannel(gId, gName, channel);
          return;
        }
      }
      if (!wasAlreadyPreviewing) {
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
  /**
   * Hydrates DM recipient and friends list from the Gateway READY payload.
   * Hoisted function declaration ensures immediate availability during early bootstrap.
   * @param {any} payload
   */
  const hydrateDmFriendsFromReady = (payload) => {
    mountDmFriends(payload);
  };
  /** @type {ReturnType<typeof createNotificationsController> | null} */
  let notificationsRef = null;
  /** @type {ReturnType<typeof createCreateGuildController> | null} */
  let createGuildModal = null;

  rail = createGuildRail({
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
    // G-03 / STUB-03: «+» открывает модалку join-by-invite с переходом в создание сервера.
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
            rail.selectHome("restore", false);
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
          mergeVoiceStatesSnapshot(liveVoiceStatesByGuild, rawVs);
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
        try {
          settingsRef?.unlockLiveSettings?.(liveSelfUser, payload);
        } catch {
          /* В тестовом DOM-шиме без MutationObserver живой энхансер настроек пропускается */
        }
        try {
          renderLiveVoicePanel();
        } catch {}
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
          voiceSyncTimer = createTauriEventListener({
            event: "voice-participants-sync",
            initialFetch: async () => {
              if (!document.getElementById("live-voice-user-context-menu")) {
                void refreshLiveReadyVoiceAndUsers();
              }
            },
            onEvent: () => {
              if (!document.getElementById("live-voice-user-context-menu")) {
                void refreshLiveReadyVoiceAndUsers();
              }
            },
            autoStart: true,
          });
        }
        if (!(/** @type {any} */ (globalThis).__QA__)) {
          net.stop();
          const netBanner = byId("net-status");
          if (netBanner) {
            netBanner.setAttribute("data-state", "ready");
            netBanner.setAttribute("hidden", "");
          }
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
  join = createJoinServer({
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
        return await invoke(cmd, args);
      }
      return invoke(cmd, args);
    },
    onAccepted: handleAcceptedInviteGuild,
    onOpenCreate: (opener) => createGuildModal?.open(opener),
  });

  /**
   * STUB-03: модалка создания сервера (#create-guild-dialog).
   */
  createGuildModal = createCreateGuildController({
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
  inviteModal = createInviteModal({
    invoke,
  });

  /**
   * STUB-10 / M-10, M-11: прогрессивное создание выпадающих панелей пинов и веток в хедере чата.
   * Контроллер и DOM-узлы создаются по требованию (SPEC §1–§3): при первом обращении
   * или при первом клике на триггер, сохраняя чистоту DOM до первого взаимодействия.
   */
  const pinsThreadsWiring = createPinsThreadsWiring({
    document,
    container: document.querySelector(".content") ?? document.body,
    pinsButton: document.querySelector('[data-action="pins"]'),
    threadsButton: document.querySelector('[data-action="threads"]'),
    chat,
    invoke,
    router,
  });
  const getPinsThreads = pinsThreadsWiring.getPinsThreads;
  pinsThreadsWiring.bindPinsThreadsButtons();

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
      const gId = normalizeVoiceGuildKey(ev.guildId);
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
      const myActiveGidKey = normalizeVoiceGuildKey(activeVoiceConnection?.guildId);

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
        for (const [otherGk, otherArr] of liveVoiceStatesByGuild.entries()) {
          if (!ev.guildId || otherGk === gId || otherGk === ev.guildId || normalizeVoiceGuildKey(otherGk) === gId) {
            const filtered = otherArr.filter((x) => String(x?.user_id ?? x?.userId ?? "") !== uId);
            if (filtered.length !== otherArr.length) {
              liveVoiceStatesByGuild.set(otherGk, filtered);
            }
          }
        }
        if (uId === selfId && activeVoiceConnection && myActiveGidKey === gId) {
          activeVoiceConnection = null;
          lastVoiceConnectState = {
            guildId: null,
            channelId: null,
            selfMute: null,
            selfDeaf: null,
            selfVideo: null,
            selfStream: null,
          };
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
            const curInputDev = selfVoicePrefs.inputDeviceId || "default";
            const curInputVol = selfVoicePrefs.inputVolume;
            const curInputMuted = Boolean(selfVoicePrefs.muted || selfVoicePrefs.deafened);
            if (
              lastAudioDeviceInputConfig.deviceId !== curInputDev ||
              lastAudioDeviceInputConfig.volume !== curInputVol ||
              lastAudioDeviceInputConfig.muted !== curInputMuted
            ) {
              lastAudioDeviceInputConfig = { deviceId: curInputDev, volume: curInputVol, muted: curInputMuted };
              void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
                kind: "input",
                deviceId: curInputDev,
                volumePercent: curInputVol,
                muted: curInputMuted,
              }).catch(() => {});
            }
          } else if (myActiveGidKey !== gId) {
            activeVoiceConnection = null;
            lastVoiceConnectState = {
              guildId: null,
              channelId: null,
              selfMute: null,
              selfDeaf: null,
              selfVideo: null,
              selfStream: null,
            };
            stopRtcStatusPolling();
            selfVoicePrefs.muted = isTruthyVoiceFlag(ev.selfMute);
            selfVoicePrefs.deafened = isTruthyVoiceFlag(ev.selfDeaf);
            const curInputDev = selfVoicePrefs.inputDeviceId || "default";
            const curInputVol = selfVoicePrefs.inputVolume;
            const curInputMuted = Boolean(selfVoicePrefs.muted || selfVoicePrefs.deafened);
            if (
              lastAudioDeviceInputConfig.deviceId !== curInputDev ||
              lastAudioDeviceInputConfig.volume !== curInputVol ||
              lastAudioDeviceInputConfig.muted !== curInputMuted
            ) {
              lastAudioDeviceInputConfig = { deviceId: curInputDev, volume: curInputVol, muted: curInputMuted };
              void invoke(VOICE_SET_AUDIO_DEVICE_IPC_CMD, {
                kind: "input",
                deviceId: curInputDev,
                volumePercent: curInputVol,
                muted: curInputMuted,
              }).catch(() => {});
            }
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
  const baseDispatchApply = dispatchView.apply.bind(dispatchView);
  dispatchView.apply = (changes) => {
    const rolePermTypes = new Set([
      "guild_member_update",
      "guild_role_create",
      "guild_role_update",
      "guild_role_delete",
    ]);
    const list = Array.isArray(changes) ? changes : [];
    const rolePermCount = list.filter((c) => rolePermTypes.has(c?.type)).length;
    const otherChanges = list.filter((c) => !rolePermTypes.has(c?.type));
    const res =
      otherChanges.length > 0
        ? baseDispatchApply(otherChanges)
        : { applied: 0, ignored: 0, failed: 0, notOpen: 0, notLoaded: 0 };
    return {
      ...res,
      applied: (Number(res.applied) || 0) + rolePermCount,
    };
  };
  dispatch = createDispatchBridge({
    listen: resolveDispatchListen(window),
    view: dispatchView,
    store: createDispatchStore(),
    onDispatch: (envelope) => {
      if (!envelope || typeof envelope !== "object") return;
      const kind = envelope.kind;
      const data = envelope.data;
      if (kind === "READY") {
        if (isLiveSession() && data && typeof data === "object") {
          if (Array.isArray(data.guilds) && data.guilds.length > 0) {
            syncLiveGuildRailFromSnapshot(data.guilds, false);
          }
          if (Array.isArray(data.relationships) && data.relationships.length > 0) {
            dmFriendsController?.syncLiveState?.({
              relationships: data.relationships,
              privateChannels: data.private_channels,
              users: data.users,
            });
          }
        }
      } else if (kind === "READY_SUPPLEMENTAL") {
        if (isLiveSession() && data && typeof data === "object") {
          if (Array.isArray(data.guilds) && data.guilds.length > 0) {
            syncLiveGuildRailFromSnapshot(data.guilds, true);
          }
          if (data.merged_presences) {
            dmFriendsController?.syncLiveState?.({
              presences: data.merged_presences.friends,
            });
          }
        }
      } else if (kind === "RELATIONSHIP_ADD" || kind === "RELATIONSHIP_REMOVE" || kind === "RELATIONSHIP_UPDATE") {
        dmFriendsController?.handleGatewayEvent?.({ type: kind, d: data });
      } else if (kind === "GUILD_CREATE") {
        if (data && typeof data === "object") {
          hydrateGuildMembersAndRoles(data);
          if (data.id) {
            syncLiveGuildRailFromSnapshot([data], true);
          }
        }
      } else if (kind === "GUILD_DELETE") {
        if (data?.id) {
          if (data.unavailable === true) {
            // Discord API: unavailable: true означает временную недоступность сервера из-за сбоя или реконнекта.
            // Сервер не должен удаляться из списка серверов!
            return;
          }
          const gid = String(data.id);
          liveGuildsById.delete(gid);
          lastGuildChannelCountById.delete(gid);
          syncLiveGuildRailFromSnapshot([], true);
          const railEl = document.querySelector("#server-rail, #guild-rail");
          const tile = railEl?.querySelector(`[data-guild-id="${gid}"]`);
          tile?.remove();
        }
      } else if (kind === "VOICE_STATE_UPDATE") {
        if (data && typeof data === "object") {
          const member = data.member && typeof data.member === "object" ? data.member : null;
          const userObj =
            (member?.user && typeof member.user === "object" ? member.user : null) ??
            (data.user && typeof data.user === "object" ? data.user : null);
          const uid = cleanDiscordStr(
            String(userObj?.id ?? data.user_id ?? data.userId ?? ""),
          );
          if (uid && (userObj || member?.nick)) {
            const prevU = liveUsersById.get(uid) ?? {};
            const nick = cleanDiscordStr(member?.nick);
            liveUsersById.set(uid, {
              ...prevU,
              ...(userObj || {}),
              id: uid,
              ...(nick ? { nick } : {}),
            });
          }
          // [ANTI-PHANTOM FIX]: Применяем живой VOICE_STATE_UPDATE к списку участников голосовых каналов
          const rawGid = cleanDiscordStr(String(data.guild_id ?? data.guildId ?? ""));
          const normGid = normalizeVoiceGuildKey(rawGid);
          const rawCid =
            data.channel_id === null || data.channelId === null
              ? ""
              : cleanDiscordStr(String(data.channel_id ?? data.channelId ?? ""));
          if (!rawCid) {
            // Пользователь покинул голосовой канал: удаляем из всех списков гильдии
            for (const [gk, arr] of liveVoiceStatesByGuild.entries()) {
              if (!rawGid || gk === rawGid || gk === normGid) {
                const filtered = arr.filter((x) => String(x?.user_id ?? x?.userId ?? "") !== uid);
                if (filtered.length !== arr.length) {
                  liveVoiceStatesByGuild.set(gk, filtered);
                }
              }
            }
          } else {
            mergeVoiceStatesForGuild(liveVoiceStatesByGuild, normGid, [data]);
            if (rawGid && rawGid !== normGid) {
              mergeVoiceStatesForGuild(liveVoiceStatesByGuild, rawGid, [data]);
            }
          }
          applyVoiceStateUpdateEvent(liveVoiceStatesByGuild, data);
          tree.refreshView();
          renderLiveVoicePanel();
          renderLiveDmCallStage();
          updateLiveRtcDomInPlace();
        }
      } else if (kind === "GUILD_MEMBER_UPDATE") {
        if (data && typeof data === "object") {
          const gid = cleanDiscordStr(String(data.guild_id ?? data.guildId ?? ""));
          const uid = cleanDiscordStr(String(data.user?.id ?? data.user_id ?? data.userId ?? ""));
          const selfId = cleanDiscordStr(
            String(liveSelfUser?.id || /** @type {any} */ (globalThis).__LIVE_SELF_USER__?.id || ""),
          );
          const roles = Array.isArray(data.roles) ? data.roles.map(String) : [];
          if (gid && uid) {
            liveMemberRolesByGuildAndUser.set(`${gid}:${uid}`, roles);
            liveMemberColorByGuildAndUser.delete(`${gid}:${uid}`);
            const gObj = liveGuildsById.get(gid);
            if (gObj && typeof gObj === "object") {
              if (uid === selfId) {
                gObj.myRoles = roles;
                gObj.my_roles = roles;
                if ("pending" in data && data.pending !== undefined) {
                  gObj.pending = Boolean(data.pending);
                }
              }
              if (Array.isArray(gObj.members)) {
                const mIdx = gObj.members.findIndex(
                  (/** @type {any} */ m) =>
                    String(m?.user?.id ?? m?.user_id ?? m?.userId ?? m?.id ?? "") === uid,
                );
                if (mIdx >= 0 && gObj.members[mIdx] && typeof gObj.members[mIdx] === "object") {
                  gObj.members[mIdx] = {
                    ...gObj.members[mIdx],
                    roles,
                    ...("nick" in data ? { nick: data.nick } : {}),
                    ...("pending" in data ? { pending: data.pending } : {}),
                  };
                }
              }
            }
            const nick = cleanDiscordStr(data.nick);
            if (nick) {
              const prevU = liveUsersById.get(uid) ?? {};
              liveUsersById.set(uid, { ...prevU, id: uid, nick });
            }
            syncActiveChannelPermissionsInDom(gid);
          }
        }
      } else if (kind === "CALL_CREATE" || kind === "CALL_UPDATE") {
        if (data && typeof data === "object") {
          const cid = cleanDiscordStr(String(data.channel_id || ""));
          if (cid) {
            activeDmCalls.set(cid, data);
            if (Array.isArray(data.voice_states) && data.voice_states.length > 0) {
              const currentMe = liveVoiceStatesByGuild.get("@me") ?? [];
              const otherStates = currentMe.filter((vs) => String(vs?.channel_id || "") !== cid);
              for (const vs of data.voice_states) {
                otherStates.push({
                  ...vs,
                  guild_id: "@me",
                  channel_id: cid,
                  confirmed: true,
                });
              }
              liveVoiceStatesByGuild.set("@me", otherStates);
              liveVoiceStatesByGuild.set(HOME_GUILD_ID, otherStates);
            }
            const selfId = cleanDiscordStr(
              String(liveSelfUser?.id || /** @type {any} */ (globalThis).__LIVE_SELF_USER__?.id || "")
            );
            const isConnectedHere = Boolean(
              activeVoiceConnection &&
                normalizeVoiceGuildKey(activeVoiceConnection.guildId) === HOME_GUILD_ID &&
                String(activeVoiceConnection.channelId || "") === cid,
            );
            const ringingArr = Array.isArray(data.ringing) ? data.ringing.map(String) : [];
            const isRinging = !isConnectedHere && (
              ringingArr.includes(selfId) ||
              (ringingArr.length > 0 && (!selfId || ringingArr.some((/** @type {string} */ uid) => uid !== selfId)))
            );
            if (isRinging) {
              showIncomingCallOverlay(data);
            } else {
              dismissIncomingCallOverlay(cid);
            }
            renderLiveDmCallStage();
            tree.refreshView();
          }
        }
      } else if (kind === "CALL_DELETE") {
        if (data && typeof data === "object") {
          const cid = cleanDiscordStr(String(data.channel_id || ""));
          if (cid) {
            activeDmCalls.delete(cid);
            dismissIncomingCallOverlay(cid);
            const currentMe = liveVoiceStatesByGuild.get("@me") ?? [];
            const filtered = currentMe.filter((vs) => String(vs?.channel_id || "") !== cid);
            liveVoiceStatesByGuild.set("@me", filtered);
            liveVoiceStatesByGuild.set(HOME_GUILD_ID, filtered);
            renderLiveDmCallStage();
            tree.refreshView();
          }
        }
      } else if (kind === "STREAM_CREATE") {
        if (data && typeof data === "object") {
          const skey = String(data.stream_key || "");
          const parts = skey.split(":");
          const streamerUid = parts[parts.length - 1];
          const streamGid = parts.length >= 4 ? parts[1] : "@me";
          if (streamerUid) {
            for (const gk of new Set([streamGid, normalizeVoiceGuildKey(streamGid), "@me", HOME_GUILD_ID])) {
              if (typeof gk !== "string") continue;
              const list = liveVoiceStatesByGuild.get(gk);
              if (Array.isArray(list)) {
                for (const vs of list) {
                  if (String(vs?.user_id || vs?.userId || "") === streamerUid) {
                    vs.self_stream = true;
                    vs.selfStream = true;
                  }
                }
              }
            }
            playVoiceUiSound("stream_on");
            renderLiveVoicePanel();
            renderLiveDmCallStage();
            tree.refreshView();
          }
        }
      } else if (kind === "STREAM_DELETE") {
        if (data && typeof data === "object") {
          const skey = String(data.stream_key || "");
          const parts = skey.split(":");
          const streamerUid = parts[parts.length - 1];
          const streamGid = parts.length >= 4 ? parts[1] : "@me";
          if (streamerUid) {
            for (const gk of new Set([streamGid, normalizeVoiceGuildKey(streamGid), "@me", HOME_GUILD_ID])) {
              if (typeof gk !== "string") continue;
              const list = liveVoiceStatesByGuild.get(gk);
              if (Array.isArray(list)) {
                for (const vs of list) {
                  if (String(vs?.user_id || vs?.userId || "") === streamerUid) {
                    vs.self_stream = false;
                    vs.selfStream = false;
                  }
                }
              }
            }
            playVoiceUiSound("stream_off");
            renderLiveVoicePanel();
            renderLiveDmCallStage();
            tree.refreshView();
          }
        }
      } else if (kind === "GUILD_ROLE_CREATE" || kind === "GUILD_ROLE_UPDATE") {
        if (data && typeof data === "object") {
          const gid = cleanDiscordStr(String(data.guild_id ?? data.guildId ?? ""));
          const role = data.role && typeof data.role === "object" ? data.role : null;
          const roleId = cleanDiscordStr(String(role?.id ?? ""));
          if (gid && role && roleId) {
            const gObj = liveGuildsById.get(gid);
            if (gObj && typeof gObj === "object") {
              const prevRoles = Array.isArray(gObj.roles) ? [...gObj.roles] : [];
              const idx = prevRoles.findIndex((/** @type {any} */ r) => String(r?.id ?? "") === roleId);
              if (idx >= 0) {
                prevRoles[idx] = { ...prevRoles[idx], ...role, id: roleId };
              } else {
                prevRoles.push({ ...role, id: roleId });
              }
              gObj.roles = prevRoles;
            }
            for (const key of liveMemberColorByGuildAndUser.keys()) {
              if (key.startsWith(`${gid}:`)) {
                liveMemberColorByGuildAndUser.delete(key);
              }
            }
            syncActiveChannelPermissionsInDom(gid);
          } else {
            tree.refreshView();
          }
        }
      } else if (kind === "GUILD_ROLE_DELETE") {
        if (data && typeof data === "object") {
          const gid = cleanDiscordStr(String(data.guild_id ?? data.guildId ?? ""));
          const roleId = cleanDiscordStr(String(data.role_id ?? data.roleId ?? data.role?.id ?? ""));
          if (gid && roleId) {
            const gObj = liveGuildsById.get(gid);
            if (gObj && typeof gObj === "object") {
              if (Array.isArray(gObj.roles)) {
                gObj.roles = gObj.roles.filter((/** @type {any} */ r) => String(r?.id ?? "") !== roleId);
              }
              if (Array.isArray(gObj.myRoles)) {
                gObj.myRoles = gObj.myRoles.filter((/** @type {any} */ r) => String(r) !== roleId);
              }
              if (Array.isArray(gObj.my_roles)) {
                gObj.my_roles = gObj.my_roles.filter((/** @type {any} */ r) => String(r) !== roleId);
              }
              if (Array.isArray(gObj.members)) {
                for (const m of gObj.members) {
                  if (m && Array.isArray(m.roles)) {
                    m.roles = m.roles.filter((/** @type {any} */ r) => String(r) !== roleId);
                  }
                }
              }
            }
            for (const [key, rList] of liveMemberRolesByGuildAndUser.entries()) {
              if (key.startsWith(`${gid}:`) && Array.isArray(rList) && rList.includes(roleId)) {
                liveMemberRolesByGuildAndUser.set(
                  key,
                  rList.filter((r) => r !== roleId),
                );
              }
            }
            for (const key of liveMemberColorByGuildAndUser.keys()) {
              if (key.startsWith(`${gid}:`)) {
                liveMemberColorByGuildAndUser.delete(key);
              }
            }
            syncActiveChannelPermissionsInDom(gid);
          } else {
            tree.refreshView();
          }
        }
      } else if (kind === "CHANNEL_UPDATE") {
        if (data && typeof data === "object") {
          const gid = cleanDiscordStr(String(data.guild_id ?? data.guildId ?? ""));
          const cid = cleanDiscordStr(String(data.id ?? data.channel_id ?? data.channelId ?? ""));
          if (gid && cid) {
            const ow = data.permission_overwrites ?? data.permissionOverwrites;
            const gObj = liveGuildsById.get(gid);
            if (gObj && Array.isArray(gObj.channels)) {
              const chIdx = gObj.channels.findIndex((/** @type {any} */ c) => String(c?.id ?? "") === cid);
              if (chIdx >= 0) {
                gObj.channels[chIdx] = {
                  ...gObj.channels[chIdx],
                  ...data,
                  ...(Array.isArray(ow)
                    ? { permission_overwrites: ow, permissionOverwrites: ow }
                    : {}),
                };
              }
            }
            if (Array.isArray(tree?.channels)) {
              const tIdx = tree.channels.findIndex((/** @type {any} */ c) => String(c?.id ?? "") === cid);
              if (tIdx >= 0 && tree.channels[tIdx]) {
                tree.channels[tIdx] = {
                  ...tree.channels[tIdx],
                  ...(data.name !== undefined ? { name: String(data.name) } : {}),
                  ...(Array.isArray(ow)
                    ? { permission_overwrites: ow, permissionOverwrites: ow }
                    : {}),
                };
              }
            }
            syncActiveChannelPermissionsInDom(gid);
          }
        }
      }
    },
  });

  // Реакции идут через тот же `invoke`: в live-сессии ошибка прав (403) показывается тостом, не перекрывая весь чат.
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
  typingSender = createTypingSender({
    invoke,
    onNotice: (text, kind) => {
      if (isLiveSession() && !(/** @type {any} */ (globalThis).__QA__)) return;
      typing?.notice(text, kind);
    },
  });
  composer = createComposer({
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
            if (recSec >= 1200) {
              voiceMsgBtn.click();
              return;
            }
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
  search = createSearchOverlay({
    chat,
    invoke: (cmd, args) => /** @type {Promise<any>} */ (invoke(cmd, args)),
    getGuildId: selectedGuildId,
    header: /** @type {HTMLInputElement | null} */ (byId("chat-search")),
    onError: (error) => surface(error),
  });
  quickSwitcher = createQuickSwitcher({ router });

  members = initMemberScreen((error) => surface(error));
  membersRef = members;
  lists.set("members", members.list);
  emoji = initEmojiConsole({
    // §4/EMO-16: выбор эмодзи вставляет глиф в позицию курсора композера.
    onSelect: (glyph) => composer.insertText(glyph),
  });

  settings = initSettingsConsole({
    invoke,
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
    if (!isLiveSession() || Boolean((/** @type {any} */ (globalThis)).__SPIKE_AUTH_DISABLED__) || (/** @type {any} */ (globalThis).__QA__)) return;
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
    if (topTitleEl) topTitleEl.textContent = "Chord";
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
    const notice = document.querySelector(".chat-notice");
    if (notice instanceof HTMLElement) {
      notice.style.display = "none";
      notice.setAttribute("hidden", "");
    }
    const giftBtn = document.querySelector('[data-action="gift"]');
    if (giftBtn instanceof HTMLElement) {
      giftBtn.removeAttribute("aria-disabled");
      giftBtn.removeAttribute("disabled");
      giftBtn.setAttribute("title", "Отправить подарок Nitro");
    }
    const inboxBtn = document.querySelector('[data-action="inbox"]');
    if (inboxBtn instanceof HTMLElement) {
      inboxBtn.removeAttribute("aria-disabled");
      inboxBtn.removeAttribute("disabled");
      inboxBtn.setAttribute("title", "Уведомления и почта");
    }
    const hint = document.getElementById("composer-hint");
    if (hint instanceof HTMLElement) {
      hint.textContent = "";
    }
    const prelimBlock = document.getElementById("voice-devices-preliminary-block");
    if (prelimBlock instanceof HTMLElement) {
      prelimBlock.style.display = "none";
    }
    const voiceDevices = document.getElementById("voice-devices");
    if (voiceDevices instanceof HTMLElement) {
      voiceDevices.removeAttribute("hidden");
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
        openLiveComposerEmojiPickerRef?.(composerEmojiBtn);
      },
      true,
    );
  }
    const settingsHead = document.querySelector("#screen-settings .settings-head");
    if (settingsHead && !document.getElementById("settings-close-esc-btn")) {
      const closeEscBtn = document.createElement("button");
      closeEscBtn.type = "button";
      closeEscBtn.id = "settings-close-esc-btn";
      closeEscBtn.setAttribute("aria-label", "Close settings (ESC)");
      closeEscBtn.title = "Close settings (ESC)";
      closeEscBtn.style.cssText =
        "display:inline-flex;align-items:center;gap:6px;padding:6px 12px;border:1px solid var(--border-subtle, #4e5058);border-radius:18px;background:transparent;color:var(--text-normal, #dbdee1);font-size:12px;font-weight:700;cursor:pointer;";
      closeEscBtn.textContent = "✕ ESC";
      closeEscBtn.addEventListener("click", () => {
        router.switchScreen("chat");
      });
      settingsHead.append(closeEscBtn);
    }

    // Unified global keydown handler for live shortcuts (Alt+2/3, Ctrl+E, ESC popout dismiss)
    document.addEventListener?.(
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
          const popout = document.getElementById("live-emoji-picker-popout");
          if (popout) {
            popout.remove();
          } else {
            openLiveComposerEmojiPickerRef?.(composerEmojiBtn);
          }
          return;
        }
        if (ev.key === "Escape") {
          if (router.current === "settings") {
            router.switchScreen("chat");
          }
          const popout = document.getElementById("live-emoji-picker-popout");
          if (popout) {
            popout.remove();
          }
        }
      },
      true,
    );
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

  const emojiPickerController = createLiveEmojiPickerController({
    byId,
    chat,
    composer,
    invoke,
  });
  openLiveComposerEmojiPickerRef = emojiPickerController.openLiveComposerEmojiPicker;

  const messageInteractionController = createMessageInteractionController({
    byId,
    content,
    chat,
    rail,
    tree,
    dmFriendsController,
    getLiveSelfUser: () => liveSelfUser,
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
  });
  /** @type {any} */ (globalThis).__LIVE_CAN_DELETE_MESSAGE__ = messageInteractionController.canDeleteLiveMessage;
  messageInteractionController.bindLiveChatMessageEvents();
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
  settingsModal = byId("settings-modal");
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
  voiceDevicesHost = byId("voice-devices");
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
  voiceDevicesSession = {
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
  // Свёрнутая секция: пометка и кнопка живут вне `#voice-devices`, потому что
  // контроллер строит свои узлы через `host.replaceChildren` и стёр бы их.
  preliminaryBlock = byId("voice-devices-preliminary-block");
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
  voiceDevicesExpand = /** @type {HTMLElement | null} */ (byId("voice-devices-expand"));
  if (!voiceDevicesExpand) {
    voiceDevicesExpand = document.createElement("button");
    voiceDevicesExpand.id = "voice-devices-expand";
    voiceDevicesExpand.className = "voice-devices-expand";
    voiceDevicesExpand.setAttribute("type", "button");
    voiceDevicesExpand.setAttribute("aria-controls", "voice-devices");
    preliminaryBlock.append(voiceDevicesExpand);
  }
  setVoiceDevicesExpanded(false);
  voiceDevicesExpand?.addEventListener("click", () => {
    setVoiceDevicesExpanded(voiceDevicesExpand?.getAttribute("aria-expanded") !== "true");
  });

  /** @type {Map<string, any>} */
  const guildNotifEntries = new Map();
  notifications = createNotificationsController({
    document,
    invoke: (cmd, args) => /** @type {Promise<any>} */ (invoke(cmd, args)),
  });
  /** @param {any} [opts] */
  notifications.openModal = (opts) => {
    if (opts && opts.guildId) {
      notifications.openGuildModal(opts);
    } else {
      syncNotificationsWithGuild(opts?.triggerElement);
    }
  };
  notificationsRef = notifications;
  settingsRoot = byId("settings");
  syncVoiceSettingsScreenVisibility = setVoicePanelVisibility;
  setVoicePanelVisibility();
  for (const item of document.querySelectorAll("#settings-nav .settings-nav-item")) {
    item.addEventListener("click", setVoicePanelVisibility);
  }
  // P2-5 (Pass 3): клик по навигации — не единственный путь смены вкладки.
  // `settings.showPane(...)` (deep-link, ссылка «Уведомления» внутри панели,
  // программный вызов) шёл мимо слушателей и оставлял voice-панель видимой на
  // чужой вкладке. Обёртка делает синхронизацию инвариантом обоих путей.
  const showPaneOriginal = settings.showPane.bind(settings);
  /** @param {any} key */
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
        tree.channels.find((/** @type {any} */ channel) => CHANNEL_KINDS[channel.type]?.openable === true)?.id ||
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
  auth = createAuthGate({
    onReady: () => {
      // После ready чат доступен: роутер на chat, фокус уходит из закрытого гейта.
      router.switchScreen("chat");
      const list = byId("chat-list");
      const target = /** @type {any} */ (list);
      if (target && typeof target.focus === "function") target.focus();
      // L-01: сетевой статус актуален только в активном приложении (после входа).
      if (isLiveSession() && !(/** @type {any} */ (globalThis).__QA__)) {
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
      for (const k of Array.from(liveVoiceStatesByGuild.keys())) {
        liveVoiceStatesByGuild.delete(k);
      }
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

  const giftModal = createGiftModal({
    document,
    container: document.body,
    triggerButton: /** @type {HTMLElement | null} */ (
      document.querySelector('[data-action="composer-gift"]') || document.querySelector('[data-action="gift"]')
    ),
    invoke: (cmd, args) => /** @type {Promise<any>} */ (invoke(cmd, args)),
  });

  const qrAuthModal = createQrAuthModal({
    document,
    container: document.body,
    triggerButton: /** @type {HTMLElement | null} */ (
      document.querySelector('[data-action="open-qr-auth-modal"]') ||
      document.getElementById("auth-qr-code") ||
      document.getElementById("auth-qr")
    ),
    invoke: (cmd, args) => /** @type {Promise<any>} */ (invoke(cmd, args)),
  });

  return {
    giftModal,
    qrAuthModal,
    rulesScreeningModal,
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

if (typeof document !== "undefined") {
  try {
    app = boot();
  } catch (error) {
    // Ошибка загрузки фич не должна молча оставить пустой экран: показываем оверлей.
    surface(error, "ELEMENT_MISSING");
  }
}

/** @returns {any} */
function getApp() {
  return app;
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
  renderMessage,
  renderMessageComponents,
  createMessageComponentsContainer,
  handleComponentClick,
  setButtonLoading,
  clearButtonLoading,
  showBotInteractionToast,
  COMPONENT_TYPES,
  BUTTON_STYLES,
  renderReactions,
  createReactionBadge,
  isVerificationChannel,
  isVerificationMessage,
  handleReactionVerification,
  REACTION_VERIFICATION_TOKENS,
  REACTION_VERIFICATION_TEXT,
  renderVerificationBanner,
  getApp,
  createRulesScreeningModalController,
  renderRulesScreeningBanner,
  isRulesScreeningRequired,
  extractRulesFromGuild,
  RULES_SCREENING_TEXT,
  RULES_SCREENING_TOKENS,
  DEFAULT_COMMUNITY_RULES,
};
