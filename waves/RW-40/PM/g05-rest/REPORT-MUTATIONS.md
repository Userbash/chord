# G-05-REST — отчёт по мутациям (закрытие P1 из REVIEW.md)

Дата: 2026-10-30. Воркер: **G2 Frontend Lane B**. Карточка: **G05-REST-MUTATIONS**.
Основание: `waves/RW-40/PM/g05-rest/REVIEW.md`, раздел «P1 — нет mutation evidence для
новых инвариантов» (§3/§9 `TEST-POLICY.md`). Предыдущий отчёт: `REPORT.md`.

## 1. Что сделано

Прогнал мутационное тестирование инвариантов D1–D7 в
`rust/gui-spike/ui/features/actions.js`. Шесть целевых мутантов, по одному за раз,
орacle — существующий `node --test tests/unit/server-menu.test.mjs` (27 тестов) без
единой правки тестов. Плюс два диагностических мутанта по D1, чтобы объяснить miss.

Оракул не менялся: `tests/unit/server-menu.test.mjs` md5 `83ed20625a33779ea173431c3d0da836`,
как в `REPORT.md`. Мутировал только продуктовый файл и каждый раз возвращал его из
побайтовой копии.

## 2. Результаты

| ID | Инвариант | Мутация | md5 мутанта | exit | fail | Вердикт |
|---|---|---|---|---|---|---|
| M1 | D1 | guard `aria-disabled` в диспетчере → `if (false)` | `8dcb1b42e02b9a9594c1cbae3084237d` | 0 | 0 | **missed** |
| M1b | D1 | guard в обработчике `guild-invite` снят | `7e37114b05410eb785dc65bfc1753f4e` | 0 | 0 | **missed** |
| M1c | D1 | сняты оба guard'а (M1+M1b) | `cb40980705b5cd22feb50afb45149960` | **1** | 3 | caught |
| M2 | D2 | `setRoving` больше не переносит `tabindex=0` | `1b3c47b4d7d4937330677b690e7d16f7` | **1** | 1 | caught |
| M3 | D3 | Tab не закрывает меню | `783c525d99850bddf2087aeae9cb40f5` | **1** | 1 | caught |
| M4 | D4 | `catch` пишет «ID скопирован» | `6ef41fee42be9bf7399d718567f06326` | **1** | 1 | caught |
| M5 | D6 | приоритет причин перевёрнут | `c9a77ba82187ad31f1eb39a074e5419e` | **1** | 2 | caught |
| M6 | D7 | кнопки не блокируются на 429 | `0c9db71d1e48c50310240f357c6fd648` | **1** | 1 | caught |

Ловящие тесты:

- M1c → `D1: клик по каждому disabled-пункту ничего не меняет` (`:183`),
  `D1: capability=true не включает invite до G-06` (`:225`),
  `T-05-08: false/true/absent → invite всегда disabled` (`:641`);
- M2 → `D2: ↑/↓/Home/End: tabindex=0 ровно у сфокусированного enabled` (`:246`);
- M3 → `D3: Tab закрывает меню, фокус на opener, default не отменяется` (`:303`);
- M4 → `D4: reject writeText → ошибка, ложного успеха нет` (`:373`);
- M5 → `D6: приоритет причин: owner > capability-unknown > not-implemented` (`:449`)
  и `D1: capability=true …` (`:225`);
- M6 → `D7: 3 с → тик каждую секунду до 0 …` (`:559`).

Подробности, первые строки `AssertionError` и пути логов — в `mutations.log`.

## 3. M1: честный miss, а не «подогнанный» результат

Задание предполагало exit 1 на каждой из шести мутаций. По M1 факт другой: **exit 0,
27 pass**. Не стал это прятать и не стал менять мутацию до «удобного» результата.
Причина: в D1 три независимых слоя защиты.

1. У 7 статичных disabled-пунктов нет `data-action`, поэтому слушателя клика у них нет
   вообще (`SERVER_MENU_SPEC`, строки 49–56).
2. `guild-invite` и `guild-leave` проверяют `isEntryDisabled(node)` в самом обработчике
   (строки 856, 863).
3. Enter/Space идут через `enabledEntries()`, куда disabled-пункт не попадает (строка 960).

Поэтому M1 (только диспетчер) и M1b (только обработчик) по отдельности проходят, а M1c
(оба сразу) даёт 3 падения. Инвариант D1 тестом покрыт, но диспетчерский guard —
defense-in-depth: отдельного assert'а, который падал бы именно от него, нет.

Решение за Team Lead, по `TEST-POLICY.md` §3.6 («missed — карточка»):

- либо зафиксировать guard как дублирующий слой и закрыть P1 на M1c;
- либо завести отдельную карточку/строку теста на диспетчерский путь — это правка
  `server-menu.test.mjs`, её делает TL, не я.

## 4. Восстановление файла

```
до мутаций:   4effc2bec384b30ba074bb26933a6863  rust/gui-spike/ui/features/actions.js
после restore:4effc2bec384b30ba074bb26933a6863  rust/gui-spike/ui/features/actions.js
cmp /tmp/opencode/g05/actions.js.orig <файл> → идентичны (побайтово)
grep "M[1-6]" rust/gui-spike/ui/features/actions.js → следов мутаций нет
```

Финальный прогон: `node --test tests/unit/server-menu.test.mjs` → **exit 0, 27 pass /
0 fail**, лог `mutations/RESTORE-green.log`. Baseline до мутаций — тот же результат
(27/27, exit 0), прогон в `/tmp/opencode/g05-baseline.log`.

## 5. Файлы и md5

Новые артефакты (все — `waves/RW-40/PM/g05-rest/`):

```
ae44e9bf0b50e8cce75ea4a9d236d175  mutations/M1b.log
f05282b9c66a5975aff048e515d1bee9  mutations/M1c.log
cfe627f150356499c35ece9fad36f085  mutations/M1.log
10c3d81ae46117c35ac8a5c9188c1be8  mutations/M2.log
3cf0f81e43c1b9e4afa91c2de51d378b  mutations/M3.log
96e8e58d6d7229e507a4613a0cdc391a  mutations/M4.log
c51a8700ee7609aa128bacd3a0c25b20  mutations/M5.log
2ebc6fc6fdf8e8c2e15dcd30adcade3f  mutations/M6.log
01c41be79171ff66423dcba91d2af050  mutations/RESTORE-green.log
```

Продуктовый файл после работы — без изменений относительно отчёта карточки:

```
4effc2bec384b30ba074bb26933a6863  rust/gui-spike/ui/features/actions.js
```

Не трогал: `tests/**`, `components/server-menu.css`, `index.html`, `app.js`, `src/**`,
`TDD-PLAN.md`, `TASKS.md`, `waves/db.jsonl`, `waves/STATE.md`. Git commit не делал.

## 6. Риски и границы

1. **M1/M1b missed** — единственный содержательный результат, требующий решения TL
   (см. §3). Для приёмки P1 это не блокер: M1c доказывает, что D1 ловится.
2. **D9 не мутировался.** Проверки D9/D5 — статические assert'ы по CSS, а карточка
   разрешает мутировать только `actions.js`. Если ревью хочет mutation evidence по
   геометрии, это отдельный прогон по `server-menu.css` (например, 44 → 40 px,
   разделитель 8 → 4) — вне текущего ТЗ.
3. **Прогон только targeted.** Полный `gate:worker` не запускал: по ТЗ карточки —
   `node --test tests/unit/server-menu.test.mjs`; cargo, сеть, окно, live не трогал.
4. **Флаки.** Все восемь прогонов уложились в 300–400 мс каждый, повторов из-за
   таймеров/флаки не было; мутанты M2–M6 падали на первом же assert'е по заявленной
   причине, не по синтаксису.
