# TEST-MATRIX P2-10 — voice states

Оракулы: `fixtures/gateway/by-type/003-READY_SUPPLEMENTAL.json` (md5
`55983830547abaf8d1b122798fdcf568`), live-события из
`reference-oracle/gw-proto/msgs-*.jsonl` (точные значения в
`RESEARCH-P2-10.md` §2), замороженный `DESIGN-P2-10.md` §4–§6.

Правило: тест сначала падает (compile-red или behavior-red) с сохранённым exit 101; зелёный
без изменения ожиданий. Синтетические кадры строятся через `tests/support/state_fixture` и
повторяют форму провода (атомы `nil/true/false`, строки-id).

## `tests/voice_state_types.rs` (6)

| ID | Вход | Ожидание |
| --- | --- | --- |
| v01 | `Event::from_frame(003)` | snapshot 1; guild `274093052673392641`, channel `351403515723120651`, user `289091818442719242`, `connected_at=1792367828`, все флаги false, `discoverable=true`, request `None`; diagnostics пусты |
| v02 | synthetic `VOICE_STATE_UPDATE` s=111 | guild `1014537851418980372`, user `384299970632876032`, channel `1176785373989445703`, connected `1792391032`, `self_mute=true`, `self_video=false`, `suppress=false`; `member_user.id == user_id`, username не пуст; diagnostics пусты |
| v03 | synthetic leave s=51 | `channel_id=None`, `connected_at=Some(1792395566)`, `member_user=Some`, флаги как в capture |
| v04 | timer numeric s=112, timer nil s=52, status nil s=53, synthetic status `"Music"` | numeric `Some(1792391032)`, nil `None`; status nil `None`, `"Music"` `Some("Music")`; diagnostics пусты |
| v05 | table: нет/битый `guild_id`, `user_id`; `channel_id` bool; bad bools; missing `discoverable`; `request_to_speak_timestamp` string-int | missing/invalid → `Missing`/`InvalidId`; bad bool → default + `ignored`; missing discoverable → `true`; timestamp → `Some` |
| v06 | member string; member без user; member.user.id ≠ user_id; `d` не map | `ignored "voice.member"`, `malformed "voice.member.user"`, inconsistency `voice:<g>:<u>:member-user:<id>`; `member_user=None`; no panic |

## `tests/voice_state_reducer.rs` (8)

| ID | Вход | Ожидание |
| --- | --- | --- |
| v07 | join v02 в minimal State | map len 1; `State.users` содержит member; `Guild.members` не изменён; effects пуст |
| v08 | join → move (другой channel) → leave | после move channel заменён; после leave map пуст; повторный leave — no-op; `duplicates` пуст |
| v09 | `003` supplemental на State | 1 запись; затем `READY` — карты пусты; `last_seq` по READY |
| v09b | live join другого пользователя (`s=1`), затем реальный `003` (`s=2`) | 2 записи; обе с исходными каналами; snapshot аддитивен |
| v09c | synthetic snapshot: два элемента одного `(guild,user)`, разные каналы | last wins; `duplicates` пуст; в store 1 запись |
| v09d | прямой `apply_snapshot` с `channel_id: None` | существующая запись удалена |
| v10 | timer set/clear, status set/empty/clear | set → Some; nil/empty → remove; отрицательный timer не паникует и не оставляет запись; `Some(0)` — валидное epoch-время, остаётся `Some(0)` (QA после мутации V3) |
| v11 | stale s=51 после s=100; malformed `VOICE_STATE_UPDATE` | stale диагностика, map не меняется; malformed диагностика `event:`, `last_seq` не откатывается |

## `tests/voice_state_selectors.rs` (4)

| ID | Вход | Ожидание |
| --- | --- | --- |
| v12 | empty State; один self voice | `self_voice_state=None`, `is_self_in_voice=false`; после join — Some/true |
| v13 | 3 пользователя в одном канале, 1 в другом, 2 гильдии; во второй гильдии тот же `channel_id` | `voice_users_in_channel` по возрастанию user и только своя гильдия; counts 3/1; `voice_guild_count=2` (QA после мутации V5) |
| v14 | connected_at 100, now 100_000, now 99_000, negative, overflow | 0 мс на старте; 99_900; 0 при now<start; `None` для negative; saturating без panic |
| v15 | timer 100, status "Music", другой guild | elapsed 99_900; status Some; отсутствие — None; несколько guild не смешиваются |

## Команды

```bash
cargo test --offline -p discord-core --test voice_state_types --test voice_state_reducer \
  --test voice_state_selectors
```

Red-лог: до I0/I1 модуль не объявлен или variant не существует → compile-red, exit 101;
после I0 — behavior-red на заглушках; оба сохраняются в `waves/RW-19/`.

## Мутации

См. `DESIGN-P2-10.md` §9. QA: 8 мутаций voice, все caught; два выживших сначала закрыты
red-first assertions (V3 zero timer, V5 guild filter). Полная таблица — `waves/B01/MUTATIONS.md`,
логи — `waves/B01/mutations/`. Исходники восстановлены по md5.
