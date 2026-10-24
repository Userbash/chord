# PM / LOOP-LEDGER — счётчик итераций и сигнатуры отказов

Правило: сигнатура `sha1(finding | scope | command | first-error-line)`. Две одинаковые
подряд → `LOOP-DETECTED` → шаг 2 лестницы (смена подхода), третий идентичный заход запрещён.

| ID | Волна/карточка | Итер. | Сигнатура / суть | Статус | Решение |
|---|---|---|---|---|---|
| LOOP-01 | RW-38 acceptance | 3 | `P1=2` без исполнителя; ревью прозой, repro нет; PM не может ни принять, ни править | `closed` (D-05) | W38f green → REV-2 **PASS P0=0 P1=0**; принято |
| LOOP-02 | Проверка GUI | — | живое окно открывается в каждой итерации: `gate:worker` = 2 запуска, цикл воркера = 3–5; окно флейкует 1,6–3 с → PM повторяет ту же команду | `mitigated` | `WINDOW-BUDGET.md` + `window-budget.mjs` + `loop-guard.mjs`; итерации только DOM/model |
| L-01a | RW-38 P1-1 | 1 | `gui-window-probe.py` берёт первое окно/приложение, не свой PID → ложный pass | `closed` | PID-ветки подтверждены кодом + fake-desktop (0→None, 1→app, 2→Fail) |
| L-01b | RW-38 P1-2 | 1 | `gridcell` без `row` → AT-SPI `table children=0`; проба зелёная | `closed` | `listbox`+`option` + ≥10 именованных ячеек |
| L-01c | RW-38 P2-1 | 1 | при дисплее нет бинаря/`status=2` → `skip`, гейт зелёный | `closed` | независимо: exit 1, не skip |
| L-01d | RW-38 P2-3 | 1 | окно умирает через 1,6–3 с, launch/smoke зелёные; нет `proc.poll()` | `closed` | fake-спайк → launch fail на 1,05 с |
| OBS-01 | RW-38 P1-2 follow-up | — | `role=listbox/option` без клавиатуры: в `app.js` нет `keydown`/`tabindex`/`focus`/`aria-activedescendant` | `open` (новая сигнатура, не повтор P1-2) | после REV-2: либо `role=grid`+`row` (display:contents) с roving tabindex, либо `list/listitem`; отдельная карточка |
| OBS-02 | REV-2 P2-A | — | ветки PID-фильтра пробы (0 / ≥2 совпадений) не покрыты автотестом → регресс гейт не поймает | `open`, QA | карточка: параметризованный тест пробы на фейковых `get_process_id`/`_NET_WM_PID`, red-first |
| OBS-03 | REV-2 P3-3 | — | `smoke` без `failOnSkips`: live-слой может молча выпасть в skip; по `TEST-POLICY` skip ≠ pass | `open`, QA | `failOnSkips` условно (только при `DISPLAY`/`WAYLAND`), red-first на стабе без бинаря |

## Проверка PM (независимая, 2026-10-23 21:12)

- **P1-1 verified**: `gui-window-probe.py` — `_NET_WM_PID` (стр. 245) + `get_process_id`
  (стр. 123–129) + `proc.poll()` (стр. 98, 139); в `--json` есть `window.pid`.
- **P1-2 verified**: `#emoji-grid` → `role=listbox` (index.html:39), ячейки → `role=option`
  + `aria-label` (app.js:506–507); DOM-инвариант (dom.test:363–381); проба требует
  именованные ячейки в listbox (`CELL_ROLES`, стр. 394–405).
- **P2-1 verified**: skip только при `GUI_SPIKE_E2E=0` / `GUI_WINDOW_PROBE_OPTIONAL=1` /
  отсутствии дисплея.
- **P2-3 verified**: launch 9,16 с green, smoke 14,4 с green.
- Гейт W38f: `passed` (`.gate/worker-2026-10-23T16-10-23-494Z.json`).
- Тесты не ослаблены: в tracked-диффе (`git diff tests/`) удалённых `assert`/`t.skip` нет.
- **Пробел на REV-2**: `rust/gui-spike/**` untracked → диф недоступен, ревьюер читает тесты
  вручную и проверяет отсутствие ослаблений по содержимому.

## Закрытие LOOP-01 (2026-10-24)

- REV-2: **PASS** — P0 = 0, P1 = 0 (P1-1, P1-2, P2-1, P2-3 закрыты), P2 = 3, P3 = 4.
  Мутации 6/6 caught, md5 7/7, DOM 21/21, smoke 2/2 (0 skip), launch 1/1, проба
  `pid == window.pid`. Файл: `waves/RW-38/REVIEW-2.md`.
- Петля закрыта: у находок были исполнитель (W38f) и независимый ревьюер (REV-2),
  критерии — исполняемые команды, а не проза. Третий заход не потребовался.
- Открытые риски REV-2 переведены в карточки OBS-02 (PID-ветки без автотеста) и
  OBS-03 (`smoke` без `failOnSkips` — нарушение `TEST-POLICY` §2.3). Оба — QA-лейн.

## Журнал итераций

| Дата | ID | Итер. | Команда | Результат | sig |
|---|---|---|---|---|---|
| 2026-10-23 | LOOP-01 | 1 | `waves/RW-38/REVIEW-codex.md` (pass1) | FAIL, P0=0 P1=2 P2=10 | seed |
| 2026-10-23 | LOOP-01 | 2 | — | петля: нет исполнителя/исполняемых критериев | seed |
| 2026-10-23 | LOOP-01 | 3 | W38f red/green + `node --test tests/unit/gui-spike-dom.test.mjs` | green: DOM 20/20, launch 9,16 с, gate passed; смена подхода = исполняемые критерии | `ef633a2e` |
| 2026-10-23 | LOOP-02 | 1 | подсчёт запусков окна по логам W38f | ≥8 запусков на волну; причина — живая проба в цикле | `win-8` |
| 2026-10-23 | LOOP-02 | 2 | `node --test gui-spike-dom+model` (без окна) | 47/47 pass, 0 skip, 119 мс — детерминированный слой есть | `dom-47` |
| 2026-10-23 | LOOP-01 | 4 | `waves/RW-38/REVIEW-pass2.md` (REV-2) | **PASS** P0=0 P1=0; P2-1/P2-3 воспроизведены headless; окно 0/1 | `rev2-pass` |

## Эскалация

- 2 одинаковые сигнатуры → PM меняет подход (механизм/контракт/инструмент), запись в ledger.
- 3 возврата по карточке → эскалация владельцу с вариантами: принять ограничение / вынести
  в отдельную волну / сменить контракт (ADR).
