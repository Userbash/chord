# REPORT — RW-37: оконная/постраничная модель данных (ADR-1 follow-up)

Статус: **принято**. Дата: 2026-10-23. PM: DeepSeek. Воркеры: W37a (REA), W37b (код),
QA-1 (мутации), REV-1 (ревью). Карточка — foundation-условие ADR-1, счётчик PLAN не меняется
(48/90).

## 1. Итог

| Метрика | RW-36 | RW-37 | Цель |
|---|---:|---:|---|
| PSS дерева | 393,9 МБ | **276,2 МБ** | ≤300 МБ ✅ |
| Объектов в JS | 124 229 | **500** | — |
| FPS / p95 | 60 / 18 мс | 60 / 18 мс | ≥55 / ≤20 ✅ |
| Cold start | 137/102 мс | 142/102 мс | ≤1,5 с ✅ |

`PageStore` держит не более `pageSize*maxPages = 500` строк на список; остальное
подгружается страницами (LRU, `inFlight`, точная вместимость страницы).

## 2. PM-процесс: постановка → проверка → возврат → приёмка

1. **Постановка** (`waves/RW-37/TZ.md`): замороженный контракт §4, файловая матрица,
   DoD, RE-контракт. Запущены 2 воркера параллельно (REA + код).
2. **W37b, проход 1**: принят по PSS-предварительному (287,9/298,2), но **возвращён**:
   R1 — короткая не-последняя страница = вечные дыры; R2 — хрупкий динамический import;
   R3 — placeholder без CSS. Переработка: red-first R1, статический ES-модуль, `.placeholder`.
3. **PM-замер**: PSS 274,2 МБ, 60 fps; бинарь пересобран.
4. **QA-1**: 11 caught, 2 эквивалентных, 1 пробел m07 (`evict` игнорирует pinned).
   **W37b, проход 2**: m07 red-first (мутант killed), 64/64.
5. **REV-1**: PASS (P0/P1=0), 5 P2. **W37b, проход 3**: P2-1 (дубли DOM в placeholder),
   P2-2 (пустая последняя страница = вечная дыра) — red-first, 65/65.
6. **PM**: P2-5 — smoke теперь ассертит `loadedRows ≤ 500`; P2-3 — драйвер мутаций
   восстанавливает mtime; P2-4 — этот `METRICS.md` + sha; rebuild-guard в smoke.
7. **QA-1, повтор**: m07 caught; m09 (перегрузка страницы) выжил → **W37b, проход 4**:
   тест «capacity+1 → RangeError» red-first; 66/66; m09 caught.
8. **a02** (`initRowLists` не вызван): PM провёл контрольную мутацию + rebuild → live smoke
   упал «UI_SNAPSHOT отсутствует»; restore md5 совпал. Пробел закрыт процессно (smoke+guard).
9. **Приёмка**: PSS 276,2 ≤300, 60 fps, P0/P1=0, все non-equivalent мутанты killed.

## 3. Артефакты

| Артефакт | Содержимое |
|---|---|
| `waves/RW-37/TZ.md` | контракт, матрица, DoD |
| `waves/RW-37/RE/RESEARCH.md`, `layout.json` | RE-контракт раскладки/пагинации (W37a) |
| `waves/RW-37/workers/W37b/` | red/green/checks-логи 4 проходов, REPORT |
| `waves/RW-37/mutations-model.log` | 16 мутаций: 12 caught, 2 эквивалентных, a02 процессно |
| `waves/RW-37/REVIEW-codex.md` | независимое ревью (PASS, 5 P2 разобраны) |
| `waves/RW-37/METRICS.md` | финальные числа и sha |
| `waves/RW-37/REVIEW-CHECKLIST.md`, `RUNBOOK.md` | PM-чек-лист и runbook |

## 4. Файлы + md5

```
ccdbb0a4c7e407aaa16f824ee4eeffe9  rust/gui-spike/ui/model.js
32ed027bc8cf5c72f5a0ca4705706eab  rust/gui-spike/ui/app.js
d2ca86ad0a798854aa2c609697ae6dba  rust/gui-spike/ui/index.html
f944ef11bfed75cd38116df9b563a4c8  rust/gui-spike/ui/style.css
8580f717aab812215cc8101b8cf7123a  tests/unit/gui-spike-model.test.mjs
39946fc601b458f142dc02c7a6a3f2ec  tests/smoke/gui-spike.smoke.test.mjs
e88c9c0572b3339516bcd4fa507b50a2  waves/RW-37/run-mutations-model.py
27b0161f40b0c1ac28ed736158a1e2c4  waves/RW-37/mutations-model.log
8250fae62af4be137f76c593f4031962  waves/RW-37/RE/RESEARCH.md
10af9a8783ae64a9f8cba8e8e4201929  waves/RW-37/RE/layout.json
e1093c12d1547e657ec74e144dc6ab38  waves/RW-37/METRICS.md
```

Бинарь: sha256 `dfca31381a0501c914004d7f2ce626c57103f624fd7701885ffc87662c0b0d45`.

## 5. Проверки

| Проверка | Результат |
|---|---|
| targeted model+bench+dom | **66/66** |
| Rust-спайк | **16/16** |
| UI tsc / root tsc | 0 / 0 |
| live smoke (`GUI_SPIKE_E2E=1`) | **1/1** |
| `npm run gate:worker` | **passed_with_skips** (meta 29/29, unit 234/234, smoke 1 opt-in) |
| `cargo test --offline --workspace` | **1069 passed / 0 failed** |
| Мутации model.js | 12 caught, 2 эквивалентных (m01, m10), 0 inconclusive |
| Мутации app.js | a01 compile-error (by design), a02 — live smoke после rebuild |

## 6. RE-контракт (W37a, кратко)

- Вьюпорт 2048×1106 (dpr 1.25): rail 64, tree 311, chat 1417, member list 256×1025,
  header 49, titlebar 32, composer 56.
- Строки: сообщение cozy 26/48 (+22 перенос), compact 26, участник 42, канал 28.
- Пагинация: реальные `limit=12/24/30`, история канала **≤120, trim до 60**, на старте 12.
- READY: 7 гильдий / 272 канала / 146 users / 17 ЛС; участники — окно **100** (`range=0x0063`).
- 10 assumption (A1–A10) помечены; `op37` в дампе нет — механизм op14+op8.

## 7. Риски / follow-up (P3)

1. **Release-профиль** не измерялся; PSS/старт — debug. Перед релизом повторить.
2. **UI-tsc не в `npm run typecheck`** — правка `package.json` требует разрешения владельца.
3. **a02 unit-оракул**: app.js не исполняется в unit-тестах (нет DOM-харнесса); сейчас
   покрыт live smoke + rebuild-guard. jsdom — новая зависимость, не добавлялась.
4. **RE — один вьюпорт/сессия**; для P3-01 точные панели проверить на живом клиенте в
   согласованное окно.
5. Эквивалентные мутанты m01/m10 — не дефект; код не менялся.
6. Коммитов нет — по процессу их делает координатор.
