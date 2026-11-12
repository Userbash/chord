# B02 / RW-21 — матрица тестов и оракулов

Дата: 2026-10-21. Batch: **B02**. Карточки: P2-08, P1-01, P1-02, P0-04, P0-07.
Все тесты офлайн; lab-файлы read-only; при отсутствии lab — skip с причиной.

| Карточка | Тесты | Оракул | Red | Green |
|---|---|---:|---|---|
| P2-08 settings-proto | `settings_proto` 18 | `waves/RW-21/oracle.py` + READY/UPDATE из `reference-oracle/`; base64/raw sha256 в REPORT | `logs/p08-red-compile.log` | `logs/p08-targeted-final3.log` 18/18 |
| P1-01 ETF full | `etf_full` 6 + принятые `etf_vectors` 13, `etf_properties` 9 | `docs/re/gateway-protocol.md` §2; реальные термы capture 95974.1389; канон RW-01 | `logs/etf-full-red.log` | `logs/b02-codec-targeted.log` 43/43 |
| P1-02 zstd strict | `zstd_stream` 13 | lab `gw-proto/zstd-frames-95974_1389.hexl`; 759 280 Б = 824 816 − 65 536; synth-frame | `logs/zstd-strict-red.log` | `logs/b02-codec-targeted.log` 43/43 |
| P0-04 conformance | `fixtures` 6 | `fixtures/MANIFEST.json` + `reference-oracle/`; `RE_FIXTURES_ROOT`/`RE_LAB_ROOT` | `logs/conformance-red.log` | `logs/b02-conformance-require-green2.log` 6/6 |
| P0-07 legal | docs-only (без runtime) | `README.md` §Правовая рамка, `CONCEPT.md` §1/§7 | — | `LEGAL.md` + README/rust-README |

## Проверки уровня batch

| Проверка | Результат | Лог |
|---|---|---|
| `cargo test -p discord-core` | **524 passed, 0 failed** | `logs/b02-core-final.log` |
| `cargo test --workspace` | **580 passed, 0 failed** | `logs/b02-workspace-final2.log` |
| `cargo fmt --all -- --check` | exit 0 | `logs/b02-fmt-final.log` |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | `logs/b02-clippy-final.log` |
| `npm run gate:worker` | **passed**: typecheck, meta 25/25, unit 169/169, smoke 20/20 | `gate-worker-b02.json`, `gate-worker-b02.log` |
| mutations B02 | **11/11 caught** (`P101-M1…M6`, `P102-M1…M5`) | `mutations/summary-b02.log` |
| mutations P2-08 | **13/13 caught** (`M1…M13`) | `mutations/M*.log`, `summary-p208-extra.log` |
| P2 review | round 1 P2→fixed; round 2 accepted with P3; P3 closed | `reviews/review-p2-08-round2.md` |
| P1/P0 reviews | P1-01/P1-02: accepted with P3 (doc fixed); P0-04/P0-07: round 1 P2 addressed; round 2 accepted with P3, P3 closed | `reviews/review-codec-codex.md`, `reviews/review-harness-legal-round2.md` |

## Границы

- P1-02 не «дочитывает» последнее окно незавершённого zstd-кадра: это ограничение
  streaming API `ruzstd`, а не ошибка; контракт и число зафиксированы тестом.
- P0-04 — скелет загрузчика (T-HARN-01); полный побайтовый replay T-HARN-02/03 требует
  JSON-компаратора и отдельной карточки, чтобы не добавлять зависимость в скелет.
- P0-04 lab-test: обычный прогон soft-skip; acceptance — `RE_REQUIRE_LAB=1` (red при
  отсутствии lab, green 6/6 при наличии).
- P0-07 — правовая рамка и чек-лист; юридическую оценку даёт владелец.
- P2-08 type 2 остаётся structural: favorite/frecency semantics не выдумана.
