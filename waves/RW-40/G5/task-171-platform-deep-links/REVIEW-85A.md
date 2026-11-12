# Техническое ревью (Review 85A): task-171 (BE-P2-13: Deep links `disclip://`, `extract_url_from_args`, `DeepLinkQueue`, `MAIN_WINDOW_PATH`)

- **Пулл-реквест:** `PR-56` (`feature/task-171-platform-deep-links -> unstable`)
- **Карточка:** task-171 / BE-P2-13
- **Отдел:** G5 Voice / Platform
- **Ревьюер:** Technical Reviewer (DeepSeek / Review 85A)
- **Вердикт:** **APPROVED (Ready for Merge)**
- **Замечания:** `P0 = 0`, `P1 = 0`, `P2 = 0`, `P3 = 0`

---

## 1. Проверка зоны ответственности и изменённых файлов

Проверено состояние рабочего каталога (`git status`, `git diff HEAD`):
- Изменены исключительно разрешённые ТЗ файлы:
  - `rust/crates/discord-platform/src/deep_link.rs` (`08793af4c3c5caa4a33095ebda472532`)
  - `rust/crates/discord-platform/tests/deep_link_contract.rs` (`1aafec7b2d2f9c6d6eabdf1652d4f248`)
- Посторонние файлы, общие модули ядра и служебные файлы не затронуты.
- Коммиты в git не создавались.

---

## 2. Верификация тестов и линтеров

Запущены целевые проверки со связкой с общим `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`:
1. `cargo test --manifest-path rust/Cargo.toml -p discord-platform --test deep_link_contract --lib`:
   - 1 unit test в `lib.rs`: `ok`
   - 17/17 контрактных тестов в `tests/deep_link_contract.rs`: `ok` (0 failed).
2. `cargo clippy --manifest-path rust/Cargo.toml -p discord-platform --test deep_link_contract --lib -- -D warnings`:
   - `0 warnings`, clean.

---

## 3. Анализ архитектуры и соответствия RE-оракулу

1. **Протокол клипов `disclip://`:**
   - Внедрены константы `DISCORD_APP_PROTOCOL = "discord"`, `DISCORD_CLIP_PROTOCOL = "disclip"`.
   - В `DeepLink` добавлено поле `scheme`, парсер корректно поддерживает формы `disclip://<id>`, `disclip:///path`, `disclip:<id>` в соответствии с `RE/main-arch.json` §9, §10.
   - Секретные данные и токены продолжают маскироваться (`<redacted>`) в `Debug`.
2. **Парсинг аргументов (`extract_url_from_args`):**
   - Точно повторяет логику Electron `extractUrlFromArgs`:
     - `--url` перед `--` с аргументом сразу после `--`;
     - `--url=<val>` и `--url <val>` (включая передачу после `--`);
     - прямые аргументы со схемами `discord://`, `disclip://`, `discord:`, `disclip:` в `argv[1..]`.
   - Сохранена обратная совместимость функции `extract_deep_link`.
3. **Очередь и маршрутизация (`DeepLinkQueue` & `main_window_path`):**
   - Структура `DeepLinkQueue` реализует поведение `openOrQueueUrl` с ограниченным FIFO буфером (`capacity.max(1)`).
   - При неготовности окна ссылки буферизуются, при переполнении корректно вытесняется старейший элемент.
   - После вызова `mark_window_ready` ссылки возвращаются немедленно (`Ok(Some(path))`).
   - Метод `drain_queued` освобождает буфер в FIFO-порядке.
   - Функция `main_window_path` преобразует ссылки в канонический путь для `MAIN_WINDOW_PATH` (`/channels/...`, `/settings/...`, `/clips/...`) с опциональным query.
   - Ошибки типизированы через `MainWindowPathError` без паник и без подмены на `Ok`.

---

## 4. Верификация мутаций (5/5 KILLED)

Проведено тестирование устойчивости к мутациям (Mutation Testing):
1. **Мутация 1 (Rejecting `disclip://` scheme):**
   - Подмена: отключение проверки `is_disclip` в парсере схемы.
   - Результат: **KILLED** (тесты `d14_disclip_protocol_parsing`, `d16_deep_link_queue_lifecycle`, `d17_main_window_path_routing_and_errors` упали).
2. **Мутация 2 (Failing to extract `--url` after `--`):**
   - Подмена: игнорирование префикса `--url=` после `--`.
   - Результат: **KILLED** (тест `d15_extract_url_from_args_oracle` упал).
3. **Мутация 3 (Losing queued deep links before `mark_window_ready`):**
   - Подмена: пропуск сохранения ссылки в `self.queue.push_back(path)`.
   - Результат: **KILLED** (тест `d16_deep_link_queue_lifecycle` упал).
4. **Мутация 4 (Wrong `MAIN_WINDOW_PATH` routing for channels/settings/clips):**
   - Подмена: генерация пути `/wrong/{section}` вместо `/settings/{section}`.
   - Результат: **KILLED** (тесты `d16_deep_link_queue_lifecycle`, `d17_main_window_path_routing_and_errors` упали).
5. **Мутация 5 (Panicking or returning Ok on unknown route):**
   - Подмена: возврат `Ok(MainWindowPath)` вместо `Err(MainWindowPathError::UnknownRoute)`.
   - Результат: **KILLED** (тест `d17_main_window_path_routing_and_errors` упал).

Итог: **5/5 мутаций успешно уничтожены**.

---

## 5. Итог

Задача полностью удовлетворяет критериям качества, DoD и регламенту репозитория.
Рекомендуется к слиянию в ветку `unstable` в рамках `PR-56`.
