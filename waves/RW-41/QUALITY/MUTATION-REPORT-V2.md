# MUTATION REPORT V2 — TASK-622 (Quality Gate 15/15)

- **Дата:** 2026-11-07
- **Воркер / Роль:** QA Automation Lead G4
- **Ветка:** `qa/task-622-mutation-suite`
- **Тестовый сьют:** `tests/meta/mutations.test.mjs`
- **Статус Quality Gate:** **15/15 KILLED (100% Mutation Score)**, 0 survivors, 0 inconclusive.

---

## 1. Сводка результатов мутационного тестирования

Мутационный сьют расширен с базовых 5 до 15 комплексных точек отказов и аномалий, охватывающих сетевой стек, шлюз, шифрование DAVE MLS v1, медиа-буфер LatestFrameSlot, захват экрана, валидацию прав, аутентификацию, защиту от XSS и хранилище секретов Keyring.

| ID | Подсистема | Описание мутации / точка отказа | Защитный контур / Оракул | Результат |
|---|---|---|---|:---:|
| **MUT-01** | Auth / Network | Инъекция некорректного токена auth (битый base64, спецсимволы, NUL-байт) | `ui/features/auth.js` (`InvalidToken`, `AUTH_MESSAGES.invalid`) | **KILLED** |
| **MUT-02** | Voice Gateway | Аварийный разрыв сокета голосового шлюза (WS 1006 / network failure) | `ui/modules/voice-rtc-panel.js` (`resolveVoiceCallPhaseView`, `canRetry: true`) | **KILLED** |
| **MUT-03** | Invites / Captcha | Эмуляция ошибки HTTP 400 hCaptcha (`code: 40003`, `captcha_service: 'hcaptcha'`) | `ui/features/captcha-modal.js` (`isCaptchaChallengeError`, widget URL) | **KILLED** |
| **MUT-04** | Media / Buffer | Переполнение кольцевого буфера LatestFrameSlot и невалидные параметры кадра | `discord-media/src/buffer.rs` (`LatestFrameSlot`, atomic replacement, drop counter) | **KILLED** |
| **MUT-05** | Voice / E2EE | Искажение заголовка DAVE MLS v1 (отсутствие `0xFAFA` или даунгрейд версии) | `discord-voice/src/srtp.rs` (`DAVE_MARKER_BYTES`, `formatDaveTransitionSummary`) | **KILLED** |
| **MUT-06** | Screen Capture | Подмена геометрии ScreenCapturer (отрицательные, нулевые размеры, overflow) | `discord-media/src/capture/x11.rs` & `buffer.rs` (stride & bounds check) | **KILLED** |
| **MUT-07** | Chat / Composer | Превышение лимита 2000 символов и разрыв UTF-16 суррогатной пары | `ui/features/composer.js` (`COMPOSER_LIMIT = 2000`, surrogate guard) | **KILLED** |
| **MUT-08** | Roles / Perms | Попытка перезаписи `PERMISSION_FLAGS` и подделка прав администратора | `ui/features/role-permissions.js` (`Object.isFrozen`, bitfield parse) | **KILLED** |
| **MUT-09** | REST / Backoff | Инъекция некорректного заголовка 429 Rate Limit (отрицательный / NaN backoff) | `ui/features/auth.js` & backoff clamp (safe minimum ≥ 1s) | **KILLED** |
| **MUT-10** | Identity / Data | Подмена Snowflake ID на 0, отрицательное число или SQLi/XSS payload | Snowflake validator (`u64` range, `parseRemoteAuthUserRecord`) | **KILLED** |
| **MUT-11** | Codec / Wire | Искажение сжатого Gateway ETF / Zstandard фрейма (битый magic byte != 131) | `discord-codec/src/etf.rs` (ETF magic 0x83 wire validator) | **KILLED** |
| **MUT-12** | Chat / Security | Попытка XSS / Script Injection через разметку сообщений и спойлеры | `ui/features/markdown-helper.js` (`extractSpoilerSpans`, HTML escaping) | **KILLED** |
| **MUT-13** | Voice / Audio | Выход пользовательской/устройственной громкости за допустимые границы | `ui/features/voice-volume.js` (`clampUserVolume`, `clampDeviceVolume`, gain) | **KILLED** |
| **MUT-14** | Platform / Video | Отказ бэкенда захвата экрана PipeWire / X11 (отказ пользователя, битый fd) | `discord-platform/src/screencast.rs` (`PortalRequestError::ConsentDenied/InvalidFd`) | **KILLED** |
| **MUT-15** | Platform / Vault | Повреждение зашифрованного хранилища Keyring (NUL-байты, пустые токены) | `discord-platform/src/keyring.rs` (`SafeStorageMigrationError::InvalidToken`) | **KILLED** |

---

## 2. Подробное описание точек отказа и убитых мутантов

### MUT-01: Инъекция некорректного токена auth
- **Мутант:** Инъекция строк без точек, с управляющими символами, NUL-байтами и повреждённым base64 payload.
- **Оракул:** `parseRemoteAuthUserRecord` возвращает `null`, `formatRemoteAuthQrUrl` отклоняет токены с неразрешёнными символами, а UI маппит ошибку в `AUTH_MESSAGES.invalid` ("Токен недействителен — проверьте вставку") без утечки сырых данных в DOM или логи.

### MUT-02: Разрыв сокета голосового шлюза
- **Мутант:** Аварийное закрытие сокета (WS 1006 / TCP RST) при нахождении в фазе `connected` или `negotiating`.
- **Оракул:** Модуль `voice-rtc-panel.js` (`resolveVoiceCallPhaseView`) переводит состояние в `phase: 'error'`, сбрасывает флаг `isConnected: false`, устанавливает `isError: true` и активирует интерфейс повторного подключения `canRetry: true`.

### MUT-03: Эмуляция 400 hCaptcha ошибки
- **Мутант:** Симуляция ответа сервера 400 при вызове `accept_invite` с кодом `40003` и `captcha_service: 'hcaptcha'`.
- **Оракул:** Хелпер `isCaptchaChallengeError` строго идентифицирует ошибку как необходимость решения капчи, извлекает валидный `captcha_sitekey` и строит URL виджета на доверенном домене `https://newassets.hcaptcha.com`, отсекая общие сетевые ошибки 400.

### MUT-04: Переполнение кольцевого буфера LatestFrameSlot
- **Мутант:** Насыщение видеобуфера кадрами с высокой частотой (100 кадров за один цикл рендера) и подача кадров с нулевыми габаритами (`width == 0 || height == 0`).
- **Оракул:** `LatestFrameSlot` в `discord-media` атомарно сохраняет только последний актуальный кадр, инкрементирует `dropped_frames` при вытеснении, увеличивает `rejected_frames` при нулевых габаритах и предотвращает неконтролируемый рост памяти.

### MUT-05: Искажение заголовка DAVE MLS v1
- **Мутант:** Передача бинарных DAVE-фреймов без маркера `DAVE_MARKER_BYTES = [0xFA, 0xFA]` или попытка даунгрейда протокола до `version = 0`.
- **Оракул:** Контракт демультиплексирования SRTP/DAVE отбрасывает фреймы с некорректным маркером, а `formatDaveTransitionSummary` при `protocolVersion = 0` отключает индикатор DAVE (`isActive: false`), переводя связь в стандартный SRTP.

### MUT-06: Подмена размера экрана ScreenCapturer
- **Мутант:** Передача отрицательных, нулевых или превышающих `u32` значений геометрии захвата экрана X11/PipeWire.
- **Оракул:** Валидатор `width > 0 && height > 0` и проверка `layout.byte_len(width, height)` отсекают выделение испорченного буфера до обращения к MIT-SHM.

### MUT-07: Обрезка 2000 символов сообщения
- **Мутант:** Вставка текста длиной > 2000 символов, в том числе с эмодзи (суррогатной парой UTF-16) на границе 2000-го символа.
- **Оракул:** Константа `COMPOSER_LIMIT = 2000` обрезает ввод; логика защиты суррогатных пар исключает появление повисших High Surrogate (`[\uD800-\uDBFF]`).

### MUT-08: Невалидные флаги прав permissions
- **Мутант:** Попытка перезаписи свойств `PERMISSION_FLAGS` или передача нечисловых/пустых bitfield-строк для несанкционированного получения прав `ADMINISTRATOR`.
- **Оракул:** Объект `PERMISSION_FLAGS` заморожен (`Object.isFrozen`), попытка записи вызывает `TypeError`, а функция `hasPermissionBit` возвращает `false` на любом повреждённом bitfield.

### MUT-09: Инъекция 429 Rate Limit без retry_after
- **Мутант:** Сервер возвращает HTTP 429 с отрицательным (`-500`) или отсутствующим значением `retry_after_ms`.
- **Оракул:** Формула нормализации `Math.max(1, Math.ceil(retry_after / 1000))` зажимает задержку в безопасный минимум (≥ 1 секунда), исключая зацикливание запросов в бесконечный DDoS-цикл.

### MUT-10: Подмена Snowflake ID на нечисловой или 0
- **Мутант:** Передача `0`, отрицательных чисел, строк `' OR 1=1` или чисел свыше `18446744073709551615` (`u64::MAX`).
- **Оракул:** Строгий валидатор диапазона Discord Snowflake отклоняет нечисловые строки и значения вне диапазона 64-битного целого.

### MUT-11: Искажение сжатого Gateway ETF / Zstandard фрейма
- **Мутант:** Повреждение стартового байта бинарного ETF-пакета (байт `0x00` или zlib-преамбула вместо ETF magic `131` / `0x83`).
- **Оракул:** Валидатор шлюза `bytes[0] === 131` сразу отбрасывает фрейм до передачи в декомпрессор, защищая парсер от повреждённых данных.

### MUT-12: XSS / HTML Script Injection в разметке чата
- **Мутант:** Внедрение тегов `<script>alert('pwn')</script>` и обработчиков `<img onerror=...>` в тело сообщения и спойлеры.
- **Оракул:** `extractSpoilerSpans` извлекает содержимое как сырую строку, а рендерер экранирует HTML-сущности (`&lt;script&gt;`), блокируя исполнение сценариев внутри WebView.

### MUT-13: Выход за границы громкости звука
- **Мутант:** Установка громкости в `-100%`, `+9999%`, `NaN` или `Infinity`.
- **Оракул:** Функции `clampUserVolume` и `clampDeviceVolume` зажимают значения в допустимые диапазоны (0..200 для пользователя, 0..100 для устройства), а `computeAudioGainMultiplier` гарантирует неотрицательный множитель усиления (0.0..2.0).

### MUT-14: Отказ бэкенда захвата экрана PipeWire / X11
- **Мутант:** Отказ пользователя в диалоге выбора экрана портала (`ConsentDenied`) или возврат повреждённого файлового дескриптора (`fd < 0`).
- **Оракул:** Типизированное перечисление `PortalRequestError` и `CaptureError` перехватывает сбой без падения приложения и зависания процесса.

### MUT-15: Повреждение зашифрованного хранилища токенов Keyring
- **Мутант:** Запись в системный Keyring токенов с NUL-байтами, битым UTF-8 или пустых строк.
- **Оракул:** Крейт `discord-platform` в модуле `keyring.rs` возвращает типизированную ошибку `SafeStorageMigrationError::InvalidToken`, а UI отображает понятное сообщение `AUTH_MESSAGES.keyring`.

---

## 3. Воспроизведение

```bash
# 1. Запуск расширенного мутационного сьюта
node --test tests/meta/mutations.test.mjs

# 2. Полный прогон всех мета-тестов Quality Gate
npm run test:meta

# 3. Контроль типов TypeScript / JSDoc
npm run typecheck
```

### Фактический вывод сьюта:
```text
▶ TASK-622: Расширенный 15/15 мутационный сьют тестирования (Quality Gate v2)
  ✔ MUT-01: Инъекция некорректного токена auth (Auth token injection) (0.974419ms)
  ✔ MUT-02: Разрыв сокета голосового шлюза (Voice gateway socket disconnect) (0.257523ms)
  ✔ MUT-03: Эмуляция 400 hCaptcha ошибки (400 hCaptcha challenge emulation) (0.234882ms)
  ✔ MUT-04: Переполнение кольцевого буфера LatestFrameSlot (LatestFrameSlot ring buffer overflow) (1.085291ms)
  ✔ MUT-05: Искажение заголовка DAVE MLS v1 (Corrupted DAVE MLS v1 header) (0.784278ms)
  ✔ MUT-06: Подмена размера экрана ScreenCapturer (ScreenCapturer screen size tampering) (0.267273ms)
  ✔ MUT-07: Обрезка 2000 символов сообщения (Message 2000 chars truncation) (0.137801ms)
  ✔ MUT-08: Невалидные флаги прав permissions (Invalid permission flags) (0.368334ms)
  ✔ MUT-09: Инъекция 429 Rate Limit без retry_after (REST 429 Rate Limit Header Tampering) (0.132632ms)
  ✔ MUT-10: Подмена Snowflake ID на нечисловой или 0 (Snowflake ID Invariant Tampering) (0.225872ms)
  ✔ MUT-11: Искажение сжатого Gateway ETF / Zstandard фрейма (Corrupted ETF / Zstd Wire Frame) (0.118621ms)
  ✔ MUT-12: XSS / HTML Script Injection в разметке чата (Chat Markdown Script Injection) (0.168942ms)
  ✔ MUT-13: Выход за границы громкости звука (Voice Audio Volume Boundary Violation) (0.105251ms)
  ✔ MUT-14: Отказ бэкенда захвата экрана PipeWire / X11 (Screen Capture Backend Failure & Fallback) (0.172492ms)
  ✔ MUT-15: Повреждение зашифрованного хранилища токенов Keyring (Keyring SafeStorage Tampering) (0.294743ms)
  ✔ итоговый мутационный счёт: ровно 15/15 мутантов уничтожено (100% kill rate) (0.119142ms)
✔ TASK-622: Расширенный 15/15 мутационный сьют тестирования (Quality Gate v2) (6.985309ms)
ℹ tests 16
ℹ suites 1
ℹ pass 16
ℹ fail 0
ℹ cancelled 0
ℹ skipped 0
ℹ todo 0
```

## 4. Заключение

Требования **TASK-622** выполнены в полном объёме:
- Мутационный сьют расширен до 15 комплексных точек отказов в `tests/meta/mutations.test.mjs`.
- Все 15 мутантов уничтожаются (15/15 killed, 100% mutation score).
- Мета-тест синхронизирован в `TDD-PLAN.md` (§8), `npm run test:meta` проходит 118/118 тестов.
- `npm run typecheck` проходит чисто без ошибок.
- Подготовлен настоящий отчёт `waves/RW-41/QUALITY/MUTATION-REPORT-V2.md`.
