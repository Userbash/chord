# Отчёт воркера: TASK-633 — Аппаратный обработчик Push-to-Talk, шумоподавление (Krisp / RNNoise / WebRTC) и автомат микрофона в `discord-voice`

- **Batch / Карточка**: `RW-40` / `TASK-633` (`G5 Voice & Platform`)
- **Ветка**: `feature/task-633-hardware-ptt-voice`
- **Воркер**: `term_8dfcbc70-27d9-465f-9c23-bec1a7f85336` (`task_e7cdbc6e5543`, `ctx_5359760a8ac1`)

## 1. Сделано (сверка с RE-оракулами `/home/sanya/re-deep/`)

1. **Модуль Push-to-Talk и автомат микрофона (`rust/crates/discord-voice/src/ptt.rs`)**:
   - **Оракулы RE**:
     - `/home/sanya/re-deep/voice-deep.md` (§3–4) и `/home/sanya/re-deep/native-re/raw/discord_voice/{nm-all-demangled.txt,rabin2-strings.txt}` (`discord::PttKeyState`, `discord::AudioModePTT`, `discord::AudioModeType` `MODE_UNINIT`/`MODE_VAD`/`MODE_PTT`, `LocalUser::UpdateMuteState`, `LocalUser::UpdateSpeaking`, `SetOnSpeakingWhileMutedCallback`, `SetNoAudioInputThreshold`).
     - `/home/sanya/re-deep/native-re.md` (§5.2–5.4, `discord_utils.node` `src/input_events.cpp`, `src/input_events_linux.cpp`): `InputEventRegister`, `InputEventUnregister`, `InputEventServiceSetAllowed`, `InputSetFocused`, `OnSystemInputEvent`, `DeviceIdRegex = "^([0-9]+):([0-9]+)$"`, флаги `keydown`/`keyup`/`focused`/`blurred`, лимит `MAX_KEYBIND_COMBO_LEN = 4`, таймаут `5000` мс, запрет ЛКМ (код 1) и одиночных буквенных скан-кодов `[24..=33]`, `[38..=46]`, `[52..=58]` на Linux.
     - `waves/RW-40/QUALITY/G5-VOICE-PLAN.md` (§3.3, §6): `org.freedesktop.portal.GlobalShortcuts` (`Activated`/`Deactivated`) и индикация `PttAccessStatus::Blocked` при потере фокуса без портала.
   - Реализованы `PttKeyState` (состояния `Inactive`, `Active`, `ReleaseHangover { remaining_ms }`, `release_delay_ms` в границах `0..=2000` мс, `latch`-режим, приоритетный голос `SpeakingFlags::MICROPHONE | SpeakingFlags::PRIORITY`), `ScanCodeCombo`, `InputDeviceId`, `HardwarePttKeybindHandler` и конечный автомат микрофона `MicStateMachine` (`Uninitialized`, `DeviceUnavailable`, `Muted`, `SpeakingWhileMuted`, `GateClosed`, `Speaking`, `PrioritySpeaking`, `PttReleaseHangover`, `NoAudioInput`).

2. **Модуль шумоподавления и VAD-гейтинга (`rust/crates/discord-voice/src/noise.rs`)**:
   - **Оракулы RE**:
     - `/home/sanya/re-deep/voice-deep.md` (§3, строка 110: `[VOICE_DISCONNECT] krisp_nc_model: krisp-nc-o-med-v7.kef`).
     - `/home/sanya/re-deep/native-re.md` (§4.1 `discord_krisp.node`: `.kef`-модели `krisp-nc-o-med-v7.kef`, `krisp-nc-o-lite-v1.kef`, `krisp-nc-o-nb-v2.kef`, `krisp-bvc-o-pro-v3.kef`, `krisp-vad-o-v2.kef`, `setSuppressionLevel` `0..=100`).
     - `/home/sanya/re-deep/native-re/raw/discord_voice/rabin2-strings.txt` (`src/krisp/noise_canceller.cpp`, `src/krisp/voice_activity_detector.cpp`, `src/media/captured_audio_processor.cpp`): тиры `"small_NC_8k"`, `"small_NC"`, `"full_NC"`, жизненный цикл `NotInitialized -> Initialized -> Configured -> DisabledByCpuOveruse`, диагностические строки ошибок движка, пропуск инференса при mute (`skipNoiseCancellationIfMuted`), откат на `RnNoise` при перегрузке CPU (`"Noise canceller CPU overuse"`) или `sampleRateMismatchPercent > 15%`, и JSON-телеметрия сессии `NoiseCancellationStats::to_json()`.
   - Реализованы расчёт RMS уровня сигнала в dBFS (`compute_frame_rms_dbfs`), подавление фонового шума в дБ (`attenuate_pcm_s16_in_place`), классификация корзин шума (`classify_noise_band`) и полный конвейер `NoiseSuppressionPipeline` с `vad_leading_frames` / `vad_trailing_frames` hangover-гейтингом.

3. **Экспорты и план (`rust/crates/discord-voice/src/lib.rs`, `TDD-PLAN.md`)**:
   - Подключены модули `pub mod noise;` и `pub mod ptt;` с публичным реэкспортом ключевых типов в `discord_voice`, обновлён раздел `### RC` в `TDD-PLAN.md`.

## 2. Red-прогон (Strict TDD)

- **Команда**:
  `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/Cargo.toml -p discord-voice --test ptt_contract --test noise_contract`
- **Лог**: `waves/RW-40/G5/task-633-hardware-ptt-voice/red.log` (`error[E0432]: unresolved imports ... could not compile discord-voice (test "ptt_contract") / (test "noise_contract")`).

## 3. Green-прогон и Targeted / Gate проверки

- **Команда**:
  `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/Cargo.toml -p discord-voice`
- **Лог**: `waves/RW-40/G5/task-633-hardware-ptt-voice/green.log`
  - `ptt_contract`: `5 passed; 0 failed` (`t_ptt_01`..`t_ptt_05`)
  - `noise_contract`: `4 passed; 0 failed` (`t_noise_01`..`t_noise_04`)
  - Все тестовые таргеты `discord-voice` (`lib`, `crypto_vectors`, `devices_contract`, `media_contract`, `noise_contract`, `ptt_contract`, `reception_contract`, `reconnect_contract`, `regions_contract`, `rtcp_contract`, `rtp_video_contract`, `signaling`, `srtp_contract`, `stats_contract`, `udp`, `voice_call_state_machine_contract`): **100% PASS (0 failed)**.
- **Форматирование и линтер**:
  - `rustfmt --edition 2024 --check rust/crates/discord-voice/src/ptt.rs rust/crates/discord-voice/src/noise.rs rust/crates/discord-voice/src/lib.rs rust/crates/discord-voice/tests/ptt_contract.rs rust/crates/discord-voice/tests/noise_contract.rs` — **OK (0 diff)**.
  - `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target flock /tmp/discord-lab-cargo.lock cargo clippy --manifest-path rust/Cargo.toml -p discord-voice --all-targets -- -D warnings` — **OK (0 warnings)**.
  - `node --test tests/meta/plan-sync.test.mjs tests/meta/rust-client-sync.test.mjs` — **19/19 PASS**.

## 4. Мутационное тестирование (`5/5 = 100% Killed`)

- **Лог**: `waves/RW-40/G5/task-633-hardware-ptt-voice/mutations.log`
  1. **M1 (`ptt.rs`)**: `PttKeyState::activation_state` — замена граничного условия `elapsed <= delay` на `elapsed < delay` → **KILLED** (`t_ptt_02_ptt_key_state_release_delay_boundaries_and_priority_flags`).
  2. **M2 (`ptt.rs`)**: `PttKeyState::speaking_flags` — отключение выставления бита `SpeakingFlags::PRIORITY` при `wants_priority && priority_speaker_allowed` → **KILLED** (`t_ptt_02_ptt_key_state_release_delay_boundaries_and_priority_flags`, `t_ptt_05_mic_state_machine_mute_deafen_speaking_while_muted_and_no_input`).
  3. **M3 (`ptt.rs`)**: `MicStateMachine::evaluate_frame` — обход проверки `self.is_effectively_muted()` → **KILLED** (`t_ptt_05_mic_state_machine_mute_deafen_speaking_while_muted_and_no_input`).
  4. **M4 (`noise.rs`)**: `NoiseSuppressionPipeline::process_frame` — сдвиг порога открытия VAD-гейта `leading_streak >= vad_leading_frames` на `>` → **KILLED** (`t_noise_03_pcm_attenuation_in_db_and_vad_hangover_gating`).
  5. **M5 (`noise.rs`)**: `attenuate_pcm_s16_in_place` — отключение ослабления фонового шума в дБ (`gain = 1.0`) → **KILLED** (`t_noise_03_pcm_attenuation_in_db_and_vad_hangover_gating`).
- Все файлы восстановлены после мутаций, контрольные суммы `md5sum` совпадают с эталоном (`matches baseline: true`).

## 5. Изменённые файлы и MD5

| Файл | MD5 |
|---|---|
| `rust/crates/discord-voice/src/ptt.rs` | `976860bd3f7da6d7300b8f03d6fb93b7` |
| `rust/crates/discord-voice/src/noise.rs` | `3faeb35b4ce047559ee7d807fa27a5e5` |
| `rust/crates/discord-voice/src/lib.rs` | `31c38271cbf2fa7292814756b20086bc` |
| `rust/crates/discord-voice/tests/ptt_contract.rs` | `d693c52bac679e440660b76ca5b182bf` |
| `rust/crates/discord-voice/tests/noise_contract.rs` | `dedaf1134e5eddbad38cb9c6b32f47cf` |
| `TDD-PLAN.md` | `249088640ab901ad63af85b65a5f5276` |
| `waves/RW-40/G5/task-633-hardware-ptt-voice/red.log` | `acc1e98d209f08dd373c4b4525fff693` |
| `waves/RW-40/G5/task-633-hardware-ptt-voice/green.log` | `44fb88e9a7221c2fce64311049bb81f8` |
| `waves/RW-40/G5/task-633-hardware-ptt-voice/mutations.log` | `851b910db9cfd7283533a64379a3012a` |

## 6. Риски и ограничения

- Модули `ptt.rs` и `noise.rs` реализованы на чистом безопасном Rust (`#![forbid(unsafe_code)]`) без внешних сетевых/системных вызовов и полностью детерминированы.
- Реальный захват глобальных хоткеев на Wayland в runtime зависит от наличия интерфейса `org.freedesktop.portal.GlobalShortcuts` в окружении рабочего стола; при его отсутствии и потере фокуса окна обработчик детерминированно переходит в `PttAccessStatus::Blocked`.
