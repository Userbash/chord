# RW-37 — PM acceptance checklist

PM принимает волну только при выполнении всех пунктов. Любой FAIL — возврат воркеру
с конкретным списком (файл:строка, что не так, что ожидается).

## A. Контракт и red-first

- [ ] `PageStore` реализует ровно контракт TZ §4 (сигнатуры, возвраты, TypeError).
- [ ] Тесты появились ДО кода: red-лог в `waves/RW-37/workers/W37b/red.log` фиксирует падение.
- [ ] ≥10 тестов, включая границы/LRU/идемпотентность/out-of-range/невалидные параметры.
- [ ] Тесты проверяют контракт, а не внутреннюю реализацию.

## B. Инварианты

- [ ] `loadedRows ≤ pageSize * maxPages` при любом сценарии.
- [ ] LRU-эвикция не выкидывает текущий видимый диапазон.
- [ ] Одна страница не грузится дважды при конкурентных `ensureRange`.
- [ ] `getRow` вне диапазона → `undefined`, без throw.
- [ ] `pageSize/maxPages/totalRows ≤ 0` или нецелые → `TypeError`.

## C. Интеграция

- [ ] `app.js` не создаёт `makeMessages(100000)`/`makeMembers(24229)` целиком.
- [ ] Виртуализация работает: `UI_SNAPSHOT.chatRenderedRows ≥ 10`, `memberRenderedRows ≥ 10`.
- [ ] `BENCH` присутствует, `lowSample`/`stalled` отсутствуют.
- [ ] `UI_SNAPSHOT` не потерял обязательные поля (`SNAPSHOT_FLOORS` Rust-валидатора).

## D. Метрики

- [ ] Финальный бинарь пересобран; sha256 зафиксирован в `METRICS.md`.
- [ ] `pss.json`: **PSS ≤ 300 МБ**.
- [ ] FPS ≥55, p95 ≤20 мс.
- [ ] Артефакты: `app.log`, `metrics.json`, `metrics-tree.json`, `pss.json`.

## E. RE-контракт

- [ ] `waves/RW-37/RE/RESEARCH.md` + `layout.json` существуют.
- [ ] Каждое число имеет провенанс (путь к артефакту); `assumption` помечены.
- [ ] Нет изменений в `reference-oracle/` (golden read-only).

## F. Гейт и ревью

- [ ] `node --test` (модель + bench + dom) зелёный.
- [ ] `npx tsc -p rust/gui-spike/ui/tsconfig.json` — 0.
- [ ] `cargo test` спайка зелёный; `cargo test --offline --workspace` — 0 failed.
- [ ] `npm run gate:worker` — passed (или passed_with_skips с объяснением).
- [ ] Мутации по `model.js`/`app.js` — списки survivors разобраны.
- [ ] Ревью codex: **P0/P1 = 0**; P2 закрыты или осознанно отложены.
