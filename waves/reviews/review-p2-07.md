# Независимое ревью RW-17 / P2-07 — read-state и бейджи

Дата: 2026-10-20. Ревьюер — отдельный агент, не автор кода. Режим: read-only + прогоны.
Контракт: `waves/RW-17/DESIGN-P2-07.md`, `RESEARCH-P2-07.md`, `TZ-P2-07.md`,
`TEST-CORRECTIONS.md`.

## Раунд 1 — не принято

**P0 = 0, P1 = 0, P2 = 1, P3 = 7.**

### P2-1 — channel fallback в `acknowledged_message_id`

`src/state/read_state_selectors.rs` для channel-записи (`read_state_type` `None`/0)
возвращал `last_message_id.or(last_acked_id)`. DESIGN §5/§6 разводит семантику:
`last_message_id` — ack-точка канала, `last_acked_id` — non-channel. Fallback давал
ложное `is_message_read == true` для канала без `last_message_id`; тест s06 закреплял
это поведение.

**Закрытие red-first:**

- добавлен s11 `channel_ack_point_does_not_use_last_acked_id`;
- на старом коде: `Some(700) != None`, exit 101 (`red-p2-1.txt`);
- селектор исправлен: channel → только `last_message_id`;
- s06 исправлен на `None`, описано в `TEST-CORRECTIONS.md` Correction 2;
- green targeted: 40/40.

## Раунд 2 — принято

**P0 = 0, P1 = 0, P2 = 0, P3 = 5.**

Проверено повторно:

- P2-1 закрыт в коде, тесте и документации; red-копия и лог на месте;
- non-channel ветка сохраняет `last_acked_id → last_message_id`;
- r15 forward-ACK, r16 version без max-guard, r17 unknown-channel версия — корректны;
- `raw_display` перенесён без регрессий; clippy/fmt чистые;
- DESIGN §4.2/§4.7/§5 согласован с кодом и тестами;
- mutations 7/7 caught, md5 восстановлены.

### P3 (backlog, не блокирует)

1. `red-w17b.txt` содержит номера строк ревизии до финального форматирования; копия теста
   совпадает, причина падения та же. Помечено в отчёте.
2. Старый `mutations.txt` (4/4) оставлен как исторический и помечен superseded;
   актуальный `MUTATIONS.md` — 7/7.
3. Live-smoke отсутствует (нет окна владельца); проверка только на фикстурах.
4. Нет `unread_count`: в READY/MESSAGE_ACK нет количества непрочитанных; политика — P2-11.
5. Нет cap/prune read-state: READY 157 записей, ACK только upsert; ограничение осознанное.

## Прогоны раунда 2

| Команда | Результат |
|---|---|
| targeted | **40 passed** (12 + 17 + 11) |
| `cargo test -p discord-core` | **418 passed** |
| `cargo test --workspace` | **460 passed** |
| `cargo fmt --all -- --check` | чисто |
| `cargo clippy -p discord-core --tests -- -D warnings` | чисто |

## Вердикт

**Принято.** P0/P1/P2 = 0. Карточка P2-07 закрыта; артефакты — `waves/RW-17/`.
