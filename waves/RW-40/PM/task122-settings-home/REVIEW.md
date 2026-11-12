# REVIEW — task-122-settings-store-home-mode (RW-40)

- **Ревьюер:** Review 43A (DeepSeek), независимый технический ревьюер
- **Карточка:** `RW-40 / task-122-settings-store-home-mode` (`task_0286fbbb15d9`)
- **Ветка / HEAD:** `feature/task-122-settings-store-home-mode`, `4e240481e59ff4d49732ac8bd5ba08a006363748`
- **Дата:** 2026-10-30
- **Вердикт:** **PASS** (P0 — 0, P1 — 0 в границах карточки, P2 — 3, P3 — 1; открытый пункт вне scope — один)

## 1. Что проверено

| Проверка | Команда / способ | Результат |
|---|---|---|
| MD5 `rust/gui-spike/src/settings_store.rs` | `md5sum` | `654be49c2b2627baf20b77a67363a01b` — совпал с REPORT §5 |
| MD5 `rust/gui-spike/tests/settings_contract.rs` | `md5sum` | `62920daa758f3755e66b7bfe663deb56` — совпал с REPORT §5 |
| MD5 `red.log` / `green.log` / `mutations.log` | `md5sum` | `60bbdeff…`, `75c91f13…`, `30bb3424…` — все совпали с REPORT §5 |
| Контрактные тесты | `CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test --manifest-path rust/gui-spike/Cargo.toml --test settings_contract` | `26 passed; 0 failed` (exit 0) |
| Clippy | `cargo clippy --manifest-path rust/gui-spike/Cargo.toml --lib --test settings_contract -- -D warnings` | 0 предупреждений (exit 0) |
| Сборка всех целей крейта | `cargo check --manifest-path rust/gui-spike/Cargo.toml --all-targets` | чисто (exit 0) |
| Мутации 5/5 | собственный прогон `/tmp/opencode/rw40-43a-mutations.py`, файл восстановлен по MD5 | 5/5 KILLED, MD5 после прогона `654be49c…` |

Мутации воспроизведены независимо от лога автора; убившие тесты в каждом прогоне совпали с `mutations.log` построчно:

| # | Мутант | Итог | Убито тестов |
|---|---|---|---|
| M1 | убрать `value == HOME_GUILD_ID \|\|` | KILLED (exit 101) | 4 |
| M2 | `value.starts_with('@')` вместо точного `@me` | KILLED (exit 101) | 5 |
| M3 | `is_snowflake` вместо `is_valid_last_guild_id` в `validate` | KILLED (exit 101) | 4 |
| M4 | не обновлять `home_mode` в `apply_patch` | KILLED (exit 101) | 1 |
| M5 | не нормализовать `lastGuildId` в `apply_patch` + `SettingsStore::patch` | KILLED (exit 101) | 1 |

## 2. Существо изменения

Требование P1 из `waves/RW-40/DESIGN/DM-FRIENDS-SPEC-REVIEW-PASS2.md` §P1.1 закрыто в backend-части:

- `HOME_GUILD_ID = "@me"` введён и пропускается ровно одним общим предикатом `is_valid_last_guild_id`, который используют `normalize_last_guild_id`, `deserialize_last_guild_id` и `UiSettings::validate`. Три точки не могут разойтись — это правильная форма для «одно правило на все входы».
- Точное сравнение, а не префикс: `"@you"`, `" @me"`, `"@me "`, `"@ME"`, `"abc"`, не-строки и переполнение `u64` отбрасываются в `None`. Тесты `non_snowflake_last_guild_id_normalizes_to_none`, `save_drops_non_snowflake_last_guild_id_without_failing`, `validate_rejects_non_snowflake_last_guild_id` бьют именно по этим границам, а не по реализации.
- Канонический 5-полевой JSON не изменился: оба новых поля (`lastGuildId`, `homeMode`) при `None` не сериализуются, это зафиксировано точным сравнением `Value` в `serialized_defaults_are_canonical_and_hold_no_secrets`.
- Полный цикл `save → load` для `lastGuildId: "@me"` проверен на диске (`value["lastGuildId"] == "@me"`), а не только через API.

Оракулы в тестах внешние: `ui/index.html`, `ui/features/settings.js`, содержимое файла на диске. Подгонки теста под код нет, `.only`/`skip`/`todo` отсутствуют.

## 3. P2 — не блокируют приёмку, но требуют решения

### P2-1. Реализованы оба варианта из P1.1 сразу: `lastGuildId: "@me"` **и** `homeMode`. Инвариант между ними не задан

PASS2 §P1.1 предлагает «одно из двух»: либо отдельное булево поле (A), либо расширение G-01b до `"@me"` (B). Карточка делает и A, и B. Каждое поле само по себе безвредно, но они описывают одно состояние и могут противоречить друг другу:

- `UiSettings { last_guild_id: Some(GUILD_ID), home_mode: Some(true) }` и `{ last_guild_id: Some("@me"), home_mode: Some(false) }` проходят `validate` без ошибки. Тест `settings_patch_updates_home_mode_and_last_guild_id_preserving_other_fields` фиксирует только «правильные» комбинации, инвариант не проверяется.
- Спецификация (`DM-FRIENDS-SPEC.md` §2.1.1–§2.1.2) определяет boot по `lastGuildId === "@me"`; потребителя у `homeMode` в замороженном UI нет (`grep homeMode` по `rust/gui-spike/ui` и `tests/unit` — 0 вхождений). Фронтенд, сохраняющий настройки через `normalizeSettings`, вообще не отправляет `homeMode`, то есть при любом сохранении из диалога настроек `homeMode: true` на диске превратится в отсутствие ключа.
- `Option<bool>` даёт третье состояние (`None` ≠ `Some(false)`), тогда как вариант A в ревью описан как булево с дефолтом `false`.

Что нужно: либо оставить только B (тогда `homeMode` и `UiSettingsPatch.home_mode` удаляются до заморозки контракта), либо, если оба поля остаются, зафиксировать инвариант в `validate` (`home_mode == Some(true) ⇒ last_guild_id == Some(HOME_GUILD_ID)`) и записать выбор в §0.2 как `[F]`-изменение. Сейчас это не дефект поведения, а неразрешённая двусмысленность контракта, которую следующая карточка G2/G3 унаследует.

### P2-2. Шапка модуля противоречит новому поведению

`rust/gui-spike/src/settings_store.rs:13-15`: «Значение — только snowflake; пустое/чужое при чтении и записи нормализуется в `None`». После изменения допустимы два значения (`@me` и snowflake). Доккомментарий поля `last_guild_id` и сообщение `Invalid` обновлены, шапка — нет. Правка на одну строку, но именно этот файл читают следующие воркеры.

### P2-3. Невалидный `lastGuildId` в patch молча стирает сохранённый выбор

`UiSettings::apply_patch` (`settings_store.rs:242-244`) при `patch.last_guild_id = Some("@you")` записывает `None`, а `SettingsStore::patch` сохраняет это на диск. То есть опечатка в патче не отвергается и не игнорируется, а обнуляет ранее сохранённую гильдию. Тест `settings_patch_...` закрепляет именно такое поведение (строка 882: `after_bad.last_guild_id == None`). Это согласовано с `save`, но у partial-API последствие сильнее: `patch` — единственный способ обновить одно поле, и он не должен терять другое. Предпочтительно `Invalid { field: "lastGuildId" }` или игнор поля, но это изменение контракта — решает Team Lead.

## 4. P3

- `UiSettingsPatch.last_guild_id` — обычный `Option<String>` без `deserialize_last_guild_id`: числовой `lastGuildId` в патче даёт `Parse`-ошибку serde, тогда как в файле то же значение молча читается как `None`. Расхождение допустимое, но не описанное.
- Ни `lastGuildId`, ни `homeMode` нельзя сбросить в `None` через patch: `null` и отсутствие ключа неразличимы. Для `homeMode` это мешает вернуть «состояние не задано».

## 5. Открытый пункт вне границ карточки (не влияет на вердикт)

P1 из PASS2 закрыт **только в Rust**. End-to-end режим Home по-прежнему не переживает перезапуск, потому что три UI-фильтра из того же ревью не тронуты и принимают только цифры:

- `rust/gui-spike/ui/app.js:335-339` — `readLastGuildId`, `/^\d{1,20}$/`;
- `rust/gui-spike/ui/features/settings.js:209-212` — `normalizeSettings`;
- `rust/gui-spike/ui/features/settings.js:281-285` — `getEffectiveLastGuildId`.

Отчёт карточки это честно фиксирует (§6), файлы вне write-scope. Требуемый PASS2 оракул `T-N107-HOME-NAV-01` (полный цикл `settings_save → settings_load → boot`) в репозитории не существует — `grep` находит только его упоминание в спецификации. Приёмка родительского пункта DM-FRIENDS возможна только после карточки G2/G3, которая снимет эти три фильтра и добавит сквозной тест.

## 6. Границы и процесс

- Изменены ровно два файла карточки: `rust/gui-spike/src/settings_store.rs`, `rust/gui-spike/tests/settings_contract.rs`. Артефакты — в `waves/RW-40/PM/task122-settings-home/`. Golden-каталоги (`logs/`, `functions/`, `evidence/`) не тронуты, сетевых вызовов нет.
- `red.log` правдоподобен: шаг 1 — runtime-падение на `@me` при 24 тестах, шаг 2 — 9 ошибок компиляции, и номера строк в ошибках (`src/settings_store.rs:470`) соответствуют старой версии файла, а не текущей (метод `path` сейчас на строке 550).
- Замечание к процессу: работа лежит в рабочем дереве поверх `4e24048` без коммита; это соответствует правилу «коммитит координатор», но отчёт называет HEAD как точку ветки, а не точку с изменением.

## 7. Итог

Backend-часть `lastGuildId: "@me"` сделана корректно и доказана: точный предикат в одном месте, `save → load` без потерь, негативные кейсы на все соседние формы, 26/26 тестов, чистый clippy, 5/5 мутаций убиты независимым прогоном, MD5 всех артефактов сходятся. Карточка принимается (**PASS**). До заморозки контракта нужно решить P2-1 (дублирующий `homeMode` без инварианта) и поправить шапку модуля (P2-2); P1 родительского пункта остаётся открытым до G2/G3.
