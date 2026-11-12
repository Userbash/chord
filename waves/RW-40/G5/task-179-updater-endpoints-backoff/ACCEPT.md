# Приёмка PM: task-179 (BE-P3-07) — Эндпоинты апдейтера, `ScheduledUpdateBackoff` и `SplashUpdateState`

- **PR:** PR-64 (`feature/task-179-updater-endpoints-backoff -> unstable`)
- **Статус:** `Ready for Technical Review` (передано Техническому ревьюеру в Review 93A)
- **Дата:** 2026-10-31
- **Автор ревью:** Product Manager (PM)

## Проверка соответствия ТЗ и DoD
1. **Константы и каналы релиза (`reference-oracle/main-arch.md` §5):**
   - `NEW_UPDATE_ENDPOINT = "https://updates.discord.com/"`, `DEFAULT_UPDATE_ENDPOINT = "https://discord.com/api"`, `DEFAULT_UPDATE_BACKOFF_MAX_MS = 30_000`, `SCHEDULED_UPDATE_BACKOFF_MS = [2_000, 3_000, 3_000, 4_000, 5_000, 7_000, 10_000, 12_000, 15_000]`.
   - `ReleaseChannel` (`Stable`, `Ptb`, `Canary`, `Development`) и `should_disable_updater`.
   - `LinuxPackageFormat` (`Deb`, `TarGz`), `build_linux_download_url`, `build_linux_feed_url`, `build_modules_base_url` со строгой валидацией `ManifestError::InvalidEndpoint`.
2. **Расписание ретраев и состояния сплэш-экрана:**
   - `ScheduledUpdateBackoff` (`next_delay_ms`, `retry_seconds`, `succeed`) и `SplashUpdateState::event_name`.
3. **Тесты и чистота:**
   - Red -> Green подтверждён (`46/46 PASS` в `updater_contract`).
   - Clippy (`-D warnings`) и `rustfmt --edition 2024 --check` чистые, write-scope строго соблюдён.

## Решение PM
**ПРИНЯТО (Accepted for Technical Review).** Передаётся на Техническое ревью (Review 93A) с проверкой 5/5 мутаций.
