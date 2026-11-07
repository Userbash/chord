# G05-REST-MUTATIONS-FIX — pass 3: guard D1 больше не дублируется

Дата: 2026-10-30. Воркер: **G2 Frontend Lane C**. Карточка: **G05-REST-MUTATIONS-FIX**.
Основание: `waves/RW-40/PM/g05-rest/REVIEW-PASS2.md`, P1 — M1 и M1b по отдельности
проходили (exit 0), потому что клик по disabled-пункту защищали два независимых guard'а.

## 1. Что изменено

Продуктовый файл `rust/gui-spike/ui/features/actions.js`:

- удалены guard-проверки из обработчиков `guild-invite` и `guild-leave`
  (бывшие строки 856 и 863);
- в клик-диспетчере вместо прямой проверки атрибута теперь единый предикат
  `isEntryDisabled(node)` (строка 932). Тот же предикат уже используют
  `enabledEntries()` и `setRoving()`.

Итог: единственная точка проверки `aria-disabled` для клика. Снятие этой проверки
не маскируется вторым слоем и падает тестом.

Тестовый файл `tests/unit/server-menu.test.mjs`:

- новый тест `guard диспетчера гасит disabled data-action вне меню` (`:228`)
  кликает по `[data-action="toggle-tree"]` с `aria-disabled="true"` и требует,
  чтобы обработчик не выполнился. У этого действия нет собственного guard'а,
  поэтому тест бьёт ровно в диспетчер;
- в тесте `Enter/Space на disabled-пункте` (`:210`) добавлена проверка
  `event.prevented === false`: disabled-пункт не должен присваивать клавишу
  активации и полагаться на последующий guard клика.

Поведение продукта не изменилось: disabled-элемент по-прежнему полный no-op,
клавиатурный вход в disabled-пункт по-прежнему не активирует его.

## 2. RED

RED снят на pre-fix продукте с одиночной мутацией M1 (guard диспетчера → `if (false)`).
Это доказывает, что новый тест закрывает именно пропущенный путь P1, а не просто
дублирует существующие assert'ы.

```
$ node --test --test-name-pattern='guard диспетчера' tests/unit/server-menu.test.mjs
pre-fix md5: 4effc2bec384b30ba074bb26933a6863
mutant  md5: 799f869e7b1ac36fb55d07edf1c00d7f
→ exit 1, 0 pass / 1 fail
→ AssertionError: disabled data-action выполнил обработчик — guard диспетчера не сработал
```

Полный лог: `waves/RW-40/PM/g05-rest/red-pass3.log` (exit=1).

## 3. GREEN

```
$ npx tsc -p tsconfig.json                 → exit 0
$ node --test tests/unit/server-menu.test.mjs
→ exit 0, 28 passed / 0 failed (12 suites)
```

Полный лог: `waves/RW-40/PM/g05-rest/green-pass3.log`.

## 4. Мутации M1–M5: 5/5 KILLED

Все мутации — одиночные, по одной на прогон, на финальном продукте
(md5 `e7ba88e8c347012340f0e7e448d71b68`). Оракул между мутантами не менялся.

| ID | Мутация | md5 мутанта | exit | pass/fail | Вердикт |
|---|---|---|---|---|---|
| M1 | `isEntryDisabled` всегда `false` | `83986478f35420a41786dcaf467deb0b` | 1 | 22/6 | **KILLED** |
| M2 | guard диспетчера → `if (false) return;` | `2b8bcfbdb4f04db9c322d2a9f3322c14` | 1 | 24/4 | **KILLED** |
| M3 | `enabledEntries()` без фильтра disabled | `cb6c96f1dbe3ae7ab8da8e6164a11226` | 1 | 26/2 | **KILLED** |
| M4 | Enter/Space без `enabledEntries().includes(active)` | `a21109199d9d079c85503a65f85b53b4` | 1 | 27/1 | **KILLED** |
| M5 | причина invite → `""` (invite становится enabled) | `3564e1be047bfd26961bfee6a5451c9a` | 1 | 21/7 | **KILLED** |

Первые падения по каждому мутанту:

- **M1** — `disabled-пункт закрыл меню` (`:197`); также падают новый тест диспетчера,
  D2 roving и тесты invite;
- **M2** — `disabled-пункт закрыл меню` (`:197`) и новый тест
  `guard диспетчера гасит disabled data-action вне меню` (`:228`);
- **M3** — `disabled invite перехватил Enter` (`:220`) и D2
  `tabindex=0 ровно у сфокусированного enabled`;
- **M4** — `disabled invite перехватил Enter` (`:220`);
- **M5** — `disabled-пунктов 7, а не 8` (`:191`), плюс причины D6 и invite-тесты.

Полный лог с restore-контролем: `waves/RW-40/PM/g05-rest/mutations-pass3.log`.

## 5. Восстановление

```
final md5:    e7ba88e8c347012340f0e7e448d71b68  rust/gui-spike/ui/features/actions.js
restore md5:  e7ba88e8c347012340f0e7e448d71b68  (совпал после каждой из 5 мутаций)
```

Тест после прогона мутаций не менялся:
`6e20b67c93fc348c835bd8bb193a7414  tests/unit/server-menu.test.mjs`.

## 6. Файлы и md5

Продукт и тест:

```
e7ba88e8c347012340f0e7e448d71b68  rust/gui-spike/ui/features/actions.js
6e20b67c93fc348c835bd8bb193a7414  tests/unit/server-menu.test.mjs
```

Артефакты (`waves/RW-40/PM/g05-rest/`):

```
88dfdecbef04a4482fd291623a187e5b  red-pass3.log
c2f0efc8257691d32c401c28ff662795  green-pass3.log
1803f7cef5288b856c42f859298bcffc  mutations-pass3.log
9ecd10c07c1a6b79f7d789663c64a36c  unit-pass3.log
3647ef8fa30c8657a75008d6098bc1d2  unit-pristine-guild-rail-pass3.log
```

Не трогал: `server-menu.css`, `index.html`, `app.js`, `src/**`, `TDD-PLAN.md`,
`TASKS.md`, `waves/db.jsonl`, `waves/STATE.md`, чужие тесты. Git commit не делал.

## 7. Границы и внешний красный тест

1. Карточка закрыта targeted-прогоном: `node --test tests/unit/server-menu.test.mjs`
   (28/28) + `tsc` (exit 0). Полный `gate:worker` не запускал; cargo, сеть,
   окно и live не трогал.
2. **Полный `npm run test:unit` сейчас красный, и это не следствие правки.**
   Падает один тест в `tests/unit/guild-rail.test.mjs`:
   `быстрые клики серверов A -> B …` (`:545`), сохранён `lastGuildId` плитки A
   вместо B. Остальные 824 теста проходят. Тот же тест падает и на pristine-копии
   файлов pass 2 (без dedup и без нового теста) — лог
   `unit-pristine-guild-rail-pass3.log`. Мой диф этих файлов не касается;
   чужой тест не правил.
   Логи: `unit-pass3.log` (exit 1, 824 pass / 1 fail),
   `unit-pristine-guild-rail-pass3.log` (exit 1, 12 pass / 1 fail на чистом pass 2).
3. Удаление handler-guard'ов не ослабляет D1: клик целиком закрыт диспетчером,
   клавиатура — фильтром `enabledEntries()`; оба пути теперь проверяются
   отдельными мутантами (M2, M3/M4).
4. Мутанты M2–M6 прошлого прогона (D2–D7) не перепрогонялись: их код в этой
   правке не менялся, evidence остаётся в `mutations.log` и
   `REPORT-MUTATIONS.md`.
5. Новый тест диспетчера использует `toggle-tree` как нейтральный `data-action`
   без доменного guard'а. Это проверка общего контракта диспетчера, а не
   поведение меню сервера.
