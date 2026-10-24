# M-06 — реакции в реальном канале (RW-40, G2 Frontend)

Дата: 2026-10-24. Воркер: G2 Frontend. PM: DeepSeek.
Режим: строгий TDD (red → green), только офлайн. Окно/live/cеть/cargo/gate/commit
не запускались.

## 1. Карточка и контекст

`G6-BATCH-MERGE-01.md` §2.2/§5: `renderChannelMessage` не рендерил pills,
`reactions.js` слал `channelId:"news"`, `messageId=index` — в реальном канале реакций
не было (регресс при переходе C-02 на `messages_page.items`). Backend принят D-53
(Tauri `toggle_reaction {channelId,messageId,emoji,active}`), форма строки —
`MessageRow.reactions: [{emoji,count,me}]` (`rust/gui-spike/src/data.rs`).

## 2. Сделано

- `ui/features/reactions.js`
  - ключ состояния: числовой bench-индекс **или** `channel:<channelId>:<messageId>`
    (`channelReactionKey`) — синтетическое окно и реальный канал больше не делят Map;
  - `requestReaction` принимает `channelId`/`messageId`; для реального канала уходит
    `item.id` и настоящий snowflake канала, bench-путь по умолчанию остаётся
    `"news"`/`String(index)` (B3b не сломан);
  - `toggleReaction(..., {allowUnknown})`: эмодзи из реального сообщения (в т.ч.
    кастомный `emoji.name/id`) кликабельны, прямой API по-прежнему отвергает
    неизвестную реакцию без флага (старый тест `🧪` зелёный);
  - base нормализуется: `null`/не-объект/пустой emoji отбрасываются, `count` → целое ≥ 0;
  - pending по ключу `key:emoji` — один IPC на двойной клик; откат 401/429/сеть как раньше.
- `ui/features/chat.js`
  - `parseChannelPage` читает `item.reactions` (`parseChannelReactions`), `channelId`
    строки; битые записи не роняют окно;
  - `renderChannelMessage` рисует `.reactions-row` → `button.reaction` с
    `.reaction-count`, `aria-pressed`, `data-me`, `aria-label`; ключ оптимистичного
    состояния — `channel:...:item.id`; `data-message-id` остаётся `item.id`;
  - клик → `toggle_reaction` с реальными `channelId`/`messageId`, optimistic
    счётчик/`aria-pressed`, затем перерисовка строки (rollback виден);
  - hover-панель: кнопки `[data-command="react"]` больше не скрываются в канале;
    `reactionTargetOf(index)` отдаёт реальный base/id строки (bench — синтетика).
- `ui/features/message-toolbar.js`
  - новый `reactionTargetOf` для `syncPressed`/`run("react")`; без него поведение
    bench идентично прежнему.
- `ui/components/chat.css`
  - `.channel-message .text` — высота по контенту (в потоке, не 40px с обрезкой);
  - `.channel-message .reactions-row` — отступ под текстом.

`CHAT_ROWS=100_000`, `BENCH_CHANNEL_ID="news"`, `SNAPSHOT_FLOORS` не менялись.

## 3. Red-first

`waves/RW-40/PM/m06/red.log` (до правок продукта):

```
node --test tests/unit/chat-reactions.test.mjs
ℹ tests 9
ℹ pass 0
ℹ fail 9
```

Падения по существу: нет pills в реальной строке, нет `reactions` в `parseChannelPage`,
`toggle_reaction` не вызывается, быстрые реакции скрыты, строковый ключ
`channel:...` отвергается `toggleReaction`.

Новый файл: `tests/unit/chat-reactions.test.mjs` (9 кейсов). Оракул — контракт
`data.rs`/D-53/MSG-23…26, не реализация; реальные 19-значные id, ни одного
`channelId:"news"`/индекса в аргументах.

## 4. Green

`waves/RW-40/PM/m06/green.log`:

```
node --test tests/unit/chat-reactions.test.mjs \
  tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-model.test.mjs \
  tests/unit/gui-spike-bench.test.mjs tests/unit/gui-spike-features.test.mjs \
  tests/unit/gui-spike-auth.test.mjs tests/unit/gui-spike-regressions.test.mjs \
  tests/unit/guild-rail.test.mjs tests/unit/voice-ui.test.mjs \
  tests/unit/channel-switch.test.mjs tests/unit/read-state-ui.test.mjs
ℹ tests 336
ℹ pass 336
ℹ fail 0
ℹ skipped 0
```

Дополнительно (frontend-файлы, не входившие в список проверки):
`join-server.test.mjs + test-impact.test.mjs` — 34/34 pass.

`npx tsc -p rust/gui-spike/ui/tsconfig.json` — exit 0 (`tsc.log`).
`node waves/RW-39/PM/w39c/geometry-check.mjs` — 31/31, exit 0 (`geometry.log`).

Ослаблений нет: существующие тесты не правились, `.only`/`.skip` не добавлялись,
`toggle_reaction` для bench по-прежнему `{channelId:"news", messageId:"26"}`.

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/features/chat.js` | `fae078c6229eb51fc5f2704c086c5a86` |
| `rust/gui-spike/ui/features/reactions.js` | `aae0171204506a81bb9f72937735fb81` |
| `rust/gui-spike/ui/features/message-toolbar.js` | `9cd1968535a6a983cc00a7a0af604e43` |
| `rust/gui-spike/ui/components/chat.css` | `3e242d8b06d74e29ef6f49f04f891e56` |
| `tests/unit/chat-reactions.test.mjs` (new) | `0dd579d0bae3162385bf459ec95dfe33` |
| `waves/RW-40/PM/m06/red.log` | `338294e110e3eda304b41e80e4ddc5f8` |
| `waves/RW-40/PM/m06/green.log` | `7fd391b83c4433f8693a2cab102637b8` |

`ui/features/rows.js` не менялся (был в списке разрешённых). `src/**`,
search/voice/settings/join не тронуты.

## 6. Остаток

1. **Live не проверено** (запрет окна/сети): реальный PUT/DELETE по живому snowflake
   и 429-тело — за live-окном PM (D-53 live).
2. Реакции в UI обновляются только локально: события `MESSAGE_REACTION_ADD/REMOVE`
   из gateway в UI не диспатчатся (зависит от L-02), счётчики чужих реакций до
   перезагрузки страницы не меняются.
3. `add-reaction` без emoji по-прежнему ставит `👍` (прежняя политика панели).
4. `count` не-число трактуется как 0 (backend сериализует `u64`; защита от битых данных).
5. `gate:worker`/полный workspace не запускались — по ТЗ только targeted/tsc/geometry.
