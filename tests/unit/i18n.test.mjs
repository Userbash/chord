// @ts-check
/**
 * TASK-194 (RW-40 / G2): русский каталог i18n (`rust/gui-spike/ui/i18n.js`).
 *
 * Независимый оракул:
 * - `/home/sanya/re-deep/i18n-deep.md` (§1 хеш-ключи base64 xxh64[0..6], §2 плурализация CLDR ru, §6 числа/размеры)
 * - `/home/sanya/re-deep/ru-catalog.json` (канонические 6-символьные ключи и русские строки Discord 1.0.162)
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  DEFAULT_LOCALE,
  FORBIDDEN_ENGLISH_LABELS,
  HASHED_RU_KEYS,
  RU_CATALOG,
  auditUntranslatedDomText,
  formatDate,
  formatNumber,
  plural,
  t,
  tHashed,
} from "../../rust/gui-spike/ui/i18n.js";

describe("HASHED_RU_KEYS и RU_CATALOG (оракул /home/sanya/re-deep/ru-catalog.json)", () => {
  it("экспортирует замороженную карту >= 25 канонических 6-символьных base64 xxh64 ключей Discord", () => {
    assert.equal(DEFAULT_LOCALE, "ru");
    assert.equal(Object.isFrozen(HASHED_RU_KEYS), true);
    assert.equal(Object.isFrozen(RU_CATALOG), true);
    assert.equal(Object.isFrozen(FORBIDDEN_ENGLISH_LABELS), true);

    const keys = Object.keys(HASHED_RU_KEYS);
    assert.ok(
      keys.length >= 25,
      `ожидалось >= 25 хеш-ключей, получено ${keys.length}`,
    );

    /** @type {Record<string, string>} */
    const map = HASHED_RU_KEYS;
    for (const key of keys) {
      assert.match(
        key,
        /^[A-Za-z0-9+/]{6}$/,
        `ключ ${key} должен быть 6-символьным base64 xxh64 хешем`,
      );
      const value = map[key] ?? "";
      assert.equal(typeof value, "string");
      assert.ok(value.length > 0, `пустое значение для ключа ${key}`);
    }
  });

  it("содержит точные русские строки из ru-catalog.json по всем 10 категориям UI", () => {
    // Inbox & Notifications
    assert.equal(HASHED_RU_KEYS["GSmTKJ"], "Почта");
    assert.equal(HASHED_RU_KEYS["NaZyYC"], "Уведомления");
    assert.equal(HASHED_RU_KEYS["e6RscS"], "Пометить как прочитанное");
    assert.equal(HASHED_RU_KEYS["M3tKAP"], "Помечено как прочитанное");

    // Search
    assert.equal(HASHED_RU_KEYS["pw+r5b"], "Поиск");
    assert.equal(HASHED_RU_KEYS["41014u"], "По вашему запросу ничего не найдено");

    // Settings & Locales (i18n-deep.md §1)
    assert.equal(HASHED_RU_KEYS["QV4/6p"], "Настройки");
    assert.equal(HASHED_RU_KEYS["cduTBL"], "Настройки пользователя");
    assert.equal(HASHED_RU_KEYS["+irF5O"], "Русский");
    assert.equal(HASHED_RU_KEYS["+dXJII"], "Английский, США");
    assert.equal(HASHED_RU_KEYS["TyCVIq"], "Хорошо");

    // Threads & Pins
    assert.equal(HASHED_RU_KEYS["B2panI"], "Ветки");
    assert.equal(HASHED_RU_KEYS["mp1N/2"], "Закреплённые сообщения");

    // Polls
    assert.equal(HASHED_RU_KEYS["Flr51u"], "Создать опрос");
    assert.equal(HASHED_RU_KEYS["RgIi2B"], "Опрос");

    // Voice
    assert.equal(HASHED_RU_KEYS["daXg45"], "Голосовая связь подключена");
    assert.equal(HASHED_RU_KEYS["PygWIS"], "Переподключение");
    assert.equal(HASHED_RU_KEYS["juL9/L"], "Голосовая связь отключена");
    assert.equal(HASHED_RU_KEYS["lhPHmz"], "Этот микрофон отключён");
    assert.equal(HASHED_RU_KEYS["HITUcR"], "Заглушить");
    assert.equal(HASHED_RU_KEYS["wjcRFX"], "Откл. звук");
    assert.equal(HASHED_RU_KEYS["6vrfgt"], "Отключиться");

    // Attachments & Composer
    assert.equal(HASHED_RU_KEYS["d3+iYs"], "Отправить файл");
    assert.equal(HASHED_RU_KEYS["QuPY4s"], "Отправить сообщение");
    assert.equal(HASHED_RU_KEYS["3Gc2XP"], "Файл скрыт под спойлером");

    // Tooltips & Message Actions
    assert.equal(HASHED_RU_KEYS["lfIHs4"], "Добавить реакцию");
    assert.equal(HASHED_RU_KEYS["5IEsGx"], "Ответить");
    assert.equal(HASHED_RU_KEYS["ZXE1s4"], "Редактировать сообщение");
    assert.equal(HASHED_RU_KEYS["JrGD7E"], "Скопировать текст");
    assert.equal(HASHED_RU_KEYS["zBoHlf"], "Копировать ID сообщения");
    assert.equal(HASHED_RU_KEYS["xwMqD7"], "Удалить сообщение");

    // Shortcuts
    assert.equal(HASHED_RU_KEYS["T9DA2K"], "Горячие клавиши");
    assert.equal(HASHED_RU_KEYS["43Qn2r"], "Список горячих клавиш");

    // Errors
    assert.equal(
      HASHED_RU_KEYS["S69lJR"],
      "Не удаётся подключить вас к этому каналу. Повторите попытку позже.",
    );
    assert.equal(
      HASHED_RU_KEYS["PW7+do"],
      "Недостаточно прав для выполнения запроса.",
    );
    assert.equal(
      HASHED_RU_KEYS["1BRkQ1"],
      "Сетевое подключение ограничено или недоступно.",
    );
    assert.equal(HASHED_RU_KEYS["sAfT1K"], "Не удалось загрузить изображение.");
  });
});

describe("tHashed и t — разрешение ключей, плейсхолдеры и русские плюральные формы", () => {
  it("разрешает канонические 6-символьные хеш-ключи и семантические ключи RU_CATALOG", () => {
    assert.equal(tHashed("GSmTKJ"), "Почта");
    assert.equal(tHashed("pw+r5b"), "Поиск");
    assert.equal(tHashed("QV4/6p"), "Настройки");
    assert.equal(tHashed("HITUcR"), "Заглушить");
    assert.equal(tHashed("inbox.title"), "Почта");
    assert.equal(tHashed("search.placeholder"), "Поиск");
    assert.equal(tHashed("settings.title"), "Настройки");
    assert.equal(t("voice.connected"), "Голосовая связь подключена");
  });

  it("подставляет {param} плейсхолдеры (включая повторяющиеся вхождения и числа)", () => {
    assert.equal(
      tHashed("c1qUOQ", { keybind: "Ctrl+Shift+M" }),
      "Режим рации (Ctrl+Shift+M) активирован",
    );
    assert.equal(
      tHashed("DYFPg2", { maxSizeMb: 25 }),
      "Слишком большой размер вложений. Максимальный общий размер\u00A0— 25\u00A0МБ.",
    );
    assert.equal(
      tHashed("BagU2U", { online: 42 }),
      "В сети\u00A0— 42",
    );
    assert.equal(
      tHashed("composer.placeholder", { channel: "general" }),
      "Написать сообщение в #general",
    );
  });

  it("выбирает русские плюральные формы CLDR (one/few/many/other) для канонических ключей i18n-deep.md §2", () => {
    // pAQJsJ: 1 участник, 2 участника, 5 участников, 11 участников, 21 участник, 22 участника
    const memberCases = [
      [1, "1 участник"],
      [2, "2 участника"],
      [5, "5 участников"],
      [11, "11 участников"],
      [21, "21 участник"],
      [22, "22 участника"],
      [0, "0 участников"],
    ];
    for (const [count, expected] of memberCases) {
      assert.equal(
        tHashed("pAQJsJ", { count }),
        expected,
        `pAQJsJ count=${count}`,
      );
    }

    // r/EUap: игра / игры / игр
    assert.equal(tHashed("r/EUap", { count: 1 }), "1 игра");
    assert.equal(tHashed("r/EUap", { count: 3 }), "3 игры");
    assert.equal(tHashed("r/EUap", { count: 5 }), "5 игр");
    assert.equal(tHashed("r/EUap", { count: 12 }), "12 игр");
    assert.equal(tHashed("r/EUap", { count: 21 }), "21 игра");
    assert.equal(tHashed("r/EUap", { count: 24 }), "24 игры");

    // 4Y3O+O: результаты поиска
    assert.equal(tHashed("4Y3O+O", { count: 1 }), "Найден 1 результат");
    assert.equal(tHashed("4Y3O+O", { count: 2 }), "Найдено 2 результата");
    assert.equal(tHashed("4Y3O+O", { count: 5 }), "Найдено 5 результатов");
    assert.equal(tHashed("4Y3O+O", { count: 11 }), "Найдено 11 результатов");
    assert.equal(tHashed("4Y3O+O", { count: 21 }), "Найден 21 результат");
  });

  it("поддерживает ICU-подобный синтаксис {count, plural, one {...} few {...} many {...} other {...}} и пресеты", () => {
    const icu =
      "В канале {channel}: {count, plural, one {{count} новое сообщение} few {{count} новых сообщения} many {{count} новых сообщений} other {{count} новых сообщений}}";
    assert.equal(
      tHashed(icu, { channel: "dev", count: 1 }),
      "В канале dev: 1 новое сообщение",
    );
    assert.equal(
      tHashed(icu, { channel: "dev", count: 4 }),
      "В канале dev: 4 новых сообщения",
    );
    assert.equal(
      tHashed(icu, { channel: "dev", count: 14 }),
      "В канале dev: 14 новых сообщений",
    );
    assert.equal(
      tHashed(icu, { channel: "dev", count: 21 }),
      "В канале dev: 21 новое сообщение",
    );

    // Пресеты из plural-format.js также доступны через tHashed
    assert.equal(
      tHashed("unreadMessages", { count: 1 }),
      "1 непрочитанное сообщение",
    );
    assert.equal(
      tHashed("unreadMessages", { count: 3 }),
      "3 непрочитанных сообщения",
    );
    assert.equal(
      tHashed("unreadMessages", { count: 11 }),
      "11 непрочитанных сообщений",
    );
  });

  it("безопасен на неизвестных, пустых и прототипных ключах", () => {
    assert.equal(tHashed("unknown_key_xyz"), "");
    assert.equal(tHashed(""), "");
    assert.equal(tHashed(/** @type {any} */ (null)), "");
    assert.equal(tHashed("toString"), "");
    assert.equal(tHashed("valueOf"), "");
    assert.equal(tHashed("constructor"), "");
  });

  it("форматирует числа с неразрывным пробелом ru-RU и плюральные формы через хелперы", () => {
    assert.equal(formatNumber(24229), "24\u00A0229");
    assert.equal(
      plural(2, {
        one: "{count} ветка",
        few: "{count} ветки",
        many: "{count} веток",
      }),
      "2 ветки",
    );
    const formattedDate = formatDate(new Date("2026-10-30T12:00:00Z"), "ru-RU", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      timeZone: "UTC",
    });
    assert.equal(formattedDate, "30.10.2026");
  });
});

describe("auditUntranslatedDomText — аудит непереведённых английских подписей в DOM", () => {
  it("возвращает пустой массив для полностью локализованного русского DOM-дерева", () => {
    const ruRoot = {
      tagName: "DIV",
      attributes: { "aria-label": "Главный экран" },
      children: [
        {
          tagName: "INPUT",
          getAttribute(/** @type {string} */ name) {
            return name === "placeholder" ? "Поиск" : null;
          },
          children: [],
        },
        {
          tagName: "BUTTON",
          getAttribute(/** @type {string} */ name) {
            if (name === "aria-label") return "Отправить сообщение";
            if (name === "title") return "Заглушить";
            return null;
          },
          textContent: "Отправить сообщение",
          children: [],
        },
        {
          tagName: "SPAN",
          textContent: "Настройки",
          children: [],
        },
      ],
    };

    assert.deepEqual(auditUntranslatedDomText(ruRoot), []);
  });

  it("обнаруживает запрещённые английские метки в текстовых узлах и атрибутах aria-label/placeholder/title", () => {
    const mixedRoot = {
      tagName: "SECTION",
      childNodes: [
        { nodeType: 3, nodeValue: "  Send Message  " },
        {
          tagName: "INPUT",
          getAttribute(/** @type {string} */ name) {
            if (name === "placeholder") return "Search";
            if (name === "aria-label") return "Upload File";
            return null;
          },
          childNodes: [],
        },
        {
          tagName: "DIV",
          getAttribute(/** @type {string} */ name) {
            return name === "title" ? "Settings" : null;
          },
          childNodes: [
            {
              tagName: "BUTTON",
              getAttribute(/** @type {string} */ name) {
                return name === "aria-label" ? "Mute" : null;
              },
              textContent: "Deafen",
              childNodes: [],
            },
            {
              tagName: "BUTTON",
              getAttribute(/** @type {string} */ name) {
                return name === "title" ? "Disconnect" : null;
              },
              textContent: "Отключиться",
              childNodes: [],
            },
          ],
        },
      ],
    };

    const hits = auditUntranslatedDomText(mixedRoot);
    assert.deepEqual(hits, [
      "Send Message",
      "Upload File",
      "Search",
      "Settings",
      "Mute",
      "Deafen",
      "Disconnect",
    ]);
  });

  it("корректно обрабатывает null/undefined и дедуплицирует повторяющиеся нарушения", () => {
    assert.deepEqual(auditUntranslatedDomText(null), []);
    assert.deepEqual(auditUntranslatedDomText(undefined), []);
    assert.deepEqual(auditUntranslatedDomText("not-an-element"), []);

    const dupRoot = {
      tagName: "DIV",
      getAttribute(/** @type {string} */ name) {
        return name === "aria-label" ? "Search" : null;
      },
      children: [
        { tagName: "SPAN", textContent: "Search", children: [] },
        { tagName: "SPAN", textContent: "Settings", children: [] },
      ],
    };
    assert.deepEqual(auditUntranslatedDomText(dupRoot), ["Search", "Settings"]);
  });
});
