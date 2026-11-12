# Отчёт W9c — RW-09 / P1-08 (T-REST-04): честная идентичность и сборка

## Сделано

- `rust/crates/discord-core/tests/rest_identity.rs` — 8 кейсов I1–I8 из ТЗ §6 на детерминированных
  `FakeSources` (env/файлы — литералы §6.1; `random_bytes` — `0x00..0x0F`, `0x10..0x1F`,
  `0x20..0x2F`):
  - I1 `wayland_kde_full_build` — все 22 поля точной структурой, три UUID, ровно 3 вызова
    `random_bytes`; `client_version` сверен с `env!("CARGO_PKG_VERSION")` = `0.1.0`;
  - I2 `display_server_fallbacks` — `XDG_SESSION_TYPE` (дословно, в т.ч. `tty`) → `WAYLAND_DISPLAY`
    → `DISPLAY` → `unknown`;
  - I3 `locale_parsing` — `LC_ALL` → `LANG`, обрезка `.`/`@`, затем `_`, `C` сохраняется;
  - I4 `os_arch_mapping` — linux/macos/windows/иное, x86_64/aarch64/x86/иное; `app_arch = os_arch`;
  - I5 `distro_sources` — `PRETTY_NAME` дословно с кавычками, без файла/ключа → `unknown`,
    значение без кавычек — как есть;
  - I6 `uuid_v4_shape` — точные строки, формат `8-4-4-4-12`, только `0-9a-f`, маскирование
    версии/варианта на всех `0xFF` → `ffffffff-ffff-4fff-bfff-ffffffffffff`, разные байты → разные
    UUID;
  - I7 `config_overrides_win` — конфиг со всеми 22 `Some(...)` (включая `u32` 641004/94186 и
    `client_event_source = Some("probe")`) побеждает env/файлы/константы;
  - I8 `integration_base64_headers` — `build` → `to_base64` → `from_base64` без потерь, канонический
    base64 стабилен; `AuthHeaderValues` даёт 6 пар, значение `X-Super-Properties` непустое.
- `rust/crates/discord-core/src/rest/identity.rs` — реализованы тела `uuid_v4` и `build` плюс
  приватные помощники (`map_os`, `map_arch`, `parse_locale`, `parse_pretty_name`,
  `resolve_os_version`, `resolve_distro`, `resolve_display_server`, `first_env`, `non_empty`).
  Трейт/структуры/публичные сигнатуры не менялись (в diff только имена параметров `_sources` →
  `sources` и `_config` → `config`, без изменения типов/арности); `SystemSources` не тронут.
  UUID v4: `(b[6] & 0x0f) | 0x40`, `(b[8] & 0x3f) | 0x80`, hex в нижнем регистре.
  Порядок генерации UUID фиксирован: `client_launch_id`, `launch_signature`,
  `client_heartbeat_session_id`.
- Границы: `Cargo.toml`/`Cargo.lock` не менялись (`git diff` пуст); чужие файлы, `docs/`, `waves/`,
  golden и `reference-oracle/` не трогались.

## Red

Команда:

```bash
cd /home/user/orca/project/rust
bash -c 'cargo test --offline -p discord-core --test rest_identity 2>&1 \
  | tee /tmp/opencode/rw-09/w9c/red.log; \
  echo "# EXIT=${PIPESTATUS[0]}" | tee -a /tmp/opencode/rw-09/w9c/red.log'
md5sum crates/discord-core/tests/rest_identity.rs | tee -a /tmp/opencode/rw-09/w9c/red.log
cp crates/discord-core/tests/rest_identity.rs /tmp/opencode/rw-09/w9c/rest_identity-red.rs
```

Результат до реализации: `# EXIT=101`, `test result: FAILED. 0 passed; 8 failed; 0 ignored`.
Падения — ассерты на заглушке (`build` → `SuperProperties::default()`, `uuid_v4` → `""`),
без `todo!()`/`panic!()`.

Честная хронология red (тест правился только по форматированию/багу проверки, реализация — нет):

1. первый red: md5 `ee2fffd6786363aa6b3fc930da20b169` — лог сохранён `red-first.log`;
2. после исправления проверки hex (символы `-` в UUID) и `rustfmt` — повторный red:
   md5 `93507dfaa6f95a034c9e1c490e508d58` — лог `red-unformatted.log`;
3. финальный red после `rustfmt` (лог `red.log`, копия `rest_identity-red.rs`):
   md5 теста `e6c721047d944b0d270565513d90df49`, `# EXIT=101`, 0/8 passed.

## Green

- Свой таргет ×5 (после финального состояния W9a/W9b): `# RUNS=5 FAILS=0`, каждый прогон
  `8 passed; 0 failed` (`/tmp/opencode/rw-09/w9c/green.log`).
- `cargo test --offline -p discord-core`: 116 passed, 0 failed, 0 ignored, 13 test-бинарей +
  doc-tests, `EXIT=0` (`crate-tests.log`).
- `cargo clippy --offline -p discord-core --all-targets -- -D warnings`: чисто, `EXIT=0`
  (`clippy.log`).
- `rustfmt --check` двух своих файлов: чисто, `EXIT=0`.
- `npm run gate:worker`: **passed**, отчёт `.gate/worker-2026-10-19T20-32-35-801Z.json`
  (typecheck ✔, meta 25/25, unit 169/169, smoke 20/20, skip 0).

## Файлы

| Файл | md5 |
|---|---|
| `rust/crates/discord-core/src/rest/identity.rs` | `4a2554d757056dd7d4c6a08d352bfcc8` |
| `rust/crates/discord-core/tests/rest_identity.rs` | `e6c721047d944b0d270565513d90df49` |

Артефакты: `/tmp/opencode/rw-09/w9c/{red.log,rest_identity-red.rs,green.log,crate-tests.log,clippy.log,gate-worker.log,identity.diff,identity-stub.rs,report.md}`.

## Риски/вопросы

- **Зависимость I8 от W9a/W9b.** На момент финального зелёного: `super_properties.rs` md5
  `debbb208b8bb4d10f2e2c8d8fd0fc2d8`, `base64.rs` md5 `f240f0695c512ae7d06159ec1ebb2cec`.
  W9b менял файл после первого зелёного прогона — все 5 финальных прогонов и полный `discord-core`
  сделаны уже на этой версии. Если W9b/W9a изменятся после отчёта, I8 нужно перепрогнать.
- **Пустые значения.** Для источников «пустая env/файл» трактуются как отсутствие (fallback/
  `unknown`); конфиг `Some("")` возвращается дословно (правило «`config.Some(v)` → `v`»). В ТЗ этот
  крайний случай не оговорён — если нужна другая семантика, скажите.
- **`SystemSources::random_bytes`** остаётся best-effort (при сбое `/dev/urandom` буфер не меняется,
  это задокументировано в трейте и коде); `uuid_v4` при этом вернёт UUID корректного формата из
  нулевого буфера — для тестов не используется (все тесты на `FakeSources`).
- **§8.** Новый тест-файл создан, но строку §8/TASKS в `docs/` воркеру ТЗ запрещает — оставляю
  координатору.
