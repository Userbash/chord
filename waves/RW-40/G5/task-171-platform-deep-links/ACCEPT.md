# Приёмка PM: task-171 (BE-P2-13) — Deep links: протокол disclip://, разбор --url после --, пред-очередь DeepLinkQueue и маршруты MAIN_WINDOW_PATH

- **Статус:** `Ready for Technical Review` (Этап 1 PM Приёмки — **ОДОБРЕНО**)
- **Ветка:** `feature/task-171-platform-deep-links`
- **Пулл-реквест:** `PR-56` (`feature/task-171-platform-deep-links -> unstable`)

## Сверка с продуктовым ТЗ и DoD
1. **Протокол `disclip://` и `extract_url_from_args` в `deep_link.rs`**:
   - Поддержаны `DISCORD_CLIP_PROTOCOL = "disclip"` и `DISCORD_APP_PROTOCOL = "discord"`, а также разбор `--url` (в том числе после `--`) по оракулу `main-arch.json` §9.
2. **Очередь `DeepLinkQueue` и маршрутизация `main_window_path`**:
   - Реализованы `open_or_queue_url` (bounded FIFO до готовности окна), `mark_window_ready`, `drain_queued`, `main_window_path` (`/channels/...`, `/settings/...`, `/clips/...`) и типизированный `MainWindowPathError`.
   - Все 17 тестов в `deep_link_contract.rs` проходят (`17/17 PASS`, `clippy 0 warnings`).

Передано на **Техническое ревью (Review 85A)** (`5/5` мутаций).
