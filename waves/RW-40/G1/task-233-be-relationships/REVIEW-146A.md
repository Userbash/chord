# Техническое ревью — PR-117 / task-233 (`relationships`)

**Вердикт: APPROVED** (`P0 = 0`, `P1 = 0`, мутации `5/5`)

- Ветка: `feature/task-233-be-relationships`, HEAD `ae1edc5`.
- Проверяющий: Technical Reviewer (DeepSeek V4.1 Flash), окно review-146A.
- Файлы в scope: `rust/gui-spike/src/relationships.rs` (new), `rust/gui-spike/tests/relationships_contract.rs` (new), `rust/gui-spike/src/lib.rs` (+2 строки: doc + `pub mod relationships;`).

## 1. Targeted test (green)

```
CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target \
  cargo test --manifest-path rust/gui-spike/Cargo.toml --test relationships_contract
running 7 tests
test build_block_user_body_returns_blocked_relationship_type ... ok
test build_relationship_user_route_validates_canonical_snowflake_and_formats_path ... ok
test build_send_friend_request_body_returns_trimmed_username_and_null_discriminator ... ok
test relationships_constants_match_discord_contract ... ok
test send_friend_request_serde_roundtrip ... ok
test validate_friend_request_username_rejects_forbidden_characters ... ok
test validate_friend_request_username_trims_and_enforces_2_to_32_unicode_char_bounds ... ok
test result: ok. 7 passed; 0 failed; 0 ignored
```

## 2. Мутационное тестирование (5/5 killed)

Каждая мутация вносилась в `rust/gui-spike/src/relationships.rs`, прогонялся targeted test, затем файл восстанавливался из побайтовой копии.

| # | Мутация | Ожидание | Результат | Killed |
|---|---|---|---|---|
| M1 | `RELATIONSHIP_FRIEND: u8 = 1` → `= 0` | constants-тест | `6 passed; 1 failed`, rc=101 | ✅ |
| M2 | `MAX_USERNAME_CHARS = 32` → `= 33` | границы `2..=32` и длина тела заявки | `4 passed; 3 failed`, rc=101 | ✅ |
| M3 | убрана проверка `':'` в `validate_friend_request_username` | `rejects_forbidden_characters`, `build_send_friend_request_body` | `5 passed; 2 failed`, rc=101 | ✅ |
| M4 | убрана защита от ведущего нуля (`starts_with('0')`) | `build_relationship_user_route` (`"00"`, `"01"`, `"0194…"`) | `6 passed; 1 failed`, rc=101 | ✅ |
| M5 | `build_block_user_body`: `type` → `RELATIONSHIP_PENDING_OUTGOING` (4) | `build_block_user_body_returns_blocked_relationship_type` | `6 passed; 1 failed`, rc=101 | ✅ |

**Итог: 5/5 мутаций убиты**, выживших нет.

## 3. Восстановление и целостность

| Файл | md5 (после восстановления) | md5 в REPORT.md | Совпадает |
|---|---|---|---|
| `rust/gui-spike/src/relationships.rs` | `17d84aa2bee7b81297f8b313b2a9a953` | `17d84aa2bee7b81297f8b313b2a9a953` | ✅ |
| `rust/gui-spike/src/lib.rs` | `d60aa48c0daf799de67a16388a7d9f04` | `d60aa48c0daf799de67a16388a7d9f04` | ✅ |
| `rust/gui-spike/tests/relationships_contract.rs` | `b8387f1244df8c7c5608c48a11a03ba7` | `b8387f1244df8c7c5608c48a11a03ba7` | ✅ |

После мутаций рабочее дерево вернулось к исходному состоянию: `git status` показывает только `M rust/gui-spike/src/lib.rs` (экспорт модуля, +2 строки) и два новых untracked-файла задачи.

## 4. Замечания

- **P2 (не блокирует):** прогон `cargo` перегенерирует `rust/gui-spike/Cargo.lock` (дрейф из-за `discord-codec`/`zstd`). Я откатил его через `git checkout -- rust/gui-spike/Cargo.lock`, как и описано в §7 отчёта; в PR изменений `Cargo.lock` нет.
- `M package-lock.json` существует в дереве до старта задачи, воркером не трогался.
- Константы и payload (`{"username": trimmed, "discriminator": null}`, `{"type": 2}`, маршрут `/users/@me/relationships/{id}`) согласуются с RE-ораклом (`rest-static.json:19-44`, `DM-FRIENDS-RE.md` §4.2/§4.3/§4.7).
- TDD-цикл подтверждён: `red.log` (`E0432`, unresolved import `relationships`), `green.log` (7/7).

`git commit` не выполнялся.
