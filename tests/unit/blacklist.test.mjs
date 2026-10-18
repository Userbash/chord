// @ts-check
// RED (M1): реализация — src/core/ui/blacklist.mjs. Контракт: объединение
// чёрных списков всех прогонов 2026-10-18 (settings / menus / extra).
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { isBlacklisted } from "../../src/core/ui/blacklist.mjs";

const walkFixtures = [
  fileURLToPath(new URL("../fixtures/logs/walk-settings.jsonl", import.meta.url)),
  fileURLToPath(new URL("../fixtures/logs/walk-menus.jsonl", import.meta.url)),
];

describe("isBlacklisted", () => {
  it("блокирует выход и покидание сервера", () => {
    for (const label of ["Выйти", "Log Out", "Sign Out", "Покинуть сервер", "Leave Server", "Отключиться", "Выйти из аккаунта"]) {
      assert.equal(isBlacklisted(label), true, label);
    }
  });

  it("блокирует удаление и блокировку", () => {
    for (const label of ["Удалить сообщение", "Delete Server", "Заблокировать пользователя", "Block User"]) {
      assert.equal(isBlacklisted(label), true, label);
    }
  });

  it("блокирует mute/deafen и скрытие каналов", () => {
    for (const label of ["Заглушить", "Mute Channel", "Deafen", "Скрыть заглушенные каналы", "Показать скрытые каналы"]) {
      assert.equal(isBlacklisted(label), true, label);
    }
  });

  it("блокирует сброс/очистку/забыть/отписаться/статус", () => {
    for (const label of ["Сбросить настройки", "Reset", "Очистить кэш", "Забыть устройство", "Отписаться от категории", "Статус"]) {
      assert.equal(isBlacklisted(label), true, label);
    }
  });

  it("не блокирует обычные пункты настроек и меню", () => {
    const allowed = [
      "Мой аккаунт",
      "Профиль",
      "Голос и видео",
      "Уведомления",
      "Настройки пользователя",
      "Пометить как прочитанное",
      "Закрепить вверху",
      "Параметры уведомлений",
      "Пригласить",
      "Копировать ID",
      "Копировать ссылку",
      "Показать все каналы",
    ];
    for (const label of allowed) {
      assert.equal(isBlacklisted(label), false, label);
    }
  });

  // Источники: logs/ui-walk-settings.jsonl (bf5b40e345d8b86d71cfcbf1b50d0948),
  // logs/ui-walk-menus.jsonl (de63e05ea72c09185384426d775be99d).
  // Оракулы: jq -r '.label' | sort -u | wc -l = 56 и 46; для меню 6 повторов.
  // Строгое объединённое правило блокирует три реальные подписи (закреплено
  // кейсами выше): «Статус учётной записи» (settings, шаг 4 — «статус»),
  // «Игнорировать и заблокировать» (settings, шаг 26 — «заблокировать»),
  // «Меню сервера2 → Скрыть заглушенные каналы» (menus, шаг 38 — «скрыт»).
  // Остальные записи журналов обязаны оставаться разрешёнными.
  it("блокирует ровно три подписи-исключения из реальных журналов, остальные разрешены", async () => {
    const strictBlocked = new Set([
      "Статус учётной записи",
      "Игнорировать и заблокировать",
      "Меню сервера2 → Скрыть заглушенные каналы",
    ]);
    for (const path of walkFixtures) {
      const records = (await readFile(path, "utf8")).trim().split("\n").map((line) => JSON.parse(line));
      for (const { label } of records) assert.equal(isBlacklisted(label), strictBlocked.has(label), label);
    }
  });

  it("блокирует запрещённые подстроки независимо от позиции в подписи", () => {
    /** @type {Array<[string, boolean]>} */
    const cases = [
      ["Выйти из аккаунта", true], ["Выйти", true], ["Log Out", true],
      ["LogOut", true], ["Sign Out", true], ["SignOut", true],
      ["Отключиться", true], ["Удалить сервер", true],
      ["Выйти и сохранить изменения", true], ["Сначала выйти", true],
      ["Профиль", false], ["Мой аккаунт", false],
    ];
    for (const [label, expected] of cases) assert.equal(isBlacklisted(label), expected, label);
  });
});
