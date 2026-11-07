// @ts-check
/**
 * task-206: входящий индикатор набора — TTL, дедупликация и русская строка.
 * Оракул — ТЗ карточки task-206 (TYPING_TTL_MS, pruneExpiredTypingUsers,
 * formatTypingIndicatorRu), независимо от реализации.
 */
import assert from "node:assert/strict";
import test from "node:test";

import {
  TYPING_TTL_MS,
  formatTypingIndicatorRu,
  pruneExpiredTypingUsers,
} from "../../rust/gui-spike/ui/features/typing-indicator.js";

test("TYPING_TTL_MS = 10000", () => {
  assert.equal(TYPING_TTL_MS, 10000);
});

test("pruneExpiredTypingUsers: окно 0 <= age < TTL, границы", () => {
  const now = 1_000_000;
  /** @type {Array<{ userId: string, displayName: string, startedAtMs: number }>} */
  const entries = [
    { userId: "fresh", displayName: "Свежий", startedAtMs: now }, // age 0 -> keep
    { userId: "edge-in", displayName: "Граница", startedAtMs: now - (TYPING_TTL_MS - 1) }, // -> keep
    { userId: "edge-out", displayName: "Ровно TTL", startedAtMs: now - TYPING_TTL_MS }, // -> drop
    { userId: "stale", displayName: "Старый", startedAtMs: now - 60_000 }, // -> drop
    { userId: "future", displayName: "Из будущего", startedAtMs: now + 5 }, // age < 0 -> drop
  ];

  const kept = pruneExpiredTypingUsers(entries, now);
  assert.deepEqual(kept.map((e) => e.userId).sort(), ["edge-in", "fresh"]);
  assert.ok(kept.every((e) => typeof e.displayName === "string"));
});

test("pruneExpiredTypingUsers: дедупликация по userId, остаётся latest startedAtMs", () => {
  const now = 500_000;
  const entries = [
    { userId: "u1", displayName: "Старое имя", startedAtMs: now - 9_000 },
    { userId: "u2", displayName: "Второй", startedAtMs: now - 4_000 },
    { userId: "u1", displayName: "Новое имя", startedAtMs: now - 1_000 },
    { userId: "u3", displayName: "Истёкший", startedAtMs: now - TYPING_TTL_MS },
  ];

  const kept = pruneExpiredTypingUsers(entries, now);
  assert.equal(kept.length, 2);
  assert.deepEqual(kept.map((e) => e.userId).sort(), ["u1", "u2"]);
  const u1 = kept.find((e) => e.userId === "u1");
  assert.ok(u1);
  assert.equal(u1.displayName, "Новое имя");
  assert.equal(u1.startedAtMs, now - 1_000);
});

test("pruneExpiredTypingUsers: по умолчанию nowMs = Date.now()", () => {
  const nowMs = Date.now();
  const kept = pruneExpiredTypingUsers([
    { userId: "live", displayName: "Live", startedAtMs: nowMs },
    { userId: "dead", displayName: "Dead", startedAtMs: nowMs - 20_000 },
  ]);
  assert.deepEqual(kept.map((e) => e.userId), ["live"]);
});

test("pruneExpiredTypingUsers: пустой вход и мусор не бросают", () => {
  assert.deepEqual(pruneExpiredTypingUsers([], 1), []);
  assert.deepEqual(pruneExpiredTypingUsers([{ userId: "", displayName: "", startedAtMs: 0 }], 1), []);
});

test("formatTypingIndicatorRu: 0 имён -> пустая строка", () => {
  assert.equal(formatTypingIndicatorRu([]), "");
  assert.equal(formatTypingIndicatorRu(["", "   ", "\t\n"]), "");
});

test("formatTypingIndicatorRu: 1 имя -> 'печатает...'", () => {
  assert.equal(formatTypingIndicatorRu(["Аня"]), "Аня печатает...");
  assert.equal(formatTypingIndicatorRu(["Аня", "", "  "]), "Аня печатает...");
});

test("formatTypingIndicatorRu: 2 имени -> 'и ... печатают...'", () => {
  assert.equal(formatTypingIndicatorRu(["Аня", "Боб"]), "Аня и Боб печатают...");
});

test("formatTypingIndicatorRu: 3 имени -> перечисление через запятую и 'и'", () => {
  assert.equal(
    formatTypingIndicatorRu(["Аня", "Боб", "Вика"]),
    "Аня, Боб и Вика печатают...",
  );
});

test("formatTypingIndicatorRu: 4+ имён -> 'Несколько человек печатают...'", () => {
  assert.equal(
    formatTypingIndicatorRu(["Аня", "Боб", "Вика", "Гриша"]),
    "Несколько человек печатают...",
  );
  assert.equal(
    formatTypingIndicatorRu(["Аня", "Боб", "Вика", "Гриша", "Дина"]),
    "Несколько человек печатают...",
  );
});

test("formatTypingIndicatorRu: имена обрезаются, пустые не считаются при подсчёте", () => {
  assert.equal(
    formatTypingIndicatorRu([" Аня ", "  ", " Боб "]),
    "Аня и Боб печатают...",
  );
  assert.equal(
    formatTypingIndicatorRu(["Аня", "Боб", "Вика", "", "  "]),
    "Аня, Боб и Вика печатают...",
  );
});
