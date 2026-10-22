# Независимое ревью RW-16 / P2-05 — чанки и догрузка

Дата: 2026-10-20. Ревьюер: отдельный read-only агент
(`ses_ea69ef126ffe1uhFnPcOM0Xrer`), автор кода в ревью не участвовал.
Контракт: `waves/RW-16/DESIGN-P2-05.md`, `TZ-P2-05.md`; research:
`RESEARCH-P2-05.md`. Сеть/live Discord не использовались.

## Итог

**Принято. P0 = 0, P1 = 0, P2 = 0.** P3 — только трассируемость/документация.

## Раунд 1 — не принято (P2-1)

P2-1: `duplicate-index` подавлялся для **любого** `nonce=None`, включая `chunk_count > 1`:

- `member_chunk.rs` писал диагностику только при `chunk.nonce.is_some()`;
- DESIGN §4.4 разрешал исключение только для `nonce=None/count=1` (capture s=78/s=84);
- тесты `r04` (`Some/count1`) и `r12` (`None/count1`) не покрывали `None/count2`;
- независимый probe: `none/count1 → []`, `none/count2 → []` (ожидался duplicate),
  `some/count2 → duplicate`.

P2-1 закрыт red-first:

- `red-w16-r13.log`, exit **101**, падение
  `nonce=None/count>1: повтор индекса обязан быть диагностирован: []`;
- условие: `duplicate_index && (chunk.nonce.is_some() || chunk.chunk_count > 1)`;
- `r13` — regression на `None/count2`; `r12` сохраняет исключение `None/count1`;
- DESIGN §4.4/§6.4 и TZ W16b синхронизированы.

## Раунд 2 — принято

Проверено:

| Пункт | Подтверждение |
|---|---|
| Ключ `(guild_id, Option<String>)` | `state/types.rs`, `member_chunk.rs` |
| `chunk_count` first-wins, duplicate last-wins | `member_chunk.rs`; `r05`, `r04`/`r13` |
| `nonce=None/count=1` — новый ответ, без duplicate | `r12` |
| `nonce=None/count>1` и `nonce=Some` — duplicate | `r13`, `r04` |
| Out-of-order, `is_complete` | `r03`, `s03` |
| Unknown guild, сборка сохраняется | `r02` |
| Merge `Guild.members`/`State.users` | `r01`, `r10` |
| `not_found` не удаляет, counts/list/messages не меняются | `r01`, `r07` |
| Stale no-op | `r08` |
| `presences[]` не изобретён, ignored | `c13` |
| Legacy `009` | `c02`, `c03` |
| Presence effect — декларация, без сети | `member_chunk_selectors.rs`; `s07`–`s09` |
| Нет новых зависимостей/сети/SQLite/UI | проверено |

## Прогоны ревьюера

| Команда | Результат |
|---|---|
| targeted `member_chunk_*` | **43/43**, exit 0 (types 19, reducer 13, selectors 11) |
| `cargo test --offline -p discord-core` | **378 passed / 0 failed**, exit 0 |
| `cargo test --offline --workspace` | **420 passed / 0 failed**, exit 0 |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --offline --workspace --all-targets -- -D warnings` | exit 0 |
| `tests-final/MD5SUMS.txt` / `SOURCE-MD5SUMS.txt` | архивы совпадают с live |
| gate `.gate/worker-2026-10-20T13-21-59-680Z.json` | `verdict=passed`, все фазы `ok`, `warnings=[]`; ревьюер проверял более ранний `13-09-21-812Z` с тем же verdict |
| `waves/RW-16/gate-w16.log` | `GATE_EXIT=0` |

## P3 (не блокируют)

1. `REPORT.md` до правки говорил, что gate «ожидает»; закрыто — указан passed и путь.
2. `red-w16-r13.log` ссылается на строку 392; текущий assertion — 394–402 из-за двух
   last-wins assert-ов, добавленных после red. Причина падения та же; сдвиг отмечен.
3. Нет отдельного regression на `nonce=Some/count>1` duplicate; ветки покрыты `r04` и
   `r13`, probe ревьюера для `some/count2` проходит.
4. `review-p2-05.md` создан этим файлом.
