# Technical Review 127A — task-209 (PR-98): контракт форум-публикаций

## Verdict: APPROVED

- **Карточка**: `task-209` (`BE-P1-07` / `forum`), волна `RW-40`, отдел `G1`.
- **Ветка / worktree**: `feature/task-209-be-forum-contract`.
- **Ревьюер**: Technical Reviewer (DeepSeek), review-сессия 127A.
- **Дата**: 2026-10-31.

## 1. Целевой прогон (после восстановления файлов)

```
CARGO_TARGET_DIR=/home/user/orca/project/rust/target \
cargo test --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test forum_contract
```

```text
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Замечание по команде из ТЗ: `--manifest-path rust/Cargo.toml` — это workspace-манифест,
в нём пакет `discord-gui-spike` не зарегистрирован как member, поэтому команда падает с
`package ID specification discord-gui-spike did not match any packages`. Рабочий вариант —
`--manifest-path rust/gui-spike/Cargo.toml` (пакет и тест-бинарь те же).

## 2. Соответствие контракту и оракулу (по коду и тестам)

- `POST /channels/{channel_id}/threads`, тело `{"name", "auto_archive_duration", "applied_tags", "message": {"content"}}`
  плюс опциональный `rate_limit_per_user` — совпадает с `rest-static.json` (`CHANNEL_THREADS`).
- `MAX_FORUM_APPLIED_TAGS = 5`, `ALLOWED_AUTO_ARCHIVE_MINUTES = [60, 1440, 4320, 10080]`,
  `ForumSortOrder` (`0 = LatestActivity`, `1 = CreationDate`) — совпадает с `gateway-schema.md`.
- Валидация по `.chars().count()` после `trim()` (Unicode, не байты) — корректно для кириллицы.
- Snowflake: только ASCII-цифры, `u64`, `> 0` — границы `1` и `u64::MAX` покрыты, `u64::MAX + 1` отклоняется.
- Тесты проверяют контракт (маршрут, JSON-тело, коды ошибок), а не внутреннюю реализацию;
  ослаблений и `#[ignore]` нет.

## 3. Мутационное тестирование: 5/5 killed

Каждая мутация вносилась в `rust/gui-spike/src/forum.rs` по одной, прогонялся тот же targeted-тест,
затем файл восстанавливался из байтовой копии.

| # | Мутация | Ожидание | Результат прогона | Итог |
|---|---|---|---|---|
| M1 | `MAX_FORUM_APPLIED_TAGS: usize = 5` → `6` | тест констант и лимита тегов падает | `FAILED. 8 passed; 2 failed` | KILLED |
| M2 | условие `!(1..=MAX_FORUM_TITLE_CHARS).contains(&title_chars)` → `title_chars == 0` (снят верхний предел 100) | падает граничный тест 101 символа | `FAILED. 9 passed; 1 failed` | KILLED |
| M3 | `Ok(n) if n > 0 => Ok(())` → `Ok(_) => Ok(())` (принимается Snowflake `0`) | падают тесты `channel_id="0"`/`"0000"` и тегов `"0"`/`"00"` | `FAILED. 7 passed; 3 failed` | KILLED |
| M4 | `if !ALLOWED_AUTO_ARCHIVE_MINUTES.contains(...)` → `if false` (обход whitelist) | падает тест запрещённых значений `auto_archive_duration` | `FAILED. 8 passed; 2 failed` | KILLED |
| M5 | маршрут `"/channels/{}/threads"` → `"/channels/{}/thread"` | падает тест маршрута и тела | `FAILED. 9 passed; 1 failed` | KILLED |

**Итого: 5/5 мутаций убиты (100%), выживших нет.**

## 4. Восстановление файла и md5

| Файл | md5 в REPORT.md | md5 после ревью | Совпадение |
|---|---|---|---|
| `rust/gui-spike/src/forum.rs` | `3b5be56436eca056649ba8533172a5f2` | `3b5be56436eca056649ba8533172a5f2` | ✅ |
| `rust/gui-spike/src/lib.rs` | `95bbf8db66a9efa9fc6d37f45e165c35` | `95bbf8db66a9efa9fc6d37f45e165c35` | ✅ |
| `rust/gui-spike/tests/forum_contract.rs` | `47a3d82fbdc7c1036ff7365285800dae` | `47a3d82fbdc7c1036ff7365285800dae` | ✅ |

`forum.rs` восстановлен байт-в-байт (md5 совпал), мутационных следов в рабочем дереве нет.

## 5. Замечания (не блокирующие, P3)

1. **P3**: `rust/gui-spike/Cargo.lock` перегенерировался при прогоне (`zstd` из `discord-codec`) —
   это артефакт запуска `cargo`, а не PR. После ревью откатан `git checkout -- rust/gui-spike/Cargo.lock`;
   рабочее дерево по этому файлу чистое.
2. **P3**: `M` по `package-lock.json` существовал до задачи (отмечен в REPORT.md) — не относится к PR.
3. **P3 (наблюдение)**: `ForumSortOrder` сериализуется как имя варианта (`"LatestActivity"`), а не как
   числовой код `0/1`. Тест это допускает намеренно (`from_str("\"LatestActivity\"").or_else(from_str("0"))`),
   но если контракт требует числовой JSON на проводе — нужна карточка на `#[serde(rename_all)]`/ручной
   `serialize` с числом. На текущий scope task-209 не влияет.

## 6. Блокеры

`P0 = 0`, `P1 = 0`. `git commit` не выполнялся, файлы PR не изменялись.

**Вердикт: APPROVED** — можно переводить в `Ready for Merge` после одобрения PM.
