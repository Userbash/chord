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

/* =============================================================================
 * M-08 (RW-40, G2 Frontend Lane C): безопасный markdown и упоминания сообщений.
 *
 * Контракт `rust/gui-spike/ui/features/markdown.js`:
 *   parseMessageMarkdown(text, context) -> Token[]          // без DOM
 *   renderMessageMarkdown(container, text, context)         // DOM через ownerDocument
 *
 * Оракул безопасности — не текущий код модуля, а правила Discord-клиента и
 * требования M-08: сырой HTML остаётся текстом, ссылками становятся только
 * http(s), опасные схемы (javascript:, data:, vbscript:, file:) не дают <a>,
 * внутри кода markdown и упоминания не парсятся, спойлер скрыт до активации.
 * DOM — шим `tests/support/gui-dom-harness.mjs`: документ создаётся напрямую,
 * без импорта `app.js`, поэтому утечки состояния приложения нет.
 * ========================================================================== */

import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  parseMessageMarkdown,
  renderMessageMarkdown,
} from "../../rust/gui-spike/ui/features/markdown.js";

/** @typedef {import("../support/gui-dom-harness.mjs").ShimElement} ShimElement */

/**
 * @typedef {{
 *   users?: Record<string, string>,
 *   channels?: Record<string, string>,
 *   roles?: Record<string, string>,
 * }} RenderContext
 */

const markdownDoc = new ShimDocument();

/**
 * Рендер одного сообщения в свежий контейнер.
 * @param {string} text
 * @param {RenderContext} [context]
 * @returns {ShimElement}
 */
function renderMarkdown(text, context) {
  const container = /** @type {any} */ (markdownDoc.createElement("div"));
  renderMessageMarkdown(container, text, context);
  return /** @type {ShimElement} */ (container);
}

/**
 * Ровно один элемент по селектору (падение, если их 0 или больше).
 * @param {ShimElement} root
 * @param {string} selector
 * @returns {ShimElement}
 */
function only(root, selector) {
  const found = root.querySelectorAll(selector);
  assert.equal(found.length, 1, `ожидался ровно один элемент «${selector}»`);
  const first = found[0];
  assert.ok(first, `элемент «${selector}» не найден`);
  return first;
}

describe("M-08: markdown и упоминания — безопасный рендер сообщений", () => {
  it("parseMessageMarkdown: форматы становятся токенами, DOM не требуется", () => {
    const tokens = /** @type {Array<any>} */ (
      parseMessageMarkdown("**жирный** *курсив* ~~зачёркнуто~~ ||спойлер|| `код`")
    );
    assert.ok(Array.isArray(tokens), "parseMessageMarkdown должен вернуть массив токенов");
    assert.deepEqual(
      tokens.filter((token) => token.type !== "text").map((token) => token.type),
      ["bold", "italic", "strikethrough", "spoiler", "code"],
      "порядок и типы токенов не совпадают с markdown-разметкой",
    );
    const bold = tokens.find((token) => token.type === "bold");
    const boldChildren = /** @type {Array<{ value?: string }>} */ (bold.children);
    assert.equal(
      boldChildren.map((child) => child.value ?? "").join(""),
      "жирный",
      "содержимое токена bold",
    );
    const code = tokens.find((token) => token.type === "code");
    assert.equal(code.value, "код", "содержимое инлайн-кода");
  });

  it("сырой HTML остаётся текстом: <script>, <img onerror> не создают узлов", () => {
    const source = `<script>alert("xss")</script><img src=x onerror="alert(1)">`;
    const container = renderMarkdown(source);
    assert.equal(container.textContent, source, "текст сообщения должен дойти до DOM дословно");
    assert.equal(container.querySelectorAll("script").length, 0, "<script> не должен стать элементом");
    assert.equal(container.querySelectorAll("img").length, 0, "<img> не должен стать элементом");
    for (const element of container.querySelectorAll("*")) {
      assert.equal(element.hasAttribute("onerror"), false, "атрибут onerror не должен попасть в DOM");
      assert.equal(element.hasAttribute("onclick"), false, "атрибут onclick не должен попасть в DOM");
    }
  });

  it("ссылки: http(s) получают rel=noopener noreferrer и target=_blank", () => {
    const container = renderMarkdown("[сайт](https://example.com/a?b=1) и [второй](http://example.org)");
    const links = container.querySelectorAll("a");
    assert.equal(links.length, 2, "обе http(s)-ссылки должны отрендериться");
    const first = links[0];
    assert.ok(first, "первая ссылка должна существовать");
    assert.match(first.getAttribute("href") ?? "", /^https:\/\/example\.com\/a\?b=1$/, "href https");
    assert.equal(first.getAttribute("target"), "_blank", "target=_blank");
    const rel = (first.getAttribute("rel") ?? "").split(/\s+/);
    assert.ok(rel.includes("noopener"), "rel должен содержать noopener");
    assert.ok(rel.includes("noreferrer"), "rel должен содержать noreferrer");
    assert.equal(first.textContent, "сайт", "подпись ссылки");
    const second = links[1];
    assert.ok(second);
    assert.match(second.getAttribute("href") ?? "", /^http:\/\/example\.org/, "href http");
  });

  it("опасные схемы не становятся ссылкой", () => {
    const dangerous = [
      "javascript:alert(1)",
      "JaVaScRiPt:alert(1)",
      "data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==",
      "vbscript:msgbox(1)",
      "file:///etc/passwd",
      "java\tscript:alert(1)",
      "//evil.example.com/x",
    ];
    for (const href of dangerous) {
      const container = renderMarkdown(`[опасно](${href})`);
      assert.equal(container.querySelectorAll("a").length, 0, `«${href}» не должен стать ссылкой`);
      assert.match(container.textContent, /опасно/, "подпись отклонённой ссылки остаётся текстом");
    }
  });

  it("код: markdown и упоминания внутри не парсятся", () => {
    const context = { users: { "123": "Alice" } };
    const inline = renderMarkdown("`**не жирный** <@123> [x](https://e.com)`", context);
    const span = only(inline, "code");
    assert.equal(span.textContent, "**не жирный** <@123> [x](https://e.com)", "код дословный");
    assert.equal(inline.querySelectorAll("strong").length, 0, "внутри инлайн-кода нет жирного");
    assert.equal(inline.querySelectorAll("a").length, 0, "внутри инлайн-кода нет ссылок");
    assert.equal(inline.querySelectorAll("[data-user-id]").length, 0, "внутри кода нет упоминаний");

    const block = renderMarkdown("```js\n<@123> **b** ||s||\n```", context);
    const pre = only(block, "pre");
    assert.equal(pre.getAttribute("data-language"), "js", "язык блока кода");
    assert.equal(only(pre, "code").textContent, "<@123> **b** ||s||", "содержимое блока дословное");
    assert.equal(block.querySelectorAll("strong").length, 0, "внутри блока нет жирного");
    assert.equal(block.querySelectorAll("[data-spoiler]").length, 0, "внутри блока нет спойлеров");
    assert.equal(block.querySelectorAll("[data-user-id]").length, 0, "внутри блока нет упоминаний");
  });

  it("спойлер: data-spoiler=hidden, клик/Enter/Space раскрывают", () => {
    const container = renderMarkdown("до ||секрет|| после");
    const spoiler = only(container, "[data-spoiler]");
    assert.equal(spoiler.getAttribute("data-spoiler"), "hidden", "спойлер скрыт по умолчанию");
    assert.equal(spoiler.textContent, "секрет", "текст спойлера на месте");
    assert.equal(spoiler.getAttribute("tabindex"), "0", "спойлер доступен с клавиатуры");

    spoiler.dispatchEvent("keydown", { key: "a" });
    assert.equal(spoiler.getAttribute("data-spoiler"), "hidden", "посторонняя клавиша не раскрывает");
    spoiler.dispatchEvent("keydown", { key: "Enter" });
    assert.equal(spoiler.getAttribute("data-spoiler"), "revealed", "Enter раскрывает спойлер");

    const byClick = only(renderMarkdown("||клик||"), "[data-spoiler]");
    byClick.click();
    assert.equal(byClick.getAttribute("data-spoiler"), "revealed", "клик раскрывает спойлер");

    const bySpace = only(renderMarkdown("||пробел||"), "[data-spoiler]");
    bySpace.dispatchEvent("keydown", { key: " " });
    assert.equal(bySpace.getAttribute("data-spoiler"), "revealed", "Space раскрывает спойлер");
  });

  it("цитата > quote → blockquote, остальной текст не съеден", () => {
    const container = renderMarkdown("> первая строка\n> вторая строка\nобычный текст");
    const quote = only(container, "blockquote");
    assert.equal(quote.textContent, "первая строка\nвторая строка", "строки цитаты склеены построчно");
    assert.match(container.textContent, /обычный текст/, "текст после цитаты сохранён");
  });

  it("упоминания: пользователи, каналы, роли из context", () => {
    const context = {
      users: { "123": "Alice" },
      channels: { "456": "general" },
      roles: { "789": "Moderator" },
    };
    const container = renderMarkdown("<@123> <@!123> <#456> <@&789>", context);
    const users = container.querySelectorAll("[data-user-id]");
    assert.equal(users.length, 2, "оба варианта <@id> и <@!id> — упоминания пользователя");
    for (const user of users) {
      assert.equal(user.getAttribute("data-user-id"), "123");
      assert.equal(user.textContent, "@Alice", "имя из context.users");
      assert.ok(user.classes.has("mention"), "у упоминания есть класс mention");
    }
    const channel = only(container, "[data-channel-id]");
    assert.equal(channel.getAttribute("data-channel-id"), "456");
    assert.equal(channel.textContent, "#general", "имя канала из context.channels");
    const role = only(container, "[data-role-id]");
    assert.equal(role.getAttribute("data-role-id"), "789");
    assert.equal(role.textContent, "@Moderator", "имя роли из context.roles");
    assert.equal(
      container.querySelectorAll("[data-user-id=789]").length,
      0,
      "роль не должна рендериться как упоминание пользователя",
    );
  });

  it("@everyone и @here — только целым словом", () => {
    const container = renderMarkdown("@everyone @here почта@everyone.com @everyoneX");
    const everyone = container.querySelectorAll("[data-mention=everyone]");
    const here = container.querySelectorAll("[data-mention=here]");
    assert.equal(everyone.length, 1, "ровно одно @everyone");
    assert.equal(here.length, 1, "ровно одно @here");
    assert.equal(only(container, "[data-mention=everyone]").textContent, "@everyone");
    assert.equal(only(container, "[data-mention=here]").textContent, "@here");
    assert.match(container.textContent, /почта@everyone\.com/, "адрес почты остаётся текстом");
    assert.match(container.textContent, /@everyoneX/, "слово с продолжением не упоминание");
  });

  it("неизвестные id → fallback, битые упоминания остаются текстом", () => {
    const container = renderMarkdown("<@999> <#999> <@&999> <@abc> <@12 34> <@123 <img src=x>", {});
    assert.equal(only(container, "[data-user-id]").textContent, "@пользователь", "fallback пользователя");
    assert.equal(only(container, "[data-channel-id]").textContent, "#канал", "fallback канала");
    assert.equal(only(container, "[data-role-id]").textContent, "@роль", "fallback роли");
    assert.match(container.textContent, /<@abc>/, "нецифровой id остаётся текстом");
    assert.match(container.textContent, /<@12 34>/, "id с пробелом остаётся текстом");
    assert.match(container.textContent, /<@123/, "незакрытое упоминание остаётся текстом");
    assert.equal(container.querySelectorAll("img").length, 0, "инъекция через хвост не создаёт <img>");
  });

  it("данные context не инжектят HTML", () => {
    const container = renderMarkdown("<@123> <@&789> <#456>", {
      users: { "123": "<img src=x onerror=alert(1)>" },
      roles: { "789": "<script>alert(1)</script>" },
      channels: { "456": "\"><svg onload=alert(1)>" },
    });
    assert.equal(container.querySelectorAll("img").length, 0);
    assert.equal(container.querySelectorAll("script").length, 0);
    assert.equal(container.querySelectorAll("svg").length, 0);
    assert.match(container.textContent, /<script>alert\(1\)<\/script>/, "имя роли остаётся текстом");
  });

  it("жирный/курсив/зачёркнутый рендерятся элементами", () => {
    const container = renderMarkdown("**жирный** и *курсив* и ~~зачёркнуто~~");
    assert.equal(only(container, "strong").textContent, "жирный");
    assert.equal(only(container, "em").textContent, "курсив");
    assert.equal(only(container, "s").textContent, "зачёркнуто");
  });

  it("ссылка с форматированием: anchor содержит strong, href проверен", () => {
    const container = renderMarkdown("[**жирная** ссылка](https://example.com)");
    const link = only(container, "a");
    assert.equal(only(link, "strong").textContent, "жирная");
    assert.match(link.textContent ?? "", /ссылка/);
    assert.match(link.getAttribute("href") ?? "", /^https:\/\/example\.com/);
  });

  it("пустой текст не создаёт узлов, нестроковый вход → TypeError", () => {
    const container = renderMarkdown("");
    assert.equal(container.childElementCount, 0, "пустое сообщение не создаёт элементов");
    assert.equal(container.textContent, "", "пустое сообщение не создаёт текста");
    assert.throws(() => parseMessageMarkdown(/** @type {any} */ (null)), TypeError);
    assert.throws(() => parseMessageMarkdown(/** @type {any} */ (42)), TypeError);
  });

  it("рендер не использует innerHTML/outerHTML/insertAdjacentHTML", () => {
    const source = readFileSync(
      fileURLToPath(new URL("../../rust/gui-spike/ui/features/markdown.js", import.meta.url)),
      "utf8",
    );
    assert.doesNotMatch(source, /\binnerHTML\b/, "innerHTML в рендере сообщений запрещён");
    assert.doesNotMatch(source, /\bouterHTML\b/, "outerHTML в рендере сообщений запрещён");
    assert.doesNotMatch(source, /insertAdjacentHTML/, "insertAdjacentHTML запрещён");
    assert.doesNotMatch(source, /document\.write/, "document.write запрещён");
  });
});

/* =============================================================================
 * INT-02-UI-CHAT-MARKDOWN-TYPING (RW-40, G2 Frontend Integration Lane A):
 * подключение безопасного рендера renderMessageMarkdown в строки чата
 * (renderChatRow / renderChannelMessage) и токенизированные стили в composer.css.
 * ========================================================================== */

import {
  CHAT_ROWS,
  makeMessage,
  renderChannelMessage,
  renderChatRow,
} from "../../rust/gui-spike/ui/features/chat.js";

/**
 * Выполняет синхронную проверку с установленным `globalThis.document` для `chat.js`.
 * @template T
 * @param {(doc: ShimDocument) => T} fn
 * @returns {T}
 */
function withChatDoc(fn) {
  const prevDoc = globalThis.document;
  const doc = new ShimDocument();
  globalThis.document = /** @type {any} */ (doc);
  try {
    return fn(doc);
  } finally {
    if (prevDoc === undefined) {
      delete /** @type {any} */ (globalThis).document;
    } else {
      globalThis.document = prevDoc;
    }
  }
}

describe("INT-02: интеграция renderMessageMarkdown в строки чата и стили composer.css", () => {
  it("renderChatRow: рендерит markdown внутри .text-body и сохраняет контракт строки (§7.1)", () => {
    withChatDoc((doc) => {
      const row = doc.createElement("div");
      const base = makeMessage(12);
      const item = {
        ...base,
        text: "#12 **жирный** *курсив* ~~зачёркнуто~~ `код` [ссылка](https://example.com/docs)",
      };
      renderChatRow(/** @type {any} */ (row), item, 12);

      assert.equal(row.className, "row", "класс строки должен оставаться row");
      assert.equal(row.getAttribute("role"), "listitem");
      assert.equal(row.getAttribute("data-index"), "12");
      assert.equal(row.getAttribute("data-message-id"), "12");
      assert.equal(row.getAttribute("data-group"), item.group);
      assert.equal(row.getAttribute("aria-setsize"), String(CHAT_ROWS));
      assert.equal(row.getAttribute("aria-posinset"), "13");
      assert.equal(row.children.length, 2, "у строки ровно два ребёнка: .author и .text (§7.1)");

      const textNode = only(row, ".text");
      const body = /** @type {ShimElement | undefined} */ (textNode.children[0]);
      assert.ok(body, "первый ребёнок .text должен существовать");
      assert.equal(body.className, "text-body", "первый ребёнок .text всегда .text-body");
      assert.match(body.textContent, /^#12 /, "текст .text-body начинается с #<index>");

      assert.equal(only(body, "strong.md-bold").textContent, "жирный");
      assert.equal(only(body, "em.md-italic").textContent, "курсив");
      assert.equal(only(body, "s.md-strike").textContent, "зачёркнуто");
      assert.equal(only(body, "code.md-code").textContent, "код");

      const link = only(body, "a.md-link");
      assert.equal(link.getAttribute("href"), "https://example.com/docs");
      assert.equal(link.getAttribute("target"), "_blank");
      assert.equal(link.getAttribute("rel"), "noopener noreferrer");
      assert.equal(link.textContent, "ссылка");

      // Extras строки 12 (edited: 12 % 11 === 1, reply: 12 % 40 === 12) сохранены после .text-body
      assert.ok(textNode.querySelector(".edited"), "метка .edited сохранена");
      assert.ok(textNode.querySelector(".reply-quote"), "блок .reply-quote сохранён");
    });
  });

  it("renderChatRow: блоки кода, цитаты, спойлеры и упоминания в строке чата", () => {
    withChatDoc((doc) => {
      const row = doc.createElement("div");
      const item = {
        ...makeMessage(1),
        text: "||секрет|| <@123> <#456> <@&789> @everyone\n> цитата\n```rs\nlet x = 1;\n```",
      };
      const context = {
        users: { "123": "Alice" },
        channels: { "456": "general" },
        roles: { "789": "Core" },
      };
      renderChatRow(/** @type {any} */ (row), item, 1, context);

      const body = only(row, ".text .text-body");
      const spoiler = only(body, ".md-spoiler");
      assert.equal(spoiler.getAttribute("data-spoiler"), "hidden");
      assert.equal(spoiler.getAttribute("aria-expanded"), "false");
      spoiler.click();
      assert.equal(spoiler.getAttribute("data-spoiler"), "revealed");
      assert.equal(spoiler.getAttribute("aria-expanded"), "true");

      const userMention = only(body, ".mention--user");
      assert.equal(userMention.getAttribute("data-user-id"), "123");
      assert.equal(userMention.textContent, "@Alice");

      const channelMention = only(body, ".mention--channel");
      assert.equal(channelMention.getAttribute("data-channel-id"), "456");
      assert.equal(channelMention.textContent, "#general");

      const roleMention = only(body, ".mention--role");
      assert.equal(roleMention.getAttribute("data-role-id"), "789");
      assert.equal(roleMention.textContent, "@Core");

      const everyone = only(body, ".mention--everyone");
      assert.equal(everyone.getAttribute("data-mention"), "everyone");
      assert.equal(everyone.textContent, "@everyone");

      assert.equal(only(body, "blockquote.md-quote").textContent, "цитата");
      const pre = only(body, "pre.md-code-block");
      assert.equal(pre.getAttribute("data-language"), "rs");
      assert.equal(only(pre, "code.md-code").textContent, "let x = 1;");
    });
  });

  it("renderChatRow: блокирует XSS и опасные схемы ссылок в строке чата при активном markdown", () => {
    withChatDoc((doc) => {
      const row = doc.createElement("div");
      const item = {
        ...makeMessage(2),
        text: `**безопасно** <script>alert(1)</script><img src=x onerror=alert(1)> [клик](javascript:alert(1))`,
      };
      renderChatRow(/** @type {any} */ (row), item, 2);

      const body = only(row, ".text .text-body");
      assert.equal(only(body, "strong.md-bold").textContent, "безопасно");
      assert.equal(body.querySelectorAll("script").length, 0, "<script> не должен создаваться в строке чата");
      assert.equal(body.querySelectorAll("img").length, 0, "<img> не должен создаваться в строке чата");
      assert.equal(body.querySelectorAll("a").length, 0, "javascript:-ссылка не должна создавать <a>");
      assert.match(body.textContent, /<script>alert\(1\)<\/script>/);
      assert.match(body.textContent, /клик/);
    });
  });

  it("renderChannelMessage: рендерит markdown, спойлеры и упоминания в реальном канале без потери атрибутов", () => {
    withChatDoc((doc) => {
      const row = doc.createElement("div");
      /** @type {import("../../rust/gui-spike/ui/features/chat.js").ChannelMessage} */
      const item = {
        id: "1561627527296979046",
        author: "habr_bot",
        authorId: "9001",
        text: "**Заголовок** `inline` ||спойлер|| <@42> <#77> [habr](https://habr.com/ru/news/1/) <script>x</script>",
        time: "21:15",
        iso: "2026-10-23T21:15:00Z",
        day: "2026-10-23",
        dayLabel: "2026-10-23",
        group: "start",
        edited: true,
        bot: true,
        hue: 120,
        channelId: "1163408591315812392",
        reactions: [{ emoji: "🔥", count: 3, me: true }],
      };

      renderChannelMessage(/** @type {any} */ (row), item, 0, 3, {
        channelId: "1163408591315812392",
        context: {
          users: { "42": "Bob" },
          channels: { "77": "news" },
        },
      });

      assert.equal(row.className, "row channel-message");
      assert.equal(row.getAttribute("data-channel-message"), "true");
      assert.equal(row.getAttribute("data-message-id"), "1561627527296979046");
      assert.equal(row.getAttribute("data-group"), "start");
      assert.equal(row.getAttribute("aria-setsize"), "3");
      assert.equal(row.getAttribute("aria-posinset"), "1");

      const body = only(row, ".text .text-body");
      assert.equal(
        /** @type {any} */ (body.style).whiteSpace,
        "pre-wrap",
        "переносы реального канала сохраняются (pre-wrap)",
      );
      assert.equal(only(body, "strong.md-bold").textContent, "Заголовок");
      assert.equal(only(body, "code.md-code").textContent, "inline");
      assert.equal(only(body, ".md-spoiler").getAttribute("data-spoiler"), "hidden");
      assert.equal(only(body, ".mention--user").textContent, "@Bob");
      assert.equal(only(body, ".mention--channel").textContent, "#news");
      assert.equal(only(body, "a.md-link").getAttribute("href"), "https://habr.com/ru/news/1/");
      assert.equal(body.querySelectorAll("script").length, 0, "сырой <script> экранирован");

      assert.ok(row.querySelector(".edited"), "метка (изменено) сохранена в реальном канале");
      assert.ok(row.querySelector(".day-divider"), "разделитель дня сохранён в реальном канале");
      assert.ok(row.querySelector(".reactions-row .reaction"), "реакции сохранены в реальном канале");
    });
  });

  it("composer.css: токенизированные стили .md-code, .md-code-block, .md-spoiler, .mention без hex/rgb", () => {
    const composerCss = readFileSync(
      fileURLToPath(new URL("../../rust/gui-spike/ui/components/composer.css", import.meta.url)),
      "utf8",
    );
    const tokensCss = readFileSync(
      fileURLToPath(new URL("../../rust/gui-spike/ui/tokens.css", import.meta.url)),
      "utf8",
    );

    assert.match(composerCss, /\.md-code\s*\{/, "в composer.css должен быть селектор .md-code");
    assert.match(composerCss, /\.md-code-block\s*\{/, "в composer.css должен быть селектор .md-code-block");
    assert.match(composerCss, /\.md-spoiler\s*\{/, "в composer.css должен быть селектор .md-spoiler");
    assert.match(
      composerCss,
      /\.md-spoiler\[data-spoiler="hidden"\]/,
      "в composer.css должно быть состояние .md-spoiler[data-spoiler=\"hidden\"]",
    );
    assert.match(
      composerCss,
      /\.md-spoiler\[data-spoiler="revealed"\]/,
      "в composer.css должно быть состояние .md-spoiler[data-spoiler=\"revealed\"]",
    );
    assert.match(composerCss, /\.mention\s*\{/, "в composer.css должен быть селектор .mention");

    const withoutComments = composerCss.replace(/\/\*[\s\S]*?\*\//g, "");
    assert.doesNotMatch(withoutComments, /#[0-9a-fA-F]{3,8}\b/, "в composer.css запрещены raw hex-цвета");
    assert.doesNotMatch(withoutComments, /\brgba?\s*\(/i, "в composer.css запрещены raw rgb/rgba()");
    assert.doesNotMatch(withoutComments, /\bhsla?\s*\(/i, "в composer.css запрещены raw hsl/hsla()");
    assert.doesNotMatch(withoutComments, /!important/, "в composer.css запрещён !important");

    const definedTokens = new Set(
      [...tokensCss.matchAll(/(--[a-z0-9-]+)\s*:/gi)].map((match) => match[1]),
    );
    const mdStart = composerCss.indexOf(".md-code");
    assert.ok(mdStart >= 0, "секция markdown-стилей в composer.css не найдена");
    const mdSection = composerCss.slice(mdStart);
    const usedTokens = [...mdSection.matchAll(/var\(\s*(--[a-z0-9-]+)\s*\)/gi)].map((match) => match[1]);
    assert.ok(usedTokens.length >= 10, "стили markdown должны использовать токены var(--token)");
    for (const token of usedTokens) {
      assert.ok(definedTokens.has(token), `токен ${token} из composer.css отсутствует в tokens.css`);
    }
  });
});

