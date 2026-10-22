# ТЗ W24b — P8-04: reproducible SBOM/licenses

Волна: RW-23 / B04. Владелец: W24b. Работать в `/home/sanya/orca/project`, не коммитить.

## 0. Обязательное чтение

`AGENTS.md`, `PROCESS.md` §4, `waves/RW-23/WAVE-PLAN.md`, `docs/rust-client/PLAN.md` (P8-04),
`docs/rust-client/TEST-PLAN.md` (T-HARN-07), `scripts/ci.sh`, `scripts/release.sh`,
`rust/deny.toml`, `rust/Cargo.lock`, `rust/crates/discord-conformance/tests/ci_contract.rs`.

Твои файлы — только `scripts/sbom.mjs`, `scripts/ci.sh`, `scripts/release.sh`,
`rust/crates/discord-conformance/tests/sbom_contract.rs`. Новых зависимостей нет.

## 1. Метод

Strict TDD. Сначала тесты, red-лог:

```bash
cd /home/sanya/orca/project/rust
flock /tmp/opencode/discord-cargo.lock cargo test --offline -p discord-conformance --test sbom_contract \
  2>&1 | tee ../waves/RW-23/red-w24b.log
```

Red — compile-red/behavior-red, exit 101. Затем реализация до green, `green-w24b.log`.

## 2. Контракт

- `scripts/sbom.mjs` — Node ESM, без внешних пакетов, offline:
  - `cargo metadata --offline --filter-platform <host> --format-version 1` для лицензий/связей;
  - `Cargo.lock` парсить как TOML-подмножество (`[[package]]`, name/version/source/deps) и
    использовать как авторитетный полный набор пакетов (включая target-specific);
  - для 14 платформенных пакетов без локального манифеста — точный version-pinned fallback;
    иначе `NOASSERTION` и `--check` fail;
  - SPDX 2.3 JSON: `spdxVersion`, `dataLicense`, `SPDXID`, `name`, `documentNamespace`
    (детерминированный: `https://discord-lab.local/spdx/<sha256 Cargo.lock>`), `creationInfo`,
    `packages` (name, versionInfo, SPDXID, licenseDeclared, downloadLocation, filesAnalyzed=false,
    externalRefs purl), `relationships` (DESCRIBES/DEPENDS_ON).
  - `--output PATH` (default `rust/dist/sbom.spdx.json`), `--check` (allowlist), `--check-license ID`
    (self-test: exit 1 для запрещённой), `--dry-run`.
  - Allowlist: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib, MPL-2.0,
    CC0-1.0, Unlicense, 0BSD, CDLA-Permissive-2.0; `LLVM-exception` — только exception в `WITH`;
    `OR` — достаточно одной разрешённой; `AND` — все; невалидные токены/`NOASSERTION` — fail.
  - Никаких `curl`/`wget`/сети.
- `scripts/ci.sh`: после deny и до release вызвать `node scripts/sbom.mjs --check --output
  "$RUST_DIR/dist/sbom.spdx.json"`; dry-run печатает шаг; не ломать `--skip-release`.
- `scripts/release.sh`: если `rust/dist/sbom.spdx.json` есть, включить его в checksum
  (`SHA256SUMS`), иначе не выдумывать; same-file сравнение через `-ef` (эквивалентные пути).
- `tests/sbom_contract.rs`: 4+ теста:
  1. `node scripts/sbom.mjs --output <tmp>` exit 0; JSON SPDX 2.3, packages == Cargo.lock,
     licenseDeclared у всех, DESCRIBES/DEPENDS_ON валидны, две генерации byte-equal,
     documentNamespace содержит 64 hex.
  2. `--check` exit 0 на текущем lock; `--check-license GPL-3.0-only` exit 1.
  3. `ci.sh --dry-run` содержит `sbom`; actual hook `node ... --check` присутствует; `release.sh --dry-run` не ломается, same-file `-ef` есть.
  4. scripts не содержат сетевых клиентов и `cargo metadata` идёт с `--offline`.

## 3. Отчёт

Red/green, файлы, тесты, мутации, риски. Не запускай workspace.
