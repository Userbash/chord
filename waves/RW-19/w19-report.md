# W19 / P2-10 — отчёт воркера (memory-only voice states)

- **batch/карточка:** B01 / RW-19 / P2-10 (worker CORE-1).
- **контракт:** `DESIGN-P2-10.md`, `TEST-MATRIX-P2-10.md` — заморожены, не менялись.
- **статус:** **targeted green 15/15, EXIT=0 в рабочем дереве.** I0/I1 TL-2 применил в ходе
  работы (см. «Интеграция»); воркер shared-файлы не правил. Shared-фикс `state_reducer.rs`
  применён TL-2, полный core/workspace зелёный; остаются batch gate и независимое review.

## Сделано

1. **Тесты (red-first)** — 15 кейсов, ровно по матрице:
   - `tests/voice_state_types.rs` — v01…v06 (снимок `003`, live join s=111, leave s=51,
     timer/status s=112/52/53, таблица опциональных/битых полей, member/user mismatch).
   - `tests/voice_state_reducer.rs` — v07…v11 (join, move/leave last-wins, snapshot→READY reset,
     timer/status границы, stale/malformed).
   - `tests/voice_state_selectors.rs` — v12…v15 (self voice, users in channel + counts,
     `voice_connected_for_ms` границы, channel timer/status).
   - Оракулы: реальный `003-READY_SUPPLEMENTAL.json` через `Event::from_frame`, synthetic-кадры
     через `tests/support/state_fixture`, live-значения s=111/51/112/52/53 из
     `RESEARCH-P2-10.md` §2. Ожидания не подгонялись под код.
2. **Product-модуль** `state/voice/` (5 файлов, только новые):
   - `types.rs` — `VoiceKey`, `VoiceChannelKey`, `VoiceState` (+ `connected_at_ms`,
     `is_connected`), `VoiceStateUpdate`, `VoiceChannelStartTime`, `VoiceChannelStatus`.
   - `parser.rs` — `parse_voice_states` (`ready_supplemental.guilds[i].voice_states`),
     `parse_voice_state_update`, `parse_voice_channel_start_time`, `parse_voice_channel_status`;
     диагностика `malformed`/`ignored`/`inconsistencies` с точными путями.
   - `reducer.rs` — `apply_snapshot` (last-wins по `(guild,user)`, merge `member_user` в
     `State.users` через `state::reducer::upsert_user` при совпадении id), `apply_update`
     (join/move/leave, stale по `last_seq`), `apply_start_time`, `apply_status`.
   - `selectors.rs` — `self_voice_state`, `is_self_in_voice`, `voice_users_in_channel`,
     `voice_channel_user_count`, `voice_guild_count`, `voice_connected_for_ms`
     (checked/saturating), `voice_channel_start_time`, `voice_channel_status`.
   - `mod.rs` — объявление/ре-экспорты.
3. **Проверки:** `rustfmt --check` по 8 своим файлам — exit 0; в product-коде нет
   `unwrap/expect/panic!/unreachable!`; полный workspace/clippy/gate не запускались
   (запрет ТЗ, batch-слот). `cargo` — только последовательно под
   `flock /tmp/opencode/discord-cargo.lock`.

## Интеграция (TL-2, не воркер)

Пока шла работа, TL-2 применил I0/I1 в shared-файлах; воркер их не касался:

- `state/mod.rs`: `pub mod voice;` + ре-экспорты;
- `state/types.rs`: поля `State.voice_states` / `voice_channel_start_times` /
  `voice_channel_statuses`, `Debug`, инициализация в `from_ready`,
  `ReadySupplemental.voice_states`, сбор `voice_states` в `parse_ready_supplemental`,
  варианты `EventPayload::{VoiceStateUpdate, VoiceChannelStartTime, VoiceChannelStatus}` и arms
  в `Event::from_routed`;
- `state/reducer.rs`: arms в `apply_event`, `merge_supplemental` →
  `voice::reducer::apply_snapshot`.

Совпадение с reference-диффом `w19-simulated-integration.diff` — по голосовой части полное.

## Red

| лог | команда | результат |
| --- | --- | --- |
| `waves/RW-19/w19-compile-red.log` | targeted-команда до product-кода | compile-red, **EXIT=101**, `E0432 unresolved import discord_core::state::voice` |
| `waves/RW-19/w19-compile-red-final.log` | targeted-команда после product-кода, до I0/I1 | compile-red, **EXIT=101**, 37 ошибок (`E0432`, `E0599` для `EventPayload::VoiceStateUpdate`, `State::voice_*`) |

Отдельный behavior-red-лог не снимался: до I0 модуль не объявлен, до I1 нет вариантов
`EventPayload`/методов `State`, поэтому поведенческий прогон недостижим. Матрица допускает
compile-red **или** behavior-red; зафиксирован compile-red с exit 101 (дважды).

## Green / targeted

- **Рабочее дерево, после I0/I1:**
  `cargo test --offline -p discord-core --test voice_state_types --test voice_state_reducer --test voice_state_selectors`
  → **EXIT=0, 15/15 passed** (6 + 5 + 4); лог `waves/RW-19/w19-green-integrated.log`.
- Ранее тем же кодом в изолированной копии `/tmp/opencode/w19-check/rust` (симуляция I0/I1,
  repo не изменялся) — **EXIT=0, 15/15**, лог `waves/RW-19/w19-green-targeted.log`.
  Совпадение двух прогонов подтверждает, что зелёный не зависит от копии.

## gate:worker

Не запускался — по ТЗ batch воркер выполняет только targeted; полный `gate:worker` — у
Integration/QA на batch. **Для batch-гейта нужно:** добавить `voice_states: Vec::new()` в
struct literal `ReadySupplemental` в `tests/state_reducer.rs:433` (shared test, integration
lane) — сейчас `cargo test --test state_reducer --no-run` падает с `E0063` (проверено,
EXIT=101). Это единственное известное препятствие для полного suite.

## Файлы + md5

| файл | md5 |
| --- | --- |
| `rust/crates/discord-core/src/state/voice/mod.rs` | `e4a0f720e269799fe19f03c41d77b084` |
| `rust/crates/discord-core/src/state/voice/types.rs` | `5992af731202361ba19158114367f4d9` |
| `rust/crates/discord-core/src/state/voice/parser.rs` | `cb4e8744ddf3da2c77896a2e57e4a7f5` |
| `rust/crates/discord-core/src/state/voice/reducer.rs` | `8e3ff3ad7d2016463847101e1336973b` |
| `rust/crates/discord-core/src/state/voice/selectors.rs` | `7ebc28eb2989bc63d58e3bde120840c7` |
| `rust/crates/discord-core/tests/voice_state_types.rs` | `9fc7f6633f878a6efb64dfd4f636bb82` |
| `rust/crates/discord-core/tests/voice_state_reducer.rs` | `66b82ef2e1d2dfc068fd478c9ed39120` |
| `rust/crates/discord-core/tests/voice_state_selectors.rs` | `f639cf6b027e828fb9d4397d8db7002e` |

Артефакты волны: `w19-compile-red.log`, `w19-compile-red-final.log`, `w19-green-targeted.log`,
`w19-green-integrated.log`, `w19-simulated-integration.diff`, этот отчёт. Вне списка ТЗ
изменений нет; Cargo/package, доки, golden, live — не трогались, коммитов нет.

## Риски / вопросы

- **P1 (закрыт integration):** `tests/state_reducer.rs:433` — shared struct literal без
  `voice_states`; TL-2 добавил `voice_states: Vec::new(),`. Полный core/workspace зелёный
  (`waves/B01/INTEGRATION.md`), targeted 15/15 не изменился.
- **P2 (v01):** матрица требует «diagnostics пусты», но фикстура `003` содержит дубль
  `289091818442719242` в `merged_members[0]`, и P2-01 штатно пишет `duplicates`. Тест v01
  проверяет пустоту именно voice-диагностики (`malformed`/`ignored`/`inconsistencies` пусты,
  `duplicates` без voice-путей). Если TL требует буквально пустой `StateDiagnostics` — нужна
  правка P2-01 или фикстуры вне границ P2-10.
- **P3:** `apply_update` мержит `member_user` в `State.users` только при
  `member_user.id == voice.user_id` (по §1 «upsert»); при mismatch — только inconsistency, без
  записи в users. `voice_connected_for_ms` при переполнении `connected_at * 1000` возвращает
  `None` (DESIGN допускал checked/saturating).
- **P3:** negative `connected_at` трактуется как `None` в селекторе (v14), запись при этом
  сохраняется — соответствует матрице.
- **Наблюдение по параллелизму (закрыто):** во время работы shared-файлы правились
  параллельно (в `state/mod.rs` временно был ре-экспорт несуществующего `apply_frame_at`, в
  `types.rs` — незавершённый typing-патч). К финальному прогону lib собирается; воркер эти
  файлы не трогал. Integration/QA прогнал targeted на замороженном срезе — 30/30.

## QA integration addendum (TL-2)

- `voice/parser.rs`: clippy `redundant_guards` — `Some(text) if text.is_empty()` → `Some("")`;
  поведение то же, md5 обновлён.
- Мутации нашли две непокрытые ветви: V3 (`voice_start_time=Some(0)`) и V5 (одинаковый
  `channel_id` в двух гильдиях). В v10/v13 добавлены assertions; red на мутантах — exit 101,
  затем green на восстановленном коде. Число тестов осталось 15.
- Полный core **503**, workspace **545**, clippy/fmt exit 0; см.
  `waves/B01/INTEGRATION.md`, `waves/B01/MUTATIONS.md`.

## Review addendum (2026-10-21)

- P2-1 (`waves/reviews/review-p2-10.md`): добавлены `v09b` (snapshot поверх непустого store,
  2 записи), `v09c` (дубликат `(guild,user)` — last wins), `v09d` (прямой `channel_id: None`
  удаляет запись). Матрица расширена через `TEST-CORRECTIONS.md` C4; reducer-тестов 8,
  voice targeted 18.
- Red-first доказан мутантами: `clear-before-loop` → v09b exit 101; `first-wins` → v09c
  exit 101; логи в `waves/B01/mutations/p2-10-*.log`, продукт восстановлен по md5
  `8e3ff3ad7d2016463847101e1336973b`.
- Продуктовый код не менялся; md5 reducer/selectors/parser прежние.

## Следующий шаг


Batch `gate:worker` на замороженном срезе, затем два независимых review (P2-10/P2-11) и
приёмка Team Lead. После accepted — один раз доки/журнал.
