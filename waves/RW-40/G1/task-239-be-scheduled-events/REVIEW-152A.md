# Техническое ревью 152A — task-239 (`BE-P1-25`, `scheduled_events`)

**Verdict: APPROVED**

- Волна: `RW-40`, отдел `G1` (Backend).
- Ревьюер: Technical Reviewer (DeepSeek, dispatch `ctx_0d5e0134eeb3`).
- Ветка/worktree: `feature/task-239-be-scheduled-events`, HEAD `8e8d13a3d4a1a1750dae83de2ac242fb41887f9d`.
- Кандидат: `rust/gui-spike/src/scheduled_events.rs` (`b4c3e2052468f296dad69f0cbe4c0188`), `rust/gui-spike/tests/scheduled_events_contract.rs` (`2529b0ad5e21883960effd7b0d21c505`).
- Отчёт воркера: `waves/RW-40/G1/task-239-be-scheduled-events/REPORT.md`.

## 1. Что проверено

- Контракт модуля совпадает с Discord API v9 и маршрутами из `REPORT.md` §2: типы сущностей `1/2/3`, статусы `1..=4`, лимиты `100/1000/100`, `privacy_level = 2`, `channel_id` обязателен для `STAGE`/`VOICE`, `location` + `scheduled_end_time` обязательны для `EXTERNAL`.
- Валидация Snowflake отсекает пустую строку, длину `> 20`, ведущий `'0'` и нецифровые символы — границы `"1"`, `u64::MAX`, 20 цифр проходят, 21 цифра падает.
- Тесты сверяют поведение по внешним для кода значениям (assert на константы, границы, точные строки маршрутов), а не по текущей реализации.
- `scheduled_events.rs` не содержит `unwrap`/`panic!`, ошибки возвращаются `Err(String)` с указанием поля.
- Побочных правок в кандидате нет: изменились только `src/scheduled_events.rs`, `src/lib.rs` (экспорт модуля), `tests/scheduled_events_contract.rs`; `Cargo.lock` откатан, `package-lock.json` и `node_modules` — предсуществующее состояние рабочего дерева.

## 2. Целевой прогон

```
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
cargo test --manifest-path rust/gui-spike/Cargo.toml --test scheduled_events_contract
```

Baseline до мутаций: `8 passed; 0 failed`, exit `0`.
После всех мутаций и восстановления файла: `8 passed; 0 failed`, exit `0`.

## 3. Мутационное тестирование — 5/5 убито

Каждая мутация вносилась в `rust/gui-spike/src/scheduled_events.rs`, прогонялся тот же целевой тест, затем файл восстанавливался из побайтовой копии.

| # | Мутация | Результат | Тест-детектор |
|---|---|---|---|
| M1 | `validate_snowflake`: `value.len() > 20` → `>= 20` | KILLED (exit 101) | `build_event_rsvp_route_validates_both_snowflakes_and_formats_path`, `validate_scheduled_event_request_validates_stage_and_voice_channel_snowflake` |
| M2 | Граница имени: `1..=MAX_EVENT_NAME_CHARS` → `1..MAX_EVENT_NAME_CHARS` | KILLED (exit 101) | `validate_scheduled_event_request_enforces_name_and_description_bounds` |
| M3 | `privacy_level != 2` → `privacy_level > 2` | KILLED (exit 101) | `validate_scheduled_event_request_enforces_privacy_level_and_start_time` |
| M4 | Описание: `desc_len > MAX_EVENT_DESC_CHARS` → `>=` | KILLED (exit 101) | `validate_scheduled_event_request_enforces_name_and_description_bounds` |
| M5 | `build_guild_events_route`: query `with_user_count={with_user_count}` → `=false` | KILLED (exit 101) | `build_guild_events_route_validates_guild_id_and_formats_query` |

Итог: `mutation score = 5/5 (100%)`.

## 4. Восстановление исходника

```
md5sum rust/gui-spike/src/scheduled_events.rs
b4c3e2052468f296dad69f0cbe4c0188  rust/gui-spike/src/scheduled_events.rs
cmp -s orig.rs src/scheduled_events.rs → BYTE-IDENTICAL=YES
```

MD5 совпадает со значением в `REPORT.md` §6 — файл восстановлен побайтово, следов мутаций нет.

## 5. Ограничения ревью

- По ТЗ прогонялся только целевой контрактный тест; workspace-тесты, `clippy` и `fmt` в этом ревью не повторялись (результаты воркера — `REPORT.md` §5).
- `git commit`/`push` не выполнялись.

## 6. Замечания

Блокирующих (`P0`) и существенных (`P1`) замечаний нет. Замечания уровня `P3`: в тесте `validate_scheduled_event_request_enforces_name_and_description_bounds` граничные значения имени и описания проверяются на одном и том же объекте запроса — читаемость ниже, чем у отдельных кейсов; на покрытие это не влияет.

**Решение: APPROVED** — код, тесты и контракт соответствуют задаче, мутации 5/5, исходник восстановлен байт-в-байт.
