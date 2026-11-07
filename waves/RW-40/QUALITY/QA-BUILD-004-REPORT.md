# Отчёт централизованного тестирования QA и сертификации сборки — `BUILD-004`

- **Роли:** DevOps / Integration + G4 QA Lead + G6 Tech Writer + PM Review
- **Проверяемая сборка:** `discord-lab` (ветка `testing` / `main`, рабочее дерево `BUILD-004`)
- **Манифест сборки:** [waves/RW-40/BUILDS/BUILD-004.md](file:///home/sanya/orca/project/waves/RW-40/BUILDS/BUILD-004.md)
- **Дата и время сертификации:** 2026-11-07, 14:36 +05:00
- **Финальный вердикт:** **QA Passed / Ready for Release** — `P0 = 0`, `P1 = 0`, `P2 = 0`.
- **Мутационный гейт:** **5/5 мутаций убиты (100%)**, исходники побайтно верифицированы.

Все обязательные контуры качества (`typecheck`, `ui-tsc`, `meta`, `unit`, `workspace-rust`, `smoke`, `sbom`, `release`) зелёные. Продуктовый код и тестовые оракулы полностью согласованы.

---

## 1. Окружение и состояние сборки

| Параметр | Значение |
|---|---|
| **ОС / Ядро** | `Linux 7.3.0-5-cachyos x86_64` |
| **Node.js / npm** | `v26.13.0` / `11.20.0` |
| **TypeScript (`tsc`)** | `Version 7.0.2` (`tsconfig.json` + `tsconfig.agent.json` — 0 ошибок) |
| **Rust** | `rustc 1.99.0` / `cargo 1.99.0` (`CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`) |
| **Release бинарь** | `rust/dist/discord-rs` (SHA256: `42ba8f0938a03d998eac713f61aa56d1210b7ffb4e1261b719ed35bba93b12ae`) |
| **SBOM спецификация** | `rust/dist/sbom.spdx.json` (98 пакетов, SHA256: `acb867f70bd6b4891a274388677363fbb9a521213234bbf384c14f9806de2921`) |
| **Контрольные суммы** | `rust/dist/SHA256SUMS` (`sha256sum -c SHA256SUMS` — **ЦЕЛ**) |

---

## 2. Результаты сводного гейта качества (`npm run gate:worker`)

Сквозной гейт выполнен с кодом выхода `0`. Лог: `.gate/worker-2026-11-07T09-34-26-647Z.json`.

| Фаза гейта | Объём проверки | Результат | Статус |
|---|---|---|---|
| **typecheck** | `tsc -p tsconfig.json && tsc -p tsconfig.agent.json` | 0 ошибок | **PASS** |
| **ui-tsc** | `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | 0 ошибок | **PASS** |
| **meta** | `tests/meta/**/*.test.mjs` | 102 теста: 102 pass, 0 fail, 0 skip | **PASS** |
| **unit** | `tests/unit/**/*.test.mjs` | 1826 тестов: 1826 pass, 0 fail, 0 skip | **PASS** |
| **spike-rust** | `cargo test --manifest-path rust/gui-spike/Cargo.toml` | 28 lib, 13 main, 25 suites pass | **PASS** |
| **workspace-rust** | `cargo test --manifest-path rust/Cargo.toml --workspace` | 1341 тест: 1329 pass, 0 fail, 12 ignored | **PASS** |
| **smoke** | `tests/smoke/**/*.test.mjs` (live Tauri + AT-SPI + X11) | 22 теста: 22 pass, 0 fail, 0 skip | **PASS** |

---

## 3. Сертифицированные модули и фичи сборки `BUILD-004`

### 3.1. STUB-13: Вложения и медиа-контент (Attachments)
- Полка вложений `#attachment-shelf` над полем ввода композера с превью, спойлерами (`SPOILER_`) и удалением.
- Протокол выгрузки: предварительное получение слотов `POST /channels/{id}/attachments` (`attachments_upload_slots`) и прямой `PUT` байтов (`attachments_upload_put`).
- Корректная связка кнопки `[data-action="attach"]` с hidden input `#composer-file-input` и передача `uploaded_filename` при отправке сообщения.
- Верификация: 57 unit-тестов в `tests/unit/attachments-*.test.mjs` и `tests/unit/composer.test.mjs` зелёные.

### 3.2. STUB-10: Закреплённые сообщения и Ветки (Pins & Threads)
- Прогрессивные выпадающие панели `#chat-pins-panel` и `#chat-threads-panel` в рабочей области чата с взаимным исключением (открытие одного закрывает другое).
- Интеграция с IPC `pins_list`, `unpin_channel_message`, `threads_list`.
- Вкладки активных и архивных веток, поиск, валидация сроков автоархивации (60, 1440, 4320, 10080 мин).
- Верификация: 17 unit-тестов в `tests/unit/pins-threads.test.mjs` и `tests/unit/threads-drawer.test.mjs` зелёные.

### 3.3. CARD-TEL-006: Captcha Challenge (код 40003) при `accept_invite`
- Обработка HTTP 400 со специфическим телом Discord Captcha Challenge (`code: 40003`, `captchaRequired`).
- Дружелюбный пользовательский текст в UI («Discord запросил проверку Captcha при вступлении на сервер (код 40003)...») вместо неопределённой ошибки `[object Object]`.
- Верификация: `tests/unit/join-server.test.mjs` (26/26 pass).

### 3.4. CARD-TEL-007: Реактивный Gateway Dispatch
- Доставка реальных событий шлюза (`GUILD_CREATE/DELETE`, `RELATIONSHIP_ADD/REMOVE/UPDATE`, `CALL_CREATE/UPDATE/DELETE`) через `channel.deliver(DispatchPayload)`.
- Подписка `guild-rail` и `dmFriendsController` на события шлюза ядра без агрессивного поллинга (интервал фонового опроса снижен с 6 до 60 секунд, ручной клик троттлится до 30 секунд).
- Верификация: `tests/net_contract.rs` (32/32 pass), `tests/ipc_wiring_contract.rs` (9/9 pass), `tests/unit/dispatch_ui.test.mjs` (57/57 pass).

### 3.5. STUB-11 & STUB-16: Панель «Входящие» (Inbox 🔔) и Профиль (User Popover)
- Выпадающая панель входящих `#chat-inbox-panel` с 4 вкладками («Упоминания», «Непрочитанные», «Для вас», «Заявки»), переходом к сообщению и отметкой прочитанности.
- Полнофункциональная карточка профиля пользователя (аватар, баннер, роли, заметки, вызов модалки).
- Верификация: `tests/unit/inbox*.test.mjs` (25 pass), `tests/unit/user-popout.test.mjs` (17 pass).

---

## 4. Мутационное тестирование (Двойная приёмка PM + Tech Reviewer: 5/5 убиты)

| ID | Мутация | Целевой модуль | Ожидаемый сбой | Факт | Итог |
|---|---|---|---|---|---|
| **M1** | Изменение имени `case "captchaRequired":` на `case "captchaRequired_mutated":` | `join-server.js` | Тест `CARD-TEL-006` в `join-server.test.mjs` | `AssertionError`: input did not match regexp (получено «Невозможно присоединиться...») | **УБИТА** |
| **M2** | Очистка атрибута title у кнопки прикрепления (`attach.setAttribute("title", "")`) | `actions.js` | Тест `gui-spike-features.test.mjs:1627` | `AssertionError`: у attach нет подсказки | **УБИТА** |
| **M3** | Изменение атрибута `aria-disabled` с `"true"` на `"false"` в спайке | `actions.js` | Тест `gui-spike-features.test.mjs:1626` | `AssertionError`: `'false' !== 'true'` | **УБИТА** |
| **M4** | Подмена кода ошибки капчи `40003` на `99999` в ветке `case "client":` | `join-server.js` | Тест `Client 400 с code 40003` в `join-server.test.mjs` | `AssertionError`: получено «запрос отклонён сервером (HTTP 400)» | **УБИТА** |
| **M5** | Уменьшение лимита `MAX_UPLOAD_FILE_BYTES` с 20 МиБ до 10 МиБ | `composer.js` | Тесты лимитов в `composer.test.mjs` | `AssertionError`: 3 теста упали (`10485760 !== 20971520`, regex mismatch) | **УБИТА** |

Все исходные файлы после мутаций восстановлены побайтно, целостность проверена повторным прогоном.

---

## 5. Итоговый вердикт и готовность к релизу

Сборка `BUILD-004` полностью удовлетворяет критериям приёмки:
1. **P0 = 0, P1 = 0, P2 = 0** — блокирующие дефекты отсутствуют.
2. **Мутационный гейт = 5/5 (100%)** — устойчивость тестов доказана.
3. **Релизный бинарь и контрольные суммы SHA256SUMS актуализированы и проверены.**
4. **Статус:** **QA PASSED / APPROVED FOR RELEASE (testing ➔ beta ➔ main)**.
