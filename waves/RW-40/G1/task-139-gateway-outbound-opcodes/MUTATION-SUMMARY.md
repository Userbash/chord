# Мутационный прогон task-139 (Review 52A)

Файл: `rust/crates/discord-core/src/gateway.rs`, md5 до/после: `958a3708bc00f4caf6126686e7d91f4f` / `958a3708bc00f4caf6126686e7d91f4f`.

| # | Мутация | Результат | Убитые тесты |
|---|---|---|---|
| M1 | op3: PRESENCE_UPDATE_OP 3 -> 13 | killed | `gateway::tests::presence_update_frame_bytes_match_wetf_layout`, `gateway::tests::presence_update_frame_passes_activity_terms_and_afk` |
| M2 | op4: VOICE_STATE_UPDATE_OP 4 -> 14 | killed | `gateway::tests::voice_state_update_frame_matches_client_shape` |
| M3 | op8: REQUEST_GUILD_MEMBERS_OP 8 -> 18 | killed | `gateway::tests::guild_members_request_covers_query_and_user_ids_paths` |
| M4 | op37: GUILD_SUBSCRIPTIONS_BULK_OP 37 -> 14 | killed | `gateway::tests::guild_subscriptions_frame_matches_client_shape` |
| M5 | chunking: GUILD_MEMBERS_CHUNK_SIZE 100 -> 50 | killed | `gateway::tests::user_ids_chunk_at_one_hundred` |
| M6 | op3: drop required key `afk` from the frame | killed | `gateway::tests::presence_update_frame_bytes_match_wetf_layout`, `gateway::tests::presence_update_frame_passes_activity_terms_and_afk` |

Итог: **6/6** мутантов убиты (score 100%).
