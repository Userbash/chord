# REVIEW-58A — Сводка мутационных проверок (`task-142`, `rust/crates/discord-core/src/ws.rs`)

Дата: 2026-10-30.
Целевой файл: `rust/crates/discord-core/src/ws.rs` (исходный и восстановленный `md5 = 9976b056a9ff18559867c1ad110a290f`).
Команда прогона каждого мутанта:
```sh
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --offline --manifest-path rust/Cargo.toml -p discord-core --lib
```

| # | Мутация | Место в `ws.rs` | Статус | Код возврата | Какими тестами убита | Лог |
|---|---|---|---|---:|---|---|
| M1 | Пропуск отправки кадра OP 3 (`PRESENCE_UPDATE`) в буфер исходящих сообщений (`GatewayPipe::send_presence_update` возвращает `Ok(())` без `send_raw_frame`) | `ws.rs:494-505` | **KILLED** | `101` | `ws::tests::send_presence_update_sends_binary_etf_op3_frame`, `ws::tests::outbound_guards_dead_session_and_flushes_queued_frames_after_hello` | `REVIEW-58A-MUTATION-M1.log` |
| M2 | Пропуск отправки кадра OP 4 (`VOICE_STATE_UPDATE`) в буфер исходящих сообщений (`GatewayPipe::send_voice_state_update` возвращает `Ok(())` без `send_raw_frame`) | `ws.rs:508-519` | **KILLED** | `101` | `ws::tests::send_voice_state_update_sends_binary_etf_op4_join_and_leave` | `REVIEW-58A-MUTATION-M2.log` |
| M3 | Пропуск отправки кадра OP 37 (`GUILD_SUBSCRIPTIONS_BULK`) в буфер исходящих сообщений (`GatewayPipe::send_guild_subscriptions_bulk` возвращает `Ok(())` без `send_raw_frame`) | `ws.rs:546-557` | **KILLED** | `101` | `ws::tests::send_guild_subscriptions_bulk_sends_binary_etf_op37_frame` | `REVIEW-58A-MUTATION-M3.log` |
| M4 | Поломка чанкования `user_ids` по 100 в `encode_request_guild_members_frames` / `send_request_guild_members` (кодирование всех `user_ids` одним кадром OP 8 без `chunk_user_ids`) | `ws.rs:592-615` | **KILLED** | `101` | `ws::tests::send_request_guild_members_chunks_user_ids_by_100_and_supports_query`, `ws::tests::outbound_guards_dead_session_and_flushes_queued_frames_after_hello` | `REVIEW-58A-MUTATION-M4.log` |
| M5 | Подмена опкода в исходящем бинарном ETF-кадре (`op = 3 -> op = 4` в `GatewayPipe::send_presence_update`, байт `frame[14] = 4`) | `ws.rs:494-505` | **KILLED** | `101` | `ws::tests::send_presence_update_sends_binary_etf_op3_frame` (побайтный hex-оракул `...6f706104... != ...6f706103...`) | `REVIEW-58A-MUTATION-M5.log` |

**Итог:** 5/5 мутантов убиты (Mutation Score = 100%). После серии мутаций `rust/crates/discord-core/src/ws.rs` восстановлен побайтно (`md5 = 9976b056a9ff18559867c1ad110a290f`), контрольный прогон `--lib` зелёный (`14 passed; 0 failed`).
