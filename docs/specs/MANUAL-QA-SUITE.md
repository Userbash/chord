# Регламент, чек-листы и сценарии ручного исследовательского тестирования (`docs/specs/MANUAL-QA-SUITE.md`)

- **Задача:** `TASK-908` (`G4 QA & Automation`)
- **Подзадачи:** `SUB-908-1`, `SUB-908-2`, `SUB-908-3`
- **Связанные регламенты:** [`docs/specs/QA-AUTOMATION-STRATEGY-AND-TASKS.md`](file:///home/sanya/orca/project/docs/specs/QA-AUTOMATION-STRATEGY-AND-TASKS.md), [`TEST-POLICY.md`](file:///home/sanya/orca/project/TEST-POLICY.md), [`docs/specs/STREAM-WAYLAND-GUIDE.md`](file:///home/sanya/orca/project/docs/specs/STREAM-WAYLAND-GUIDE.md)
- **Программный модуль и контрактные тесты:**
  - [`rust/discord-gpui/src/manual_qa/mod.rs`](file:///home/sanya/orca/project/rust/discord-gpui/src/manual_qa/mod.rs)
  - [`rust/discord-gpui/src/manual_qa/checklists.rs`](file:///home/sanya/orca/project/rust/discord-gpui/src/manual_qa/checklists.rs)
  - [`rust/discord-gpui/src/manual_qa/desktop_matrix.rs`](file:///home/sanya/orca/project/rust/discord-gpui/src/manual_qa/desktop_matrix.rs)
  - [`rust/discord-gpui/src/manual_qa/exploratory.rs`](file:///home/sanya/orca/project/rust/discord-gpui/src/manual_qa/exploratory.rs)
  - [`rust/discord-gpui/tests/manual_qa_suite_contract.rs`](file:///home/sanya/orca/project/rust/discord-gpui/tests/manual_qa_suite_contract.rs)
- **Эталонные RE-оракулы (`/home/sanya/re-deep/`):**
  - `/home/sanya/re-deep/rest-static.md` (§1 `POST /auth/login`, `/auth/mfa/totp`, `/auth/logout`, лимиты сообщений и вложений)
  - `/home/sanya/re-deep/gateway-protocol.md` и `/home/sanya/re-deep/gateway-schema.md` (`OP 2 IDENTIFY`, `OP 4 VOICE_STATE_UPDATE`, `OP 6 RESUME`, `OP 18..22 STREAM_*`)
  - `/home/sanya/re-deep/native-re.md` (`org.freedesktop.portal.ScreenCast`, `BaseCapturerPipeWire`, `XShmGetImage`)
  - `/home/sanya/re-deep/voice-deep.md` (WebRTC аудиоустройства, Push-to-Talk, DAVE MLS v1)

---

## 1. Чек-листы приемочного тестирования — Smoke & Sanity Checklists (`SUB-908-1`)

Комплект приемочных чек-листов обязателен к прохождению перед переводом сборки из ветки `testing` в `beta` и `main`. Интерактивный жизненный цикл компонента `ManualQA` проходит состояния:
`checklist ready` (`ChecklistReady`) → `running bug-bash` (`RunningBugBash`) → `sign-off complete` (`SignOffComplete`).

### 1.1. Шаг 1.1: Чек-лист авторизации (`AuthChecklistValidator`)

| ID | Сценарий проверки | Шаги выполнения | Ожидаемый результат (Оракул `/home/sanya/re-deep/rest-static.md`) | Статус |
|---|---|---|---|---|
| `AUTH-01` | Вход по логину и паролю (`POST /auth/login`) | 1. Открыть окно входа.<br>2. Ввести валидный email и пароль (`>= 8` символов).<br>3. Нажать «Войти». | Запрос `POST /auth/login` возвращает `ticket` для 2FA (`LoginStep::Mfa`) либо сессионный токен. Ошибки валидации отсутствуют. | `[x]` |
| `AUTH-02` | Вход по QR-коду (Remote Auth WS) | 1. Переключиться на блок QR-кода.<br>2. Проверить генерацию отпечатка `discord-remote-auth-*`.<br>3. Подтвердить вход с мобильного клиента. | Отображается четкий QR-код без обрезки; при невалидном отпечатке возвращается `InvalidQrFingerprint`. | `[x]` |
| `AUTH-03` | Прохождение капчи и обработка кода `40003` | 1. Сымитировать ответ API с кодом ошибки `40003` / `captcha_key` (`hcaptcha`, `sitekey`, `rqdata`).<br>2. Пройти вызов в `CaptchaModal`. | При получении кода `40003` открывается модальное окно `CaptchaChallenge` с переданным `sitekey` и `rqdata`. | `[x]` |
| `AUTH-04` | Сохранение токена в системном Keyring и маскирование | 1. Успешно завершить вход.<br>2. Проверить сохранение в `libsecret` (`org.freedesktop.secrets`) и логи приложения. | `active_token` сохранен; во всех логах и отчетах токен `mfa.*` замаскирован как `mfa.[REDACTED]`. | `[x]` |
| `AUTH-05` | Выход из аккаунта (`Logout`) | 1. Открыть `SettingsDialog` → «Моя учётная запись».<br>2. Нажать «Выйти». | Вызывается очистка `active_token = None`, сброс `active_captcha = None`, возврат в `LoginStep::Credentials`. | `[x]` |

### 1.2. Шаг 1.2: Чек-лист чата (`ChatChecklistValidator`)

| ID | Сценарий проверки | Шаги выполнения | Ожидаемый результат (Граничные инварианты) | Статус |
|---|---|---|---|---|
| `CHAT-01` | Отправка текстового сообщения и лимит `2000 символов` | 1. Отправить сообщение длиной `1..=2000` символов.<br>2. Попытаться отправить `2001` символ и пустую строку из пробелов. | `2000 символов` отправляется мгновенно; `2001` символ блокируется с `MessageTooLong`; пустая строка отклоняется с `EmptyMessagePayload`. | `[x]` |
| `CHAT-02` | Эмодзи и реакции (`MessageReaction`) | 1. Открыть `EmojiPicker` или кликнуть «Добавить реакцию» на сообщении.<br>2. Выбрать Unicode/кастомный эмодзи (например, `🦀`). | Счетчик реакции увеличивается на `+1`, выставляется флаг `me: true`, повторный клик переключает состояние. | `[x]` |
| `CHAT-03` | Вложения до `25 МБ` и до `10` файлов | 1. Прикрепить файл размером ровно `25 МБ` (`26 214 400` байт).<br>2. Попытаться прикрепить `25 МБ + 1 байт` или `11` файлов. | До `10` файлов и `<= 25 МБ` успешно прикрепляются; превышение размера возвращает `413 Payload Too Large` (`PayloadTooLarge413`); `11` файлов — `TooManyAttachments`. | `[x]` |
| `CHAT-04` | Закрепление (`Pin`), Редактирование (`Edit`), Удаление (`Delete`), Цитирование (`Quote`) | 1. Закрепить сообщение через контекстное меню ПКМ.<br>2. Отредактировать текст.<br>3. Ответить с цитированием.<br>4. Удалить исходное сообщение. | При закреплении `is_pinned = true`; при редактировании появляется пометка `(изменено)`; цитата формирует блок `> исходный текст`; удаление убирает сообщение из ленты. | `[x]` |

### 1.3. Шаг 1.3: Чек-лист голоса (`VoiceChecklistValidator`)

| ID | Сценарий проверки | Шаги выполнения | Ожидаемый результат (Оракул `/home/sanya/re-deep/voice-deep.md`) | Статус |
|---|---|---|---|---|
| `VOICE-01` | Подключение к голосовому каналу | 1. Кликнуть на голосовой канал в `ChannelSidebar`.<br>2. Проверить плашку `VoicePanel` (52px) и пинг. | Отправляется `OP 4 VOICE_STATE_UPDATE`, устанавливается `VoiceSessionState` с индикацией пинга и эндпоинта. | `[x]` |
| `VOICE-02` | Переключение `Mute` и `Deafen` | 1. Нажать кнопку микрофона (`Mute`).<br>2. Нажать кнопку наушников (`Deafen`).<br>3. Снять `Deafen`. | Включение `Deafen` (`is_deafened = true`) принудительно включает `is_muted = true` (строгий инвариант Discord). | `[x]` |
| `VOICE-03` | Переключение аудиоустройств и hot-unplug fallback | 1. Выбрать микрофон и наушники PipeWire в настройках.<br>2. Физически отключить активный USB-микрофон во время звонка. | Поток переключается без падения процесса; при исчезновении устройства автоматически выбирается `"Default"`. | `[x]` |
| `VOICE-04` | Режим `Push-to-Talk` и индикатор громкости (`0..200%`) | 1. Включить режим `Push-to-Talk` и зажать клавишу активации.<br>2. Проверить ползунки громкости ввода/вывода на границах `0%`, `100%`, `200%` и `201%`. | При зажатом PTT загорается зеленое кольцо `is_speaking = true`; диапазон `0..=200%` (`0.0..=2.0`) применяется штатно, `> 200%` отклоняется с `VolumeOutOfBounds`. | `[x]` |

### 1.4. Шаг 1.4: Чек-лист шаринга экрана (`ScreenShareChecklistValidator`)

| ID | Сценарий проверки | Шаги выполнения | Ожидаемый результат (`OP 18..22` & Portal/XShm) | Статус |
|---|---|---|---|---|
| `STREAM-01` | Выбор вкладки «Экраны» (`Monitors`) и «Окна приложений» (`Windows`) | 1. В активном звонке нажать «Продемонстрировать свой экран».<br>2. Переключить вкладки `Monitors` и `Windows`. | Модальное окно `ScreenShareModal` фильтрует источники по `is_screen` без дубликатов. | `[x]` |
| `STREAM-02` | Пресеты качества `720p` (`1280×720`) и `1080p` (`1920×1080`), `30`/`60` FPS | 1. Запустить стрим экрана в `720p` @ `30 FPS`.<br>2. Проверить согласованные параметры RTP (`90 000 Hz`). | Устанавливается `is_screen_sharing = true`, разрешение `1280×720`, частота `30 FPS`. | `[x]` |
| `STREAM-03` | Горячее переключение источника и пресета (`720p` → `1080p` 60 FPS) в процессе стрима | 1. Не останавливая стрим, открыть выбор источника.<br>2. Переключиться с монитора `screen:0` на окно `{uuid}` и пресет `1080p` @ `60 FPS`. | Источник и пресет меняются на лету (`1920×1080 @ 60 FPS`), голосовая сессия `VoiceSessionState` не прерывается. | `[x]` |

---

## 2. Кросс-платформенная матрица тестирования Linux Desktop (`SUB-908-2`)

Клиент `discord-gpui` проходит обязательную сертификацию на 4 эталонных графических стеках Linux:

| Окружение (`DesktopEnvironmentKind`) | Сессия и переменные | Системный трей и уведомления | Захват экрана и масштабирование | Ключевые инварианты проверки |
|---|---|---|---|---|
| **1. GNOME 46/47 Wayland** (`Шаг 2.1`) | `XDG_SESSION_TYPE=wayland`<br>`XDG_CURRENT_DESKTOP=GNOME`<br>`WAYLAND_DISPLAY=wayland-0` | Трей `AppIndicator` (`org.kde.StatusNotifierWatcher`) + нативные D-Bus уведомления `org.freedesktop.Notifications` | `xdg-desktop-portal-gnome` (`org.freedesktop.portal.ScreenCast` + PipeWire) | Иконка в трее отображает статус (Online/DND/Unread), клик по уведомлению фокусирует окно. |
| **2. KDE Plasma 6 Wayland** (`Шаг 2.2`) | `XDG_SESSION_TYPE=wayland`<br>`XDG_CURRENT_DESKTOP=KDE`<br>`WAYLAND_DISPLAY=wayland-0` | Нативный StatusNotifierItem (`org.kde.StatusNotifierWatcher`) + `org.freedesktop.Notifications` | PipeWire portal + `org.kde.KWin.ScreenShot2`; **Fractional Scaling `125%` и `150%`** | При масштабе `125%` (`1600×900` физ.) и `150%` (`1920×1080` физ.) нет мыла, наложений текста и сдвига сетки `72/240/flex-1/240`. |
| **3. Hyprland / Sway** (`Шаг 2.3`) | `XDG_SESSION_TYPE=wayland`<br>`XDG_CURRENT_DESKTOP=Hyprland:sway`<br>`WAYLAND_DISPLAY=wayland-1` | Waybar SNI (`org.kde.StatusNotifierWatcher`) + `mako`/`dunst` (`org.freedesktop.Notifications`) | `xdg-desktop-portal-hyprland` / `xdg-desktop-portal-wlr` (PipeWire) | При динамическом ресайзе тайлов от `900×600` до `3840×2160` нижняя панель `UserPanel` (52px) не выталкивается, `min_h_0`/`min_w_0` сохраняют скролл. |
| **4. X11 (`Xfce` / `i3`)** (`Шаг 2.4`) | `XDG_SESSION_TYPE=x11`<br>`DISPLAY=:0`<br>`WAYLAND_DISPLAY` отсутствует | Системный трей XEmbed / StatusNotifier + `xfce4-notifyd` / `dunst` | Автоматический fallback на **X11 MIT-SHM (`XShmGetImage`, `kFullDesktopScreenId = -1`)** | При отсутствии Wayland-сокета захват экрана бесшовно переключается на X11 SHM без паники и без черного экрана. |

---

## 3. Сценарии исследовательского тестирования — Exploratory Testing Charters (`SUB-908-3`)

### 3.1. Шаг 3.1: Сценарий стрессового ввода (`StressInputAnalyzer`)
- **Цель:** Проверить устойчивость композера, парсера Markdown и шейпинга шрифтов GPUI к экстремальным входным данным.
- **Тестовый набор:**
  1. Граничная длина: сообщение ровно в **`2000 символов`** (проходит) и `2001` символ (отклоняется).
  2. Спецсимволы Unicode: составные эмодзи с Zero-Width Joiner (`👨‍👩‍👧‍👦`, `🏳️‍🌈`), Zalgo-диакритика, суррогатные пары, CJK-иероглифы.
  3. Двунаправленный **`RTL`-текст**: смешение русского/английского текста с арабским (`مرحبا`) и ивритом (`שלום`), а также управляющими символами `U+202E` (Right-to-Left Override). Проверяется принудительная изоляция `U+2068` (FSI) … `U+2069` (PDI), чтобы `RTL`-вставка не разворачивала верстку соседних элементов.
  4. Битые Markdown теги: незакрытые блоки кода ```` ```rust ````, непарные `**жирный`, `||спойлер`, вложенные цитаты `>>>`. Парсер обязан сбалансировать AST без паники и зависания потока рендеринга.

### 3.2. Шаг 3.2: Сценарий быстрого переключения серверов и каналов — Monkey Testing (`10 кликов в секунду`)
- **Цель:** Выявить гонки асинхронных задач Tokio ↔ GPUI, утечки памяти и потерю черновиков при высокочастотной навигации.
- **Профиль нагрузки:** Серия из `>= 30` переключений между серверами в `ServerRail` и каналами в `ChannelSidebar` со скоростью **`10 кликов в секунду`** (интервал `100 мс`).
- **Критерии прохождения:**
  - Время отрисовки кадра строго **`< 8.33 мс`** (120 FPS).
  - Потребление памяти `VmRSS < 60 МБ` (отсутствие утечек текстур и подписок).
  - Набранный в поле ввода черновик (`draft_text`) полностью сохраняется.

### 3.3. Шаг 3.3: Сценарий работы с нестабильным интернетом — обрыв кабеля на `30 секунд`
- **Цель:** Проверить надежность конечного автомата переподключения шлюза (`Opcode 6 RESUME`).
- **Шаги сценария:**
  1. Установить активную сессию (`READY`, запомнить `session_id` и `last_seq`) и набрать черновик сообщения в композере.
  2. Физически отключить сетевой интерфейс / кабель (`ip link set eth0 down` или `nmcli networking off`) ровно на **`30 секунд`**.
  3. Включить сеть обратно через `30 секунд`.
- **Критерии прохождения:**
  - Клиент автоматически выполняет рукопожатие **`Opcode 6 RESUME`** (с передачей `session_id` и `seq`), не требуя повторного ввода пароля.
  - Все пропущенные за `30 секунд` сообщения догружаются строго по порядку без единого дубликата (`duplicate_messages_count == 0`).
  - Неотправленный черновик в поле ввода сохраняется.

### 3.4. Шаг 3.4: Регламент предрелизных `Bug-bash` сессий и фиксация дефектов (`P0–P3`)

Перед каждым выпуском кандидата в релиз (`testing -> beta`) отдел `G4 QA` проводит синхронную **`Bug-bash`** сессию длительностью 60 минут с участием представителей `G1 Backend`, `G2 Frontend`, `G5 Voice & Platform` и `PM`.

#### Этапы проведения `Bug-bash` сессии:
1. **Инициализация (`checklist ready`):** Координатор фиксирует хэш бинарника `discord-gpui` из `SHA256SUMS` и распределяет 4 окружения Linux (`GNOME 46/47`, `KDE Plasma 6`, `Hyprland / Sway`, `X11`) между участниками.
2. **Активная фаза (`running bug-bash`):** Выполнение Smoke/Sanity чек-листов (`SUB-908-1`) и исследовательских чартеров (`SUB-908-3`).
3. **Стандартизированная регистрация дефектов (`BugReport`):**
   Каждая карточка дефекта в баг-трекере обязана содержать:
   - Уникальный ID (`BUG-908-XX`), заголовок и окружение (`DesktopEnvironmentKind`).
   - Пошаговый сценарий воспроизведения (`reproduction_steps`, не менее 1 непустого шага).
   - Путь к скриншоту / видеозаписи артефакта (`screenshot_path`).
   - Стек вызовов **`GDB`** (`gdb -batch -ex "thread apply all bt full"`), прошедший автоматическую санитизацию секретов (`mask_secret_tokens` заменяет любые `mfa.<token>` на `mfa.[REDACTED]`).
   - Классификацию приоритета **`P0–P3`**:
     - **`P0` (Blocker):** Паника процесса (`panic!`/`SIGSEGV`), утечка токена, отказ входа, потеря данных или поломка шифрования DAVE E2EE. **Мгновенная блокировка релиза.**
     - **`P1` (Critical):** Не работает ключевая функция (отправка сообщений, подключение к голосу, шаринг экрана, сломана верстка при `125%`/`150%`). **Блокировка релиза.**
     - **`P2` (Minor):** Косметические или неблокирующие отклонения UX (сдвиг тултипа на 1px, задержка анимации) при наличии обходного пути.
     - **`P3` (Trivial):** Пожелания по улучшению и мелкие неточности локализации.
4. **Вердикт приёмки (`sign-off complete`):** Сборка получает статус `release_approved = true` и переходит в `SignOffComplete` **только при условии `P0 == 0` и `P1 == 0`**.
