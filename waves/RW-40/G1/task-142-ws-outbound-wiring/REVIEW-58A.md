# REVIEW-58A — task-142 / BE-RE-03 (`BE-P1-01`), подключение кодировщиков исходящих клиентских кадров OP 3/4/8/37 к транспорту `ws.rs` (F-GW-02)

Дата: 2026-10-30. Ревьюер: Технический ревьюер, Review 58A.
Ветка: `feature/task-142-ws-outbound-wiring`, worktree
`/home/sanya/orca/workspaces/project/feature-task-142-ws-outbound-wiring`,
HEAD `4e240481e59ff4d49732ac8bd5ba08a006363748`.
Режим: сверка с отчётом и RE-оракулом (`F-GW-02`) + аудит диффа `ws.rs` + целевые и интеграционные тесты + 5/5 мутационных прогонов.

## Вердикт

**PASS. Ready for Merge.** P0 = 0, P1 = 0. P2 = 1, P3 = 2.

Срез подключает кодировщики исходящих клиентских опкодов из `crate::gateway` (`task-139` / PR-24) к живому WebSocket-транспорту `rust/crates/discord-core/src/ws.rs`:
1. Публично ре-экспортированы константы, структуры и энкодеры исходящих кадров (`PRESENCE_UPDATE_OP`, `VOICE_STATE_UPDATE_OP`, `REQUEST_GUILD_MEMBERS_OP`, `GUILD_SUBSCRIPTIONS_BULK_OP`, `GUILD_MEMBERS_CHUNK_SIZE`, `Outbound*`, `encode_*`, `chunk_user_ids`) и объявлены публичные алиасы типов `PresenceUpdate`, `VoiceStateUpdateOut`, `RequestGuildMembers`, `GuildSubscriptionsBulk`.
2. В `TransportError` добавлен вариант `Outbound(OutboundError)` с полной реализацией `Display` и `std::error::Error::source` без `unwrap`/`panic!` на путях кодирования и отправки.
3. Реализовано чанкование списка `user_ids` по `GUILD_MEMBERS_CHUNK_SIZE` (`100`) при отправке OP 8 (`encode_request_guild_members_frames`, `GatewayPipe::send_request_guild_members`, `GatewayPipe::queue_request_guild_members`, `ws::send_request_guild_members`): на каждый чанк из ≤100 ID формируется и отправляется отдельный несжатый бинарный ETF-кадр (`Message::binary`), при пустом `user_ids` и заданном `query` отправляется 1 кадр поиска, при пустых `user_ids` и `query` — `0` кадров.
4. На `GatewayPipe` и на уровне модуля `ws` реализованы асинхронные функции отправки бинарных ETF-кадров `send_presence_update` (OP 3), `send_voice_state_update` (OP 4), `send_request_guild_members` (OP 8) и `send_guild_subscriptions_bulk` (OP 37), а также буфер очереди исходящих кадров (`queue_*`, `pending_outbound_frames`, `maybe_flush_outbound`), который дренируется в `connect_and_run_resume` строго после получения `HELLO` и отправки `IDENTIFY`/`RESUME`. Методы `GatewayPipe` проверяют живость сессии (`!session.is_dead()`) и ведут счётчики `frames_sent()` / `bytes_sent()`.
5. Попутно закрыто замечание F-01 из Review 53A: устаревший комментарий в шапке `ws.rs:55-62` про удержание 64 КиБ окна `ruzstd` (44 терма / 759 280 Б) обновлён под потоковый декодер `libzstd` (`task-137` / PR-23: 824 816 Б и все 137 термов без задержки).

## Что проверено

- **Файловый скоуп и контрольные суммы:**
  - Основной файл задачи — `rust/crates/discord-core/src/ws.rs` (`md5 = 9976b056a9ff18559867c1ad110a290f`, совпадает с `REPORT.md`).
  - Для сборки поверх принятых в основной репозиторий PR-23 (`task-137`) и PR-24 (`task-139`) в worktree скопированы 7 файлов из `/home/sanya/orca/project` (`gateway.rs`, `zstd.rs`, `discord-codec/Cargo.toml`, `Cargo.lock`, `zstd_stream.rs`, `dispatch_router.rs`, `ws_transport.rs`); проверено побайтное совпадение `md5` всех 7 файлов с `/home/sanya/orca/project`.
  - Общие файлы (`rust/crates/discord-core/src/lib.rs`, `state/*`) не затронуты.
- **Качество тестов и соблюдение TEST-POLICY:**
  - В `RED.log` (`md5 = 7289e64df38a5d56f1ef41ca04b496c9`) зафиксирован честный RED-прогон до реализации (`EXIT_CODE=101`, ошибки `E0432`/`E0599` на отсутствующих типах и методах `ws`).
  - В `ws::tests` добавлены 5 новых модульных тестов, проверяющих:
    1. побайтное совпадение бинарного WS-кадра OP 3 с независимым hex-оракулом wetf-раскладки (`8374000000026d000000026f706103...`) и отправку второго кадра со статусом `dnd`, `since`, custom activity и `afk: true`;
    2. отправку бинарных кадров OP 4 для входа (`guild_id`/`channel_id` как binary-строки) и выхода (`None -> EtfValue::Nil`);
    3. чанкование `user_ids` в OP 8 на границах `0` (0 кадров), `100` (1 кадр), `101` (2 кадра: 100 + 1), `205` (3 кадра: 100 + 100 + 5) и поисковый путь `query: Some("monolit")`;
    4. отправку бинарного кадра OP 37 с картой подписок двух гильдий (`BTreeMap`) и проверку всех полей `OutboundGuildSubscription`;
    5. отказ отправки на мёртвой сессии (`op7`), проброс ошибки записи закрытого сокета (`TransportError::Ws`) и дренаж очереди из 5 кадров (`[3, 4, 8, 8, 37]`) в `connect_and_run` строго после `HELLO` (с проверкой 50 мс тишины до `HELLO`).
  - Ни один тест не ослаблен, не удалён и не помечен `#[ignore]`.

## Прогоны

Все команды запускались последовательно с `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.

| Прогон | Команда | Результат | Лог |
|---|---|---|---|
| Targeted unit (`--lib`) | `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib` | **14 passed, 0 failed** (`EXIT=0`) | `REVIEW-58A-GREEN-LIB.log` |
| Интеграционные тесты транспорта (`ws_transport`) | `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --test ws_transport` | **19 passed, 0 failed** (`EXIT=0`) | `REVIEW-58A-WS-TRANSPORT-IT.log` |
| Clippy (`discord-core --all-targets`) | `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-core --all-targets -- -D warnings` | **0 предупреждений** (`EXIT=0`) | `REVIEW-58A-CLIPPY.log` |
| Форматирование (`rustfmt`) | `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` | **чисто** (`EXIT=0`) | `REVIEW-58A-FMT.log` |

Контрольные суммы артефактов воркера проверены и совпадают с `REPORT.md`:
- `rust/crates/discord-core/src/ws.rs` — `9976b056a9ff18559867c1ad110a290f`
- `waves/RW-40/G1/task-142-ws-outbound-wiring/RED.log` — `7289e64df38a5d56f1ef41ca04b496c9`
- `waves/RW-40/G1/task-142-ws-outbound-wiring/GREEN-LIB.log` — `cbb72228bf4f0987585448ba034c9cf6`
- `waves/RW-40/G1/task-142-ws-outbound-wiring/GREEN.log` — `e6322fb56b220b2530ae4164133573d6`
- `waves/RW-40/G1/task-142-ws-outbound-wiring/CLIPPY.log` — `d191a896ec84e9969c377f8c0df91aad`
- `waves/RW-40/G1/task-142-ws-outbound-wiring/FMT.log` — `505495a845524a233efdaf568f0a0a59`

## Мутационные проверки (5/5 убиты)

Подробная сводка — в `REVIEW-58A-MUTATION-SUMMARY.md`, полные выводы — в `REVIEW-58A-MUTATION-M1.log` … `REVIEW-58A-MUTATION-M5.log`. После завершения серии `ws.rs` восстановлен из резервной копии (`md5 = 9976b056a9ff18559867c1ad110a290f`), тестовый бинарь пересобран начисто.

| # | Мутация | Место | Результат | Чем поймана |
|---|---|---|---|---|
| M1 | Пропуск отправки кадра OP 3 (`PRESENCE_UPDATE`) в буфер исходящих сообщений (`GatewayPipe::send_presence_update` возвращает `Ok(())` без `send_raw_frame`) | `ws.rs:494-505` | **KILLED** (`EXIT=101`, 2 failed) | `ws::tests::send_presence_update_sends_binary_etf_op3_frame`, `ws::tests::outbound_guards_dead_session_and_flushes_queued_frames_after_hello` |
| M2 | Пропуск отправки кадра OP 4 (`VOICE_STATE_UPDATE`) в буфер исходящих сообщений (`GatewayPipe::send_voice_state_update` возвращает `Ok(())` без `send_raw_frame`) | `ws.rs:508-519` | **KILLED** (`EXIT=101`, 1 failed) | `ws::tests::send_voice_state_update_sends_binary_etf_op4_join_and_leave` |
| M3 | Пропуск отправки кадра OP 37 (`GUILD_SUBSCRIPTIONS_BULK`) в буфер исходящих сообщений (`GatewayPipe::send_guild_subscriptions_bulk` возвращает `Ok(())` без `send_raw_frame`) | `ws.rs:546-557` | **KILLED** (`EXIT=101`, 1 failed) | `ws::tests::send_guild_subscriptions_bulk_sends_binary_etf_op37_frame` |
| M4 | Поломка чанкования `user_ids` в `encode_request_guild_members_frames` / `send_request_guild_members` (отправка всех `user_ids` одним кадром OP 8 без `chunk_user_ids`) | `ws.rs:592-615` | **KILLED** (`EXIT=101`, 2 failed) | `ws::tests::send_request_guild_members_chunks_user_ids_by_100_and_supports_query`, `ws::tests::outbound_guards_dead_session_and_flushes_queued_frames_after_hello` |
| M5 | Подмена опкода в исходящем бинарном ETF-кадре (`op = 3 -> op = 4` в `GatewayPipe::send_presence_update`, байт `frame[14] = 4`) | `ws.rs:494-505` | **KILLED** (`EXIT=101`, 1 failed) | `ws::tests::send_presence_update_sends_binary_etf_op3_frame` (`...6f706104... != ...6f706103...`) |

## Findings

**P0:** нет.

**P1:** нет.

**P2-1. Дренаж `outbound_queue` в `connect_and_run_resume` привязан к приходу входящих кадров (`ws.rs:912, 962`), без отдельной ветки асинхронного канала команд в `tokio::select!`.**
Очередь `GatewayPipe::queue_*` корректно удерживает кадры до `HELLO`/`IDENTIFY` и дренируется сразу после рукопожатия и после каждого входящего `Message::Binary`. Для следующей карточки `BE-P1-01` (проброс runtime-команд OP 3/4/8/37 из `supervisor.rs` / Tauri IPC в уже открытый сокет во время простоя между входящими событиями) потребуется либо передача `mpsc::Receiver` в `select!` цикла `connect_and_run_resume`, либо вызов `GatewayPipe::send_*` напрямую на разделяемом `Sink`.

**P3-1. Свободные функции `ws::send_*` (`ws.rs:618-685`) не обновляют счётчики `GatewayPipe` и не проверяют `Session::is_dead()`.**
Это ожидаемо для свободных хелперов поверх `W: Sink<Message>`, не принимающих `&mut GatewayPipe`; при наличии экземпляра `GatewayPipe` вызывающему коду следует предпочитать методы `GatewayPipe::send_*`.

**P3-2. Сопутствующие синхронизированные файлы в рабочем дереве ветки.**
В `git status` помимо целевого `rust/crates/discord-core/src/ws.rs` присутствуют синхронизированные из `/home/sanya/orca/project` файлы PR-23 / PR-24 и общая для worktree модификация `package-lock.json`. При переносе результата `task-142` в `/home/sanya/orca/project` нужно забирать только `rust/crates/discord-core/src/ws.rs` (`md5 = 9976b056a9ff18559867c1ad110a290f`).
