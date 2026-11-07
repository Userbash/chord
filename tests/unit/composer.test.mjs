// @ts-check
/**
 * B2b-ATTACH-UI + STUB-12-GIFT-UI (RW-40): композер — вложения и подарок 🎁.
 *
 * Оракулы:
 *   - waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md §1…§5
 *   - waves/RW-40/RE-DEEP-REA.md §1 (STUB-13), §5 (STUB-12)
 *   - rust/gui-spike/src/attachments.rs (MAX_UPLOAD_FILE_BYTES = 20971520, MAX_ATTACHMENTS_PER_MESSAGE = 10)
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  createComposer,
  evaluateClientChannelPolicy,
  MAX_UPLOAD_FILE_BYTES,
  MAX_ATTACHMENTS_PER_MESSAGE,
  ATTACH_LIMITS,
  ATTACH_TEXT,
  GIFT_TEXT,
  formatBytes,
} from "../../rust/gui-spike/ui/features/composer.js";

/**
 * Создаёт тестовую среду вокруг #composer в соответствии с index.html.
 * @param {{
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   uploadPut?: (payload: any) => Promise<any>,
 *   onSubmit?: (content: string, attachments?: any[]) => void,
 * }} [extra]
 */
function createTestEnv(extra = {}) {
  const doc = new ShimDocument();
  Object.assign(globalThis, {
    document: /** @type {any} */ (doc),
    window: /** @type {any} */ ({
      document: /** @type {any} */ (doc),
      __TAURI__: {
        core: {
          invoke: extra.invoke ?? (async () => ({ attachments: [] })),
        },
      },
    }),
  });

  const form = /** @type {any} */ (doc.createElement("div"));
  form.id = "composer";
  form.className = "composer";
  form.setAttribute("role", "group");
  form.setAttribute("aria-label", "Поле ввода сообщения");

  const attachBtn = /** @type {any} */ (doc.createElement("button"));
  attachBtn.className = "icon-btn composer-attach";
  attachBtn.setAttribute("type", "button");
  attachBtn.setAttribute("data-action", "attach");
  attachBtn.setAttribute("aria-label", "Прикрепить файл");
  attachBtn.textContent = "＋";
  form.append(attachBtn);

  const input = /** @type {any} */ (doc.createElement("textarea"));
  input.id = "composer-input";
  input.className = "composer-input";
  input.setAttribute("rows", "1");
  input.setAttribute("placeholder", "Написать сообщение…");
  form.append(input);

  const emojiBtn = /** @type {any} */ (doc.createElement("button"));
  emojiBtn.className = "icon-btn";
  emojiBtn.setAttribute("type", "button");
  emojiBtn.setAttribute("data-action", "emoji");
  emojiBtn.textContent = "😀";
  form.append(emojiBtn);

  const giftBtn = /** @type {any} */ (doc.createElement("button"));
  giftBtn.className = "icon-btn composer-gift";
  giftBtn.setAttribute("type", "button");
  giftBtn.setAttribute("data-action", "composer-gift");
  giftBtn.setAttribute("aria-label", "Подарок");
  giftBtn.textContent = "🎁";
  form.append(giftBtn);

  doc.body.append(form);

  const composer = createComposer(
    /** @type {any} */ ({
      form: /** @type {any} */ (form),
      input: /** @type {any} */ (input),
      invoke: extra.invoke,
      uploadPut: extra.uploadPut,
      onSubmit: extra.onSubmit,
    }),
  );

  return { doc, form, input, attachBtn, giftBtn, composer };
}

describe("B2b-ATTACH-UI: константы и оракулы лимитов", () => {
  it("лимиты совпадают с бэкендом B2a и RE (20 МиБ на файл, 10 файлов на сообщение)", () => {
    assert.equal(MAX_UPLOAD_FILE_BYTES, 20 * 1024 * 1024, "базовый лимит файла — 20 МиБ (20971520)");
    assert.equal(MAX_ATTACHMENTS_PER_MESSAGE, 10, "максимум 10 вложений на сообщение");
    assert.equal(ATTACH_LIMITS.maxFileSize, 20971520);
    assert.equal(ATTACH_LIMITS.maxCount, 10);
  });

  it("formatBytes форматирует размеры в соответствии с §2.3 (неразрывный пробел, запятая)", () => {
    assert.equal(formatBytes(0), "0\u00A0Б");
    assert.equal(formatBytes(999), "999\u00A0Б");
    assert.equal(formatBytes(1024), "1\u00A0КиБ");
    assert.equal(formatBytes(1536), "1,5\u00A0КиБ");
    assert.equal(formatBytes(10485760), "10\u00A0МиБ");
    assert.equal(formatBytes(20971520), "20\u00A0МиБ");
  });
});

describe("B2b-ATTACH-UI: progressive enhancement композера", () => {
  it("идемпотентно достраивает узлы очереди, счётчика, скрытого input и live-region", () => {
    const { doc, form, attachBtn } = createTestEnv();

    const queue = /** @type {any} */ (form.querySelector("#composer-attachments"));
    assert.ok(queue, "нет #composer-attachments");
    assert.equal(queue.getAttribute("role"), "list");
    assert.equal(queue.hidden, true, "очередь изначально скрыта");

    const count = /** @type {any} */ (form.querySelector("#composer-attach-count"));
    assert.ok(count, "нет #composer-attach-count");
    assert.equal(count.getAttribute("data-visible"), "false");
    assert.equal(count.textContent, "0 / 10");

    const fileInput = /** @type {any} */ (form.querySelector("#composer-file-input"));
    assert.ok(fileInput, "нет #composer-file-input");
    assert.equal(fileInput.getAttribute("type"), "file");
    assert.equal(fileInput.getAttribute("multiple"), "");

    const statusRegion = /** @type {any} */ (doc.getElementById("composer-attach-status"));
    assert.ok(statusRegion, "нет #composer-attach-status");
    assert.equal(statusRegion.getAttribute("role"), "status");

    assert.equal(form.getAttribute("data-attach-state"), "none");
    assert.equal(attachBtn.getAttribute("title"), "Прикрепить файл (до 10)");
  });

  it("клик по [data-action='attach'] триггерит click() на скрытом file input", () => {
    const { form, attachBtn } = createTestEnv();
    const fileInput = /** @type {any} */ (form.querySelector("#composer-file-input"));
    let clicked = false;
    fileInput.click = () => {
      clicked = true;
    };

    attachBtn.click();
    assert.equal(clicked, true, "клик по attach не вызвал click() у file input");
  });
});

describe("B2b-ATTACH-UI: очередь файлов и валидация лимитов", () => {
  it("добавление валидного файла создаёт чип со статусом queued и обновляет счётчик", async () => {
    const { form, composer } = createTestEnv({
      invoke: undefined, // без сети - остаётся в queued
    });

    await composer.addFiles([
      { name: "документ.pdf", size: 1024 * 500, type: "application/pdf" },
    ]);

    const queue = /** @type {any} */ (form.querySelector("#composer-attachments"));
    assert.equal(queue.hidden, false);
    const chips = queue.querySelectorAll(".attachment-chip");
    assert.equal(chips.length, 1);

    const chip = /** @type {any} */ (chips[0]);
    assert.equal(chip.getAttribute("role"), "listitem");
    assert.ok(chip.getAttribute("data-attachment-id"));
    assert.equal(chip.getAttribute("data-state"), "queued");

    const name = chip.querySelector(".attachment-name");
    assert.equal(name?.textContent, "документ.pdf");
    const size = chip.querySelector(".attachment-size");
    assert.equal(size?.textContent, "500\u00A0КиБ");

    const count = /** @type {any} */ (form.querySelector("#composer-attach-count"));
    assert.equal(count.getAttribute("data-visible"), "true");
    assert.equal(count.textContent, "1 / 10");
  });

  it("addFiles сохраняет ссылку на исходный File и его байты (bytes/data) в элементе очереди", async () => {
    const { composer } = createTestEnv({
      invoke: undefined,
    });

    const rawBytes = new Uint8Array([0x25, 0x50, 0x44, 0x46, 0x2d, 0x31, 0x2e, 0x37]);
    const fileObj = new File([rawBytes], "contract.pdf", { type: "application/pdf" });

    await composer.addFiles([fileObj]);

    assert.equal(composer.attachments.length, 1);
    const queued = composer.attachments[0];
    assert.ok(queued, "элемент очереди должен существовать");
    assert.equal(queued.file, fileObj, "должна сохраняться ссылка на исходный объект File");
    assert.deepEqual(queued.bytes, rawBytes, "байты из File должны быть сохранены в item.bytes");
    assert.deepEqual(queued.data, rawBytes, "байты из File должны быть доступны в item.data");
  });

  it("файл больше MAX_UPLOAD_FILE_BYTES получает состояние error-size/error-413 без сети", async () => {
    let networkCalled = false;
    const { form, composer } = createTestEnv({
      invoke: async () => {
        networkCalled = true;
        return { attachments: [] };
      },
    });

    await composer.addFiles([
      { name: "огромный.zip", size: MAX_UPLOAD_FILE_BYTES + 1, type: "application/zip" },
    ]);

    assert.equal(networkCalled, false, "сеть не должна вызываться при превышении лимита размера");
    const queue = /** @type {any} */ (form.querySelector("#composer-attachments"));
    const chip = /** @type {any} */ (queue?.querySelector(".attachment-chip"));
    assert.ok(chip);
    const state = chip.getAttribute("data-state");
    assert.ok(state === "error-size" || state === "error-413", "состояние должно быть error-size или error-413");
    const errorEl = chip.querySelector(".attachment-error");
    assert.match(errorEl?.textContent ?? "", /больше 20\s*(?:МиБ|МБ)/i);
    assert.equal(form.getAttribute("data-attach-state"), "error");
  });

  it("файл размером до 20 МиБ (в диапазоне 10…20 МиБ) принимается в очередь без ошибки error-size", async () => {
    const { form, composer } = createTestEnv({
      invoke: undefined,
    });

    await composer.addFiles([
      { name: "clip-15mib.mp4", size: 15 * 1024 * 1024, type: "video/mp4" },
      { name: "archive-20mib.zip", size: 20 * 1024 * 1024, type: "application/zip" },
    ]);

    const chips = form.querySelectorAll(".attachment-chip");
    assert.equal(chips.length, 2);
    for (const chip of chips) {
      assert.notEqual(chip.getAttribute("data-state"), "error-size");
      assert.equal(chip.getAttribute("data-state"), "queued");
    }
  });

  it("пустой файл 0 байт получает состояние error-empty/error-size без сети", async () => {
    const { form, composer } = createTestEnv();

    await composer.addFiles([
      { name: "пустой.txt", size: 0, type: "text/plain" },
    ]);

    const chip = /** @type {any} */ (form.querySelector(".attachment-chip"));
    assert.ok(chip);
    const state = chip.getAttribute("data-state");
    assert.ok(state === "error-empty" || state === "error-size");
    const errorEl = chip.querySelector(".attachment-error");
    assert.match(errorEl?.textContent ?? "", /пустой/i);
  });

  it("превышение лимита 10 файлов помечает 11-й как error-limit и блокирует кнопку attach", async () => {
    const { form, attachBtn, composer } = createTestEnv();

    const files = Array.from({ length: 11 }, (_, i) => ({
      name: `file-${i + 1}.txt`,
      size: 1000,
      type: "text/plain",
    }));

    await composer.addFiles(files);

    const chips = form.querySelectorAll(".attachment-chip");
    assert.equal(chips.length, 11);
    const eleventhChip = /** @type {any} */ (chips[10]);
    assert.equal(eleventhChip.getAttribute("data-state"), "error-limit");

    const count = /** @type {any} */ (form.querySelector("#composer-attach-count"));
    assert.equal(count.getAttribute("data-full"), "true");
    assert.equal(attachBtn.getAttribute("aria-disabled"), "true");
  });
});

describe("B2b-ATTACH-UI: состояния чипов и прогресс", () => {
  it("чип содержит progressbar с aria-valuenow и aria-valuetext", async () => {
    const { form, composer } = createTestEnv();
    await composer.addFiles([{ name: "test.png", size: 2000, type: "image/png" }]);

    const chip = /** @type {any} */ (form.querySelector(".attachment-chip"));
    const progress = chip?.querySelector(".attachment-progress");
    assert.ok(progress, "нет .attachment-progress");
    assert.equal(progress?.getAttribute("role"), "progressbar");
    assert.ok(progress?.hasAttribute("aria-valuenow"));
    assert.ok(progress?.hasAttribute("aria-valuetext"));
  });

  it("удаление чипа по кнопке attachment-remove убирает его из DOM и обновляет счётчик", async () => {
    const { form, composer } = createTestEnv();
    await composer.addFiles([
      { name: "first.png", size: 1000, type: "image/png" },
      { name: "second.png", size: 2000, type: "image/png" },
    ]);

    const chips = form.querySelectorAll(".attachment-chip");
    assert.equal(chips.length, 2);

    const removeBtn = /** @type {any} */ (chips[0].querySelector('[data-action="attachment-remove"]'));
    assert.ok(removeBtn);
    removeBtn.click();

    const remaining = form.querySelectorAll(".attachment-chip");
    assert.equal(remaining.length, 1);
    assert.equal(remaining[0].querySelector(".attachment-name")?.textContent, "second.png");
    assert.equal(form.querySelector("#composer-attach-count")?.textContent, "1 / 10");
  });
});

describe("B2b-ATTACH-UI: транспорт слотов и отправка сообщения", () => {
  it("вызов invoke('attachments_upload_slots') и последующий invoke('attachments_upload_put') переводит чип в uploading -> ready с подтверждённым uploaded_filename", async () => {
    /** @type {any[]} */
    const calls = [];
    const { form, composer } = createTestEnv({
      invoke: async (cmd, args) => {
        calls.push({ cmd, args });
        if (cmd === "attachments_upload_slots") {
          return {
            attachments: [
              {
                id: args.files[0].id,
                upload_url: "https://discord.upload/test",
                upload_filename: "uploaded-test-file.png",
              },
            ],
          };
        }
        if (cmd === "attachments_upload_put") {
          return { ok: true };
        }
        return null;
      },
    });

    const avatarBytes = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    const avatarFile = { name: "avatar.png", size: 5000, type: "image/png", bytes: avatarBytes };
    await composer.addFiles([avatarFile]);

    // Запрос слота и вызов PUT должны уйти в invoke последовательно
    assert.equal(calls.length, 2, "должны быть вызваны attachments_upload_slots и attachments_upload_put");
    assert.equal(calls[0].cmd, "attachments_upload_slots");
    assert.equal(calls[0].args.files[0].filename, "avatar.png");

    assert.equal(calls[1].cmd, "attachments_upload_put");
    assert.equal(calls[1].args.channelId, "default");
    assert.equal(calls[1].args.id, calls[0].args.files[0].id);
    assert.equal(calls[1].args.uploadUrl, "https://discord.upload/test");
    assert.equal(calls[1].args.uploadFilename, "uploaded-test-file.png");
    assert.equal(calls[1].args.filename, "avatar.png");
    assert.equal(calls[1].args.fileSize, 5000);
    assert.equal(calls[1].args.file, avatarFile, "в PUT должна передаваться ссылка на исходный файл");
    assert.deepEqual(calls[1].args.bytes, avatarBytes, "в PUT должны передаваться байты файла");
    assert.deepEqual(calls[1].args.data, avatarBytes, "в PUT должны передаваться данные файла");

    // После успешного PUT чип переходит в ready с подтверждённым uploaded_filename
    const chip = /** @type {any} */ (form.querySelector(".attachment-chip"));
    assert.equal(chip?.getAttribute("data-state"), "ready");
    assert.equal(chip?.getAttribute("aria-busy"), "false");
    assert.equal(chip?.querySelector(".attachment-status")?.textContent, "Готово");
    assert.equal(form.getAttribute("data-attach-state"), "ready");
  });

  it("поддерживает кастомный options.uploadPut вместо invoke('attachments_upload_put')", async () => {
    /** @type {any[]} */
    const putCalls = [];
    const { form, composer } = createTestEnv({
      invoke: async (cmd, args) => {
        if (cmd === "attachments_upload_slots") {
          return {
            attachments: [
              {
                id: args.files[0].id,
                upload_url: "https://discord.upload/custom",
                upload_filename: "custom-uploaded.png",
              },
            ],
          };
        }
        return null;
      },
      uploadPut: async (payload) => {
        putCalls.push(payload);
        return { ok: true };
      },
    });

    const customBytes = new Uint8Array([1, 2, 3, 4]);
    const customFile = new File([customBytes], "custom.png", { type: "image/png" });
    await composer.addFiles([customFile]);

    assert.equal(putCalls.length, 1);
    assert.equal(putCalls[0].uploadUrl, "https://discord.upload/custom");
    assert.equal(putCalls[0].uploadFilename, "custom-uploaded.png");
    assert.equal(putCalls[0].filename, "custom.png");
    assert.equal(putCalls[0].fileSize, 4);
    assert.equal(putCalls[0].file, customFile);
    assert.deepEqual(putCalls[0].bytes, customBytes);
    assert.deepEqual(putCalls[0].data, customBytes);

    const chip = /** @type {any} */ (form.querySelector(".attachment-chip"));
    assert.equal(chip?.getAttribute("data-state"), "ready");
    assert.equal(form.getAttribute("data-attach-state"), "ready");
  });

  it("передаёт экземпляр File и прочитанные байты в attachments_upload_put, сохраняя их при повторе после сбоя", async () => {
    /** @type {any[]} */
    const putPayloads = [];
    let attempt = 0;
    const { form, composer } = createTestEnv({
      invoke: async (cmd, args) => {
        if (cmd === "attachments_upload_slots") {
          return {
            attachments: [
              {
                id: args.files[0].id,
                upload_url: "https://discord.upload/retry-bin",
                upload_filename: "retry-uploaded.bin",
              },
            ],
          };
        }
        if (cmd === "attachments_upload_put") {
          attempt += 1;
          putPayloads.push(args);
          if (attempt === 1) {
            throw new Error("Temporary network error");
          }
          return { ok: true };
        }
        return null;
      },
    });

    const rawBytes = new Uint8Array([10, 20, 30, 40, 50]);
    const realFile = new File([rawBytes], "payload.bin", { type: "application/octet-stream" });

    await composer.addFiles([realFile]);

    assert.equal(putPayloads.length, 1);
    assert.equal(putPayloads[0].file, realFile);
    assert.deepEqual(putPayloads[0].bytes, rawBytes);
    assert.deepEqual(putPayloads[0].data, rawBytes);

    const chip = /** @type {any} */ (form.querySelector(".attachment-chip"));
    assert.equal(chip?.getAttribute("data-state"), "error-network");

    const retryBtn = /** @type {any} */ (chip?.querySelector('[data-action="attachment-retry"]'));
    assert.ok(retryBtn && !retryBtn.hidden, "кнопка повтора должна быть доступна");
    retryBtn.click();
    await new Promise((resolve) => setTimeout(resolve, 10));

    assert.equal(putPayloads.length, 2, "повтор должен снова вызвать attachments_upload_put");
    assert.equal(putPayloads[1].file, realFile, "при повторе ссылка на File сохраняется");
    assert.deepEqual(putPayloads[1].bytes, rawBytes, "при повторе байты файла передаются снова");
    assert.equal(chip?.getAttribute("data-state"), "ready");
  });

  it("ошибка сервера 413 при выдаче слота переводит чип в error-413", async () => {
    const { form, composer } = createTestEnv({
      invoke: async () => {
        const err = Object.assign(new Error("EntityTooLarge"), { status: 413 });
        throw err;
      },
    });

    await composer.addFiles([{ name: "server-reject.png", size: 5000, type: "image/png" }]);

    const chip = /** @type {any} */ (form.querySelector(".attachment-chip"));
    assert.equal(chip?.getAttribute("data-state"), "error-413");
    assert.equal(form.getAttribute("data-attach-state"), "error");
  });

  it("ошибка сервера 429 при выдаче слота переводит чип в error-429", async () => {
    const { form, composer } = createTestEnv({
      invoke: async () => {
        const err = Object.assign(new Error("RateLimited"), {
          status: 429,
          retry_after_ms: 3000,
        });
        throw err;
      },
    });

    await composer.addFiles([{ name: "rate-limited.png", size: 5000, type: "image/png" }]);

    const chip = /** @type {any} */ (form.querySelector(".attachment-chip"));
    assert.equal(chip?.getAttribute("data-state"), "error-429");
  });

  it("ошибка PUT (413) переводит чип в error-413 и блокирует отправку сообщения", async () => {
    let sent = false;
    const { form, input, composer } = createTestEnv({
      invoke: async (cmd, args) => {
        if (cmd === "attachments_upload_slots") {
          return {
            attachments: [
              {
                id: args.files[0].id,
                upload_url: "https://upload.url",
                upload_filename: "big-file.bin",
              },
            ],
          };
        }
        if (cmd === "attachments_upload_put") {
          const err = Object.assign(new Error("EntityTooLarge"), { status: 413 });
          throw err;
        }
        return null;
      },
      onSubmit: () => {
        sent = true;
      },
    });

    await composer.addFiles([{ name: "big.bin", size: 5000, type: "application/octet-stream" }]);

    const chip = /** @type {any} */ (form.querySelector(".attachment-chip"));
    assert.equal(chip?.getAttribute("data-state"), "error-413");
    assert.equal(form.getAttribute("data-attach-state"), "error");

    input.value = "Попытка отправки";
    input.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(sent, false, "отправка заблокирована при ошибке PUT");
  });

  it("ошибка PUT (429) переводит чип в error-429 и блокирует отправку сообщения", async () => {
    let sent = false;
    const { form, input, composer } = createTestEnv({
      invoke: async (cmd, args) => {
        if (cmd === "attachments_upload_slots") {
          return {
            attachments: [
              {
                id: args.files[0].id,
                upload_url: "https://upload.url",
                upload_filename: "rl-file.bin",
              },
            ],
          };
        }
        if (cmd === "attachments_upload_put") {
          const err = Object.assign(new Error("RateLimited"), { status: 429, retry_after_ms: 2000 });
          throw err;
        }
        return null;
      },
      onSubmit: () => {
        sent = true;
      },
    });

    await composer.addFiles([{ name: "rl.bin", size: 5000, type: "application/octet-stream" }]);

    const chip = /** @type {any} */ (form.querySelector(".attachment-chip"));
    assert.equal(chip?.getAttribute("data-state"), "error-429");
    assert.equal(form.getAttribute("data-attach-state"), "error");

    input.value = "Попытка отправки";
    input.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(sent, false, "отправка заблокирована при ошибке PUT");
  });

  it("сетевая ошибка PUT переводит чип в error-network и блокирует отправку сообщения", async () => {
    let sent = false;
    const { form, input, composer } = createTestEnv({
      invoke: async (cmd, args) => {
        if (cmd === "attachments_upload_slots") {
          return {
            attachments: [
              {
                id: args.files[0].id,
                upload_url: "https://upload.url",
                upload_filename: "net-file.bin",
              },
            ],
          };
        }
        if (cmd === "attachments_upload_put") {
          throw new Error("Network timeout");
        }
        return null;
      },
      onSubmit: () => {
        sent = true;
      },
    });

    await composer.addFiles([{ name: "net.bin", size: 5000, type: "application/octet-stream" }]);

    const chip = /** @type {any} */ (form.querySelector(".attachment-chip"));
    assert.equal(chip?.getAttribute("data-state"), "error-network");
    assert.equal(form.getAttribute("data-attach-state"), "error");

    input.value = "Попытка отправки";
    input.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(sent, false, "отправка заблокирована при ошибке PUT");
  });

  it("отсутствие слота или upload_filename не синтезирует fallback и переводит чип в ошибку", async () => {
    let sent = false;
    const { form, input, composer } = createTestEnv({
      invoke: async (cmd) => {
        if (cmd === "attachments_upload_slots") {
          return { attachments: [] };
        }
        return null;
      },
      onSubmit: () => {
        sent = true;
      },
    });

    await composer.addFiles([{ name: "noslot.png", size: 3000, type: "image/png" }]);

    const chip = /** @type {any} */ (form.querySelector(".attachment-chip"));
    assert.notEqual(chip?.getAttribute("data-state"), "ready");
    assert.equal(form.getAttribute("data-attach-state"), "error");

    input.value = "Проверка";
    input.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(sent, false, "нельзя отправить файл без подтверждённого слота");
  });

  it("отправка сообщения блокируется при наличии ошибок вложения", async () => {
    let sent = false;
    const { input, composer } = createTestEnv({
      onSubmit: () => {
        sent = true;
      },
    });

    await composer.addFiles([{ name: "empty.txt", size: 0, type: "text/plain" }]);
    input.value = "Привет";

    input.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(sent, false, "отправка не должна происходить при наличии ошибки вложения");
  });

  it("при отправке сообщения передаются uploaded_filename готовых вложений и очередь очищается", async () => {
    /** @type {any} */
    let submittedPayload = null;
    const { form, input, composer } = createTestEnv({
      invoke: async (cmd, args) => {
        if (cmd === "attachments_upload_slots") {
          return {
            attachments: [
              {
                id: args.files[0].id,
                upload_url: "https://upload.url",
                upload_filename: "server-uuid-photo.jpg",
              },
            ],
          };
        }
        return null;
      },
      onSubmit: (text, attachments) => {
        submittedPayload = { text, attachments };
      },
    });

    await composer.addFiles([{ name: "photo.jpg", size: 4000, type: "image/jpeg" }]);

    input.value = "Смотри фото";
    input.dispatchEvent("keydown", { key: "Enter" });

    assert.ok(submittedPayload, "сообщение должно быть отправлено");
    assert.equal(submittedPayload.text, "Смотри фото");
    assert.equal(submittedPayload.attachments.length, 1);
    assert.equal(submittedPayload.attachments[0].uploaded_filename, "server-uuid-photo.jpg");

    // Очередь очищена после отправки
    assert.equal(form.querySelectorAll(".attachment-chip").length, 0);
    assert.equal(form.getAttribute("data-attach-state"), "none");
  });
});

describe("STUB-12-GIFT-UI: модалка подарка #gift-modal", () => {
  it("клик по [data-action='composer-gift'] открывает #gift-modal и выставляет aria-expanded='true'", () => {
    const { doc, giftBtn } = createTestEnv();

    assert.equal(giftBtn.getAttribute("aria-expanded"), "false");
    giftBtn.click();

    const modal = /** @type {any} */ (doc.getElementById("gift-modal"));
    assert.ok(modal, "нет #gift-modal");
    assert.equal(modal.hidden, false, "#gift-modal должен быть виден");
    assert.equal(giftBtn.getAttribute("aria-expanded"), "true");

    const card = /** @type {any} */ (modal.querySelector(".gift-modal-card"));
    assert.ok(card);
    assert.equal(card.getAttribute("role"), "dialog");
    assert.equal(card.getAttribute("aria-modal"), "true");
    assert.equal(card.getAttribute("data-state"), "ready");
  });

  it("отображает честный статус Nitro-каталога без фейковой оплаты", () => {
    const { doc, giftBtn } = createTestEnv();
    giftBtn.click();

    const modal = doc.getElementById("gift-modal");
    const notice = /** @type {any} */ (modal?.querySelector(".gift-catalog-notice") ?? modal?.querySelector("#gift-catalog-status"));
    assert.ok(notice, "нет блока статуса Nitro-каталога");
    assert.match(notice.textContent ?? "", /каталог\s*nitro|режим просмотра|без фейковой оплаты|спайк/i);
  });

  it("закрывается по Escape и возвращает фокус на кнопку подарка", () => {
    const { doc, giftBtn } = createTestEnv();
    giftBtn.click();
    assert.equal(giftBtn.getAttribute("aria-expanded"), "true");

    const modal = /** @type {any} */ (doc.getElementById("gift-modal"));
    // Эмитируем Escape на документе/модалке
    modal?.dispatchEvent("keydown", { key: "Escape" });

    assert.equal(modal?.hidden, true, "модалка должна закрыться по Escape");
    assert.equal(giftBtn.getAttribute("aria-expanded"), "false");
    assert.equal(modal?.querySelector(".gift-modal-card")?.getAttribute("data-state"), "closed");
  });

  it("закрывается по клику на backdrop", () => {
    const { doc, giftBtn } = createTestEnv();
    giftBtn.click();

    const modal = /** @type {any} */ (doc.getElementById("gift-modal"));
    assert.equal(modal?.hidden, false);

    // Клик именно по backdrop
    modal?.click();
    assert.equal(modal?.hidden, true);
    assert.equal(giftBtn.getAttribute("aria-expanded"), "false");
  });

  it("закрывается по кнопке закрытия [data-action='gift-close']", () => {
    const { doc, giftBtn } = createTestEnv();
    giftBtn.click();

    const modal = /** @type {any} */ (doc.getElementById("gift-modal"));
    const closeBtn = /** @type {any} */ (modal?.querySelector('[data-action="gift-close"]'));
    assert.ok(closeBtn);
    closeBtn.click();

    assert.equal(modal?.hidden, true);
    assert.equal(giftBtn.getAttribute("aria-expanded"), "false");
  });

  it("клик при aria-disabled='true' не открывает модалку", () => {
    const { doc, giftBtn } = createTestEnv();
    giftBtn.setAttribute("aria-disabled", "true");

    giftBtn.click();
    const modal = /** @type {any} */ (doc.getElementById("gift-modal"));
    assert.ok(modal?.hidden !== false, "модалка не должна открываться при aria-disabled=true");
    assert.equal(giftBtn.getAttribute("aria-expanded"), "false");
  });
});

describe("P1-MEMBER-VERIFICATION: блокировка композера и обработка 403 Forbidden", () => {
  it("evaluateClientChannelPolicy выставляет canSend=false и использует custom permissionDeniedReason", () => {
    const policy = evaluateClientChannelPolicy({
      channel: {
        id: "123",
        name: "rules",
        canSendMessages: false,
        permissionDeniedReason: "Примите правила сервера для отправки сообщений",
      },
      userId: "999",
    });

    assert.equal(policy.canSend, false);
    assert.equal(policy.disabledReason, "Примите правила сервера для отправки сообщений");
    assert.equal(policy.placeholder, "Примите правила сервера для отправки сообщений");
  });

  it("applyChannelPolicy(canSend=false) переводит input в disabled=true и aria-disabled=true", () => {
    let submitted = false;
    const { input, composer } = createTestEnv({
      onSubmit: () => {
        submitted = true;
      },
    });

    composer.applyChannelPolicy({
      canSend: false,
      disabledReason: "Примите правила сервера для отправки сообщений",
      placeholder: "Примите правила сервера для отправки сообщений",
    });

    assert.equal(input.disabled, true, "input.disabled обязан быть true");
    assert.equal(input.getAttribute("aria-disabled"), "true");
    assert.equal(input.placeholder, "Примите правила сервера для отправки сообщений");

    // Попытка нажать Enter при заблокированном композере не должна отправлять сообщение
    input.value = "Привет";
    input.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(submitted, false, "сообщение не должно отправляться при disabled");
  });

  it("applyChannelPolicy(canSend=true) разблокирует input", () => {
    const { input, composer } = createTestEnv();

    composer.applyChannelPolicy({
      canSend: false,
      disabledReason: "Примите правила сервера для отправки сообщений",
      placeholder: "Примите правила сервера для отправки сообщений",
    });
    assert.equal(input.disabled, true);

    composer.applyChannelPolicy({
      canSend: true,
      placeholder: "Написать в #general",
    });
    assert.equal(input.disabled, false);
    assert.equal(input.getAttribute("aria-disabled"), null);
    assert.equal(input.placeholder, "Написать в #general");
  });
});
