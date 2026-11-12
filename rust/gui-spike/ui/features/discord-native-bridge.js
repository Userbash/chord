// @ts-check

/**
 * Normalizes an IPC event name and ensures it starts with DISCORD_.
 * @param {string} ev
 * @returns {string}
 */
export function getDiscordIPCEvent(ev) {
  if (typeof ev !== "string" || ev.trim() === "") {
    throw new Error("invalid IPC event");
  }
  return /^DISCORD_/.test(ev) ? ev : "DISCORD_" + ev;
}

/**
 * Whitelist of IPC events allowed for send().
 * @type {readonly string[]}
 */
export const RENDERER_IPC_SEND_WHITELIST = Object.freeze([
  "DISCORD_WINDOW_MINIMIZE",
  "DISCORD_WINDOW_MAXIMIZE",
  "DISCORD_WINDOW_CLOSE",
  "DISCORD_WINDOW_FOCUS",
  "DISCORD_WINDOW_BLUR",
  "DISCORD_WINDOW_RESTORE",
  "DISCORD_WINDOW_DEVTOOLS_OPEN",
  "DISCORD_WINDOW_SET_ALWAYS_ON_TOP",
  "DISCORD_WINDOW_SET_PROGRESS_BAR",
  "DISCORD_CLIPBOARD_COPY",
  "DISCORD_CLIPBOARD_CUT",
  "DISCORD_CLIPBOARD_PASTE",
  "DISCORD_PROCESS_UTILS_FLUSH_DNS_CACHE",
  "DISCORD_PROCESS_UTILS_FLUSH_STORAGE_DATA",
  "DISCORD_SETTINGS_UPDATE_BACKGROUND_COLOR",
  "DISCORD_USER_DATA_CACHE_SAVE",
  "DISCORD_USER_DATA_CACHE_DELETE",
  "DISCORD_CRASH_REPORTER_UPDATE_METADATA",
  "DISCORD_CRASH_REPORTER_TRIGGER_JS_EXCEPTION",
  "DISCORD_GPU_SETTINGS_SET_ENABLE_HWACCEL",
  "DISCORD_GPU_SETTINGS_SET_CHROMIUM_SWITCHES",
]);

/**
 * Whitelist of IPC events allowed for invoke().
 * @type {readonly string[]}
 */
export const RENDERER_IPC_INVOKE_WHITELIST = Object.freeze([
  "DISCORD_POWER_MONITOR_GET_SYSTEM_IDLE_TIME",
  "DISCORD_CLIPBOARD_READ_TEXT",
  "DISCORD_CLIPBOARD_WRITE_TEXT",
  "DISCORD_PROCESS_UTILS_GET_SYSTEM_INFO",
  "DISCORD_PROCESS_UTILS_GET_SYSTEM_METRICS",
  "DISCORD_FILE_MANAGER_SHOW_OPEN_DIALOG",
  "DISCORD_FILE_MANAGER_SHOW_SAVE_DIALOG",
  "DISCORD_SAFE_STORAGE_IS_ENCRYPTION_AVAILABLE",
  "DISCORD_SAFE_STORAGE_ENCRYPT_STRING",
  "DISCORD_SAFE_STORAGE_DECRYPT_STRING",
  "DISCORD_THUMBAR_BUTTONS_CLICKED",
  "DISCORD_SPELLCHECK_GET_AVAILABLE_DICTIONARIES",
  "DISCORD_SPELLCHECK_SET_LOCALE",
  "DISCORD_SPELLCHECK_SET_LANGUAGES",
  "DISCORD_SPELLCHECK_SET_LEARNED_WORDS",
  "DISCORD_SPELLCHECK_REPLACE_MISSPELLING",
  "DISCORD_SETTINGS_GET",
  "DISCORD_SETTINGS_SET",
  "DISCORD_USER_DATA_CACHE_GET",
  "DISCORD_WINDOW_RESTORE",
  "DISCORD_WINDOW_SHOW_INACTIVE",
  "DISCORD_WINDOW_BLUR",
  "DISCORD_WINDOW_SET_PROGRESS_BAR",
  "DISCORD_WINDOW_TOGGLE_FULLSCREEN",
  "DISCORD_WINDOW_SET_MINIMUM_SIZE",
  "DISCORD_WINDOW_SET_BACKGROUND_THROTTLING",
  "DISCORD_WINDOW_SET_FRAME_RATE",
  "DISCORD_WINDOW_SET_CONTENT_PROTCTION",
  "DISCORD_WINDOW_SET_WINDOW_CONTENT_PROTECTION",
  "DISCORD_WINDOW_GET_NATIVE_HANDLE",
  "DISCORD_WINDOW_GET_MEDIA_SOURCE_ID",
  "DISCORD_WINDOW_SET_FOCUSABLE",
  "DISCORD_WINDOW_OPEN_DOCUMENT_PIP",
  "WINDOW_SET_TRAFFIC_LIGHT_POSITION",
  "WINDOW_SET_TRAFFIC_LIGHT_APPEARANCE",
]);

/**
 * Whitelist of IPC events allowed for sendSync().
 * @type {readonly string[]}
 */
export const RENDERER_IPC_SEND_SYNC_WHITELIST = Object.freeze([
  "DISCORD_SAFE_STORAGE_IS_ENCRYPTION_AVAILABLE",
  "DISCORD_SAFE_STORAGE_ENCRYPT_STRING",
  "DISCORD_SAFE_STORAGE_DECRYPT_STRING",
  "DISCORD_NATIVE_MODULES_GET_PATHS",
  "DISCORD_GPU_SETTINGS_GET_ENABLE_HWACCEL",
  "DISCORD_FEATURES_GET_BROWSER_FEATURES",
]);

/**
 * Whitelist of allowed voice recording filenames.
 * @type {readonly string[]}
 */
export const ALLOWED_VOICE_RECORDING_FILENAMES = Object.freeze([
  "clip.ogg",
  "clip.wav",
  "clip.mp3",
  "clip.webm",
  "clip.m4a",
  "clip.aac",
]);

/**
 * Maximum voice recording byte size (25 MiB).
 * @type {number}
 */
export const MAX_VOICE_RECORDING_BYTES = 25 * 1024 * 1024;

/**
 * Checks whether a filename is an allowed voice recording filename.
 * @param {string} fileName
 * @returns {boolean}
 */
export function isAllowedVoiceRecordingFilename(fileName) {
  if (typeof fileName !== "string") {
    return false;
  }
  return ALLOWED_VOICE_RECORDING_FILENAMES.includes(fileName);
}

/**
 * Regex for safe save dialog filenames (alphanumeric, dot, underscore, dash).
 * @type {RegExp}
 */
export const SAFE_SAVE_FILENAME_REGEX = /^[a-zA-Z0-9._-]+$/;

/**
 * Validates save dialog filename.
 * @param {string} fileName
 * @returns {boolean}
 */
export function isValidSaveDialogFilename(fileName) {
  return (
    typeof fileName === "string" &&
    fileName.length > 0 &&
    fileName !== "." &&
    fileName !== ".." &&
    SAFE_SAVE_FILENAME_REGEX.test(fileName)
  );
}

/**
 * Normalizes clipboard copy action payload.
 * @param {string} text
 * @returns {{ mode: "copy_selection" | "write_text", text: string }}
 */
export function normalizeClipboardCopyAction(text) {
  if (typeof text !== "string" || text.length === 0) {
    return { mode: "copy_selection", text: "" };
  }
  return { mode: "write_text", text };
}


/**
 * Allowed settings keys that can be set by the renderer.
 * RE-GAP-27 §9.1-9.3. Exactly 17 keys.
 * @type {readonly string[]}
 */
export const RENDERER_SET_ALLOWLIST = Object.freeze([
  "audioSubsystem",
  "offloadAdmControls",
  "debugLogging",
  "asyncVideoInputDeviceInit",
  "ALWAYS_ALLOW_UPDATES",
  "ALLOW_OPTIONAL_UPDATES",
  "openH264Enabled",
  "DESKTOP_TTI_DNSTCP_WARMUP",
  "DESKTOP_TTI_REMOVE_V8_CACHE_CLEAR",
  "DESKTOP_TTI_REMOVE_SHADER_CACHE_CLEAR",
  "DESKTOP_TTI_EARLY_UPDATE_CHECK",
  "DESKTOP_TTI_HTTP_CDT",
  "DESKTOP_TTI_SPLASH_USE_WEBP",
  "DESKTOP_TTI_LEGACY_OVERLAY_WORKER",
  "DESKTOP_TTI_UPDATE_BACKOFF_MAX_MS",
  "DESKTOP_TTI_UPDATE_BACKOFF_ALGO",
  "DESKTOP_NATIVE_PREMAP",
]);

/**
 * Checks if a setting key is allowed to be set by the renderer.
 * @param {string} name
 * @returns {boolean}
 */
export function isAllowedRendererSettingKey(name) {
  return typeof name === "string" && RENDERER_SET_ALLOWLIST.includes(name);
}

/**
 * Asserts that a setting key is allowed, throwing an Error if not.
 * @param {string} name
 * @returns {string}
 */
export function assertAllowedSettingKey(name) {
  if (!isAllowedRendererSettingKey(name)) {
    throw new Error(`Cannot set setting key: ${String(name)}`);
  }
  return name;
}

/**
 * Creates a DiscordNative bridge protecting renderer IPC channels.
 * @param {{
 *   invoke?: (channel: string, ...args: any[]) => Promise<any>,
 *   send?: (channel: string, ...args: any[]) => void
 * }} [options]
 */
export function createDiscordNativeBridge({ invoke, send } = {}) {
  return {
    /**
     * Sends an asynchronous one-way IPC message.
     * @param {string} ev
     * @param {...any} args
     */
    send(ev, ...args) {
      const canonical = getDiscordIPCEvent(ev);
      if (!RENDERER_IPC_SEND_WHITELIST.includes(canonical)) {
        throw new Error("cannot send this event");
      }
      if (typeof send === "function") {
        send(canonical, ...args);
      }
    },

    /**
     * Invokes an asynchronous two-way IPC call.
     * @param {string} ev
     * @param {...any} args
     * @returns {Promise<any>}
     */
    async invoke(ev, ...args) {
      const canonical = getDiscordIPCEvent(ev);
      if (!RENDERER_IPC_INVOKE_WHITELIST.includes(canonical)) {
        throw new Error("cannot invoke this event");
      }
      if (typeof invoke === "function") {
        return await invoke(canonical, ...args);
      }
      return undefined;
    },
  };
}

/**
 * Empty allowlist for renderer-initiated chromium switches (RE-GAP-29 §1.6).
 * @type {readonly string[]}
 */
export const RENDERER_SET_CHROMIUM_SWITCHES_ALLOWLIST = Object.freeze(/** @type {readonly string[]} */ ([]));

/**
 * Validates that chromium switches payload is a plain object and all keys are allowlisted.
 * @param {Record<string, any>} switches
 * @returns {boolean}
 */
export function validateChromiumSwitches(switches) {
  if (switches == null || typeof switches !== "object" || Array.isArray(switches)) {
    throw new TypeError("switches must be a plain object");
  }
  for (const key of Object.keys(switches)) {
    if (!RENDERER_SET_CHROMIUM_SWITCHES_ALLOWLIST.includes(key)) {
      throw new Error(`Cannot set chromium switch key: ${key}`);
    }
  }
  return true;
}

/**
 * Native modules permitted only during preload phase (RE-GAP-29 §2.1).
 * @type {readonly string[]}
 */
export const PRELOAD_ONLY_NATIVE_MODULES = Object.freeze(["discord_premap"]);

/**
 * Regex matching whitelisted native module naming pattern.
 */
export const NATIVE_MODULE_NAME_REGEX = /^discord_[a-z0-9_-]+$/;

/**
 * Asserts that a native module name is whitelisted.
 * @param {string} name
 * @returns {string}
 */
export function assertWhitelistedNativeModule(name) {
  const str = String(name);
  if (
    typeof name !== "string" ||
    (!NATIVE_MODULE_NAME_REGEX.test(str) && str !== "erlpack") ||
    PRELOAD_ONLY_NATIVE_MODULES.includes(str)
  ) {
    throw new Error(`"${str}" is not a whitelisted native module`);
  }
  return str;
}

/**
 * Creates a browser features facade with supported checks and declaration.
 * @param {string[]} [initialFeatures=[]]
 * @returns {{ supports(feature: string): boolean, declareSupported(feature: string): void }}
 */
export function createBrowserFeaturesFacade(initialFeatures = []) {
  const supportedFeatures = new Set(Array.isArray(initialFeatures) ? initialFeatures : []);
  return Object.freeze({
    supports(feature) {
      return supportedFeatures.has(feature);
    },
    declareSupported(feature) {
      supportedFeatures.add(feature);
    },
  });
}

/**
 * Normalizes raw API endpoint string. Returns null if null, undefined or empty string.
 * @param {any} rawEndpoint
 * @returns {string | null}
 */
export function normalizeApiEndpoint(rawEndpoint) {
  if (rawEndpoint == null || rawEndpoint === "") {
    return null;
  }
  return String(rawEndpoint);
}

/**
 * Sanitizes native module paths by filtering out electron.asar and taking up to 2 renderer module paths.
 * @param {{ browserModulePaths?: string[], rendererModulePaths?: string[] }} [paths={}]
 * @returns {readonly string[]}
 */
export function sanitizeNativeModulePaths({ browserModulePaths = [], rendererModulePaths = [] } = {}) {
  const safeBrowser = Array.isArray(browserModulePaths)
    ? browserModulePaths.filter((p) => typeof p === "string" && !p.includes("electron.asar"))
    : [];
  const safeRenderer = Array.isArray(rendererModulePaths)
    ? rendererModulePaths.slice(0, 2)
    : [];
  return Object.freeze([...safeBrowser, ...safeRenderer]);
}

/**
 * Whitelisted application paths accessible via native bridge (module 1405).
 * @type {readonly string[]}
 */
export const ALLOWED_APP_PATHS = Object.freeze([
  "home",
  "appData",
  "desktop",
  "documents",
  "downloads",
  "crashDumps",
  "exe",
  "videos",
  "temp",
]);

/**
 * Validates whether pathName is in ALLOWED_APP_PATHS. Throws Error if not allowed.
 * @param {string} pathName
 * @returns {string}
 */
export function assertAllowedAppPath(pathName) {
  if (typeof pathName !== "string" || !ALLOWED_APP_PATHS.includes(pathName)) {
    throw new Error(`${String(pathName)} is not an allowed app path`);
  }
  return pathName;
}

/**
 * Custom protocol name and UUID for Discord clip protocol (RE-GAP-30 §7).
 */
export const DISCORD_CLIP_PROTOCOL = "disclip";
export const DISCORD_CLIP_UUID = "a1c8529933464db888f083f57a75a5ef";

/**
 * Determines whether dock cancel bounce should be invoked for given bounce ID (module 2100).
 * @param {number} id
 * @returns {boolean}
 */
export function shouldInvokeDockCancelBounce(id) {
  return typeof id === "number" && !Number.isNaN(id) && id >= 0;
}

/**
 * Resolves OS architecture and application architecture based on platform and environment heuristics.
 * @param {{ baseArch?: any, platform?: string, env?: Record<string, string | undefined> }} options
 * @returns {{ appArch: string, arch: string }}
 */
export function resolveOsArch({ baseArch, platform, env = {} } = {}) {
  let arch = String(baseArch ?? "");
  if (platform === "win32" && env?.PROCESSOR_ARCHITEW6432 != null) {
    arch = "x64";
  }
  if (
    String(env?.PROCESSOR_ARCHITECTURE ?? "").toLowerCase() === "arm64" ||
    String(env?.PROCESSOR_ARCHITEW6432 ?? "").toLowerCase() === "arm64" ||
    String(env?.PROCESSOR_IDENTIFIER ?? "").toLowerCase().includes("arm")
  ) {
    arch = "arm64";
  }
  return { appArch: String(baseArch ?? ""), arch };
}

/**
 * MP4 clip processing and export error reason constants (RE-GAP-30 §5.1).
 * @readonly
 * @enum {string}
 */
export const DISCORD_MP4_ERROR_REASONS = Object.freeze({
  NOT_MP4_EXTENSION: "not-mp4-extension",
  NOT_MP4_SIGNATURE: "not-mp4-signature",
  MALFORMED_BOX: "malformed-box",
  NO_DISCORD_UUID_BOX: "no-discord-uuid-box",
  NO_METADATA: "no-metadata",
  RENAME_NOT_ALLOWED: "rename-not-allowed",
  OUTPUT_ALREADY_EXISTS: "output-already-exists",
  MALFORMED_RECIPE: "malformed-recipe",
});

/**
 * Formats a filesystem path as a disclip:// custom protocol URL (module 6752, RE-GAP-30 §5.2).
 * @param {string} filepath
 * @returns {string}
 */
export function formatClipProtocolUrl(filepath) {
  if (typeof filepath !== "string" || filepath.trim() === "") {
    throw new Error("Invalid file: not-mp4-extension");
  }
  const normalized = filepath.startsWith("/") ? filepath : "/" + filepath;
  return "disclip://disclip" + normalized.replace(/ /g, "%20");
}

/**
 * Parses an HTTP Range header for clip streaming requests (RE-GAP-30 §5.3-5.4).
 * @param {string | null | undefined} rangeHeader
 * @param {number} fileSize
 * @returns {{ status: number, start?: number, end?: number, contentLength?: number, contentRange?: string }}
 */
export function parseClipRangeHeader(rangeHeader, fileSize) {
  if (typeof fileSize !== "number" || !Number.isFinite(fileSize) || fileSize < 0) {
    return { status: 416, contentRange: "bytes */0" };
  }
  if (rangeHeader == null) {
    return {
      status: 200,
      start: 0,
      end: Math.max(0, fileSize - 1),
      contentLength: fileSize,
    };
  }
  const matches = /bytes=(\d+)?-(\d+)?/.exec(String(rangeHeader));
  if (!matches) {
    return { status: 416, contentRange: `bytes */${fileSize}` };
  }
  const start = matches[1] != null ? parseInt(matches[1], 10) : 0;
  const end = matches[2] != null ? parseInt(matches[2], 10) : fileSize - 1;
  if (Number.isNaN(start) || start > fileSize || end > fileSize || fileSize === 0) {
    return { status: 416, contentRange: `bytes */${fileSize}` };
  }
  return {
    status: 206,
    start,
    end,
    contentLength: end - start + 1,
    contentRange: `bytes ${start}-${end}/${fileSize}`,
  };
}

/**
 * WebAuthn convertible error codes mapped to DOMException (module 3797, RE-GAP-31 §3).
 * @readonly
 * @type {readonly string[]}
 */
export const WEBAUTHN_CONVERTIBLE_ERROR_CODES = Object.freeze([
  "EncodingError",
  "UnknownError",
  "NotAllowedError",
  "InvalidStateError",
  "NotSupportedError",
  "SecurityError",
  "SyntaxError",
  "NetworkError",
]);

/**
 * Parses WebAuthn native response and maps errors to DOMException or Error (RE-GAP-31 §3-§4).
 * @param {{ code?: any, message?: any }} response
 * @returns {any}
 */
export function parseWebAuthnNativeResponse(response) {
  if (response == null || typeof response !== "object") {
    throw new TypeError("response must be an object");
  }
  if (response.code == null) {
    return response.message;
  }
  const err = JSON.parse(String(response.message));
  if (WEBAUTHN_CONVERTIBLE_ERROR_CODES.includes(err.code)) {
    throw new DOMException(err.message, err.code);
  }
  // @ts-ignore Error cause support
  throw new Error(err.message, { cause: { code: err.code } });
}

/**
 * Forbidden frame debugger IPC channels guarded from renderer bridge exposure (RE-GAP-31 §5).
 * @readonly
 * @type {readonly string[]}
 */
export const FORBIDDEN_FRAME_DEBUGGER_CHANNELS = Object.freeze([
  "DISCORD_FRAME_DEBUGGER_ATTACH",
  "DISCORD_FRAME_DEBUGGER_RUN_BATCH",
  "DISCORD_FRAME_DEBUGGER_DETACH",
  "DISCORD_FRAME_DEBUGGER_DETACHED",
]);

/**
 * Clamps tracing duration in milliseconds between 1000 and 60000 ms (module 1742, RE-GAP-31 §5).
 * @param {number | string | null | undefined} [durationMs]
 * @returns {number}
 */
export function clampTracingDurationMs(durationMs) {
  const raw = durationMs ?? 30000;
  return Math.min(Math.max(Number(raw), 1000), 60000);
}

/**
 * Allowed desktop capture source types (module 7671, RE-GAP-31 §2).
 * @readonly
 * @type {readonly string[]}
 */
export const ALLOWED_DESKTOP_CAPTURE_TYPES = Object.freeze(["screen", "window"]);

/**
 * Normalizes desktop capture types array, filtering only valid types (module 7671, RE-GAP-31 §2).
 * @param {unknown} types
 * @returns {string[]}
 */
export function normalizeDesktopCaptureTypes(types) {
  if (!Array.isArray(types)) {
    return [];
  }
  return types.filter((t) => t === "screen" || t === "window");
}

/**
 * Allowed process environment variable keys projected to renderer (module 8434, RE-GAP-31 §11).
 * @readonly
 * @type {readonly string[]}
 */
export const ALLOWED_PROCESS_ENV_KEYS = Object.freeze([
  "LOCALAPPDATA",
  "XDG_CURRENT_DESKTOP",
  "XDG_SESSION_DESKTOP",
  "XDG_SESSION_TYPE",
  "DESKTOP_SESSION",
  "GDMSESSION",
  "SNAP",
  "FLATPAK_ID",
  "WAYLAND_DISPLAY",
]);

/**
 * Projects a raw environment object to a frozen object containing only allowed keys (module 8434, RE-GAP-31 §11).
 * @param {Record<string, unknown> | null | undefined} rawEnv
 * @returns {Readonly<Record<string, string | undefined>>}
 */
export function projectProcessEnv(rawEnv) {
  /** @type {Record<string, string | undefined>} */
  const projected = {};
  for (const k of ALLOWED_PROCESS_ENV_KEYS) {
    projected[k] =
      rawEnv != null && typeof rawEnv === "object" && typeof rawEnv[k] === "string"
        ? /** @type {string} */ (rawEnv[k])
        : undefined;
  }
  return Object.freeze(projected);
}

/**
 * Riot Live Client default origin (module 223, RE-GAP-31 §8).
 * @readonly
 * @type {string}
 */
export const RIOT_LIVE_CLIENT_ORIGIN = "https://127.0.0.1:2999";

/**
 * Builds and validates a Riot Live Client URL for a given endpoint (module 223, RE-GAP-31 §8).
 * @param {string} endpoint
 * @returns {string}
 */
export function buildRiotLiveClientUrl(endpoint) {
  if (
    typeof endpoint !== "string" ||
    !endpoint.startsWith("/liveclientdata/") ||
    endpoint.includes("..")
  ) {
    throw new RangeError("invalid Riot Live Client endpoint");
  }
  return `${RIOT_LIVE_CLIENT_ORIGIN}${endpoint}`;
}

/**
 * Blocked external URL protocols (splashScreenPreload.js modules 1331 & 85425).
 * @readonly
 * @type {readonly string[]}
 */
export const BLOCKED_EXTERNAL_URL_PROTOCOLS = Object.freeze([
  "file:",
  "javascript:",
  "vbscript:",
  "data:",
  "about:",
  "chrome:",
  "ms-cxh:",
  "ms-cxh-full:",
  "ms-word:",
  "shell:",
  "ms-msdt:",
  "search-ms:",
  "ms-officecmd:",
  "ms-appinstaller:",
]);

/**
 * Evaluates whether an external URL is safe to open (splashScreenPreload.js modules 1331 & 85425).
 * @param {unknown} externalUrl
 * @returns {boolean}
 */
export function shouldOpenExternalUrl(externalUrl) {
  if (typeof externalUrl !== "string") {
    return false;
  }
  try {
    const parsed = new URL(String(externalUrl));
    const protocol = parsed.protocol.toLowerCase();
    return (
      !BLOCKED_EXTERNAL_URL_PROTOCOLS.includes(protocol) &&
      !(protocol.replace(/:$/, "").length < 2)
    );
  } catch {
    return false;
  }
}

/**
 * Checks whether two URLs have matching protocol and host (splashScreenPreload.js modules 1331 & 85425).
 * @param {unknown} urlA
 * @param {unknown} urlB
 * @returns {boolean}
 */
export function checkUrlOriginMatches(urlA, urlB) {
  if (typeof urlA !== "string" || typeof urlB !== "string") {
    return false;
  }
  try {
    const parsedA = new URL(urlA);
    const parsedB = new URL(urlB);
    return parsedA.protocol === parsedB.protocol && parsedA.host === parsedB.host;
  } catch {
    return false;
  }
}

/**
 * Formats splash screen state status text (splashScreenPreload.js modules 1331 & 85425).
 * @param {Record<string, unknown> | null | undefined} state
 * @returns {string}
 */
export function formatSplashStatusText(state) {
  if (!state || typeof state !== "object") {
    return "Checking for updates\u2026";
  }
  switch (state.status) {
    case "installing-updates":
      return `Installing update ${Number(state.current ?? 0)} of ${Number(state.total ?? 0)}\u2026`;
    case "downloading-updates":
      return `Downloading update ${Number(state.current ?? 0)} of ${Number(state.total ?? 0)}\u2026`;
    case "update-failure":
      return `Update failed \u2014 retrying in ${Number(state.seconds ?? 0)} sec\u2026`;
    case "launching":
      return "Starting\u2026";
    default:
      return "Checking for updates\u2026";
  }
}

/**
 * Splash download options for manual installer selection (splashScreenPreload.js module 85425).
 * @type {readonly (Readonly<{ value: string, label: string }>)[]}
 */
export const SPLASH_DOWNLOAD_OPTIONS = Object.freeze([
  Object.freeze({ value: "deb", label: "Ubuntu (deb)" }),
  Object.freeze({ value: "tar.gz", label: "Linux (tar.gz)" }),
  Object.freeze({ value: "nope", label: "I'll figure it out" }),
]);

/**
 * Determines whether splash progress bar should be shown (splashScreenPreload.js module 85425).
 * @param {Record<string, unknown> | null | undefined} state
 * @returns {boolean}
 */
export function shouldShowSplashProgress(state) {
  return Boolean(
    state != null &&
      typeof state === "object" &&
      typeof state.progress === "number" &&
      (state.status === "downloading-updates" || state.status === "installing-updates")
  );
}

/**
 * Checks if bootstrap updater should be disabled (module 55405).
 * @param {Record<string, unknown> | null | undefined} buildInfo
 * @returns {boolean}
 */
export function shouldDisableBootstrapUpdater(buildInfo) {
  return Boolean(
    buildInfo != null &&
      typeof buildInfo === "object" &&
      (buildInfo.disableUpdater ||
        (buildInfo.releaseChannel === "development" &&
          (buildInfo.standaloneModules || buildInfo.localModulesRoot != null)))
  );
}

/**
 * Resolves bootstrap application identity constants based on release channel (app-asar/bundle.js module 19192).
 * @param {string | null | undefined} releaseChannel
 * @returns {Readonly<{
 *   appCompany: string,
 *   appDescription: string,
 *   appName: string,
 *   appNameForHumans: string,
 *   appId: string,
 *   appProtocol: string,
 *   newUpdateEndpoint: string
 * }>}
 */
export function resolveBootstrapAppIdentity(releaseChannel) {
  const ch = typeof releaseChannel === "string" && releaseChannel.length > 0 ? releaseChannel : "stable";
  const suffix = ch === "stable" ? "" : ch.charAt(0).toUpperCase() + ch.slice(1);
  const appName = "Discord" + suffix;
  return Object.freeze({
    appCompany: "Discord Inc",
    appDescription: "Discord - https://discord.com",
    appName,
    appNameForHumans: "Discord" + (suffix !== "" ? " " + suffix : ""),
    appId: "com.squirrel." + appName + "." + appName,
    appProtocol: "Chord",
    newUpdateEndpoint: "https://updates.discord.com/",
  });
}

/**
 * Expected binary size of raw code integrity payload (app-asar/bundle.js module 55264).
 */
export const CODE_INTEGRITY_BYTE_SIZE = 76;

/**
 * Minidump user stream type identifier for Discord code integrity records (app-asar/bundle.js module 55264).
 */
export const CODE_INTEGRITY_STREAM_TYPE = 0xd15c0d01;

/**
 * Constructs Sentry crash reporter tags based on code integrity inspection data (app-asar/bundle.js module 55264).
 * @param {Record<string, any> | null | undefined} codeIntegrity
 * @returns {Readonly<Record<string, string>>}
 */
export function buildCodeIntegritySentryTags(codeIntegrity) {
  if (codeIntegrity == null || typeof codeIntegrity !== "object") {
    return Object.freeze({ code_integrity: "unavailable" });
  }

  if (codeIntegrity.bytesDifferent === 0) {
    return Object.freeze({ code_integrity: "clean" });
  }

  /** @type {string} */
  let distance;
  if (codeIntegrity.nearestDifferenceFromInstructionPointer == null) {
    distance = "unknown";
  } else {
    const abs = Math.abs(Number(codeIntegrity.nearestDifferenceFromInstructionPointer));
    distance = abs <= 64 ? "near" : abs <= 4096 ? "within_page" : "distant";
  }

  return Object.freeze({
    code_integrity: "modified",
    "code_integrity.distance": distance,
  });
}

/**
 * Resolves the filesystem path used for storing/loading bootstrap modules (app-asar/bundle.js module 37948).
 * @param {string} userDataPath
 * @param {string} resourcesPath
 * @param {Record<string, any> | null | undefined} buildInfo
 * @returns {string}
 */
export function determineBootstrapModuleDataPath(userDataPath, resourcesPath, buildInfo) {
  if (buildInfo != null && typeof buildInfo === "object") {
    if (buildInfo.localModulesRoot != null) {
      return String(buildInfo.localModulesRoot);
    }
    if (buildInfo.standaloneModules) {
      return `${resourcesPath}/standalone_modules`;
    }
  }
  return `${userDataPath}/module_data`;
}

export const OLD_VERSION_DIR_REGEX = /^[0-9]+\.[0-9]+\.[0-9]+/;

/**
 * Determines whether an old version directory entry should be cleaned up (module 37948 cleanOldVersions).
 * @param {any} entryName
 * @param {any} currentVersion
 * @returns {boolean}
 */
export function shouldRemoveOldVersionDirectory(entryName, currentVersion) {
  if (typeof entryName !== "string" || typeof currentVersion !== "string" || currentVersion.length === 0) {
    return false;
  }
  return !entryName.includes(currentVersion) && OLD_VERSION_DIR_REGEX.test(entryName);
}

/**
 * Builds the code integrity context payload for Sentry (module 55264).
 * @param {any} codeIntegrity
 * @returns {Record<string, any> | null}
 */
export function buildCodeIntegritySentryContext(codeIntegrity) {
  if (codeIntegrity == null || typeof codeIntegrity !== "object") {
    return null;
  }
  return {
    bytes_different: codeIntegrity.bytesDifferent ?? 0,
    bytes_compared: codeIntegrity.bytesCompared ?? 0,
    nearest_difference_from_instruction_pointer: codeIntegrity.nearestDifferenceFromInstructionPointer ?? null,
    reloc_bytes_different: codeIntegrity.relocBytesDifferent ?? 0,
    reloc_bytes_compared: codeIntegrity.relocBytesCompared ?? 0,
    bytes_skipped: codeIntegrity.bytesSkipped ?? 0,
    is_crash_page_private: codeIntegrity.isCrashPagePrivate ?? null,
    are_relocations_applied: Boolean(codeIntegrity.areRelocationsApplied),
    flags: codeIntegrity.flags ?? 0,
  };
}

/**
 * Resolves standard bootstrap paths given user data root, release channel and version (module 37948).
 * @param {string} userDataRoot
 * @param {string | null | undefined} releaseChannel
 * @param {string} version
 * @returns {{ userDataPath: string, userDataVersionedPath: string, assetCachePath: string, logPath: string }}
 */
export function resolveBootstrapPaths(userDataRoot, releaseChannel, version) {
  const ch = typeof releaseChannel === "string" && releaseChannel.length > 0 ? releaseChannel.toLowerCase() : "stable";
  const userDataPath = `${userDataRoot}/discord${ch === "stable" ? "" : ch}`;
  return {
    userDataPath,
    userDataVersionedPath: `${userDataPath}/${version}`,
    assetCachePath: `${userDataPath}/discord_asset_cache`,
    logPath: `${userDataPath}/logs`,
  };
}

/**
 * Desktop TTI warmup domains used for early DNS resolution and socket preconnection (module 72100).
 */
export const DESKTOP_TTI_WARMUP_DOMAINS = Object.freeze([
  "discord.com",
  "cdn.discordapp.com",
  "gateway.discord.gg",
  "updates.discord.com",
]);

/**
 * Common errors ignored by crash reporter and Sentry client (module 62457).
 */
export const SENTRY_IGNORE_ERRORS = Object.freeze([
  "EADDRINUSE",
  "ResizeObserver loop limit exceeded",
  "ResizeObserver loop completed with undelivered notifications.",
  "EACCES: permission denied",
  "BetterDiscord",
  "VencordPatcher",
  "mwittrien.github.io",
  "Error: getaddrinfo ENOTFOUND raw.githubusercontent.com",
]);

/**
 * Builds standard Sentry DSN given public client key (module 62457).
 * @param {string | null | undefined} dsnKey
 * @returns {string | null}
 */
export function buildSentryDsn(dsnKey) {
  return typeof dsnKey === "string" && dsnKey.length > 0
    ? `https://${dsnKey}@o64374.ingest.sentry.io/146342`
    : null;
}

/**
 * Resolves Sentry error sample rate based on platform and release channel (module 62457).
 * Linux, macOS, and pre-release channels use sampleRate 1.0, while Windows stable uses 0.01.
 * @param {string | null | undefined} platform
 * @param {string | null | undefined} releaseChannel
 * @returns {number}
 */
export function resolveSentrySampleRate(platform, releaseChannel) {
  return platform === "linux" ||
    platform === "darwin" ||
    releaseChannel === "ptb" ||
    releaseChannel === "canary" ||
    releaseChannel === "development"
    ? 1
    : 0.01;
}

/**
 * Detects Linux packaging runtime environment from process.env (module 62457).
 * @param {unknown} env
 * @returns {"flatpak" | "snap" | "appimage" | "native"}
 */
export function detectLinuxRuntimeEnvironment(env) {
  if (env == null || typeof env !== "object") {
    return "native";
  }
  /** @type {any} */
  const envObj = env;
  if (envObj.FLATPAK_ID) {
    return "flatpak";
  }
  if (envObj.SNAP || envObj.SNAP_NAME) {
    return "snap";
  }
  if (envObj.APPIMAGE || envObj.APPDIR) {
    return "appimage";
  }
  return "native";
}

export const DESKTOP_ANALYTICS_CACHE_FILENAME = "desktop_analytics_cache.json";
export const DESKTOP_ANALYTICS_CACHE_TTL_MS = 1800000;
export const DESKTOP_TTI_EVENT_TYPES = Object.freeze([
  "mainapp_init",
  "splash_created",
  "splash_duration",
  "splash_duration_with_updates",
  "splash_restart",
  "mainwin_created",
  "mainwin_loadstart",
  "mainwin_loadcomplete",
  "mainwin_loadjsappcomplete",
  "mainwin_interactive_jsappcomplete",
  "full_tti_complete",
  "full_tti_with_restart_complete",
  "full_interactive_tti_complete",
  "full_interactive_tti_with_restart_complete",
]);

/**
 * Computes restart and full TTI duration if within TTL bounds (module 45152).
 * @param {unknown} nowMs
 * @param {unknown} splashRestartTimepointMs
 * @param {unknown} prevProcessDurationMs
 * @returns {number | null}
 */
export function computeRestartAndFullTtiDuration(nowMs, splashRestartTimepointMs, prevProcessDurationMs) {
  if (
    typeof nowMs !== "number" ||
    !Number.isFinite(nowMs) ||
    typeof splashRestartTimepointMs !== "number" ||
    !Number.isFinite(splashRestartTimepointMs) ||
    typeof prevProcessDurationMs !== "number" ||
    !Number.isFinite(prevProcessDurationMs)
  ) {
    return null;
  }
  const sinceRestart = nowMs - splashRestartTimepointMs;
  const full = sinceRestart + prevProcessDurationMs;
  return sinceRestart < 0 ||
    sinceRestart > DESKTOP_ANALYTICS_CACHE_TTL_MS ||
    full < 0 ||
    full > DESKTOP_ANALYTICS_CACHE_TTL_MS
    ? null
    : full;
}

/**
 * Builds Windows autostart command line arguments (module 74332).
 * @param {string} targetExe
 * @param {string} fullExeName
 * @param {boolean} startMinimized
 * @returns {string}
 */
export function buildWindowsAutostartCommand(targetExe, fullExeName, startMinimized) {
  return `"${targetExe}" --processStart ${fullExeName} --process-start-args "${startMinimized ? "--start-inactive --start-minimized" : "--start-inactive"}"`;
}

export const DNS_TELEMETRY_BASE_DOMAIN = "dc-telemetry.net";
export const BLACKBOX_MAX_MINIDUMP_AGE_MS = 60000;
export const BLACKBOX_MAX_STORED_MINIDUMPS = 10;

/**
 * Normalizes a DNS telemetry label (module 65445).
 * @param {any} value
 * @returns {string | null}
 */
export function normalizeDnsLabel(value) {
  if (typeof value !== "string" || value.length === 0) {
    return null;
  }
  const norm = value.toLowerCase().replace(/[^a-z0-9-]/g, "-");
  if (norm.length > 63 || norm.startsWith("-") || norm.endsWith("-")) {
    return null;
  }
  return norm.replace(/-+/g, "-");
}

/**
 * Builds DNS telemetry FQDN from labels (module 65445).
 * @param {unknown} labels
 * @param {unknown} [baseDomain]
 * @param {unknown} [version]
 * @returns {string}
 */
export function buildDnsTelemetryFqdn(labels, baseDomain = DNS_TELEMETRY_BASE_DOMAIN, version = "v1") {
  if (!Array.isArray(labels) || labels.length === 0) {
    return String(baseDomain);
  }
  const suffix = `${version}.${baseDomain}`;
  let curLen = suffix.length;
  let count = 0;
  for (let i = labels.length - 1; i >= 0; i--) {
    const next = curLen + 1 + String(labels[i]).length;
    if (next > 253) {
      break;
    }
    curLen = next;
    count++;
  }
  return count === 0 ? suffix : `${labels.slice(0, count).join(".")}.${suffix}`;
}

/**
 * Computes scheduled backoff step with optional jitter (module 1447).
 * @param {unknown} steps
 * @param {unknown} failCount
 * @param {unknown} [randomVal]
 * @returns {number | null}
 */
export function computeScheduledBackoffStep(steps, failCount, randomVal = null) {
  if (!Array.isArray(steps) || steps.length === 0 || typeof failCount !== "number" || failCount < 1) {
    return null;
  }
  const idx = Math.min(Math.floor(failCount) - 1, steps.length - 1);
  let delay = steps[idx];
  if (typeof randomVal === "number" && typeof delay === "number") {
    delay = Math.round(delay * (0.85 + 0.3 * Math.max(0, Math.min(1, randomVal))));
  }
  return delay ?? null;
}

/**
 * Startup command line switches recognized by Squirrel.Windows (module 27251).
 */
export const SQUIRREL_STARTUP_COMMANDS = Object.freeze([
  "--squirrel-install",
  "--squirrel-updated",
  "--squirrel-uninstall",
  "--squirrel-obsolete",
  "--squirrel-firstrun",
]);

/**
 * Magic header byte signature of a Windows Shell Link (.lnk) file (module 27543).
 */
export const SHELL_LINK_HEADER_BYTES = Object.freeze([
  76, 0, 0, 0, 1, 20, 2, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 70,
]);

/**
 * Privileged scheme configuration for disclip:// protocol (module 46312).
 */
export const DISCLIP_PRIVILEGED_SCHEME = Object.freeze({
  scheme: "disclip",
  privileges: Object.freeze({
    standard: true,
    secure: true,
    corsEnabled: true,
    supportFetchAPI: true,
    stream: true,
  }),
});

/**
 * Verifies if the provided bytes buffer starts with the Windows Shell Link magic header (module 27543).
 * @param {unknown} bytes
 * @returns {boolean}
 */
export function isWindowsShellLinkBytes(bytes) {
  if (!(bytes instanceof Uint8Array || Array.isArray(bytes)) || bytes.length < SHELL_LINK_HEADER_BYTES.length) {
    return false;
  }
  for (let i = 0; i < SHELL_LINK_HEADER_BYTES.length; i++) {
    if (bytes[i] !== SHELL_LINK_HEADER_BYTES[i]) {
      return false;
    }
  }
  return true;
}

/**
 * Resolves platform standard accelerator for Quit action (modules 19725, 75150, 40467).
 * @param {unknown} platform
 * @returns {string}
 */
export function resolvePlatformQuitAccelerator(platform) {
  return platform === "darwin" ? "Command+Q" : platform === "linux" ? "Control+Q" : "Alt+F4";
}

/**
 * Scheduled backoff step delays in ms for splash screen update attempts (module 53006).
 */
export const SPLASH_SCHEDULED_BACKOFF_STEPS_MS = Object.freeze([
  2000, 3000, 3000, 4000, 5000, 7000, 10000, 12000, 15000,
]);

/**
 * Obsoleted settings keys cleaned up on startup (module 70447).
 */
export const OBSOLETED_SETTINGS_KEYS = Object.freeze([
  "USE_NEW_UPDATER",
  "USE_RUST_BSPATCH",
]);

/**
 * Resolves splash window dimension and format configurations (module 53006).
 * @param {string} platform
 * @param {boolean} [useWebp]
 * @returns {Readonly<{ width: number, height: number, mediaFormatArg: string }>}
 */
export function resolveSplashWindowConfig(platform, useWebp = false) {
  return Object.freeze({
    width: 300,
    height: platform === "darwin" ? 300 : 350,
    mediaFormatArg: `--splash-media-format=${useWebp === true ? "webp" : "webm"}`,
  });
}

/**
 * Formats connectivity error message for DNS reporter / telemetry (module 51937).
 * @param {string} message
 * @returns {string}
 */
export function formatConnectivityErrorMessage(message) {
  if (typeof message !== "string" || message.length === 0) {
    return "unknown";
  }
  const match = message.match(/net::?ERR_([A-Z_]+)/);
  if (match === null || !match[1]) {
    return message
      .toLowerCase()
      .replace(/[^a-z0-9]/g, "-")
      .replace(/-+/g, "-")
      .replace(/^-|-$/g, "")
      .substring(0, 55);
  }
  let abbreviated = match[1].toLowerCase().replace(/_/g, "-");
  /** @type {Record<string, string>} */
  const abbreviations = {
    connection: "conn",
    certificate: "cert",
    authentication: "auth",
    aborted: "abort",
    network: "net",
    invalid: "inv",
    response: "resp",
    request: "req",
    address: "addr",
    unreachable: "unreach",
  };
  for (const [full, abbr] of Object.entries(abbreviations)) {
    abbreviated = abbreviated.replace(full, abbr);
  }
  return abbreviated.substring(0, 55);
}

/**
 * Base list of disabled Chromium features across all platforms (module 23151).
 */
export const DISABLED_CHROMIUM_FEATURES_BASE = Object.freeze([
  "WinRetrieveSuggestionsOnlyOnDemand",
  "HardwareMediaKeyHandling",
  "MediaSessionService",
  "UseEcoQoSForBackgroundProcess",
  "IntensiveWakeUpThrottling",
  "AllowAggressiveThrottlingWithWebSocket",
]);

/**
 * List of ScreenCaptureKit disabled Chromium features for macOS before Darwin 24 (module 23151).
 */
export const DISABLED_CHROMIUM_FEATURES_DARWIN_PRE24 = Object.freeze([
  "ScreenCaptureKitMac",
  "ScreenCaptureKitMacWindow",
  "ScreenCaptureKitMacScreen",
  "ScreenCaptureKitPickerScreen",
  "ScreenCaptureKitStreamPickerSonoma",
  "WarmScreenCaptureSonoma",
  "UseSCContentSharingPicker",
]);

/**
 * Cookie name used for Build Override testing in Discord client (module 96020).
 */
export const BUILD_OVERRIDE_COOKIE_NAME = "buildOverride";

/**
 * Builds the array of disabled Chromium features based on platform and OS version (module 23151).
 * @param {string} platform
 * @param {number | string} [darwinMajorRelease=24]
 * @returns {ReadonlyArray<string>}
 */
export function buildDisabledChromiumFeatures(platform, darwinMajorRelease = 24) {
  /** @type {string[]} */
  const features = [...DISABLED_CHROMIUM_FEATURES_BASE];
  if (platform === "darwin" && Number(darwinMajorRelease) < 24) {
    features.push(...DISABLED_CHROMIUM_FEATURES_DARWIN_PRE24);
  }
  if (platform === "linux") {
    features.push("WaylandWpColorManagerV1");
  }
  return Object.freeze(features);
}

/**
 * Extracts URL from bootstrap process arguments (module 23151).
 * @param {string[]} args
 * @param {string} [appProtocol="discord"]
 * @returns {string | null}
 */
export function extractUrlFromBootstrapArgs(args, appProtocol = "discord") {
  if (!Array.isArray(args)) {
    return null;
  }
  const urlArgIndex = args.indexOf("--url");
  if (urlArgIndex >= 0) {
    const passIdx = args.indexOf("--");
    if (passIdx < 0 || passIdx < urlArgIndex) {
      return null;
    }
    const targetUrl = args[passIdx + 1];
    return typeof targetUrl === "string" ? targetUrl : null;
  }
  const prefix = `${String(appProtocol).toLowerCase()}://`;
  for (let i = 1; i < args.length; i++) {
    const arg = args[i];
    if (typeof arg === "string" && arg.toLowerCase().startsWith(prefix)) {
      return arg;
    }
  }
  return null;
}

/**
 * Builds removal URL for Build Override cookie (module 96020).
 * @param {{ domain?: string | null, secure?: boolean | null, path?: string | null } | null | undefined} cookie
 * @returns {string}
 */
export function buildCookieRemovalUrl(cookie) {
  if (!cookie || typeof cookie.domain !== "string" || cookie.domain.length === 0) {
    throw new Error("Build override cookie has no domain");
  }
  const protocol = cookie.secure ? "https" : "http";
  const domain = cookie.domain.startsWith(".") ? cookie.domain.slice(1) : cookie.domain;
  const path = cookie.path != null ? cookie.path : "/";
  return `${protocol}://${domain}${path}`;
}

/**
 * Standard comment for Linux desktop autostart entry (module 53657).
 */
export const LINUX_AUTOSTART_COMMENT = "Text and voice chat for gamers.";

/**
 * Maximum number of handled bootstrap errors reported to Sentry (module 37079).
 */
export const MAX_HANDLED_BOOTSTRAP_ERRORS = 10;

/**
 * Formats Linux XDG autostart .desktop file content (module 53657).
 * @param {string} appName
 * @param {string} exePath
 * @param {string} iconPath
 * @returns {string}
 */
export function formatLinuxAutostartDesktopEntry(appName, exePath, iconPath) {
  return (
    "[Desktop Entry]\n" +
    "Type=Application\n" +
    "Exec=" + exePath + "\n" +
    "Hidden=false\n" +
    "NoDisplay=false\n" +
    "Name=" + appName + "\n" +
    "Icon=" + iconPath + "\n" +
    "Comment=" + LINUX_AUTOSTART_COMMENT + "\n" +
    "X-GNOME-Autostart-enabled=true\n"
  );
}

/**
 * Formats arguments for reg.exe add command (module 59311).
 * @param {unknown[]} [rawArgs]
 * @returns {readonly string[]}
 */
export function formatWindowsRegAddArgs(rawArgs) {
  return Object.freeze(["add", ...(Array.isArray(rawArgs) ? rawArgs.map(String) : []), "/f"]);
}

/**
 * Evaluates whether handled non-fatal bootstrap error should be reported to Sentry (module 37079).
 * @param {string} releaseChannel
 * @param {unknown} errorMessage
 * @param {number} randomPermille
 * @param {number} totalHandledErrors
 * @param {number} handledErrorCounter
 * @returns {boolean}
 */
export function shouldReportHandledBootstrapError(
  releaseChannel,
  errorMessage,
  randomPermille,
  totalHandledErrors,
  handledErrorCounter
) {
  if (
    releaseChannel !== "ptb" &&
    releaseChannel !== "canary" &&
    releaseChannel !== "development"
  ) {
    return false;
  }
  if (
    typeof errorMessage === "string" &&
    (errorMessage.includes("cert_chain_failed") ||
      errorMessage.includes("network_error") ||
      errorMessage.includes("storage_error")) &&
    randomPermille > 1
  ) {
    return false;
  }
  return totalHandledErrors < MAX_HANDLED_BOOTSTRAP_ERRORS && handledErrorCounter % 3 === 0;
}

/**
 * Cache filename for updater events (module 47421).
 */
export const UPDATER_EVENT_CACHE_FILENAME = "updater_events.json";

/**
 * Error identifier for inconsistent installer state (module 47421).
 */
export const UPDATER_INCONSISTENT_INSTALLER_STATE_ERROR = "InconsistentInstallerState";

/**
 * Frozen array of native updater task states (module 47421).
 * @type {readonly ["Complete", "Failed", "Waiting", "Working"]}
 */
export const UPDATER_TASK_STATES = Object.freeze(["Complete", "Failed", "Waiting", "Working"]);

/**
 * 15 error classification rules matching module 47421 in app-asar/bundle.js.
 * @type {readonly { substring: string, category: string, message: string }[]}
 */
const NATIVE_UPDATER_ERROR_RULES = Object.freeze([
  {
    substring: "code: 10013",
    category: "network_error",
    message: "An attempt was made to access a socket in a way forbidden by its access permissions",
  },
  {
    substring: "code: 10054",
    category: "network_error",
    message: "An existing connection was forcibly closed by the remote host",
  },
  {
    substring: "code: 10055",
    category: "network_error",
    message:
      "An operation on a socket could not be performed because the system lacked sufficient buffer space or because a queue was full",
  },
  {
    substring: "code: 10060",
    category: "network_error",
    message:
      "A connection attempt failed because the connected party did not properly respond after a period of time",
  },
  {
    substring: "code: 10061",
    category: "network_error",
    message: "No connection could be made because the target machine actively refused it",
  },
  {
    substring: "code: 11001",
    category: "network_error",
    message: "No such host is known",
  },
  {
    substring: "code: 11002",
    category: "network_error",
    message:
      "This is usually a temporary error during hostname resolution and means that the local server did not receive a response from an authoritative server",
  },
  {
    substring: "code: 11004",
    category: "network_error",
    message: "The requested name is valid, but no data of the requested type was found",
  },
  {
    substring: "kind: Status(500)",
    category: "network_error",
    message: "Status 500",
  },
  {
    substring: "kind: Status(502)",
    category: "network_error",
    message: "Status 502",
  },
  {
    substring: "kind: UnexpectedEof",
    category: "network_error",
    message: "Unexpected EOF during handshake",
  },
  {
    substring: "code: -2146762487",
    category: "cert_chain_failed",
    message: "Cert chain processed, root cert not trusted by trust provider",
  },
  {
    substring: "code: -2146893018",
    category: "cert_chain_failed",
    message: "The message received was unexpected or badly formatted",
  },
  {
    substring: "code: -2146762481",
    category: "cert_chain_failed",
    message: "The certificate's CN name does not match the passed value",
  },
  {
    substring: "kind: StorageFull",
    category: "storage_error",
    message: "Storage full",
  },
]);

/**
 * Classifies a native updater error kind and details against 15 substring rules (module 47421).
 * @param {string} kind
 * @param {unknown} details
 * @returns {string}
 */
export function classifyNativeUpdaterError(kind, details) {
  const d = String(details ?? "");
  for (const rule of NATIVE_UPDATER_ERROR_RULES) {
    if (d.includes(rule.substring)) {
      return `(${kind}) (${rule.category}): ${rule.message}`;
    }
  }
  return `(${kind}) ${details}`;
}

/**
 * Builds legacy updater feed URL and remote platform descriptor (module 47924).
 * @param {string} endpoint
 * @param {string} releaseChannel
 * @param {string} platform
 * @param {string} version
 * @returns {Readonly<{ feedUrl: string, remotePlatform: string }> | null}
 */
export function buildLegacyUpdaterFeedUrl(endpoint, releaseChannel, platform, version) {
  switch (platform) {
    case "darwin":
      return Object.freeze({
        feedUrl: `${endpoint}/updates/${releaseChannel}?platform=osx&version=${version}`,
        remotePlatform: "osx",
      });
    case "win32":
      return Object.freeze({
        feedUrl: `${endpoint}/updates/${releaseChannel}`,
        remotePlatform: "win",
      });
    case "linux":
      return Object.freeze({
        feedUrl: `${endpoint}/updates/${releaseChannel}?platform=linux&version=${version}`,
        remotePlatform: "linux",
      });
    default:
      return null;
  }
}

/**
 * Maximum held events count for frame debugger CDP session buffer (module 5490).
 */
export const FRAME_DEBUGGER_MAX_HELD_EVENTS = 200;

/**
 * Minimum macOS Darwin kernel major version required for notifications (module 1279).
 */
export const MIN_MACOS_KERNEL_MAJOR = 21;

/**
 * Allowed CDP methods for frame debugger (modules 5490, 7026).
 * @type {readonly string[]}
 */
export const FRAME_DEBUGGER_ALLOWED_METHODS = Object.freeze([
  "Runtime.evaluate",
  "Runtime.callFunctionOn",
  "Runtime.releaseObjectGroup",
  "DOM.describeNode",
  "DOM.querySelectorAll",
  "DOM.getBoxModel",
  "DOM.scrollIntoViewIfNeeded",
  "Accessibility.getFullAXTree",
  "Page.navigate",
  "Page.reload",
  "Input.dispatchMouseEvent",
  "Input.dispatchKeyEvent",
  "Input.insertText",
]);

/**
 * Denied CDP method prefixes for frame debugger (module 5490).
 * @type {readonly string[]}
 */
export const FRAME_DEBUGGER_DENIED_PREFIXES = Object.freeze([
  "Target.",
  "Storage.",
  "DOMStorage.",
  "Browser.",
  "SystemInfo.",
  "Network.",
  "Fetch.",
  "Emulation.",
]);

/**
 * Denied CDP methods for frame debugger (module 5490).
 * @type {readonly string[]}
 */
export const FRAME_DEBUGGER_DENIED_METHODS = Object.freeze([
  "DOM.setFileInputFiles",
  "DOM.getFileInfo",
  "Page.addScriptToEvaluateOnNewDocument",
  "Page.removeScriptToEvaluateOnNewDocument",
  "Runtime.addBinding",
]);

/**
 * Validates and checks a CDP method invocation for frame debugger (module 5490).
 * @param {string} method
 * @returns {string | null}
 */
export function checkFrameDebuggerCdpMethod(method) {
  if (typeof method !== "string" || method.length === 0) {
    return "FRAME_DEBUGGER_BAD_METHOD";
  }
  if (
    FRAME_DEBUGGER_DENIED_METHODS.includes(method) ||
    FRAME_DEBUGGER_DENIED_PREFIXES.some((prefix) => method.startsWith(prefix))
  ) {
    return `FRAME_DEBUGGER_METHOD_DENIED: ${method}`;
  }
  if (FRAME_DEBUGGER_ALLOWED_METHODS.includes(method)) {
    return null;
  }
  return `FRAME_DEBUGGER_METHOD_NOT_ALLOWED: ${method}`;
}

/**
 * Classifies an Electron process metric type into normalized CPU bucket (module 9984).
 * @param {unknown} processType
 * @returns {string}
 */
export function classifyElectronProcessMetricType(processType) {
  switch (processType) {
    case "Browser":
      return "main";
    case "Tab":
      return "renderer";
    case "GPU":
      return "gpu";
    case "Utility":
    case "Zygote":
    case "Sandbox helper":
    case "Pepper Plugin":
    case "Pepper Plugin Broker":
      return "utility";
    default:
      return "unknown";
  }
}

/**
 * Parses Darwin kernel major version from release string (module 1279).
 * @param {unknown} release
 * @returns {number}
 */
export function parseDarwinKernelMajor(release) {
  const major = Number.parseInt(String(release ?? "").split(".")[0] ?? "", 10);
  return Number.isNaN(major) || major < 0 ? 0 : major;
}

/**
 * WebAuthn macOS Keychain Access Group (module 5544).
 */
export const WEBAUTHN_MACOS_KEYCHAIN_ACCESS_GROUP = "53Q6R32WPB.com.hnc.agekeys";

/**
 * Root CDP methods allowed directly on root frame debugger session (module 7026).
 * @type {readonly string[]}
 */
export const FRAME_DEBUGGER_ROOT_METHODS = Object.freeze([
  "Target.getTargetInfo",
  "Target.setDiscoverTargets",
  "Target.attachToTarget",
]);

/**
 * Redacts and normalizes URLs captured inside frame debugger sessions (module 5490).
 * @param {string} url
 * @param {string} frameOrigin
 * @returns {string}
 */
export function redactFrameDebuggerUrl(url, frameOrigin) {
  try {
    const parsed = new URL(url, frameOrigin);
    if (parsed.protocol === "data:" || parsed.protocol === "blob:") {
      return parsed.protocol;
    }
    /** @type {string[]} */
    const names = [];
    parsed.searchParams.forEach((_value, name) => names.push(`${name}=\u{2026}`));
    const pathname = parsed.pathname
      .split("/")
      .map((segment) => (/^(?=.*[A-Za-z])(?=.*\d)[\w.~-]{24,}$/.test(segment) ? ":redacted" : segment))
      .join("/");
    const path = parsed.origin === frameOrigin ? pathname : `${parsed.protocol}//${parsed.host}${pathname}`;
    return (names.length > 0 ? `${path}?${names.join("&")}` : path).slice(0, 300);
  } catch {
    return "(unparsed url)";
  }
}

/**
 * Validates if an origin is an allowed activity frame origin (module 7026).
 * @param {unknown} origin
 * @param {boolean} [allowDevHosts=false]
 * @returns {boolean}
 */
export function isValidActivityFrameOrigin(origin, allowDevHosts = false) {
  try {
    const parsed = new URL(String(origin ?? ""));
    if (parsed.origin !== origin) {
      return false;
    }
    const isProd =
      parsed.protocol === "https:" &&
      /^\d{17,20}\.(?:[a-z0-9-]+\.)?discordsays\.com$/.test(parsed.hostname);
    const isDev =
      Boolean(allowDevHosts) &&
      /^\d{17,20}(?:\.|--)[a-z0-9.-]+$/.test(parsed.hostname);
    return isProd || isDev;
  } catch {
    return false;
  }
}

/**
 * Allowed media origins for age verification and external media embeds (module 6605).
 * @readonly
 * @type {Readonly<{ K_ID: string, K_ID_V2: string, K_ID_FACE_SCAN: string }>}
 */
export const ALLOWED_MEDIA_ORIGINS = Object.freeze({
  K_ID: "https://d3ogqhtsivkon3.cloudfront.net",
  K_ID_V2: "https://family.k-id.com",
  K_ID_FACE_SCAN: "https://age-verification-k-id.privately.swiss",
});

/**
 * Resolves Windows taskbar overlay badge index and description (module 6820).
 * @param {number} count
 * @returns {Readonly<{ index: number | null, description: string }>}
 */
export function resolveWindowsAppBadge(count) {
  if (count === -1) {
    return Object.freeze({ index: 10, description: "Unread messages" });
  }
  if (count === 0) {
    return Object.freeze({ index: null, description: "No Notifications" });
  }
  const index = Math.max(1, Math.min(count, 10)) - 1;
  return Object.freeze({ index, description: `${index} notifications` });
}

/**
 * Parses an HTTP Range header for clip byte requests (module 7635).
 * @param {string | null | undefined} rangeHeader
 * @param {number} fileSize
 * @returns {Readonly<{ start: number, end: number }> | null}
 */
export function parseClipHttpByteRange(rangeHeader, fileSize) {
  const matches = /bytes=(\d+)?-(\d+)?/.exec(String(rangeHeader ?? ""));
  if (matches == null || typeof fileSize !== "number" || fileSize < 0) {
    return null;
  }
  const start = matches[1] != null ? Number.parseInt(matches[1], 10) : 0;
  const end = matches[2] != null ? Number.parseInt(matches[2], 10) : fileSize - 1;
  if (Number.isNaN(start) || Number.isNaN(end) || end < 0 || start > fileSize || end > fileSize) {
    return null;
  }
  return Object.freeze({ start, end });
}

/**
 * NTPv4 packet size in bytes (RFC 5905, module 1830).
 */
export const NTP_PACKET_SIZE_BYTES = 48;

/**
 * Seconds difference between NTP epoch (1900-01-01) and Unix epoch (1970-01-01) (module 1830).
 */
export const NTP_EPOCH_OFFSET_SECONDS = 0x83aa7e80;

/**
 * Allowed BrowserWindow feature keys for popout windows (module 9698).
 * @readonly
 * @type {readonly string[]}
 */
export const POPOUT_ALLOWED_FEATURES = Object.freeze([
  "width",
  "height",
  "left",
  "top",
  "resizable",
  "movable",
  "alwaysOnTop",
  "frame",
  "transparent",
  "hasShadow",
  "closable",
  "skipTaskbar",
  "backgroundColor",
  "menubar",
  "toolbar",
  "location",
  "directories",
  "titleBarStyle",
  "outOfProcessOverlay",
  "focusable",
]);

/**
 * Key name used for the staff DevTools bundle activation setting (module 4520).
 */
export const STAFF_DEV_TOOLS_KEY = "ENABLE_STAFF_DEV_TOOLS_BUNDLE";

/**
 * Base64-encoded Ed25519 public key used for verifying staff DevTools bundle signature (module 4520).
 */
export const STAFF_DEV_TOOLS_PUBLIC_KEY_B64 = "MCowBQYDK2VwAyEAIoaTnSgNjloxqzHKGBXIp0F7JX3AaZKZCocqMBpSi/A=";

/**
 * Cooldown period in milliseconds for reporting non-renderer crashes (module 5203).
 */
export const NON_RENDERER_CRASH_COOLDOWN_MS = 30000;

/**
 * Utility process exit code to be ignored from crash reporting (STATUS_CONTROL_C_EXIT, module 5203).
 */
export const UTILITY_IGNORED_CRASH_EXIT_CODE = -1073741205;

/**
 * Formats canonical payload string for Ed25519 staff DevTools bundle signature verification (module 4520).
 * @param {unknown} version - Semantic version string (e.g. "1.0.9001").
 * @param {unknown} hash - 64-character hex hash string.
 * @returns {string | null} Formatted payload or null if invalid inputs.
 */
export function formatStaffDevToolsSignaturePayload(version, hash) {
  if (typeof version !== "string" || !/^\d+\.\d+\.\d+$/.test(version)) {
    return null;
  }
  if (typeof hash !== "string" || !/^[0-9a-fA-F]{64}$/.test(hash)) {
    return null;
  }
  return "staff_dev_tools:v1:" + version + ":" + hash.toLowerCase();
}

/**
 * Extracts encrypted or plaintext user token from LevelDB text dump (module 5203).
 * @param {unknown} text - Raw LevelDB text content.
 * @returns {Readonly<{ token: string, encrypted: boolean }> | null} Extracted token descriptor or null.
 */
export function extractTokenFromLevelDbText(text) {
  if (typeof text !== "string") {
    return null;
  }
  const encryptedMatch = text.match(/"dQw4w9WgXcQ:([A-Za-z0-9+/=]+)"/);
  if (encryptedMatch && encryptedMatch[1]) {
    return Object.freeze({ token: encryptedMatch[1], encrypted: true });
  }
  const plainMatch = text.match(/"([\w-]{24,}\.[\w-]{6,}\.[\w-]{27,})"/);
  if (plainMatch && plainMatch[1]) {
    return Object.freeze({ token: plainMatch[1], encrypted: false });
  }
  return null;
}

/**
 * Determines whether a non-renderer process crash should be reported based on exit code and cooldown (module 5203).
 * @param {unknown} processType - Process type string (e.g. "Utility", "GPU").
 * @param {unknown} exitCode - Integer process exit code.
 * @param {unknown} [elapsedSinceLastCrashMs] - Milliseconds elapsed since last crash.
 * @returns {boolean} True if crash should be reported, false otherwise.
 */
export function shouldReportNonRendererCrash(processType, exitCode, elapsedSinceLastCrashMs) {
  if (typeof processType !== "string" || typeof exitCode !== "number" || !Number.isInteger(exitCode)) {
    return false;
  }
  if (processType === "Utility" && exitCode === UTILITY_IGNORED_CRASH_EXIT_CODE) {
    return false;
  }
  if (typeof elapsedSinceLastCrashMs === "number" && elapsedSinceLastCrashMs >= 0 && elapsedSinceLastCrashMs < NON_RENDERER_CRASH_COOLDOWN_MS) {
    return false;
  }
  return true;
}

export const ZSTD_PRECOMPRESS_MAX_SIZE_MULTIPLIER = 10;
export const COMPRESSED_FILE_SUFFIX = ".zst";
export const DETAILED_TTI_FCP_TIMEOUT_MS = 60000;
export const RENDERER_PERFORMANCE_MARKS = Object.freeze([
  "renderer-first-paint",
  "renderer-first-contentful-paint",
  "renderer-full-interactive",
]);

/**
 * Computes maximum allowed size for file read accounting for compression (module 9590).
 * @param {unknown} dataMaxSize
 * @param {unknown} maxBufferLength
 * @param {unknown} willCompress
 * @returns {number | null}
 */
export function computeFileReadMaxSize(dataMaxSize, maxBufferLength, willCompress) {
  if (typeof dataMaxSize !== "number" || !Number.isFinite(dataMaxSize) || dataMaxSize < 0) {
    return null;
  }
  if (typeof maxBufferLength !== "number" || !Number.isFinite(maxBufferLength) || maxBufferLength < 0) {
    return null;
  }
  const capped = Math.min(dataMaxSize, maxBufferLength);
  return willCompress === true ? capped * ZSTD_PRECOMPRESS_MAX_SIZE_MULTIPLIER : capped;
}

/**
 * Formats a startup timing line for logging/telemetry (module 5203).
 * @param {unknown} entryName
 * @param {unknown} entryType
 * @param {unknown} startTime
 * @param {unknown} [duration]
 * @returns {string | null}
 */
export function formatStartupTimingLine(entryName, entryType, startTime, duration = 0) {
  if (
    typeof entryName !== "string" ||
    typeof entryType !== "string" ||
    typeof startTime !== "number" ||
    !Number.isFinite(startTime) ||
    typeof duration !== "number" ||
    !Number.isFinite(duration)
  ) {
    return null;
  }
  const label = ("|  " + entryName + ":").padEnd(50);
  if (entryType === "mark") {
    return `${label} ${startTime.toFixed(2)}`;
  }
  if (entryType === "measure") {
    return `${label} ${(startTime + duration).toFixed(2)} (${duration.toFixed(2)})`;
  }
  return null;
}

/**
 * Tray menu rebuild state keys (modules 1275, 9090).
 */
export const TRAY_MENU_REBUILD_KEYS = Object.freeze(["connected", "muteStates", "deafStates"]);

/**
 * Thumbar button names (module 4161).
 */
export const THUMBAR_BUTTON_NAMES = Object.freeze(["VIDEO", "MUTE", "DEAFEN", "DISCONNECT"]);

/**
 * Resource scale variants (module 9090).
 */
export const RESOURCE_SCALE_VARIANTS = Object.freeze([
  "@1.25x",
  "@1.33x",
  "@1.4x",
  "@1.5x",
  "@1.8x",
  "@2x",
  "@2.5x",
  "@3x",
  "@4x",
  "@5x",
]);

/**
 * Resolves Windows system tray GUID for release channel (module 9090).
 * @param {unknown} channel
 * @returns {string | null}
 */
export function resolveTrayGuidForChannel(channel) {
  if (channel === "stable") {
    return "699e423d-73da-42ba-b94e-881ff8672467";
  }
  if (channel === "ptb") {
    return "8bdf397e-37fc-440a-ac55-ec80cc585152";
  }
  if (channel === "canary") {
    return "d9c5e1da-d4ab-424f-97cf-00c2ce9bec67";
  }
  if (channel === "development") {
    return "6603ea8b-b21d-46e9-8ea5-d87b9105613f";
  }
  return null;
}

/**
 * Resolves system tray icon name given voice and unread state (module 1275).
 * @param {unknown} state
 * @returns {string}
 */
export function resolveTrayIconName(state) {
  if (!state || typeof state !== "object") {
    return "tray";
  }
  /** @type {any} */
  const stateObj = state;
  if (stateObj.connected === true) {
    if (stateObj.selfDeaf === true || stateObj.serverDeaf === true) {
      return "tray-deafened";
    }
    if (stateObj.selfMute === true || stateObj.serverMute === true || stateObj.suppress === true) {
      return "tray-muted";
    }
    if (stateObj.speaking === true) {
      return "tray-speaking";
    }
    return "tray-connected";
  }
  if (stateObj.unread === true) {
    return "tray-unread";
  }
  return "tray";
}

/**
 * Formats thumbar button icon filename depending on active state and system theme (module 4161).
 * @param {unknown} name
 * @param {unknown} active
 * @param {unknown} isSystemDarkMode
 * @returns {string | null}
 */
export function formatThumbarButtonIconFilename(name, active, isSystemDarkMode) {
  if (typeof name !== "string" || !THUMBAR_BUTTON_NAMES.includes(name)) {
    return null;
  }
  return `${name.toLowerCase()}${active === true ? "" : "-off"}${isSystemDarkMode === true ? "" : "-light"}.png`;
}

export const NETWORK_LOGGING_CDP_PROTOCOL_VERSION = "1.1";
export const NETWORK_LOGGING_CDP_METHODS = Object.freeze(["Network.requestWillBeSent", "Network.responseReceived"]);
export const NETWORK_LOGGING_STRIPPED_HEADERS = Object.freeze(["authorization", "cookie"]);
export const DOCUMENT_PIP_TRIGGER_ID = "__document_pip_trigger";

/**
 * Sanitizes headers by stripping sensitive headers like authorization and cookie (module 7142).
 * @param {unknown} headers
 * @returns {Record<string, any> | null}
 */
export function sanitizeNetworkLogHeaders(headers) {
  if (!headers || typeof headers !== "object" || Array.isArray(headers)) {
    return null;
  }
  /** @type {Record<string, any>} */
  const sanitized = {};
  for (const [key, value] of Object.entries(headers)) {
    const lowerKey = key.toLowerCase();
    if (lowerKey !== "authorization" && lowerKey !== "cookie") {
      sanitized[key] = value;
    }
  }
  return Object.freeze(sanitized);
}

/**
 * Computes words to add and remove between current and new spellcheck dictionary lists (module 3132).
 * @param {unknown} currentWords
 * @param {unknown} newWords
 * @returns {{ wordsToRemove: readonly string[], wordsToAdd: readonly string[] } | null}
 */
export function computeSpellcheckDictionaryDiff(currentWords, newWords) {
  if (!Array.isArray(currentWords) || !Array.isArray(newWords)) {
    return null;
  }
  const curSet = new Set(currentWords.filter((w) => typeof w === "string"));
  const newSet = new Set(newWords.filter((w) => typeof w === "string"));
  return Object.freeze({
    wordsToRemove: Object.freeze([...curSet].filter((w) => !newSet.has(w))),
    wordsToAdd: Object.freeze([...newSet].filter((w) => !curSet.has(w))),
  });
}

/**
 * Resolves whether content protection should be enabled for window (module 3386).
 * @param {unknown} windowEnabled
 * @param {unknown} globalContentProtectionEnabled
 * @returns {boolean}
 */
export function resolveWindowContentProtection(windowEnabled, globalContentProtectionEnabled) {
  return windowEnabled === true || globalContentProtectionEnabled === true;
}

export const CHUNKED_REQUEST_MISSING_ENDPOINT_ERROR = "missing api endpoint setting";
export const SYSIMG_MODULE_NAME = "discord_sysimg";
export const SYSIMG_UNAVAILABLE_ERROR = "discord_sysimg not available";
export const SYSIMG_TARGET_FORMAT = "jpeg";

/**
 * Rewrites route pathname to include API endpoint prefix if not already present.
 * @param {unknown} apiEndpointPathname
 * @param {unknown} routePathname
 * @returns {string | null}
 */
export function rewriteChunkedRequestPathname(apiEndpointPathname, routePathname) {
  if (typeof apiEndpointPathname !== "string" || typeof routePathname !== "string") {
    return null;
  }
  if (routePathname.startsWith(apiEndpointPathname)) {
    return routePathname;
  }
  return `${apiEndpointPathname}${routePathname}`;
}

/**
 * Checks if a given URL matches the webapp main bundle URL pattern.
 * @param {unknown} url
 * @returns {boolean}
 */
export function isWebappBundleUrl(url) {
  if (typeof url !== "string") {
    return false;
  }
  return /\/(?:assets\/)?web(?:\.\w+)?\.js$/.test(url);
}

/**
 * Computes TTFB and download/decompression durations for main bundle stats.
 * @param {unknown} fetchStartMs
 * @param {unknown} responseStartMs
 * @param {unknown} responseEndMs
 * @param {unknown} [downlinkMbps=null]
 * @returns {{ downloadDecompressDurationMs: number, ttfbMs: number, connectionDownlinkKbps: number | null } | null}
 */
export function computeBundleStatsDurations(fetchStartMs, responseStartMs, responseEndMs, downlinkMbps = null) {
  if (
    typeof fetchStartMs !== "number" ||
    !Number.isFinite(fetchStartMs) ||
    typeof responseStartMs !== "number" ||
    !Number.isFinite(responseStartMs) ||
    typeof responseEndMs !== "number" ||
    !Number.isFinite(responseEndMs)
  ) {
    return null;
  }
  const downloadDecompressDurationMs = Math.round(responseEndMs - responseStartMs);
  const ttfbMs = Math.round(responseStartMs - fetchStartMs);
  const connectionDownlinkKbps =
    typeof downlinkMbps === "number" && Number.isFinite(downlinkMbps)
      ? Math.round(1000 * downlinkMbps)
      : null;
  return Object.freeze({
    downloadDecompressDurationMs,
    ttfbMs,
    connectionDownlinkKbps,
  });
}

export const PARTIAL_DOWNLOAD_SUFFIX = ".partial";
export const HOOK_MINIDUMP_MAX_SIZE_BYTES = 10485760;
export const CLIPS_SENTRY_DIR_NAME = "sentry_clips";

/**
 * Determines if an unused asset file should be cleaned up.
 *
 * @param {unknown} neededFiles
 * @param {unknown} fileName
 * @returns {boolean}
 */
export function shouldDeleteUnusedAssetFile(neededFiles, fileName) {
  if (
    !Array.isArray(neededFiles) ||
    typeof fileName !== "string" ||
    fileName.length === 0
  ) {
    return false;
  }
  if (neededFiles.includes(fileName)) {
    return false;
  }
  if (fileName.endsWith(".partial")) {
    return !neededFiles.includes(fileName.slice(0, -PARTIAL_DOWNLOAD_SUFFIX.length));
  }
  if (fileName.startsWith(".") || !fileName.includes(".")) {
    return false;
  }
  return true;
}

/**
 * Determines whether a log file should be compressed prior to upload.
 *
 * @param {unknown} fileName
 * @param {unknown} compressLogs
 * @returns {boolean}
 */
export function shouldCompressLogFile(fileName, compressLogs) {
  if (typeof fileName !== "string" || fileName.length === 0) {
    return false;
  }
  return (
    fileName.toLowerCase().endsWith(".dmp") ||
    (compressLogs === true &&
      (fileName.endsWith(".log") || fileName.endsWith("webrtc")))
  );
}

/**
 * Orders a pair of WebRTC log file names by their modification time (mtimeMs).
 * The older log comes first, followed by the newer log.
 *
 * @param {unknown} firstName
 * @param {unknown} firstMtimeMs
 * @param {unknown} secondName
 * @param {unknown} secondMtimeMs
 * @returns {readonly [string, string] | null}
 */
export function orderWebRtcLogPairByMtime(
  firstName,
  firstMtimeMs,
  secondName,
  secondMtimeMs
) {
  if (
    typeof firstName !== "string" ||
    typeof secondName !== "string" ||
    typeof firstMtimeMs !== "number" ||
    !Number.isFinite(firstMtimeMs) ||
    typeof secondMtimeMs !== "number" ||
    !Number.isFinite(secondMtimeMs)
  ) {
    return null;
  }
  return Object.freeze(
    firstMtimeMs > secondMtimeMs
      ? [secondName, firstName]
      : [firstName, secondName]
  );
}

/**
 * Valid file names for recorded voice messages.
 * @type {readonly string[]}
 */
export const VOICE_MESSAGE_RECORDING_FILE_NAMES = Object.freeze(["recording.ogg", "0recording.ogg", "1recording.ogg", "2recording.ogg", "3recording.ogg", "4recording.ogg"]);

/**
 * Subdirectory for voice recordings.
 * @type {string}
 */
export const VOICE_MESSAGE_RECORDING_SUBDIR = "discord_voice";

/**
 * Checks whether a given filename is a valid voice message recording file.
 * @param {unknown} fileName
 * @returns {boolean}
 */
export function isValidVoiceRecordingFilename(fileName) {
  return typeof fileName === "string" && VOICE_MESSAGE_RECORDING_FILE_NAMES.includes(fileName);
}

/**
 * Checks whether a native module name is whitelisted.
 * @param {unknown} name
 * @returns {boolean}
 */
export function isWhitelistedNativeModule(name) {
  return typeof name === "string" && !PRELOAD_ONLY_NATIVE_MODULES.includes(name) && (/^discord_[a-z0-9_-]+$/.test(name) || name === "erlpack");
}

/**
 * Prefix for splash media format command line flag.
 * @type {string}
 */
export const SPLASH_MEDIA_FORMAT_FLAG_PREFIX = "--splash-media-format=";

/**
 * List of target native modules for pre-mapping.
 * @type {readonly string[]}
 */
export const PREMAP_TARGET_MODULES = Object.freeze(["discord_voice", "discord_zstd", "discord_cloudsync", "discord_media"]);

/**
 * Command line flag to enable native pre-mapping.
 * @type {string}
 */
export const NATIVE_PREMAP_FLAG = "--enable-native-premap";

/**
 * Error message when no committed module paths are available to pre-map.
 * @type {string}
 */
export const PREMAP_NO_COMMITTED_PATHS_ERROR = "no committed module paths to pre-map";

/**
 * Resolves splash media format from command line arguments.
 * @param {unknown} argv
 * @returns {"webp" | "webm"}
 */
export function resolveSplashMediaFormatFromArgv(argv) {
  if (!Array.isArray(argv)) {
    return "webm";
  }
  const flag = argv.find((arg) => typeof arg === "string" && arg.startsWith(SPLASH_MEDIA_FORMAT_FLAG_PREFIX));
  if (!flag) {
    return "webm";
  }
  return flag.slice(SPLASH_MEDIA_FORMAT_FLAG_PREFIX.length) === "webp" ? "webp" : "webm";
}

/**
 * Formats Linux manual download URL for a release channel and format.
 * @param {unknown} releaseChannel
 * @param {unknown} format
 * @returns {string | null}
 */
export function formatLinuxManualDownloadUrl(releaseChannel, format) {
  if (
    typeof releaseChannel !== "string" ||
    releaseChannel.length === 0 ||
    typeof format !== "string" ||
    format.length === 0
  ) {
    return null;
  }
  return `https://discord.com/api/download/${releaseChannel}?platform=linux&format=${format}`;
}

/**
 * Resolves manual download button text based on selected download format.
 * @param {unknown} selectedDownload
 * @returns {string}
 */
export function resolveSplashManualDownloadButtonText(selectedDownload) {
  return selectedDownload !== "nope" ? "Download" : "Okay";
}

/**
 * Suffix for current updater log file.
 * @type {string}
 */
export const UPDATER_CURRENT_LOG_SUFFIX = "updater_rCURRENT.log";

/**
 * Suffix for current admin service log file.
 * @type {string}
 */
export const ADMIN_SERVICE_CURRENT_LOG_SUFFIX = "admin_rCURRENT.log";

/**
 * Filename for Squirrel setup log.
 * @type {string}
 */
export const SQUIRREL_SETUP_LOG_FILENAME = "SquirrelSetup.log";

/**
 * Directory name for clips decider stash.
 * @type {string}
 */
export const CLIPS_DECIDER_STASH_DIR_NAME = "decider_stash";

/**
 * Data filename for clips decider stash.
 * @type {string}
 */
export const CLIPS_DECIDER_DATA_FILENAME = "data.json";

/**
 * Resolves Discord service channel folder name.
 * @param {unknown} releaseChannel
 * @returns {string}
 */
export function resolveDiscordServiceChannelFolder(releaseChannel) {
  if (typeof releaseChannel !== "string" || releaseChannel.length === 0 || releaseChannel === "stable") {
    return "Discord";
  }
  return "Discord" + releaseChannel.charAt(0).toUpperCase() + releaseChannel.slice(1);
}

/**
 * Formats deduplicated download filename with numeric suffix.
 * @param {unknown} fileName
 * @param {unknown} suffix
 * @returns {string | null}
 */
export function formatDeduplicatedDownloadFilename(fileName, suffix) {
  if (
    typeof fileName !== "string" ||
    fileName.length === 0 ||
    typeof suffix !== "number" ||
    !Number.isInteger(suffix) ||
    suffix < 0
  ) {
    return null;
  }
  if (suffix === 0) {
    return fileName;
  }
  const idx = fileName.lastIndexOf(".");
  if (idx > 0) {
    return `${fileName.slice(0, idx)}-${suffix}${fileName.slice(idx)}`;
  }
  return `${fileName}-${suffix}`;
}

/**
 * Checks whether a filename matches hook minidump naming pattern.
 * @param {unknown} fileName
 * @returns {boolean}
 */
export function isHookMinidumpFilename(fileName) {
  return typeof fileName === "string" && /^.{8}-.{4}-.{4}-.{4}-.{12}\.dmp$/i.test(fileName);
}

/**
 * Prefix for raw osclipboard MIME format string.
 * @type {string}
 */
export const CLIPBOARD_RAW_FORMAT_PREFIX = 'electron application/osclipboard;format="';

/**
 * Menu event identifiers.
 * @type {readonly string[]}
 */
export const MENU_EVENTS = Object.freeze(["menu:open-help", "menu:open-settings", "menu:check-for-updates"]);

/**
 * Allowed webapp paths.
 * @type {readonly string[]}
 */
export const WEBAPP_PATHS = Object.freeze(["/app", "/conference-mode"]);

/**
 * Required desktop capturer source types.
 * @type {readonly string[]}
 */
export const DESKTOP_CAPTURER_REQUIRED_TYPES = Object.freeze(["screen", "window"]);

/**
 * Unwraps raw clipboard format from electron MIME type prefix.
 * @param {unknown} mimeType
 * @returns {string | null}
 */
export function unwrapClipboardRawFormat(mimeType) {
  if (typeof mimeType !== "string") {
    return null;
  }
  if (mimeType.startsWith(CLIPBOARD_RAW_FORMAT_PREFIX) && mimeType.endsWith('"')) {
    return mimeType.slice(CLIPBOARD_RAW_FORMAT_PREFIX.length, -1);
  }
  return mimeType;
}

/**
 * Checks if clipboard formats contain mixed content (images/bitmaps) alongside text.
 * @param {unknown} formats
 * @param {unknown} textContent
 * @returns {boolean}
 */
export function hasClipboardMixedContent(formats, textContent) {
  if (!Array.isArray(formats) || typeof textContent !== "string" || textContent.trim().length === 0) {
    return false;
  }
  return formats.some((f) => {
    if (typeof f !== "string") {
      return false;
    }
    const lower = f.toLowerCase();
    return (
      f.startsWith("image/") ||
      f === "CF_DIB" ||
      f === "CF_BITMAP" ||
      f === "CF_DIBV5" ||
      lower.includes("image") ||
      lower.includes("bitmap")
    );
  });
}

/**
 * Filters desktop capturer types to required types (screen, window).
 * @param {unknown} types
 * @returns {readonly string[] | null}
 */
export function filterDesktopCapturerTypes(types) {
  if (!Array.isArray(types)) {
    return null;
  }
  return Object.freeze(
    types
      .filter((t) => typeof t === "string" && DESKTOP_CAPTURER_REQUIRED_TYPES.includes(t.toLowerCase()))
      .map((t) => t.toLowerCase())
  );
}

/**
 * Delay in milliseconds before reporting unhandled JS exceptions.
 * @type {number}
 */
export const UNHANDLED_JS_EXCEPTION_DELAY_MS = 50;

/**
 * Tag keys extracted for Sentry metadata.
 * @type {readonly string[]}
 */
export const SENTRY_METADATA_TAG_KEYS = Object.freeze(["nativeBuildNumber", "isStaff"]);

/**
 * Formats nested metadata key with bracket notation.
 * @param {unknown} prefix
 * @param {unknown} key
 * @returns {string | null}
 */
export function formatNestedMetadataKey(prefix, key) {
  if (typeof prefix !== "string" || typeof key !== "string" || key.length === 0) {
    return null;
  }
  return prefix === "" ? key : `${prefix}[${key}]`;
}

/**
 * Computes diff of crash reporter metadata keys and entries.
 * @param {unknown} oldKeys
 * @param {unknown} newEntries
 * @returns {{ keysToRemove: readonly string[], entriesToAdd: readonly (readonly [string, string])[] } | null}
 */
export function computeCrashReporterMetadataDiff(oldKeys, newEntries) {
  if (!Array.isArray(oldKeys) || newEntries == null || typeof newEntries !== "object" || Array.isArray(newEntries)) {
    return null;
  }
  /** @type {Record<string, any>} */
  const entriesObj = /** @type {any} */ (newEntries);
  return Object.freeze({
    keysToRemove: Object.freeze(
      oldKeys.filter((k) => typeof k === "string" && !Object.prototype.hasOwnProperty.call(entriesObj, k))
    ),
    entriesToAdd: Object.freeze(
      Object.keys(entriesObj)
        .filter((k) => !oldKeys.includes(k))
        .map((k) => /** @type {readonly [string, string]} */ (Object.freeze([k, String(entriesObj[k])])))
    ),
  });
}

/**
 * Formats unhandled JS exception message for process type.
 * @param {unknown} processType
 * @param {unknown} [delayed=false]
 * @returns {string | null}
 */
export function formatUnhandledJsExceptionMessage(processType, delayed = false) {
  if (typeof processType !== "string" || processType.length === 0) {
    return null;
  }
  return delayed === true ? `Delayed UNHANDLED_EXCEPTION ${processType}` : `UNHANDLED_EXCEPTION ${processType}`;
}
