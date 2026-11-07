// @ts-check
/**
 * Контроллер очереди вложений композера (RW-40 / M-11 / FE-P0-05 / task-151).
 *
 * Отвечает за:
 * - приём файлов с кнопки прикрепления (`#composer-attach-btn` / `[data-action="attach"]` + `<input type="file">`),
 *   drag-and-drop на зону композера и вставку из буфера обмена (`paste`);
 * - валидацию лимитов очереди: не более 10 файлов (`MAX_ATTACHMENT_COUNT = 10`)
 *   и не более 25 МБ на один файл (`MAX_ATTACHMENT_SIZE_BYTES = 25 * 1024 * 1024`);
 * - рендеринг полки вложений (`#attachment-shelf`) над полем ввода композера с карточками файлов,
 *   переключателем спойлера (`SPOILER_`), полем ввода описания (`alt text` / `description`)
 *   и кнопкой удаления файла из очереди;
 * - пакетную выгрузку вложений `uploadAll()` через IPC (`upload_attachments`) с возвратом
 *   готовых метаданных для отправки сообщения;
 * - очистку полки после успешной отправки и корректное освобождение ресурсов при `destroy()`.
 */

/** Максимум файлов в очереди одного сообщения (RE §1.5: `n4 = 10`). */
export const MAX_ATTACHMENT_COUNT = 10;

/** Алиас лимита количества вложений на сообщение. */
export const MAX_ATTACHMENTS_PER_MESSAGE = MAX_ATTACHMENT_COUNT;

/** Максимальный размер одного файла в очереди контроллера: 25 МБ (`26_214_400` байт). */
export const MAX_ATTACHMENT_SIZE_BYTES = 25 * 1024 * 1024;

/** Алиас лимита размера файла (25 МБ). */
export const MAX_FILE_SIZE_BYTES = MAX_ATTACHMENT_SIZE_BYTES;

/** Префикс имени файла-спойлера по контракту Discord (`SPOILER_`). */
export const SPOILER_PREFIX = "SPOILER_";

/** Битовый флаг спойлера вложения (`AttachmentFlags.IS_SPOILER = 1 << 3 = 8`). */
export const ATTACHMENT_FLAG_IS_SPOILER = 8;

/** Имя IPC-команды пакетной загрузки вложений. */
export const UPLOAD_ATTACHMENTS_COMMAND = "upload_attachments";

/** Русские строки интерфейса полки вложений. */
export const ATTACHMENT_SHELF_TEXT = Object.freeze({
  shelfLabel: "Вложения к сообщению",
  attachButtonLabel: "Прикрепить файл",
  attachButtonTitle: "Прикрепить файл (до 10, не более 25 МБ)",
  spoilerLabel: "Пометить как спойлер",
  spoilerBadge: "СПОЙЛЕР",
  altPlaceholder: "Описание (alt text)…",
  altAriaLabel: "Описание вложения (alt text)",
  /** @param {string} name */
  removeAriaLabel: (name) => `Удалить файл «${name}» из очереди`,
  removeTitle: "Удалить из очереди",
  /** @param {number} max */
  errorMaxFiles: (max) => `Можно прикрепить не больше ${max} файлов.`,
  /**
   * @param {string} maxFormatted
   * @param {string} [fileName]
   */
  errorFileTooLarge: (maxFormatted, fileName = "") =>
    fileName
      ? `Файл «${fileName}» превышает допустимый размер (${maxFormatted} / 25 МБ).`
      : `Файл превышает допустимый размер (${maxFormatted} / 25 МБ).`,
  errorEmptyFile: "Файл пустой — выберите другой файл.",
  errorUploadFailed: "Не удалось выгрузить вложения.",
});

/**
 * Форматирует размер файла в человекочитаемую русскую строку (`Б`, `КиБ`, `МиБ`, `ГиБ`).
 * @param {number} bytes
 * @returns {string}
 */
export function formatAttachmentSize(bytes) {
  const safe = Number.isFinite(bytes) && bytes > 0 ? bytes : 0;
  if (safe < 1024) return `${safe}\u00A0Б`;
  /** @type {number} */
  let value;
  /** @type {string} */
  let unit;
  if (safe < 1024 * 1024) {
    value = safe / 1024;
    unit = "КиБ";
  } else if (safe < 1024 * 1024 * 1024) {
    value = safe / (1024 * 1024);
    unit = "МиБ";
  } else {
    value = safe / (1024 * 1024 * 1024);
    unit = "ГиБ";
  }
  const fixed = value.toFixed(1).replace(".", ",");
  const compact = fixed.endsWith(",0") ? fixed.slice(0, -2) : fixed;
  return `${compact}\u00A0${unit}`;
}

/**
 * Проверяет, начинается ли имя файла с префикса `SPOILER_`.
 * @param {string} filename
 * @returns {boolean}
 */
export function isSpoilerFilename(filename) {
  return typeof filename === "string" && filename.startsWith(SPOILER_PREFIX);
}

/**
 * Возвращает базовое имя файла без ведущего префикса `SPOILER_`.
 * @param {string} filename
 * @returns {string}
 */
export function stripSpoilerPrefix(filename) {
  const raw = String(filename || "file");
  if (raw.startsWith(SPOILER_PREFIX)) {
    const stripped = raw.slice(SPOILER_PREFIX.length);
    return stripped.length > 0 ? stripped : raw;
  }
  return raw;
}

/**
 * Применяет или снимает префикс `SPOILER_` у имени файла без дублирования.
 * @param {string} filename
 * @param {boolean} spoiler
 * @returns {string}
 */
export function applySpoilerFilename(filename, spoiler) {
  const base = stripSpoilerPrefix(filename);
  return spoiler ? `${SPOILER_PREFIX}${base}` : base;
}

/**
 * Вставляет `node` в `parent` перед `referenceNode` с поддержкой как стандартного DOM,
 * так и минимального `ShimElement` из `gui-dom-harness.mjs`.
 * @param {any} parent
 * @param {any} node
 * @param {any} [referenceNode]
 */
export function insertNodeBefore(parent, node, referenceNode = null) {
  if (!parent || !node) return;
  if (referenceNode && referenceNode.parentNode === parent) {
    if (typeof parent.insertBefore === "function") {
      parent.insertBefore(node, referenceNode);
      return;
    }
    if (Array.isArray(parent.children)) {
      if (node.parentNode === parent) {
        const existingIdx = parent.children.indexOf(node);
        if (existingIdx >= 0) parent.children.splice(existingIdx, 1);
      } else if (node.parentNode && typeof node.remove === "function") {
        node.remove();
      }
      node.parentNode = parent;
      const targetIdx = parent.children.indexOf(referenceNode);
      if (targetIdx >= 0) {
        parent.children.splice(targetIdx, 0, node);
      } else {
        parent.children.push(node);
      }
      return;
    }
  }
  if (node.parentNode !== parent) {
    if (typeof parent.append === "function") {
      parent.append(node);
    } else if (typeof parent.appendChild === "function") {
      parent.appendChild(node);
    }
  }
}

/**
 * Синхронно извлекает `Uint8Array` из файла/дескриптора, если байты уже доступны.
 * @param {any} file
 * @returns {Uint8Array | null}
 */
function extractSyncBytes(file) {
  if (!file || typeof file !== "object") return null;
  if (file.bytes instanceof Uint8Array) return file.bytes;
  if (ArrayBuffer.isView(file.bytes)) {
    return new Uint8Array(file.bytes.buffer, file.bytes.byteOffset, file.bytes.byteLength);
  }
  if (file.bytes instanceof ArrayBuffer) return new Uint8Array(file.bytes);
  if (Array.isArray(file.bytes)) return new Uint8Array(file.bytes);
  if (file.data instanceof Uint8Array) return file.data;
  if (ArrayBuffer.isView(file.data)) {
    return new Uint8Array(file.data.buffer, file.data.byteOffset, file.data.byteLength);
  }
  if (file.data instanceof ArrayBuffer) return new Uint8Array(file.data);
  if (Array.isArray(file.data)) return new Uint8Array(file.data);
  return null;
}

/**
 * Асинхронно дочитывает байты из `File`/`Blob` при наличии `arrayBuffer()`.
 * @param {any} file
 * @returns {Promise<Uint8Array | null>}
 */
async function extractAsyncBytes(file) {
  const sync = extractSyncBytes(file);
  if (sync) return sync;
  if (file && typeof file.arrayBuffer === "function") {
    try {
      const buf = await file.arrayBuffer();
      return new Uint8Array(buf);
    } catch {
      return null;
    }
  }
  return null;
}

/**
 * Извлекает список файлов из события `drop` или `paste`.
 * @param {any} event
 * @returns {any[]}
 */
export function extractFilesFromTransferEvent(event) {
  if (!event || typeof event !== "object") return [];
  const transfer = event.dataTransfer || event.clipboardData || null;
  if (transfer) {
    if (transfer.files && typeof transfer.files.length === "number" && transfer.files.length > 0) {
      return Array.from(transfer.files);
    }
    if (transfer.items && typeof transfer.items.length === "number") {
      /** @type {any[]} */
      const extracted = [];
      for (const item of Array.from(transfer.items)) {
        if (!item) continue;
        if (item.kind === "file" && typeof item.getAsFile === "function") {
          const f = item.getAsFile();
          if (f) extracted.push(f);
        } else if (item instanceof Object && ("name" in item || "filename" in item)) {
          extracted.push(item);
        }
      }
      if (extracted.length > 0) return extracted;
    }
  }
  if (event.files && typeof event.files.length === "number") {
    return Array.from(event.files);
  }
  return [];
}

/**
 * @typedef {{
 *   id: string,
 *   file: any,
 *   originalFilename: string,
 *   effectiveFilename: string,
 *   filename: string,
 *   name: string,
 *   size: number,
 *   contentType: string,
 *   content_type: string,
 *   type: string,
 *   spoiler: boolean,
 *   description: string,
 *   alt: string,
 *   status: "queued" | "uploading" | "uploaded" | "error",
 *   state: "queued" | "uploading" | "ready" | "error",
 *   progress: number,
 *   uploadedFilename: string,
 *   uploaded_filename: string,
 *   url: string,
 *   bytes: Uint8Array | null,
 *   data: Uint8Array | null,
 *   error: string | null,
 * }} AttachmentQueueItem
 */

/**
 * @typedef {{
 *   id: string,
 *   filename: string,
 *   uploaded_filename: string,
 *   size: number,
 *   content_type: string,
 *   spoiler: boolean,
 *   description: string,
 *   flags?: number,
 *   url?: string,
 * }} UploadedAttachmentMetadata
 */

/**
 * @typedef {{
 *   code: "MAX_FILES_EXCEEDED" | "FILE_TOO_LARGE" | "EMPTY_FILE" | "UPLOAD_FAILED",
 *   message: string,
 *   file?: any,
 * }} AttachmentControllerError
 */

/**
 * Создаёт контроллер очереди вложений и полки `#attachment-shelf`.
 *
 * @param {{
 *   shelfEl?: any,
 *   shelf?: any,
 *   composerEl?: any,
 *   form?: any,
 *   inputEl?: any,
 *   input?: any,
 *   attachBtn?: any,
 *   attachBtnEl?: any,
 *   button?: any,
 *   fileInput?: any,
 *   fileInputEl?: any,
 *   dropZone?: any,
 *   document?: any,
 *   channelId?: string,
 *   getChannelId?: () => string,
 *   maxFiles?: number,
 *   maxAttachments?: number,
 *   maxCount?: number,
 *   maxFileSize?: number,
 *   maxSizeBytes?: number,
 *   rejectEmpty?: boolean,
 *   invoke?: ((cmd: string, args?: any) => Promise<any>) | null,
 *   invokeFn?: ((cmd: string, args?: any) => Promise<any>) | null,
 *   upload?: ((files: any[], context: { channelId: string }) => Promise<any>) | null,
 *   onChange?: ((items: AttachmentQueueItem[], state: any) => void) | null,
 *   onError?: ((error: any) => void) | null,
 * }} [options]
 */
export function createAttachmentsController(options = {}) {
  const composerEl = /** @type {any} */ (options.composerEl ?? options.form ?? null);
  const inputEl = /** @type {any} */ (
    options.inputEl ??
      options.input ??
      composerEl?.querySelector?.("#composer-input") ??
      null
  );
  const doc = /** @type {any} */ (
    options.document ??
      options.shelfEl?.ownerDocument ??
      options.shelf?.ownerDocument ??
      composerEl?.ownerDocument ??
      inputEl?.ownerDocument ??
      (typeof document !== "undefined" ? document : null)
  );

  const maxFiles = Number(
    options.maxFiles ?? options.maxAttachments ?? options.maxCount ?? MAX_ATTACHMENT_COUNT,
  );
  const maxFileSize = Number(
    options.maxFileSize ?? options.maxSizeBytes ?? MAX_ATTACHMENT_SIZE_BYTES,
  );
  const rejectEmpty = Boolean(options.rejectEmpty);

  // Полка вложений (#attachment-shelf) над полем ввода композера
  /** @type {any} */
  let shelfEl =
    options.shelfEl ??
    options.shelf ??
    composerEl?.querySelector?.("#attachment-shelf") ??
    doc?.getElementById?.("attachment-shelf") ??
    null;

  if (!shelfEl && doc) {
    shelfEl = doc.createElement("div");
    shelfEl.id = "attachment-shelf";
    shelfEl.className = "attachment-shelf";
  }

  if (shelfEl) {
    if (!shelfEl.id) shelfEl.id = "attachment-shelf";
    if (!shelfEl.className) shelfEl.className = "attachment-shelf";
    else if (!shelfEl.classList?.contains?.("attachment-shelf")) {
      shelfEl.classList?.add?.("attachment-shelf");
    }
    shelfEl.setAttribute("role", "list");
    shelfEl.setAttribute("aria-label", ATTACHMENT_SHELF_TEXT.shelfLabel);
    if (composerEl) {
      let refNode = inputEl;
      while (refNode && refNode.parentNode && refNode.parentNode !== composerEl) {
        refNode = refNode.parentNode;
      }
      if (!refNode || refNode.parentNode !== composerEl) {
        refNode = composerEl.firstChild ?? composerEl.children?.[0] ?? null;
      }
      insertNodeBefore(composerEl, shelfEl, refNode);
    }
  }

  // Кнопка прикрепления (#composer-attach-btn / [data-action="attach"])
  /** @type {any} */
  const attachBtn =
    options.attachBtn ??
    options.attachBtnEl ??
    options.button ??
    composerEl?.querySelector?.("#composer-attach-btn") ??
    composerEl?.querySelector?.('[data-action="attach"]') ??
    doc?.getElementById?.("composer-attach-btn") ??
    null;

  if (attachBtn) {
    if (!attachBtn.id) attachBtn.id = "composer-attach-btn";
    if (!attachBtn.getAttribute("aria-label")) {
      attachBtn.setAttribute("aria-label", ATTACHMENT_SHELF_TEXT.attachButtonLabel);
    }
  }

  // Скрытый <input type="file" multiple>
  /** @type {any} */
  let fileInput =
    options.fileInput ??
    options.fileInputEl ??
    composerEl?.querySelector?.("#composer-file-input") ??
    composerEl?.querySelector?.('input[type="file"]') ??
    doc?.getElementById?.("composer-file-input") ??
    null;

  if (!fileInput && doc && composerEl) {
    fileInput = doc.createElement("input");
    fileInput.id = "composer-file-input";
    fileInput.setAttribute("type", "file");
    fileInput.setAttribute("multiple", "");
    fileInput.hidden = true;
    fileInput.setAttribute("aria-hidden", "true");
    fileInput.tabIndex = -1;
    composerEl.append(fileInput);
  }

  const dropZone = /** @type {any} */ (options.dropZone ?? composerEl ?? null);

  /** @type {AttachmentQueueItem[]} */
  const items = [];
  /** @type {Map<string, any>} */
  const cardElements = new Map();
  /** @type {AttachmentControllerError | null} */
  let lastError = null;
  let uploading = false;
  let destroyed = false;
  let idSeq = 0;

  /**
   * @param {AttachmentQueueItem} item
   * @returns {AttachmentQueueItem}
   */
  function cloneItem(item) {
    return { ...item };
  }

  /**
   * @returns {AttachmentQueueItem[]}
   */
  function getItems() {
    return items.map(cloneItem);
  }

  function getState() {
    return {
      items: getItems(),
      count: items.length,
      maxFiles,
      maxFileSize,
      uploading,
      lastError: lastError ? { ...lastError } : null,
      destroyed,
    };
  }

  function notifyChange() {
    options.onChange?.(getItems(), getState());
  }

  /**
   * @param {AttachmentControllerError["code"]} code
   * @param {string} message
   * @param {any} [file]
   */
  function reportError(code, message, file) {
    lastError = { code, message, ...(file !== undefined ? { file } : {}) };
    options.onError?.(lastError);
    syncShelfAttributes();
  }

  function syncAttachButtonState() {
    if (!attachBtn) return;
    if (items.length >= maxFiles) {
      attachBtn.setAttribute("aria-disabled", "true");
    } else {
      attachBtn.removeAttribute("aria-disabled");
    }
  }

  function syncShelfAttributes() {
    if (!shelfEl) return;
    const isEmpty = items.length === 0;
    shelfEl.hidden = isEmpty;
    if (isEmpty) {
      shelfEl.classList?.add?.("hidden");
      shelfEl.setAttribute("hidden", "");
      shelfEl.setAttribute("data-empty", "true");
    } else {
      shelfEl.classList?.remove?.("hidden");
      shelfEl.removeAttribute("hidden");
      shelfEl.setAttribute("data-empty", "false");
    }
    shelfEl.setAttribute("data-count", String(items.length));
    if (lastError) {
      shelfEl.setAttribute("data-error-code", lastError.code);
      shelfEl.setAttribute("data-error-message", lastError.message);
    } else {
      shelfEl.removeAttribute("data-error-code");
      shelfEl.removeAttribute("data-error-message");
    }
  }

  /**
   * Обновляет атрибуты и дочерние узлы карточки файла в `#attachment-shelf` без пересоздания `<input>`,
   * чтобы не терять фокус при вводе `alt text`.
   * @param {AttachmentQueueItem} item
   */
  function updateCardElement(item) {
    const card = cardElements.get(item.id);
    if (!card) return;

    card.setAttribute("data-attachment-id", item.id);
    card.setAttribute("data-filename", item.filename);
    card.setAttribute("data-spoiler", item.spoiler ? "true" : "false");
    card.setAttribute("data-state", item.status);
    card.setAttribute("data-description", item.description);

    const nameEl = card.querySelector(".attachment-card-name");
    if (nameEl) {
      nameEl.textContent = item.filename;
      nameEl.setAttribute("title", item.filename);
    }

    const sizeEl = card.querySelector(".attachment-card-size");
    if (sizeEl) {
      sizeEl.textContent = formatAttachmentSize(item.size);
    }

    const badgeEl = card.querySelector(".attachment-spoiler-badge");
    if (badgeEl) {
      badgeEl.hidden = !item.spoiler;
      if (item.spoiler) badgeEl.removeAttribute("hidden");
      else badgeEl.setAttribute("hidden", "");
    }

    const spoilerBtn = card.querySelector('[data-action="toggle-spoiler"]');
    if (spoilerBtn) {
      spoilerBtn.setAttribute("aria-checked", item.spoiler ? "true" : "false");
      spoilerBtn.setAttribute("aria-pressed", item.spoiler ? "true" : "false");
      spoilerBtn.checked = item.spoiler;
    }

    const altInput = card.querySelector(".attachment-alt-input");
    if (altInput && altInput.value !== item.description) {
      altInput.value = item.description;
    }

    const removeBtn = card.querySelector('[data-action="remove-attachment"]');
    if (removeBtn) {
      removeBtn.setAttribute("aria-label", ATTACHMENT_SHELF_TEXT.removeAriaLabel(item.filename));
    }
  }

  /**
   * Создаёт DOM-карточку файла для полки `#attachment-shelf`.
   * @param {AttachmentQueueItem} item
   * @returns {any}
   */
  function createCardElement(item) {
    if (!doc) return null;
    const card = doc.createElement("div");
    card.className = "attachment-card attachment-shelf-item";
    card.setAttribute("role", "listitem");
    card.setAttribute("data-attachment-id", item.id);
    card.setAttribute("data-filename", item.filename);
    card.setAttribute("data-spoiler", item.spoiler ? "true" : "false");
    card.setAttribute("data-state", item.status);
    card.setAttribute("data-description", item.description);

    const header = doc.createElement("div");
    header.className = "attachment-card-header";

    const nameEl = doc.createElement("span");
    nameEl.className = "attachment-card-name";
    nameEl.setAttribute("title", item.filename);
    nameEl.textContent = item.filename;

    const sizeEl = doc.createElement("span");
    sizeEl.className = "attachment-card-size";
    sizeEl.textContent = formatAttachmentSize(item.size);

    const badgeEl = doc.createElement("span");
    badgeEl.className = "attachment-spoiler-badge";
    badgeEl.textContent = ATTACHMENT_SHELF_TEXT.spoilerBadge;
    badgeEl.hidden = !item.spoiler;
    if (!item.spoiler) badgeEl.setAttribute("hidden", "");

    header.append(nameEl, sizeEl, badgeEl);

    const controls = doc.createElement("div");
    controls.className = "attachment-card-controls";

    const spoilerBtn = doc.createElement("button");
    spoilerBtn.className = "attachment-spoiler-toggle";
    spoilerBtn.setAttribute("type", "button");
    spoilerBtn.setAttribute("role", "switch");
    spoilerBtn.setAttribute("data-action", "toggle-spoiler");
    spoilerBtn.setAttribute("data-attachment-id", item.id);
    spoilerBtn.setAttribute("aria-label", ATTACHMENT_SHELF_TEXT.spoilerLabel);
    spoilerBtn.setAttribute("aria-checked", item.spoiler ? "true" : "false");
    spoilerBtn.setAttribute("aria-pressed", item.spoiler ? "true" : "false");
    spoilerBtn.checked = item.spoiler;
    spoilerBtn.textContent = "SPOILER";
    spoilerBtn.addEventListener("click", () => {
      if (destroyed) return;
      toggleSpoiler(item.id);
    });

    const altInput = doc.createElement("input");
    altInput.className = "attachment-alt-input attachment-description-input";
    altInput.setAttribute("type", "text");
    altInput.setAttribute("data-action", "attachment-alt");
    altInput.setAttribute("data-field", "description");
    altInput.setAttribute("data-attachment-id", item.id);
    altInput.setAttribute("placeholder", ATTACHMENT_SHELF_TEXT.altPlaceholder);
    altInput.setAttribute("aria-label", ATTACHMENT_SHELF_TEXT.altAriaLabel);
    altInput.setAttribute("maxlength", "1024");
    altInput.value = item.description;
    const onAltInput = () => {
      if (destroyed) return;
      setDescription(item.id, altInput.value);
    };
    altInput.addEventListener("input", onAltInput);
    altInput.addEventListener("change", onAltInput);

    const removeBtn = doc.createElement("button");
    removeBtn.className = "attachment-remove-btn";
    removeBtn.setAttribute("type", "button");
    removeBtn.setAttribute("data-action", "remove-attachment");
    removeBtn.setAttribute("data-attachment-id", item.id);
    removeBtn.setAttribute("aria-label", ATTACHMENT_SHELF_TEXT.removeAriaLabel(item.filename));
    removeBtn.setAttribute("title", ATTACHMENT_SHELF_TEXT.removeTitle);
    removeBtn.textContent = "✕";
    removeBtn.addEventListener("click", () => {
      if (destroyed) return;
      removeFile(item.id);
    });

    controls.append(spoilerBtn, altInput, removeBtn);
    card.append(header, controls);
    return card;
  }

  /**
   * Полная синхронизация DOM-полки `#attachment-shelf` с очередью `items`.
   */
  function render() {
    if (!shelfEl) return;
    syncAttachButtonState();
    syncShelfAttributes();

    const activeIds = new Set(items.map((item) => item.id));
    for (const [id, card] of cardElements.entries()) {
      if (!activeIds.has(id)) {
        card.remove?.();
        cardElements.delete(id);
      }
    }

    for (const item of items) {
      let card = cardElements.get(item.id);
      if (!card) {
        card = createCardElement(item);
        if (card) {
          cardElements.set(item.id, card);
          shelfEl.append(card);
        }
      } else {
        updateCardElement(item);
      }
    }
  }

  /**
   * Добавляет массив или `FileList` файлов в очередь с проверкой лимитов (10 файлов, <= 25 МБ).
   * @param {Iterable<any> | ArrayLike<any> | null | undefined} fileList
   * @returns {AttachmentQueueItem[]}
   */
  function addFiles(fileList) {
    if (destroyed || !fileList) return [];
    const incoming = Array.from(fileList);
    /** @type {AttachmentQueueItem[]} */
    const added = [];

    for (const rawFile of incoming) {
      if (!rawFile || typeof rawFile !== "object") continue;

      if (items.length >= maxFiles) {
        reportError(
          "MAX_FILES_EXCEEDED",
          ATTACHMENT_SHELF_TEXT.errorMaxFiles(maxFiles),
          rawFile,
        );
        break;
      }

      const rawName = String(rawFile.name ?? rawFile.filename ?? "file").trim() || "file";
      const syncBytes = extractSyncBytes(rawFile);
      const rawSize =
        typeof rawFile.size === "number"
          ? rawFile.size
          : typeof rawFile.file_size === "number"
            ? rawFile.file_size
            : syncBytes
              ? syncBytes.byteLength
              : 0;

      if (rawSize < 0 || (rejectEmpty && rawSize === 0)) {
        reportError("EMPTY_FILE", ATTACHMENT_SHELF_TEXT.errorEmptyFile, rawFile);
        continue;
      }

      if (rawSize > maxFileSize) {
        reportError(
          "FILE_TOO_LARGE",
          ATTACHMENT_SHELF_TEXT.errorFileTooLarge(formatAttachmentSize(maxFileSize), rawName),
          rawFile,
        );
        continue;
      }

      const initialSpoiler = Boolean(rawFile.spoiler) || isSpoilerFilename(rawName);
      const originalFilename = stripSpoilerPrefix(rawName);
      const filename = applySpoilerFilename(originalFilename, initialSpoiler);
      const contentType =
        String(
          rawFile.type ?? rawFile.content_type ?? rawFile.contentType ?? "application/octet-stream",
        ).trim() || "application/octet-stream";
      const description = String(
        rawFile.description ?? rawFile.alt ?? rawFile.altText ?? "",
      ).slice(0, 1024);

      idSeq += 1;
      const id = String(rawFile.id ?? `att-${idSeq}`);

      /** @type {AttachmentQueueItem} */
      const item = {
        id,
        file: rawFile,
        originalFilename,
        effectiveFilename: filename,
        filename,
        name: filename,
        size: rawSize,
        contentType,
        content_type: contentType,
        type: contentType,
        spoiler: initialSpoiler,
        description,
        alt: description,
        status: "queued",
        state: "queued",
        progress: 0,
        uploadedFilename: "",
        uploaded_filename: "",
        url: "",
        bytes: syncBytes,
        data: syncBytes,
        error: null,
      };

      items.push(item);
      added.push(cloneItem(item));
    }

    if (added.length > 0) {
      lastError = null;
    }
    render();
    notifyChange();
    return added;
  }

  /**
   * Добавляет один файл в очередь.
   * @param {any} file
   * @returns {AttachmentQueueItem | null}
   */
  function addFile(file) {
    const added = addFiles(file ? [file] : []);
    return added[0] ?? null;
  }

  /**
   * Переключает или явно устанавливает признак спойлера (`SPOILER_`) для файла в очереди.
   * @param {string} id
   * @param {boolean} [force]
   * @returns {AttachmentQueueItem | null}
   */
  function toggleSpoiler(id, force) {
    if (destroyed) return null;
    const item = items.find((entry) => entry.id === id);
    if (!item) return null;
    const nextSpoiler = typeof force === "boolean" ? force : !item.spoiler;
    item.spoiler = nextSpoiler;
    item.filename = applySpoilerFilename(item.originalFilename, nextSpoiler);
    item.effectiveFilename = item.filename;
    item.name = item.filename;
    updateCardElement(item);
    notifyChange();
    return cloneItem(item);
  }

  /**
   * Устанавливает описание (`alt text`) для файла в очереди.
   * @param {string} id
   * @param {string} description
   * @returns {AttachmentQueueItem | null}
   */
  function setDescription(id, description) {
    if (destroyed) return null;
    const item = items.find((entry) => entry.id === id);
    if (!item) return null;
    const cleaned = String(description ?? "").slice(0, 1024);
    item.description = cleaned;
    item.alt = cleaned;
    updateCardElement(item);
    notifyChange();
    return cloneItem(item);
  }

  /**
   * Удаляет файл из очереди по его `id`.
   * @param {string} id
   * @returns {boolean}
   */
  function removeFile(id) {
    if (destroyed) return false;
    const idx = items.findIndex((entry) => entry.id === id);
    if (idx === -1) return false;
    items.splice(idx, 1);
    const card = cardElements.get(id);
    if (card) {
      card.remove?.();
      cardElements.delete(id);
    }
    render();
    notifyChange();
    return true;
  }

  /**
   * Полностью очищает очередь вложений и скрывает `#attachment-shelf`.
   */
  function clear() {
    items.length = 0;
    lastError = null;
    for (const card of cardElements.values()) {
      card.remove?.();
    }
    cardElements.clear();
    if (shelfEl) {
      shelfEl.replaceChildren?.();
    }
    render();
    notifyChange();
  }

  /**
   * Разрешает функцию IPC-вызова из опций или глобального Tauri-контекста.
   * @returns {((cmd: string, args?: any) => Promise<any>) | null}
   */
  function resolveRpc() {
    if (typeof options.invoke === "function") return options.invoke;
    if (typeof options.invokeFn === "function") return options.invokeFn;
    /** @type {any} */
    const g = typeof globalThis !== "undefined" ? globalThis : undefined;
    const tauriInvoke = g?.window?.__TAURI__?.core?.invoke ?? g?.__TAURI__?.core?.invoke;
    return typeof tauriInvoke === "function" ? tauriInvoke : null;
  }

  /**
   * Выгружает все файлы из очереди через `upload_attachments` (или `options.upload`)
   * и возвращает массив готовых метаданных `UploadedAttachmentMetadata[]`.
   *
   * @param {string | { channelId?: string }} [overrideChannelId]
   * @returns {Promise<UploadedAttachmentMetadata[]>}
   */
  async function uploadAll(overrideChannelId) {
    if (destroyed || items.length === 0) return [];
    const overrideStr =
      typeof overrideChannelId === "string"
        ? overrideChannelId
        : overrideChannelId && typeof overrideChannelId === "object"
          ? overrideChannelId.channelId
          : undefined;
    const channelId =
      overrideStr ?? options.getChannelId?.() ?? options.channelId ?? "default";

    uploading = true;
    for (const item of items) {
      item.status = "uploading";
      item.state = "uploading";
      item.error = null;
      updateCardElement(item);
    }
    notifyChange();

    try {
      const preparedFiles = [];
      for (const item of items) {
        const bytes = item.bytes ?? (await extractAsyncBytes(item.file));
        item.bytes = bytes;
        item.data = bytes;
        preparedFiles.push({
          id: item.id,
          filename: item.filename,
          original_filename: item.originalFilename,
          name: item.filename,
          size: item.size,
          file_size: item.size,
          content_type: item.contentType,
          contentType: item.contentType,
          spoiler: item.spoiler,
          flags: item.spoiler ? ATTACHMENT_FLAG_IS_SPOILER : 0,
          description: item.description,
          alt: item.description,
          is_clip: false,
          file: item.file,
          bytes,
          data: bytes,
        });
      }

      /** @type {any} */
      let rawResponse = null;
      if (typeof options.upload === "function") {
        rawResponse = await options.upload(preparedFiles, { channelId });
      } else {
        const rpc = resolveRpc();
        if (rpc) {
          rawResponse = await rpc(UPLOAD_ATTACHMENTS_COMMAND, {
            channelId,
            channel_id: channelId,
            files: preparedFiles,
            attachments: preparedFiles,
          });
        }
      }

      /** @type {any[]} */
      const responseList = Array.isArray(rawResponse)
        ? rawResponse
        : Array.isArray(rawResponse?.attachments)
          ? rawResponse.attachments
          : Array.isArray(rawResponse?.files)
            ? rawResponse.files
            : [];

      /** @type {UploadedAttachmentMetadata[]} */
      const uploadedMeta = items.map((item, idx) => {
        const matched =
          responseList.find((entry) => entry && String(entry.id) === item.id) ??
          responseList[idx] ??
          null;
        const resolvedFilename = String(matched?.filename ?? item.filename);
        const resolvedUploadedFilename = String(
          matched?.uploaded_filename ??
            matched?.upload_filename ??
            (item.uploadedFilename || `${item.id}/${item.filename}`),
        );
        const resolvedDescription = String(
          matched?.description ?? matched?.alt ?? item.description ?? "",
        );
        const resolvedSpoiler =
          typeof matched?.spoiler === "boolean" ? matched.spoiler : item.spoiler;
        const resolvedSize = Number(matched?.size ?? matched?.file_size ?? item.size);
        const resolvedContentType = String(
          matched?.content_type ?? matched?.contentType ?? item.contentType,
        );

        item.status = "uploaded";
        item.state = "ready";
        item.progress = 100;
        item.uploadedFilename = resolvedUploadedFilename;
        item.uploaded_filename = resolvedUploadedFilename;
        if (matched?.url) item.url = String(matched.url);
        updateCardElement(item);

        /** @type {UploadedAttachmentMetadata} */
        const meta = {
          id: String(matched?.id ?? item.id),
          filename: resolvedFilename,
          uploaded_filename: resolvedUploadedFilename,
          size: resolvedSize,
          content_type: resolvedContentType,
          spoiler: resolvedSpoiler,
          description: resolvedDescription,
          ...(resolvedSpoiler ? { flags: ATTACHMENT_FLAG_IS_SPOILER } : {}),
          ...(matched?.url ? { url: String(matched.url) } : {}),
        };
        return meta;
      });

      uploading = false;
      notifyChange();
      return uploadedMeta;
    } catch (err) {
      uploading = false;
      const message =
        err instanceof Error && err.message
          ? err.message
          : ATTACHMENT_SHELF_TEXT.errorUploadFailed;
      for (const item of items) {
        item.status = "error";
        item.state = "error";
        item.error = message;
        updateCardElement(item);
      }
      reportError("UPLOAD_FAILED", message);
      notifyChange();
      throw err;
    }
  }

  // --- Слушатели DOM (кнопка прикрепления, file input, drag-and-drop, paste) ---
  /** @type {Array<() => void>} */
  const cleanups = [];

  /**
   * @param {any} target
   * @param {string} type
   * @param {(event: any) => void} handler
   */
  function listen(target, type, handler) {
    if (!target || typeof target.addEventListener !== "function") return;
    target.addEventListener(type, handler);
    cleanups.push(() => {
      if (typeof target.removeEventListener === "function") {
        target.removeEventListener(type, handler);
      }
    });
  }

  if (attachBtn) {
    listen(attachBtn, "click", (event) => {
      if (destroyed) return;
      if (attachBtn.getAttribute?.("aria-disabled") === "true" || items.length >= maxFiles) {
        event?.preventDefault?.();
        return;
      }
      fileInput?.click?.();
    });
  }

  if (fileInput) {
    listen(fileInput, "change", (event) => {
      if (destroyed) return;
      const selected = fileInput.files ?? event?.target?.files ?? [];
      if (selected && selected.length > 0) {
        addFiles(selected);
      }
      fileInput.value = "";
    });
  }

  if (dropZone) {
    listen(dropZone, "dragenter", (event) => {
      if (destroyed) return;
      event?.preventDefault?.();
      dropZone.classList?.add?.("composer-drag-over");
      dropZone.setAttribute?.("data-drop", "active");
      dropZone.setAttribute?.("data-dragover", "true");
    });

    listen(dropZone, "dragover", (event) => {
      if (destroyed) return;
      event?.preventDefault?.();
      dropZone.classList?.add?.("composer-drag-over");
      dropZone.setAttribute?.("data-drop", "active");
      dropZone.setAttribute?.("data-dragover", "true");
    });

    listen(dropZone, "dragleave", () => {
      if (destroyed) return;
      dropZone.classList?.remove?.("composer-drag-over");
      dropZone.removeAttribute?.("data-drop");
      dropZone.removeAttribute?.("data-dragover");
    });

    listen(dropZone, "drop", (event) => {
      if (destroyed) return;
      event?.preventDefault?.();
      dropZone.classList?.remove?.("composer-drag-over");
      dropZone.removeAttribute?.("data-drop");
      dropZone.removeAttribute?.("data-dragover");
      const droppedFiles = extractFilesFromTransferEvent(event);
      if (droppedFiles.length > 0) {
        addFiles(droppedFiles);
      }
    });

    listen(dropZone, "paste", (event) => {
      if (destroyed) return;
      if (inputEl && (event?.target === inputEl || (typeof inputEl.contains === "function" && inputEl.contains(event?.target)))) {
        return;
      }
      const pastedFiles = extractFilesFromTransferEvent(event);
      if (pastedFiles.length > 0) {
        event?.preventDefault?.();
        event?.stopPropagation?.();
        addFiles(pastedFiles);
      }
    });
  }

  if (inputEl && inputEl !== dropZone) {
    listen(inputEl, "paste", (event) => {
      if (destroyed) return;
      const pastedFiles = extractFilesFromTransferEvent(event);
      if (pastedFiles.length > 0) {
        event?.preventDefault?.();
        event?.stopPropagation?.();
        addFiles(pastedFiles);
      }
    });
  }

  render();

  function destroy() {
    if (destroyed) return;
    while (cleanups.length > 0) {
      const fn = cleanups.pop();
      try {
        fn?.();
      } catch {
        // best-effort cleanup при размонтировании
      }
    }
    if (dropZone) {
      dropZone.classList?.remove?.("composer-drag-over");
      dropZone.removeAttribute?.("data-drop");
      dropZone.removeAttribute?.("data-dragover");
    }
    clear();
    destroyed = true;
  }

  return {
    get items() {
      return getItems();
    },
    get count() {
      return items.length;
    },
    get maxFiles() {
      return maxFiles;
    },
    get maxFileSize() {
      return maxFileSize;
    },
    get shelfEl() {
      return shelfEl;
    },
    get shelf() {
      return shelfEl;
    },
    get isUploading() {
      return uploading;
    },
    get lastError() {
      return lastError ? { ...lastError } : null;
    },
    getItems,
    getQueue: getItems,
    getCount() {
      return items.length;
    },
    /**
     * @param {string} id
     * @returns {AttachmentQueueItem | null}
     */
    getItem(id) {
      const found = items.find((entry) => entry.id === id);
      return found ? cloneItem(found) : null;
    },
    getState,
    getLastError() {
      return lastError ? { ...lastError } : null;
    },
    addFiles,
    addFile,
    removeFile,
    removeAttachment: removeFile,
    remove: removeFile,
    toggleSpoiler,
    setSpoiler(/** @type {string} */ id, /** @type {boolean} */ flag) {
      return toggleSpoiler(id, Boolean(flag));
    },
    setDescription,
    setAltText: setDescription,
    uploadAll,
    clear,
    clearAttachments: clear,
    render,
    isDestroyed() {
      return destroyed;
    },
    destroy,
  };
}
