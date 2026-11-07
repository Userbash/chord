// @ts-check
import test from "node:test";
import assert from "node:assert/strict";
import {
  EVENT_ENTITY_LABELS_RU,
  EVENT_STATUS_LABELS_RU,
  formatEventSummaryRu,
  toggleEventRsvpState,
} from "../../rust/gui-spike/ui/features/event-banner.js";

test("EVENT_ENTITY_LABELS_RU is frozen and maps known entity types", () => {
  assert.ok(Object.isFrozen(EVENT_ENTITY_LABELS_RU));
  assert.equal(EVENT_ENTITY_LABELS_RU[1], "Трибуна");
  assert.equal(EVENT_ENTITY_LABELS_RU[2], "Голосовой канал");
  assert.equal(EVENT_ENTITY_LABELS_RU[3], "Внешнее место");
});

test("EVENT_STATUS_LABELS_RU is frozen and maps known statuses", () => {
  assert.ok(Object.isFrozen(EVENT_STATUS_LABELS_RU));
  assert.equal(EVENT_STATUS_LABELS_RU[1], "Запланировано");
  assert.equal(EVENT_STATUS_LABELS_RU[2], "Идёт сейчас");
  assert.equal(EVENT_STATUS_LABELS_RU[3], "Завершено");
  assert.equal(EVENT_STATUS_LABELS_RU[4], "Отменено");
});

test("formatEventSummaryRu builds full summary for an active event", () => {
  const summary = formatEventSummaryRu({
    name: "  Открытый микрофон  ",
    entityType: 2,
    status: 2,
    userCount: 42,
    location: "Голосовой канал",
  });
  assert.deepEqual(summary, {
    title: "Открытый микрофон",
    entityLabel: "Голосовой канал",
    statusLabel: "Идёт сейчас",
    interestedLabel: "Интересуются: 42",
    isActive: true,
  });
});

test("formatEventSummaryRu falls back for empty input", () => {
  const summary = formatEventSummaryRu({});
  assert.equal(summary.title, "Событие без названия");
  assert.equal(summary.entityLabel, "Неизвестно");
  assert.equal(summary.statusLabel, "Запланировано");
  assert.equal(summary.interestedLabel, "Интересуются: 0");
  assert.equal(summary.isActive, false);
});

test("formatEventSummaryRu falls back for unknown codes", () => {
  const summary = formatEventSummaryRu({ name: "Тест", entityType: 99, status: 77 });
  assert.equal(summary.entityLabel, "Неизвестно");
  assert.equal(summary.statusLabel, "Запланировано");
  assert.equal(summary.isActive, false);
});

test("formatEventSummaryRu clamps negative and non-numeric userCount", () => {
  assert.equal(formatEventSummaryRu({ userCount: -5 }).interestedLabel, "Интересуются: 0");
  assert.equal(formatEventSummaryRu({ userCount: "abc" }).interestedLabel, "Интересуются: 0");
  assert.equal(formatEventSummaryRu({ userCount: 12.9 }).interestedLabel, "Интересуются: 12");
});

test("formatEventSummaryRu is not active for non-running statuses", () => {
  assert.equal(formatEventSummaryRu({ status: 1 }).isActive, false);
  assert.equal(formatEventSummaryRu({ status: 3 }).isActive, false);
  assert.equal(formatEventSummaryRu({ status: 4 }).isActive, false);
});

test("toggleEventRsvpState joins when not interested", () => {
  assert.deepEqual(toggleEventRsvpState(10, false), { interested: true, userCount: 11 });
});

test("toggleEventRsvpState leaves when already interested", () => {
  assert.deepEqual(toggleEventRsvpState(10, true), { interested: false, userCount: 9 });
});

test("toggleEventRsvpState clamps at zero", () => {
  assert.deepEqual(toggleEventRsvpState(0, true), { interested: false, userCount: 0 });
});

test("toggleEventRsvpState coerces garbage input", () => {
  assert.deepEqual(toggleEventRsvpState("7", 0), { interested: true, userCount: 8 });
  assert.deepEqual(toggleEventRsvpState(undefined, undefined), { interested: true, userCount: 1 });
});
