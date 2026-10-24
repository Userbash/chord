# L-02-UI-IMPL — подписка UI на dispatch и проекции 6 kind (red-first)

Дата: 2026-10-24. Воркер: **G2 Frontend**. PM: DeepSeek. Карточка: `L-02-UI-IMPL` (P1A).
Контракт: `waves/RW-40/PM/l02-ui/PLAN.md` (заморожен). Core-хук: `waves/RW-40/PM/l02-core/REPORT.md`.
Статус: **готово к приёмке**. Live/окно/браузер/сеть/`cargo`/`npm run gate:worker` не запускались.

## 1. Сделано

### Новые модули (весь файл — G2)

| Файл | Что внутри |
|---|---|
| `ui/features/dispatch-model.js` | `DISPATCH_EVENT`, `FORWARDED_KINDS` (6), нормализация §2 (`nilish/asString/asNumber/asBool/asArray/asRecord`), `normalizeEnvelope` (`{kind,data}` + опциональный `seq`), `createDispatchStore`, `applyDispatch`, проекции `projectReady/projectMembersChunk/projectMemberListUpdate/projectPresence/projectReaction`, `setDispatchSelfUserId`, подсчёт `redactedKeys` без чтения значений |
| `ui/features/dispatch-view.js` | DOM-адаптер `Change[]`: рейл (upsert), `.tree-guild`, `#member-online`, live-строки `#member-panel-list .member[data-user-id]`, presence, делегирование реакций в `chat.applyReaction`; `selectGuild`, `setSource` (`data-dispatch-source` на `body` и `#net-status`), счётчики `applied/ignored/failed/notOpen/notLoaded` |
| `ui/features/dispatch-bridge.js` | `resolveDispatchListen`, `createDispatchBridge` (`start/stop/handle/stats/source/store/setSelfUserId`), разбор Tauri-конверта `{event,payload}`, изоляция ошибок model/view, `source=live/absent/unavailable`, 15 счётчиков без payload |

### Узкие правки существующих модулей (по §7.1)

- `ui/features/members.js`: экспорт `STATUS_LABELS`; 4-й аргумент `renderMemberRow(row, item, index, {total?, userId?, label?})`. Без аргумента bench-строка не меняется (`aria-setsize = MEMBER_ROWS`, нет `data-user-id`), `data-role` пишется только при непустой роли.
- `ui/features/chat.js`: приватный `reactionsGroupOf(row)` (вынесен из `renderChannelMessage`, bench-разметка та же) + `applyReaction({channelId,messageId,emoji,count,me,removed}) → "applied"|"not-open"|"not-loaded"`. Видимый счётчик = базовый `messages_page` + live-состояние dispatch; override в `reactions.js` обновляется, чтобы `refreshRow` не откатил пилюлю.
- `ui/features/reactions.js`: аддитивный `adjustReactionOverride(key, emoji, delta, me, base?)`; `toggleReaction`/`requestReaction` не менялись.
- `ui/app.js`: `rail.applyGuilds(items)` (upsert без `onSelect`/фокуса/смены выбора), wiring `dispatchView`/`dispatch` в `boot()` после `join`, `dispatch` в `App`, `ready()` → `setSelfUserId`, `auth onReady → start()`, logout → `stop()`, `dispatch: stats` в `report_ui_snapshot`, `onSelect → view.selectGuild`.

### Что не делалось (границы §1.2)

`MESSAGE_CREATE/UPDATE` в чат, `TYPING_START`, read-state/бейджи, группировка 6 групп,
live-данные на экране «Участники» (`#member-list`), живой источник ядра, окно PM.
`index.html`, CSS, `tests/support/gui-dom-harness.mjs`, `ui/model.js`, Rust, `data.rs`,
фикстуры и чужие тесты **не менялись** (md5 harness/CSS не приводится — файлы не тронуты).

## 2. RED (до продуктового кода)

Тест написан первым, запущен до создания модулей. Ожидаемая причина падения по §8.5 —
отсутствие модулей, а не assert:

```
node --test tests/unit/dispatch_ui.test.mjs
# Error [ERR_MODULE_NOT_FOUND]: Cannot find module
#   '.../ui/features/dispatch-model.js' imported from tests/unit/dispatch_ui.test.mjs
# exit code 1
```

Лог: `waves/RW-40/PM/l02-ui/red.log` (35 строк, `ERR_MODULE_NOT_FOUND`).

## 3. GREEN / targeted

| Проверка | Результат | Лог |
|---|---|---|
| `node --test tests/unit/dispatch_ui.test.mjs` | **37 passed, 0 failed** (A1–A3, B1–B15 + B7a, C1–C14 + C5b, D1–D5) | `green.log` |
| `node --test` существующих UI-тестов: `guild-rail, chat-reactions, channel-switch, typing-ui, read-state-ui, gui-spike-{features,dom,regressions,bench,auth}, join-server, voice-ui` | **347 passed, 0 failed** (регресс bench-строк/панели/рейла не сломан) | `targeted-ui.log` |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json` | чисто (exit 0) | `tsc.log` |
| `npm run gate:worker` | **не запускался** — по ТЗ карточки только targeted; полный gate — integration-лейн | — |

Покрытие negative cases §5 (минимум по одному assert):

| # | Тест |
|---|---|
| N1 | B14, C10 |
| N2 | B13 (null/[]/"x"/{} × 6 kind), C10 |
| N3 | B13, B4 (пропуск без `user.id`), C10 |
| N4 | B3 (READY дважды), B5 (чанк), C1 (повтор READY без IPC) |
| N5 | B8 (дубль INSERT), B11 (повтор ADD) |
| N6 | B9 (DELETE вне окна), B12 (REMOVE без ADD), C7 (count→0) |
| N7 | B15, C9 (DOM + снапшот + `redactedKeys`) |
| N8 | нормализация (снежинки-строки), B4/B6 (id не проходят `Number`) |
| N9 | нормализация + B2 (дамп `"nil"` ≡ `null`) |
| N10 | C11 (burst 100 presence, последний статус) |
| N11 | C12 (экран settings, фоновое обновление) |
| N12 | C13 (`absent` на body/#net-status, `handle` жив), D4/D5 |
| N13 | D3 (`failed`+`lastErrorKind`, следующее событие применено) |

Инварианты §4.4 проверены в C1/C2 (нулевые новые `ready/channels/messages_page`),
C4 (фокус/экран/скролл/тот же узел), C8 (`#chat-list` не пересобран), C11 (узлы не
пересозданы), C13 (нет транспорта — без throw).

## 4. Файлы + md5

`rust/gui-spike/**` вне git-базы (как в L-02), поэтому md5 — единственный слепок.

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/features/dispatch-model.js` | `154eec1272f9f320c0fb79ce21feb63d` |
| `rust/gui-spike/ui/features/dispatch-view.js` | `fd1842ac442dae108f80b3872f163f05` |
| `rust/gui-spike/ui/features/dispatch-bridge.js` | `1cad6f1324b2fe508866d29097792443` |
| `rust/gui-spike/ui/features/chat.js` | `2e918f3cff5dc968ca3117480e306ce4` |
| `rust/gui-spike/ui/features/members.js` | `98801ba746103791f06234d08d2a5ae7` |
| `rust/gui-spike/ui/features/reactions.js` | `ef8f63e02c586bd2c9f43016a92b6b40` |
| `rust/gui-spike/ui/app.js` | `16411a312d7b94f2064e2dc1a14b0266` |
| `tests/unit/dispatch_ui.test.mjs` | `611876033058344cc6b5a48303b91eae` |

## 5. Решения, которые стоит зафиксировать при приёмке

1. **Дедуп реакций.** Бизнес-ключ из §3.5 (`channel:message:emoji:user:action`)
   реализован как точная `Set<userId>` на `(channel,message,emoji)` + счётчик
   анонимных (`degraded`, без `user_id`). Это строже LRU-512 из плана: исключает
   и повтор ADD, и отрицательный REMOVE, и при этом не блокирует повторный ADD
   после REMOVE (LRU по ключу события блокировал бы). Память ограничена окном
   участников реакции, а не временем.
2. **Счётчик реакции.** `Change.count` — live-слой dispatch; `chat.applyReaction`
   складывает его с базой `messages_page` (`C5b`). Иначе первый же live-ADD на
   сообщении с 3 реакциями показал бы «1». Второго источника истины нет: база
   остаётся в `channelWindows.items[].reactions`, live-дельта — в override.
3. **`adjustReactionOverride` + `base`.** Пятый необязательный аргумент нужен,
   чтобы override нёс абсолютный счётчик, а не дельту поверх базы; без него
   `refreshRow` применил бы патч к базе некорректно. Экспорт аддитивный.
4. **`source`.** С транспортом `live`; без `__TAURI__.event` — `absent` с первого
   кадра (C13); ошибка `listen` — `unavailable` (D5). Текст/бейдж не добавлялись (§9.2 п.5).
5. **Порядок live-панели.** Авторитетен `order` из LIST_UPDATE; CHUNK дописывает
   имена в те же строки, но не расширяет окно. Группы в live-режиме не создаются (MB-01).

## 6. Остаток / риски

1. **`MESSAGE_REACTION_*` — форма `[A]`** (1 ADD в union, 0 REMOVE). Парсинг
   толерантный (snake/camel, `emoji.name|id`); после первого live-захвата PM
   решает: типизировать в ядре (G1) или жить на raw.
2. **Live-проверка «событие дошло до окна»** — окно PM, здесь запрещена.
3. **База реакций и повторный `messages_page`.** Если история перезагрузится с
   уже учтённым live-пользователем, live-слой может дать +1 до перезагрузки окна
   (следствие отсутствия per-user данных в `MessageRow.reactions`; M-06/M-01).
4. **`#member-online` после reconnect.** `READY` не перебивает точный
   `online_count` (onlineCountKnown); новый гильдейский список обновит счётчик
   только своим `LIST_UPDATE` — иначе показывался бы member_count.
5. **Группировка live-участников по 6 группам** — MB-01 (нужны роли гильдии);
   панель честно помечена `data-source="live"`, bench-разметка не тронута.
6. **`stop()` на logout** снимает подписку (unlisten), но `data-dispatch-source`
   остаётся capability-значением (`live`): UI за гейтом входа; если нужен явный
   `absent` на logout — отдельное решение PM (нового текста не добавлял).
7. **`gate:worker`** не запускался по ТЗ; интеграционный прогон (fmt/clippy/
   `test --workspace`/e2e) — за integration-лейном.
