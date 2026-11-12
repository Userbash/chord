# Техническое ревью (Review 86A): task-172 (BE-P2-09) — POST /attachments/refresh-urls для истёкших CDN URL вложений

- **Ветка:** `feature/task-172-attachments-refresh-urls`
- **Пулл-реквест:** `PR-57` (`feature/task-172-attachments-refresh-urls -> unstable`)
- **Статус:** **APPROVED (Ready for Merge)**
- **Критичность замечаний:** `P0 = 0`, `P1 = 0`, `P2 = 0`
- **Проверка мутаций:** `5/5 (100%) KILLED`

---

## 1. Проверка состава изменений (Scope & Git Diff)

- Проверено состояние `git status` и `git diff HEAD`:
  - Модифицированы исключительно разрешённые файлы:
    - `rust/gui-spike/src/attachments.rs` (md5: `3400567c7ac7c990c3b7078e5ca99174`)
    - `rust/gui-spike/tests/attachments_contract.rs` (md5: `d53a458fcb5d6586cf17d082af98d5e0`)
  - Рабочее дерево чисто от посторонних файлов, не затронуты общие модули или файлы других воркеров.
  - Изменения согласованы с базовой веткой `unstable`.

---

## 2. Результаты запуска тестов и линтера

Тесты и clippy запущены в изолированном workspace `rust/gui-spike` с общим `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`:
- **`cargo test --manifest-path rust/gui-spike/Cargo.toml --test attachments_contract --lib`**:
  - `src/lib.rs`: 17 passed, 0 failed, 0 ignored.
  - `tests/attachments_contract.rs`: 30 passed (23 существующих + 7 новых), 0 failed, 0 ignored.
  - Итого: **47 passed, 0 failed**.
- **`cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test attachments_contract --lib -- -D warnings`**:
  - **0 warnings / 0 errors**.
- **`rustfmt`**: стиль и форматирование кода соответствуют стандартам кодовой базы.

---

## 3. Анализ и убийство мутаций (5/5 Mutations Killed)

| # | Мутация | Описание мутации | Результат проверки | Убивающий тест в `attachments_contract.rs` |
|---|---|---|---|---|
| **M1** | Неверный маршрут `!= /attachments/refresh-urls` | Подмена `REFRESH_ATTACHMENT_URLS_PATH` на `/attachments/refresh` или другой путь. | **KILLED** | `refresh_attachment_urls_constants_match_oracle_and_limits` + `refresh_attachment_urls_sends_post_and_parses_response` проверяют точный маршрут и заголовок запроса. |
| **M2** | Пропуск лимитов `> 50` URL или пустого списка | Отключение проверки `urls.is_empty() \|\| urls.len() > 50` в `validate_refresh_urls`. | **KILLED** | `refresh_attachment_urls_rejects_empty_and_oversized_batches_before_network` (проверяет пустой срез и срез из 51 URL, ожидая `AttachmentError::InvalidArg` без сетевого вызова). |
| **M3** | Разрешение `http://` или дубликатов | Отключение `seen.insert(...)` или проверки схемы `https://` / запрещённых символов. | **KILLED** | `refresh_attachment_urls_rejects_invalid_urls_and_duplicates_before_network` (проверяет `http://`, пробелы, управляющие символы, фрагменты `#` и дубликаты). |
| **M4** | Приём пустых или `http://` URL в ответе | Пропуск валидации `parse_refreshed_urls` для `original` и `refreshed` полей ответа. | **KILLED** | `refresh_attachment_urls_rejects_malformed_and_invalid_responses` (проверяет пустые строки `original`/`refreshed` и `http://` в `refreshed`, ожидая `AttachmentError::InvalidResponse`). |
| **M5** | Сбой разбора `Retry-After` при HTTP 429 | Игнорирование или неправильный разбор заголовка `retry-after` при статусе 429. | **KILLED** | `refresh_attachment_urls_maps_rest_error_status_codes` (проверяет 429 с заголовком `retry-after: 3.5` -> `RateLimited { retry_after_ms: 3500 }`). |

Все 5/5 мутаций надёжно покрыты независимыми тестами-оракулами.

---

## 4. Архитектурное и техническое качество

1. **Безопасность и валидация:** Все проверки аргументов (длина, схема HTTPS, ASCII, отсутствие дубликатов) выполняются локально до осуществления сетевого I/O, исключая спам сервера невалидными запросами.
2. **Типизация ошибок:** Все статусы Discord REST API корректно маппятся в строгие enum-варианты `AttachmentError` (`RateLimited`, `Unauthorized`, `Forbidden`, `NotFound`, `Server`, `Client`, `Network`).
3. **Совместимость с RE-оракулом:** Контракт точно соответствует дампу Discord (`reference-oracle/decoded/4d2a64cb5df5fc4e_0:149`) и документации REST маршрутов (`POST /attachments/refresh-urls`).
4. **Конфиденциальность:** Ошибки `AttachmentError` не логируют и не передают токены авторизации или тело файлов.

---

## 5. Итоговый вердикт

- **P0 = 0**, **P1 = 0**.
- **Мутации: 5/5 убито (100%)**.
- **Вердикт:** **APPROVED (Ready for Merge)**. PR-57 готов к слиянию в ветку `unstable`.
