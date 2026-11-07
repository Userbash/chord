# Review 52A — task-139: исходящие клиентские опкоды 3/4/8/37

- **Ревьюер:** Технический ревьюер (DeepSeek), Review 52A.
- **Ветка:** `feature/task-139-gateway-outbound-opcodes`, HEAD `4e24048`.
- **Предмет:** `rust/crates/discord-core/src/gateway.rs` (diff +553/−1, 1010 строк,
  md5 `958a3708bc00f4caf6126686e7d91f4f`), отчёт
  `waves/RW-40/G1/task-139-gateway-outbound-opcodes/REPORT.md`, находка F-GW-02 в
  `waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` (строки 155–198).
- **Вердикт: PASS / Ready for Merge.** P0 = 0, P1 = 0. Замечания P2/P3 ниже — не блокируют
  приёмку и оформляются как follow-up, а не как возврат работы.

## Что проверено

### 1. Соответствие отчёта фактам

| Утверждение отчёта | Проверка | Итог |
|---|---|---|
| Файл один, md5 `958a3708…` | `git diff --numstat`, `md5sum` | сходится |
| red-фаза: E0432 на 9 имён | `RED.log` | сходится |
| green: `-p discord-core` 583 passed | мой прогон: **583 passed, 0 failed**, exit 0 | сходится |
| targeted: 6 новых тестов + `op_codes_match_protocol_table` | `--lib`: **9/9**, в модуле `gateway::tests` 7 тестов | сходится |
| clippy `-D warnings`, fmt | exit 0 / exit 0 | сходится |
| `gate:worker` = failed из-за HEAD-дефектов | изменены только `gateway.rs` (+ `package-lock.json` до старта); `tests/unit/dispatch_ui.test.mjs` и §8-тесты в diff не входят | сходится, к карточке не относится |

`package-lock.json` в рабочем дереве содержит правку `bin`/`engines` (6 строк) — появилась до
старта задачи, воркер её не трогал; к приёмке task-139 отношения не имеет.

### 2. Оракул: сверка с клиентом 1.0.162 (бандл `discord.com_assets_web.4d4b2f75fdea5470.js`)

Проверял не по отчёту, а по самому бандлу (`dd` по байтовым смещениям) и
`/home/sanya/re-deep/gateway-protocol.json`:

| Что | Смещение | Факт |
|---|---:|---|
| Таблица опкодов | 4 996 200 | `PRESENCE_UPDATE=3`, `VOICE_STATE_UPDATE=4`, `REQUEST_GUILD_MEMBERS=8`, `GUILD_SUBSCRIPTIONS_BULK=37` |
| `presenceUpdate(e,t,n,i)` | 4 997 751 | `send(PRESENCE_UPDATE,{status:e,since:t,activities:n,afk:i})` — совпадает с `encode_presence_update` |
| `voiceStateUpdate` | 4 997 900 | `{guild_id, channel_id, self_mute, self_deaf, self_video, flags}`; `preferred_region(s)`/`tracks` — условно, в коде не реализованы (заявлено) |
| `requestGuildMembers` | 4 998 400 | `{guild_id, query, limit, user_ids, presences}` — совпадает |
| `.chunk(100)` | 4 954 008 | в `GUILD_MEMBERS_REQUEST`: `r()(e.userIds).chunk(100).forEach(...)` |
| `updateGuildSubscriptions` | 4 998 666 | чанки по `>15360` байт JSON; в коде не реализовано (P2-1) |
| `SubscriptionManager.get` | 9 110 400 | `{typing, threads, activities, members, member_updates, channels, thread_member_lists}`; `thread_member_lists` не включён (P3-6) |
| wetf-раскладка | 4 971 540 | `encoding:{string:"binary", key:"binary", array:"list", null:"atom", undefined:"null"}, useLegacyAtoms:!0` — код и hex-оракул совпадают |
| Кадр отправки | 5 024 300 | `t_.pack({op:e,d:t})` — `encode_frame` даёт `{op, d}` |
| Исходящие в захвате | `gateway-protocol.json` | `opcodes.sentGateway.decodable=false` («sha256+len») — декодируемого дампа нет, оракул только исходник клиента |

Байтовый тест `presence_update_frame_bytes_match_wetf_layout` действительно независим: hex-кадр
собран вручную по тегам (`0x74` map, `0x6d` binary, `0x61` small int, `0x6c` list + `0x6a` NIL,
`0x64` atom) и сравнивается с выводом энкодера; я вручную разобрал ожидаемую строку — раскладка
корректна, включая `since:0 → 61 00` и `afk:false → 64 0005 "false"`.

### 3. Код

Проверены константы (`gateway.rs:97–106`), ошибка и тела (`:110–198`), кодировщики
(`:201–287`), хелперы (`:289–320`), тесты (`:696–1005`).

- Формы тел совпадают с клиентом; порядок ключей и типы значений — как у клиента.
- Кодирование: ключи/строки — `BINARY_EXT`, массивы — `LIST_EXT` (с NIL-хвостом), `null` —
  `NIL_EXT`, булевы — атомы `true`/`false`. `NIL_EXT` для отсутствующих значений согласован с
  уже принятым кодом (`identify.rs:330`, `ws.rs:420,450`) и `docs/re/gateway-schema.md` §1.4
  (тег NIL = JSON `null`).
- Ошибок-паник нет: энкод идёт через `etf::encode_checked`, ошибка заворачивается в
  `OutboundError::Encode`; `unsafe` в файле нет; `unwrap/expect` — только в тестах.
- Границы: `chunk_user_ids` — 0 → 0 чанков, 100 → 1, 101 → 2 (проверено тестом и мутацией M5).
- `BTreeMap` в op37 даёт детерминированный порядок гильдий — хорошо для воспроизводимости.
- Циклических зависимостей нет: `gateway` использует `state::PresenceStatus`, `state` на
  `gateway` не ссылается.

### 4. Целевые тесты

```
CARGO_TARGET_DIR=<worktree>/rust/target cargo test --offline \
  --manifest-path rust/Cargo.toml -p discord-core --lib
→ 9 passed; 0 failed   (REVIEW-52A-GREEN-LIB.log)
```

Модуль `gateway::tests` — 7 тестов, 6 новых: `presence_update_frame_bytes_match_wetf_layout`,
`presence_update_frame_passes_activity_terms_and_afk`,
`voice_state_update_frame_matches_client_shape`,
`guild_members_request_covers_query_and_user_ids_paths`, `user_ids_chunk_at_one_hundred`,
`guild_subscriptions_frame_matches_client_shape`.

Полный крэйт: `cargo test -p discord-core` → **583 passed, 0 failed**, exit 0
(`REVIEW-52A-FULL.log`). `cargo clippy -p discord-core --all-targets -- -D warnings` → exit 0
(`REVIEW-52A-CLIPPY.log`), `cargo fmt --check` → exit 0 (`REVIEW-52A-FMT.log`).

### 5. Мутации — 6/6 killed (score 100%)

Требуемые ТЗ пять проверок покрыты шестью мутантами: номера op3/op4/op8/op37 разбиты на
отдельные мутанты, чтобы видеть, какой тест ловит каждый. Перед каждым прогоном файл
подменялся, после — восстанавливался; md5 до и после `958a3708…` совпадает.

| # | Мутация | Итог | Убийца |
|---|---|---|---|
| M1 | op3 `3 → 13` | killed | `presence_update_frame_bytes_match_wetf_layout`, `..._passes_activity_terms_and_afk` |
| M2 | op4 `4 → 14` | killed | `voice_state_update_frame_matches_client_shape` |
| M3 | op8 `8 → 18` | killed | `guild_members_request_covers_query_and_user_ids_paths` |
| M4 | op37 `37 → 14` | killed | `guild_subscriptions_frame_matches_client_shape` |
| M5 | чанк `100 → 50` | killed | `user_ids_chunk_at_one_hundred` |
| M6 | удалён обязательный ключ `afk` из op3 | killed | оба presence-теста |

Логи: `MUTATION-M1..M6.log`, сводка `MUTATION-SUMMARY.md`. В M1 видно, что байтовый оракул
ловит подмену на уровне байта (`…6f70610d…` вместо `…6f706103…`), а не только по семантике.

## Замечания (не блокируют)

- **P2-1. op37 не чанкуется по 15 360 Б.** Клиент режет карту подписок на кадры
  (`updateGuildSubscriptions` @4 998 666); `encode_guild_subscriptions` кодирует всё одним
  кадром. На большом числе гильдий кадр может превысить лимит сервера. В отчёте это заявлено и
  вынесено в бэклог; вызывающий может резать `BTreeMap` сам, но готового хелпера нет. Предлагаю
  отдельную карточку сразу после интеграции отправки.
- **P3-1. op8 всегда шлёт `nonce: null`**, тогда как веб-клиент ключ не отправляет. Требование
  ТЗ выполнено, поле необязательное; стоит проверить на живом шлюзе, что сервер не придирается
  к явному `null`.
- **P3-2. op8 не запрещает одновременно `query` и `user_ids`** — у клиента это два
  взаимоисключающих пути. Сейчас ответственность на вызывающем; можно добавить валидацию в
  билдер.
- **P3-3. `int()` насыщает `u64 > i64::MAX` до `i64::MAX` молча** (`gateway.rs:302`). Для
  `since`/`flags` практически недостижимо, поведение задокументировано, но это единственное
  место, где значение меняется без ошибки.
- **P3-4. Ветка `OutboundError::Encode` не покрыта тестом.** Достижима только на значениях
  >4 ГиБ, поэтому тест нецелесообразен; фиксирую как известный пробел покрытия.
- **P3-5. Новые опкоды не связаны с `Op`.** `Op::from_code(3)` по-прежнему `Unknown(3)` —
  это парсер входящих, так и задумано, но при логировании исходящих коды придётся сопоставлять
  вручную. Кандидат на общий `OutboundOp` в следующей карточке.
- **P3-6. `thread_member_lists` в op37 не кодируется** — заявлено в отчёте, связано с открытой
  RE-P1-01a; для текущего продукта (без стриминга тредов) допустимо.
- **P3-7. Неточность в комментариях/отчёте:** `presenceUpdate` в бандле начинается на 4 997 751,
  а не на 4 998 360 (4 998 360 — это `requestGuildMembers`). На код не влияет, но при будущем
  аудите смещение лучше поправить.

## Замечание по инфраструктуре ревью (важно для batch)

Общий `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target` используется несколькими
worktree, и имя тестового бинаря `discord_core-<hash>` у них совпадает. Первый прогон в этом
каталоге выполнил **чужой** бинарь (8 тестов `identify`/`rest`, которых в этом дереве нет),
а параллельная сборка другого агента перезаписала бинарь между компиляцией и запуском (полный
прогон дал 582 вместо 583). Поэтому все выводы выше пересняты в приватном
`<worktree>/rust/target` с `TMPDIR` на `/home`: 9/9 lib, 583/583 full, 6/6 мутаций. Рекомендация:
на ревью и в batch не делить target-dir между worktree либо сериализовать сборки; иначе
green/red-логи могут относиться к чужому коду. `TMPDIR` на `/tmp` тоже рискован — там срабатывает
квота tmpfs (`Disk quota exceeded` при сборке `ring`).

## Итог

Изменения соответствуют F-GW-02 и ТЗ карточки: формы тел и кодирование сверены с клиентом,
ошибки не паникуют, границы 100/101 покрыты, 6/6 мутантов убиты, полный крэйт зелёный.
Блокирующих дефектов нет — **PASS, Ready for Merge**. Открытым остаётся только то, что заявлено
вне scope: чанкование op37 по 15 360 Б, `thread_member_lists`, отправка в сокет
(`ws.rs`/`supervisor.rs`) — следующая карточка.

## Артефакты ревью

| Файл | Что |
|---|---|
| `REVIEW-52A.md` | этот вердикт |
| `REVIEW-52A-GREEN-LIB.log` | targeted `--lib`, 9/9 |
| `REVIEW-52A-FULL.log` | полный `-p discord-core`, 583/583 |
| `REVIEW-52A-CLIPPY.log` | clippy `-D warnings`, exit 0 |
| `REVIEW-52A-FMT.log` | `cargo fmt --check`, exit 0 |
| `MUTATION-M1..M6.log` | по одному прогону на мутанта (exit 101) |
| `MUTATION-SUMMARY.md` | таблица мутантов, 6/6 killed |
