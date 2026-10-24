# COVERAGE-GAPS — ядро и backend (RW-39, 2026-10-23)

Метод: статическая эвристика — для каждого `pub fn` проверено, встречается ли имя в
`tests/*.rs` или в inline `#[cfg(test)]`-блоке. Это **не** line coverage: функция может
покрываться косвенно. Сырые данные: `PM/coverage-heuristic.json`.

## 1. Сводка

| Крейт / слой | pub fn | есть ссылка в тестах | без прямых тестов |
|---|---:|---:|---:|
| `discord-core` | 265 | 237 | **28** |
| `discord-ui` | 112 | 92 | **20** |
| `discord-platform` | 62 | 56 | 6 |
| `discord-json` | 14 | 10 | 4 |
| `discord-i18n` | 41 | 38 | 3 |
| `discord-voice` | 84 | 81 | 3 |
| `discord-conformance` | 9 | 8 | 1 |
| `discord-codec` | 18 | 18 | 0 |
| `discord-store` | 11 | 11 | 0 |
| `discord-perf` | 8 | 8 | 0 |
| `discord-updater` | 13 | 13 | 0 |
| `gui-spike` backend (`data.rs`) | 12 | 12 | 0 |

## 2. Что без прямых тестов (приоритет)

### P0 — безопасность/крипта
- `discord-voice::srtp_key_derive` — вывод ключей SRTP. Нет прямого теста на векторы;
  при ошибке ломается шифрование/совместимость. **Карточка: тест на эталонные ключи.**

### P1 — функции, которые напрямую нужны UI
- `discord-core::settings::decode_preloaded_user_settings`,
  `decode_frecency_user_settings` — настройки (18 разделов/38 тумблеров).
- `discord-core::state::voice::{parse_voice_state_update, parse_voice_states,
  parse_voice_channel_start_time, parse_voice_channel_status}` — голосовые состояния.
- `discord-core::identify::build_identify_frame` — сборка IDENTIFY (auth-путь);
  проверить, что покрыт косвенно через `identify`-тесты, иначе добавить прямой.
- `discord-platform::close_fds` — закрытие fd (утечки/безопасность).
- `discord-ui::{set_scroll_position, set_unread_count, set_sort, set_text,
  focused_user, set_focused_user}` — состояние UI (виртуализация/поиск/фокус).
- `discord-ui::{add_user_to_cache, get_user, has_user, get_message}` — кэш/селекторы.

### P2 — качество и утилиты
- `discord-core::{text_list, dedup_last_by, opt_text, note_ignored_message, ...}` —
  вспомогательные; часть может покрываться косвенно.
- `discord-json::{as_bool, from_i64, from_u64, is_null}` — конверсии.
- `discord-i18n::{format_date_time_pattern, plural_category, plural_category_integer}` —
  форматирование; проверить косвенное покрытие.
- `discord-platform::{authenticate, close_all, next_signal, add_match, from_code}`.
- `discord-conformance::read_text`.
- `discord-voice::{send_empty_rr, send_srtcp_empty_rr}`.

## 3. Backend (`data.rs`)

Все 12 `pub fn` встречаются в тестах: `ready` 27, `messages_page` 23, `members_page` 12,
`search_page` 32, `settings` 10, ошибки/конструкторы 1–41. Прямых «сирот» нет.

Но: эвристика не показывает **ветки**. Отдельно проверить (E9):
- пустая страница, границы `limit=1/100/101`;
- `d: null`, кадр без `t`, битый item;
- файл >64 MiB, вход >256;
- отсутствующий id/каталог, битый JSON;
- `search_page` с пустыми фильтрами и сортировкой.

## 4. Как получить настоящий coverage

`cargo-llvm-cov` не установлен, сеть/установка запрещены. Когда будет разрешено:

```bash
cargo install cargo-llvm-cov            # или системный пакет
cargo llvm-cov --workspace --summary-only
cargo llvm-cov --manifest-path rust/gui-spike/Cargo.toml --summary-only
```

До этого — только эвристика; в отчётах писать «нет прямых тестов», а не «0 % coverage».

## 5. Предлагаемые карточки

| ID | Что | Приоритет |
|---|---|---|
| COV-01 | `srtp_key_derive` — эталонные векторы | P0 |
| COV-02 | `decode_preloaded/frecency_user_settings` — фикстуры протобаф | P1 |
| COV-03 | voice parsers (4 функции) — позитив/негатив | P1 |
| COV-04 | `build_identify_frame` — прямой тест структуры | P1 |
| COV-05 | `close_fds` — утечки/двойное закрытие | P1 |
| COV-06 | `discord-ui` state mutators (8) — сеттеры/геттеры | P2 |
| COV-07 | json/i18n helpers — таблица краевых значений | P2 |
