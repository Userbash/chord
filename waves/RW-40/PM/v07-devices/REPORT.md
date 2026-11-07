# Отчёт: V-07-VOICE-DEVICES-CONTRACT — Аудио-устройства, mute/deafen/PTT и hot-plug (`discord-voice`)

- **Карточка / batch:** `RW-40` / `V-07-VOICE-DEVICES-CONTRACT` (`PARITY-MATRIX.md` строка `V-07`, P1)
- **Роль:** Воркер G5 Platform / Voice
- **HEAD:** `4e240481e59ff4d49732ac8bd5ba08a006363748`
- **Контракт-оракул:** `waves/RW-40/PARITY/PARITY-MATRIX.md` (строка `V-07`: `F162:63-83` · `[M]`), `waves/RW-40/QUALITY/G5-VOICE-PLAN.md` (§5.1, §5.4, §6, §7.4), `waves/RW-40/DESIGN/VOICE.md` (§2.4, §3.1–§3.5), `waves/RW-40/DESIGN/voice-spec.json` (`sliders`: `OX2Bnr` 0..=100 = 100, `aUJ062` 0..=200 = 100).
- **Изоляция сборки:** все вызовы `cargo` выполнялись исключительно с `CARGO_TARGET_DIR=/tmp/target-g5-devices` и флагом `--offline`.
- **Границы файлов:** изменены только `rust/crates/discord-voice/src/devices.rs` и `rust/crates/discord-voice/tests/devices_contract.rs` (+ артефакты в `waves/RW-40/PM/v07-devices/`). Файлы `src/lib.rs` (md5 `f365261d5c7322594bb7370a8e213b38` — не тронут), `src/session.rs`, `tests/session_contract.rs` и прочие модули не изменялись.

---

## 1. Сделано

1. **Контрактный тестовый набор (`rust/crates/discord-voice/tests/devices_contract.rs`, RED-first):**
   - `t_v07_01_initial_state_and_device_selection_by_kind` — проверка начального состояния (`VoiceDevicesState::new()`), первичной инициализации default-устройств, явного выбора микрофона (`select_input`) и устройства вывода (`select_output`), отказа при попытке выбрать устройство чужого типа (`DeviceSelectError::WrongKind`), несуществующего (`DeviceSelectError::NotFound`) или пустого `id` (`DeviceSelectError::EmptyId`) без порчи текущего выбора.
   - `t_v07_02_hotplug_preserves_present_device_and_falls_back_to_default_on_removal` — проверка hot-plug (`update_device_list`): сохранение выбранного устройства, если оно остаётся в списке; автоматический откат именно на системное `is_default == true` устройство (даже если в списке первым идёт не-default устройство) с генерацией `DeviceEvent::DeviceFallback { kind, previous_id, fallback_id }`; откат на единственное оставшееся не-default устройство при отсутствии default; сброс в `None` с уведомлением `DeviceEvent::NoDeviceAvailable { kind, previous_id }` при отключении всех устройств данного направления.
   - `t_v07_03_mute_deafen_invariant_forces_effective_mute_and_restores_user_mute` — проверка инварианта `mute/deafen`: при включении `self_deaf = true` эффективный `effective_mute()` (и первая компонента `gateway_mute_deaf()`) автоматически становится `true`, а после снятия `self_deaf = false` возвращается к явному `user_mute` (`false` или `true`), включая смену `user_mute` во время активного `self_deaf`.
   - `t_v07_04_voice_activity_and_ptt_release_delay_and_mute_blocking` — проверка `VoiceInputMode::VoiceActivity { threshold_db }` (передача при `vad_db >= threshold_db`), `VoiceInputMode::PushToTalk { release_delay_ms }` (передача при `key_down == true` и удержание в пределах `release_delay_ms` после отпускания клавиши), а также безусловного блокирования передачи (`is_transmitting == false` и сброс PTT-хвоста) при `effective_mute() == true` (`user_mute` или `self_deaf`) либо при отсутствии выбранного микрофона (`None`).
   - `t_v07_05_volume_clamping_and_privacy_redaction` — проверка регулировки громкости микрофона (`0..=100%`, кламп `> 100` в `100`) и вывода (`0..=200%` по оракулу `aUJ062`, кламп `> 200` в `200`), а также проверка приватности (`G5-VOICE-PLAN.md` §5.1, §7.4 п.4): `Debug`-представление `AudioDevice` и `VoiceDevicesState` редактирует `label` (`<redacted>`) и не выводит названия устройств ОС в логи.

2. **Продуктовая реализация (`rust/crates/discord-voice/src/devices.rs`):**
   - Реализованы структуры и перечисления `AudioDevice`, `AudioDeviceKind::{Input, Output}`, `DeviceEvent::{DeviceFallback, NoDeviceAvailable}`, `DeviceSelectError::{EmptyId, NotFound, WrongKind}`, `VoiceInputMode::{VoiceActivity, PushToTalk}` и `VoiceDevicesState`.
   - Соблюдён запрет `unsafe_code = "forbid"` крейта `discord-voice`: модуль полностью детерминированный, без системного I/O, таймеров и паник.

---

## 2. RED-прогон (до реализации `src/devices.rs`)

Команда:
```bash
CARGO_TARGET_DIR=/tmp/target-g5-devices cargo test --manifest-path rust/Cargo.toml --offline -p discord-voice --test devices_contract > waves/RW-40/PM/v07-devices/red.log 2>&1
```
- **Exit code:** `101`
- **Первая строка ошибки:** `error[E0432]: unresolved imports discord_voice::devices::AudioDevice, discord_voice::devices::AudioDeviceKind, ...` (`crates/discord-voice/tests/devices_contract.rs:9:5`)
- **Артефакт:** `waves/RW-40/PM/v07-devices/red.log`

---

## 3. GREEN-прогон и targeted-проверки

| Команда | Exit code | Результат | Артефакт |
|---|---:|---|---|
| `CARGO_TARGET_DIR=/tmp/target-g5-devices cargo test --manifest-path rust/Cargo.toml --offline -p discord-voice --test devices_contract > waves/RW-40/PM/v07-devices/green.log 2>&1` | `0` | **5 passed; 0 failed; 0 ignored** (`t_v07_01` … `t_v07_05`) | `waves/RW-40/PM/v07-devices/green.log` |
| `CARGO_TARGET_DIR=/tmp/target-g5-devices cargo clippy --manifest-path rust/Cargo.toml --offline -p discord-voice --test devices_contract -- -D warnings > waves/RW-40/PM/v07-devices/clippy.log 2>&1` | `0` | `0 warnings` | `waves/RW-40/PM/v07-devices/clippy.log` |
| `rustfmt --check --edition 2024 rust/crates/discord-voice/src/devices.rs rust/crates/discord-voice/tests/devices_contract.rs` | `0` | чисто | — |

В соответствии с правилом batch (`AGENTS.md` §3) воркер запускает только targeted-проверки своего модуля с изолированным `CARGO_TARGET_DIR=/tmp/target-g5-devices`; полный `gate:worker` запускается один раз в integration-лейне.

---

## 4. Мутационное тестирование (`waves/RW-40/PM/v07-devices/mutations.log`)

Выполнены 5 целевых мутаций в `rust/crates/discord-voice/src/devices.rs` (baseline MD5: `ceaa10efd2729455e76ba05656f2fc98`). Все 5 мутантов убиты (`5/5 KILLED`), после каждой мутации файл восстановлен и сверен по MD5:

| ID | Мутация в `src/devices.rs` | MD5 мутанта | Exit code | Убившие тесты | Вердикт |
|---|---|---|---:|---|---|
| **MUT-1** | `effective_mute()` игнорирует `self_deaf` (`self.user_mute` вместо `self.user_mute \|\| self.self_deaf`) | `d6d5cc0e9fe209259c591e9fde793b26` | `101` | `t_v07_03`, `t_v07_04` | **KILLED** |
| **MUT-2** | `reconcile_kind` при hot-plug откатывается на первый элемент вместо `is_default == true` | `153a42db1bcfb7942419dc92b3fe3622` | `101` | `t_v07_02` | **KILLED** |
| **MUT-3** | `is_transmitting` не проверяет `self.effective_mute()` | `7517f2a32fe1a75ebc6c0afddb8d6466` | `101` | `t_v07_04` | **KILLED** |
| **MUT-4** | В `VoiceInputMode::PushToTalk` игнорируется `release_delay_ms` (`elapsed_ms == 0` вместо `<= release_delay_ms`) | `09cf2e0c0fd0aaa0734382eca68a0c64` | `101` | `t_v07_04` | **KILLED** |
| **MUT-5** | `set_output_volume` клампит по `MAX_INPUT_VOLUME` (`100`) вместо `MAX_OUTPUT_VOLUME` (`200`) | `b28b6b2bb527165d0796061afd60b90d` | `101` | `t_v07_05` | **KILLED** |
| **Restore** | Восстановление эталонного `src/devices.rs` и контрольный прогон | `ceaa10efd2729455e76ba05656f2fc98` | `0` | `5 passed; 0 failed` | **GREEN** |

---

## 5. Файлы и MD5

| Файл | Статус | MD5 |
|---|---|---|
| `rust/crates/discord-voice/src/devices.rs` | modified | `ceaa10efd2729455e76ba05656f2fc98` |
| `rust/crates/discord-voice/tests/devices_contract.rs` | created | `65c2b4b0d6b29e55276046b68f5bae57` |
| `rust/crates/discord-voice/src/lib.rs` | unchanged | `f365261d5c7322594bb7370a8e213b38` |
| `waves/RW-40/PM/v07-devices/red.log` | created | `409600b33aff87c8a8fce784c8e42164` |
| `waves/RW-40/PM/v07-devices/green.log` | created | `ff6f12cb37fa71b94cdf684540190a18` |
| `waves/RW-40/PM/v07-devices/clippy.log` | created | `4632358e89f5433a9536dcf232395a25` |
| `waves/RW-40/PM/v07-devices/mutations.log` | created | `1850df5a939e06e15320ed6ac9f07d0f` |

---

## 6. Риски и открытые границы

1. **Интеграция с системным аудио-бэкендом (ALSA / Portal GlobalShortcuts):** согласно `G5-VOICE-PLAN.md` (§2.3, §5.4, §6), FFI к `libasound` и D-Bus-порталам живёт в `discord-platform`, тогда как `discord-voice::devices` предоставляет чистую детерминированную машину состояний без `unsafe` и системного I/O.
2. **Приватность меток устройств (`G5-VOICE-PLAN.md` §5.1, §7.4 п.4):** поле `AudioDevice::label` доступно для рендеринга в UI, но скрыто (`<redacted>`) в `fmt::Debug`, чтобы исключить попадание описаний аудио-устройств ОС в диагностические логи.
3. **Запреты соблюдены:** `src/lib.rs`, `src/session.rs`, `tests/session_contract.rs`, `discord-core`, `gui-spike` и golden-артефакты не затрагивались; `git commit` не выполнялся.
