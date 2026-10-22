# TEST-MATRIX P2-11 — typing TTL

Оракулы: 28 `TYPING_START` из `/home/sanya/re-deep/gw-proto/msgs-*.jsonl`, счётчики
`RESEARCH-P2-11.md` §1–§2; fixture `tests/data/typing-run2.jsonl` — 28 минимизированных
строк (копия `waves/RW-20/tests-data/typing-run2.jsonl`, создана из capture). TTL —
`10_000` мс (28/28; `TypingStore` bundle).

Правило: тест сначала падает с exit 101; ожидания не правятся под код. Синтетика —
`tests/support/state_fixture`; fixture читается `include_str!` + `ready_fixture::parse_json`.

## `tests/typing_types.rs` (5)

| ID | Вход | Ожидание |
| --- | --- | --- |
| t01 | synthetic first capture (`idx 117`/`s=192`): guild `942355007271997450`, channel `942355562958573628`, user `1301880740245405707`, timestamp `1792392992`, member.user | поля точные; `member_user.id == user_id`; `received_at_ms` передан; diagnostics пусты |
| t02 | fixture 28 строк через `parse_typing_start` | 28 Ok; 2 уникальные пары (channel/user); 27 у `1434201417467433001/367288240207822858`, 1 у `942355562958573628/1301880740245405707`; все guild `942355007271997450` |
| t03 | `d` только `channel_id`/`user_id` | guild/timestamp/member `None`; diagnostics пусты (DM-форма) |
| t04 | table: missing/bad ids, guild list, timestamp bool, style, member string | `Missing/InvalidId` для обязательных; optional bad → `ignored` по путям; style присутствует → `ignored`, поля нет |
| t05 | member без user, user-id mismatch; `Event::from_frame` vs `from_frame_at(…, 1_000_000)` | `malformed "typing.member.user"` / inconsistency; member_user None при mismatch; from_frame → `received_at_ms=None`, from_frame_at → `Some(1_000_000)` |

## `tests/typing_reducer.rs` (5)

| ID | Вход | Ожидание |
| --- | --- | --- |
| t06 | READY + start `(200,300)` at `1_000_000` | entry; started 1_000_000, expires 1_010_000; map len 1; member upsert; effects/diag пусты |
| t07 | repeat pair at +8_000 | len 1; expiry = new+10_000; active at new+9_999, не active на new+10_000; `duplicates` пуст |
| t08 | 28 fixture, `now=wire_timestamp*1000` | map len 2; оба канала/users; active count 1 на финальном now; пара A истекла |
| t09 | MESSAGE_CREATE автора пары; MESSAGE_CREATE другого автора | первая пара удалена, вторая осталась; `remove_for_message` true/false |
| t10 | READY reset; start без `received_at_ms`; stale s; malformed `d`; prune at expiry | map пуст после READY; no-clock → `ignored "typing.received_at_ms"`; stale/malformed диагностика; prune counts |

## `tests/typing_selectors.rs` (5)

| ID | Вход | Ожидание |
| --- | --- | --- |
| t11 | empty; один self entry at 1_000_000, now 1_009_999/1_010_000 | entry Some/None; is_typing true/false; remaining `Some(1)` на 1_009_999 и `None` на 1_010_000; `TypingEntry::remaining_ms` на границе `Some(0)` |
| t12 | 3 users, 2 entries + 1 expired | ids/users только активные, по возрастанию; users из `State.users` |
| t13 | 2 guilds | `typing_entries_by_guild` точный guild; другой guild пуст |
| t14 | 2 канала, 3 активные, 1 expired | entry_count 3, channel_count 2; boundary now=expires |
| t15 | 28 fixture final at max expiry | store `2 entries, 2 channels, 1 guild`; active selectors на финальном now `1/1/1` (пара A скрыта); `typing_users` не падает на неизвестных |

## Команды

```bash
cargo test --offline -p discord-core --test typing_types --test typing_reducer \
  --test typing_selectors
```

Red: до I0 модуль не объявлен/`EventPayload` отсутствует → compile-red; после I0 — поведение
на заглушках; оба exit 101 сохраняются в `waves/RW-20/`.

## Мутации

`DESIGN-P2-11.md` §9; QA: 8 мутаций typing, все caught (TTL/removal — 100 %).
Полная таблица — `waves/B01/MUTATIONS.md`, логи — `waves/B01/mutations/`; исходники
восстановлены по md5.
