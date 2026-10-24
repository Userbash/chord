// @ts-check
/**
 * Композер (SPEC-P3-02 §4, CMP-01…CMP-15).
 *
 * Состояния: `empty` (пусто) → `typing` (есть текст) → `disabled` (нет прав/сети;
 * поле остаётся в DOM и фокусируемым, CMP-07). Лимит 2000 символов (K-10):
 * при превышении ввод обрезается, счётчик краснеет (CMP-13). Счётчик появляется
 * с 1800 символов (CMP-06). Отправки нет — спайк без сети, это видно в
 * `#composer-hint`/`.chat-notice`.
 */
import { UIError, asUIError } from "./errors.js";

/** K-10: платформенный лимит текста сообщения (нужен контракт W39i). */
export const COMPOSER_LIMIT = 2000;
/** K-11: счётчик показывается, когда остаток ≤ 200 символов (`max/10`). */
export const COUNTER_REMAINING = 200;
/** CMP-06: счётчик появляется при length ≥ 1800 (2000 − 200). */
export const COUNTER_FROM = COMPOSER_LIMIT - COUNTER_REMAINING;

/** @typedef {"empty" | "typing" | "disabled"} ComposerState */

/**
 * @param {{
 *   form: HTMLElement,
 *   input: HTMLTextAreaElement,
 *   hint?: HTMLElement | null,
 *   onError?: (error: UIError) => void,
 *   onEscape?: () => void,
 *   onSubmit?: (content: string) => void,
 *   onType?: () => void,
 * }} options
 */
export function createComposer(options) {
  const { form, input } = options;
  const onError = options.onError ?? (() => {});
  const onSubmit = options.onSubmit ?? (() => {});
  const onType = options.onType ?? (() => {});
  if (!form || !input) {
    throw new UIError("ELEMENT_MISSING", "композер: нет #composer/#composer-input");
  }

  const counter = document.createElement("span");
  counter.className = "composer-counter";
  counter.setAttribute("data-visible", "false");
  counter.setAttribute("data-exceeded", "false");
  counter.setAttribute("aria-hidden", "true");
  counter.textContent = `0/${COMPOSER_LIMIT}`;
  form.append(counter);

  const replyChip = document.createElement("div");
  replyChip.className = "composer-reply";
  replyChip.setAttribute("data-visible", "false");
  const replyText = document.createElement("span");
  replyText.className = "composer-reply-text";
  const replyCancel = document.createElement("button");
  replyCancel.className = "composer-reply-cancel";
  replyCancel.setAttribute("type", "button");
  replyCancel.setAttribute("aria-label", "Отменить ответ");
  replyCancel.textContent = "✕";
  replyChip.append(replyText, replyCancel);
  form.append(replyChip);

  /** @type {ComposerState} */
  let state = "empty";
  /** @type {{author: string, text: string} | null} */
  /** @type {{author: string, text: string, id?: string} | null} */
  let reply = null;
  let disabledReason = "";
  /** Значение на момент disabled: поле остаётся в DOM, но ввод не проходит. */
  let frozenValue = "";

  /** @param {ComposerState} next */
  function setState(next) {
    state = next;
    form.setAttribute("data-state", next);
    if (next === "disabled") {
      input.setAttribute("aria-disabled", "true");
      form.setAttribute("data-disabled-reason", disabledReason);
    } else {
      input.removeAttribute("aria-disabled");
      form.removeAttribute("data-disabled-reason");
    }
  }

  /** @returns {string} */
  function value() {
    return typeof input.value === "string" ? input.value : "";
  }

  /**
   * P-03-UI: сообщить о наборе (исходящий `send_typing` троттлит `app.js`).
   * Ошибка колбэка не должна ломать ввод — она идёт в общий обработчик.
   */
  function notifyType() {
    try {
      onType();
    } catch (error) {
      onError(asUIError("ACTION_FAILED", error));
    }
  }

  function refresh() {
    const length = value().length;
    const exceeded = length >= COMPOSER_LIMIT;
    form.setAttribute("data-limit", exceeded ? "exceeded" : "none");
    counter.textContent = `${length}/${COMPOSER_LIMIT}`;
    counter.setAttribute("data-visible", length >= COUNTER_FROM ? "true" : "false");
    counter.setAttribute("data-exceeded", exceeded ? "true" : "false");
    // Счётчик озвучивается только когда он действительно показан (иначе шум).
    if (length >= COUNTER_FROM) counter.setAttribute("aria-hidden", "false");
    else counter.setAttribute("aria-hidden", "true");
  }

  /** Обработчик ввода: бюджет + состояния. Ошибки — только через UIError. */
  function onInput() {
    try {
      if (state === "disabled") {
        if (value() !== frozenValue) input.value = frozenValue;
        return;
      }
      const raw = value();
      if (raw.length > COMPOSER_LIMIT) {
        input.value = raw.slice(0, COMPOSER_LIMIT);
      }
      refresh();
      setState(value().length === 0 ? "empty" : "typing");
      if (value().length > 0) notifyType();
    } catch (error) {
      onError(asUIError("ACTION_FAILED", error));
    }
  }

  /**
   * Esc: сначала отменяет ответ, иначе отдаёт событие приложению.
   * G2-01: режим ответа — состояние, а не клавиша: обычный набор текста
   * (`x`, `Backspace`, стрелки) его не трогает; отменяют только Escape и крестик.
   */
  function onKeydown(/** @type {KeyboardEvent} */ event) {
    if (event.key === "Enter") {
      if (event.shiftKey) return;
      event.preventDefault();
      const text = value();
      if (text.trim() !== "") {
        input.value = "";
        refresh();
        setState("empty");
        onSubmit(text);
      }
      return;
    }
    if (event.key !== "Escape") return;
    if (reply) {
      clearReply();
      return;
    }
    options.onEscape?.();
  }

  /**
   * @param {{author: string, text: string, id?: string}} target
   */
  function setReply(target) {
    reply = target;
    replyText.textContent = `Ответ ${target.author}: ${target.text}`;
    replyText.setAttribute("title", target.text);
    replyChip.setAttribute("data-visible", "true");
    form.setAttribute("data-reply", "true");
  }

  function clearReply() {
    reply = null;
    replyChip.setAttribute("data-visible", "false");
    replyText.textContent = "";
    form.setAttribute("data-reply", "false");
  }

  /**
   * @param {boolean} flag
   * @param {string} [reason]
   */
  function setDisabled(flag, reason = "") {
    disabledReason = flag ? reason : "";
    if (flag) {
      frozenValue = value();
      setState("disabled");
      return;
    }
    refresh();
    setState(value().length === 0 ? "empty" : "typing");
  }

  /**
   * Каретка/выделение поля. В шиме `selectionStart` может быть null — тогда
   * вставка идёт в конец (честный fallback, а не исключение).
   * @returns {{ start: number, end: number }}
   */
  function caretRange() {
    const length = value().length;
    const rawStart = typeof input.selectionStart === "number" ? input.selectionStart : null;
    const rawEnd = typeof input.selectionEnd === "number" ? input.selectionEnd : null;
    const start = rawStart === null ? length : Math.min(length, Math.max(0, rawStart));
    const end = rawEnd === null ? start : Math.min(length, Math.max(start, rawEnd));
    return { start, end };
  }

  /**
   * Обрезка до лимита без разрыва суррогатной пары: половина эмодзи в поле — это
   * битый текст, поэтому отдаём на code unit меньше.
   * @param {string} text
   * @returns {string}
   */
  function clipToLimit(text) {
    if (text.length <= COMPOSER_LIMIT) return text;
    let end = COMPOSER_LIMIT;
    const code = text.charCodeAt(end - 1);
    if (code >= 0xd800 && code <= 0xdbff) end -= 1;
    return text.slice(0, end);
  }

  /**
   * Вставка текста (эмодзи из пикера) в позицию курсора с учётом лимита K-10.
   * Возвращает false, если поле disabled или вставлять нечего.
   * @param {string} text
   * @returns {boolean}
   */
  function insertText(text) {
    try {
      if (state === "disabled" || text === "") return false;
      const { start, end } = caretRange();
      const next = clipToLimit(value().slice(0, start) + text + value().slice(end));
      input.value = next;
      const caret = Math.min(start + text.length, next.length);
      input.setSelectionRange(caret, caret);
      refresh();
      setState(next.length === 0 ? "empty" : "typing");
      if (next.length > 0) notifyType();
      return true;
    } catch (error) {
      onError(asUIError("ACTION_FAILED", error));
      return false;
    }
  }

  input.addEventListener("input", onInput);
  input.addEventListener("keydown", onKeydown);
  replyCancel.addEventListener("click", () => clearReply());

  form.setAttribute("data-reply", "false");
  form.setAttribute("data-limit", "none");
  setState("empty");

  return {
    /** @returns {ComposerState} */
    get state() {
      return state;
    },
    /** @returns {string} */
    get replyAuthor() {
      return reply?.author ?? "";
    },
    /** @returns {string} */
    get replyTo() {
      return reply?.id ?? "";
    },
    setDisabled,
    setReply,
    clearReply,
    refresh,
    insertText,
    /** @param {HTMLElement | null} [fallback] */
    focus(fallback = null) {
      const target = /** @type {any} */ (input);
      if (typeof target.focus === "function") target.focus();
      else if (fallback && typeof /** @type {any} */ (fallback).focus === "function") {
        /** @type {any} */ (fallback).focus();
      }
    },
  };
}

/** @typedef {ReturnType<typeof createComposer>} Composer */
