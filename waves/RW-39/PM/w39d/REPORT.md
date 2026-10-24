# Отчёт W39d (Frontend) — интеракции на разметке W39c2

Дата: 2026-10-23. PM: DeepSeek. Воркер: W39d. `cargo`, окно, probe, `gate:worker` —
**не запускались** (WINDOW-BUDGET, параллельный W39e).

## 0. Статус: **GREEN**

| Проверка | Exit | Результат |
|---|---|---|
| `node --test tests/unit/gui-spike-dom.test.mjs` | 0 | **21/21**, 0 fail, 0 skip |
| `node --test tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs` | 0 | **71/71** (model 26 + bench 45) |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | 0 | пусто |
| `node waves/RW-39/PM/w39c/geometry-check.mjs` | 0 | **31/31** в допуске |
| `node waves/RW-39/PM/w39d/features-check.mjs` (мой оракул) | 0 | **9/9** |
| `node waves/RW-39/PM/w39d/model-bench.mjs` (headless D-04) | 0 | p95 0.0067 мс/окно |
| Мутация «константа 44» (`GUI_DOM_APP`) | **1 (ожидаемо)** | 3 теста падают |

## 1. Фича → проверка → результат

| Фича | Проверка | Результат |
|---|---|---|
| Rail-навигация 4 экранов, `body[data-active-screen]` вместо стейл-атрибута, `aria-current` только у активной | DOM-тест «четыре экрана…»; features-check «body[data-active-screen] + aria-current» | ok: ровно один `.rail-btn.active`/`.screen.active`, атрибут на body меняется на каждом клике |
| `#emoji-grid` — **плоский** `listbox`, ячейки `option` + `aria-label`, 10 вкладок-глифов, поиск, roving tabindex, стрелки/Home/End/Enter | DOM-тесты P1-2/10 вкладок/поиск; features-check «плоский listbox», «стрелки двигают aria-selected» | ok: `.emoji-row` = 0, `title === textContent`, один `tabindex=0`, `aria-selected` ровно у одной ячейки/вкладки |
| Настройки: 38 канонических тумблеров (`.toggle-label` + `input.toggle-input[role=switch]` + `.toggle-track > .toggle-thumb`), `#settings-nav` из 3 кнопок, `aria-checked` на click **и** change | DOM-тесты 38 чекбоксов/клик; features-check «38 канонических тумблеров» | ok: 38/38, 3 h3, nav без h3, `aria-checked` синхронизирован |
| Панель участников `#member-panel-list` (≤50, высота 42, `data-window=preview`) и экран `#member-list` из `model.js` (PageStore, `loadedRows ≤ 500`) | DOM-тест PageStore/участники; features-check «панель участников» | ok: панель 50 строк, виртуализация 24 229×42, `loadedRows` 100…500 и кратно 100 |
| `data-action` §8.1, клавиши §8.2 (Alt+1…4, Ctrl+E, Ctrl+,, Ctrl+F/`/`, Escape, Tab-trap, стрелки списков) | features-check «клавиши», «data-action» | ok: экран переключается, Tab удерживается в панели, Escape возвращает чат |
| `aria-live` только у счётчиков: снят с `.metrics` (FPS/p95), оставлен у `#member-count`/`#member-online`/`.screen-title` | features-check «счётчики live» | ok |
| Ошибки: `UIError` (код + причина) → `reportError` + видимый `.error-state` в родителе списка/`.content`; штатный сценарий — 0 `console.error/warn` | DOM-тест «при обычной работе…»; features-check «штатная работа…» | ok, оба массива пусты |
| D-04: переменная высота 26/48/+22, участник 42 — модель, виртуализатор, ожидания теста, контракт | 5 DOM-тестов высот; мутация «44» | ok, мутация ловится (см. §4) |

## 2. D-04: old → new (RE-оракул `waves/RW-37/RE/layout.json`)

| Что | Было | Стало |
|---|---|---|
| Высота строки сообщения | `44` (константа) | `26` продолжение / `48` начало группы / `+22` за перенос |
| Высота строки участника | `40` (трек) | **42** (панель и виртуализация) |
| Sizer `#chat-list` (100 000 строк) | `4 400 000 px` | `3 375 060 px` |
| Sizer `#member-list` (24 229 строк) | `969 160 px` | `1 017 618 px` |
| `top(i)` | `i × 44` | префиксная сумма высот (`HeightIndex`, period 1564) |
| Строк в окне при 800 px | 35 | ~41 (лимит теста 60) |
| `#chat-list[data-row-height]` (runtime) | `44` | `26/48/+22` |
| `#member-list[data-row-height]` (runtime) | `40` | `42` |

Пары «индекс → высота» (демо-данные: группа из 4, +1 строка у `i%17=9`, ещё +1 у
`i%23=9`) — в тесте литерально: `0→48`, `1→26`, `4→48`, `9→70`, `26→48`, `32→70`.
Независимость: тест считает префиксные суммы сам, `app.js`/`model.js` не импортируются
для ожиданий. Контракт: `DOM-CONTRACT.md` §2 (ревизия), §6.1, §6.3, §7.1–§7.2, §9.5, §10.2.

## 3. Вывод проверок

```text
$ node --test tests/unit/gui-spike-dom.test.mjs
ℹ tests 21  ℹ pass 21  ℹ fail 0  ℹ skipped 0            exit=0

$ node --test tests/unit/gui-spike-model.test.mjs tests/unit/gui-spike-bench.test.mjs
ℹ tests 71  ℹ pass 71  ℹ fail 0  ℹ skipped 0            exit=0

$ npx tsc -p rust/gui-spike/ui/tsconfig.json
(пусто)                                                 exit=0

$ node waves/RW-39/PM/w39c/geometry-check.mjs
все 31 геометрических значений в допуске                exit=0

$ node waves/RW-39/PM/w39d/features-check.mjs
ok | body[data-active-screen] + aria-current на 4 экранах
ok | эмодзи: плоский listbox ... и 10 вкладок-глифов
ok | эмодзи: стрелки двигают aria-selected и roving tabindex
ok | настройки: 38 канонических тумблеров ... + #settings-nav
ok | панель участников: ≤50 строк из model.js, высота 42, data-window=preview
ok | клавиши: Alt+1…4, Escape, Tab-trap верхнего слоя
ok | data-action: mute/deafen, emoji/emoji-close, settings/settings-close
ok | счётчики live (polite), FPS/p95 — не live
ok | штатная работа без console.error/warn
features: все проверки пройдены                          exit=0
```

## 4. Мутация «вернуть константу 44» (обязательна по D-04)

Копия `ui/` в `/tmp/opencode/w39d-mut44`, в `model.js::messageHeight()` возвращён
`return 44`; прогон через `GUI_DOM_APP` (репозиторий не менялся):

```text
ℹ tests 21  ℹ pass 18  ℹ fail 3     exit=1
✖ чат: … sizer = сумма оракульных высот
  actual 4400000 !== expected 3375060
✖ чат: top строки = префикс оракульных высот …
  '44px' !== '48px'
✖ скролл чата: окно меняется …
```

Лог: `mutation-const44.log`. Тест **ловит** константу 44 (sizer, высота строки 0,
позиции/скролл).

## 5. Метрики (100 000 / 24 229 / 10 / 38)

- **Живой BENCH не перемерялся** (окно запускает только приёмка; W39e с cargo
  параллельно). База RW-38: 60 fps, p95 кадра **18 мс**, PSS 247,4 МБ.
- Headless-оценка накладных D-04 (`model-bench.mjs`, только расчёт окна, без layout):
  - `HeightIndex` на 100 000 строк строится за **0,307 мс** (period 1564, без массива смещений);
  - чат: окно 41 строка, paint **avg 0,0028 мс / p95 0,0067 мс** (n=300);
  - участники: окно 36 строк, paint **avg 0,0013 мс / p95 0,0018 мс**;
  - round-trip `indexAt(top(i)) = i` и `top(totalRows) = totalHeight` — ok.
- Расчётная часть D-04 не может съесть 20 мс бюджета (0,007 мс ≈ 0,04 % кадра);
  рост DOM-строк 35 → 41 остаётся под лимитом 60. Реальный p95 — за приёмкой.
- RAM: число материализованных строк не изменилось (`loadedRows ≤ 500` на список,
  `PageStore.inFlight` без изменений), новых массивов на 100 k нет.

## 6. Файлы и md5

| Файл | md5 | Что |
|---|---|---|
| `rust/gui-spike/ui/app.js` | `b88b1be996dc95d6894f044304ab8b5a` | сборка фич, UI_SNAPSHOT, autobench, видимая поверхность ошибок |
| `rust/gui-spike/ui/model.js` | `642e1d8f9a35ca0bc6e0dced394ebd77` | **только D-04**: `ROW_HEIGHTS`, `messageHeight`, `HeightIndex`, `createChatHeights`, `createMemberHeights` |
| `rust/gui-spike/ui/features/dom.js` | `60871f52feea9d56bdf05db36837c1b7` | хелперы |
| `rust/gui-spike/ui/features/errors.js` | `61f08a21e6e98acdb0437c23796477f4` | `UIError`, `.error-state` |
| `rust/gui-spike/ui/features/virtual-list.js` | `e8d0db2247cc7ef03c78a8efc257aa27` | виртуализация на `HeightIndex` |
| `rust/gui-spike/ui/features/chat.js` | `d2d6433e6e47dd19c28aad8f073c2c17` | строки §7.1, поиск, store |
| `rust/gui-spike/ui/features/members.js` | `a468b04cd1362ff1c28c850b492cdd15` | экран 42, панель 50, поиск |
| `rust/gui-spike/ui/features/emoji.js` | `19593612f1c70827ba9e5d8b28a4bcde` | плоский listbox, 10 вкладок, roving |
| `rust/gui-spike/ui/features/settings.js` | `1499ed3b396ae1a57b6ec73b31178af9` | 38 тумблеров, `#settings-nav` |
| `rust/gui-spike/ui/features/screens.js` | `b5cad5847b8ca4b99d70a8df9ababfc5` | rail, `body[data-active-screen]` |
| `rust/gui-spike/ui/features/metrics.js` | `3d1fba588afb9270ea83dad610958a14` | FPS/p95, process_metrics, bench |
| `rust/gui-spike/ui/features/actions.js` | `74c343c68b806d79975c2bb8156c05f1` | §8.1/§8.2, focus-trap, клавиатура списков |
| `rust/gui-spike/ui/features/search.js` | `a95bec6d0a5c22ccf5fa4dacfce883e7` | поиск по загруженному окну |
| `tests/unit/gui-spike-dom.test.mjs` | `a0c2f7c53fd06db1781825862ea7189e` | **только высоты D-04** (константы/пары/top/sizer/скролл) |
| `tests/support/gui-dom-harness.mjs` | `335f762fa72c44640c85db8c373cba96` | аддитивно: `data-action` у зеркальных кнопок (см. §7.2) |
| `waves/RW-39/DESIGN/DOM-CONTRACT.md` | `0dc3e0cf2207789c7281c7688d0b057e` | высоты 26/48/+22/42, §9.5 |

Read-only не тронуты (md5 совпадают с W39c2): `index.html 3dcf9bc4…`,
`style.css f660b65d…`, `tokens.css 3c88f2d7…`, все `components/*.css`, `src/**`.

## 7. Отклонения и остаток (честно)

1. **`DOM-CONTRACT §9.2` vs факт.** В брифе высоты адресованы «§9.2», но §9.2 в
   контракте — Plan A про харнесс; числа 44/40 живут в §2.4/§2.6/§6.1/§6.3/§7.1–7.2.
   Сделал связную правку именно там (иначе документ противоречил бы себе) + новый
   **§9.5 «Ревизия D-04»** с таблицей и мутацией. §9.2 не менялся. Прошу PM
   подтвердить такую трактовку.
2. **Харнесс — единственная правка `tests/support`.** `buildDom` не зеркалил
   `data-action` из `index.html`, поэтому §8.1-действия были недоступны в шиме.
   Добавил только атрибуты (`iconBtn(..., action)`, `profile`, `attach`,
   `all-members`) — узлы/порядок/`boxHeight`/assert-ы не тронуты (диф в отчёте PM).
   Без этого `features-check` не может проверить действия; сам DOM-тест остаётся 21/21.
3. **Постоянные тесты на новые фичи не добавлены в `tests/**`** — брифе разрешает
   менять DOM-тест только по высотам. Оракул `features-check.mjs` лежит в
   `waves/RW-39/PM/w39d/` (9/9). Предлагаю отдельной карточкой перенести его в
   `tests/unit/` после согласования файловой матрицы.
4. **CSS-остаток (read-only):** `chat.css` держит `.text { height: 40px }`,
   `.text-body { white-space: nowrap }`; W39d компенсирует inline-высотой и
   `white-space: normal` у многострочных строк, канонические правила — W39c2.
   Блок `:has()` в `style.css` теперь избыточен (атрибут выставляется) — удалять W39c2/PM.
   `#chat-list/#member-list[data-row-height]` в статическом html всё ещё 44/40;
   runtime обновляет, статику стоит поправить W39c2.
5. **Счётчик онлайна** `#member-online` = 18 172 — из синтетических данных
   (3 статуса из 4); в html было 735. Число согласовано с генерируемыми статусами,
   не с реальным сервером.
6. **Поиск** (chat/member) ищет по загруженному окну (≤500 строк) и показывает
   `найдено / загружено`; полный поиск — серверный, Phase 3. Это осознанная граница.
7. **Живой p95/PSS/AT-SPI/скриншот** — не запускались (WINDOW-BUDGET). Нужен один
   прогон на приёмке: `BENCH` + `UI_SNAPSHOT` (`loadedRows ≤ 500`) + визуальная
   сверка 26/48/+22.
8. Фазы 3 (`threads/pins/profile/guild-menu/attach`) — заглушки-affordance с
   комментариями best-effort; ошибок не глотают (клик меняет `aria-expanded/pressed`).

## 8. TDD red → green (логи в этой папке)

| Шаг | Лог | Результат |
|---|---|---|
| baseline до правок | `red-baseline-dom.log` | 21/21 (старые ожидания 44/40) |
| red: новые ожидания D-04 | `red-dom.log` | **16/21**, 5 fail (sizer 4 400 000 ≠ 3 375 060; `44px` ≠ `48px`) |
| red: оракул фич | `red-features.log` | fail на первом же пункте (`data-active-screen` = null) |
| green: DOM | `dom.log` | 21/21 |
| green: model+bench | `model-bench.log` | 71/71 |
| green: tsc | `tsc.log` | пусто, exit 0 |
| green: geometry | `geometry.log` | 31/31 |
| green: features | `features.log` | 9/9 |
| мутация 44 | `mutation-const44.log` | 3 fail (ожидаемо) |
| headless D-04 | `model-bench-headless.log` | p95 0,0067 / 0,0018 мс |

Коммитов/пушей нет, сеть не использовалась, `logs/`, `functions/`, `evidence/`
не трогались.
