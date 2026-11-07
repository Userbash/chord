# Ревью — `RW-40 / task-124`, BFF голосовых устройств (`voice_devices.rs`)

**Вердикт: RETURN** (нужны правки до PASS; см. §4 и §5).

- Ревьюер: независимый технический ревьюер DeepSeek (Review 46A), карточка `task_ece18fe6d6aa` / dispatch `ctx_a7f584c084e0`.
- Дата: 2026-10-30.
- Ветка: `feature/task-124-voice-devices-tauri-bff`, базовый коммит `4e24048` (HEAD не двигался, коммитов в ветке нет — работа лежит незакоммиченной в рабочем дереве работы).
- Ревьюируемые файлы (md5 совпадает с отчётом воркера):
  - `rust/gui-spike/src/voice_devices.rs` — `08560cb909a06d2027e2554a1e2896b4` (1453 строки);
  - `rust/gui-spike/tests/voice_devices_contract.rs` — `4be0edebbdfc5c4d215ee20130a8aee5` (1177 строк, 14 тестов).
- Оракулы, с которыми сверялся контракт: `waves/RW-40/DESIGN/VOICE-DEVICES-SETTINGS-SPEC.md` (§0.1, §1.1–§1.3, §3.2, §3.4, §3.7, §4 таблица проверок), `waves/RW-40/DESIGN/voice-spec.json` (`OX2Bnr`, `aUJ062`, `devices.hotplug.autoswitch`), `rust/gui-spike/ui/features/voice-devices-settings.js` (сестринский UI-контроллер).

## 1. Гейты

| Проверка | Команда | Результат |
|---|---|---|
| Контрактные тесты | `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --offline --test voice_devices_contract` | **14 passed, 0 failed** (повторено дважды, лог `REVIEW-test.log`) |
| Clippy по карточке | `cargo clippy ... --test voice_devices_contract -- -D warnings` | **EXIT=0**, чисто (лог `REVIEW-clippy.log`) |
| Clippy по пакету | `cargo clippy ... --all-targets -- -D warnings` | EXIT=101, но падения только в `tests/net_contract.rs:782` и `src/main.rs:1703` (чужие карточки, `type_complexity`); ни одного замечания в `voice_devices.rs`/`voice_devices_contract.rs` — grep по «voice_devices» в выводе даёт 0 совпадений |
| Форматирование | `rustfmt --edition 2021 --check rust/gui-spike/src/voice_devices.rs rust/gui-spike/tests/voice_devices_contract.rs` | EXIT=0 |
| RED-фаза | сверены `red.log` / `green.log` / `mutations.log` отчёта | md5 совпадают с отчётом: `a4ff2e9d…`, `359d8f73…`, `9547faf3…`; в `red.log` `0 passed; 14 failed`, `EXIT_CODE=101` |

Замечание по батчу, не по карточке: `gate:worker` уровня пакета будет красным, пока не починят `net_contract.rs:782` и `main.rs:1703` (это не файлы task-124 и в дефекты карточки не засчитано).

## 2. Мутации (собственный прогон, не пересказ лога воркера)

Скрипт: `/tmp/opencode/rw40-task124/run_mutations.py`; каждая мутация применялась к копии baseline (`08560cb9…`), тест запускался, файл восстанавливался из baseline, md5 перепроверялся. Полный лог — `REVIEW-mutations.log`.

| ID | Мутация | MD5 мутанта | Итог | Убившие тесты |
|---|---|---|---|---|
| MUT-1 | `effective_mute`: `\|\|` → `&&` | `1e7385ab…` | KILLED | t13, t14 |
| MUT-2 | `pick_fallback`: игнор приоритета `is_default` (`.find(.. && false)`) | `10e80cf7…` | KILLED | t01, t06, t07 |
| MUT-3 | `set_output_volume`: клампинг к `MAX_INPUT_VOLUME` (100) вместо 200 | `3a850ac1…` | KILLED | t08, t14 |
| MUT-4 | окно PTT: `<=` → `<` | `b282cd6b…` | KILLED | t13 |
| MUT-5 | `Debug for AudioDevice`: сырой `label` вместо `"<redacted>"` | `ef0b18ae…` | KILLED | t03 |
| X1 (доп.) | `sensitivity_percent_to_threshold_db`: `round` → `trunc` | `556f8968…` | KILLED | t09 |
| X2 (доп.) | VAD-гейт: `>=` → `>` на границе порога | `14c7bad2…` | KILLED | t13 |
| X3 (доп.) | `DEFAULT_MIC_TEST_LEVEL_DB`: `-100` → `-90` | `475e4953…` | **SURVIVED** | — |

Итог по заявленным пяти: **5/5 KILLED подтверждено**. MD5 мутанта MUT-2 отличается от отчёта (`82a289…`), потому что я применил эквивалентную мутацию иначе; набор убитых тестов тот же. Выжившая X3 — не регресс, а дыра в тесте (см. P3-1).

## 3. Что в реализации сделано хорошо

- Тесты сверяются с внешними оракулами, а не сами с собой: `voice-spec.json` читается с диска (t01, t05), проверяются `OX2Bnr` `0..=100/100`, `aUJ062` `0..=200/100`, `autoswitch: false`.
- Hot-plug реконсиляция не вызывает `service.select*` (t05–t07 проверяют `service.calls().is_empty()`), приоритет `is_default` не зависит от позиции в списке.
- `ptt`-гейтинг, окно `release_delay_ms` на границе 200/201 мс, сброс хвоста при mute/deaf, `gateway_mute_deaf` — покрыты точно.
- `tracking` ошибок сервиса: состояние применяется, ошибка уходит в `role="alert"` с текстом `Не удалось сохранить настройку: …` (t14).
- `fmt::Debug for AudioDevice` редактирует `label` маркером `<redacted>`, тест t03 это проверяет.

## 4. Дефекты

### P1-1. `fmt::Debug for VoiceDevicesState` утекает системные имена устройств через `notice`

`rust/gui-spike/src/voice_devices.rs:497` печатает `notice` как есть, а `VoiceNotice::message` после hot-plug отката содержит метки устройств (`rust/gui-spike/src/voice_devices.rs:764-766`: `Устройство «{prev_label}» отключено. Выбрано «{next_label}».`). Спецификация §0.1 п.3 требует: метки ОС показываются только в DOM и **никогда** не попадают в диагностические логи, «в Rust `fmt::Debug` поле `label` принудительно заменяется на `<redacted>`». Здесь политика приватности обходится через текст уведомления.

Воспроизведение (скрипт `REVIEW-privacy-probe.rs`, лог `REVIEW-privacy-probe.log`; включается через `#[path]`, репозиторий не менялся):

```
VoiceDevicesState { devices: [AudioDevice { id: "mic-builtin", label: "<redacted>", … }],
  …, notice: Some(VoiceNotice { level: Status,
  message: "Устройство «Система: USB Studio Microphone» отключено. Выбрано «Система: Built-in Microphone»." }) }
```

Тест t03 (`tests/voice_devices_contract.rs:361-372`) эту дыру не ловит: он проверяет `Debug` до hot-plug, когда `notice == None`. Нужен отдельный тест «Debug после `DeviceFallback` не содержит меток».

### P1-2. `MIN_VAD_THRESHOLD_DB` / `MAX_VAD_THRESHOLD_DB` противоречат нормативной таблице и UI-контроллеру

- Спецификация §1.1, строка `DEFAULT_VAD_THRESHOLD_DB`: минимум `-128` (`MIN_VAD_THRESHOLD_DB`), максимум `127` (`MAX_VAD_THRESHOLD_DB`), §1.2: «`thresholdDb` клампится в `-128..=127` (i8)».
- Сестринский контроллер `rust/gui-spike/ui/features/voice-devices-settings.js:45,48` экспортирует ровно `-128` / `127` и клампит `setInputMode` по ним (строки 639-649).
- Rust-модуль объявляет те же публичные имена со значениями `-100` / `0` (`src/voice_devices.rs:34,37`), и `set_vad_threshold_db` (`:976-980`) клампит вход по ним. Значение `-120 дБ`, которое UI считает валидным, молча превращается в `-100`.
- Тест это фиксирует как норму: `assert_eq!(MIN_VAD_THRESHOLD_DB, -100); assert_eq!(MAX_VAD_THRESHOLD_DB, 0);` (`tests/voice_devices_contract.rs:725-726`) — то есть тест защищает расхождение, а не контракт.

Полный диапазон доступен только через отдельный `set_raw_vad_threshold_db`, но штатный путь BFF (`set_vad_threshold_db`) остаётся уже спеки. Либо имена должны означать полный диапазон `i8` (как в спеке и JS), а рабочее окно `-100..=0` — отдельные константы, либо таблицу спеки нужно менять синхронно с JS. Сейчас рассинхрон гарантированно всплывёт при подключении BFF к UI.

### P2-1. Публичный API разрешает громкость микрофона 200 %, хотя оракул `OX2Bnr` — 100 %

`set_input_volume_200` (`src/voice_devices.rs:899-910`) и `clamp_volume_200` (`:379-381`) дают входу `0..=200`; тест это закрепляет (`tests/voice_devices_contract.rs:691-694`: 165 % и 200 % для входа). Спецификация §1.1 и JS (`voice-devices-settings.js:30` `MAX_INPUT_VOLUME = 100`) знают только 100 %. Дополнительный метод не запрещён явно, но он создаёт состояние, которого нет в оракуле; как минимум нужна пометка `[A]` в спеке, как максимум — метод убрать.

### P3-1. Константа `DEFAULT_MIC_TEST_LEVEL_DB` не закреплена тестом

`tests/voice_devices_contract.rs:1112` сравнивает `state.mic_test_level_db()` с импортированной из модуля константой. Мутация X3 (`-100` → `-90`) выжила: тест тавтологичен, нормативное значение `-100` из таблицы §1.1 не проверяется. Ожидаемое значение нужно брать литералом или из спеки.

### P3-2. `#![allow(dead_code)]` и модуль вне сборки крейта

Модуль не подключён в `rust/gui-spike/src/lib.rs`/`main.rs` (grep по `voice_devices` пуст), компилируется только контрактным тестом через `#[path]`. Для этой карточки это осознанное решение (интеграция — отдельный шаг), но модульный `#![allow(dead_code)]` (`:11`) отключает сигнал о мёртвом коде на весь файл и скроет его при подключении. Лучше сузить allow до конкретных элементов или снять после интеграции.

## 5. Что нужно для PASS

1. P1-1: убрать метки устройств из `Debug`-пути (`notice` хранить как структуру с `id`, а текст собирать в view-слое, либо редактировать сообщение в `Debug`); добавить тест «`Debug` после `DeviceFallback` не содержит меток ОС».
2. P1-2: привести `MIN_VAD_THRESHOLD_DB`/`MAX_VAD_THRESHOLD_DB` к `-128`/`127` (или переименовать рабочие границы), синхронизировать `set_vad_threshold_db` с диапазоном слайдера из спеки/JS; t09 переписать так, чтобы значения брались из спеки/JS, а не из литералов, совпадающих с реализацией.
3. P2-1: решить судьбу 200 % для микрофона — убрать или зафиксировать в спеке; тест не должен закреплять поведение вне оракула без пометки.
4. P3-1: закрепить `DEFAULT_MIC_TEST_LEVEL_DB == -100` независимо от константы модуля.
5. Перезапустить контрактные тесты, clippy по карточке и 5 мутаций после правок; повторное ревью — по этому же чек-листу.

## 6. Артефакты ревью

Все пути относительно `waves/RW-40/PM/task124-voice-devices-bff/` в рабочем дереве `feature/task-124-voice-devices-tauri-bff`:

| Файл | Содержание |
|---|---|
| `REVIEW.md` | этот вердикт |
| `REVIEW-test.log` | повторный прогон 14 тестов |
| `REVIEW-clippy.log` | clippy по карточке (EXIT=0) и по пакету (EXIT=101, чужие файлы) |
| `REVIEW-mutations.log` | независимый прогон 5 заявленных + 3 дополнительных мутаций |
| `REVIEW-privacy-probe.rs`, `REVIEW-privacy-probe.log` | воспроизведение утечки меток в `Debug` (P1-1) |
