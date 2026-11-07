// @ts-check
/**
 * T-190 (UI forum tags filter): контракт чистого модуля фильтрации и
 * сортировки тредов форума.
 *
 * Проверяет: точные значения режимов сортировки, приоритет закреплённых
 * тредов, режимы совпадения тегов `any`/`all`, регистронезависимый поиск
 * по заголовку, порядок сортировки с тай-брейкером по id, подсчёт тредов
 * по тегам и отсутствие мутации входного массива.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  FORUM_SORT_MODES,
  filterAndSortForumThreads,
  countThreadsByTag,
} from "../../rust/gui-spike/ui/features/forum-filter.js";

/**
 * @param {string} id
 * @param {string[]} tagIds
 * @param {{ pinned?: boolean, lastActivityMs?: number, createdAtMs?: number, title?: string }} [extra]
 */
function thread(id, tagIds, extra = {}) {
  return {
    id,
    title: extra.title ?? `Thread ${id}`,
    tagIds,
    pinned: extra.pinned ?? false,
    lastActivityMs: extra.lastActivityMs,
    createdAtMs: extra.createdAtMs,
  };
}

/** @param {Array<{ id: string }>} list */
function ids(list) {
  return list.map((t) => t.id);
}

describe("FORUM_SORT_MODES", () => {
  it("содержит ровно два режима с точными значениями", () => {
    assert.deepEqual(FORUM_SORT_MODES, {
      LATEST_ACTIVITY: "latest_activity",
      CREATION_DATE: "creation_date",
    });
  });

  it("заморожен", () => {
    assert.equal(Object.isFrozen(FORUM_SORT_MODES), true);
    assert.throws(() => {
      // @ts-expect-error проверка заморозки в strict mode
      FORUM_SORT_MODES.LATEST_ACTIVITY = "changed";
    }, TypeError);
  });
});

describe("filterAndSortForumThreads — закреплённые треды", () => {
  it("закреплённые всегда идут первыми, порядок внутри групп сохраняется", () => {
    const input = [
      thread("a", [], { lastActivityMs: 500 }),
      thread("b", [], { lastActivityMs: 100, pinned: true }),
      thread("c", [], { lastActivityMs: 900 }),
      thread("d", [], { lastActivityMs: 300, pinned: true }),
    ];
    const out = filterAndSortForumThreads(input, {});
    assert.deepEqual(ids(out), ["d", "b", "c", "a"]);
  });

  it("не мутирует входной массив", () => {
    const input = [
      thread("x", [], { lastActivityMs: 1 }),
      thread("y", [], { lastActivityMs: 2 }),
    ];
    const snapshot = ids(input);
    filterAndSortForumThreads(input, {});
    assert.deepEqual(ids(input), snapshot);
  });
});

describe("filterAndSortForumThreads — фильтр по тегам", () => {
  const input = [
    thread("t1", ["one", "two"], { lastActivityMs: 1 }),
    thread("t2", ["two"], { lastActivityMs: 2 }),
    thread("t3", ["three"], { lastActivityMs: 3 }),
    thread("t4", [], { lastActivityMs: 4 }),
  ];

  it("без выбранных тегов возвращает все треды", () => {
    assert.deepEqual(ids(filterAndSortForumThreads(input, { selectedTagIds: [] })), [
      "t4",
      "t3",
      "t2",
      "t1",
    ]);
  });

  it("режим any (по умолчанию) оставляет треды хотя бы с одним тегом", () => {
    const out = filterAndSortForumThreads(input, { selectedTagIds: ["one", "three"] });
    assert.deepEqual(ids(out), ["t3", "t1"]);
  });

  it("явный режим any совпадает с режимом по умолчанию", () => {
    const a = filterAndSortForumThreads(input, { selectedTagIds: ["two"], matchMode: "any" });
    assert.deepEqual(ids(a), ["t2", "t1"]);
  });

  it("режим all оставляет только треды со всеми выбранными тегами", () => {
    const out = filterAndSortForumThreads(input, {
      selectedTagIds: ["one", "two"],
      matchMode: "all",
    });
    assert.deepEqual(ids(out), ["t1"]);
  });

  it("режим all без полного совпадения даёт пустой результат", () => {
    const out = filterAndSortForumThreads(input, {
      selectedTagIds: ["two", "three"],
      matchMode: "all",
    });
    assert.deepEqual(ids(out), []);
  });
});

describe("filterAndSortForumThreads — поиск по заголовку", () => {
  const input = [
    thread("s1", [], { title: "Release Notes v2", lastActivityMs: 1 }),
    thread("s2", [], { title: "release planning", lastActivityMs: 2 }),
    thread("s3", [], { title: "Random chat", lastActivityMs: 3 }),
  ];

  it("регистронезависимый поиск подстрокой", () => {
    assert.deepEqual(ids(filterAndSortForumThreads(input, { query: "RELEASE" })), ["s2", "s1"]);
  });

  it("query из пробелов не фильтрует", () => {
    assert.deepEqual(ids(filterAndSortForumThreads(input, { query: "   " })), ["s3", "s2", "s1"]);
  });

  it("поиск и теги применяются вместе", () => {
    const mixed = [
      { id: "m1", title: "Alpha release", tagIds: ["news"], lastActivityMs: 1 },
      { id: "m2", title: "Alpha chat", tagIds: ["off"], lastActivityMs: 2 },
    ];
    const out = filterAndSortForumThreads(mixed, { query: "alpha", selectedTagIds: ["news"] });
    assert.deepEqual(ids(out), ["m1"]);
  });
});

describe("filterAndSortForumThreads — сортировка", () => {
  it("latest_activity по умолчанию: убывание lastActivityMs", () => {
    const input = [
      thread("p", [], { lastActivityMs: 10 }),
      thread("q", [], { lastActivityMs: 30 }),
      thread("r", [], { lastActivityMs: 20 }),
    ];
    assert.deepEqual(ids(filterAndSortForumThreads(input, {})), ["q", "r", "p"]);
  });

  it("latest_activity падает на createdAtMs, если lastActivityMs отсутствует", () => {
    const input = [
      thread("p", [], { lastActivityMs: 10, createdAtMs: 999 }),
      thread("q", [], { createdAtMs: 50 }),
      thread("r", []),
    ];
    assert.deepEqual(ids(filterAndSortForumThreads(input, { sortBy: "latest_activity" })), [
      "q",
      "p",
      "r",
    ]);
  });

  it("creation_date: убывание createdAtMs, отсутствующие в конце", () => {
    const input = [
      thread("p", [], { createdAtMs: 5, lastActivityMs: 100 }),
      thread("q", [], { createdAtMs: 40 }),
      thread("r", []),
    ];
    assert.deepEqual(ids(filterAndSortForumThreads(input, { sortBy: "creation_date" })), [
      "q",
      "p",
      "r",
    ]);
  });

  it("тай-брейкер: String(b.id).localeCompare(String(a.id))", () => {
    const input = [
      thread("b", [], { lastActivityMs: 7 }),
      thread("c", [], { lastActivityMs: 7 }),
      thread("a", [], { lastActivityMs: 7 }),
    ];
    assert.deepEqual(ids(filterAndSortForumThreads(input, {})), ["c", "b", "a"]);
  });

  it("тай-брейкер не переставляет закреплённые перед незакреплёнными", () => {
    const input = [
      thread("a", [], { lastActivityMs: 7 }),
      thread("b", [], { lastActivityMs: 7, pinned: true }),
      thread("c", [], { lastActivityMs: 7, pinned: true }),
    ];
    assert.deepEqual(ids(filterAndSortForumThreads(input, {})), ["c", "b", "a"]);
  });
});

describe("countThreadsByTag", () => {
  it("считает треды по тегам с дедупликацией внутри треда", () => {
    const input = [
      thread("1", ["alpha", "beta", "alpha"]),
      thread("2", ["beta"]),
      thread("3", ["gamma", "beta"]),
      thread("4", []),
    ];
    assert.deepEqual(countThreadsByTag(input), { alpha: 1, beta: 3, gamma: 1 });
  });

  it("возвращает обычный объект и пустой результат для пустого входа", () => {
    const out = countThreadsByTag([]);
    assert.equal(Object.getPrototypeOf(out), Object.prototype);
    assert.deepEqual(out, {});
  });

  it("не мутирует входной массив", () => {
    const input = [thread("1", ["alpha"])];
    countThreadsByTag(input);
    assert.deepEqual(input[0]?.tagIds, ["alpha"]);
  });
});
