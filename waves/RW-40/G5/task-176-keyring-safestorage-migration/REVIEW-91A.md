# Техническое ревью 91A — PR-62 / task-176 (BE-P3-08: миграция токена из Electron safeStorage)

- **Ревьюер:** Технический ревьюер, Review 91A (`deepseek/deepseek-v4.1-flash`)
- **Дата прогонов:** 2026-10-31 03:20–03:50 (+08:00, CST), rustc/cargo 1.99.0, rustfmt 1.10.0-stable
- **Ветка:** `feature/task-176-keyring-safestorage-migration`, HEAD `bbb6068` (рабочее дерево, коммитов нет)
- **База:** `unstable` = `c07e610`; ветка отстаёт на 2 мержа (PR-57 `38f7b5c`, PR-58 `c07e610`), не опережает
- **PR:** PR-62 `feature/task-176-keyring-safestorage-migration -> unstable`
- **PM-приёмка:** `ACCEPT.md`, статус `Ready for Technical Review`

## Вердикт: **NOT APPROVED (возвращено на доработку)**

- **P0 = 0**
- **P1 = 1** — обязательная мутация №5 («утечка секрета в `Debug`/`Display`») **не убита**: тест
  `k17_migration_errors_do_not_leak_secrets` тавтологичен, утечка проходит мимо всех 12 offline-тестов
- **Мутации: 4/5 обязательных убиты**, 2 живых мутанта по классу «утечка в Debug» (M5, M6)
- Тесты: `12 passed / 13 ignored / 0 failed` в `keyring_contract`, `1/1` в lib, полный крейт `129 passed / 13 ignored / 0 failed`
- Clippy с `-D warnings`, `cargo fmt --check`: чисто

Боевой код утечки не содержит: в `SafeStorageMigrationError` нет полей с данными, логирующих
макросов в `keyring.rs` нет (`grep` по `println!|eprintln!|log::|tracing|dbg!` — пусто). Проблема
в доказательстве: тест безопасности не может упасть ни на какой реалистичной утечке, поэтому
DoD-пункт «5/5 мутаций убито» не выполнен. Фикс — только тестовый файл, одна новая тест-функция.

## 1. Границы изменений

`git status --short` — ровно два файла из write-scope ТЗ, лишнего нет:

```
 M rust/crates/discord-platform/src/keyring.rs
 M rust/crates/discord-platform/tests/keyring_contract.rs
```

`git diff --stat` против `unstable`: `keyring.rs` +239, `keyring_contract.rs` +304/−1. Воркер
коммитов не делал, источник ревью — рабочее дерево.

Замечание по базе: `git diff --name-only unstable` дополнительно показывает 5 файлов
(`discord-core/src/settings/*`, `discord-core/tests/settings_proto_http.rs`, `gui-spike/src/attachments*`).
Это не правки воркера — это содержимое `unstable`, ушедшее вперёд после отведения ветки
(`HEAD..unstable` = PR-57/PR-58). Файлы keyring эти коммиты не трогают, конфликта при rebase не будет.

md5 совпадают с §4 `REPORT.md` и восстановлены после каждого мутационного прогона (`md5sum` сверен):

| файл | md5 (REPORT.md) | md5 (после мутаций) |
| --- | --- | --- |
| `rust/crates/discord-platform/src/keyring.rs` | `934bfb9f9cefd76fb67640c07909c68c` | `934bfb9f9cefd76fb67640c07909c68c` |
| `rust/crates/discord-platform/tests/keyring_contract.rs` | `08e66e11b0d0bb312f939468dab73dab` | `08e66e11b0d0bb312f939468dab73dab` |

## 2. Прогоны

Команды из ТЗ, `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`:

```
cargo test   --manifest-path rust/Cargo.toml -p discord-platform --test keyring_contract --lib
  -> unittests src/lib.rs:            1 passed; 0 failed
  -> tests/keyring_contract.rs:      12 passed; 0 failed; 13 ignored  (k01..k12, k25 = live, skip-aware)

cargo clippy --manifest-path rust/Cargo.toml -p discord-platform --test keyring_contract --lib -- -D warnings
  -> Checking discord-platform ... Finished `dev` profile, 0 warnings, 0 errors (exit 0)

cargo fmt --manifest-path rust/Cargo.toml -p discord-platform --check
  -> чисто (exit 0)
```

Полный крейт `cargo test -p discord-platform`: `lib 1`, `autostart 9`, `dbus 23`, `deep_link 17`,
`keyring_contract 12 (+13 ignored)`, `keyring_offline 3`, `notifications 25`, `power 16`,
`spellcheck 13`, `tray 10` — **129 passed, 0 failed, 13 ignored**. Регрессий нет; новых публичных
символов никто, кроме теста, не использует (`grep -rn "SafeStorageMigrator|ENCRYPTED_TOKEN_PREFIX|
is_safe_storage_token"` вне `keyring.rs`/`keyring_contract.rs` — пусто).

Про общий таргет (риск 6.1 из REVIEW-88A): первый прогон отдал артефакты воркера за 0.06 с, но в
списке тестов были `k13..k25`, которых нет ни в одном другом дереве, то есть бинарь собран из этого
рабочего дерева. Каждый мутационный прогон компилировал заново (`Compiling discord-platform`
в выводе), поэтому kill-результаты не кэшированные.

Red-first подтверждён содержимым `red.log`: E0432 на отсутствующие `SafeStorageMigrator`,
`SafeStorageMigrationError`, `extract_safe_storage_ciphertext`, `is_safe_storage_token`,
`ENCRYPTED_TOKEN_PREFIX` — ошибка отсутствия реализации, а не подгонка тестов.

## 3. Мутационное тестирование: 4/5 обязательных убито, 1 выживший (+1 родственный)

Каждая мутация вносилась в `keyring.rs`, прогонялся `keyring_contract --lib`, файл восстанавливался
из бэкапа, md5 сверялся (`match=True` после каждого прогона).

| # | Мутация | Результат прогона | Итог |
| --- | --- | --- | --- |
| M1 | `ENCRYPTED_TOKEN_PREFIX = "dQw4w9WgXcY:"` | `9 failed` (k13, k14, k18–k24) | **KILLED** |
| M2 | пустой payload после префикса → `Ok(Vec::new())` | `k15_..._rejects_empty_payload FAILED` | **KILLED** |
| M3 | снята проверка `token.contains(['\0','\n','\r'])` | `k23_migrator_rejects_invalid_tokens FAILED` | **KILLED** |
| M4 | снят `if self.migrated { return Ok(false) }` в `migrate_once` | `k19_migrator_idempotent_no_reimport FAILED` | **KILLED** |
| M5 | `Debug` для `SafeStorageMigrationError` печатает сырой safeStorage-entry, захваченный в `migrate_once` | `12 passed; 0 failed` | **SURVIVED** |
| M6 | в `SafeStorageMigrator` добавлено поле `last_raw: String` (derived `Debug`) | `12 passed; 0 failed` | **SURVIVED** |

M5 (публичный API не меняется, тесты компилируются как есть):

```rust
static LEAKED_ENTRY: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

impl fmt::Debug for SafeStorageMigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let leaked = LEAKED_ENTRY.lock().map(|v| v.clone()).unwrap_or_default();
        write!(f, "SafeStorageMigrationError({leaked})")
    }
}
// в migrate_once, сразу после проверки is_safe_storage_token:
// *LEAKED_ENTRY.lock().expect("leak slot") = raw_entry.to_owned();
```

M6 ещё реалистичнее: мигратор «запоминает» последнюю строку (полезно для диагностики), `Debug`
выводит её вместе с ciphertext. Ни один тест этого не замечает.

M5b (`InvalidToken(String)` с расшифрованным токеном + `Display` печатает его) — **невалидный
мутант**: `k17` и `k23` конструируют `SafeStorageMigrationError::InvalidToken` как unit-вариант,
тесты не компилируются (E0277/E0308). То есть единственный способ «уронить» этот класс —
изменить сигнатуру публичного типа, а не поймать утечку проверкой. Поведенческого kill нет.

## 4. Почему P1: `k17` ничего не проверяет

```rust
let errors = [
    SafeStorageMigrationError::EmptyPayload,
    SafeStorageMigrationError::InvalidBase64,
    SafeStorageMigrationError::DecryptFailed,
    SafeStorageMigrationError::InvalidToken,
    SafeStorageMigrationError::AlreadyMigrated,
    SafeStorageMigrationError::Storage(KeyringError::InvalidSecret),
];
for error in &errors { /* !debug.contains(secret_payload) */ }
```

Все шесть вариантов сконструированы литералами и **не имеют полей с данными** — секрету неоткуда
взяться. Утверждение «Debug не содержит `super-secret-raw-token-data`» верно при любой реализации
`Debug`, которая не читает глобальное состояние. Тест не проходит ни через один реальный путь
`migrate_once` (ошибка расшифровки, невалидный токен, ошибка хранилища) и не смотрит на
`Debug` мигратора — ровно те места, где секрет физически присутствует. Отсюда M5/M6 и выживают.

**Фикс (P1-1, только `tests/keyring_contract.rs`, одна функция):**

```rust
#[test]
fn k26_migration_failure_paths_and_state_do_not_leak_secrets() {
    let raw = "dQw4w9WgXcQ:c2VjcmV0LWNpcGhlcnRleHQ="; // base64("secret-ciphertext")
    let secret_token = "mfa.super_secret_token_value_123";

    let mut migrator = SafeStorageMigrator::new();
    let err = migrator
        .migrate_once(raw, |_| Err(SafeStorageMigrationError::DecryptFailed), |_| Ok(true))
        .unwrap_err();
    for text in [format!("{err:?}"), err.to_string()] {
        assert!(!text.contains("c2VjcmV0") && !text.contains(raw) && !text.contains("secret-ciphertext"));
    }

    let mut migrator = SafeStorageMigrator::new();
    let err = migrator
        .migrate_once(raw, |_| Ok(format!("{secret_token}\0").into_bytes()), |_| Ok(true))
        .unwrap_err();
    for text in [format!("{err:?}"), err.to_string()] {
        assert!(!text.contains(secret_token) && !text.contains(raw) && !text.contains("c2VjcmV0"));
    }

    let mut migrator = SafeStorageMigrator::new();
    let err = migrator
        .migrate_once(raw, |_| Ok(b"valid-token".to_vec()), |_| Err(KeyringError::InvalidSecret.into()))
        .unwrap_err();
    for text in [format!("{err:?}"), err.to_string()] {
        assert!(!text.contains("c2VjcmV0") && !text.contains(raw) && !text.contains("valid-token"));
    }

    let mut migrator = SafeStorageMigrator::new();
    assert_eq!(
        migrator.migrate_once(raw, |_| Ok(b"valid-token".to_vec()), |_| Ok(true)),
        Ok(true)
    );
    let debug = format!("{migrator:?}");
    assert!(!debug.contains("c2VjcmV0") && !debug.contains(raw), "Debug мигратора содержит ciphertext: {debug}");
}
```

Проверено на временной копии (тест-файл после прогона восстановлен, md5 сверен): на текущем коде
`k26` зелёный (`13 passed`), на M5 и M6 падает с содержательными сообщениями:

```
M5: panicked at keyring_contract.rs:686: ошибка расшифровки раскрывает ciphertext:
    SafeStorageMigrationError(dQw4w9WgXcQ:c2VjcmV0LWNpcGhlcnRleHQ=)
M6: panicked at keyring_contract.rs:720: Debug мигратора содержит ciphertext:
    SafeStorageMigrator { migrated: true, last_raw: "dQw4w9WgXcQ:c2VjcmV0LWNpcGhlcnRleHQ=" }
```

После добавления мутация №5 убивается поведенчески, а не ошибкой компиляции.

## 5. Находки

### P0 — нет

### P1 — 1 (блокирует merge)

**P1-1. Тест утечки секрета в `Debug`/`Display` тавтологичен; обязательная мутация №5 не убита.**
Подробности и фикс — §4. Затрагивает DoD BE-P3-08 («без утечки секретов в логи/Debug») и
`TEST-POLICY.md` §9 «Новые инварианты покрыты; мутации caught».

### P2 (не блокируют merge отдельно, но P1-1 всё равно надо закрыть)

**P2-1. Вариант `SafeStorageMigrationError::AlreadyMigrated` не конструируется нигде.** Мёртвая
часть публичного API: `migrate_once`/`migrate_keyring` в этом случае возвращают `Ok(false)`, а не
ошибку. Либо использовать вариант на пути «уже мигрирован, а вызывающий ждёт ошибку», либо убрать.

**P2-2. `extract_safe_storage_ciphertext` путает «нет префикса» и «битый base64»** — оба дают
`InvalidBase64`. Диагностика в логах теряет различие (и это единственное место, где в будущем
легко случайно вывести `payload` в сообщение). Отдельный вариант `MissingPrefix` или явный
комментарий в doc-строке закроют вопрос.

**P2-3. Асимметрия детектора и извлекателя.** `is_safe_storage_token("dQw4w9WgXcQ:")` возвращает
`true` (зафиксировано в `k13`), а `extract_safe_storage_ciphertext` на той же строке даёт
`EmptyPayload`. Поведение согласовано с ТЗ (префикс есть — значит кандидат на миграцию, пустой
payload отвергается на следующем шаге), но это стоит написать в doc-строке `is_safe_storage_token`.

**P2-4. Base64-декодер принимает неканонические хвостовые биты** (`"YR=="` декодируется в `'a'`,
как и `"YQ=="`). Для safeStorage-входа безвредно (ciphertext уходит в decrypt, который его
отвергнет), размер входа ограничен записью keyring. Комментарий в `decode_base64` снимет вопрос
при следующем чтении кода.

**P2-5. Ветка отстаёт от `unstable` на 2 мержа (PR-57, PR-58).** Конфликтов в файлах PR-62 нет,
но merge потребует rebase — шаг координатора, не воркера.

## 6. Что осталось

- Воркеру: добавить `k26` (§4) в `tests/keyring_contract.rs`, прогнать
  `cargo test -p discord-platform --test keyring_contract --lib` и clippy, приложить red/green
  мутационного прогона M5/M6 в `REPORT.md` (файл `keyring.rs` менять не нужно — боевой код корректен).
- Ревьюеру после фикса: повторить M1–M6 и проверить, что все шесть убиты.
- Merge в `unstable` — координатор/интегратор после PM + повторного технического ревью; коммитов
  воркер и ревьюер не делают. `git status` после ревью содержит только два файла PR, md5 совпадают
  с §4 `REPORT.md`.
- Полный `gate:worker` не запускался: по ТЗ ревью ограничено targeted-прогоном `discord-platform`
  + clippy; полный гейт — за integration-лейном.
