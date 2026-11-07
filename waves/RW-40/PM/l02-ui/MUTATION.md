# L02-UI-MUTATION — мутационная проверка dispatch-UI

- **Карточка:** `L02-UI-MUTATION`, отряд G2 Frontend, batch RW-40/PM.
- **Дата отчёта:** 2026-10-30. **HEAD:** `4e240481e59ff4d49732ac8bd5ba08a006363748`.
- **Объект мутаций:** `rust/gui-spike/ui/features/dispatch-model.js`, `rust/gui-spike/ui/app.js`.
- **Оракул:** `tests/unit/dispatch_ui.test.mjs` (43 кейса, 262 `assert`).
- **Что запускалось:** только `node --test` против копии дерева в `/tmp`. `cargo`, `gate`, e2e, live/окно/Discord, сеть и `npm run *` не запускались. Продукт, тесты и чужие файлы не менялись; мутации жили только в копии.

## 1. Актуальное состояние объекта

| Файл | md5 | Совпадает с `md5-before` всех мутантов |
|---|---|---|
| `rust/gui-spike/ui/features/dispatch-model.js` | `37dacc61d7daf74c144681637dded1f5` | да |
| `rust/gui-spike/ui/app.js` | `1ac5c36976a064120529c132e61cfb54` | да |
| `tests/unit/dispatch_ui.test.mjs` | `d30771d65b6373ecbbac6de1f9e61a3d` | нет, см. §5.1 |

Baseline на текущем дереве (`node --test tests/unit/dispatch_ui.test.mjs`):
`tests 43 / pass 43 / fail 0 / skipped 0`, 4 сьюта, ~0.33 s. Лог прогона —
`waves/RW-40/PM/l02-ui/logs/mutation-baseline-product.log` (тот же результат
до и после восстановления копии — `mutation-baseline-post-restore.log`).

## 2. Стенд

`waves/RW-40/PM/l02-ui/logs/mutation-run.sh`:

1. Копирует дерево в `/tmp/opencode/l02-ui-mutation`; продукт и тесты репозитория
   не трогаются. Для A1/A2 тест читает `rust/gui-spike/src/net.rs` — копия должна
   содержать этот файл (иначе baseline падает на `ENOENT`, см. §5.3).
2. Кладёт эталоны в `.pristine`, перед каждым мутантом восстанавливает оба файла.
3. Применяет патч через `python3` с проверкой «anchor встречается ровно один раз»;
   при `ANCHOR-COUNT != 1` мутант не засчитывается.
4. Запускает `node --test --test-reporter=spec tests/unit/dispatch_ui.test.mjs`,
   пишет `exit`, `md5-mutated`, затем восстанавливает файлы и сверяет md5 с
   `.pristine` (`restore-ok`).
5. Мутант считается убитым, если `exit != 0` и падают именно те кейсы, которые
   заявлены в `expected-caught-by`.

Каждый лог содержит `md5-before`, `md5-mutated`, полный вывод тестов, `exit`,
`restore-ok`. Восстановление после всех пяти прогонов подтверждено md5
`37dacc61…` / `1ac5c369…`.

## 3. Мутанты

Всего 5, все убиты. Прогон шёл 2026-10-24 16:25–16:26 на тестовом хеше
`f2806f3f…` (см. §5.1).

| # | Файл | Мутация | Что ломает | Ожидаемо ловят | Фактически упало | `md5-mutated` | pass/fail |
|---|---|---|---|---|---|---|---|
| M1 | `dispatch-model.js` | ранний `return []` при пустом списке гильдий в READY-проекции | пустой READY перестаёт быть полным снапшотом: старые гильдии остаются в store и рейле | B16, B16b, C15, C15b | B16, B16b, C15, C15b | `80e3b6ca0f36bfb747b3b25ccebcdae3` | 39/4 |
| M2 | `dispatch-model.js` | `asSnowflake` → `asString` для fallback `selfUserId` из READY | unsafe integer `user.id` (вне safe range) проходит как строка с потерей точности | B17 | B17 | `931b9ff8eb53cb5f55dd3f2debab51b2` | 42/1 |
| M3 | `dispatch-model.js` | `asSnowflake` → `asString` в `setDispatchSelfUserId` | то же для авторитетного `ready()` self ID, пришедшего сеттером | B17 | B17 | `104de361d6cb8d2e14a8be78e8d47823` | 42/1 |
| M4 | `app.js` | ветка пустого `guilds` в `rail.applyGuilds` возвращается без `renderEmpty()` | рейл не очищается: после пустого READY висят плитки прошлого снапшота | C15, C15b | C15, C15b | `1acc5d9c879f0f363c214262d04d014f` (app.js) | 41/2 |
| M5 | `app.js` | `asSnowflake` → `asString` на границе `ready()` IPC | `ready()` перестаёт отвергать unsafe integer self ID | C16 | C16 | `b6951da8733c3c8cf0eabcaf582a7071` (app.js) | 42/1 |

Мутанты M1 и M4 — парные: первый ломает очистку store, второй — очистку DOM.
Поэтому пустой READY проверен с двух сторон, и оба дефекта ловятся независимо
(B16/B16b без DOM и C15/C15b через рейл). M2/M3 закрывают оба входа `selfUserId`
(данные dispatch и авторитетный `ready()`), M5 — третий, IPC-шов в `app.js`.
Ни один мутант не выжил; «эквивалентных» (неотличимых тестами) мутантов в
наборе нет — каждый ломает ровно заявленные кейсы.

Логи: `logs/preliminary-tsc-test-md5-f2806f3f/mutation-M1.log` … `mutation-M5.log`.

## 4. Покрытие и чего в нём нет

Пять мутантов бьют по дефектам P2-1/P2-2 из `FIX-REPORT.md` (пустой READY и
self ID). Это не оценка всего набора: кейсы A1–A3 (контракт с `net.rs`),
B1–B15 (проекции, дедуп, LWW, реакции, malformed), C1–C14 (живой DOM вместо
reload) и D1–D5 (транспорт bridge) мутациями не покрыты. Число «5/5» ниже
означает «все выбранные мутанты убиты», а не «тесты полные».

Ближайшие кандидаты на следующую волну, если нужен сквозной mutation score:
порядок `store.order` из `ops[0].items` (B6), приоритет имени
`global_name → nick → username → id` (B4), LWW по `ts` в PRESENCE (B10),
`±1` реакции и no-op на повтор ADD (B11/B12), `duplicate` в CHUNK (B5).

## 5. Ограничения и оговорки

### 5.1 Хеш теста в момент мутаций

Логи сняты, когда `tests/unit/dispatch_ui.test.mjs` имел md5
`f2806f3f4088b0527f6a31ff1d7bce2a`. Карточка `TSC-FIX` затем довела тест до
`d30771d65b6373ecbbac6de1f9e61a3d`. Диff — только JSDoc-типы: `assert`-строк
`262 → 262`, `it(` `43 → 43`, `.only`/`.skip` — 0 (это же зафиксировано в
`waves/RW-40/QUALITY/TSC-FIX.md` §1 и `REVIEW-PASS2.md`).

Проверка не «на слово»: `mutation-run.sh` прогнан заново на текущем дереве
(копия в `/tmp/opencode/g2b-mut`, тот же набор патчей, продукт md5
`37dacc61…` / `1ac5c369…`, тест `d30771d6…`). Результат совпал с логами
карточки полностью: те же `md5-mutated` у всех пяти мутантов, те же упавшие
кейсы, `pass/fail` 39/4, 42/1, 42/1, 41/2, 42/1, после каждого — `restore-ok`.
Свежие логи лежат вне репозитория (`/tmp/opencode/g2b-mut/logs/`), потому что
каталог `logs/` карточки не входит в файлы этого воркера; здесь приведены
дословные строки результата.

### 5.2 Мутация — не доказательство в браузере

Тесты исполняют настоящий `app.js` в DOM-шиме, который не считает layout
(`scrollHeight` = `boxHeight`). Мутанты проверяют логику данных и побочные
эффекты в DOM, но не пиксельную геометрию — для неё есть отдельный
геометрический кейс в `tests/unit/channel-paging.test.mjs`
(карточка `C-02-UI-OLDER-FIX`).

### 5.3 Неудачная попытка в `preliminary-tsc-test-md5-f2806f3f/`

`mutation-setup-missing-oracle.log` — прогон, упавший до мутаций:
`ENOENT rust/gui-spike/src/net.rs` в копии, тест A1/A2 читает этот файл как
оракул. К результатам не относится и в счёт не идёт; `mutation-baseline-temp.log`
в той же папке — baseline копии 43/43 после того, как оракул появился.

## 6. Воспроизведение

```bash
# baseline на дереве репозитория
node --test tests/unit/dispatch_ui.test.mjs

# мутации (копия в /tmp; продукт репозитория не меняется)
bash waves/RW-40/PM/l02-ui/logs/mutation-run.sh
```

Ожидаемый итог: baseline `43/43`, затем пять строк
`---- killed? expected: …; exit=1` и `restore-ok` после каждой.

## 7. Вывод

- Отчётный набор: 5 мутантов, 5 убито, 0 выжило, 0 ошибок применения патча.
- Оба продуктовых файла на текущем HEAD совпадают с `md5-before` логов,
  восстановление после мутаций проверено md5.
- Тестовый файл изменился только типами; повторный прогон на текущем хеше
  дал те же результаты.
- Ограничение по объёму: покрыты дефекты пустого READY и self ID, остальные
  требования A/B/C/D мутациями не проверялись (см. §4).
