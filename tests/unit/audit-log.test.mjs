// @ts-check
/**
 * task-225 (FE-P2-24): контракт меток действий журнала аудита сервера и
 * форматтера диффа изменений.
 *
 * Проверяет: точные карты `AUDIT_ACTION_TYPES`/`AUDIT_ACTION_LABELS_RU`
 * (включая неизменяемость), формат строки записи `актор: метка (цель)` с
 * опциональной причиной и фолбэком «Неизвестный», формат диффа
 * `ключ: было → стало` с подстановкой «нет» на пустых значениях.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  AUDIT_ACTION_LABELS_RU,
  AUDIT_ACTION_TYPES,
  formatAuditChangeDiffRu,
  formatAuditLogEntryRu,
} from "../../rust/gui-spike/ui/features/audit-log.js";

describe("AUDIT_ACTION_TYPES", () => {
  it("содержит точные коды действий Discord", () => {
    assert.deepEqual({ ...AUDIT_ACTION_TYPES }, {
      GUILD_UPDATE: 1,
      CHANNEL_CREATE: 10,
      CHANNEL_UPDATE: 11,
      CHANNEL_DELETE: 12,
      MEMBER_KICK: 20,
      MEMBER_BAN_ADD: 22,
      MEMBER_BAN_REMOVE: 23,
      ROLE_CREATE: 30,
      ROLE_UPDATE: 31,
      ROLE_DELETE: 32,
      MESSAGE_DELETE: 72,
    });
  });

  it("заморожен: мутация не меняет карту", () => {
    assert.ok(Object.isFrozen(AUDIT_ACTION_TYPES));
    assert.throws(() => {
      // @ts-expect-error проверка неизменяемости в рантайме
      AUDIT_ACTION_TYPES.GUILD_UPDATE = 999;
    }, TypeError);
    assert.equal(AUDIT_ACTION_TYPES.GUILD_UPDATE, 1);
  });
});

describe("AUDIT_ACTION_LABELS_RU", () => {
  it("покрывает все коды из AUDIT_ACTION_TYPES русскими метками", () => {
    assert.deepEqual({ ...AUDIT_ACTION_LABELS_RU }, {
      1: "Обновление сервера",
      10: "Создание канала",
      11: "Изменение канала",
      12: "Удаление канала",
      20: "Исключение участника",
      22: "Бан участника",
      23: "Разбан участника",
      30: "Создание роли",
      31: "Изменение роли",
      32: "Удаление роли",
      72: "Удаление сообщения",
    });
    for (const code of Object.values(AUDIT_ACTION_TYPES)) {
      assert.equal(typeof AUDIT_ACTION_LABELS_RU[code], "string");
      assert.notEqual(AUDIT_ACTION_LABELS_RU[code], "");
    }
  });

  it("заморожен", () => {
    assert.ok(Object.isFrozen(AUDIT_ACTION_LABELS_RU));
    assert.throws(() => {
      // @ts-expect-error проверка неизменяемости в рантайме
      AUDIT_ACTION_LABELS_RU[1] = "другое";
    }, TypeError);
    assert.equal(AUDIT_ACTION_LABELS_RU[1], "Обновление сервера");
  });
});

describe("formatAuditLogEntryRu", () => {
  it("форматирует запись с целью без причины", () => {
    assert.equal(
      formatAuditLogEntryRu({
        actionType: AUDIT_ACTION_TYPES.CHANNEL_CREATE,
        actorName: "Алиса",
        targetName: "#общий",
      }),
      "Алиса: Создание канала (#общий)",
    );
  });

  it("добавляет причину через тире, обрезая пробелы", () => {
    assert.equal(
      formatAuditLogEntryRu({
        actionType: 22,
        actorName: "Модератор",
        targetName: "Боб",
        reason: "  спам  ",
      }),
      "Модератор: Бан участника (Боб) — Причина: спам",
    );
  });

  it("опускает пустую и пробельную причину", () => {
    const base = { actionType: 72, actorName: "Алиса", targetName: "сообщение" };
    assert.equal(
      formatAuditLogEntryRu({ ...base, reason: "" }),
      "Алиса: Удаление сообщения (сообщение)",
    );
    assert.equal(
      formatAuditLogEntryRu({ ...base, reason: "   " }),
      "Алиса: Удаление сообщения (сообщение)",
    );
    assert.equal(
      formatAuditLogEntryRu({ ...base, reason: null }),
      "Алиса: Удаление сообщения (сообщение)",
    );
  });

  it("пропускает цель, если её нет", () => {
    assert.equal(
      formatAuditLogEntryRu({ actionType: 1, actorName: "Алиса", targetName: "" }),
      "Алиса: Обновление сервера",
    );
    assert.equal(
      formatAuditLogEntryRu({ actionType: 1, actorName: "Алиса", targetName: null }),
      "Алиса: Обновление сервера",
    );
  });

  it("подставляет «Неизвестный» на пустом акторе", () => {
    assert.equal(
      formatAuditLogEntryRu({ actionType: 30, actorName: "", targetName: "role" }),
      "Неизвестный: Создание роли (role)",
    );
    assert.equal(
      formatAuditLogEntryRu({ actionType: 30, targetName: "role" }),
      "Неизвестный: Создание роли (role)",
    );
  });

  it("не падает на неизвестном действии и отдаёт числовой фолбэк", () => {
    assert.equal(
      formatAuditLogEntryRu({ actionType: 999, actorName: "Алиса", targetName: "x" }),
      "Алиса: Действие 999 (x)",
    );
  });
});

describe("formatAuditChangeDiffRu", () => {
  it("форматирует изменение строкового значения", () => {
    assert.equal(
      formatAuditChangeDiffRu({ key: "name", oldValue: "старое", newValue: "новое" }),
      "name: старое → новое",
    );
  });

  it("подставляет «нет» на пустых сторонах", () => {
    assert.equal(
      formatAuditChangeDiffRu({ key: "topic", oldValue: null, newValue: "тема" }),
      "topic: нет → тема",
    );
    assert.equal(
      formatAuditChangeDiffRu({ key: "topic", oldValue: "тема", newValue: undefined }),
      "topic: тема → нет",
    );
  });

  it("не считает falsy-значения пустыми", () => {
    assert.equal(
      formatAuditChangeDiffRu({ key: "nsfw", oldValue: false, newValue: 0 }),
      "nsfw: false → 0",
    );
    assert.equal(
      formatAuditChangeDiffRu({ key: "empty", oldValue: "", newValue: "x" }),
      "empty:  → x",
    );
  });
});
