# RW-09 final — снимок принятых файлов (2026-10-20)

Копии файлов на момент приёмки (источник — рабочее дерево репозитория после закрытия нитов N1–N6;
рефакторинг W9d включён). Сверка: `md5sum` копий совпадают с файлами в `rust/crates/discord-core/`.

## src

| Файл | md5 |
|---|---|
| `src/rest/base64.rs` (fb49dae8) | `fb49dae8d59f03b2124b83e1c73aa2f4` |
| `src/rest/super_properties.rs` | `6ef2ac9d5fe452fb3eb583254212719c` |
| `src/rest/identity.rs` | `25d5005219ee7e3cc26ded118f017e0d` |
| `src/rest/mod.rs` | `0eba823bf46c2958cac102dbb42cbb06` |

## tests

| Файл | md5 |
|---|---|
| `tests/rest_base64.rs` | `60c50ec340537f0f88e81bf5359c1f6f` |
| `tests/rest_super_properties.rs` | `34dc62487dfb99553d7f99449fb67911` |
| `tests/rest_identity.rs` | `9946bc26c7014a9778f8ae8d91a12c15` |

## tests/data (выдержки-оракулы, read-only)

| Файл | md5 |
|---|---|
| `tests/data/rest-base64-vectors.tsv` | `8b5bb8c97841773bd13e0fb9f29e8b51` |
| `tests/data/rest-super-properties-22.tsv` | `95e2f8616043041431fcef2d38d6fac5` |
| `tests/data/rest-super-properties-2323.b64` | `b9e8f8430d8b496df50da9419e760a2d` |
| `tests/data/rest-super-properties-2323.json` | `90b06ded88a98dd05d3b821f6bedc52b` |

Провенанс выдержек: `reference-oracle/d02-snapshot/capture-run2.snap.jsonl`
(md5 `abfe14b76cec1e559c3b45522bc44b7a`), запрос `95974.2323`; команды снятия — ТЗ §3.1.

Сопутствующее: ТЗ `waves/RW-09/TZ.md` `5dc281581e7c0ee88be77ac39e373ce3`; ревью
`waves/reviews/review-rw-09.md` `22cdf1fcbc9fd32659216206368d78ef`; отчёт `waves/RW-09/report.md`.
