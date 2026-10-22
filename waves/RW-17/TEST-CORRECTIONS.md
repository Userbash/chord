# RW-17 / P2-07 — документированные corrections тестов

## Correction 1 (2026-10-20): `badge_entries` и нулевой `badge_count`

**Что было в ТЗ:** W17c s01 ожидал
`badge_entries() == [384299970632876032, 1014537851418980372]` — обе записи, у которых
в READY присутствует поле `badge_count`.

**Факт фикстуры:** у `1014537851418980372` `badge_count=0`; у
`384299970632876032` `badge_count=16` (RESEARCH-P2-07 §2).

**Контракт:** DESIGN-P2-07 §5 определяет `badge_entries()` как записи с
`badge_count > 0`. Поэтому нулевая запись не должна попадать в список; `badge_count(id)`
для неё возвращает 0, `total_badges()` равен 16.

**Исправление green-теста:** s01 проверяет
`badge_entries() == [384299970632876032]`, отдельно
`badge_count(1014537851418980372) == 0` и `total_badges() == 16`.
Red-копия `tests-red/read_state_selectors.rs` сохранена с исходной формулировкой ТЗ;
red-лог — `red-w17c.txt`. Код селектора не менялся: он уже соответствовал DESIGN §5.

## Correction 2 (2026-10-20): channel-запись не падает назад в `last_acked_id`

**Finding:** независимое ревью P2-1 — `acknowledged_message_id` для channel-записи
(`read_state_type` `None`/0) использовал `last_message_id.or(last_acked_id)`. DESIGN §5/§6
разводит поля: `last_message_id` — ack-точка канала, `last_acked_id` — non-channel.
Fallback давал ложный `is_message_read == true` для канала без `last_message_id`.

**Red-first:** добавлен s11
`channel_ack_point_does_not_use_last_acked_id`, ожидающий `None` и `false`; на старом коде
он упал (`left: Some(700), right: None`), лог — `red-p2-1.txt`, копия —
`tests-red/p2-1_read_state_selectors.rs`.

**Исправление:** для `None | Some(0)` селектор возвращает только `last_message_id`;
non-channel ветка сохраняет `last_acked_id → last_message_id`. Ошибочное ожидание s06
(`id=20`, channel с одним `last_acked_id`) исправлено с `Some(300)` на `None`.
После исправления targeted selectors — 11/11, reducer — 17/17 (добавлены r15 forward-ACK,
r16 version без max-guard и r17 unknown-channel version), types — 12/12.
