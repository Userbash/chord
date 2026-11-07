# Независимое ревью — task-125-test-impact-v2 (Review 44A)

Дата: **2026-10-30, 20:56–21:06 +05**. Ревьюер: `DeepSeek V4.1 Flash` (`Review 44A`), независимая сессия.
Репозиторий: `/home/sanya/orca/workspaces/project/qa-task-125-test-impact-v2`, HEAD `4e240481e59ff4d49732ac8bd5ba08a006363748`
(`RW-40: integrate Discord GUI spike, dispatch, paging and QA evidence`), ветка `qa/task-125-test-impact-v2`.

Ревьюируемые артефакты (MD5 зафиксированы на момент проверки; любая правка инвалидирует ревью):

| Артефакт | MD5 |
|---|---|
| `scripts/test-impact.mjs` (220 строк) | `b60689e8cfdf7d068c1af2e6ff584662` |
| `tests/unit/test-impact.test.mjs` (268 строк) | `7e876c80b90f4e5fa1ce412b5578c9eb` |
| `waves/RW-40/G4/task-125-test-impact-v2/REPORT.md` (58 строк) | `6b458ffeafa2979a93c88769e2a2dabd` |
| `red.log` / `green.log` | `9c519974ae0496625481e298fff70a32` / `bd3a25d70f019f61fb156edbd6240008` |

Оракул: `waves/RW-40/QUALITY/TEST-STRATEGY-V2.md` §5.2.1–§5.2.3 (стр. 213–215) и §8.1 (стр. 276).
Продуктовый код, тесты и отчёт ревьюер не менял. Созданы только этот файл и `evidence/` рядом с ним.

## Вердикт

**RETURN.** P0 = 0, **P1 = 2**, P2 = 2, P3 = 2.

Заявленный red-first честный: падение воспроизведено независимо (19 тестов, 11 pass, 8 fail), зелёный
прогон 36/36 подтверждён, MD5 из отчёта совпали. Но сам маппинг не работает на реальных путях
репозитория: все семь ключей указывают на `rust/gui-spike/ui/<mod>.js`, тогда как модули RW-40 лежат в
`rust/gui-spike/ui/features/<mod>.js`, а двух «модулей» (`attachments-render.js`, `chat-reactions.js`)
не существует ни в одной ветке. Плюс полностью отсутствует вторая половина карточки — маппинг Rust BFF
(`§5.2.2`), прямо названный в `§8.1` вместе с UI-модулями. Тест написан на те же фантомные пути, поэтому
зелёный прогон подтверждает только самосогласованность патча, а не пользу для T0.

## 1. Запрошенные прогоны

| Команда | Результат | Лог |
|---|---|---|
| `node --test tests/unit/test-impact.test.mjs tests/meta/test-policy.test.mjs` | **exit 0**, tests 36, pass 36, fail 0 (19 + 17) | `evidence/targeted.log` |
| `npm run typecheck` | **exit 1**, 16 ошибок `TS7006/TS2339/TS2345`, все в `tests/unit/dispatch_ui.test.mjs`; в файлах карточки ошибок нет | `evidence/typecheck.log` |
| Независимый red (селектор из `HEAD` + новый тест) | **exit 1**, tests 19, pass 11, fail 8 — совпало с `red.log` | `evidence/red-repro-head-selector.log` |

`npm run typecheck` на неизменённом `HEAD` падает из-за `tests/unit/dispatch_ui.test.mjs` (в рабочем
дереве координатора `unstable` этот файл уже пропатчен, `npm run typecheck` там exit 0; `TDD-PLAN.md §8`
там тоже дополнен, `plan-sync` 5/5). То есть красный гейт карточки — внешний, карточкой не создан;
`REPORT.md` описывает его корректно. Подробнее — §4.

## 2. Находки

### P1-1. Маппинг указывает на несуществующие пути — на реальных модулях правило не срабатывает

`scripts/test-impact.mjs:33-67`: ключи `UI_MODULE_TESTS` — `rust/gui-spike/ui/notifications.js`,
`.../voice-devices-settings.js`, `.../dm-friends.js`, `.../search.js`, `.../search-overlay.js`,
`.../attachments-render.js`, `.../chat-reactions.js`. В репозитории таких файлов нет:

- `git ls-files | grep -E "ui/(notifications|voice-devices-settings|dm-friends|search|search-overlay|attachments-render|chat-reactions)\.js$"` → **пусто**;
- на этой ветке в `rust/gui-spike/ui/` только `app.js` и `model.js`, реальные модули — в `features/`:
  `features/search.js`, `features/search-overlay.js` (в этой ветке), `features/notifications.js`,
  `features/voice-devices-settings.js`, `features/dm-friends.js` (в `unstable`/`beta` и на ветках
  `feature/task-112…`, `feature/task-113…`);
- `BOARD.md:57` прямо фиксирует путь карточки task-113: `rust/gui-spike/ui/features/dm-friends.js`.

Следствие: для реального файла правило не находится, и `planFor` уходит в общий
`under(raw, "rust/gui-spike/ui/")` → полный `FRONTEND` (9 файлов) + `geometry-check.mjs`. Проверено
командой:

```
$ node scripts/test-impact.mjs --changed rust/gui-spike/ui/features/notifications.js
  $ npm run typecheck
  $ node --test tests/unit/gui-spike-dom.test.mjs … tests/unit/channel-switch.test.mjs   # весь FRONTEND
  $ node waves/RW-39/PM/w39c/geometry-check.mjs
```

То же для `features/voice-devices-settings.js`, `features/dm-friends.js`, `features/search.js`,
`features/search-overlay.js` (полный лог — `evidence/impact-probe.log`). Цель карточки («T0 за секунды
по затронутому модулю») не достигается; единственное, что теперь зелено, — тест на фантомные пути.

Отдельно про `attachments-render.js` и `chat-reactions.js`: таких модулей нет ни в одной ветке.
Профильные тесты существуют и тестируют другие файлы: `attachments-render.test.mjs:25` импортирует
`rust/gui-spike/ui/features/chat.js`, `chat-reactions.test.mjs:31` — `features/reactions.js`
(проверено в `unstable`). Значит, для них нужны правила на `features/chat.js` (добавить
`attachments-render.test.mjs` к существующей ветке chat.js) и `features/reactions.js` (новое правило с
`chat-reactions.test.mjs`), а не фантомные ключи.

Тест закрепляет ту же ошибку: `tests/unit/test-impact.test.mjs:144-194` переписывает таблицу путей из
реализации, а не из репозитория/BOARD, поэтому красная фаза ловит только отсутствие правила, но не
проверяет, что путь существует. Оракул в этой части не независим (TEST-POLICY: ожидания сверяются с
внешним источником, а не с текущей реализацией).

**Исправить:** ключи → `rust/gui-spike/ui/features/<mod>.js`; для search/search-overlay это существующие
файлы; для attachments/chat-reactions — правила на `features/chat.js` и `features/reactions.js`;
в тест добавить проверку существования каждого модуля (например, `existsSync` по пути из `BOARD.md`)
и кейс «изменение `features/<mod>.js` даёт ровно профильный набор».

### P1-2. Маппинг Rust BFF (`§5.2.2`) не реализован, хотя входит в task-125

`TEST-STRATEGY-V2.md:276` перечисляет в задаче task-125 «…и Rust-модулей `rust/gui-spike/src/<mod>.rs`»;
`§5.2.2` (стр. 214) требует для каждого `src/<mod>.rs` таргеты `--test <mod>_contract` +
`--test ipc_wiring_contract`. В патче этого нет: `scripts/test-impact.mjs:156-158` по-прежнему даёт один
общий прогон крейта:

```
$ node scripts/test-impact.mjs --changed rust/gui-spike/src/data.rs
  $ npm run typecheck
  $ cargo test --offline --manifest-path rust/gui-spike/Cargo.toml     # весь gui-spike
```

Ожидалось `cargo test … --test data_contract --test ipc_wiring_contract`. В `tests/unit/test-impact.test.mjs`
нет ни одного кейса на `rust/gui-spike/src/*.rs`, в `REPORT.md` §«Сделано» Rust не упомянут, оракулом
заявлен только `§5.2.1`. Это не «не проверено», а незакрытая часть ТЗ.

**Исправить:** таблица `src/<mod>.rs → <mod>_contract` (с исключениями: `settings_store.rs →
settings_contract`, `lib.rs`/`main.rs` → полный прогон по `§5.2.3`; `ipc_wiring_contract` добавлять
всегда), red-first тесты на 3–4 модуля и строку в отчёте. Если Rust-часть решено вынести в отдельную
карточку — это нужно письменно зафиксировать в стратегии/ТЗ и в отчёте, а не молча сузить оракул.

### P2-1. Для `attachments-render` и `chat-reactions` выпал общий `gui-spike-features.test.mjs`

`§5.2.1` требует «профильный тест **плюс** общие интеграционные тесты `gui-spike-dom.test.mjs` и
`gui-spike-features.test.mjs`». В таблице `scripts/test-impact.mjs:59-66` и в тесте
(`test-impact.test.mjs:186-192`) у двух модулей оставлен только `gui-spike-dom`. Это не косметика:
`gui-spike-features.test.mjs` рендерит строки чата и реакции (`:1027-1052`, `:1174-1213`, импорт
`features/reactions.js` на `:1477`) и может упасть от правки `features/reactions.js`/`chat.js`.
Тест опять фиксирует более слабое ожидание, чем стратегия. Вернуть `gui-spike-features` обоим (после
исправления P1-1 — для `features/chat.js` и `features/reactions.js`).

### P2-2. Safety Net `§5.2.3` не работает, и селектор не запускает собственный контракт

`§5.2.3` (стр. 215) требует для `package.json`, `scripts/*`, `Cargo.toml`, `src/lib.rs`, `src/main.rs`
полный прогон. Фактически:

```
$ node scripts/test-impact.mjs --changed package.json      → typecheck + tests/meta/plan-sync.test.mjs
$ node scripts/test-impact.mjs --changed scripts/test-impact.mjs → typecheck + tests/meta/plan-sync.test.mjs
```

То есть правка самого селектора не запускает `tests/unit/test-impact.test.mjs` — ровно тот тест, который
карточка и меняет. Это поведение унаследовано от QA-IMPACT v1, но именно v2 объявлена как «без потери
покрытия», а карточка правит `scripts/*`. Минимум: `scripts/*` → `test-impact.test.mjs` + `plan-sync`
(лучше — полный прогон, как в стратегии). Если решено оставить на отдельную карточку, зафиксировать это
в отчёте и снять формулировку «v2 = §5.2».

### P3-1. В отчёте неверное имя файла плана

`REPORT.md`, «Гейты»: «в §8 `PLAN.md` нет строк…». Файл называется `TDD-PLAN.md` (мета-тест
`tests/meta/plan-sync.test.mjs` читает §8 именно оттуда; в этой ветке `PLAN.md` вообще нет). Поправить
формулировку.

### P3-2. Риск №1 описан неполно

`REPORT.md`, «Риски»: «профильные файлы `notifications-ui.test.mjs`, … ещё не существуют». Фактически на
этой ветке нет и самих модулей (`ui/notifications.js` и т.д.), а профильные тесты `attachments-render` и
`chat-reactions` покрывают `features/chat.js`/`features/reactions.js`, а не одноимённые модули. После
исправления P1-1 риск нужно переписать: зависимость не «появится тест», а «появится `features/<mod>.js`
из смежных карточек; до этого правило — мёртвый код».

## 3. Что проверено и совпало

- Red-first: независимо воспроизведено на `HEAD`-селекторе + новом тесте — **19 / 11 pass / 8 fail**,
  совпадает с `red.log` построчно по списку упавших кейсов.
- Green: `test-impact` 19/19, `test-policy` 17/17, вместе 36/36, exit 0 (`evidence/targeted.log`).
- MD5 `scripts/test-impact.mjs` и `tests/unit/test-impact.test.mjs` совпали с таблицей отчёта.
- Диф `scripts/test-impact.mjs` — чистая вставка таблицы и одной ветки `Object.hasOwn` перед общим
  `ui/*`-fallback; существующие правила (`app.js`, `tokens.css`, join-server, voice, chat) не тронуты.
- `.only`/`.skip`/`todo` в новом тесте отсутствуют; JSDoc и `// @ts-check` на месте; анти-live проверки
  (`FORBIDDEN`) сохранены и покрывают новые кейсы.
- `--json` после `--changed` по-прежнему не считается файлом; `search.js`/`search-overlay.js` дают
  одинаковый набор (для фантомных путей — да, но см. P1-1).

## 4. Внешние блокеры (не дефекты карточки)

| Блокер | Факт | Статус |
|---|---|---|
| `typecheck` | 16 ошибок в `tests/unit/dispatch_ui.test.mjs` — файл не менялся карточкой, версия из `HEAD` (`f2806f3f…`); в `unstable` рабочая копия уже исправлена (`d30771d6…`), `npm run typecheck` → exit 0 | чинится вне карточки |
| `meta` / `plan-sync` | 10 файлов без строки в `TDD-PLAN.md §8` (`dispatch_ui`, `search-remote-real-index`, `e2e-required`, 7 × `m2-*.e2e`); в `unstable` `TDD-PLAN.md` уже дополнен (+15 строк), `plan-sync` 5/5 | чинится вне карточки |

Оба факта в `REPORT.md` раскрыты честно, `gate-worker.log` им соответствует. Приёмка карточки не может
дать зелёный `gate:worker` до интеграции этих правок — это ограничение ветки, а не подлог отчёта.

## 5. Что ревью не проверяло

- Полный `npm run gate:worker` не перезапускался (использован `gate-worker.log` автора + точечные
  прогоны). `cargo`-фазы и `gate:release`/`gate:qualify` не запускались.
- Живой Discord, окно, сеть не использовались (запрет окна live-операций).
- Поведение селектора на ветках, где модули `features/*` уже есть, проверено только `--changed`-пробами
  на этой ветке (правила чисто строковые, поэтому вывод переносится).

## 6. Рекомендация

1. Переписать ключи маппинга на `rust/gui-spike/ui/features/<mod>.js`; для attachments/chat-reactions
   расширить правила `features/chat.js` и `features/reactions.js` (P1-1).
2. Добавить маппинг `rust/gui-spike/src/<mod>.rs → <mod>_contract + ipc_wiring_contract` и red-first
   тесты на него (P1-2).
3. Вернуть `gui-spike-features.test.mjs` обоим спорным модулям (P2-1), поправить safety net для
   `scripts/*`/`package.json` или явно вынести его из scope (P2-2).
4. В тесте сверять пути с репозиторием (`git ls-files`/`BOARD.md`), чтобы фантомный ключ снова не стал
   «зелёным».
5. После правок — повторный прогон `node --test tests/unit/test-impact.test.mjs
   tests/meta/test-policy.test.mjs` и `npm run typecheck`; MD5 трёх артефактов сверить с этим ревью.
