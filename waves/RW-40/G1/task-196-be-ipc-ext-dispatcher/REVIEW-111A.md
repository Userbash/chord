# REVIEW-111A — task-196 (`ipc_ext`), Technical Review

**Вердикт: APPROVED** — P0 = 0, P1 = 0, мутации 5/5 убиты.

## Проверено
- `rust/gui-spike/src/ipc_ext.rs` (193 строки) сверен с `REPORT.md` §2: 5 вариантов `ExtendedIpcCommand`
  (`serde(tag="cmd", content="args", snake_case)`), `ExtendedIpcPlan { method, path, body }`, лимиты
  `100 / 10 / 9975 / 25`, `validate_snowflake` = непустые ASCII-цифры + `u64 > 0`.
- Targeted-прогон (один раз): `cargo test -p discord-gui-spike --test ipc_ext_contract` →
  `12 passed; 0 failed`, exit 0.
- Мутации по одной, файл восстановлен после каждой:

| # | Мутация | Итог |
|---|---|---|
| m1 | `MAX_SEARCH_OFFSET 9975 -> 9976` | KILLED (11 passed / 1 failed) |
| m2 | `MAX_SEARCH_LIMIT 25 -> 26` | KILLED (11 / 1) |
| m3 | `MAX_POLL_ANSWERS 10 -> 11` | KILLED (11 / 1) |
| m4 | `MAX_BULK_READ_STATES 100 -> 101` | KILLED (11 / 1) |
| m5 | `Ok(n) if n > 0` -> `n >= 0` (приём `"0"`) | KILLED (7 / 5) |

- Восстановление: `src/ipc_ext.rs` побайтово равен бэкапу, md5 `9938b029541a73a27882c5859226021b` —
  совпадает с `REPORT.md` §6.

## Замечания (не блокируют, P2)
1. `emoji` и `content` не URL-кодируются при сборке path/query. Это соответствует RE-дампу для штатных
   значений, но спецсимволы (`?`, `&`, `%`, пробел) дадут битый маршрут — кандидат в отдельную карточку.
2. `rust/gui-spike/Cargo.lock` снова дрейфует после `cargo` (zstd/discord-codec, отмечено в `REPORT.md` §7);
   вне write-scope этого ревью, откат — за интегратором перед merge.

`git commit` не выполнялся. Изменён только этот файл.
