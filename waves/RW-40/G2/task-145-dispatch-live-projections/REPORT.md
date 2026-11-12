# task-145 — Живые проекции событий шлюза в `dispatch-model.js`

- **batch/карточка:** RW-40 / G2 Frontend / `task-145` (продолжение `task-138` / F-GW-03: перевод живых событий чата, голоса и тредов из `skipped` в типизированные проекции модели).
- **worktree:** `/home/user/orca/workspaces/project/feature-task-145-dispatch-live-projections` (база синхронизирована с `/home/user/orca/project` после PR-22).
- **файлы (write-scope):**
  - `rust/gui-spike/ui/features/dispatch-model.js`;
  - `tests/unit/dispatch_ui.test.mjs`;
  - этот отчёт и логи рядом в `/home/user/orca/project/waves/RW-40/G2/task-145-dispatch-live-projections/`.

## Сделано

1. **Синхронизация с PR-22:** скопированы актуальные версии `rust/gui-spike/ui/features/dispatch-model.js` и `tests/unit/dispatch_ui.test.mjs` из `/home/user/orca/project`.
2. **Red-тесты (Strict TDD):**
   - Расширен `PROJECTED_KINDS` в `tests/unit/dispatch_ui.test.mjs` с 6 до 13 типов (`+ MESSAGE_CREATE`, `MESSAGE_UPDATE`, `MESSAGE_ACK`, `TYPING_START`, `VOICE_STATE_UPDATE`, `VOICE_SERVER_UPDATE`, `THREAD_LIST_SYNC`), благодаря чему мета-тест `A4` и таблица битых данных `B13` автоматически проверяют все новые проекции.
   - Добавлены целевые контрактные тесты `B18–B23` на реальных фикстурах `fixtures/gateway/by-type/*`:
     - `B18` (`MESSAGE_CREATE`, `023/024-MESSAGE_CREATE.json`): добавление и обновление (upsert) сообщения в `store.messages`, инкремент `store.messageCounts`, идемпотентность (`duplicate` при повторе без изменений), приоритет имени автора `global_name → member.nick → username → id`.
     - `B19` (`MESSAGE_UPDATE`, `026/064-MESSAGE_UPDATE.json`): обновление `content`, `editedTimestamp`, `edited` и `embedCount` как поверх созданного сообщения, так и standalone (отрицательные `i64` снежинки), без увеличения счётчика сообщений канала.
     - `B20` (`MESSAGE_ACK`, `046-MESSAGE_ACK.json`): сброс `unread = false` и `mentionCount = 0` для `channel_id` до `message_id`, сохранение `lastViewed` (`4310`) и `version` (`128950`), идемпотентность повтора.
     - `B21` (`TYPING_START`): запись активного тайпинга `(channelId, userId)` с `timestamp` и `guildId` в `store.typing`, отсев устаревших событий (`stale`) и дублей (`duplicate`), автоматическое погашение тайпинга автора при приходе `MESSAGE_CREATE`.
     - `B22` (`VOICE_STATE_UPDATE` и `VOICE_SERVER_UPDATE`): проекция `channelId`, `selfMute`, `selfDeaf`, `selfStream`, `selfVideo`, выход из голоса (`channel_id: "nil"` → `channelId: null`) и сохранение `endpoint` в `store.voiceServers` без утечки `session_id`/`token` в `store` или `changes` (с учётом в `redactedKeys`).
     - `B23` (`THREAD_LIST_SYNC`, `004/007/020-THREAD_LIST_SYNC.json`): синхронизация списка тредов гильдии в `store.threads`, сохранение точности отрицательных `i64` снежинок, обработка `threads: null` как пустого списка и идемпотентность повтора.
   - Обновлены `C10` и `C14`: неполные `TYPING_START`/`MESSAGE_CREATE` (без `user_id`/`id`) теперь учитываются как `malformed`, а для проверки `skipped` добавлены вызовы канонических событий без проекции (`CHANNEL_INFO`, `READY_SUPPLEMENTAL`).
3. **Чистые проекции в `rust/gui-spike/ui/features/dispatch-model.js`:**
   - Расширен `DispatchStore` (`messages`, `messageCounts`, `readStates`, `typing`, `voiceStates`, `voiceServers`, `threads`) и union `DispatchChange` (`message_create`, `message_update`, `message_ack`, `typing`, `voice_state`, `voice_server`, `threads`).
   - Реализованы и экспортированы `projectMessageCreate`, `projectMessageUpdate`, `projectMessageAck`, `projectTypingStart`, `projectVoiceState`, `projectVoiceServer`, `projectThreadListSync` и подключены в `switch (kind)` внутри `applyDispatch`.

## red (до реализации)

`node --test tests/unit/dispatch_ui.test.mjs` → exit 1 (`red-dispatch-ui.log`, 41 passed / 10 failed):

- `A4`: `MESSAGE_ACK проекция не дала изменений` (`0 !== true`);
- `B13`: `MESSAGE_CREATE: [] не malformed` (`0 !== 1`);
- `B18`: `MESSAGE_CREATE попал в skipped` (`1 !== 0`);
- `B19`: `MESSAGE_UPDATE попал в skipped` (`1 !== 0`);
- `B20`: `MESSAGE_ACK попал в skipped` (`1 !== 0`);
- `B21`: `TYPING_START попал в skipped` (`1 !== 0`);
- `B22`: `VOICE_STATE_UPDATE попал в skipped` (`1 !== 0`);
- `B23`: `THREAD_LIST_SYNC попал в skipped` (`1 !== 0`);
- `C10`: `skipped не 2` (`4 !== 2`);
- `C14`: `пустой TYPING_START не учтён как malformed` (`0 !== 1`).

## green

- `node --test tests/unit/dispatch_ui.test.mjs` → **51 passed, 0 failed, 0 skipped**, exit 0 (`green-dispatch-ui.log`, существующие 45 кейсов + 6 новых `B18–B23`).
- `node --test 'tests/unit/**/*.test.mjs'` → **710 passed, 0 failed, 0 skipped**, exit 0 (`green-unit-all.log`).
- `npx tsc -p tsconfig.json` → exit 0 (`tsc-root.log`, вывод пуст).
- `npx tsc -p rust/gui-spike/ui/tsconfig.json` → exit 0 (`tsc-ui.log`, вывод пуст).

## Файлы + md5

| файл | md5 |
|---|---|
| `rust/gui-spike/ui/features/dispatch-model.js` | `67c6dda6480a4dc8c25db1ecd9d1c1b1` |
| `tests/unit/dispatch_ui.test.mjs` | `6b61e8a2a70d751d2e5771e6fd7634b0` |

Логи: `red-dispatch-ui.log`, `green-dispatch-ui.log`, `green-unit-all.log`, `tsc-root.log`, `tsc-ui.log`.

## Риски / заметки

1. **Граница write-scope соблюдена:** менялись только `rust/gui-spike/ui/features/dispatch-model.js` и `tests/unit/dispatch_ui.test.mjs`. DOM-адаптер `dispatch-view.js` в write-scope карточки не входит, поэтому новые типы `Change` (`message_create`, `message_update`, `message_ack`, `typing`, `voice_state`, `voice_server`, `threads`) формируются и сохраняются в `DispatchStore`, а подключение их отрисовки в `dispatch-view.js` — предмет интеграционного/DOM-среза.
2. **Защита от утечки секретов:** `VOICE_STATE_UPDATE.session_id` и `VOICE_SERVER_UPDATE.token` учитываются в `counters.redactedKeys` и не сохраняются ни в `DispatchStore`, ни в `DispatchChange[]` (проверено в `B22`).
