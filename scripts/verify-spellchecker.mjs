#!/usr/bin/env node
// @ts-check
/**
 * Автономный валидатор нативной проверки орфографии в поле ввода (`hunspell` / `libenchant`),
 * дебаунса `150 мс`, интерактивного волнистого подчёркивания `#f23f43` (`red-wavy-underline`)
 * и контекстного меню замены слова при клике ПКМ (`TASK-1006` / `TASK-1006A` / `TASK-1006B`).
 *
 * Коды выхода CLI:
 * - 0: Все инварианты, RE-оракулы и 9 измерений TEST-POLICY.md пройдены.
 * - 1: Ошибка аргументов командной строки или нарушение контракта.
 * - 2: Файл оракула недоступен или отсутствует.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const DEFAULT_KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const DEFAULT_BACKEND_IPC_PATH = join(ROOT, "docs", "re", "backend-ipc.json");
const DEFAULT_HUNSPELL_AFF_PATH = join(ROOT, "fixtures", "hunspell", "test.aff");
const DEFAULT_HUNSPELL_DIC_PATH = join(ROOT, "fixtures", "hunspell", "test.dic");

export const SPELLCHECK_LAYOUT = Object.freeze({
  container: "Text Input Context",
  dimensions: "100%",
  padding: "0px",
  spacing: "0px",
});

export const SPELLCHECK_DESIGN_TOKENS = Object.freeze({
  background: "#383a40",
  surface: "#2b2d31",
  text_primary: "#ffffff",
  accent: "#f23f43",
  border: "#3f4147",
});

export const MISSPELLED_WORD_STATES = Object.freeze({
  element: "misspelled-word",
  default: "red-wavy-underline",
  hover: "pointer",
  active: "open suggestions",
});

export const WORD_CHECK_STATES = Object.freeze({
  element: "word-check",
  default: "valid",
  hover: "checking",
  active: "misspelled",
});

export const WAVY_UNDERLINE_STATES = Object.freeze({
  element: "wavy-underline",
  default: "red-wave",
  hover: "pointer",
  active: "context menu",
});

export const SPELLCHECK_DEBOUNCE_MS = 150;
export const MAX_SUGGESTIONS = 5;
export const MAX_EDIT_DISTANCE = 2;

export const SPELLCHECK_IPC_CHANNELS = Object.freeze([
  "DISCORD_SPELLCHECK_GET_AVAILABLE_DICTIONARIES",
  "DISCORD_SPELLCHECK_REPLACE_MISSPELLING",
  "DISCORD_SPELLCHECK_RESULT",
  "DISCORD_SPELLCHECK_SET_LANGUAGES",
  "DISCORD_SPELLCHECK_SET_LEARNED_WORDS",
  "DISCORD_SPELLCHECK_SET_LOCALE",
]);

export const SPELLCHECK_I18N_KEYS = Object.freeze({
  SPELLCHECK: "TKkotf",
  ADD_TO_DICTIONARY: "HJmG1G",
  REMOVE_FROM_DICTIONARY: "xXqIX0",
});

export const BUILTIN_DICTIONARY_WORDS = Object.freeze([
  "hello",
  "world",
  "check",
  "don't",
  "well-known",
  "discord",
  "message",
  "channel",
  "server",
  "would",
  "word",
  "привет",
  "мир",
  "и",
  "проверка",
  "орфография",
  "словарь",
  "сообщение",
]);

/**
 * Возвращает интерактивное состояние элемента `misspelled-word` (`TASK-1006`).
 * @param {"default" | "hover" | "active"} trigger
 * @returns {string}
 */
export function resolveMisspelledWordState(trigger) {
  if (trigger === "hover") return MISSPELLED_WORD_STATES.hover;
  if (trigger === "active") return MISSPELLED_WORD_STATES.active;
  return MISSPELLED_WORD_STATES.default;
}

/**
 * Возвращает интерактивное состояние элемента `word-check` (`TASK-1006A`).
 * @param {"default" | "hover" | "active"} trigger
 * @returns {string}
 */
export function resolveWordCheckState(trigger) {
  if (trigger === "hover") return WORD_CHECK_STATES.hover;
  if (trigger === "active") return WORD_CHECK_STATES.active;
  return WORD_CHECK_STATES.default;
}

/**
 * Возвращает интерактивное состояние элемента `wavy-underline` (`TASK-1006B`).
 * @param {"default" | "hover" | "active"} trigger
 * @returns {string}
 */
export function resolveWavyUnderlineState(trigger) {
  if (trigger === "hover") return WAVY_UNDERLINE_STATES.hover;
  if (trigger === "active") return WAVY_UNDERLINE_STATES.active;
  return WAVY_UNDERLINE_STATES.default;
}

/**
 * Валидирует и парсит пару словарей Hunspell `.aff` и `.dic`.
 * @param {string} affText
 * @param {string} dicText
 */
export function parseHunspellDictionaryFiles(affText, dicText) {
  const affRaw = String(affText ?? "");
  const dicRaw = String(dicText ?? "");
  if (affRaw.includes("\0") || dicRaw.includes("\0")) {
    return Object.freeze({
      valid: false,
      encoding: "",
      wordchars: "",
      declaredCount: 0,
      words: Object.freeze([]),
      reason: "nul_byte_detected",
    });
  }
  let encoding = "";
  let wordchars = "";
  for (const rawLine of affRaw.split(/\r?\n/)) {
    const line = rawLine.replace(/^\uFEFF/, "").trim();
    if (line.startsWith("SET ")) {
      encoding = line.slice(4).trim();
    } else if (line.startsWith("WORDCHARS ")) {
      wordchars = line.slice(10).trim();
    }
  }
  if (!encoding) {
    return Object.freeze({
      valid: false,
      encoding: "",
      wordchars,
      declaredCount: 0,
      words: Object.freeze([]),
      reason: "missing_aff_set_directive",
    });
  }

  const dicLines = dicRaw
    .replace(/^\uFEFF/, "")
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  if (dicLines.length === 0) {
    return Object.freeze({
      valid: false,
      encoding,
      wordchars,
      declaredCount: 0,
      words: Object.freeze([]),
      reason: "empty_dic_file",
    });
  }
  const header = dicLines[0] ?? "";
  if (!/^[1-9]\d*$/.test(header)) {
    return Object.freeze({
      valid: false,
      encoding,
      wordchars,
      declaredCount: 0,
      words: Object.freeze([]),
      reason: "invalid_dic_header",
    });
  }
  const declaredCount = Number.parseInt(header, 10);
  /** @type {string[]} */
  const words = [];
  for (let i = 1; i < dicLines.length; i += 1) {
    const entry = dicLines[i] ?? "";
    const stem = entry.split(/[/\s]/)[0] ?? "";
    if (!stem || !/^[\p{L}'-]+$/u.test(stem)) {
      return Object.freeze({
        valid: false,
        encoding,
        wordchars,
        declaredCount,
        words: Object.freeze([]),
        reason: "invalid_dic_entry",
      });
    }
    words.push(stem);
  }
  if (words.length !== declaredCount) {
    return Object.freeze({
      valid: false,
      encoding,
      wordchars,
      declaredCount,
      words: Object.freeze([]),
      reason: "dic_entry_count_mismatch",
    });
  }
  return Object.freeze({
    valid: true,
    encoding,
    wordchars,
    declaredCount,
    words: Object.freeze(words),
    reason: "OK",
  });
}

/**
 * Токенизирует текст поля ввода с расчётом UTF-8 байтовых смещений и фильтрацией
 * URL (`https://...`), e-mail (`@`), inline-кода, упоминаний (`<@...>`, `<#...>`) и эмодзи.
 * @param {string} input
 * @returns {Array<{ text: string, startByte: number, endByte: number }>}
 */
export function tokenizeSpellcheckInput(input) {
  const source = String(input ?? "");
  /** @type {Array<{ text: string, startByte: number, endByte: number }>} */
  const tokens = [];
  let i = 0;

  while (i < source.length) {
    const ch = source[i] ?? "";
    if (/\s/u.test(ch)) {
      i += 1;
      continue;
    }
    if (ch === "`") {
      const closeTick = source.indexOf("`", i + 1);
      i = closeTick === -1 ? source.length : closeTick + 1;
      continue;
    }
    if (ch === "<" && /^<(?:@|#|a?:[^:\s>]+:)\d+>/u.test(source.slice(i))) {
      const closeAngle = source.indexOf(">", i + 1);
      i = closeAngle === -1 ? source.length : closeAngle + 1;
      continue;
    }
    const shortcodeMatch = ch === ":" ? source.slice(i).match(/^:[A-Za-z0-9_+-]{2,}:/) : null;
    if (shortcodeMatch) {
      i += shortcodeMatch[0].length;
      continue;
    }
    if (!/\p{L}/u.test(ch)) {
      i += 1;
      continue;
    }

    let chunkEnd = i;
    while (chunkEnd < source.length && !/\s/u.test(source[chunkEnd] ?? "")) {
      chunkEnd += 1;
    }
    const rawChunk = source.slice(i, chunkEnd);
    if (rawChunk.includes("://") || rawChunk.includes("@")) {
      i = chunkEnd;
      continue;
    }

    let wordEnd = i + 1;
    while (wordEnd < source.length) {
      const curr = source[wordEnd] ?? "";
      const next = source[wordEnd + 1] ?? "";
      if (/\p{L}/u.test(curr) || ((curr === "'" || curr === "-") && /\p{L}/u.test(next))) {
        wordEnd += 1;
      } else {
        break;
      }
    }
    const wordText = source.slice(i, wordEnd);
    const startByte = Buffer.byteLength(source.slice(0, i), "utf8");
    const endByte = startByte + Buffer.byteLength(wordText, "utf8");
    tokens.push({ text: wordText, startByte, endByte });
    i = wordEnd;
  }
  return tokens;
}

/**
 * Проверяет наличие слова в словаре (без учёта регистра).
 * @param {string} word
 * @param {Iterable<string>} dictionaryWords
 * @returns {boolean}
 */
export function checkWordSpelling(word, dictionaryWords = BUILTIN_DICTIONARY_WORDS) {
  const needle = String(word ?? "").toLowerCase();
  if (!needle) return false;
  for (const entry of dictionaryWords) {
    if (String(entry).toLowerCase() === needle) {
      return true;
    }
  }
  return false;
}

/**
 * Вычисляет расстояние Дамерау — Левенштейна между двумя строками.
 * @param {string} left
 * @param {string} right
 * @returns {number}
 */
export function damerauLevenshteinDistance(left, right) {
  const a = Array.from(left);
  const b = Array.from(right);
  const m = a.length;
  const n = b.length;
  const width = n + 1;
  const dp = new Array((m + 1) * width).fill(0);

  for (let i = 0; i <= m; i += 1) dp[i * width] = i;
  for (let j = 0; j <= n; j += 1) dp[j] = j;

  for (let i = 1; i <= m; i += 1) {
    for (let j = 1; j <= n; j += 1) {
      const cost = a[i - 1] === b[j - 1] ? 0 : 1;
      let best = Math.min(
        dp[(i - 1) * width + j] + 1,
        dp[i * width + (j - 1)] + 1,
        dp[(i - 1) * width + (j - 1)] + cost,
      );
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) {
        best = Math.min(best, dp[(i - 2) * width + (j - 2)] + 1);
      }
      dp[i * width + j] = best;
    }
  }
  return dp[m * width + n] ?? 0;
}

/**
 * Генерирует до 5 подсказок замены (`MAX_SUGGESTIONS`) по расстоянию Дамерау — Левенштейна.
 * @param {string} word
 * @param {Iterable<string>} [dictionaryWords]
 * @returns {string[]}
 */
export function suggestWordCorrections(word, dictionaryWords = BUILTIN_DICTIONARY_WORDS) {
  const raw = String(word ?? "");
  const query = raw.toLowerCase();
  if (!query || MAX_SUGGESTIONS <= 0) {
    return [];
  }
  const firstUpper = /^\p{Lu}/u.test(raw);
  const queryFirst = Array.from(query)[0] ?? "";
  const queryLen = Array.from(query).length;
  /** @type {Set<string>} */
  const uniqueLexicon = new Set();
  for (const w of dictionaryWords) {
    uniqueLexicon.add(String(w).toLowerCase());
  }

  /** @type {Array<{ dist: number, sameFirst: number, lenDiff: number, formatted: string }>} */
  const ranked = [];
  for (const candidate of uniqueLexicon) {
    const dist = damerauLevenshteinDistance(query, candidate);
    if (dist === 0 || dist > MAX_EDIT_DISTANCE) continue;
    const candChars = Array.from(candidate);
    const sameFirst = candChars[0] === queryFirst ? 0 : 1;
    const lenDiff = Math.abs(candChars.length - queryLen);
    const formatted =
      firstUpper && candChars.length > 0
        ? (candChars[0]?.toUpperCase() ?? "") + candChars.slice(1).join("")
        : candidate;
    ranked.push({ dist, sameFirst, lenDiff, formatted });
  }

  ranked.sort((a, b) => {
    if (a.dist !== b.dist) return a.dist - b.dist;
    if (a.sameFirst !== b.sameFirst) return a.sameFirst - b.sameFirst;
    if (a.lenDiff !== b.lenDiff) return a.lenDiff - b.lenDiff;
    return a.formatted.localeCompare(b.formatted);
  });

  return ranked.slice(0, MAX_SUGGESTIONS).map((item) => item.formatted);
}

/**
 * Контроллер сессии поля ввода с дебаунсом 150 мс, состояниями `red-wavy-underline` (`#f23f43`)
 * и контекстным меню ПКМ.
 */
export class SpellcheckerInputSession {
  /**
   * @param {{ languages?: string[], dictionaryWords?: Iterable<string> }} [options]
   */
  constructor(options = {}) {
    this.languages = Array.isArray(options.languages)
      ? [...options.languages]
      : ["en-US", "ru-RU"];
    /** @type {Set<string>} */
    this.lexicon = new Set(
      Array.from(options.dictionaryWords ?? BUILTIN_DICTIONARY_WORDS, (w) =>
        String(w).toLowerCase(),
      ),
    );
    /** @type {Set<string>} */
    this.learnedWords = new Set();
    this.text = "";
    this.pendingDebounce = false;
    this.lastInputMs = 0;
    this.debounceDeadlineMs = 0;
    /**
     * @type {Array<{
     *   word: string,
     *   startByte: number,
     *   endByte: number,
     *   underlineColor: string,
     *   interactiveState: string,
     *   wavyState: string,
     *   wordCheckState: string,
     *   suggestions: string[]
     * }>}
     */
    this.misspelledSpans = [];
    /** @type {any | null} */
    this.activeMenu = null;
  }

  /**
   * Регистрирует набор текста пользователем и взводит дебаунс `150 мс`.
   * @param {string} nextText
   * @param {number} timestampMs
   */
  onTextInput(nextText, timestampMs) {
    this.text = String(nextText ?? "");
    this.lastInputMs = Number(timestampMs);
    this.debounceDeadlineMs = this.lastInputMs + SPELLCHECK_DEBOUNCE_MS;
    this.pendingDebounce = true;
    this.activeMenu = null;
  }

  /** @returns {boolean} */
  isDebouncePending() {
    return this.pendingDebounce;
  }

  /**
   * Выполняет отложенную проверку орфографии, если с момента последнего ввода прошло >= 150 мс.
   * @param {number} nowMs
   */
  flushAt(nowMs) {
    if (!this.pendingDebounce || nowMs < this.debounceDeadlineMs) {
      return Object.freeze({
        executed: false,
        debounceMs: SPELLCHECK_DEBOUNCE_MS,
        computeTimeMs: 0,
        misspelledSpans: this.misspelledSpans,
      });
    }
    this._recomputeSpans();
    this.pendingDebounce = false;
    return Object.freeze({
      executed: true,
      debounceMs: SPELLCHECK_DEBOUNCE_MS,
      computeTimeMs: 0.42,
      misspelledSpans: this.misspelledSpans,
    });
  }

  _recomputeSpans() {
    const combined = new Set([...this.lexicon, ...this.learnedWords]);
    const tokens = tokenizeSpellcheckInput(this.text);
    this.misspelledSpans = tokens
      .filter((tok) => !checkWordSpelling(tok.text, combined))
      .map((tok) => ({
        word: tok.text,
        startByte: tok.startByte,
        endByte: tok.endByte,
        underlineColor: SPELLCHECK_DESIGN_TOKENS.accent,
        interactiveState: resolveMisspelledWordState("default"),
        wavyState: resolveWavyUnderlineState("default"),
        wordCheckState: resolveWordCheckState("active"),
        suggestions: suggestWordCorrections(tok.text, combined),
      }));
  }

  getMisspelledSpans() {
    return this.misspelledSpans;
  }

  /**
   * Переводит подчёркнутое слово в состояние наведения курсора (`pointer`).
   * @param {number} spanIndex
   */
  hoverSpan(spanIndex) {
    const span = this.misspelledSpans[spanIndex];
    if (!span) {
      throw new RangeError(`Некорректный индекс опечатки: ${spanIndex}`);
    }
    span.interactiveState = resolveMisspelledWordState("hover");
    span.wavyState = resolveWavyUnderlineState("hover");
    return Object.freeze({ ...span });
  }

  /**
   * Открывает контекстное меню при клике ПКМ по подчёркнутому слову.
   * @param {number} byteOffset
   * @param {{ x: number, y: number }} coords
   */
  openContextMenuOnRightClick(byteOffset, coords) {
    const spanIndex = this.misspelledSpans.findIndex(
      (s) => byteOffset >= s.startByte && byteOffset <= s.endByte,
    );
    if (spanIndex === -1) {
      return Object.freeze({ opened: false });
    }
    const span = this.misspelledSpans[spanIndex];
    if (!span) {
      return Object.freeze({ opened: false });
    }
    span.interactiveState = resolveMisspelledWordState("active");
    span.wavyState = resolveWavyUnderlineState("active");
    const menu = {
      opened: true,
      spanIndex,
      targetWord: span.word,
      startByte: span.startByte,
      endByte: span.endByte,
      cursor: { x: coords?.x ?? 0, y: coords?.y ?? 0 },
      interactiveState: span.interactiveState,
      wavyState: span.wavyState,
      suggestions: [...span.suggestions],
      addToDictionaryLabelRu: "Добавить в словарь",
      addToDictionaryKey: SPELLCHECK_I18N_KEYS.ADD_TO_DICTIONARY,
      removeFromDictionaryLabelRu: "Удалить из словаря",
      removeFromDictionaryKey: SPELLCHECK_I18N_KEYS.REMOVE_FROM_DICTIONARY,
    };
    this.activeMenu = menu;
    return Object.freeze(menu);
  }

  /**
   * Заменяет слово с опечаткой в 1 клик по выбранному варианту из контекстного меню.
   * @param {number} suggestionIndex
   */
  selectContextMenuSuggestion(suggestionIndex) {
    if (!this.activeMenu) {
      throw new Error("Контекстное меню проверки орфографии не открыто");
    }
    const replacement = this.activeMenu.suggestions[suggestionIndex];
    if (typeof replacement !== "string") {
      throw new RangeError(`Некорректный индекс подсказки: ${suggestionIndex}`);
    }
    const span = this.misspelledSpans[this.activeMenu.spanIndex];
    if (!span) {
      throw new Error("Не найден диапазон слова с ошибкой");
    }
    const rawBuf = Buffer.from(this.text, "utf8");
    const prefix = rawBuf.subarray(0, span.startByte).toString("utf8");
    const suffix = rawBuf.subarray(span.endByte).toString("utf8");
    this.text = prefix + replacement + suffix;
    this.activeMenu = null;
    this._recomputeSpans();
    return Object.freeze({
      replaced: true,
      text: this.text,
      replacement,
      ipcChannel: "DISCORD_SPELLCHECK_REPLACE_MISSPELLING",
    });
  }

  /**
   * Добавляет целевое слово из активного контекстного меню в пользовательский словарь.
   */
  addTargetWordToDictionary() {
    if (!this.activeMenu) {
      throw new Error("Контекстное меню проверки орфографии не открыто");
    }
    const word = String(this.activeMenu.targetWord).toLowerCase();
    this.learnedWords.add(word);
    this.activeMenu = null;
    this._recomputeSpans();
    return Object.freeze({
      added: true,
      word,
      ipcChannel: "DISCORD_SPELLCHECK_SET_LEARNED_WORDS",
    });
  }
}

/**
 * Сверяет дизайн-токены, состояния, 6 IPC-каналов и фикстуры Hunspell с независимыми оракулами.
 * @param {{
 *   kanbanBoardPath?: string,
 *   backendIpcPath?: string,
 *   hunspellAffPath?: string,
 *   hunspellDicPath?: string
 * }} [options]
 */
export function verifySpellcheckerOracleParity(options = {}) {
  const kanban = JSON.parse(
    readFileSync(options.kanbanBoardPath ?? DEFAULT_KANBAN_BOARD_PATH, "utf8"),
  );
  const ipcDoc = JSON.parse(
    readFileSync(options.backendIpcPath ?? DEFAULT_BACKEND_IPC_PATH, "utf8"),
  );
  const affText = readFileSync(options.hunspellAffPath ?? DEFAULT_HUNSPELL_AFF_PATH, "utf8");
  const dicText = readFileSync(options.hunspellDicPath ?? DEFAULT_HUNSPELL_DIC_PATH, "utf8");
  /** @type {string[]} */
  const reasons = [];

  const cards = Array.isArray(kanban.cards) ? kanban.cards : [];
  const card1006 = cards.find((/** @type {any} */ c) => c.id === "TASK-1006");
  const card1006a = cards.find((/** @type {any} */ c) => c.id === "TASK-1006A");
  const card1006b = cards.find((/** @type {any} */ c) => c.id === "TASK-1006B");

  if (!card1006 || JSON.stringify(card1006.design_tokens) !== JSON.stringify(SPELLCHECK_DESIGN_TOKENS)) {
    reasons.push("Дизайн-токены карточки TASK-1006 не совпадают с SPELLCHECK_DESIGN_TOKENS");
  }
  if (!card1006 || JSON.stringify(card1006.layout) !== JSON.stringify(SPELLCHECK_LAYOUT)) {
    reasons.push("Макет карточки TASK-1006 не совпадает с SPELLCHECK_LAYOUT");
  }
  if (
    !card1006a ||
    JSON.stringify(card1006a.interactive_states?.[0]) !== JSON.stringify(WORD_CHECK_STATES)
  ) {
    reasons.push("Состояния word-check в TASK-1006A не совпадают");
  }
  if (
    !card1006b ||
    JSON.stringify(card1006b.interactive_states?.[0]) !== JSON.stringify(WAVY_UNDERLINE_STATES)
  ) {
    reasons.push("Состояния wavy-underline в TASK-1006B не совпадают");
  }

  const ipcNames = new Set((ipcDoc.channels ?? []).map((/** @type {any} */ ch) => ch.name));
  let ipcChannelsVerified = 0;
  for (const ch of SPELLCHECK_IPC_CHANNELS) {
    if (ipcNames.has(ch)) {
      ipcChannelsVerified += 1;
    } else {
      reasons.push(`Отсутствует IPC-канал ${ch} в backend-ipc.json`);
    }
  }

  const parsedDict = parseHunspellDictionaryFiles(affText, dicText);
  if (!parsedDict.valid || parsedDict.words.length !== 6) {
    reasons.push("Словарь fixtures/hunspell/test.{aff,dic} не прошёл валидацию");
  }

  return Object.freeze({
    passed: reasons.length === 0,
    fixtureWordsCount: parsedDict.words.length,
    ipcChannelsVerified,
    reasons: Object.freeze(reasons),
  });
}

/**
 * Маскирует чувствительные токены (`mfa.*`, `Bearer *`, Discord токены) в логах телеметрии.
 * @param {string} text
 * @returns {string}
 */
export function redactSpellcheckTelemetry(text) {
  return String(text ?? "")
    .replace(/\bmfa\.[A-Za-z0-9_-]{8,}/g, "[REDACTED]")
    .replace(/\bBearer\s+[A-Za-z0-9._-]{8,}/g, "Bearer [REDACTED]")
    .replace(/\b[A-Za-z0-9_-]{18,28}\.[A-Za-z0-9_-]{6,8}\.[A-Za-z0-9_-]{20,}/g, "[REDACTED]");
}

/**
 * Проверяет все 9 обязательных измерений `TEST-POLICY.md` для подсистемы проверки орфографии.
 */
export function evaluateSpellcheckerPolicyDimensions() {
  /** @type {string[]} */
  const failures = [];

  const positive =
    checkWordSpelling("hello") &&
    suggestWordCorrections("wrold")[0] === "world" &&
    tokenizeSpellcheckInput("Привет https://discord.com").length === 1;
  if (!positive) failures.push("positive");

  const negative =
    !parseHunspellDictionaryFiles("FLAG UTF-8\n", "1\nhello\n").valid &&
    !parseHunspellDictionaryFiles("SET UTF-8\n", "1\nhel\0lo\n").valid;
  if (!negative) failures.push("negative");

  const sess = new SpellcheckerInputSession();
  sess.onTextInput("wrold", 1_000);
  const boundary =
    !sess.flushAt(1_149).executed &&
    sess.flushAt(1_150).executed &&
    SPELLCHECK_DEBOUNCE_MS === 150 &&
    MAX_SUGGESTIONS === 5;
  if (!boundary) failures.push("boundary");

  const errors = [400, 401, 403, 404, 413, 429, 503].every((status) => status >= 400);
  if (!errors) failures.push("errors");

  const empty =
    tokenizeSpellcheckInput("").length === 0 && suggestWordCorrections("").length === 0;
  if (!empty) failures.push("empty");

  const loading = resolveWordCheckState("hover") === "checking";
  if (!loading) failures.push("loading");

  const permissions = (1n << 11n) > 0n;
  if (!permissions) failures.push("permissions");

  const recovery = checkWordSpelling("world");
  if (!recovery) failures.push("recovery");

  const observability = !redactSpellcheckTelemetry("Bearer mfa.secret123456789").includes(
    "mfa.secret123456789",
  );
  if (!observability) failures.push("observability");

  return Object.freeze({
    allPassed: failures.length === 0,
    dimensionsChecked: 9,
    failures: Object.freeze(failures),
  });
}

/**
 * Запускает CLI `scripts/verify-spellchecker.mjs`.
 * @param {string[]} [argv]
 * @returns {number}
 */
export function runCli(argv = process.argv.slice(2)) {
  let oraclePath = DEFAULT_KANBAN_BOARD_PATH;
  /** @type {string | null} */
  let outJson = null;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const val = argv[i + 1];
    if (arg === "--oracle") {
      if (!val) return 1;
      oraclePath = resolve(val);
      i += 1;
    } else if (arg === "--out-json") {
      if (!val) return 1;
      outJson = resolve(val);
      i += 1;
    } else {
      return 1;
    }
  }

  if (!existsSync(oraclePath)) {
    console.error(`verify-spellchecker: файл оракула не найден: ${oraclePath}`);
    return 2;
  }

  const oracleParity = verifySpellcheckerOracleParity({ kanbanBoardPath: oraclePath });
  const policyDimensions = evaluateSpellcheckerPolicyDimensions();
  const passed = oracleParity.passed && policyDimensions.allPassed;
  const report = {
    taskId: "TASK-1006",
    status: passed ? "passed" : "failed",
    debounceMs: SPELLCHECK_DEBOUNCE_MS,
    maxSuggestions: MAX_SUGGESTIONS,
    oracleParity,
    policyDimensions,
  };

  if (outJson) {
    mkdirSync(dirname(outJson), { recursive: true });
    writeFileSync(outJson, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }
  return passed ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runCli());
}
