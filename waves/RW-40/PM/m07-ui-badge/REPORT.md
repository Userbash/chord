# M-07-UI-BADGE — badge/`flags` из `read_state` в UI (red-first)

Дата: 2026-10-30. Воркер: **G2 Frontend Lane A**. HEAD: `4e240481e59ff4d49732ac8bd5ba08a006363748`.
Статус: **готово к приёмке**. `cargo`, окно/live/сеть/gate/commit не запускались;
`app.js`, `quick-switcher.js` (Lane B), `fixtures/`, `logs/`, `evidence/`, `functions/` не тронуты.

- batch/карточка: **RW-40 / M-07-UI-BADGE** (отдел G2 Frontend Lane A);
- вход: `waves/RW-40/PM/m07-be3/REPORT.md` (`read_state()` отдаёт `badgeCount`/`flags`).

## Контракт (источник)

- `waves/RW-40/PM/m07-be3/REPORT.md`: `read_state()` — 9 camelCase-полей;
  `badgeCount: Option<i64>` (число как есть, отсутствие → `null`, `0` — значение),
  `flags` без изменений; `null ≠ 0`.
- `docs/rust-client/CODE-STANDARD.md` §RW-17: **`badge_count = 0` — валидное поле
  READY, но не бейдж**; предикат показа — `> 0`, это политика UI, не проекции.
  `badge_count`/`last_acked_id` — non-channel поля: `last_message_id` — ack-точка
  канала, `last_acked_id` — нет. UI **не** использует `last_acked_id` как курсор.
- `fixtures/gateway/by-type/002-READY.json`: канал-оракул `1163408591315812392` —
  `flags: 1`, `mention_count: 0`, `last_viewed: 4287`, `last_message_id:
  1553370750381990038`; badge-поля в READY — только у двух non-channel записей
  (`badge_count` 16 и 0). Тестовые badge-записи канала — синтетика поверх формы
  `m07-be3`, курсор (`lastViewed`/`lastMessageId`) берётся из READY.
- `CONTRACTS/read-state.md` §1/§4/§5 (устарел по badge — см. «Риски»).

## Сделано

`rust/gui-spike/ui/features/chat.js`:

- `loadReadState()` читает `badgeCount` и `flags` тем же строгим `counter()`
  (целое `>= 0`; строка/`null`/мусор → `null`, не 0) и складывает состояние канала
  (`flags`/`mentionCount`/`badgeCount`) в `channelReadState` (`Map` по `channelId`);
  запись без канала удаляется из карты, чтобы не отдавать чужое состояние.
- `applyReadState()`:
  - **упоминания приоритетнее**: `mentionCount > 0` → прежняя полоса
    «N упоминаний», `data-mentions`; `badgeCount` рядом сохраняется в
    `data-badges` и не подменяет/не занижает счётчик упоминаний;
  - `mentionCount == 0 | null`, но `badgeCount > 0` → бейдж непрочитанных
    «N непрочитанных», `data-badges=N`, `data-mentions` снят;
  - `badgeCount = 0` → **не бейдж** (RW-17), полоса скрыта, `data-badges` снят;
  - `flags` — в `data-flags` полосы и `data-read-flags` плитки канала как есть
    (`0` сохраняется как значение); `null` снимает атрибуты;
  - успешный ack гасит полосу (`data-cleared="true"`, `data-mentions`/`data-badges`
    сняты), но не переписывает серверные `flags`.
- `ChatScreen.readState` — состояние открытого канала из READY
  (`{flags, mentionCount, badgeCount} | null`) для последующих G-08/C-03.
- `prepareReadStateBand()` сбрасывает `data-badges`/`data-flags` при открытии
  другого канала (нет протечки счётчиков между каналами).

`tests/unit/read-state-ui.test.mjs` (+5 тестов): helper `channelReadState()`
(курсор из READY, badge-поля `m07-be3`); badge при `mentionCount = 0`; badge при
`mentionCount = null` и `flags = 0`; приоритет упоминаний при `badgeCount` больше
и меньше `mentionCount`; `badgeCount = 0` — не бейдж. Сброс бейджа после
успешного ack проверяется в первом тесте (deferred ack).

## RED (до продуктового кода)

`waves/RW-40/PM/m07-ui-badge/red.log` —

```
node --test tests/unit/read-state-ui.test.mjs
✖ badgeCount > 0 при mentionCount = 0 → бейдж непрочитанных, flags канала в dataset
✖ mentionCount null, badgeCount > 0 → бейдж без ack; flags = 0 сохраняется как значение
✖ согласованность: упоминания приоритетнее, badgeCount сохраняется отдельно
✖ badgeCount меньше mentionCount не занижает упоминания
✖ badgeCount = 0 — значение, а не бейдж (RW-17): полоса скрыта, 0 не выдуман
ℹ tests 13  ℹ pass 8  ℹ fail 5
EXIT: 1
```

Падения — по отсутствующим `data-badges`/`data-flags` (`null !== '7'` и т.п.),
а не по таймауту/harness. Существующие 8 тестов M-07-UI в red-прогоне зелёные:
контракт ack/mention не ослаблен, добавлены только новые assert'ы.

## GREEN

| Прогон | Результат | Лог |
|---|---|---|
| `node --test tests/unit/read-state-ui.test.mjs tests/unit/channel-paging.test.mjs` | **29 passed / 0 failed / 0 skipped**, EXIT 0 | `green.log` |
| `npx tsc -p tsconfig.json` | EXIT 0, без вывода | `tsc.log` |

`cargo`/окно/live/gate/commit не запускались (запрет ТЗ). Полный `gate:worker` —
за integration-лейном batch.

## Файлы + md5

| Файл | md5 до карточки | md5 после |
|---|---|---|
| `rust/gui-spike/ui/features/chat.js` | `6444e9d94a6daa9e1d7b036525295398` | `1e73c60219792da3c332b8cbc73c7607` |
| `tests/unit/read-state-ui.test.mjs` | `f9d6e653c31e5c8c1ab60ce04352f7d2` | `fe363afe9abcc82bc99e1e59b0af58c5` |

Логи: `waves/RW-40/PM/m07-ui-badge/{red.log, green.log, tsc.log}`.

`app.js` и `quick-switcher.js` не менялись: из моего мандата тронуты только
`chat.js` и `read-state-ui.test.mjs`, плюс каталог логов `waves/RW-40/PM/m07-ui-badge/`.
Остальные изменения в `git status` — чужие карточки (working tree до меня).

## Оракул

READY-запись канала (`flags: 1`, `mention_count: 0`, `last_viewed: 4287`,
`last_message_id`) + форма проекции `m07-be3` + предикат RW-17. Тесты не читают
константы реализации: badge-счётчики и `flags` ожидаются как значения из
фикстуры/переопределений, `null`/`0` различаются явно.

## Остаток / риски

- **`CONTRACTS/read-state.md` устарел**: §1 перечисляет 6 полей, §4 говорит
  «`mentionCount = 0/null` → полоса скрыта», §5 называет `badgeCount` «вне
  контракта». После карточки badge потребляется — нужен аддитивный апдейт §1
  (9 полей), §4 (`data-badges`, `data-flags`, приоритет упоминаний) и §5.
  Файл вне моего мандата — решение PM/TL.
- **Семантика `badge_count` не доказана RE**: RW-17 называет поле non-channel
  (`last_acked_id` — не курсор канала). Карточка требует показывать `badgeCount`
  как непрочитанные канала при отсутствии упоминаний; реализовано ровно так,
  `lastAckedId`/`readStateType` **не** читаются и в ack не попадают. Если RE
  подтвердит иную семантику — правка предиката в `applyReadState`, тесты
  зафиксируют смену контракта.
- **`flags` не интерпретируется** (нет RE-оракула битов): хранится в
  `data-flags`/`data-read-flags` и `ChatScreen.readState` как число `>= 0`;
  отрицательные/дробные значения считаются отсутствующими (`null`).
- **Живой паритет badge** (реальный READY с badge-полями канала, MESSAGE_ACK на
  badge) не проверялся: окно/live не использовались.
- **Дрейф до карточки**: `chat.js` в рабочем дереве уже содержал непринятые
  правки C-02-UI-OLDER (scroll snapshot) — md5 до карточки `6444e9d9…`, не
  `2e918f3c…` из `m07-be3`. Мои изменения — только блок read-state/badge.
- **Вне карточки**: `node --test tests/unit/gui-spike-features.test.mjs` падает на
  `settings_load на старте` (`2 !== 1`) и с HEAD-версией `chat.js` — это
  pre-existing дефект других правок, не от badge; в обязательные прогоны ТЗ не
  входит. `npx tsc -p rust/gui-spike/ui/tsconfig.json` даёт 2 ошибки в
  `app.js:1258/1271` (Lane B, мой файл чист); обязательный `tsc -p tsconfig.json`
  — EXIT 0.
- Счётчик badge на плитке гильдии (G-08) не делался: карточка про канал, рейл
  гильдий вне мандата.

## Следующий шаг

Приёмка Team Lead → batch-гейт integration → апдейт `CONTRACTS/read-state.md`
(§1/§4/§5) владельцем контракта.
