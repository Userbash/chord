# Техническое ревью карточки task-165 (BE-P2-02: Заголовки REST — сборка Accept-Language с весами q=0.9..0.1 и условная передача X-Fingerprint, X-Installation-ID, X-Routing-Key)

- **Карточка**: task-165 / BE-P2-02
- **Ветка**: `feature/task-165-rest-headers-qlist-fingerprint`
- **Ревьюер**: DeepSeek / Technical Reviewer
- **Дата**: 2026-10-30
- **Вердикт**: **PASS / Ready for Merge**

---

## 1. Проверенные файлы и контрольные суммы (md5)

Файлы в scope карточки task-165:
- `rust/crates/discord-core/src/rest/headers.rs` (md5 `aa20038bdd93b78ad4fabdd6764ab878`)
- `rust/crates/discord-core/src/rest/client.rs` (md5 `11e42b7d2456f0bc5d6b3258b737f61f`)
- `rust/crates/discord-core/tests/rest_client.rs` (md5 `e56e6c58aecdd4dc589d08af1f811023`)

Все контрольные суммы полностью совпадают до и после проверки мутаций. Посторонние файлы не затронуты.

---

## 2. Результаты запуска целевых тестов

Команда запуска с общим кэшем сборки `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`:
```sh
cargo test --manifest-path rust/Cargo.toml -p discord-core --test rest_client --test rest_headers --lib
```

Результаты:
- `discord_core::lib`: 33 passed, 0 failed.
- `rest_client`: 31 passed, 0 failed.
- `rest_headers`: 5 passed, 0 failed.
- Итого: 69 passed, 0 failed.

---

## 3. Проверка 5 ручных мутаций (5/5 убиты тестами)

### Мутация 1: Дефолтная локаль при пустом списке в `format_accept_language`
- **Файл**: `rust/crates/discord-core/src/rest/headers.rs`
- **Изменение**: Замена `return DEFAULT_LOCALE.to_string();` на `return "en-GB".to_string();`.
- **Результат**: Тест `format_accept_language_qlist_spec_examples` **упал** (`left: "en-GB"`, `right: "en-US"`).
- **Статус**: **УБИТ (KILLED)**.

### Мутация 2: Нижняя граница весов `q` в `format_accept_language`
- **Файл**: `rust/crates/discord-core/src/rest/headers.rs`
- **Изменение**: Замена `.max(1)` на `.max(2)` при расчёте `q_tenth` (не опускаться ниже `q=0.2` вместо `q=0.1`).
- **Результат**: Тест `format_accept_language_qlist_spec_examples` **упал** на 10-й локали (`left: "...l10;q=0.2"`, `right: "...l10;q=0.1"`).
- **Статус**: **УБИТ (KILLED)**.

### Мутация 3: Проверка пробельных строк в `HeaderConfig::extra_headers`
- **Файл**: `rust/crates/discord-core/src/rest/headers.rs`
- **Изменение**: Замена `!val.trim().is_empty()` на `!val.is_empty()` для `installation_id`.
- **Результат**: Тест `optional_headers_omitted_when_none_or_empty_or_whitespace` **упал** («пробельный X-Installation-ID утёк в запрос»).
- **Статус**: **УБИТ (KILLED)**.

### Мутация 4: Дедупликация явных заголовков при добавлении `extra_headers` в `ApiClient::prepare`
- **Файл**: `rust/crates/discord-core/src/rest/client.rs`
- **Изменение**: Удаление проверки `if !has_header(&prepared.headers, name)` перед добавлением `extra_headers`.
- **Результат**: Тест `optional_headers_builder_methods_and_post_once` **упал** (`assertion left == right failed: ровно один X-Fingerprint: left: 2, right: 1`).
- **Статус**: **УБИТ (KILLED)**.

### Мутация 5: Форматирование q-префикса в `format_accept_language`
- **Файл**: `rust/crates/discord-core/src/rest/headers.rs`
- **Изменение**: Замена `write!(result, ";q=0.{q_tenth}")` на `write!(result, ";q={q_tenth}")`.
- **Результат**: Тесты `api_auth_from_token_with_locales_sets_accept_language_and_discord_locale` и `format_accept_language_qlist_spec_examples` **упали** (`left: "ru,ru-RU;q=9,en-US;q=8"`, `right: "ru,ru-RU;q=0.9,en-US;q=0.8"`).
- **Статус**: **УБИТ (KILLED)**.

**Сводка по мутациям**: 5/5 мутантов успешно убиты тестами.

---

## 4. Архитектурный и кодовый аудит

1. **Соответствие ТЗ и RFC**:
   - Формирование `Accept-Language` полностью повторяет логику взвешенных списков с шагом `0.1` и насыщением на `0.1`.
   - Поддержка опциональных заголовков `X-Fingerprint`, `X-Installation-ID`, `X-Routing-Key` полностью исключает отправку пустых/пробельных значений или значений `None`.
   - Явно переданные в запросе заголовки имеют приоритет над глобальными заголовками клиента и не дублируются.
2. **Обратная совместимость**:
   - Базовый контракт `AUTH_HEADER_NAMES` (6 базовых заголовков) сохранён без изменений.
   - Существующие конструкторы `ApiClient::new`, `ApiAuth::from_token`, `ApiAuth::from_values` сохраняют прежнее поведение.
   - Добавлены необходимые алиасы (`OptionalHeaderConfig`, `ClientHeadersConfig`, `RestHeadersConfig`).
3. **Безопасность и чистота**:
   - Нет небезопасного кода (`unsafe`), утечек памяти или паник на пустых входных срезах.
   - Корректная обработка `trim()` и `is_empty()`.

---

## 5. Итоговое заключение

Все критерии приёмки и требования гейтов выполнены:
- Тесты модуля `discord-core` проходят успешно (33 lib + 31 rest_client + 5 rest_headers).
- 5/5 мутаций убиты независимыми проверками в тестах.
- Исходный код восстановлен в точное соответствие с исходным состоянием (`md5sum` совпадает на 100%).
- Задача **task-165** готова к вливанию в `unstable`.
