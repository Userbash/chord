# RW-37 — TZ: оконная/постраничная модель данных (ADR-1 follow-up)

Статус: red-фаза. PM: DeepSeek. Дата: 2026-10-23.
Основание: `docs/rust-client/ADR-1.md` §4.1 — PSS 394 МБ > 300 МБ из-за 124 229
материализованных JS-объектов; нужна оконная/постраничная модель.

## 1. Цель

Снизить PSS дерева GUI-спайка до **≤300 МБ** (бюджет `ARCHITECTURE.md` §9), не потеряв
60 fps и корректность виртуализации. Плюс — RE-контракт реальной раскладки Discord для
следующей карточки P3-01.

Карточка не входит в счётчик 90 (это foundation-условие ADR-1); счётчик PLAN не меняется.

## 2. DoD

1. `rust/gui-spike/ui/model.js` — `PageStore`: страницы по `pageSize`, LRU-эвикция,
   `loadedRows ≤ pageSize * maxPages`, `ensureRange(first,last)` без двойных загрузок,
   `getRow(index)` для виртуализации, `stats()`.
2. `tests/unit/gui-spike-model.test.mjs` — red-first, ≥10 тестов: пустой стор, границы
   первой/последней страницы, эвикция LRU, повторный доступ без перезагрузки, out-of-range,
   невалидные `pageSize`/`maxPages`/`totalRows`, `stats`.
3. `ui/app.js` использует `PageStore` для чата (100k) и участников (24 229); rendered-строки
   остаются ≥10; `UI_SNAPSHOT` и `BENCH` не ломаются.
4. Перезамер на финальном бинаре: **PSS ≤ 300 МБ**, 60 fps, p95 ≤ 20 мс; артефакты в
   `waves/RW-37/`.
5. RE-воркер: `waves/RW-37/RE/RESEARCH.md` + `layout.json` — реальные размеры раскладки и
   пагинация из `reference-oracle/` (только чтение), с провенансом.
6. Ревью: P0/P1 = 0; мутации Rust/JS по назначенным файлам.

## 3. Файловая матрица

| Владелец | Файлы |
|---|---|
| W37a (REA) | `waves/RW-37/RE/*` (только чтение дампа) |
| W37b (UI/model) | `rust/gui-spike/ui/model.js`, `tests/unit/gui-spike-model.test.mjs`, правки `rust/gui-spike/ui/app.js` в секциях чата/участников |
| QA-1 | `waves/RW-37/run-mutations*.py`, `waves/RW-37/mutations*.log` |
| REV-1 | `waves/RW-37/REVIEW*.md` |
| PM (Lead) | `waves/RW-37/TZ.md`, `RUNBOOK.md`, `REPORT.md`, `METRICS.md`, интеграция, гейт |

Чужие файлы не трогать. `waves/RW-36/*` — read-only история.

## 4. Контракт `PageStore` (замораживается до кода)

```js
new PageStore({ totalRows, pageSize, maxPages, fetchPage })
// fetchPage(pageIndex) -> Array<row>; может быть async
getRow(index) -> row | undefined      // undefined = страница ещё не загружена
ensureRange(first, last) -> Promise<void>  // грузит покрывающие страницы, LRU-эвикция
stats() -> { pages, loadedPages, loadedRows, maxPages, pageSize, totalRows }
```

Инварианты:
- `loadedRows ≤ pageSize * maxPages` всегда.
- Эвикция — по последнему доступу (LRU), текущий диапазон не эвиктится.
- `getRow` вне `[0,totalRows)` → `undefined`, без исключения.
- `pageSize/maxPages/totalRows` — целые > 0; иначе `TypeError` в конструкторе.
- `ensureRange` идемпотентен: повторный вызов для того же диапазона не перезагружает страницы.

## 5. RE-контракт (W37a)

Источники (только чтение): `reference-oracle/ui*/`, `docs/re/frontend-d03.md`,
`docs/re/gateway-schema.md`, `docs/re/rest-d02.md`.
Нужно: ширины рейла/дерева/списка, высоты строк (cozy/compact), размеры вьюпорта,
лимиты `GET /messages` и поведение пагинации, счётчики READY. Каждое число — с путём
к артефакту; без домыслов, спорное — помечать `assumption`.

## 6. Гейт и приёмка

- Targeted: `cargo test` спайка, `node --test tests/unit/gui-spike-model.test.mjs`, UI `tsc`.
- Один batch gate: `npm run gate:worker` + `cargo test --offline --workspace`.
- Живой замер: `python3 waves/RW-37/remeasure.py` (адаптировать из RW-36) — PSS ≤ 300 МБ.
- Ревью codex: P0/P1 = 0; иначе PM возвращает воркеру с конкретным списком.
