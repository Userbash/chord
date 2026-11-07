# REVIEW-195A — Техническое ревью PR-166 (`task-286` / `FE-FIX-08`: `dispatch-model.js` + `dispatch_ui.test.mjs`)

- **Ревьюер:** Технический ревьюер (DeepSeek), TR-195A
- **Проверяемая ветка:** `bugfix/task-286-dispatch-ui-forwarded-kinds-fix` (база `unstable`, HEAD `e2d9ae1`)
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED** — `P0 = 0`, `P1 = 0`, мутации `5/5` убиты (100%), оба файла восстановлены (MD5 совпадают с `REPORT.md`)

## 1. Объект ревью и целостность файлов

| Файл | MD5 в `REPORT.md` | MD5 до проверки | MD5 после цикла мутаций |
| --- | --- | --- | --- |
| `rust/gui-spike/ui/features/dispatch-model.js` | `9c7439d1db386a89fbd2347a95a207a2` | совпал ✅ | `9c7439d1db386a89fbd2347a95a207a2` ✅ |
| `tests/unit/dispatch_ui.test.mjs` | `ac92c85adfbf146d812921a176a19ced` | совпал ✅ | `ac92c85adfbf146d812921a176a19ced` ✅ |

Перед каждой мутацией `dispatch-model.js` копировался в `/tmp/opencode/tr195a/dispatch-model.js.orig`; после каждой — восстанавливался из бэкапа с проверкой MD5 внутри харнесса (`mutations/mutate-195a.sh`, `restore()` возвращает код `3`, если MD5 не сошёлся). Для red-прогонов отдельно снимались и возвращались обе версии `tests/unit/dispatch_ui.test.mjs`. Финальная сверка: `9c7439d1…` / `ac92c85a…`.

## 2. Состав изменений (`git status --porcelain`)

```
 D node_modules
 M package-lock.json
 M rust/gui-spike/ui/features/dispatch-model.js
 M tests/unit/dispatch_ui.test.mjs
```

Два `M` — предмет PR-166. `D node_modules` (симлинк `120000` развёрнут в каталог) и `M package-lock.json` (в корневую запись добавлены `bin`/`engines`, версии зависимостей не менялись) — не часть PR, см. `P2-2`.

Продуктовый диф — ровно один хунк: `FORWARDED_KINDS` и комментарий к нему. Логика модели (`FORWARDED.has`, `switch`, счётчики) не менялась. В тесте: обновление шапки, два новых оракула (`extendedWireNames`, `forwardedOracle`), усиленный `A2`, обновлённые негативные списки `A5`/`B14`, новый `B14b`. Число тестов выросло `57 → 58`.

## 3. Правка верна по существу

Оракул проверен собственным скриптом (не хелперами теста) — разбор реальных артефактов:

```
dispatch.rs: impl DispatchKind::as_str   -> 21 имя (уникальных 21)
dispatch.rs: ExtendedDispatchKind::NAMES -> 8 имён, impl-блок строка 165 (первый pub const NAMES в файле)
net.rs:      FORWARDED_DISPATCHES        -> 30 имён, уникальных 30
UI:          FORWARDED_KINDS             -> 30 имён, уникальных 30
наборы UI == net.rs == (as_str + NAMES + MESSAGE_REACTION_REMOVE): true
порядок UI == порядок net.rs: true
относительный порядок 21 канонического имени == порядок as_str: true
```

То есть заявленный корень в `REPORT.md` подтверждается: бэкенд с task-156 форвардит 30 имён, UI держал 22 и молча выбрасывал 8 расширенных событий (`MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`, `CHANNEL_CREATE/UPDATE/DELETE`, `THREAD_CREATE/UPDATE/DELETE`). Правилась сторона UI, `net.rs` (G1, вне write-scope) не тронут — это верное разделение ответственности.

Проверка «товар лицом», не по отчёту:

| Проверка | Результат |
| --- | --- |
| `dispatch-model.js` из HEAD + тест этой ветки | `58 / 56 pass / 2 fail`: `A2: в UI не 30 kind`, `B14b: MESSAGE_DELETE форвардится бэкендом, но отбрасывается UI` (`mutations/red-before-pr.log`) |
| `dispatch-model.js` из HEAD + тест из HEAD | `57 / 56 pass / 1 fail`: `A2: CHANNEL_CREATE форвардится бэкендом, но отбрасывается UI` — совпадает с `red.log` 1/2 |
| Негативные списки `A5` / `B14` (`MESSAGE_DELETE_EXTRA`, `GUILD_CREATE`, `message_create`, `typed`, `CHANNEL_PINS_UPDATE`, `X`, `""`) | все вне оракула — проверки не тавтологичны и не конфликтуют с расширением |

Про «ослабление теста» речи нет: `A2` стал строже. Было — длина `22` и одностороннее вхождение (`backend ⊂ UI`), причём арифметика `22` противоречила требованию надмножества для 30 имён (старый тест был самопротиворечив, падал на любой реализации). Стало — точное равенство множеств `UI == ядро`, равенство `net.rs == ядро ядра`, длина и отсутствие дублей с обеих сторон. `B14b` добавляет двустороннюю проверку приёма 8 расширенных событий (`ok=true`, `ignored=0`, `skipped=1`, `changes=[]`, `malformed=0`).

## 4. Проверки

| Проверка | Результат | Лог |
| --- | --- | --- |
| `node --test tests/unit/dispatch_ui.test.mjs` | **58/58 pass**, 0 fail, 0 skip | `mutations/baseline.log` |
| `npx tsc -p tsconfig.json --noEmit` | exit `0`, 0 ошибок; в программе 183 `.mjs`, включая оба проверяемых файла (`--listFiles`) | `mutations/baseline.log` |
| Регрессионный срез фронтенд-контракта (10 файлов: `gui-spike-{dom,model,bench,features,auth,regressions}`, `guild-rail`, `voice-ui`, `channel-switch`, `dispatch_ui`) | **458/458 pass**, 0 fail, 0 skip | прогон ревьюера |
| `tests/meta/plan-sync.test.mjs` | падает независимо от PR: `тестовые файлы без строки в §8` перечисляет десятки чужих `tests/unit/*` (`PLAN.md` в репозитории не отслеживается) | прогон ревьюера |

`tsc` важен именно с `--listFiles`: `dispatch-model.js` и `dispatch_ui.test.mjs` реально попадают в программу (первый — как импорт теста, второй — по `include: tests/**/*.mjs`), поэтому «0 errors» относится к правленому коду, а не к пустому множеству.

## 5. Мутации

Методика: `mutations/mutate-195a.sh` + appliers `mutations/applier-*.mjs` — точная замена одной конструкции в `dispatch-model.js`, прогон `node --test tests/unit/dispatch_ui.test.mjs`, откат из бэкапа с проверкой MD5. Логи по мутантам: `mutations/M1.log … M5.log`, сводка — `mutations/results.json`, полный вывод — `mutations/run.log`.

| # | Мутация | Итог | Убивший тест | fail |
| --- | --- | --- | --- | --- |
| M1 | удалён `CHANNEL_CREATE` из списка | **убит** | `A2` (`в UI не 30 kind`), `B14b` | 2 |
| M2 | `MESSAGE_DELETE` → `MESSAGE_DELETE_EXTRA` (ложное имя провода) | **убит** | `A2`, `A5`, `B14`, `B14b` | 4 |
| M3 | удалён `MESSAGE_REACTION_REMOVE` (raw kind) | **убит** | `A2`, `A4`, `B11`, `B13`, `C5–C7`, `C5b` | 6 |
| M4 | дубль `TYPING_START` (31 запись, 30 уникальных) | **убит** | `A2` | 1 |
| M5 | `THREAD_CREATE` → `THREAD_CREATED` (опечатка) | **убит** | `A2`, `A5`, `B14b` | 3 |

**Итого `5/5` (100%), обязательный минимум закрыт.** После цикла `dispatch-model.js` = `9c7439d1db386a89fbd2347a95a207a2`.

Дополнительные пробы (не входят в 5/5):

- `P1` — те же 30 имён, но `MESSAGE_REACTION_REMOVE` перенесён в начало списка: тесты **выжили** (`58/58`). Это не дефект правки: фильтр работает по `Set`, порядок не влияет на поведение. Но комментарий в коде обещает «порядок и состав — как в `net::FORWARDED_DISPATCHES`», а тест пинит только состав. См. `P3-1`.
- `P2` — обход фильтра (`if (!FORWARDED.has(kind))` → `if (false && …)`): **убит** (`B14`, `C10`, 2 fail). Значит, расширение списка не сломало сам механизм отбраковки.

## 6. Замечания

Блокирующих нет. Всё ниже — `P2`/`P3`, каждая правка вне write-scope этой карточки.

### P2-1. `CONTRACTS/dispatch.md` §4 противоречит новому оракулу
Документ по-прежнему заявляет «`net::FORWARDED_DISPATCHES` (6 имён)» и «остальные 15 типов **не** форвардятся», хотя `net.rs` держит 30 имён с task-156. Расхождение старое (не от этого PR), но теперь тест-оракул прямо противоречит контракт-документу: следующий воркер, читающий §4, может «починить» UI обратно до 22. **Рекомендация:** отдельная docs-карточка G6/Team Lead на §4–§5 `CONTRACTS/dispatch.md` (таблица 30 имён + пометка, что у 8 расширенных нет проекций).

### P2-2. Шум в рабочем дереве, не входящий в PR
`D node_modules`, `M package-lock.json` (только `bin`/`engines` в корневой записи). Полный `npm test` / `cargo test --workspace` из-за этого не запускался — гейт поднял уровень до `T2`; автор ограничился адресным срезом, что для `L1`-правки приемлемо. **Рекомендация:** merge только двух путей PR-166, lock чинить отдельной карточкой (как в `REVIEW-194A` `P2-3`).

### P3-1. Комментарий обещает порядок, тест его не проверяет
`A2` сравнивает `Set`, поэтому перестановка имён выживает (`P1`). Поведению это не вредит, но утверждение «порядок как в `net.rs`» в комментарии `FORWARDED_KINDS` ничем не закреплено. **Рекомендация:** либо добавить в `A2` `assert.deepEqual(FORWARDED_KINDS, backend)` (модель уже совпадает с `net.rs` поэлементно), либо снять из комментария слово «порядок». Первое дешевле — одна строка.

### P3-2. `extendedWireNames()` не ограничен своим impl-блоком
Регулярка ищет первый `pub const NAMES` по всему `dispatch.rs`; сейчас это `ExtendedDispatchKind::NAMES` (строка 165), но если выше появится `DispatchKind::NAMES`, оракул молча возьмёт чужие 21 имя. Падение будет громким (`assert.equal(extended.length, 8)`), однако текст ошибки уведёт не туда. **Рекомендация:** обрамить как `coreWireNames()` — `/\nimpl\s+ExtendedDispatchKind\s*\{([\s\S]*?)\n\}/`, затем искать `NAMES` внутри.

### P3-3. Удалённые сообщения теперь доходят до модели, но не исчезают из DOM
`MESSAGE_DELETE*`, `CHANNEL_*`, `THREAD_*` принимаются как `skipped` — сообщение после удаления остаётся в списке. Регрессии нет (раньше событие молча отбрасывалось, DOM вёл себя так же), но пользовательский сценарий «удалили сообщение — оно пропало» по-прежнему не закрыт. Это заявленный остаток карточки (`REPORT.md` §«Риски», `ACCEPT.md`); **рекомендация PM** — завести отдельную карточку на проекции жизненного цикла.

## 7. Соответствие DoD

| Критерий | Статус |
| --- | --- |
| MD5 файлов совпадают с `REPORT.md` | ✅ оба, до и после мутаций |
| Правка только в двух разрешённых файлах | ✅ (`D node_modules`, `M package-lock.json` — чужой шум, не PR) |
| `node --test tests/unit/dispatch_ui.test.mjs` = 58/58 | ✅, 0 skip |
| `npx tsc -p tsconfig.json --noEmit` = 0 ошибок | ✅, оба файла в программе (`--listFiles`) |
| Проверка не ослаблена | ✅ `A2` усилен до двустороннего равенства, добавлен `B14b`, тестов `57 → 58` |
| Мутации | ✅ `5/5` убиты, файл восстановлен |
| Регрессия | ✅ срез 458/458; `plan-sync` красный предсуществующе |
| `git commit/push` не делался | ✅ |
| `P0 = 0`, `P1 = 0` | ✅ |

## 8. Артефакты

```
waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/REVIEW-195A.md             — этот отчёт
waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/mutations/mutate-195a.sh   — харнесс мутаций с откатом и проверкой MD5
waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/mutations/applier-*.mjs    — 7 точечных замен (M1–M5, P1, P2)
waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/mutations/run.log          — полный прогон: diff + тесты + вердикт по каждому мутанту
waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/mutations/M1.log … M5.log  — логи обязательных мутантов
waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/mutations/P1-order.log     — проба порядка (выжила)
waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/mutations/P2-filter.log    — проба обхода фильтра (убита)
waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/mutations/results.json     — сводка, MD5, вердикты
waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/mutations/baseline.log     — чистый прогон: 58/58 + tsc exit 0
waves/RW-40/G2/task-286-dispatch-ui-forwarded-kinds-fix/mutations/red-before-pr.log — HEAD-версия модели: 2 fail
```

**Вердикт: APPROVED.** `P2-1…P2-2` и `P3-1…P3-3` — рекомендации на отдельные карточки, слияние в `unstable` ими не блокируется. Следующий статус — `Ready for Merge`; merge двух файлов и запись события в `waves/db.jsonl` выполняет координатор.
