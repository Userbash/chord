# B8a — settings persistence backend: отчёт G1

Дата: 2026-10-24. Воркер: G1 Backend. PM: DeepSeek.
Карточка: B8a (`waves/RW-40/TASKS.md`). Контракт: ТЗ B8a (PM), `ui/index.html`,
`ui/features/settings.js`, `discord-core/settings/preloaded.rs`.

## 1. Статус

**done (backend, offline).** Хранилище UI-настроек реализовано red-first, покрыто
17 контрактными тестами. Окно не запускалось, сети нет, `ui/**` и `fixtures/**`
не изменялись (только чтение как оракул). База gui-тестов 132 не ослаблена:
итог 149 = 132 + 17 новых.

## 2. Сделано

| Пункт ТЗ | Где | Проверка |
|---|---|---|
| 1. `UiSettings` (serde): `toggles: BTreeMap<String,bool>`, `density`, `theme`, `locale`, `version` | `src/settings_store.rs` (`UiSettings`, `Density`, `Theme`, `Locale`) | `serialized_defaults_are_canonical_and_hold_no_secrets`, `defaults_follow_shipped_ui_contract` |
| 1. только UI-настройки, без секретов | `#[serde(deny_unknown_fields)]`; ключи файла ровно 5 | `unknown_fields_are_rejected_and_values_not_echoed`, `serialized_defaults_are_canonical_and_hold_no_secrets` |
| 2. путь `DL_SETTINGS_PATH` или `<repo>/.local/settings.json` (без `$HOME`) | `default_settings_path`, `resolve_path`, `settings_path`, `SettingsStore::from_env` | `settings_path_prefers_env_and_defaults_to_repo_local`, `store_from_env_uses_dl_settings_path` |
| 2. атомарная запись tmp+rename, создание каталога | `SettingsStore::write_atomic`, `write_temp` (`create_new` → `write_all` → `sync_all` → `rename`) | `atomic_save_leaves_no_temp_files`, `save_creates_missing_parent_directories` |
| 3. `load`: нет файла → default; битый JSON → `Parse`, не паника | `SettingsStore::load` | `missing_file_yields_defaults_without_creating_anything`, `corrupt_json_is_typed_parse_error` |
| 4. `save`: валидация значений, лимит 256 KiB, `Io/Parse/Invalid` | `UiSettings::validate`, `SettingsStore::save`, `SettingsError` | `save_rejects_bad_toggles_and_incompatible_version`, `save_rejects_more_than_size_limit_without_touching_disk`, `errors_are_typed_and_serialize_with_kind` |
| 5. Tauri `settings_load`, `settings_save`; без токенов | `src/main.rs` (обёртки + `invoke_handler!`), I/O в `spawn_blocking` | `tauri_commands_are_registered_and_carry_no_tokens` |
| 6. тесты roundtrip/missing/corrupt/atomic/invalid/size | `tests/settings_contract.rs` (17 тестов) | см. §3 |

Семантика ошибок (`SettingsError`, JSON `{kind, message, path?, field?}`):

- `Parse` — не JSON, не UTF-8, не объект, неверный тип поля, неизвестное поле;
- `Invalid` — JSON разобран, но значение вне контракта: чужая `version`,
  `density`/`theme`/`locale` вне белого списка (ошибка называет поле и допустимые
  значения), пустой/пробельный/слишком длинный (>256 байт) id тумблера, >2048
  тумблеров, размер файла/сериализации >256 KiB;
- `Io` — отказ ФС; отсутствие файла — **не** ошибка (дефолты, файл не создаётся).

Дефолты взяты из отгруженного UI (не выдуманы): `ui/index.html` —
`<html lang="ru" data-density="cozy">` + `color-scheme: dark`; белые списки
совпадают со строками `ui/features/settings.js` и enum'ами ядра
(`Theme 1 DARK/2 LIGHT`, `UIDensity 1 COMPACT/2 COZY`). Пустая карта тумблеров =
стартовые состояния UI (38 тумблеров, SET-06/07/08) остаются за фронтендом.

## 3. Red → green

RED (`waves/RW-40/PM/b8a/red.log`, exit 101):

```
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test settings_contract
error[E0432]: unresolved import `discord_gui_spike::settings_store`
  --> tests/settings_contract.rs:21:24
```

GREEN (`waves/RW-40/PM/b8a/green.log`, exit 0): **17 passed / 0 failed**.

Новые тесты:

```
defaults_follow_shipped_ui_contract
serialized_defaults_are_canonical_and_hold_no_secrets
missing_file_yields_defaults_without_creating_anything
corrupt_json_is_typed_parse_error
unknown_fields_are_rejected_and_values_not_echoed
invalid_density_theme_and_locale_are_typed_invalid
incompatible_version_is_typed_invalid
oversize_file_is_rejected_before_parsing
save_then_load_roundtrip_is_lossless
atomic_save_leaves_no_temp_files
save_creates_missing_parent_directories
save_rejects_more_than_size_limit_without_touching_disk
save_rejects_bad_toggles_and_incompatible_version
errors_are_typed_and_serialize_with_kind
settings_path_prefers_env_and_defaults_to_repo_local
store_from_env_uses_dl_settings_path
tauri_commands_are_registered_and_carry_no_tokens
```

## 4. Проверки

| Проверка | Результат | Лог |
|---|---|---|
| `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | exit 0 · **149 passed / 0 failed** (база 132 + 17) | `waves/RW-40/PM/b8a/gui-test.log` |
| `cargo test ... --test settings_contract` | exit 0 · **17 passed / 0 failed** | `waves/RW-40/PM/b8a/green.log` |
| `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings` | exit 0 | `waves/RW-40/PM/b8a/clippy.log` |
| `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` | exit 0 | `waves/RW-40/PM/b8a/fmt.log` |

`npm run gate:worker` не запускался: работа в batch, в это же время G2 правит
`ui/**` (mtime `ui/app.js`, `ui/features/auth.js` — не мои правки), гонять общий
UI-гейт на движущемся дереве неинформативно. Окно/сеть/commit не запускались.

## 5. md5

```
561a4fa17953e6b0c3b508cc5eb7357b  rust/gui-spike/src/settings_store.rs
d933c9c820e5fa44dc9c351c345eb682  rust/gui-spike/src/lib.rs
9ed017560b6bcd2f14161e9c72486fcf  rust/gui-spike/src/main.rs
e2ff31684d167c222d32ca99853809c8  rust/gui-spike/tests/settings_contract.rs
```

Изменены только эти 4 файла (settings_store — новый, settings_contract — новый,
lib.rs — `pub mod settings_store` + doc, main.rs — импорт, 2 команды,
регистрация, обобщённый текст ошибки `blocking`). `ui/**`, `fixtures/**`,
`data.rs` не изменялись; `.local/` в репозитории не создавался (тесты пишут
только в `std::env::temp_dir()` и убирают за собой).

## 6. Остаток / открытые вопросы

1. **`.local/` нет в `.gitignore`** (корневой `.gitignore` — не мой файл).
   До первого реального `settings_save` нужно добавить `/.local/`, иначе
   локальный файл настроек может уехать в commit. Владелец — Team Lead/PM.
2. **B8b (G2)**: контракт id тумблеров — произвольная непустая строка UTF-8
   до 256 байт (запрещены только control-символы). Рекомендация фронтенду:
   стабильные ASCII-id (`qos_priority`, `voice.echo_cancel`), а не русские
   подписи — подпись может меняться при i18n (C4).
3. Миграций версии нет: `version != 1` → `Invalid`. При появлении схемы v2
   нужна явная миграция/сброс, иначе старый файл перестанет читаться (сейчас
   это честная типизированная ошибка, не тихий сброс).
4. Права файла — из umask (обычно 0644). Секретов в файле нет по контракту
   (`deny_unknown_fields`), поэтому 0600 не требуется; если PM захочет — это
   одна строка в `write_temp` (unix-only).
5. `fsync` каталога после `rename` не делается: атомарность замены есть,
   durability при жёстком сбое машины — best-effort. Для настроек приемлемо,
   отмечено честно.
