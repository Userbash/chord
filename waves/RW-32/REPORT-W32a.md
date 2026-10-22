# Отчёт воркера — RW-32 / P6-07 / W32a (PLATFORM-2)

Батч: RW-32, карточка P6-07 «Spellcheck (headless)». Дата: 2026-10-22.
Крейт: `discord-platform`. Системная библиотека: `/usr/lib/libhunspell-1.7.so` (1.7.5).
Сеть не использовалась, `git commit/push` не выполнялись, workspace `Cargo.toml`/`Cargo.lock`
не менялись.

## 1. Границы

- Изменены/созданы только файлы ТЗ:
  - `rust/crates/discord-platform/src/spellcheck.rs` — новый безопасный API + токенайзер;
  - `rust/crates/discord-platform/src/ffi_hunspell.rs` — новый, единственное место с `unsafe`;
  - `rust/crates/discord-platform/src/lib.rs` — только `pub mod spellcheck;` + `mod ffi_hunspell;`
    (с `#[allow(unsafe_code)]`);
  - `rust/crates/discord-platform/Cargo.toml` — одна строка: `unsafe_code = "forbid"` → `"deny"`;
  - `rust/crates/discord-platform/tests/spellcheck_contract.rs` — контрактные тесты h01–h13;
  - `fixtures/hunspell/test.aff`, `fixtures/hunspell/test.dic` — синтетический словарь;
  - `waves/RW-32/run-mutations-w32a.py`, `waves/RW-32/mutations-w32a.log`,
    `waves/RW-32/workers/W32a/{red.log,green.log}`, этот отчёт.
- Не трогал: `dbus/`, `notifications.rs`, `power.rs`, `discord-store`, чужие тесты, доки, golden,
  workspace-манифесты, `package.json`. Изменения W32b в рабочем дереве (`notifications.rs`,
  `dbus/message.rs`, `tests/dbus_contract.rs`, `tests/notifications_contract.rs`,
  `fixtures/dbus/hints.json`) — не мои.
- Фикстуры словаря в тестах копируются во временный каталог; исходники не изменяются
  (md5 до/после сверяет мутационный скрипт: `d2cd9b…`, `70e5b0…`).

## 2. Сделано

- `src/ffi_hunspell.rs` — тонкая обёртка `extern "C"` с `#[link(name = "hunspell-1.7")]`.
  C-API hunspell экспортирует символы с заглавной буквы, поэтому контрактные имена
  (`hunspell_create/destroy/spell/suggest/add/remove/free_list`) связаны через `#[link_name]`
  с `Hunspell_*`.
  - `Handle` владеет `NonNull<Hunhandle>`; `Drop` вызывает `hunspell_destroy` ровно один раз
    (тип не `Clone`, владелец единственный).
  - `create` передаёт пути `CString` без NUL; NULL → `FfiError` (без паники).
  - `suggest`: `char***`-out, копирование строк в `Vec<String>`, затем `Hunspell_free_list`
    с исходным числом вариантов (а не с длиной усечённого списка).
  - 11 `unsafe`-блоков, у каждого `// SAFETY:`; вне ffi `unsafe` в крейте нет.
- `src/spellcheck.rs` — замороженный контракт §1 TZ: `Token{text,start,end}`, `SpellChecker`,
  `SpellError`, `SpellChecker::{new,check,suggest,add,remove}`, `tokenize`.
  - `new`: UTF-8 путь + NUL-guard (`Nul`), проверка наличия/непустоты `.aff`, заголовка `.dic`
    (`Missing`/`InvalidDictionary`), затем `hunspell_create` (`Create` при NULL). Проверки
    обязательны: эмпирически `Hunspell_create` 1.7.5 на отсутствующих файлах возвращает
    **не-NULL** handle (пишет `cannot open` в stderr), поэтому без pre-check h10 невыполним.
  - `check`/`suggest`/`add`/`remove` делегируют в hunspell; `suggest` — не более 10 вариантов
    (`MAX_SUGGESTIONS`), порядок библиотеки; NUL в слове → `false`/пустой список (не обрезаем).
  - `Drop` — в ffi; `SpellChecker` — `!Send + !Sync` (`NonNull` + явный `PhantomData<*mut ()>`),
    что проверяется компилятором в h13.
  - `tokenize` — чистый Rust: Unicode-буквы (`char::is_alphabetic`), внутри слова `'`/`-` только
    перед буквой; цифры/`_` разрывают слово; пропуски: whitespace-токен с `://` или `@`,
    содержимое `` `…` `` (включая незакрытое), `<@…>`/`<#…>`, `:emoji:`. Смещения байтовые,
    срез исходной строки всегда равен `Token::text`.
- `Cargo.toml`: `unsafe_code = "deny"`; `lib.rs`: `#[allow(unsafe_code)] mod ffi_hunspell;`.

## 3. Red

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w32a \
  cargo test --offline -p discord-platform --test spellcheck_contract
# EXIT=101
# error[E0432]: unresolved import `discord_platform::spellcheck`
```

Лог: `waves/RW-32/workers/W32a/red.log` (последняя строка `# EXIT=101`). Red снят до
реализации: тесты ссылаются на контракт, которого в крейте ещё нет.

## 4. Green

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w32a \
  cargo test --offline -p discord-platform --test spellcheck_contract
# EXIT=0; 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Лог: `waves/RW-32/workers/W32a/green.log` (`# EXIT=0`). Предупреждений компилятора нет,
`SKIP:` нет. Повторный прогон после мутаций — тот же результат.

## 5. Тесты (`tests/spellcheck_contract.rs`, 13 из бюджета 11–13)

| ID | Тест | Проверка |
|---|---|---|
| h01 | `h01_new_on_fixtures_is_ok` | `new` на temp-копии фикстур → Ok; заголовок `.dic` = 6 |
| h02 | `h02_check_accepts_dictionary_words` | hello/world/привет/мир/don't/well-known → true |
| h03 | `h03_check_rejects_unknown_words` | helo/приветъ/zzzz/hello123 → false |
| h04 | `h04_suggest_finds_hello_and_is_bounded` | suggest("helo") содержит hello, ≤10, повтор детерминирован |
| h05 | `h05_add_and_remove_change_check` | add → check true; remove → check false |
| h06 | `h06_tokenize_ascii_offsets_and_punctuation` | смещения ASCII, пунктуация/цифры/`_` не в слове |
| h07 | `h07_tokenize_cyrillic_hyphen_apostrophe` | кириллица, дефис/апостроф только внутри слова |
| h08 | `h08_tokenize_skips_urls_emails_code_mentions_and_emoji` | URL/e-mail/`` `code` ``/`<@alice>`/`<#general>`/`:emoji_1:` пропущены |
| h09 | `h09_empty_and_wordless_texts_are_empty` | пустой/безсловный/незакрытый код → [] |
| h10 | `h10_broken_or_missing_dictionary_is_error` | нет aff, нет dic, пустые aff/dic, битый заголовок → `SpellError` |
| h11 | `h11_drop_cycles_do_not_leak_or_crash` | 100 циклов create/drop + RSS-прирост < 8 MiB (прогрев 20) |
| h12 | `h12_nul_in_path_is_error_without_panic` | NUL в aff/dic → `SpellError::Nul` |
| h13 | `h13_not_send_sync_and_tokenize_never_panics` | `!Send + !Sync` (компиляционно) + произвольный Unicode без паник |

Оракул: системная `libhunspell 1.7.5` + синтетический словарь; CLI-оракула (hunspell/aspell)
в окружении нет, поэтому проверяются семантические инварианты контракта, а не точный вывод
конкретной версии. Skip-aware: при отсутствии `libhunspell` каждая FFI-зависимая проверка печатает
`SKIP: <причина>`; в этом окружении `SKIP=0`.

## 6. Мутации (`run-mutations-w32a.py`, 8 из бюджета 6–8)

Baseline: exit=0, `SKIP=0`, `test result: ok`. Все мутации убиты (`KILLED` — только
`test result: FAILED` при `SKIP=0`); исходники восстановлены, md5 совпал; фикстуры не изменены.

| ID | Мутация | Файл | Убита тестом |
|---|---|---|---|
| 01 | `word_boundary_trailing_hyphen`: дефис в конце приклеивается | `spellcheck.rs` | h07 |
| 02 | `url_skip_removed`: нет пропуска `://` | `spellcheck.rs` | h08 |
| 03 | `suggest_limit_zero`: `MAX_SUGGESTIONS = 0` | `spellcheck.rs` | h04 |
| 04 | `add_ignored`: `add` не вызывает hunspell, но возвращает true | `ffi_hunspell.rs` | h05 |
| 05 | `drop_without_destroy`: `Drop` не освобождает handle | `ffi_hunspell.rs` | h11 (RSS) |
| 06 | `nul_guard_removed`: NUL в пути больше не `Nul` | `spellcheck.rs` | h12 |
| 07 | `mention_skip_removed`: `<@alice>` даёт слово alice | `spellcheck.rs` | h08 |
| 08 | `spell_always_true`: `check` принимает любое слово | `ffi_hunspell.rs` | h03 (+h05) |

Полный протокол: `waves/RW-32/mutations-w32a.log`. Скрипт пишет лог сам; stdout запускался
в отдельный файл (`/tmp/opencode/w32a-mutations.out`), не в лог.

## 7. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-platform/src/spellcheck.rs` | `bab7bb232b44412ebe1e42910f539474` |
| `rust/crates/discord-platform/src/ffi_hunspell.rs` | `715ab2bbef06a98a27038e2dec24f2fb` |
| `rust/crates/discord-platform/src/lib.rs` | `a9503de65c4c72aa1563daa7a3ba940d` |
| `rust/crates/discord-platform/Cargo.toml` | `873a751a031b1fec5f2193397b17fd3c` |
| `rust/crates/discord-platform/tests/spellcheck_contract.rs` | `04ec3ede4a618d48d0c2d46642593e25` |
| `fixtures/hunspell/test.aff` | `d2cd9ba27bcff9b6100199782b828dcd` |
| `fixtures/hunspell/test.dic` | `70e5b0a545d205933eb793f83049fad4` |
| `waves/RW-32/run-mutations-w32a.py` | `f8615337de6675b1499970c3b53dc567` |
| `waves/RW-32/mutations-w32a.log` | `6ee94626df6fd6b965e0f9cd79c396f9` |
| `waves/RW-32/workers/W32a/red.log` | `05045d4a6beb865af90eb6b6047b4ec7` |
| `waves/RW-32/workers/W32a/green.log` | `d9b3b4bbf2f5674daf448c15c1da5ed4` |

## 8. Риски/ограничения и вопросы

- **Системная зависимость.** Линковка `-lhunspell-1.7` — compile-time; отсутствие библиотеки
  ломает сборку (рантайм-`SKIP` в тестах страхует только сценарий «библиотека есть при сборке»).
  Для CI/упаковки нужен пакет `libhunspell-1.7` (в окружении — 1.7.5).
- **Нет CLI-оракула.** `hunspell`/`aspell`-бинарей и системных словарей нет; проверяются
  инварианты контракта, а не точный список suggest конкретной версии. h04 намеренно проверяет
  «содержит hello + ≤10 + детерминизм», а не полный порядок.
- **Синтетический словарь.** 6 слов, без аффиксных правил; реальные словари/аффиксы не покрыты.
  Кодировка предполагается UTF-8 (`SET UTF-8`); для других кодировок строки suggest
  декодируются как lossy.
- **«Битый словарь».** Проверяются отсутствие файла, пустой файл и нечисловой/нулевой заголовок
  `.dic`. Глубокая порча внутри `.aff` не детектируется: hunspell 1.7.5 молча возвращает handle
  (проверено отдельным пробником). Причина pre-check: на отсутствующих файлах библиотека тоже
  возвращает не-NULL, поэтому контрактное «NULL → ошибка» само по себе h10 не покрывает.
- **Drop/утечки.** h11 — RSS-эвристика (100 циклов, прогрев 20, порог 8 MiB), а не valgrind;
  baseline-прирост 0, мутация «без destroy» даёт ~14 МиБ и убивается. Параллельные тесты могут
  добавлять шум, запас порога большой.
- **`!Send + !Sync`** подтверждается компиляционно (ambiguity-трюк в h13): если кто-то добавит
  `Send`/`Sync`, тест перестанет компилироваться. Отдельного UI-потока/пула в карточке нет.
- **Токенайзер.** URL/e-mail определяются на уровне whitespace-токена: токен, содержащий `://`
  или `@`, пропускается целиком (слова, склеенные с e-mail без пробела, тоже исчезают) —
  осознанно и задокументировано в модуле. `:a:` (1 символ) emoji не считается.
- **Гейт.** По правилам batch воркер запускал только targeted-тесты; `cargo fmt`/`clippy`/
  `--workspace`/`npm run gate:worker` не запускались (это integration-лейн). Форматирование —
  ручное, под общий `cargo fmt` не проверялось.
- Вопросов, блокирующих приёмку, нет.

## 9. Передача

- next: Team Lead / независимое ревью W32a → integration-лейн RW-32.
- batch gate: не запускал — у Integration/QA; targeted-логи приложены.

## После интеграционного fmt/clippy (2026-10-22)

`cargo fmt` и два clippy-фикса (`collapsible_if` для `:emoji:`, объединение идентичных ветвей
классификации слова) изменили байты продукта. Переснято на финальных байтах:

- red: `E0599` на отсутствующем `SpellChecker::check`, `# EXIT=101`;
- green: 13 passed, `# EXIT=0`;
- мутации: якорь 01 приведён к post-fmt коду, прогон **8/8 KILLED, baseline SKIP=0**,
  restore продукта и фикстур по md5.

## После codex-ревью (P1)

Codex: «доработка» — валидация «битого» словаря принимала мусор (`1\n@@@`). Закрыто:

- `.aff` обязан содержать директиву `SET`; `.dic` — счётчик, совпадающий с числом записей, и
  записи только из букв/`'`/`-` (флаг после `/` не проверяется); иначе `InvalidDictionary`;
- h10 расширен кейсами `@@@`, отсутствие `SET`, несовпадение счётчика;
- red/green/мутации пересняты на финальных байтах: red `101`, green 13 passed, **8/8, SKIP=0**;
- md5 обновлены (таблица выше).
