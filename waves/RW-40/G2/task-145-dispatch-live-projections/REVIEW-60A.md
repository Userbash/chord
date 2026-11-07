# Review 60A — task-145: Живые проекции событий шлюза в `dispatch-model.js`

Дата: 2026-10-30. Ревьюер: Технический ревьюер (Review 60A), не автор кода.
Worktree: `/home/sanya/orca/workspaces/project/feature-task-145-dispatch-live-projections`,
ветка `feature/task-145-dispatch-live-projections`, HEAD `4e24048` (база синхронизирована с PR-22).

Ревьюер не менял ни продуктовый код, ни тесты в итоговом состоянии: выполнены чтение отчёта и диффа, контрольные прогоны тестов и `tsc`, серия из 6 независимых мутаций во временной правке `dispatch-model.js` с полным восстановлением исходника и сверкой `md5` до и после.

## Вердикт

**PASS — Ready for Merge.** P0 = 0, P1 = 0, P2 = 3 (один — полнота теста частичного `MESSAGE_UPDATE` без `author`, один — монотонность `version` в `MESSAGE_ACK`, один — ожидаемая граница write-scope между `dispatch-model.js` и `dispatch-view.js`), P3 = 1.

Все 7 целевых событий (`MESSAGE_CREATE`, `MESSAGE_UPDATE`, `MESSAGE_ACK`, `TYPING_START`, `VOICE_STATE_UPDATE`, `VOICE_SERVER_UPDATE`, `THREAD_LIST_SYNC`) переведены из `skipped` в чистые типизированные проекции `dispatch-model.js`, покрыты контрактными тестами `B18–B23` на реальных фикстурах `fixtures/gateway/by-type/*`, а все 45 существующих тестов (включая `FORWARDED_KINDS` из PR-22) сохранены без ослаблений.

## 1. Сводка проверок

| Проверка | Результат |
|---|---|
| `node --test tests/unit/dispatch_ui.test.mjs` | exit 0, **51 passed / 0 failed / 0 skipped** (45 существующих + 6 новых `B18–B23`) |
| `npx tsc -p tsconfig.json` | exit 0, 0 ошибок (вывод пуст) |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | exit 0, 0 ошибок (вывод пуст) |
| Мутации на `rust/gui-spike/ui/features/dispatch-model.js` (M1..M5 + бонус M6) | **6/6 убиты** (`review-60a-mutation.log`) |
| Сверка `md5` с `REPORT.md` до и после мутаций | совпали: `67c6dda6480a4dc8c25db1ecd9d1c1b1` (`dispatch-model.js`), `6b61e8a2a70d751d2e5771e6fd7634b0` (`dispatch_ui.test.mjs`) |
| Сохранение 45 тестов и `FORWARDED_KINDS` из PR-22 | подтверждено диффом против `/home/sanya/orca/project`: 22 имени в `FORWARDED_KINDS` неприкосновенны, `PROJECTED_KINDS` расширен с 6 до 13 |
| Проверка запретов (`.only` / `.skip` / `it.todo`) | 0 совпадений (`grep -nE '\.(only\|skip)\(\|it\.todo'` пуст) |
| Граница write-scope (`git status -s`) | изменены только `rust/gui-spike/ui/features/dispatch-model.js` и `tests/unit/dispatch_ui.test.mjs`; golden-каталоги не тронуты |

## 2. Проверка проекций B18..B23 и сохранения 45 существующих тестов

1. **Сохранение контракта PR-22 и существующих 45 кейсов (`A1–A5`, нормализация, `B1–B17`, `D1–D5`, `C1–C16`):**
   - `FORWARDED_KINDS` (22 канонических имени: 21 из `DispatchKind::as_str` + `MESSAGE_REACTION_REMOVE`) сохранён без изменений.
   - `PROJECTED_KINDS` расширен с 6 до 13 имён, благодаря чему мета-тест `A4` и таблица `B13` (`[null, [], "x", {}]` → `malformed: 1, applied: 0`) автоматически проверяют все 7 новых проекций наравне со старыми 6.
   - В `C10` и `C14` неполные `TYPING_START`/`MESSAGE_CREATE` (без обязательных `user_id`/`id`) теперь закономерно считаются в `malformed`, а проверка `skipped` дополнена каноническими событиями без проекции (`CHANNEL_INFO`, `READY_SUPPLEMENTAL`).
   - Исправлена неточность комментария `ModelCounters` (P2-1 из `REVIEW-51A.md`): для `ignored` явно указаны оба случая (`kind` вне `FORWARDED_KINDS` или no-op проекции `MESSAGE_REACTION_REMOVE` без предшествующего `ADD`).
2. **Разбор новых проекций и тестов `B18–B23`:**
   - **B18 (`MESSAGE_CREATE`, `projectMessageCreate`):** проверяет создание и обновление (upsert) сообщения в `store.messages.get(channelId)`, инкремент `store.messageCounts` только при новом `id`, идемпотентность точного повтора (`duplicate: 1`), приоритет имени автора `global_name → member.nick → username → id` на реальных фикстурах `023/024-MESSAGE_CREATE.json`, погашение тайпинга автора и выставление `unread`/`mentionCount` в `store.readStates`.
   - **B19 (`MESSAGE_UPDATE`, `projectMessageUpdate`):** проверяет обновление `content`, `editedTimestamp`, `edited: true` и `embedCount` поверх созданного сообщения (`026-MESSAGE_UPDATE.json`) и standalone для ранее не загруженного в `store.messages` сообщения с отрицательными `i64` снежинками (`064-MESSAGE_UPDATE.json`), без изменения `store.messageCounts`.
   - **B20 (`MESSAGE_ACK`, `projectMessageAck`):** проверяет сброс `unread = false` и `mentionCount = 0` для `channel_id` до `message_id` (`046-MESSAGE_ACK.json`), сохранение `lastViewed` (`4310`) и `version` (`128950`), идемпотентность повтора (`duplicate: 1`).
   - **B21 (`TYPING_START`, `projectTypingStart`):** проверяет запись активного тайпинга в `store.typing.get(channelId).get(userId)` с числовым `timestamp` и `guildId`, отсев устаревшего `timestamp < previous.timestamp` (`stale: 1`) и дубля (`duplicate: 1`), а также удаление записи тайпинга при приходе `MESSAGE_CREATE` от того же `authorId`.
   - **B22 (`VOICE_STATE_UPDATE` / `VOICE_SERVER_UPDATE`, `projectVoiceState` / `projectVoiceServer`):** проверяет проекцию флагов `selfMute`, `selfDeaf`, `selfStream`, `selfVideo`, нормализацию выхода из голосового канала (`channel_id: "nil"` → `channelId: null`), сохранение `endpoint` в `store.voiceServers` и защиту от утечки секретов (`session_id` и `token` учитываются в `redactedKeys` и отсутствуют в `JSON.stringify({ store, changes })`).
   - **B23 (`THREAD_LIST_SYNC`, `projectThreadListSync`):** проверяет синхронизацию списка тредов гильдии по фикстурам `004/007/020-THREAD_LIST_SYNC.json`, сохранение точности отрицательных `i64` снежинок (`-1467272931325252608`, `-5314960184493214464`), обработку `threads: null` как пустого списка и идемпотентность повтора (`duplicate: 1`).

## 3. Мутационное тестирование (M1..M5 + бонус M6)

Лог прогона сохранён в `/home/sanya/orca/project/waves/RW-40/G2/task-145-dispatch-live-projections/review-60a-mutation.log`. После серии мутаций исходный файл восстановлен байт-в-байт (`md5 = 67c6dda6480a4dc8c25db1ecd9d1c1b1`).

| # | Мутация в `rust/gui-spike/ui/features/dispatch-model.js` | Статус | Кем убит |
|---|---|---|---|
| M1 | `projectMessageCreate`: отключён инкремент `messageCount` (`previousCount` вместо `updated ? previousCount : previousCount + 1`) | **УБИТ** (exit 1) | `B18`, `B19` |
| M2 | `projectMessageUpdate`: отключено обновление `editedTimestamp` и `edited` из входящего кадра | **УБИТ** (exit 1) | `B19` |
| M3 | `projectMessageAck`: `unread: true` вместо `unread: false` при формировании `LiveReadState` | **УБИТ** (exit 1) | `B20` |
| M4 | `projectMessageCreate`: отключено погашение активного тайпинга `store.typing.get(channelId)?.delete(authorId)` | **УБИТ** (exit 1) | `B21` |
| M5 | `projectVoiceState`: отключена нормализация пустого/`"nil"` `channel_id` в `null` при выходе из голоса | **УБИТ** (exit 1) | `B22` |
| M6 (бонус) | `projectThreadListSync`: отключена проверка дубля `JSON.stringify(previousList) === JSON.stringify(threads)` | **УБИТ** (exit 1) | `B23` |

## 4. Замечания (P0 / P1 / P2 / P3)

### P0 — нет

### P1 — нет

### P2

- **P2-1. В `B19` не проверен частичный `MESSAGE_UPDATE` без поля `author` (`tests/unit/dispatch_ui.test.mjs:707–730`, `dispatch-model.js:1122–1128`).**
  В `projectMessageUpdate` корректно реализован fallback `const authorId = asString(author.id) || asString(data.author_id) || previous?.authorId || ""`. Однако в тесте `B19` кадр `dUpdate` собирается из `026-MESSAGE_UPDATE.json`, где поле `author` уже присутствует целиком. Из-за этого утверждение `assert.equal(stored.authorId, String(dCreate.author.id), "MESSAGE_UPDATE затёр автора")` проходит даже если убрать `|| previous?.authorId` из `projectMessageUpdate`. Рекомендуется в последующем срезе передавать в `B19` частичный `MESSAGE_UPDATE` без `author` (как Discord присылает при embed-unfurl).
- **P2-2. `projectMessageAck` не отсеивает устаревшие кадры по `version` (`dispatch-model.js:1193–1236`).**
  В отличие от `projectPresence` (LWW по `processed_at_timestamp`) и `projectTypingStart` (отсев `timestamp < previous.timestamp` в `counters.stale`), `projectMessageAck` применяет любой отличающийся кадр без проверки `version < previous.version`. При переподключении или доставке старого `MESSAGE_ACK` он может откатить `lastMessageId` назад. Не блокирует мерж, но стоит добавить проверку монотонности `version` при наличии обоих значений.
- **P2-3. Отрисовка 7 новых вариантов `DispatchChange` в `dispatch-view.js` остаётся для DOM-среза (ожидаемо по границе write-scope).**
  Write-scope карточки `task-145` включает только `dispatch-model.js` и `dispatch_ui.test.mjs`. Воркер соблюдал границы файлов и зафиксировал это в `REPORT.md` (раздел «Риски / заметки», п. 1).

### P3

- **P3-1. Дедупликация через `JSON.stringify` (`dispatch-model.js:1057, 1168, 1222, 1324, 1414`).**
  Так как структуры `LiveMessage`, `LiveReadState`, `LiveVoiceState`, `LiveThread` создаются внутри проекторов с фиксированным порядком полей, сравнение через `JSON.stringify` детерминировано и корректно, но для `THREAD_LIST_SYNC` на больших гильдиях покомпонентное сравнение сэкономит лишнюю сериализацию.

## 5. Итоговая рекомендация

**PASS / Ready for Merge.** Код чистый, типизация `tsc` (root и UI) проходит с 0 ошибок, 51/51 тестов в `dispatch_ui.test.mjs` зелёные, 6/6 мутантов убиты, исходные контрольные суммы `md5` сохранены.
