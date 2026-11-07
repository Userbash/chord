// @ts-check
/**
 * Unit и интеграционные тесты очереди вложений M-11 (createAttachmentsController)
 * и её монтирования в композер сообщений (FE-P0-05 / task-151).
 *
 * Независимые оракулы:
 * - docs/specs/M-11-ATTACHMENTS.md и waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md:
 *   лимит 10 файлов на сообщение, <= 25 МБ (26 214 400 байт) на файл,
 *   префикс SPOILER_ для спойлеров, alt-text (description),
 *   полка #attachment-shelf над #composer-input,
 *   вызов uploadAll() -> upload_attachments при отправке сообщения,
 *   отправка с пустым текстом при наличии хотя бы 1 вложения,
 *   очистка полки после успешной отправки и при destroy().
 */

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  createAttachmentsController,
  MAX_ATTACHMENT_COUNT,
  MAX_ATTACHMENT_SIZE_BYTES,
  SPOILER_PREFIX,
  applySpoilerFilename,
  formatAttachmentSize,
} from "../../rust/gui-spike/ui/features/attachments.js";
import * as composerMod from "../../rust/gui-spike/ui/features/composer.js";

/**
 * Создаёт синтетический дескриптор файла с заданным размером.
 * @param {string} name
 * @param {number} size
 * @param {string} [type]
 * @param {string} [path]
 */
function makeMockFile(name, size, type = "image/png", path = undefined) {
  return {
    name,
    size,
    type,
    ...(path ? { path } : {}),
    data: new Uint8Array([0x89, 0x50, 0x4e, 0x47]),
  };
}

/**
 * Создаёт минимальное DOM-окружение композера для изолированных тестов.
 */
function setupComposerDom() {
  /** @type {any} */
  const document = new ShimDocument();

  /** @type {any} */
  const composer = document.createElement("form");
  composer.id = "composer";
  composer.className = "composer";

  /** @type {any} */
  const row = document.createElement("div");
  row.className = "composer-input-row";

  /** @type {any} */
  const attachBtn = document.createElement("button");
  attachBtn.id = "composer-attach-btn";
  attachBtn.type = "button";

  /** @type {any} */
  const fileInput = document.createElement("input");
  fileInput.id = "composer-file-input";
  fileInput.type = "file";
  fileInput.multiple = true;
  fileInput.className = "hidden";

  /** @type {any} */
  const input = document.createElement("textarea");
  input.id = "composer-input";

  /** @type {any} */
  const sendBtn = document.createElement("button");
  sendBtn.id = "composer-send";
  sendBtn.type = "submit";

  row.appendChild(attachBtn);
  row.appendChild(fileInput);
  row.appendChild(input);
  row.appendChild(sendBtn);
  composer.appendChild(row);

  /** @type {any} */
  const errorBanner = document.createElement("div");
  errorBanner.id = "error-banner";
  errorBanner.className = "error-banner hidden";

  document.body.appendChild(composer);
  document.body.appendChild(errorBanner);

  return {
    document,
    composer,
    row,
    attachBtn,
    fileInput,
    input,
    sendBtn,
    errorBanner,
  };
}

describe("M-11 UI: контроллер очереди вложений createAttachmentsController", () => {
  it("экспортирует константы лимитов (10 файлов, 25 МБ = 26 214 400 байт) и SPOILER_ хелперы", () => {
    assert.equal(MAX_ATTACHMENT_COUNT, 10);
    assert.equal(MAX_ATTACHMENT_SIZE_BYTES, 25 * 1024 * 1024);
    assert.equal(MAX_ATTACHMENT_SIZE_BYTES, 26_214_400);
    assert.equal(SPOILER_PREFIX, "SPOILER_");
    assert.equal(applySpoilerFilename("cat.png", true), "SPOILER_cat.png");
    assert.equal(applySpoilerFilename("SPOILER_cat.png", true), "SPOILER_cat.png");
    assert.equal(applySpoilerFilename("SPOILER_cat.png", false), "cat.png");
    assert.equal(formatAttachmentSize(512), "512\u00A0Б");
    assert.equal(formatAttachmentSize(2048), "2\u00A0КиБ");
    assert.equal(formatAttachmentSize(25 * 1024 * 1024), "25\u00A0МиБ");
  });

  it("рендерит полку #attachment-shelf над #composer-input и управляет spoiler, alt text и удалением", () => {
    const { document, composer, input } = setupComposerDom();
    const ctrl = createAttachmentsController({
      document,
      composerEl: composer,
      inputEl: input,
    });

    const shelf = document.getElementById("attachment-shelf");
    assert.ok(shelf, "#attachment-shelf должен быть создан в DOM");
    assert.equal(shelf.classList.contains("hidden"), true, "Пустая полка скрыта");

    // Проверяем, что полка расположена в #composer перед контейнером поля ввода
    assert.equal(
      composer.children[0],
      shelf,
      "#attachment-shelf должен быть первым дочерним элементом #composer над полем ввода"
    );

    const added = ctrl.addFiles([
      makeMockFile("photo.png", 4096, "image/png", "/tmp/photo.png"),
      makeMockFile("notes.txt", 1024, "text/plain", "/tmp/notes.txt"),
    ]);
    assert.equal(added.length, 2);
    assert.equal(ctrl.getCount(), 2);
    assert.equal(shelf.classList.contains("hidden"), false, "Непустая полка видима");

    const cards = shelf.querySelectorAll(".attachment-card");
    assert.equal(cards.length, 2);

    // Переключаем спойлер на первой карточке через клик по кнопке спойлера
    const firstId = added[0]?.id ?? "";
    const spoilerBtn = cards[0]?.querySelector(".attachment-spoiler-toggle");
    assert.ok(spoilerBtn, "Кнопка спойлера должна присутствовать на карточке");
    assert.equal(spoilerBtn.getAttribute("aria-pressed"), "false");
    spoilerBtn.dispatchEvent("click");

    assert.equal(ctrl.getQueue()[0]?.spoiler, true);
    assert.equal(ctrl.getQueue()[0]?.effectiveFilename, "SPOILER_photo.png");

    // Вводим описание (alt text) на первой карточке
    const updatedCard = shelf.querySelectorAll(".attachment-card")[0];
    const altInput = updatedCard?.querySelector(".attachment-alt-input");
    assert.ok(altInput, "Поле ввода alt-text должно присутствовать на карточке");
    altInput.value = "Кот на клавиатуре";
    altInput.dispatchEvent("input", { target: altInput });
    assert.equal(ctrl.getQueue()[0]?.description, "Кот на клавиатуре");

    // Удаляем первую карточку кнопкой удаления
    const removeBtn = updatedCard?.querySelector(".attachment-remove-btn");
    assert.ok(removeBtn, "Кнопка удаления должна присутствовать на карточке");
    removeBtn.dispatchEvent("click");

    assert.equal(ctrl.getCount(), 1);
    assert.equal(ctrl.getQueue()[0]?.filename, "notes.txt");
    assert.equal(firstId !== ctrl.getQueue()[0]?.id, true);
  });

  it("соблюдает лимиты: не более 10 файлов и <= 25 МБ на каждый файл", () => {
    const { document, composer, input } = setupComposerDom();
    /** @type {string[]} */
    const errors = [];
    const ctrl = createAttachmentsController({
      document,
      composerEl: composer,
      inputEl: input,
      onError: (err) => errors.push(typeof err === "string" ? err : err.message),
    });

    // Файл ровно 25 МБ допустим
    const exact25Mb = makeMockFile("exact25mb.bin", 25 * 1024 * 1024, "application/octet-stream");
    const acceptedExact = ctrl.addFiles([exact25Mb]);
    assert.equal(acceptedExact.length, 1);
    assert.equal(errors.length, 0);

    // Файл 25 МБ + 1 байт отклоняется
    const over25Mb = makeMockFile("huge.bin", 25 * 1024 * 1024 + 1, "application/octet-stream");
    const acceptedOver = ctrl.addFiles([over25Mb]);
    assert.equal(acceptedOver.length, 0);
    assert.equal(errors.length, 1);
    assert.match(String(errors[0]), /huge\.bin/);
    assert.match(String(errors[0]), /25 МБ/);

    // Добавляем ещё 10 файлов — в очереди уже 1, должно поместиться только 9 (итого 10)
    const batch = Array.from({ length: 10 }, (_, idx) =>
      makeMockFile(`item-${idx + 1}.png`, 1024, "image/png")
    );
    const acceptedBatch = ctrl.addFiles(batch);
    assert.equal(acceptedBatch.length, 9);
    assert.equal(ctrl.getCount(), 10);
    assert.equal(errors.length, 2);
    assert.match(String(errors[1]), /10/);
  });

  it("uploadAll() вызывает upload_attachments и возвращает нормализованный массив метаданных", async () => {
    const { document, composer, input } = setupComposerDom();
    /** @type {Array<{ cmd: string, args: any }>} */
    const ipcCalls = [];

    const ctrl = createAttachmentsController({
      document,
      composerEl: composer,
      inputEl: input,
      invokeFn: async (cmd, args) => {
        ipcCalls.push({ cmd, args });
        if (cmd === "upload_attachments") {
          return args.files.map((/** @type {any} */ f, /** @type {number} */ idx) => ({
            id: String(idx),
            filename: f.filename,
            uploaded_filename: `bucket/${idx}/${f.filename}`,
            size: f.size,
            content_type: f.content_type,
            description: f.description,
            spoiler: f.spoiler,
          }));
        }
        throw new Error(`Unexpected IPC: ${cmd}`);
      },
    });

    const added = ctrl.addFiles([
      makeMockFile("diagram.png", 8192, "image/png", "/tmp/diagram.png"),
    ]);
    const firstAddedId = added[0]?.id ?? "";
    ctrl.toggleSpoiler(firstAddedId);
    ctrl.setDescription(firstAddedId, "Архитектурная схема");

    const uploaded = await ctrl.uploadAll({ channelId: "ch-101" });
    assert.equal(ipcCalls.length, 1);
    assert.equal(ipcCalls[0]?.cmd, "upload_attachments");
    assert.equal(ipcCalls[0]?.args.channelId, "ch-101");
    assert.equal(ipcCalls[0]?.args.files.length, 1);
    assert.equal(ipcCalls[0]?.args.files[0]?.filename, "SPOILER_diagram.png");
    assert.equal(ipcCalls[0]?.args.files[0]?.original_filename, "diagram.png");
    assert.equal(ipcCalls[0]?.args.files[0]?.description, "Архитектурная схема");
    assert.equal(ipcCalls[0]?.args.files[0]?.spoiler, true);

    assert.deepEqual(uploaded, [
      {
        id: "0",
        filename: "SPOILER_diagram.png",
        uploaded_filename: "bucket/0/SPOILER_diagram.png",
        size: 8192,
        content_type: "image/png",
        description: "Архитектурная схема",
        spoiler: true,
        flags: 8,
      },
    ]);
  });
});

describe("FE-P0-05 / task-151: интеграция createAttachmentsController в composer.js", () => {
  it("composer.js экспортирует createMessageComposer и mountComposerAttachments", () => {
    assert.equal(
      typeof composerMod.createMessageComposer,
      "function",
      "composer.js должен экспортировать createMessageComposer"
    );
    assert.equal(
      typeof composerMod.mountComposerAttachments,
      "function",
      "composer.js должен экспортировать mountComposerAttachments"
    );
  });

  it("кнопка #composer-attach-btn (file input), drag-and-drop и paste добавляют файлы в createAttachmentsController (лимит 10 файлов, <= 25 МБ)", () => {
    assert.equal(typeof composerMod.createMessageComposer, "function");
    const { document, composer, attachBtn, fileInput, input } = setupComposerDom();
    /** @type {string[]} */
    const errors = [];

    const mc = composerMod.createMessageComposer({
      document,
      composerEl: composer,
      inputEl: input,
      attachBtnEl: attachBtn,
      fileInputEl: fileInput,
      getChannelId: () => "ch-1",
      onError: (/** @type {any} */ err) => errors.push(typeof err === "string" ? err : err.message),
    });

    // 1. Клик по #composer-attach-btn открывает fileInput.click()
    let fileInputClicked = false;
    fileInput.click = () => {
      fileInputClicked = true;
    };
    attachBtn.dispatchEvent("click");
    assert.equal(fileInputClicked, true, "Клик по #composer-attach-btn должен вызывать click() на file input");

    // 2. Событие change на fileInput добавляет файл 24 МБ (> 20 МБ старого лимита, но <= 25 МБ по M-11!)
    const file24Mb = makeMockFile("video-24mb.mp4", 24 * 1024 * 1024, "video/mp4", "/tmp/video-24mb.mp4");
    fileInput.files = [file24Mb];
    fileInput.dispatchEvent("change");
    assert.equal(mc.attachments.getCount(), 1, "Файл 24 МБ должен быть принят (лимит M-11 = 25 МБ)");
    assert.equal(errors.length, 0);

    // 3. Drag-and-drop на зону #composer добавляет файл и снимает класс composer-drag-over
    composer.dispatchEvent("dragenter", {
      dataTransfer: { types: ["Files"] },
    });
    assert.equal(composer.classList.contains("composer-drag-over"), true);

    const dropFile = makeMockFile("dropped.png", 2048, "image/png", "/tmp/dropped.png");
    composer.dispatchEvent("drop", {
      dataTransfer: { files: [dropFile] },
    });
    assert.equal(composer.classList.contains("composer-drag-over"), false);
    assert.equal(mc.attachments.getCount(), 2, "Drop на #composer должен добавить файл в очередь");

    // 4. Paste файла из буфера обмена на поле ввода / композер
    const pastedFile = makeMockFile("clipboard.png", 1024, "image/png");
    input.dispatchEvent("paste", {
      clipboardData: { files: [pastedFile] },
    });
    assert.equal(mc.attachments.getCount(), 3, "Paste файла должен добавить его в очередь");

    // 5. Проверка лимита > 25 МБ через drop
    const over25Mb = makeMockFile("too-big-26mb.bin", 26 * 1024 * 1024, "application/octet-stream");
    composer.dispatchEvent("drop", {
      dataTransfer: { files: [over25Mb] },
    });
    assert.equal(mc.attachments.getCount(), 3, "Файл > 25 МБ не должен добавляться в очередь");
    assert.equal(errors.length, 1);
    assert.match(String(errors[0]), /25 МБ/);

    // 6. Проверка лимита 10 файлов через fileInput
    const eightMore = Array.from({ length: 8 }, (_, idx) =>
      makeMockFile(`extra-${idx + 1}.txt`, 512, "text/plain")
    );
    fileInput.files = eightMore;
    fileInput.dispatchEvent("change");
    assert.equal(mc.attachments.getCount(), 10, "Очередь ограничена максимум 10 файлами");
    assert.equal(errors.length, 2);
    assert.match(String(errors[1]), /10/);
  });

  it("рендерит полку #attachment-shelf над #composer-input с карточками, переключателем SPOILER_, alt text и кнопкой удаления", () => {
    assert.equal(typeof composerMod.createMessageComposer, "function");
    const { document, composer, attachBtn, fileInput, input } = setupComposerDom();

    const mc = composerMod.createMessageComposer({
      document,
      composerEl: composer,
      inputEl: input,
      attachBtnEl: attachBtn,
      fileInputEl: fileInput,
      getChannelId: () => "ch-1",
    });

    fileInput.files = [
      makeMockFile("secret.png", 4096, "image/png", "/tmp/secret.png"),
      makeMockFile("readme.md", 1024, "text/markdown", "/tmp/readme.md"),
    ];
    fileInput.dispatchEvent("change");

    const shelf = document.getElementById("attachment-shelf");
    assert.ok(shelf, "Полка #attachment-shelf должна быть отрендерена в DOM");
    assert.equal(shelf.classList.contains("hidden"), false);
    assert.equal(
      composer.children[0],
      shelf,
      "Полка #attachment-shelf должна находиться над полем ввода композера"
    );

    const cards = shelf.querySelectorAll(".attachment-card");
    assert.equal(cards.length, 2);

    // Переключаем спойлер на первой карточке
    const spoilerToggle = cards[0]?.querySelector(".attachment-spoiler-toggle");
    assert.ok(spoilerToggle);
    spoilerToggle.dispatchEvent("click");

    const cardsAfterSpoiler = shelf.querySelectorAll(".attachment-card");
    const firstCardName = cardsAfterSpoiler[0]?.querySelector(".attachment-card-name");
    assert.equal(firstCardName?.textContent, "SPOILER_secret.png");

    // Заполняем описание (alt text)
    const altInput = cardsAfterSpoiler[0]?.querySelector(".attachment-alt-input");
    assert.ok(altInput);
    altInput.value = "Скриншот секретной фичи";
    altInput.dispatchEvent("input", { target: altInput });
    assert.equal(mc.attachments.getQueue()[0]?.description, "Скриншот секретной фичи");

    // Удаляем вторую карточку из очереди кнопкой удаления
    const secondRemoveBtn = cardsAfterSpoiler[1]?.querySelector(".attachment-remove-btn");
    assert.ok(secondRemoveBtn);
    secondRemoveBtn.dispatchEvent("click");

    assert.equal(mc.attachments.getCount(), 1);
    assert.equal(shelf.querySelectorAll(".attachment-card").length, 1);
  });

  it("при отправке сообщения вызывает uploadAll() -> upload_attachments и передаёт метаданные attachments вместе с текстом", async () => {
    assert.equal(typeof composerMod.createMessageComposer, "function");
    const { document, composer, attachBtn, fileInput, input } = setupComposerDom();

    /** @type {Array<{ cmd: string, args: any }>} */
    const ipcCalls = [];
    /** @type {Array<any>} */
    const sentPayloads = [];

    const mc = composerMod.createMessageComposer({
      document,
      composerEl: composer,
      inputEl: input,
      attachBtnEl: attachBtn,
      fileInputEl: fileInput,
      getChannelId: () => "ch-777",
      invokeFn: async (/** @type {string} */ cmd, /** @type {any} */ args) => {
        ipcCalls.push({ cmd, args });
        if (cmd === "upload_attachments") {
          return args.files.map((/** @type {any} */ f, /** @type {number} */ idx) => ({
            id: String(idx),
            filename: f.filename,
            uploaded_filename: `uploads/ch-777/${f.filename}`,
            size: f.size,
            content_type: f.content_type,
            description: f.description,
            spoiler: f.spoiler,
          }));
        }
        if (cmd === "send_message") {
          sentPayloads.push(args);
          return { id: "msg-900", content: args.content, attachments: args.attachments };
        }
        throw new Error(`Unexpected IPC command: ${cmd}`);
      },
      onSendMessage: async (/** @type {any} */ payload) => {
        sentPayloads.push(payload);
      },
    });

    fileInput.files = [makeMockFile("report.pdf", 16384, "application/pdf", "/tmp/report.pdf")];
    fileInput.dispatchEvent("change");

    const item = mc.attachments.getQueue()[0];
    const itemId = item?.id ?? "";
    mc.attachments.toggleSpoiler(itemId);
    mc.attachments.setDescription(itemId, "Квартальный отчёт");

    input.value = "Отправляю отчёт за квартал";
    await mc.submit();

    assert.equal(ipcCalls.length, 1);
    assert.equal(ipcCalls[0]?.cmd, "upload_attachments");
    assert.equal(ipcCalls[0]?.args.channelId, "ch-777");
    assert.equal(ipcCalls[0]?.args.files[0]?.filename, "SPOILER_report.pdf");
    assert.equal(ipcCalls[0]?.args.files[0]?.description, "Квартальный отчёт");

    assert.equal(sentPayloads.length, 1);
    assert.equal(sentPayloads[0]?.channelId, "ch-777");
    assert.equal(sentPayloads[0]?.content, "Отправляю отчёт за квартал");
    assert.equal(sentPayloads[0]?.attachments.length, 1);
    assert.deepEqual(sentPayloads[0]?.attachments[0], {
      id: "0",
      filename: "SPOILER_report.pdf",
      uploaded_filename: "uploads/ch-777/SPOILER_report.pdf",
      size: 16384,
      content_type: "application/pdf",
      description: "Квартальный отчёт",
      spoiler: true,
      flags: 8,
    });

    // Полка вложений и поле ввода должны очиститься после успешной отправки
    const shelf = document.getElementById("attachment-shelf");
    assert.equal(mc.attachments.getCount(), 0);
    assert.equal(shelf?.classList.contains("hidden"), true);
    assert.equal(shelf?.children.length, 0);
    assert.equal(input.value, "");
  });

  it("позволяет отправить сообщение с пустым текстом, если в очереди есть хотя бы 1 вложение, и блокирует пустую отправку без вложений", async () => {
    assert.equal(typeof composerMod.createMessageComposer, "function");
    const { document, composer, attachBtn, fileInput, input } = setupComposerDom();

    /** @type {Array<{ cmd: string, args: any }>} */
    const ipcCalls = [];
    /** @type {Array<any>} */
    const sentPayloads = [];

    const mc = composerMod.createMessageComposer({
      document,
      composerEl: composer,
      inputEl: input,
      attachBtnEl: attachBtn,
      fileInputEl: fileInput,
      getChannelId: () => "ch-empty-text",
      invokeFn: async (/** @type {string} */ cmd, /** @type {any} */ args) => {
        ipcCalls.push({ cmd, args });
        if (cmd === "upload_attachments") {
          return args.files.map((/** @type {any} */ f, /** @type {number} */ idx) => ({
            id: String(idx),
            filename: f.filename,
            uploaded_filename: `uploads/${f.filename}`,
            size: f.size,
            content_type: f.content_type,
            description: f.description,
            spoiler: f.spoiler,
          }));
        }
        return {};
      },
      onSendMessage: async (/** @type {any} */ payload) => {
        sentPayloads.push(payload);
      },
    });

    // 1. Пустой текст и пустая очередь — отправка НЕ происходит
    input.value = "   ";
    const emptyResult = await mc.submit();
    assert.equal(emptyResult, false);
    assert.equal(ipcCalls.length, 0);
    assert.equal(sentPayloads.length, 0);

    // 2. Пустой текст, но есть 1 вложение в очереди — отправка РАЗРЕШЕНА!
    fileInput.files = [makeMockFile("meme.png", 2048, "image/png", "/tmp/meme.png")];
    fileInput.dispatchEvent("change");
    input.value = "";

    const attachOnlyResult = await mc.submit();
    assert.equal(attachOnlyResult, true);
    assert.equal(ipcCalls.length, 1);
    assert.equal(ipcCalls[0]?.cmd, "upload_attachments");
    assert.equal(sentPayloads.length, 1);
    assert.equal(sentPayloads[0]?.content, "");
    assert.equal(sentPayloads[0]?.attachments.length, 1);
    assert.equal(sentPayloads[0]?.attachments[0]?.filename, "meme.png");
  });

  it("очищает полку вложений и отписывает обработчики при вызове destroy()", () => {
    assert.equal(typeof composerMod.createMessageComposer, "function");
    const { document, composer, attachBtn, fileInput, input } = setupComposerDom();

    const mc = composerMod.createMessageComposer({
      document,
      composerEl: composer,
      inputEl: input,
      attachBtnEl: attachBtn,
      fileInputEl: fileInput,
      getChannelId: () => "ch-destroy",
    });

    fileInput.files = [makeMockFile("temp.png", 1024, "image/png")];
    fileInput.dispatchEvent("change");
    assert.equal(mc.attachments.getCount(), 1);

    const shelf = document.getElementById("attachment-shelf");
    assert.equal(shelf?.classList.contains("hidden"), false);

    mc.destroy();

    assert.equal(mc.attachments.getCount(), 0);
    assert.equal(shelf?.classList.contains("hidden"), true);
    assert.equal(shelf?.children.length, 0);

    // После destroy() новые события drop/change не должны добавлять файлы в контроллер
    fileInput.files = [makeMockFile("after-destroy.png", 1024, "image/png")];
    fileInput.dispatchEvent("change");
    assert.equal(mc.attachments.getCount(), 0);
  });

  it("отправка по клавише Enter (без Shift) и сохранение очереди при ошибке upload_attachments", async () => {
    assert.equal(typeof composerMod.createMessageComposer, "function");
    const { document, composer, attachBtn, fileInput, input } = setupComposerDom();

    let shouldFailUpload = true;
    /** @type {Array<any>} */
    const sentPayloads = [];
    /** @type {Array<any>} */
    const errors = [];

    const mc = composerMod.createMessageComposer({
      document,
      composerEl: composer,
      inputEl: input,
      attachBtnEl: attachBtn,
      fileInputEl: fileInput,
      getChannelId: () => "ch-enter",
      invokeFn: async (/** @type {string} */ cmd, /** @type {any} */ args) => {
        if (cmd === "upload_attachments") {
          if (shouldFailUpload) {
            throw new Error("Сбой сети при выгрузке");
          }
          return args.files.map((/** @type {any} */ f, /** @type {number} */ idx) => ({
            id: String(idx),
            filename: f.filename,
            uploaded_filename: `uploads/${f.filename}`,
            size: f.size,
            content_type: f.content_type,
            description: f.description,
            spoiler: f.spoiler,
          }));
        }
        return {};
      },
      onSendMessage: async (/** @type {any} */ payload) => {
        sentPayloads.push(payload);
      },
      onError: (/** @type {any} */ err) => {
        errors.push(err);
      },
    });

    fileInput.files = [makeMockFile("retry-me.png", 4096, "image/png")];
    fileInput.dispatchEvent("change");
    input.value = "Текст сообщения";

    // При сбое выгрузки очередь и текст НЕ теряются
    const failedResult = await mc.submit();
    assert.equal(failedResult, false);
    assert.equal(sentPayloads.length, 0);
    assert.equal(mc.attachments.getCount(), 1, "Очередь вложений должна сохраняться при ошибке загрузки");
    assert.equal(input.value, "Текст сообщения");
    assert.ok(errors.length >= 1);

    // Повторная отправка через Enter (без Shift) проходит успешно
    shouldFailUpload = false;
    input.dispatchEvent("keydown", { key: "Enter", shiftKey: false });
    await new Promise((resolve) => setTimeout(resolve, 10));

    assert.equal(sentPayloads.length, 1);
    assert.equal(sentPayloads[0]?.content, "Текст сообщения");
    assert.equal(sentPayloads[0]?.attachments.length, 1);
    assert.equal(mc.attachments.getCount(), 0);
    assert.equal(input.value, "");
  });
});
