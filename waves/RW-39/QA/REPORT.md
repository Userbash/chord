# Отчёт QA-1 (RW-39): постоянные frontend-тесты + мутационная проверка

Дата: 2026-10-23. Роль: QA-1. PM: DeepSeek.
Окно, probe, `cargo`, `gate:worker`, сеть, commit/push — **не запускались**.
Продукт (`rust/gui-spike/ui/**`), чужие тесты и `waves/RW-36/37/38/**` — **не изменялись**.

## 0. Статус: **GREEN с одной честной дырой** (M6a MISSED)

| Проверка | Результат |
|---|---|
| `node --test tests/unit/gui-spike-features.test.mjs` | **10/10**, 0 fail, 0 skip |
| Итоговый прогон (dom + model + bench + features) | **102/102**, 0 fail, 0 skip |
| Мутанты | **caught 10 / missed 1** |
| md5 продуктовых файлов после restore | совпадают с baseline W39d |

## 1. Порт оракула → `tests/unit/gui-spike-features.test.mjs`

Оракул `waves/RW-39/PM/w39d/features-check.mjs` (9 проверок) перенесён на `node:test` +
`node:assert/strict` **без ослабления assert-ов**: те же селекторы, числа, тексты и порядок
проверок внутри кейса. Отличие только в изоляции — каждый кейс получает свежий `loadApp()`
(свой документ/очереди), а не общее состояние 9 последовательных проверок.

| # | Оракул (9) | Тест в `gui-spike-features.test.mjs` |
|---|---|---|
| 1 | `body[data-active-screen]` + `aria-current` на 4 экранах | «body[data-active-screen] + aria-current на 4 экранах» |
| 2 | плоский listbox (`option`, без `.emoji-row`), 10 вкладок-глифов | «эмодзи: плоский listbox … и 10 вкладок-глифов» |
| 3 | стрелки двигают `aria-selected` и roving tabindex | «эмодзи: стрелки двигают aria-selected и roving tabindex» |
| 4 | 38 канонических тумблеров + `#settings-nav` | «настройки: 38 канонических тумблеров … + #settings-nav» |
| 5 | панель участников ≤50, высота 42, `data-window=preview` | «панель участников: ≤50 строк из model.js, высота 42 …» |
| 6 | Alt+1…4, Escape, Tab-trap | «клавиши: Alt+1…4, Escape, Tab-trap верхнего слоя» |
| 7 | `data-action` mute/deafen/emoji/settings/all-members | «data-action: mute/deafen, emoji/emoji-close, …» |
| 8 | `aria-live` только у счётчиков | «счётчики live (polite), FPS/p95 — не live» |
| 9 | штатная работа без `console.error/warn` | «штатная работа без console.error/warn (весь сценарий)» — сценарий проигран целиком, чтобы смысл «после всех интеракций» сохранился |
| +1 | — (в оракуле отсутствовал) | «эмодзи-поиск: вкладка фильтруется по запросу, сброс возвращает все ячейки» |

**Почему +1.** Бриф называет «эмодзи-поиск/вкладки» инвариантом, но в `features-check.mjs`
поиска не было (он жил только в DOM-тесте). Добавлена одна проверка, покрывающая фильтр,
сброс и отсутствие дублей; 9 исходных проверок не тронуты. Итог: **10 тестов**, не 9.

## 2. Мутации (`run-mutations-frontend.mjs` → `mutations-frontend.log`)

Метод: копия `rust/gui-spike/ui` в `mkdtemp` (репозиторий read-only), одна мутация на копию,
прогон через штатный `GUI_DOM_APP=<копия>/app.js`, после — удаление копии и сверка md5
исходника. Команда каждого прогона:

```text
GUI_DOM_APP=<копия>/app.js node --test tests/unit/gui-spike-dom.test.mjs \
  tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs \
  tests/unit/gui-spike-features.test.mjs
```

| ID | Мутация | Файл | md5 мутанта | exit | Итог | Ловит features-тест |
|---|---|---|---|---|---|---|
| M1 | продолжение 26→44 | `model.js` | `c30cecde…` | 1 | **CAUGHT** | нет (ловят DOM: sizer/top/скролл) |
| M2 | начало группы 48→44 | `model.js` | `d4c44d4d…` | 1 | **CAUGHT** | нет (ловят DOM: sizer/top/скролл) |
| M3 | участник 42→40 | `model.js` | `8d56039b…` | 1 | **CAUGHT** | да (панель 42px) + DOM (sizer/скролл) |
| M4 | `#emoji-grid` listbox→grid (в рантайме) | `features/emoji.js` | `52e01795…` | 1 | **CAUGHT** | да (role=listbox) |
| M5a | ячейка `option`→`gridcell` | `features/emoji.js` | `a9426855…` | 1 | **CAUGHT** | да (role=option) + DOM |
| M5b | у ячейки убран `aria-label` | `features/emoji.js` | `f5299d0b…` | 1 | **CAUGHT** | да (непустой aria-label) + DOM |
| M6a | окно: `maxPages` 5→6 (бюджет 500→600) | `features/chat.js` | `923edabc…` | **0** | **MISSED** | нет |
| M6b | окно: `pageSize` 100→101 (бюджет 500→505) | `features/chat.js` | `375a9a38…` | 1 | **CAUGHT** | нет (ловит DOM: `loadedRows ≤500` + кратность 100) |
| M7 | убран `body[data-active-screen]` | `features/screens.js` | `a6b49791…` | 1 | **CAUGHT** | да (4 теста: экраны/клавиши/actions/консоль) |
| M8 | поиск эмодзи no-op | `features/emoji.js` | `dc38a5b3…` | 1 | **CAUGHT** | да (поиск/сброс) + DOM |
| M9 | тумблеров 38→37 (убран «Превью») | `features/settings.js` | `7217fc3f…` | 1 | **CAUGHT** | да (38/38, nav) + DOM |

md5 до/после restore для всех мутантов совпадают (лог, строки `md5 после restore … == до`):
`model.js 642e1d8f…`, `features/emoji.js 19593612…`, `features/chat.js d2d6433e…`,
`features/screens.js b5cad584…`, `features/settings.js 1499ed3b…`.

### M6: почему literal «500→501» не выражен и что реально проверено

В продукте лимит окна — **не литерал 500**, а `pageSize * maxPages = 100 * 5`
(`features/chat.js: CHAT_PAGE_SIZE = 100`, `CHAT_MAX_PAGES = 5`). При страничной
гранулярности ровно 501 материализованная строка невозможна (страница всегда 100 строк,
`PageStore.loadPage` требует точную вместимость), поэтому проверены два ближайших регресса:

- **M6a** `maxPages 5→6` (бюджет 600) — **MISSED**: snapshot при загрузке материализует
  всего **400 строк** чата (4 страницы: 0, 5, ~999 + overscan) и 100 участников, поэтому
  поднятый потолок не наблюдаем ни одним существующим тестом. Это не «зелень»: мутация
  действительно не ломает ни один инвариант, который сегодня кто-либо проверяет.
- **M6b** `pageSize 100→101` (бюджет 505) — **CAUGHT** DOM-тестом: `loadedRows` перестаёт
  быть кратным 100 и выходит за оракул `MAX_LOADED_ROWS = 500`.

**Остаток (нужна отдельная карточка):** чтобы ловить именно подъём `maxPages`, нужен тест,
который последовательно прокручивает ≥6 разных страниц без эвикции и проверяет
`loadedRows ≤ 500` на живом снапшоте/сторе. В features-тесте стора нет (только DOM/ARIA),
а DOM-тест и `app.js` — вне моей файловой матрицы. Пока инвариант «кап окна» защищён только
со стороны pageSize/кратности.

## 3. Итоговый прогон (задача 3)

```text
$ node --test tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs \
    tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs
ℹ tests 102  ℹ pass 102  ℹ fail 0  ℹ skipped 0  ℹ duration_ms 1338   exit=0
```

Разбивка: DOM 21 + model 26 + bench 45 + features **10** = 102.
(До этой карточки было 101 — ровно +1 новый файл с 10 тестами; 9 из них порт оракула.)

## 4. Файлы и md5

| Файл | md5 | Что |
|---|---|---|
| `tests/unit/gui-spike-features.test.mjs` | `a84c1019f652b108bcd6b81adae048c4` | новый: 10 тестов (9 порт + поиск) |
| `waves/RW-39/QA/run-mutations-frontend.mjs` | `bb0f10ca746469a045afdb49462fb033` | раннер 11 мутантов, copy+restore+md5 |
| `waves/RW-39/QA/mutations-frontend.log` | `7415f513fd463625773be0e3f5181505` | полный лог прогонов |
| `waves/RW-39/QA/mutations-frontend.json` | `811adac11deb3711e1e6afc12f4f3e5d` | машинный итог (caught/missed, упавшие тесты) |
| `waves/RW-39/QA/REPORT.md` | — | этот отчёт |

Продукт не изменялся (md5 совпадают с W39d): `app.js b88b1be9…`, `model.js 642e1d8f…`,
`features/*.js 74c343c6…/d2d6433e…/60871f52…/19593612…/61f08a21…/a468b04c…/3d1fba58…/
b5cad584…/a95bec6d…/1499ed3b…/e8d0db22…`.

## 5. Что осталось / риски

1. **M6a MISSED** — см. §2: нужен тест на окно ≥6 страниц (новая карточка, DOM-тест/стор).
2. `features`-тест изолирует кейсы (свежий `loadApp`), поэтому старый сценарий «проверка
   консоли после всех 8 интеракций» заменён явным проигрыванием сценария в 9-м тесте —
   покрытие то же, зависимость от порядка убрана.
3. Мутации M4/M5/M7 применялись к рантайм-коду (`features/*.js`), а не к `index.html`:
   шим строит DOM сам, статическая разметка тестами фич не читается (её проверяет DOM-тест).
   Поэтому продовый `index.html` в мутационном стенде не участвовал — это ограничение метода,
   не ослабление теста.
4. `waves/RW-39/QA/mutations-frontend.json` — вспомогательный артефакт; если PM хочет только
   `.log`, JSON можно удалить.
