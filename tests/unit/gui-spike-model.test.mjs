// @ts-check
// RED (RW-37 / W37b, ADR-1 follow-up): оконная/постраничная модель данных GUI-спайка.
// Контракт заморожен в waves/RW-37/TZ.md §4:
//   new PageStore({ totalRows, pageSize, maxPages, fetchPage })
//   getRow(index) -> row | undefined
//   ensureRange(first, last) -> Promise<void>      // границы включительно
//   stats() -> { pages, loadedPages, loadedRows, maxPages, pageSize, totalRows }
//
// Инварианты: loadedRows <= pageSize*maxPages; эвикция по LRU, текущий диапазон
// не эвиктится; getRow вне [0,totalRows) -> undefined без исключения;
// невалидные pageSize/maxPages/totalRows -> TypeError в конструкторе;
// ensureRange идемпотентен и не делает двойных загрузок одной страницы.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  CHAT_HEIGHT_PERIOD,
  HeightIndex,
  PageStore,
  ROW_HEIGHTS,
  createChatHeights,
  createMemberHeights,
  memberHeight,
  messageGroup,
  messageHeight,
  messageLines,
} from "../../rust/gui-spike/ui/model.js";

/** @typedef {{ id: number }} Row */

// Для кейсов с заведомо невалидными аргументами: проверяем рантайм-контракт, не типы.
const AnyPageStore = /** @type {any} */ (PageStore);

/**
 * Детерминированная страница: строки нумеруются сквозным индексом.
 * @param {number} pageIndex
 * @param {number} totalRows
 * @param {number} pageSize
 * @returns {Row[]}
 */
function makePage(pageIndex, totalRows, pageSize) {
  const start = pageIndex * pageSize;
  const end = Math.min(totalRows, start + pageSize);
  return Array.from({ length: Math.max(0, end - start) }, (_, offset) => ({ id: start + offset }));
}

/**
 * Стор с журналом вызовов fetchPage, чтобы проверять отсутствие двойных загрузок.
 * @param {{ totalRows?: number, pageSize?: number, maxPages?: number, delayMs?: number }} [options]
 */
function makeStore(options = {}) {
  const totalRows = options.totalRows ?? 250;
  const pageSize = options.pageSize ?? 100;
  const maxPages = options.maxPages ?? 3;
  const delayMs = options.delayMs ?? 0;
  /** @type {number[]} */
  const fetched = [];
  const store = new PageStore({
    totalRows,
    pageSize,
    maxPages,
    fetchPage: async (pageIndex) => {
      fetched.push(pageIndex);
      if (delayMs > 0) await new Promise((resolve) => setTimeout(resolve, delayMs));
      return makePage(pageIndex, totalRows, pageSize);
    },
  });
  return { store, fetched, totalRows, pageSize, maxPages };
}

describe("PageStore: конструктор", () => {
  it("сохраняет параметры и считает общее число страниц (пустой стор)", () => {
    const { store } = makeStore({ totalRows: 250, pageSize: 100, maxPages: 3 });
    // pages — всего страниц (ceil(250/100)=3), они ещё не загружены.
    assert.deepEqual(store.stats(), {
      pages: 3,
      loadedPages: 0,
      loadedRows: 0,
      maxPages: 3,
      pageSize: 100,
      totalRows: 250,
    });
  });

  it("отвергает невалидный totalRows (только целое > 0)", () => {
    for (const totalRows of [0, -1, 1.5, Number.NaN, Number.POSITIVE_INFINITY, "250", null, undefined]) {
      assert.throws(
        () => new AnyPageStore({ totalRows, pageSize: 100, maxPages: 3, fetchPage: () => [] }),
        (/** @type {any} */ error) => error instanceof TypeError && /totalRows/.test(error.message),
        `totalRows=${String(totalRows)} должен отвергаться`,
      );
    }
  });

  it("отвергает невалидные pageSize и maxPages", () => {
    for (const pageSize of [0, -100, 2.5, Number.NaN, "100"]) {
      assert.throws(
        () => new AnyPageStore({ totalRows: 250, pageSize, maxPages: 3, fetchPage: () => [] }),
        (/** @type {any} */ error) => error instanceof TypeError && /pageSize/.test(error.message),
        `pageSize=${String(pageSize)} должен отвергаться`,
      );
    }
    for (const maxPages of [0, -3, 1.5, Number.NaN, "3"]) {
      assert.throws(
        () => new AnyPageStore({ totalRows: 250, pageSize: 100, maxPages, fetchPage: () => [] }),
        (/** @type {any} */ error) => error instanceof TypeError && /maxPages/.test(error.message),
        `maxPages=${String(maxPages)} должен отвергаться`,
      );
    }
  });

  it("требует объект опций и функцию fetchPage", () => {
    assert.throws(() => new AnyPageStore(), TypeError);
    assert.throws(() => new AnyPageStore(null), TypeError);
    assert.throws(() => new AnyPageStore({ totalRows: 10, pageSize: 5, maxPages: 2 }), TypeError);
    assert.throws(
      () => new AnyPageStore({ totalRows: 10, pageSize: 5, maxPages: 2, fetchPage: "nope" }),
      TypeError,
    );
  });

  it("допускает одну строку в единственной странице", async () => {
    const { store } = makeStore({ totalRows: 1, pageSize: 1000, maxPages: 1 });
    assert.equal(store.stats().pages, 1);
    await store.ensureRange(0, 0);
    assert.deepEqual(store.getRow(0), { id: 0 });
    assert.equal(store.getRow(1), undefined);
    assert.equal(store.stats().loadedRows, 1);
  });
});

describe("PageStore: пустой стор и getRow", () => {
  it("до загрузки getRow → undefined, fetchPage не вызывался", () => {
    const { store, fetched } = makeStore();
    assert.equal(store.getRow(0), undefined);
    assert.equal(store.getRow(249), undefined);
    assert.deepEqual(fetched, []);
    assert.equal(store.stats().loadedPages, 0);
  });

  it("getRow вне [0,totalRows) → undefined без исключения", async () => {
    const { store } = makeStore({ totalRows: 250, pageSize: 100, maxPages: 3 });
    await store.ensureRange(0, 249); // все страницы загружены — проверка именно границ
    for (const index of [-1, -100, 250, 251, 1e9, 1.5, 100.5, 249.5, Number.NaN, Number.POSITIVE_INFINITY, "0", null, undefined]) {
      assert.equal(store.getRow(/** @type {any} */ (index)), undefined, `index=${String(index)}`);
    }
    assert.deepEqual(store.getRow(249), { id: 249 });
  });

  it("первая страница: строки 0 и 99 доступны, 100 — ещё нет", async () => {
    const { store, fetched } = makeStore();
    await store.ensureRange(0, 99);
    assert.deepEqual(store.getRow(0), { id: 0 });
    assert.deepEqual(store.getRow(99), { id: 99 });
    assert.equal(store.getRow(100), undefined);
    assert.deepEqual(fetched, [0]);
  });

  it("последняя страница неполная: 250 строк → 50 в третьей странице", async () => {
    const { store, fetched } = makeStore();
    await store.ensureRange(200, 249);
    assert.deepEqual(store.getRow(200), { id: 200 });
    assert.deepEqual(store.getRow(249), { id: 249 });
    assert.equal(store.getRow(250), undefined);
    assert.deepEqual(fetched, [2]);
    assert.equal(store.stats().loadedRows, 50);
  });
});

describe("PageStore: ensureRange", () => {
  it("кламплит границы и ничего не грузит вне данных", async () => {
    const first = makeStore();
    await first.store.ensureRange(-50, 10);
    assert.deepEqual(first.fetched, [0]);
    assert.deepEqual(first.store.getRow(0), { id: 0 });

    const last = makeStore();
    await last.store.ensureRange(240, 10_000);
    assert.deepEqual(last.fetched, [2]);
    assert.deepEqual(last.store.getRow(249), { id: 249 });

    const outside = makeStore();
    await outside.store.ensureRange(1_000, 2_000);
    assert.deepEqual(outside.fetched, [], "диапазон целиком вне данных не грузит страницы");
  });

  it("идемпотентен: повторные вызовы не перезагружают страницы", async () => {
    const { store, fetched } = makeStore();
    await store.ensureRange(0, 99);
    await store.ensureRange(0, 99);
    await store.ensureRange(0, 99);
    assert.deepEqual(fetched, [0]);
    assert.equal(store.stats().loadedPages, 1);
  });

  it("конкурентные вызовы: одна загрузка на страницу", async () => {
    const { store, fetched } = makeStore({ delayMs: 10 });
    await Promise.all([
      store.ensureRange(0, 149),
      store.ensureRange(50, 199),
      store.ensureRange(0, 149),
    ]);
    assert.deepEqual([...fetched].sort((a, b) => a - b), [0, 1]);
    assert.equal(new Set(fetched).size, fetched.length, "двойных загрузок нет");
    assert.equal(store.stats().loadedPages, 2);
    assert.deepEqual(store.getRow(149), { id: 149 });
  });

  it("ошибка fetchPage пробрасывается и не кэшируется навсегда", async () => {
    let attempt = 0;
    const store = new PageStore({
      totalRows: 100,
      pageSize: 50,
      maxPages: 2,
      fetchPage: async (pageIndex) => {
        attempt += 1;
        if (attempt === 1) throw new Error("страница недоступна");
        return makePage(pageIndex, 100, 50);
      },
    });
    await assert.rejects(store.ensureRange(0, 49), /страница недоступна/);
    await store.ensureRange(0, 49);
    assert.deepEqual(store.getRow(0), { id: 0 });
    assert.equal(attempt, 2);
  });

  it("не-массив из fetchPage → TypeError", async () => {
    const store = new PageStore({
      totalRows: 10,
      pageSize: 5,
      maxPages: 2,
      fetchPage: () => /** @type {any} */ (null),
    });
    await assert.rejects(store.ensureRange(0, 4), TypeError);
  });

  it("диапазон шире maxPages → RangeError, ничего не грузит", async () => {
    const { store, fetched } = makeStore({ totalRows: 1000, pageSize: 100, maxPages: 2 });
    await assert.rejects(store.ensureRange(0, 250), /maxPages/);
    assert.deepEqual(fetched, []);
    assert.equal(store.stats().loadedPages, 0);
  });

  it("короткая не-последняя страница → RangeError (иначе дыра в окне навсегда)", async () => {
    // R1 (RW-37 rework): getRow для недогруженной строки отдаёт undefined, что для
    // виртуализации значит «страница не загружена». Повторной загрузки не будет,
    // поэтому неполная не-последняя страница должна падать громко, а не молча.
    const store = new PageStore({
      totalRows: 250,
      pageSize: 100,
      maxPages: 3,
      fetchPage: (pageIndex) =>
        pageIndex === 0 ? makePage(pageIndex, 250, 100).slice(0, 99) : makePage(pageIndex, 250, 100),
    });
    await assert.rejects(store.ensureRange(0, 99), (/** @type {any} */ error) => {
      assert.ok(error instanceof RangeError, `ожидался RangeError, получен ${String(error)}`);
      assert.match(error.message, /fetchPage\(0\)/);
      assert.match(error.message, /100/, "в сообщении должно быть ожидаемое число строк");
      assert.match(error.message, /99/, "в сообщении должен быть фактический размер");
      return true;
    });
    assert.equal(store.stats().loadedPages, 0, "битая страница не попадает в кэш");
  });

  it("последняя страница вернула 0 строк при capacity>0 → RangeError (симметрично R1)", async () => {
    // P2-2: пустая последняя страница принималась навсегда → вечная дыра в окне.
    const store = new PageStore({
      totalRows: 250,
      pageSize: 100,
      maxPages: 3,
      fetchPage: (pageIndex) => (pageIndex === 2 ? [] : makePage(pageIndex, 250, 100)),
    });
    await assert.rejects(store.ensureRange(200, 249), (/** @type {any} */ error) => {
      assert.ok(error instanceof RangeError, `ожидался RangeError, получен ${String(error)}`);
      assert.match(error.message, /fetchPage\(2\)/);
      assert.match(error.message, /ожидалось 50/, "в сообщении должна быть вместимость страницы");
      assert.match(error.message, /получено 0/, "в сообщении должен быть фактический размер");
      return true;
    });
    assert.equal(store.stats().loadedPages, 0, "пустая страница не попадает в кэш");
  });

  it("последняя (неполная) страница остаётся допустимой", async () => {
    const store = new PageStore({
      totalRows: 250,
      pageSize: 100,
      maxPages: 3,
      fetchPage: (pageIndex) => makePage(pageIndex, 250, 100),
    });
    await store.ensureRange(200, 249);
    assert.equal(store.stats().loadedRows, 50);
    assert.deepEqual(store.getRow(249), { id: 249 });

    const oneRow = new PageStore({
      totalRows: 101,
      pageSize: 100,
      maxPages: 2,
      fetchPage: (pageIndex) => makePage(pageIndex, 101, 100),
    });
    await oneRow.ensureRange(100, 100);
    assert.equal(oneRow.stats().loadedRows, 1);
    assert.deepEqual(oneRow.getRow(100), { id: 100 });
  });

  it("перегрузка страницы (capacity+1) → RangeError для любой страницы (m09)", async () => {
    // m09: разрешить перегрузку страницы (`rows.length < capacity` вместо `!==`) нельзя —
    // лишние строки ломают индексацию окна и не проверяются остальными кейсами.
    let attempts = 0;
    /** @type {PageStore} */
    const nonLast = new PageStore({
      totalRows: 250,
      pageSize: 100,
      maxPages: 3,
      fetchPage: (pageIndex) => {
        attempts += 1;
        const page = makePage(pageIndex, 250, 100);
        // Первый вызов отдаёт лишнюю строку, повторный — корректную (проверка retry).
        return attempts === 1 ? [...page, { id: -1 }] : page;
      },
    });
    await assert.rejects(nonLast.ensureRange(0, 99), (/** @type {any} */ error) => {
      assert.ok(error instanceof RangeError, `ожидался RangeError, получен ${String(error)}`);
      assert.match(error.message, /fetchPage\(0\)/);
      assert.match(error.message, /ожидалось 100/, "capacity не-последней страницы = pageSize");
      assert.match(error.message, /получено 101/);
      return true;
    });
    assert.equal(nonLast.stats().loadedPages, 0, "перегруженная страница не в кэше");
    assert.equal(nonLast.stats().loadedRows, 0);
    await nonLast.ensureRange(0, 99); // повтор после ошибки — успешен
    assert.equal(nonLast.stats().loadedPages, 1);
    assert.deepEqual(nonLast.getRow(99), { id: 99 });

    // Последняя страница: capacity = остаток totalRows (50), 51 строка — перегрузка.
    const lastPage = new PageStore({
      totalRows: 250,
      pageSize: 100,
      maxPages: 3,
      fetchPage: (pageIndex) => {
        const page = makePage(pageIndex, 250, 100);
        return pageIndex === 2 ? [...page, { id: -1 }] : page;
      },
    });
    await assert.rejects(lastPage.ensureRange(200, 249), (/** @type {any} */ error) => {
      assert.ok(error instanceof RangeError, `ожидался RangeError, получен ${String(error)}`);
      assert.match(error.message, /fetchPage\(2\)/);
      assert.match(error.message, /ожидалось 50/, "capacity последней страницы = остаток");
      assert.match(error.message, /получено 51/);
      return true;
    });
    assert.equal(lastPage.stats().loadedPages, 0);
  });

  it("невалидные аргументы → TypeError, пустой диапазон — no-op", async () => {
    const { store, fetched } = makeStore();
    await assert.rejects(store.ensureRange(1.5, 10), TypeError);
    await assert.rejects(store.ensureRange(0, Number.NaN), TypeError);
    await store.ensureRange(50, 10); // last < first — пустой диапазон
    assert.deepEqual(fetched, []);
  });
});

describe("PageStore: LRU-эвикция и бюджет", () => {
  it("вытесняет страницу, к которой дольше не обращались", async () => {
    const { store } = makeStore({ totalRows: 1000, pageSize: 10, maxPages: 2 });
    await store.ensureRange(0, 9);
    await store.ensureRange(10, 19);
    assert.deepEqual(store.getRow(5), { id: 5 }); // страница 0 — самая свежая по доступу
    await store.ensureRange(20, 29); // нужна третья страница при бюджете 2
    assert.equal(store.stats().loadedPages, 2);
    assert.equal(store.getRow(15), undefined, "страница 1 вытеснена как самая старая");
    assert.deepEqual(store.getRow(5), { id: 5 });
    assert.deepEqual(store.getRow(25), { id: 25 });
  });

  it("текущий диапазон не эвиктит сам себя (ровно на бюджете)", async () => {
    const { store } = makeStore({ totalRows: 1000, pageSize: 10, maxPages: 2 });
    await store.ensureRange(0, 19); // две страницы — ровно maxPages
    assert.equal(store.stats().loadedPages, 2);
    assert.deepEqual(store.getRow(0), { id: 0 });
    assert.deepEqual(store.getRow(19), { id: 19 });
    await store.ensureRange(0, 19);
    assert.deepEqual(store.getRow(0), { id: 0 });
    assert.deepEqual(store.getRow(19), { id: 19 });
  });

  it("конкурентный ensureRange не эвиктит запиненную страницу (m07)", async () => {
    // Регрессия, которую статические LRU-тесты не ловят: при конкурентных ensureRange
    // запиненная страница может оказаться самой старой по LRU, и эвикция без учёта
    // `pinned` выбрасывает видимое окно (дыра до следующего ensureRange).
    const store = new PageStore({
      totalRows: 1000,
      pageSize: 10,
      maxPages: 2,
      fetchPage: async (pageIndex) => {
        // page 1 — медленно, page 2 — быстро: порядок резолва детерминирован.
        await new Promise((resolve) => setTimeout(resolve, pageIndex === 1 ? 60 : 10));
        return makePage(pageIndex, 1000, 10);
      },
    });
    await store.ensureRange(0, 9); // page 0 в кэше и самая старая по доступу
    const rangeA = store.ensureRange(0, 19); // пин {0,1}, page 1 резолвится последней
    const rangeB = store.ensureRange(20, 29); // пин {2}, page 2 резолвится раньше
    await Promise.all([rangeA, rangeB]);
    assert.equal(store.stats().loadedPages, 2, "бюджет maxPages соблюдён");
    assert.deepEqual(store.getRow(0), { id: 0 }, "page 0 запинена диапазоном A и не вытеснена");
    assert.deepEqual(store.getRow(19), { id: 19 }, "page 1 диапазона A на месте");
  });

  it("getRow обновляет LRU: прочитанная страница выживает", async () => {
    const { store } = makeStore({ totalRows: 1000, pageSize: 10, maxPages: 2 });
    await store.ensureRange(0, 9);
    await store.ensureRange(10, 19);
    assert.deepEqual(store.getRow(0), { id: 0 }); // страница 0 → свежая
    await store.ensureRange(20, 29);
    assert.equal(store.getRow(10), undefined, "вытеснена страница 1");
    assert.deepEqual(store.getRow(0), { id: 0 });
    assert.deepEqual(store.getRow(20), { id: 20 });
  });

  it("loadedRows ≤ pageSize*maxPages при обходе всего стора", async () => {
    const { store } = makeStore({ totalRows: 250, pageSize: 24, maxPages: 3 });
    for (let page = 0; page < 11; page++) {
      const first = page * 24;
      await store.ensureRange(first, Math.min(249, first + 23));
      const stats = store.stats();
      assert.ok(stats.loadedRows <= stats.pageSize * stats.maxPages, `loadedRows=${stats.loadedRows}`);
      assert.ok(stats.loadedPages <= stats.maxPages, `loadedPages=${stats.loadedPages}`);
    }
    assert.equal(store.stats().pages, 11);
  });

  it("stats после загрузки двух страниц", async () => {
    const { store } = makeStore({ totalRows: 250, pageSize: 100, maxPages: 3 });
    await store.ensureRange(0, 199);
    assert.deepEqual(store.stats(), {
      pages: 3,
      loadedPages: 2,
      loadedRows: 200,
      maxPages: 3,
      pageSize: 100,
      totalRows: 250,
    });
  });
});

/* =============================================================================
 * DP-02 / D-04: переменная высота строк — независимый оракул RW-37.
 *
 * Числа не дублируются вручную: они читаются из `waves/RW-37/RE/layout.json`
 * (артефакт RE, не код спайка). Правила переносов демо-данных (группа из 4,
 * перенос у i % 17 === 9 и ещё один у i % 23 === 9) — из DOM-CONTRACT §7.1,
 * поэтому ожидание `messageHeight` строится здесь, а не берётся из model.js.
 * ========================================================================== */
const layout = JSON.parse(
  readFileSync(fileURLToPath(new URL("../../waves/RW-37/RE/layout.json", import.meta.url)), "utf8"),
);

/** @type {{ continuation: number, groupStart: number, lineExtra: number, member: number }} */
const RE = {
  continuation: layout.row_heights.message_cozy,
  groupStart: layout.row_heights_detail.message_cozy_group_start,
  lineExtra: layout.row_heights_detail.message_extra_line,
  member: layout.row_heights.member,
};

const GROUP_SIZE = 4;
const WRAP_1 = 17;
const WRAP_2 = 23;
const WRAP_AT = 9;

/** @param {number} index @returns {number} */
function oracleLines(index) {
  return 1 + (index % WRAP_1 === WRAP_AT ? 1 : 0) + (index % WRAP_2 === WRAP_AT ? 1 : 0);
}

/** @param {number} index @returns {"start" | "continuation"} */
function oracleGroup(index) {
  return index % GROUP_SIZE === 0 ? "start" : "continuation";
}

/** @param {number} index @returns {number} высота строки чата по RE-оракулу */
function oracleChatHeight(index) {
  const base = oracleGroup(index) === "start" ? RE.groupStart : RE.continuation;
  return base + (oracleLines(index) - 1) * RE.lineExtra;
}

describe("D-04: высоты строк — RE-оракул layout.json", () => {
  it("ROW_HEIGHTS совпадает с RE: 26 / 48 / +22 / member 42", () => {
    assert.equal(RE.continuation, 26, "оракул message_cozy изменился — обновите карточку");
    assert.equal(RE.groupStart, 48, "оракул message_cozy_group_start изменился");
    assert.equal(RE.lineExtra, 22, "оракул message_extra_line изменился");
    assert.equal(RE.member, 42, "оракул row_heights.member изменился");
    assert.equal(layout.line_height, RE.lineExtra, "line_height и line_extra разошлись в оракуле");
    assert.deepEqual(
      { ...ROW_HEIGHTS },
      {
        continuation: RE.continuation,
        groupStart: RE.groupStart,
        lineExtra: RE.lineExtra,
        member: RE.member,
      },
      "ROW_HEIGHTS не равен RE; «44/40» — приближение RW-38, а не оракул",
    );
  });

  it("messageHeight = 26/48 + 22×перенос на всём периоде (константы 44 нет)", () => {
    assert.equal(CHAT_HEIGHT_PERIOD, GROUP_SIZE * WRAP_1 * WRAP_2, "период высот не кратен правилам");
    const seen = new Set();
    for (let index = 0; index < CHAT_HEIGHT_PERIOD * 2; index++) {
      const expected = oracleChatHeight(index);
      assert.equal(messageHeight(index), expected, `высота строки ${index} не по оракулу`);
      assert.equal(messageGroup(index), oracleGroup(index), `группа строки ${index} не по оракулу`);
      assert.equal(messageLines(index), oracleLines(index), `переносы строки ${index} не по оракулу`);
      seen.add(messageHeight(index));
    }
    // Допустимые высоты периода: 26, 48, 70, 92 — «44» и «40» сюда не входят.
    assert.deepEqual(
      [...seen].sort((a, b) => a - b),
      [26, 48, 70, 92],
      "набор высот периода не 26/48/70/92 (перенос +22)",
    );
  });

  it("литеральные пары «индекс → высота» из оракула (44 их не проходит)", () => {
    /** @type {Array<[number, number]>} */
    const pairs = [
      [0, 48], // начало группы, 1 строка
      [1, 26], // продолжение, 1 строка
      [4, 48], // начало группы
      [9, 70], // оба переноса (17 и 23) — продолжение 26 + 2×22
      [12, 48], // начало группы
      [26, 48], // начало группы
      [32, 70], // начало группы + перенос (23)
      [391, 26], // 391 = lcm(17, 23), но перенос только у i ≡ 9 — здесь одна строка
      [400, 92], // начало группы (400 % 4 === 0) + оба переноса: 48 + 2×22
    ];
    for (const [index, height] of pairs) {
      assert.equal(oracleChatHeight(index), height, `оракул разошёлся для ${index}`);
      assert.equal(messageHeight(index), height, `messageHeight(${index}) ≠ ${height}`);
    }
  });

  it("HeightIndex: prefix-sum, обратный indexAt и totalHeight", () => {
    const heights = createChatHeights(12_345);
    let sum = 0;
    for (let index = 0; index < heights.totalRows; index++) {
      const height = heights.height(index);
      assert.equal(height, oracleChatHeight(index), `height(${index})`);
      assert.equal(heights.top(index), sum, `top(${index}) ≠ сумме высот [0,${index})`);
      sum += height;
      assert.equal(heights.top(index + 1), sum, `top(${index + 1}) ≠ top(${index}) + height(${index})`);
      assert.equal(heights.indexAt(sum - 1), index, `indexAt не вернул ${index} на своей последней строке`);
      assert.equal(heights.indexAt(heights.top(index)), index, `indexAt(top(${index})) ≠ ${index}`);
    }
    assert.equal(heights.totalHeight, sum, "totalHeight ≠ сумме высот");
    assert.equal(heights.top(heights.totalRows), sum, "top(totalRows) ≠ totalHeight");
  });

  it("HeightIndex не хранит смещения на все строки: prefix = period + 1", () => {
    const heights = createChatHeights(100_000);
    assert.equal(heights.period, CHAT_HEIGHT_PERIOD, "период высот чата изменился");
    assert.equal(heights.prefix.length, CHAT_HEIGHT_PERIOD + 1, "prefix-sum не периодичный (O(rows) память)");
    assert.ok(heights.prefix instanceof Float64Array, "prefix-sum должен быть Float64Array");
    // Стоимость top/indexAt не зависит от 100 000 строк — проверяет bench-тест.
  });

  it("HeightIndex отклоняет непериодичные высоты (видимая ошибка, а не тихая геометрия)", () => {
    assert.throws(
      () => new HeightIndex(100, (index) => (index === 5 ? 27 : 26), 4),
      /не периодичны/,
      "непериодичная высота не отвергнута",
    );
    assert.throws(() => new HeightIndex(100, () => 0, 4), RangeError, "высота 0 принята");
  });

  it("участники: 42 на строку, total = rows × 42, indexAt кратен 42", () => {
    assert.equal(memberHeight(), RE.member, "memberHeight() ≠ 42 (RE)");
    const heights = createMemberHeights(24_229);
    assert.equal(heights.period, 1, "member heights должны быть периодичны с периодом 1");
    assert.equal(heights.totalHeight, 24_229 * RE.member, "totalHeight участников ≠ rows×42");
    for (const index of [0, 1, 36, 1000, 24_228]) {
      assert.equal(heights.height(index), RE.member, `высота участника ${index} ≠ 42`);
      assert.equal(heights.top(index), index * RE.member, `top(${index}) ≠ index×42`);
    }
    assert.equal(heights.indexAt(42 * 100), 100, "indexAt участников не 42×index");
    assert.equal(heights.indexAt(42 * 100 + 41), 100, "последний пиксель строки участника не в ней");
  });
});
