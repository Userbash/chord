# BATCH <RW-NN> — <название>

Дата: <YYYY-MM-DD>. Team Lead: <TL-1>. Tech Lead: <TL-2>.
Статус: draft / frozen / in-progress / gate / review / accepted.

## 1. Состав batch

| Карточка | Seat | Файлы (только эти) | Зависит от | Оракул |
|---|---|---|---|---|
| P2-xx | CORE-1 | `rust/.../src/state/xxx.rs`, `tests/xxx_types.rs` | P2-02 `[x]` | `<файл:строка/команда>` |
| P2-yy | CORE-2 | `...` | ... | ... |

Общие файлы (только TL-2): `state/types.rs`, `state/reducer.rs`, `state/mod.rs`, `src/lib.rs`.
Запрещено: чужие тесты/модули, доки, golden, `Cargo.toml`, `package.json`, сеть/live.

## 2. Замороженный контракт

- Публичные типы/сигнатуры: <...>
- Ключи состояния/инварианты: <...>
- Диагностика: <malformed/ignored/inconsistencies/duplicates/stale, точные пути>
- Границы scope: <что не делаем>
- Неизвестное: <Effect/stub, без выдуманного wire>

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание | Оракул |
|---|---|---|---|
| r01 | ... | ... | ... |

Бюджет: <S/M/L/XL> → <6–10/10–16/16–24/24–32> тестов; мутации <3–5/5–8/8–12>.

## 4. Команды

```sh
# worker targeted
cargo test --offline -p discord-core --test <test>
# batch gate (integration/QA, один раз)
cargo fmt --all -- --check
cargo clippy --offline --workspace --all-targets -- -D warnings
cargo test --offline --workspace
npm run gate:worker
```

## 5. Definition of done

- [ ] red-лог с exit code; green targeted;
- [ ] файлы + md5; `TEST-CORRECTIONS.md` (если был);
- [ ] mutation caught, restore по md5;
- [ ] batch gate passed;
- [ ] независимое ревью, P0/P1/P2=0;
- [ ] `accepted` + `db.jsonl` + `STATE.md` + STATUS/ROADMAP/TASKS/map один раз.
