# Ревью 120A — task-205 (`voice_stage` BFF)

**Вердикт: APPROVED. P0 = 0, P1 = 0, мутации 5/5 killed.**

## Проверено
- `rust/gui-spike/src/voice_stage.rs` (221 строка) и `REPORT.md`; контракт совпадает с заявленным: `MAX_STAGE_VISIBLE_TILES = 25`, `StreamResolution` (`1280x720`/`1920x1080`/`2560x1440`), `StreamFps` (`30`/`60`), `ScreenShareSpec::validate` (Snowflake `1..=u64::MAX`, без ведущих нулей), `to_stream_key`/`to_stream_Key`, `op 18`/`op 19` payloads.
- Targeted: `cargo test --test voice_stage_contract` (запуск из `rust/gui-spike`, отдельный workspace) — `8 passed; 0 failed`, exit 0.
- Замечание уровня nit: команда из ТЗ (`cargo test -p discord-gui-spike` из `rust/`) не работает — `gui-spike` объявлен собственным workspace (`[workspace]` в его `Cargo.toml`), поэтому флаг `-p` не нужен. На результат не влияет.

## Мутации (по одной, с восстановлением файла)
| # | Мутация | Результат |
|---|---|---|
| m1 | `MAX_STAGE_VISIBLE_TILES` 25 -> 24 | killed |
| m2 | `P1080::width()` 1920 -> 1280 | killed |
| m3 | `Fps60::fps()` 60 -> 30 | killed |
| m4 | удалена проверка ведущих нулей в `validate_snowflake` | killed |
| m5 | guild-ветка `op 18` отдаёт `type: "call"` | killed |

Исходная m5 (`"op": 18` -> `19`) дала выживание, потому что шаблон попал в doc-комментарий; заменена на m5 выше — реальный код-мутант убит.

## Состояние дерева
- `md5 voice_stage.rs = 9e141b3332ebcc2551eb824e8d905bf5` — совпадает с `REPORT.md`, файл после мутаций побайтно восстановлен.
- `Cargo.lock` после `cargo` откатан (`git checkout --`); `git commit` не выполнялся.
