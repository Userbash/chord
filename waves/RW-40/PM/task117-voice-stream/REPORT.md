# Отчёт: feature/task-117-voice-stream-wiring — Сквозной мост устройств, RTC-сессии и Stream Gateway opcodes 18..=22

- **Batch / Карточка**: `RW-40 / task-117 (feature/task-117-voice-stream-wiring)` — связка голосовых/видео устройств (`devices.rs`), RTC-сессии (`session.rs`) и Stream Gateway opcodes `18..=22` в крейте `discord-voice`
- **Ветка**: `feature/task-117-voice-stream-wiring` (от `unstable`, базовый `HEAD`: `4e240481e59ff4d49732ac8bd5ba08a006363748`)
- **Роль**: Специалист G5 Voice/Platform (`term_0f0076aa-4af6-4c3e-8f20-efe6a0ecc674`)
- **Контракты и оракулы**:
  - `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md` (§2.1–§2.4, §3.1–§3.2, §4.1–§4.3, решения `D-27-1..D-27-8`)
  - `waves/RW-40/VOICE/stream-opcodes-fixtures.json` (`11` outbound + `9` inbound + `10` stream_keys, `schema_version = 1`)
  - `waves/RW-40/VOICE/V01-SESSION-CONTRACT.md` и `waves/RW-40/DESIGN/VOICE-DEVICES-SETTINGS-SPEC.md` (`V-07`)

---

## 1. Сделано (описание для PR в `unstable`)

1. **Связка выбора аудио/видео устройств и активной RTC-сессии (`VoiceSession` / `Session`)**:
   - В `rust/crates/discord-voice/src/session.rs` добавлены публичный алиас `VoiceSession = Session`, структуры `DeviceSelection` и `AppliedDeviceSelection`, а также встроенный `DeviceManager` (`devices()`, `devices_mut()`, `set_available_devices()`, `device_selection()`, `apply_device_selection()`).
   - Метод `Session::apply_device_selection` проверяет:
     - готовность сессии (`SessionState::Connected` и внутренняя фаза `Phase::Established`, иначе `SessionError::SessionNotEstablished`);
     - строгий диапазон громкости `input_volume` и `output_volume` в пределах `0..=200` (`MAX_OUTPUT_VOLUME`, иначе `SessionError::VolumeOutOfRange { field, value, max: 200 }` без мутации предыдущего состояния);
     - валидность идентификаторов аудиовхода (`AudioInput`), аудиовыхода (`AudioOutput`) и видеокамеры (`video_device_id`, запрет пустого ID, выставление `self_video = video_device_id.is_some()`);
     - инвариант `mute`/`deaf` (`self_deaf = true` принудительно делает `effective_mute = true`, а после `self_deaf = false` восстанавливает пользовательский `user_mute`).
   - В `SessionSnapshot` добавлены поля `selected_input_device_id`, `selected_output_device_id`, `selected_video_device_id`, `input_volume`, `output_volume`, `user_mute`, `self_deaf`, `effective_mute`, `self_video`, `active_stream_key`, `stream_phase`, `stream_transport_ready` с полным редактированием чувствительных идентификаторов (`<redacted>`) в `Debug` и `Display`.

2. **Сквозной мост Stream Gateway opcodes `18..=22` и привязка `stream_key` к `VoiceSession`**:
   - Реализованы канонический парсер/форматер `StreamKey` (`guild:{guild_id}:{channel_id}:{owner_id}` на 4 сегмента и `call:{channel_id}:{owner_id}` на 3 сегмента, валидация u64 snowflake `> 0`, лимит `MAX_STREAM_KEY_BYTES = 128`, редактирование `guild_id` в `Debug`), `StreamKind`, `StreamGatewayOp` (`18 Create`, `19 Delete`, `20 Watch`, `21 Ping`, `22 SetPaused`), `StreamGatewayCommand` с `to_json()` (`D-27-2`: явный `guild_id: null` для `call`), `StreamSecret` (`Debug`/`Display` → `<redacted>`), `StreamLifecyclePhase`, `StreamDeleteReason` (`D-27-7` включая `Unknown(String)`) и `StreamSessionEffect`.
   - В `Session` реализованы методы управления демонстрацией экрана в установленной сессии (`Connected` + `media_ready`):
     - `start_screen_share(preferred_region)` — формирует и привязывает `StreamKey` (`guild:{guild_id}:{channel_id}:{owner_id}` либо `call:{channel_id}:{owner_id}`) к `active_stream_key`, переводит `stream_phase` в `Some(StreamLifecyclePhase::Connecting)` и возвращает `(StreamKey, StreamGatewayCommand::Create)` (`op 18`);
     - `start_call_screen_share(channel_id, preferred_region)` — формирует и привязывает `call:{channel_id}:{owner_id}` с `guild_id: None` (`guild_id: null` в JSON `op 18`);
     - `stop_screen_share()` — отвязывает `active_stream_key` от сессии (`None`), переводит `stream_phase` в `Some(StreamLifecyclePhase::Ended)`, сбрасывает `stream_transport_ready = false` и возвращает `StreamGatewayCommand::Delete` (`op 19`);
     - `watch_stream(raw_stream_key)` (`op 20`), `ping_stream()` (`op 21`), `set_stream_paused(paused)` (`op 22`);
     - `on_stream_dispatch(event_name, d_json)` — обрабатывает серверные op0-диспатчи `STREAM_CREATE`, `STREAM_SERVER_UPDATE` (эмитит `OpenStreamRtc` со скрытым `StreamSecret`, не меняя фазу стрима), `STREAM_UPDATE` (`H-VIEWERS`: `None` сохраняет список зрителей, `Some([])` очищает; `H-PAUSE`: отсутствие `paused` сохраняет прежнюю фазу), `STREAM_DELETE` (строгий приоритет правил `A-REDUCER-DELETE`: `unavailable=true` → `Reconnecting` с сохранением `active_stream_key`; `unauthorized` → `Failed`; `safety_guild_rate_limited` → `Ended` + `ShowSafetyRateLimit`; `stream_full` → `Ended` + `NotifyViewerFull`; `user_requested` после `Failed` сохраняет `Failed`; неизвестный `reason` → `Ended` + `StreamDeleteReason::Unknown`).
     - При терминальном закрытии голосовой сессии (`LeaveRequested`, `1000`, фатальные ошибки) `active_stream_key` и `stream_transport_ready` автоматически очищаются.

3. **Контрактные и интеграционные тесты (`rust/crates/discord-voice/tests/session_contract.rs`)**:
   - `t_v01_17_apply_device_selection_volumes_and_mute_deaf_in_voice_session` — проверяет запрет `apply_device_selection` до `Connected`, выбор аудио/видео устройств, границы громкости `0..=200` (отклонение `201`), инвариант `mute`/`deaf`, hot-plug откат на дефолтное устройство и редактирование ID устройств в `Debug`/`Display`/`SessionSnapshot`.
   - `t_v01_18_stream_key_binding_and_opcodes_18_to_22_match_contract_and_fixtures` — сверяет коды `op 18..=22`, привязку и отвязку `stream_key` при `start_screen_share` / `stop_screen_share` / `start_call_screen_share`, а также все `11` outbound-фикстур и `10` `stream_keys`-фикстур из независимого оракула `waves/RW-40/VOICE/stream-opcodes-fixtures.json`.
   - `t_v01_19_stream_inbound_dispatches_and_delete_priority_rules_follow_contract` — сверяет все `9` inbound-фикстур `STREAM_*` из `waves/RW-40/VOICE/stream-opcodes-fixtures.json`, инварианты `H-VIEWERS`/`H-PAUSE`/`H-SERVER-UPDATE`, приоритеты `A-REDUCER-DELETE` и отсутствие утечек токена `StreamSecret`.

---

## 2. RED-прогон (до реализации в `src/session.rs`)

- **Команда**:
  ```sh
  (cd rust && CARGO_TARGET_DIR=/tmp/target-task117 cargo test --offline -p discord-voice --test session_contract -- --nocapture) > waves/RW-40/PM/task117-voice-stream/red.log 2>&1
  ```
- **Код возврата**: `101` (ошибка компиляции из-за отсутствующих символов `VoiceSession`, `DeviceSelection`, `AppliedDeviceSelection`, `StreamKey`, `StreamGatewayOp`, `StreamGatewayCommand`, `StreamLifecyclePhase`, `StreamDeleteReason`, `StreamSessionEffect` и методов `apply_device_selection`, `start_screen_share`, `stop_screen_share`, `on_stream_dispatch`).
- **Лог**: `waves/RW-40/PM/task117-voice-stream/red.log` (`md5: fada51777c1b212f43af1a889a299182`).

---

## 3. GREEN-прогон, Clippy и Rustfmt

- **Команды**:
  ```sh
  (cd rust && CARGO_TARGET_DIR=/tmp/target-task117 cargo test --offline -p discord-voice --test session_contract -- --nocapture)
  (cd rust && CARGO_TARGET_DIR=/tmp/target-task117 cargo test --offline -p discord-voice)
  (cd rust && CARGO_TARGET_DIR=/tmp/target-task117 cargo clippy --offline -p discord-voice --all-targets -- -D warnings)
  rustfmt --edition 2021 --check rust/crates/discord-voice/src/session.rs rust/crates/discord-voice/tests/session_contract.rs
  ```
- **Результаты**:
  - `session_contract`: `19 passed; 0 failed; 0 ignored` (включая `t_v01_17`, `t_v01_18`, `t_v01_19`)
  - Полный набор тестов крейта `discord-voice`: `6 unit + 16 crypto_vectors + 5 devices_contract + 13 media_contract + 19 session_contract + 11 signaling + 22 srtp_contract + 12 stats_contract + 15 udp = 119 passed; 0 failed`
  - `cargo clippy --offline -p discord-voice --all-targets -- -D warnings`: `0 warnings, exit 0`
  - `rustfmt --edition 2021 --check` (а также `--edition 2024 --check`): `exit 0`
- **Лог**: `waves/RW-40/PM/task117-voice-stream/green.log` (`md5: 2e5460d490e322ec35bf8ce8cdae265e`).

---

## 4. Мутационное тестирование (`5/5 KILLED`)

- **Baseline `rust/crates/discord-voice/src/session.rs` MD5**: `2a84f53056d5398cdb6d27e914780bba`
- **Baseline `rust/crates/discord-voice/tests/session_contract.rs` MD5**: `fcf0410d132703eb87adeb1ba4a3945b`
- **Лог**: `waves/RW-40/PM/task117-voice-stream/mutations.log` (`md5: 6f391d80ec058c2c501a9c536696b920`)

| ID | Мутация в `rust/crates/discord-voice/src/session.rs` | MD5 мутанта | Статус | Убивший тест |
|---|---|---|---|---|
| M1 | `apply_device_selection`: `selection.input_volume > MAX_OUTPUT_VOLUME` → `>= MAX_OUTPUT_VOLUME` (отклонение верхней границы `200`) | `976829c1226f5b42a0991527e190e6c4` | **KILLED** (`exit 101`) | `t_v01_17_apply_device_selection_volumes_and_mute_deaf_in_voice_session` |
| M2 | `apply_device_selection`: `effective_mute = self.devices.effective_mute()` → `selection.user_mute` (игнорирование `self_deaf`) | `a911c18f1c94947de4f09cd78ef95f32` | **KILLED** (`exit 101`) | `t_v01_17_apply_device_selection_volumes_and_mute_deaf_in_voice_session` |
| M3 | `start_screen_share`: `self.active_stream_key = Some(key.clone())` → `None` (отсутствие привязки `stream_key` к сессии) | `02c7ad5816a14960819a6f30eacae41d` | **KILLED** (`exit 101`) | `t_v01_18_stream_key_binding_and_opcodes_18_to_22_match_contract_and_fixtures` |
| M4 | `stop_screen_share`: `self.active_stream_key.take()` → `self.active_stream_key.clone()` (отсутствие отвязки `stream_key` при остановке) | `3deb5c9f8feeef6dc88473e26af4569c` | **KILLED** (`exit 101`) | `t_v01_18_stream_key_binding_and_opcodes_18_to_22_match_contract_and_fixtures` |
| M5 | `on_stream_dispatch("STREAM_DELETE")`: `if unavailable` → `if false && unavailable` (нарушение приоритета `A-REDUCER-DELETE`) | `68d64851a0324d8fe60acc21a0b970e6` | **KILLED** (`exit 101`) | `t_v01_19_stream_inbound_dispatches_and_delete_priority_rules_follow_contract` |

- **После восстановления baseline**: `md5sum` совпал побайтово (`2a84f53056d5398cdb6d27e914780bba`), контрольный прогон `session_contract` — `19 passed; 0 failed` (`RESTORED_TEST_EXIT=0`).

---

## 5. Изменённые и созданные файлы (`md5sum`)

| Файл | MD5 | Назначение |
|---|---|---|
| `rust/crates/discord-voice/src/session.rs` | `2a84f53056d5398cdb6d27e914780bba` | Сквозной мост `DeviceManager` + `VoiceSession` + Stream Gateway opcodes `18..=22` и inbound `STREAM_*` |
| `rust/crates/discord-voice/tests/session_contract.rs` | `fcf0410d132703eb87adeb1ba4a3945b` | Контрактные и интеграционные тесты `t_v01_01..t_v01_19` |
| `waves/RW-40/PM/task117-voice-stream/red.log` | `fada51777c1b212f43af1a889a299182` | Лог RED-фазы (`exit 101`) |
| `waves/RW-40/PM/task117-voice-stream/green.log` | `2e5460d490e322ec35bf8ce8cdae265e` | Лог GREEN-фазы (`cargo test`, `cargo clippy`, `rustfmt --check`) |
| `waves/RW-40/PM/task117-voice-stream/mutations.log` | `6f391d80ec058c2c501a9c536696b920` | Лог мутационного тестирования (`5/5 KILLED` + проверка восстановления MD5) |
| `waves/RW-40/PM/task117-voice-stream/REPORT.md` | *(этот файл)* | Отчёт задачи для PR в `unstable` |

---

## 6. Риски и заметки для интеграции

- Все изменения строго локализованы в разрешённом списке файлов (`rust/crates/discord-voice/src/session.rs`, `rust/crates/discord-voice/tests/session_contract.rs`, `waves/RW-40/PM/task117-voice-stream/*`).
- Форматирование `session.rs` и `session_contract.rs` совместимо как с `rustfmt --edition 2021 --check`, так и с `rustfmt --edition 2024 --check`.
- Реальный захват экрана (`ScreenCaptureKit`/`PipeWire`/`DXGI`) и открытие живого второго RTC-сокета остаются на уровне эффекта `StreamSessionEffect::OpenStreamRtc` / `CloseStreamRtc` согласно `STREAM-GATEWAY-CONTRACT.md` §1.2 и `D-27-4`.
