# Отчёт воркера — task-172 (BE-P2-09): POST /attachments/refresh-urls для истёкших CDN URL

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-172` / `BE-P2-09` (`POST /attachments/refresh-urls` для истёкших CDN URL вложений).
- **Файлы воркера**:
  - `rust/gui-spike/src/attachments.rs`
  - `rust/gui-spike/tests/attachments_contract.rs`
- **Ветка/worktree**: `feature-task-172-attachments-refresh-urls`, `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.
- **Независимые оракулы**:
  - `RE/rest-static.json` и `rest-static.md`: маршрут `POST /attachments/refresh-urls` (`ATTACHMENTS_REFRESH_URLS`).
  - Эталонный RE-дамп `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0:149`: вызов `POST /attachments/refresh-urls` с телом `{"attachment_urls": [url, ...]}` и ответом `body.refreshed_urls[...].refreshed`.
  - `ATTACH-RE.md` и `BACKLOG-RE-BACKEND.md` (секция BE-P2-09).

## 2. Что сделано

1. Добавлены константы:
   - `pub const REFRESH_ATTACHMENT_URLS_PATH: &str = "/attachments/refresh-urls";`
   - `pub const MAX_REFRESH_URLS_PER_REQUEST: usize = 50;`
2. Добавлена структура `RefreshedAttachmentUrl`:
   - `pub struct RefreshedAttachmentUrl { pub original: String, pub refreshed: String }` с реализацией `Serialize`, `Deserialize`, `Debug`, `Clone`, `PartialEq`, `Eq`.
3. Реализована функция `refresh_attachment_urls`:
   - Локальная предпроверка до сети: непустой срез, `<= MAX_REFRESH_URLS_PER_REQUEST` (50), каждый URL валидируется через `parse_https_upload_url`, отсекаются пустые строки, `http://`, пробелы, управляющие символы и дубликаты (`AttachmentError::InvalidArg`).
   - Отправка `POST /attachments/refresh-urls` с телом `{"attachment_urls": [...]}`.
   - Декодирование ответа 2xx `{"refreshed_urls": [...]}` с проверкой непустых `original`/`refreshed` и схемы `https://` в `refreshed` (`AttachmentError::InvalidResponse`).
   - Типизация статуса 429 в `AttachmentError::RateLimited { retry_after_ms }` из заголовка `Retry-After`.
   - Типизация статусов 401, 403, 404, 5xx и других ошибок через маппинг в `RestFailure`.
4. Реализован вспомогательный метод `DiscordAttachments::refresh_attachment_urls`.
5. Добавлены 7 новых контрактных тестов в `rust/gui-spike/tests/attachments_contract.rs`.
6. Сохранена 100% обратная совместимость со всеми существующими тестами (23 существующих теста + 7 новых = 30 тестов).

## 3. RED-прогон

Зафиксирован в `red.log` до реализации функционала (падение компиляции отсутствующих констант, типов и методов в тестах):
```
error[E0432]: unresolved imports `discord_gui_spike::attachments::refresh_attachment_urls`, `discord_gui_spike::attachments::RefreshedAttachmentUrl`, `discord_gui_spike::attachments::MAX_REFRESH_URLS_PER_REQUEST`, `discord_gui_spike::attachments::REFRESH_ATTACHMENT_URLS_PATH`
error[E0599]: no method named `refresh_attachment_urls` found for struct `DiscordAttachments`
```

## 4. GREEN-прогон

Зафиксирован в `green.log`:
- `lib`: 17 passed; 0 failed
- `attachments_contract`: 30 passed; 0 failed; 0 ignored
- `clippy`: 0 warnings (`--lib --test attachments_contract -- -D warnings`)
- `rustfmt`: форматирование строго по стандарту (`edition 2021`)

## 5. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/attachments.rs` | `3400567c7ac7c990c3b7078e5ca99174` |
| `rust/gui-spike/tests/attachments_contract.rs` | `d53a458fcb5d6586cf17d082af98d5e0` |
| `waves/RW-40/G1/task-172-attachments-refresh-urls/red.log` | `aae59f2ad33fbf391c4e70abb5af46fb` |
| `waves/RW-40/G1/task-172-attachments-refresh-urls/green.log` | `37b8b18e79bfb76e3e9813c482f35560` |

## 6. Риски

- Рисков регрессии нет: затронут только изолированный модуль `attachments.rs` и его контрактные тесты; все 23 существующих теста проходят без изменений.
- Запросы на обновление URL типизированы и валидируются локально до ввода/вывода, предотвращая невалидные сетевые вызовы.
