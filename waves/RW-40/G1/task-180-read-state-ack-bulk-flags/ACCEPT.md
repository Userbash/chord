# Приёмка PM: task-180 (BE-P2-12) — Read-state `POST /read-states/ack-bulk` и опции `flags`/`manual`/`mention_count`

- **PR:** PR-65 (`feature/task-180-read-state-ack-bulk-flags -> unstable`)
- **Статус:** `Ready for Technical Review` (передано Техническому ревьюеру в Review 94A)
- **Дата:** 2026-10-31
- **Автор ревью:** Product Manager (PM)

## Проверка соответствия ТЗ и DoD
1. **`POST /read-states/ack-bulk` (`ACK_BULK_PATH`, `MAX_ACK_BULK_ENTRIES = 100`):**
   - `AckBulkEntry { channel_id, message_id, read_state_type }` с пропуском `read_state_type` при `None`.
   - `ack_messages_bulk` валидирует размер пачки `1..=100` и оба ID до обращения к транспорту, формирует `{"read_states":[...]}` и маппит статусы `200/204` -> `Ok(())`, `401/403/404/429/4xx/5xx` -> `MessageError`.
2. **Опции `AckMessageOptions` (`flags`, `manual`, `mention_count`):**
   - `ack_message_with_options` сохраняет 100% обратную совместимость `ReadStateAck::ack_message` и включает `flags`/`manual`/`mention_count` в JSON-тело только при `Some(...)`.
3. **Тесты и чистота:**
   - Red -> Green подтверждён (`25/25` в `messages_contract` + `17/17` lib + `9/9` `rest_ack`).
   - Clippy (`-D warnings`) и `rustfmt --check` чистые, write-scope соблюдён.

## Решение PM
**ПРИНЯТО (Accepted for Technical Review).** Передаётся на Техническое ревью (Review 94A) с проверкой 5/5 мутаций.
