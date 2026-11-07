# Technical Review 173A — PR-144 / task-259 (`BE-P1-31`, Voice Soundboard BFF)

- **Ревьюер**: Технический ревьюер (DeepSeek), token-lean режим.
- **Дата**: 2026-10-31.
- **Ветка**: `feature/task-259-be-soundboard`, `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.
- **Объект ревью**: `rust/gui-spike/src/soundboard.rs`, `rust/gui-spike/tests/soundboard_contract.rs`, отчёт воркера `REPORT.md`.

## 1. Вердикт

**APPROVED.**

- `P0 = 0`, `P1 = 0`.
- Мутации: **5/5 убиты** (`exit=101` на каждой).
- Targeted-тест на восстановленном исходнике: `6 passed; 0 failed`, `exit=0`.
- Финальный md5 `rust/gui-spike/src/soundboard.rs` совпадает с заявленным в `REPORT.md` и с исходным состоянием до ревью.

## 2. Сверка артефактов

| Проверка | Значение |
|---|---|
| md5 `rust/gui-spike/src/soundboard.rs` (до ревью) | `8ba4d45ed8953f59f424843be1a7ad20` |
| md5 в `REPORT.md` §6 | `8ba4d45ed8953f59f424843be1a7ad20` — совпадает |
| md5 `rust/gui-spike/tests/soundboard_contract.rs` | `d96debb8e28c4f70e265caef8095b665` (совпадает с отчётом, файл не изменялся) |
| Baseline targeted-прогон | `6 passed; 0 failed; 0 ignored`, `exit=0` |
| `rust/gui-spike/src/lib.rs` | единственная строка `+pub mod soundboard;`, лишних правок нет |

Тестовый файл во время ревью не редактировался; полный `gate:worker` и `git commit` не запускались.

## 3. Мутационное тестирование (M1–M5)

Методика: одна мутация → `cargo test --manifest-path rust/gui-spike/Cargo.toml --test soundboard_contract` → фиксация exit-кода → побайтовое восстановление из бэкапа через `cp` + `touch` (свежий mtime, чтобы cargo пересобрал) → сверка `md5sum`.

| # | Мутация (файл `src/soundboard.rs`) | md5 мутанта | Exit | Killed by | Итог |
|---|---|---|---|---|---|
| M1 | Граница имени `(MIN..=MAX)` → `(MIN..MAX)`: имя ровно из 32 символов начинает отклоняться | `bdef2ab212a744475ae25079ce6130e6` | 101 | `validate_create_guild_sound_enforces_trimmed_unicode_name_bounds` (5 passed / 1 failed) | KILLED |
| M2 | Нижняя граница размера `1..=MAX_SOUND_BYTES` → `0..=MAX_SOUND_BYTES`: пустой файл проходит | `ccbe97c0b93e789f22336581920e8b2a` | 101 | `validate_create_guild_sound_enforces_byte_len_and_audio_data_uri_prefixes` (5/1) | KILLED |
| M3 | Из проверки громкости убрана верхняя граница: `!v.is_finite() \|\| v < MIN` — `1.0001` проходит | `547a937933f249d0e01a8b292af8ff13` | 101 | `validate_create_guild_sound_validates_volume_and_optional_emoji_id` (5/1) | KILLED |
| M4 | Проверка пустого base64-пейлоада отключена: `if payload.trim().is_empty()` → `if false` — `data:audio/ogg;base64,   ` проходит | `99e14d8c896b6d751801ef0c338cab16` | 101 | `validate_create_guild_sound_enforces_byte_len_and_audio_data_uri_prefixes` (5/1) | KILLED |
| M5 | Из `validate_snowflake` удалено правило ведущего нуля: `"0123"` начинает проходить | `0b8632761ccc23847ed06ffa8f5250df` | 101 | `build_send_soundboard_route_and_body_formats_route_and_validates_snowflakes` + `validate_create_guild_sound_validates_volume_and_optional_emoji_id` (4 passed / 2 failed) | KILLED |

Логи прогонов: `/tmp/opencode/tr173a/M{1..5}.log`, бэкап исходника: `/tmp/opencode/tr173a/soundboard.rs.orig`.

**Финальная сверка после восстановления**: `md5 = 8ba4d45ed8953f59f424843be1a7ad20`, повторный targeted-прогон `6 passed; 0 failed`, `exit=0`.

## 4. Замечания (не блокирующие)

- **P2** — `validate_create_guild_sound` принимает `byte_len: u64` отдельным аргументом и не проверяет, что это значение соответствует длине base64-пейлоада в `sound_data_uri`. Контракт документирован и покрыт тестами, но вызывающая сторона может передать рассогласованные значения. Оставить как есть в рамках текущей карточки; вынести проверку консистентности в задачу вызывающего слоя.
- **P3** — `emoji_name` не валидируется (ни длина, ни ограничение на один эмодзи). В протоколе Discord поле опциональное и необязательное к строгой проверке; зафиксировать как известное ограничение.
- **P3** — Список разрешённых префиксов строгий и не включает варианты вида `data:audio/ogg;codecs=opus;base64,`. Это соответствует заявленному контракту, но при появлении реального трафика с `codecs=` потребуется расширение списка.
- **Процесс** — при запуске `cargo` повторно перегенерировался `rust/gui-spike/Cargo.lock` (дрейф `zstd`); файл откатан через `git checkout --`, write-scope соблюдён. Тот же риск уже описан в `REPORT.md` §7.

## 5. Условия приёмки

`P0 = 0`, `P1 = 0`, мутации `5/5`, targeted-тест зелёный, md5 исходника совпадает с `REPORT.md` → блокирующих замечаний нет. Задача переходит к приёмке PM.
