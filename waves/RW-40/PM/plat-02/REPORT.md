# PLAT-02 — отчёт воркера G4 QA

- batch/карточка: RW-40 / PLAT-02 (`waves/RW-40/QUALITY/PLATFORM-AUDIT.md` §3, P1), PM — DeepSeek.
- дата: 2026-10-24. HEAD: `0239543297decf0eb6762160c837754e0e11548b`.
- план: `waves/RW-40/QUALITY/PLAT-02-PLAN.md` (D-110).
- режим: **без live**, без сети, без окна, без commit. Live-приёмка — отдельным окном (§5).
- логи: `waves/RW-40/QUALITY/plat-02/`.

## 1. Что сделано

| файл | изменение | md5 |
|---|---|---|
| `rust/crates/discord-platform/tests/keyring_contract.rs` | 12 × `#[ignore = "live-keyring: …"]`; `fn skip()` и все `return`-гейты удалены; `live_guard(needs_secret_tool)` паникует с `KEYRING-LIVE: not-enabled\|tool-missing\|service-unavailable`; `live_keyring()` вместо `keyring(Option)` | `1d9f1b2659f827ced080197e77bc078c` |
| `rust/crates/discord-platform/tests/keyring_offline_contract.rs` | **new**: `kr01` redaction ошибок, `kr02` `SCHEMA_NAME`, `kr03` форма `Secret{code,message}` без секрета/указателей | `e4edb69403f3d188cd585f8e66016b0e` |
| `tests/meta/keyring-live.test.mjs` | **new**: T1–T3 (13 подтестов), red-first | `1f3cec9bf9695a9073164603e8d04cc5` |
| `scripts/gate.mjs` | фаза `keyringLivePhase` (`--ignored`, `env: keyringLiveEnv()` → LIVE=1, `failOnSkips: true`, `timeoutMs: 900000`, preflight `command -v secret-tool`/`--version`), только release/qualify; `runCommand` получил `env`; `executePhase` гасит `ok` при `failOnSkips && skipped > 0`; `cargoTestCounts`/пороги не тронуты | `c8a4875ed7d7a64d9dad3a215cad9a8f` |
| `TDD-PLAN.md` | §8: одна строка для нового мета-теста (требование `tests/meta/plan-sync.test.mjs`) | `2704117fd363fdaa42ff15a0f4f30ab4` |

Прод не тронут: `keyring.rs`, `ffi_secret.rs`, `src/lib.rs` — API заморожен. `tests/meta/test-policy.test.mjs` и чужие тесты не изменялись.

## 2. Red-first

`red-t1-t3.log` — до правок: **13 fail / 0 pass** по правильным причинам:

- T1: `skip(` присутствует, у 12 `#[test]` нет `#[ignore]`, `k01..k08` завершаются `return`;
- T2: фазы `keyring-live` нет, `failOnSkips`/`preflight` нет;
- T3: cargo даёт `12 passed; 0 failed; 0 ignored`, `--list --ignored` → 0 строк.

Green: `green-t1-t3.log` → `tests 13 / pass 13 / fail 0`.

## 3. Targeted

| проверка | результат |
|---|---|
| `cargo test --offline -p discord-platform --test keyring_contract` | `ok. 0 passed; 0 failed; 12 ignored` |
| `... -- --list --ignored` | ровно 12: `k01_connect_to_default_collection` … `k12_debug_does_not_print_secret_or_pointers` |
| `cargo test --offline -p discord-platform --test keyring_offline_contract` | `3 passed; 0 failed; 0 ignored` (kr01–kr03) |
| `cargo test --offline --workspace` | `1103 passed; 0 failed; 12 ignored` (было 1112 passed / 0 ignored: −12 live +3 offline) |
| `cargo test --offline -p discord-core` | 574 passed |
| gui-spike (`--manifest-path rust/gui-spike/Cargo.toml`) | 238 passed |
| frontend UI (8 файлов ST-01) | 309 passed |
| `npm run test:unit` | 539 passed |
| `cargo fmt --check`, `cargo clippy --offline --workspace --all-targets` | чисто |

Негативные проверки live-набора (без сервиса, без live-окна):

| состояние | лог | факт |
|---|---|---|
| `not-enabled` | `negative-not-enabled.log` | 12/12 fail, `KEYRING-LIVE: not-enabled: DISCORD_LAB_KEYRING_LIVE != 1 (ожидалось 1)` |
| `service-unavailable` | `gate-release.log` (шина `unix:path=/tmp/opencode/no-such-bus`) | 12/12 fail, `KEYRING-LIVE: service-unavailable: Keyring::connect: Secret Service (код 1): Could not connect` |
| `tool-missing` | `negative-tool-missing.log` | k07 fail, `KEYRING-LIVE: tool-missing: secret-tool не найден в PATH` |

Мутации (проверка чувствительности мета-теста, файлы восстановлены, md5 совпадают):

| мутация | реакция |
|---|---|
| `return;` в начало k02 | T1 red: «в телах live-кейсов нет раннего return» |
| `LEAK_ITERATIONS: 2000 → 100` | T1 red: «пороги security-кейсов не ослаблены» |
| убрать `-- --ignored` и `failOnSkips` из фазы | T2 red: «фаза keyring-live объявлена…» |

## 4. Гейт

**worker** (`gate-worker.log`): `workspace-rust ✔ (тесты 1115: pass 1103, fail 0, skip 12)` +
`⚠ workspace-rust: пропущено 12 тестов — пропуск прохождением не считается`; фазы
`keyring-live` в worker-наборе **нет** (0 упоминаний в логе). Вердикт прогона — `failed`
**не из-за PLAT-02**: красные `typecheck`/`meta`/`unit` дают чужие in-flight файлы
(см. §6). Целевая формула вердикта при зелёных прочих фазах — `passed_with_skips`
(T2 проверяет `cargoTestCounts(ignored→skipped)` и сохранение `passed_with_skips`; тот же
вердикт зафиксирован артефактом `.gate/release-2026-10-24T08-22-41-876Z.json`).

**release** (`gate-release.log`, шина заведомо недоступна ⇒ live не выполнялся):
`✖ keyring-live — (тесты 12: pass 0, fail 12)` с `KEYRING-LIVE: service-unavailable`,
note `env: DISCORD_LAB_KEYRING_LIVE=1 (выставлен фазой); secret-tool: /usr/bin/secret-tool`.
То есть release без live-окна **не зелёный** — как требует план (§4.3 риск 1, DoD 4).
Полный `gate:release` — `failed` (плюс чужие red-фазы).

## 5. Остаток

1. **Live-окно (не выполнялось, запрещено ТЗ):** `gate:release` с поднятым
   `org.freedesktop.secrets`, разблокированной коллекцией и `secret-tool` → ожидается
   `keyring-live ✔ 12/12, ignored=0`, `failOnSkips` не срабатывает. В том же окне — повтор
   негативов `not-enabled`/`service-unavailable` уже с реальной шиной.
2. **Чужие in-flight файлы** (untracked, правятся параллельной линией, не мои):
   `tests/unit/channel-switch.test.mjs:331` (TS2339), `tests/unit/gui-spike-features.test.mjs:667`
   (TS7006) — из-за них `npm run typecheck` exit 1; три файла (`channel-switch`, `guild-rail`,
   `voice-ui`) не перечислены в §8 — из-за них `tests/meta/plan-sync.test.mjs` red
   (проверено: без моего файла он тоже red). Мои файлы `tsc` проходят чисто.
3. **kr03 и `From<FfiError>`:** `FfiError` — `pub(crate)` (`ffi_secret` приватный, API
   заморожен), поэтому конверсия снаружи недостижима. kr03 проверяет публичную форму
   `KeyringError::Secret { code, message }` (то, что производит `From`), код/сообщение
   сохраняются, секрет/`0x` не печатаются, `source().is_none()`. Чтобы проверять сам
   `From<FfiError>`, нужна отдельная карточка на `#[cfg(test)]`-доступ — вне PLAT-02.
4. `TDD-PLAN.md` §8: строка добавлена (`tests/meta/keyring-live.test.mjs`), т.к. этого
   требует `plan-sync`; если TL ведёт файл сам — достаточно оставить/переписать строку.
5. Журнал волн (`waves/db.jsonl`, `STATE.md`, `BUGS.md`, `TASKS.md`) — за Team Lead.
