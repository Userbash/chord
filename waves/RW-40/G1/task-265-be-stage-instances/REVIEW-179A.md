# Technical Review 179A — PR-150 / task-265 (`BE-P1-32`, Rust Stage Channel Instances & Speaker Request BFF)

- **Вердикт**: **APPROVED**
- **P0**: 0 · **P1**: 0 · **Мутации**: 5/5 убиты
- **Ревьюер**: Технический ревьюер (DeepSeek), режим token-lean
- **Дата**: 2026-10-31 14:14 CST (Asia/Shanghai)
- **Ветка**: `feature/task-265-be-stage-instances` (worktree `feature-task-265-be-stage-instances`, HEAD `36dc676`)
- **Окружение**: `cargo 1.99.0`, `rustc 1.99.0`, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`

Проверялись только разрешённые файлы: `rust/gui-spike/src/stage_instances.rs`,
`rust/gui-spike/tests/stage_instances_contract.rs`, `REPORT.md`. Полный `gate:worker`,
`git commit`, правки тестового файла не выполнялись.

## 1. Сверка артефактов с REPORT.md

| Файл | md5 по REPORT.md | md5 фактический | Итог |
|---|---|---|---|
| `rust/gui-spike/src/stage_instances.rs` | `9ffbfacca407637196ab0ffc9da44dfc` | `9ffbfacca407637196ab0ffc9da44dfc` | совпадает |
| `rust/gui-spike/tests/stage_instances_contract.rs` | `6bf3197d64d69726d79c0228e2391543` | `6bf3197d64d69726d79c0228e2391543` | совпадает, файл не менялся |
| `rust/gui-spike/src/lib.rs` | `dea23457b1361d1e821fe733fd7ebc2f` | `dea23457b1361d1e821fe733fd7ebc2f` | совпадает |

## 2. Baseline targeted-прогон

```
cargo test --manifest-path rust/gui-spike/Cargo.toml --test stage_instances_contract
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT=0
```

Все 7 контрактных тестов зелёные до мутаций. После каждой мутации и каждого восстановления
прогон повторялся, поэтому зелёный baseline подтверждён и в конце.

## 3. Ручные мутации (5/5 убиты)

Каждая мутация вносилась точечно в `rust/gui-spike/src/stage_instances.rs`, запускался targeted-тест,
затем файл побайтово восстанавливался из бэкапа `/tmp/opencode/review179a/stage_instances.rs.orig`
(`cp` без `-p`, чтобы mtime обновился и cargo пересобрал крейт), после чего сверялся `md5sum`
и прогон возвращался в зелёный.

| # | Мутация | md5 мутанта | Exit | Убившие тесты | Итог |
|---|---|---|---|---|---|
| M1 | `MAX_STAGE_TOPIC_CHARS: usize = 120` → `121` | `d864d451479d7c2d847d260de75c982c` | 101 | `stage_instances_constants_match_discord_contract` | убита |
| M2 | `STAGE_PRIVACY_GUILD_ONLY: u8 = 2` → `1` | `45aa7a9ba1611ba96a0fbfc9f5820217` | 101 | `stage_instances_constants_match_discord_contract`, `validate_create_stage_instance_enforces_guild_only_privacy_level`, `build_create_stage_instance_route_and_body_validates_and_trims_topic`, `stage_instances_request_structs_serde_roundtrip` | убита |
| M3 | Удалена проверка `value.starts_with('0')` в `validate_snowflake` | `3f68203d487e5a5004fefa6b03b493db` | 101 | `validate_create_stage_instance_enforces_channel_id_snowflake`, `build_update_self_stage_voice_state_route_and_body_validates_snowflakes_and_timestamp` | убита |
| M4 | Валидация длины темы по нетримленной строке: `req.topic.trim().chars().count()` → `req.topic.chars().count()` | `c2709bf4434383c0c8fa39029e585c3b` | 101 | `validate_create_stage_instance_enforces_trimmed_unicode_topic_bounds` | убита |
| M5 | В теле запроса `"topic": req.topic.trim()` → `"topic": req.topic` | `741493e0c158237a9bf2ce037063a189` | 101 | `build_create_stage_instance_route_and_body_validates_and_trims_topic` | убита |

Все мутации — семантические (компилируемые), не «поломка сборки»: падения приходят из assert-ов
контрактных тестов, а не из ошибок компиляции.

## 4. Восстановление и финальное состояние

- После каждой мутации: `cmp -s` против бэкапа — побайтовое совпадение; заключительный `md5sum`
  `rust/gui-spike/src/stage_instances.rs` = **`9ffbfacca407637196ab0ffc9da44dfc`** (равен исходному
  md5 из REPORT.md).
- Финальный прогон после последнего восстановления: `7 passed; 0 failed`, `EXIT=0` — подтверждает,
  что cargo действительно пересобрал крейт, а не отдал кэш мутанта.
- `rust/gui-spike/Cargo.lock`: при прогонах перегенерировался (`Updating crates.io index`,
  `Locking 4 packages` — транзитивный `zstd` от `discord-codec`); откатан
  `git checkout -- rust/gui-spike/Cargo.lock`, в рабочем дереве снова отсутствует в diff.
- Тестовый файл не изменялся (`git diff` пуст, md5 равен отчёту). `git commit` не выполнялся.
- Итоговый `git status` по `rust/gui-spike/`: ` M src/lib.rs`, `?? src/stage_instances.rs`,
  `?? tests/stage_instances_contract.rs` — ровно заявленный write-scope.

## 5. Ревью кода (P0/P1)

Замечаний уровня P0 и P1 нет. Контракт реализован консистентно и проверяемо:

- `validate_snowflake` закрывает пустую строку, длину > 20, ведущий `'0'` и нецифровые байты;
  проверка `bytes().all(is_ascii_digit)` при ASCII-цифрах эквивалентна проверке по символам.
- Длина темы считается через `chars()`, а не `len()` — Unicode-темы (кириллица, эмодзи) не режутся
  по байтам; границы `1..=120` после `trim()` и валидация, и тело запроса применяют одинаково,
  поэтому «пролезть» нетримленной темой нельзя.
- `build_create_stage_instance_route_and_body` обязательно вызывает валидатор (мутация с пропуском
  вызова была бы убита тестом `bad_req`), маршрут `/api/v9/stage-instances` совпадает с
  `Endpoints.STAGE_INSTANCES` из оракула по REPORT.md.
- `build_update_self_stage_voice_state_route_and_body` валидирует оба Snowflake (`guild_id` и
  `channel_id`), отвергает пустой/пробельный `request_to_speak_timestamp` и строит маршрут
  `/api/v9/guilds/{guild_id}/voice-states/@me`, что соответствует `UPDATE_VOICE_STATE`
  (`PATCH /guilds/{arg}/voice-states/{arg}`) с `@me` для собственного состояния.
- Ошибки возвращаются как `Err(String)` с диагностикой поля и фактического значения — вызывающая
  сторона не получает «молчаливый» отказ.

## 6. Неблокирующие замечания (P2/P3)

1. **P3** — `validate_snowflake` ограничивает ID 20 ASCII-цифрами, но не проверяет влезание в `u64`:
   `"99999999999999999999"` (> `u64::MAX`) проходит валидацию, и тест это поведение фиксирует.
   Для реальных Snowflake Discord недостижимо, но при желании можно добавить `parse::<u64>()`.
   Контракт карточки («`1..=20` ASCII-цифр») выполнен, поэтому не блокирует.
2. **P3** — `request_to_speak_timestamp` проверяется только на непустоту, без разбора ISO-8601.
   Соответствует ТЗ и тесту (формат валидирует сервер), но ошибка формата всплывёт только на 400.
3. **P3** — при `send_start_notification = None` в теле уходит явный `null`, а не отсутствие ключа.
   Тест это фиксирует, Discord принимает `null`; на будущее можно `skip_serializing_if`, но менять
   контракт в этой задаче не требуется.

## 7. Итог

P0=0, P1=0, 5/5 мутаций убиты, md5 всех трёх файлов совпадают с REPORT.md, тестовый файл
не тронут, `Cargo.lock` возвращён в исходное состояние. **APPROVED** — PR-150 можно передавать
дальше по пайплайну (merge в `unstable` после PM-приёмки).
