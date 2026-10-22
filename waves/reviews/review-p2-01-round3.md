# Третье независимое ревью P2-01 (round 3)

**Ревьюер:** независимый субагент (не автор исправлений).  
**Дата:** 2026-10-20.  
**Режим:** только чтение исходников, тестов, DESIGN/TZ, логов и md5; `cargo` писал только в
`target/`. Независимый пробник — `/tmp/opencode/p2-01-r2-probe` (вне репозитория). В репозитории
записан только этот отчёт. Живой Discord, attach/walk и сеть не использовались.

**Снимок:** рабочее дерево dirty (RW-10/RW-11 + RW-12), коммитов нет. Текущие md5 совпадают с
`p2-01-review-fixes.md`:

| Файл | md5 |
|---|---|
| `src/ready/value.rs` | `090bf9441acd8459d310ee93d2242dd6` |
| `src/ready/snapshot.rs` | `d88df7bc6ace6c44ca3fd81dae8b3c59` |
| `src/ready/social.rs` | `33386b7304367ff98d06e37c8d1f4e64` |
| `src/ready/guild.rs` | `2bfbea7ae9cfee092696768339f28c88` |
| `tests/ready_review.rs` | `7ab589278a949a5c3e80c8cfeeb5eea4` |
| `tests/ready_core.rs` | `04a5aa28d971041c45a2fd85ee422d5f` |
| `reports/p2-01-review-test.red.rs` | `7ab589278a949a5c3e80c8cfeeb5eea4` |

## 1. Итоговый вердикт

**Принято.** P0 = 0, P1 = 0, P2 = 0.

P2-4, P2-5, P3-4 и P3-5 закрыты; P1-1, P2-1…P2-3 и P3-1/P3-2 остаются закрытыми. P3-3
(`value` public) — корректный backlog, не блокирует. Ниже перечислены только P3-остатки,
которые не влияют на данные, безопасность или контракт READY.

## 2. Проверка P2-4 (разделение `from_dispatch`/`Ready::parse`)

Контракт синхронизирован:

- **DESIGN §2.3** (`DESIGN-P2-01.md:142`): не-map `d` — `from_dispatch` → `NotReady`,
  `Ready::parse` → `WrongType`; отсутствие `d` → `Missing`.
- **DESIGN §3.5** (`:600-601`): `Ready::parse`: `d.as_map()`, иначе `WrongType`;
  `from_dispatch`: отсутствие `d` → `Missing`, не-map → `NotReady`.
- **TZ §2** (`TZ-P2-01.md:34-35`): то же разделение.
- **Код:** `from_dispatch` проверяет `d` и возвращает `NotReady` (`snapshot.rs:159-166`);
  `Ready::parse` возвращает `WrongType` (`snapshot.rs:173-178`).
- **Тест:** `r2_non_map_d_is_not_ready` проверяет **оба** входа (`ready_review.rs:107-118`);
  отдельный пробник: `from_dispatch(d=5) → Err(NotReady)`, `Ready::parse(&Int(5)) →
  Err(WrongType { path:"d", expected:"map", found:"int" })`.

**P2-4 закрыт.**

## 3. Проверка P2-5 (сигнатуры)

- **DESIGN §3.4** (`:561-568`): `ParsedGuilds { guilds, raw_positions }`,
  `parse_guilds(...) -> ParsedGuilds`, `parse_user(value, path, diags)`.
- **DESIGN §3.5-псевдокод** (`:757-758`): `parse_guilds` с новым возвратом и `parse_user` с
  `"d.user"`/`diags`; **TZ P-2** (`:24`) явно отмечает синхронизацию приватных сигнатур.
- **Код:** `guild.rs:177-186` — `ParsedGuilds` и `parse_guilds(...)->ParsedGuilds`;
  `social.rs:250-254` — `parse_user(value, path, diagnostics)`.
- Публичные сигнатуры `Ready::parse`/`from_dispatch` не менялись.

**P2-5 закрыт.** Остаётся только P3-замечание к схематичности псевдокода (см. §7).

## 4. Проверка P3-4 (дубликат гильдии)

- **DESIGN §2.3** (`:152`) и **§3.5** (`:617`) фиксируют: побеждает группа последнего сырого
  вхождения, группы предыдущих — `malformed`.
- **Код:** дедупликация очищает предыдущие `raw_positions` (`guild.rs:199-207`), привязка
  `merged_members` идёт по сырому индексу, для `None` при непустой группе добавляется
  `malformed` (`snapshot.rs:262-268`).
- **Тест `r7_duplicate_guild_keeps_last_member_group`** (`ready_review.rs:200-242`) проверяет
  last wins по `name`, members `[200]`, `duplicates=["guilds:1"]` и
  `malformed=["merged_members[0]"]`. Независимый пробник даёт тот же результат.

**P3-4 закрыт** для зафиксированного правила. Остаётся P3-нюанс с пустыми группами (см. §7).

## 5. Проверка P3-5 (md5 и exit-коды)

- `p2-01-review-red.md5` теперь содержит **текущие** md5 `ready_review.rs` (`7ab...`) и
  `ready_core.rs` (`04a5...`); `p2-01-review-fix.md5` содержит те же md5 и md5 red-копии
  (`7ab...`). Расхождения со старым `e091...` больше нет.
- `diff -u` red-копии и текущего `ready_review.rs` — пусто; `ready_core` — тоже.
- `p2-01-review-red-final.log`: `0 passed; 7 failed`, **`# EXIT=101`**, номера строк
  `91/107/139/171/189/225/263` совпадают с текущими тестами.
- `p2-01-review-green-final.log`: `7 passed; 0 failed`, **`# EXIT=0`**.
- `p2-01-review-core-green.log`: 242 passed, `# CORE_EXIT=0`;
  `p2-01-review-workspace-green-final.log`: 284 passed, `# WORKSPACE_EXIT=0`;
  `p2-01-review-clippy.log`: `# CLIPPY_EXIT=0`.
- `cargo fmt --all -- --check` независимо — exit 0 (отдельного fmt-лога в отчёте нет, но
  проверка воспроизведена).

**P3-5 закрыт.**

## 6. Red/green и полные прогоны

| Проверка | Результат |
|---|---|
| red-копия = текущий `ready_review.rs` | `7ab589278a949a5c3e80c8cfeeb5eea4`, diff пуст |
| red-final | `0 passed; 7 failed`, `# EXIT=101` |
| green-final | `7 passed; 0 failed`, `# EXIT=0` |
| `cargo test --offline -p discord-core` | **242 passed, 0 failed**, exit 0 (25 suites) |
| `cargo test --offline --workspace` | **284 passed, 0 failed**, exit 0 (45 suites) |
| `cargo fmt --all -- --check` | exit 0, без вывода |
| `cargo clippy --offline --workspace --all-targets -- -D warnings` | exit 0, без предупреждений |

Red-final — mutation-red с временным откатом продуктовых правок: old-code warnings
(`raw_guild_positions`, `path`) ожидаемы; тестовая копия при этом совпадает с финальной.
Первый red на новой спеке также был `0/6`; после добавления `r7` финальный red — `0/7`.

## 7. P3 — backlog, не блокирует

1. **P3-3:** `value` остаётся публичным модулем (`ready.rs:17`), интеграционные тесты
   используют его как оракул `EtfValue`. Публичная поверхность избыточна, но данные/безопасность
   не затронуты; отложение корректно.
2. **§3.5-псевдокод схематичен:** `DESIGN-P2-01.md:757` присваивает результат `parse_guilds`
   переменной `guilds`, не показывая `.guilds`/`.raw_positions`; нормативные §3.4 и TZ P-2
   корректны, поэтому это P3-нит, а не P2-5.
3. **Пустые группы `merged_members`:** `snapshot.rs:265` добавляет `malformed` только для
   непустой группы; §2.3/§3.5 говорят «группы предыдущих/лишние — malformed» без оговорки о
   пустоте. `r7` покрывает непустую предыдущую группу; пустая группа данных не несёт, поэтому
   это P3-нюанс диагностики. Если хочется полной буквальности — добавить тест/уточнить «непустые
   группы».
4. **`ReadyError::NotReady` doc** (`error.rs:12`) всё ещё говорит «или нет `d`», хотя отсутствие
   `d` в `from_dispatch` даёт `Missing`; это P3-нит документации, поведение корректно.

## 8. Границы и ограничения

- Проверены DESIGN §2.3, §3.4–§3.5, TZ §2/P-2, `ready_review.rs` (r1–r7), `ready_core.rs`,
  якоря, red/green/core/workspace/clippy-логи, md5-файлы и fixture-оракул.
- Новых P0/P1/P2 не найдено; исходные P1/P2 и P3-1/P3-2 не регрессировали.
- Код, тесты и доки не менялись; записан только этот отчёт. `cargo` работал offline, писал
  только в `target/`. Golden/`logs/`/`functions/`/`evidence/` не изменялись.
- Не проверялись: живой Discord/attach/walk, gateway-run, P2-02-интеграция, reducer/SQLite,
  REST/UI/protobuf, presence/voice — вне P2-01. Реальный захват в этом раунде заново не
  декодировался; atom-формы подтверждены r1/пробником и `docs/re/gateway-schema.md:60-77`.

## 9. Короткий итог

**P2-01 принят.** P2-4/P2-5/P3-4/P3-5 закрыты; P1-1 и исходные P2/P3-1/P3-2 остаются
закрытыми. Red-final — 7/7 failed, green — 7/7, core — 242, workspace — 284; fmt/clippy чистые.
P3-3 и перечисленные P3-ниты можно оставить в backlog.
