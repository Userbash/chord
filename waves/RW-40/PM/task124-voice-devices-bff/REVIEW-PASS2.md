# Ревью Pass 2 — `RW-40 / task-124`, BFF голосовых устройств (`voice_devices.rs`)

**Вердикт: PASS.** Замечаний P0/P1/P2 нет; открытые пункты — только P3 (не блокируют).

- Ревьюер: независимый технический ревьюер DeepSeek (Review 46B), карточка `task_d83387f31eb7` / dispatch `ctx_dfc6c2a1e4bd`.
- Дата: 2026-10-30. Проверялся чек-лист §5 ревью 46A (`REVIEW.md`) после правок Pass 2.
- Ветка: `feature/task-124-voice-devices-tauri-bff`, базовый коммит `4e24048` (коммитов в ветке нет; работа в рабочем дереве).
- Ревьюируемые файлы, md5 на момент проверки:
  - `rust/gui-spike/src/voice_devices.rs` — `8c4a072084bc33c888dd06bbd8eca89c` (1460 строк; было `08560cb9…`, 1453);
  - `rust/gui-spike/tests/voice_devices_contract.rs` — `b56c7f3c8e8dc9709e0d51ebacffbd22` (1292 строки, 15 тестов; было `4be0edeb…`, 1177 строк, 14 тестов).

## 1. Гейты

| Проверка | Команда | Результат |
|---|---|---|
| Контрактные тесты | `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test voice_devices_contract` | **15 passed, 0 failed** (лог `REVIEW-PASS2-test.log`), повторено после мутационных прогонов — снова 15/15 |
| Clippy по карточке | `cargo clippy … --test voice_devices_contract -- -D warnings` | **EXIT=0**, чисто (лог `REVIEW-PASS2-clippy.log`) |
| Форматирование | `rustfmt --edition 2021 --check …` | **EXIT=0** |
| Мутации | 7 новых + 3 из ревью 46A, каждая на копии baseline с восстановлением и сверкой md5 | **10/10 KILLED** (лог `REVIEW-PASS2-mutations.log`) |
| Приватность (независимо) | проба 46A `REVIEW-privacy-probe.rs` против новой версии | **PASS**: `notice: Some(VoiceNotice { level: Status, message: "<redacted>" })` (лог `REVIEW-PASS2-privacy-probe.log`) |

Замечание по батчу, не по карточке: пакетный clippy остаётся красным из-за `tests/net_contract.rs:782` и `src/main.rs:1703` (чужие карточки, `type_complexity`); в выводе нет ни одного замечания по `voice_devices.rs` / `voice_devices_contract.rs`.

## 2. Чек-лист §5: что закрыто

### P1-1 — утечка меток через `notice` — закрыто

`VoiceNotice` получил приватное поле `origin: VoiceNoticeOrigin` (`src/voice_devices.rs:246-252`): `Static` — текст из литералов модуля, `DeviceLabels` — текст с системными метками. `VoiceNotice::device_fallback` (`:285-293`) выставляет `DeviceLabels`, а `impl fmt::Debug for VoiceNotice` (`:254-263`) печатает для этого варианта `"<redacted>"` вместо `message`. `VoiceDevicesState` печатает `notice` через этот `Debug` (`:530`), так что метки не попадают в диагностику ни через `devices`, ни через `notice`.

Новый тест `t15_debug_after_device_fallback_does_not_leak_os_device_labels` (`tests/voice_devices_contract.rs:1238-1292`) доводит состояние до реального `DeviceFallback`, проверяет, что DOM-текст баннера метки сохраняет, и что `Debug` не содержит ни «USB Studio Microphone», ни «Built-in Microphone», но содержит `<redacted>`. Мутация P2-M1 (вернуть сырой `message` в `Debug`) убивается именно t15 — тест не декоративный.

### P1-2 — границы VAD — закрыто

- `MIN_VAD_THRESHOLD_DB = -128`, `MAX_VAD_THRESHOLD_DB = 127` (`src/voice_devices.rs:29,32`) — совпадает со спекой §1.1/§1.2 и с `voice-devices-settings.js:45,48`.
- Рабочее окно шкалы чувствительности вынесено в отдельные `MIN_SENSITIVITY_DB = -100` / `MAX_SENSITIVITY_DB = 0` (`:35,38`) и используется только в `sensitivity_percent_to_threshold_db` / `threshold_db_to_sensitivity_percent`.
- `set_vad_threshold_db` (`:996-1014`) клампит по полному диапазону `i8`; `-120 дБ` больше не превращается в `-100`. `set_raw_vad_threshold_db` стал синонимом (`:1017-1019`).
- `t09` (`:741-834`) берёт `MIN`/`MAX`/`DEFAULT_VAD_THRESHOLD_DB` из JS-контроллера (`js_const_i64`), дублирует нормативные литералы `(-128, 127)` и `-40`, проверяет сохранение `-120` и клампинг `-999/999`. Мутация P2-M2 (`-128` → `-100`) убивается t09.

### P2-1 — микрофон 200 % — закрыто

Публичные `set_input_volume_200` / `clamp_volume_200` удалены, поиск по `rust/gui-spike/` (кроме артефактов сборки) совпадений не даёт. Вход клампится только в `0..=100` (`:911-927`), выход — в `0..=200` (`:929-945`). `t08` (`:695-734`) теперь закрепляет `set_input_volume(250) == 100` и `set_input_volume_signed(200) == 100` с комментарием, что буста ввода в `OX2Bnr` нет. Спека (`VOICE-DEVICES-SETTINGS-SPEC.md:9,46`) уже описывает вход как `0..=100`, править её не потребовалось. Мутация P2-M4 (клампинг входа к 200) убивается t08.

### P3-1 — `DEFAULT_MIC_TEST_LEVEL_DB` — закрыто

`t14` (`:1165-1167`) проверяет и константу модуля, и `state.mic_test_level_db()` литералом `-100` с пометкой «литерал спеки, не константа модуля». Мутация P2-M3 (`-100` → `-90`) убивается t14.

### P3-2 — `allow(dead_code)` — закрыто

Модульный `#![allow(dead_code)]` из `src/voice_devices.rs` убран. Остался только `#[allow(dead_code)]` на `#[path]`-модуле в тестовом файле (`tests/voice_devices_contract.rs:16`) с комментарием, зачем он нужен: модуль ещё не подключён в `src/lib.rs`, интеграция — отдельный шаг. Это точечное подавление в тестовой сборке, а не отключение сигнала на весь продуктовый файл.

## 3. Мутации (собственный прогон)

Скрипты: `/tmp/opencode/rw40-task124-p2/mutate.py` и `mutate-original.py`; каждая мутация применялась к baseline `8c4a0720…`, тест запускался, файл восстанавливался, md5 сверялся после каждого шага (финальный md5 — `8c4a0720…`).

| ID | Мутация | MD5 мутанта | Итог | Убившие тесты |
|---|---|---|---|---|
| P2-M1 | `VoiceNotice` `Debug`: `DeviceLabels` → сырой `message` | `676b791b…` | KILLED | t15 |
| P2-M2 | `MIN_VAD_THRESHOLD_DB`: `-128` → `-100` | `01dfaa90…` | KILLED | t09 |
| P2-M3 | `DEFAULT_MIC_TEST_LEVEL_DB`: `-100` → `-90` | `2093e803…` | KILLED | t14 |
| P2-M4 | `set_input_volume`: клампинг к `MAX_OUTPUT_VOLUME` | `6fe70eb0…` | KILLED | t08 |
| P2-M5 | `effective_mute`: `\|\|` → `&&` | `c0ffc5db…` | KILLED | t13, t14 |
| P2-M6 | окно PTT: `<=` → `<` | `4e79b263…` | KILLED | t13 |
| P2-M7 | `DEFAULT_INPUT_VOLUME`: `100` → `80` | `8ae17e89…` | KILLED | t01 |
| MUT-2 (46A) | `pick_fallback`: игнор `is_default` | `005e6688…` | KILLED | t01, t06, t07, t15 |
| MUT-3 (46A) | `set_output_volume`: клампинг к 100 | `5772dff2…` | KILLED | t08, t14 |
| MUT-5 (46A) | `Debug for AudioDevice`: сырой `label` | `6ed437af…` | KILLED | t03, t15 |

Пять мутаций из ревью 46A после правок по-прежнему убиваются; выжившая тогда X3 (`DEFAULT_MIC_TEST_LEVEL_DB`) теперь закрыта t14.

## 4. Открытые замечания (P3, не блокируют PASS)

1. **P3-1 (доккомментарий).** Шапка `tests/voice_devices_contract.rs:3` всё ещё говорит «`VoiceActivity` с `vad_threshold_db` `-100..=0` / `i8`». После правки это сбивает: порог — полный `i8` `-128..=127`, а `-100..=0` — рабочее окно шкалы чувствительности. Правится одной строкой.
2. **P3-2 (теоретический канал в `Debug`).** `VoiceNotice::alert` всегда даёт `origin: Static`, поэтому текст ошибки сервиса в `handle_service_result` (`src/voice_devices.rs:596-606`) печатается в `Debug` как есть. Сегодня метки туда не попадают: модуль формирует этот текст из строки ошибки IPC, а не из `label`. Если будущий сервис начнёт класть в ошибку системное имя устройства, утечка вернётся этим путём. Надёжнее считать любой текст извне непроверенным и редактировать его в `Debug` по умолчанию.
3. **P3-3 (процесс).** В `waves/RW-40/PM/task124-voice-devices-bff/` нет fix-отчёта Pass 2 с red/green-логами: только артефакты ревью 46A и этого ревью. Red-first для t15 подтверждён моей мутацией P2-M1, но формального red-лога воркера в карточке нет — Team Lead стоит запросить его до закрытия карточки.

## 5. Артефакты ревью

Пути относительно `waves/RW-40/PM/task124-voice-devices-bff/` в рабочем дереве `feature/task-124-voice-devices-tauri-bff`:

| Файл | Содержание |
|---|---|
| `REVIEW-PASS2.md` | этот вердикт |
| `REVIEW-PASS2-test.log` | прогон 15 контрактных тестов (EXIT=0) |
| `REVIEW-PASS2-clippy.log` | clippy по карточке (EXIT=0) |
| `REVIEW-PASS2-mutations.log` | 7 новых + 3 мутации из ревью 46A, все KILLED |
| `REVIEW-PASS2-privacy-probe.log` | независимая проба приватности: `notice` в `Debug` → `<redacted>` |
