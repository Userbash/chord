# B01 / I0+I1 — integration report (P2-10 voice, P2-11 typing)

Дата: 2026-10-21. Lane: Integration/QA (TL-2). Воркеры: W19/CORE-1, W20/CORE-2.
Контракты: `waves/RW-19/DESIGN-P2-10.md`, `waves/RW-20/DESIGN-P2-11.md` (не менялись).

## 1. Что применено

### I0 — store scaffolding

`state/mod.rs`:

- `pub mod voice; pub mod typing;`
- re-exports voice/typing типов, парсеров/reducer/селекторов и `TYPING_TTL_MS`.

`state/types.rs`:

- `State`: `voice_states: BTreeMap<VoiceKey, VoiceState>`,
  `voice_channel_start_times: BTreeMap<VoiceChannelKey, i64>`,
  `voice_channel_statuses: BTreeMap<VoiceChannelKey, String>`,
  `typing: BTreeMap<TypingKey, TypingEntry>`;
- `Debug` — только длины;
- `State::from_ready` — пустые карты;
- `ReadySupplemental.voice_states: Vec<VoiceState>`;
- `parse_ready_supplemental` вызывает `parse_voice_states` для
  `d.guilds[i].voice_states` с parent `guild_id` и точным diagnostic path.

### I1 — event API

`state/types.rs`:

- `EventPayload::{VoiceStateUpdate, VoiceChannelStartTime, VoiceChannelStatus, TypingStart}`;
- `Event::from_routed` routes:
  - `VOICE_STATE_UPDATE` → `parse_voice_state_update`,
  - `VOICE_CHANNEL_START_TIME_UPDATE` → `parse_voice_channel_start_time`,
  - `VOICE_CHANNEL_STATUS_UPDATE` → `parse_voice_channel_status`,
  - `TYPING_START` → `parse_typing_start(data, received_at_ms)`;
- `Event::from_frame_at(frame, now_ms)` — `received_at_ms=Some(now_ms)`;
  `from_frame`/`from_routed` — `None`.

`state/reducer.rs`:

- match arms для четырёх payload → voice/typing reducers;
- `merge_supplemental` → `voice::reducer::apply_snapshot`;
- `MESSAGE_CREATE` берёт `author.id` до move, вставляет сообщение, затем
  `typing::reducer::remove_for_message(channel_id, author_id)`;
- `apply_frame_at(state, frame, now_ms)` — stale-проверка до разбора, затем
  `Event::from_frame_at` + `apply`; ошибки остаются диагностикой.

`src/lib.rs` — root re-exports voice/typing типов, `TYPING_TTL_MS`, `apply_frame_at`.

## 2. Shared tests: адаптация после типизации

Типизация `TYPING_START` изменила ожидания трёх старых shared-тестов; тесты не «позеленены»
под реализацию, а переведены на другой всё ещё `Unsupported` тип:

| файл | было | стало |
| --- | --- | --- |
| `tests/state_reducer.rs` | `ReadySupplemental` без нового поля | `voice_states: Vec::new()` |
| `tests/state_reducer.rs` r10 | `TYPING_START` как Unsupported | `VOICE_SERVER_UPDATE` как Unsupported |
| `tests/state_types.rs` t08 | `TYPING_START` как Unsupported | `VOICE_SERVER_UPDATE` как Unsupported |

Причина: `TYPING_START` теперь типизирован и требует `channel_id`/`user_id`; пустой `d`
падает `Missing`, а не даёт `Unsupported`. `VOICE_SERVER_UPDATE` остаётся известным
не-поддержанным типом P2-02, поэтому проверка `Unsupported` сохранена.

## 3. Red → green

Первый прогон полного core после I0/I1:

- `integration-red-e0063.log` — `E0063 missing field voice_states` в shared
  `tests/state_reducer.rs:433`, exit 101.
- `integration-red-r10.log` — r10 ожидал `unsupported_events=["TYPING_START"]`, получил `[]`,
  exit 101.
- `integration-red-existing-tests.log` — t08 ожидал `Unsupported(TypingStart)`, получил
  `Missing("typing.channel_id")`, exit 101.

После трёх shared-правок:

- `logs/targeted-final.log` — **30/30**, exit 0 (voice 15 + typing 15);
- `logs/core-503.log` — **503**, exit 0;
- `logs/workspace-545.log` — **545**, exit 0;
- `logs/clippy.log` — `cargo clippy --offline --workspace --all-targets -- -D warnings`,
  exit 0;
- `logs/fmt-final.log` — `cargo fmt --all -- --check`, exit 0 (пустой diff).

`logs/fmt.log` — исторический red: до `cargo fmt` был diff в `reducer.rs`/`types.rs`;
после форматирования `fmt-final.log` пуст. Форматирование не меняло семантику.

## 4. QA-правки после мутаций

Мутации нашли две непокрытые ветви (подробно `MUTATIONS.md`):

1. V3: `voice_start_time = Some(0)` ошибочно трактовался как очистка. В v10 добавлены
   проверки нуля как валидного epoch-времени.
2. V5: `voice_users_in_channel` не проверял guild при совпадении `channel_id` в двух
   гильдиях. В v13 добавлен такой же `channel_id` во второй гильдии и проверки guild-split.

Оба теста сначала падают на мутанте (exit 101), затем зелёные на восстановленном коде:
`mutations/V3-red-after-new-test.log`, `mutations/V5-red-after-new-test.log`. Число тестов
не изменилось (15 voice), добавлены assertions.

Также integration-правка `voice/parser.rs`: clippy `redundant_guards` —
`Some(text) if text.is_empty()` → `Some("")`. Поведение не изменилось; md5 в отчёте W19
обновлён.

## 5. Мутации

`16/16` caught: 8 voice + 8 typing. Все мутанты восстановлены из
`/tmp/opencode/b01-mutation-backup`; после каждого восстановления md5 сверялся. Сводка —
`mutations/summary.txt`, таблица — `MUTATIONS.md`.

## 6. Файлы + md5 (на момент integration)

### Worker product/tests

```
e4a0f720e269799fe19f03c41d77b084  src/state/voice/mod.rs
cb4e8744ddf3da2c77896a2e57e4a7f5  src/state/voice/parser.rs
8e3ff3ad7d2016463847101e1336973b  src/state/voice/reducer.rs
7ebc28eb2989bc63d58e3bde120840c7  src/state/voice/selectors.rs
5992af731202361ba19158114367f4d9  src/state/voice/types.rs
9d019883be4cef4cd651f97bfe1febe1  tests/voice_state_reducer.rs
f639cf6b027e828fb9d4397d8db7002e  tests/voice_state_selectors.rs
9fc7f6633f878a6efb64dfd4f636bb82  tests/voice_state_types.rs
47f6714bde6a90db968cd93eb7f04b72  src/state/typing/mod.rs
f0c31339a80070723028b73c0ae26c62  src/state/typing/parser.rs
0f6d580c9b85007e145772d4fbe3b839  src/state/typing/reducer.rs
7815aa3ee56e3060d4c63cca99e20fbf  src/state/typing/selectors.rs
e78c66f3bc451247550a28d22fc3a337  src/state/typing/types.rs
6d38c4565f02768d2425799848f273ea  tests/typing_reducer.rs
650d67216dd31353db0fa852b4ca29a6  tests/typing_selectors.rs
1cb8621d0f20798206717d4d23982333  tests/typing_types.rs
71526755701a7e1d33061ae8a5d6ea55  tests/data/typing-run2.jsonl
```

### Shared (TL-2)

```
6ef1430b25d27fc9978510687b4e5b66  src/state/mod.rs
9fede799374d8f8973fe9c3cbbb6f533  src/state/types.rs
3d62a69917829c4c6d5c497bfb06c226  src/state/reducer.rs
cc1258f8e7fff616661246018b7836df  src/lib.rs
a40fdb0750ab6717792afa35f7694876  tests/state_reducer.rs
a173f073cdf0e36fb84e877ccd34a604  tests/state_types.rs
```

## 7. Ограничения / риски

- Live Discord, исходящий voice/typing, сеть, новые зависимости, SQLite/UI/keyring не
  использовались.
- `waves/RW-20/` был удалён внешним изменением во время работы; каталог восстановлен из
  контекста W20 (DESIGN/TZ/MATRIX/RESEARCH/fixture), md5 fixture совпал
  (`71526755701a7e1d33061ae8a5d6ea55`). Это incident процесса, не код; подробнее в отчёте W20.
- P3-замечания W19/W20 (upsert только при совпадении id, saturating/checked, отсутствие cap
  typing-store) остаются backlog-only; P0/P1/P2 не заявляются до независимого review.
- `waves/B01/BATCH-TZ.md` §5 ожидал 503/545 — подтверждено.

## 8. Что осталось

1. `npm run gate:worker` один раз на batch, артефакт в `waves/B01/`.
2. Два независимых review (P2-10, P2-11) параллельно; P0/P1/P2 — red-first.
3. Приёмка Team Lead, затем один раз доки/журнал.

## 9. Review addendum (2026-10-21)

- P2-10 review round 1: P2-1 (snapshot additivity/last-wins/remove не покрыты). Закрыто
  red-first тестами v09b/v09c/v09d и мутантами clear-before-loop/first-wins; матрица
  расширена `RW-19/TEST-CORRECTIONS.md` C4. Round 2 — **accepted**, P0/P1/P2=0, P3=11.
- P2-11 review: **accepted**, P0/P1/P2=0, P3=10.
- Итоговые targeted: voice **18/18**, typing **15/15**, всего **33/33**; core **506**,
  workspace **548**; fmt/clippy exit 0; gate passed
  `.gate/worker-2026-10-20T20-31-37-021Z.json` (копия `waves/B01/gate-worker.json`).
- Мутации: **18/18** (16 базовых + 2 snapshot-мутанта). `tests/voice_state_reducer.rs`
  md5 `9d019883be4cef4cd651f97bfe1febe1`; продуктовые voice-файлы не менялись.

