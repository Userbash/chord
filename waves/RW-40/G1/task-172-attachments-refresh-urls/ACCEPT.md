# Приёмка PM: task-172 (BE-P2-09) — POST /attachments/refresh-urls для обновления истёкших CDN URL вложений

- **Статус:** `Ready for Technical Review` (Этап 1 PM Приёмки — **ОДОБРЕНО**)
- **Ветка:** `feature/task-172-attachments-refresh-urls`
- **Пулл-реквест:** `PR-57` (`feature/task-172-attachments-refresh-urls -> unstable`)

## Сверка с продуктовым ТЗ и DoD
1. **`REFRESH_ATTACHMENT_URLS_PATH` и `RefreshedAttachmentUrl` в `attachments.rs`**:
   - Добавлены константы `REFRESH_ATTACHMENT_URLS_PATH = "/attachments/refresh-urls"`, `MAX_REFRESH_URLS_PER_REQUEST = 50` и структура `RefreshedAttachmentUrl { original, refreshed }`.
2. **`refresh_attachment_urls` с предпроверкой до сети и типизацией ошибок**:
   - Проверяет непустой список (`1..=50`), валидность `https://` URL, отсутствие дубликатов и управляющих символов (`InvalidArg`), отправляет `POST /attachments/refresh-urls` с `{"attachment_urls": [...]}`, разбирает `{"refreshed_urls": [...]}` и типизирует `429` (`RateLimited { retry_after_ms }`), `401/403/404/5xx`.
   - Все 30 тестов в `attachments_contract.rs` и 17 в `lib.rs` проходят (`47/47 PASS`, `clippy 0 warnings`).

Передано на **Техническое ревью (Review 86A)** (`5/5` мутаций).
