# RW-40 — TZ: от спайка к рабочему клиенту (функции, приоритеты, данные)

Дата: 2026-10-24. PM: DeepSeek. Основание: `DESIGN/FUNCTION-MATRIX.md`,
`NET/NET-FUNCTIONS.md`, `NET/CORE-AUDIT.md`, `NET/PROGRAM-AUTH.md`, `PLAN.md`, `QA/PLAN-E9.md`.
Предусловие: завершить RW-39/auth (W5 41/41 + tsc 0, NET-F1b, NET-E2).

## 1. Цель

Собрать из GUI-спайка функциональный клиент: вход → сессия → шлюз → данные → действия
пользователя, с продуктовыми экранами и проверяемыми кнопками. Без обхода ToS/анти-детекта.

## 2. Инвентарь функций

Статусы: `done` — принято; `partial` — есть, но не всё; `missing` — нет; `broken` — красное.
Приоритет: P0 (без этого нет клиента), P1 (ядро продукта), P2 (расширение).

### 2.1 Вход и сеть

| ID | Функция | Статус | P | Владелец | Приёмка |
|---|---|---|---|---|---|
| A-01 | Импорт токена + keyring | done | P0 | Backend | 95 тестов, keyring |
| A-02 | Пароль + MFA + SMS | done | P0 | Backend | 95 тестов |
| A-03 | Окно входа по LOGIN-SPEC | **broken** (40/41, tsc 52) | P0 | Frontend | 41/41, tsc 0 |
| A-04 | Captcha: поля session/rqdata, headers | missing | P0 | Backend | NET-F1b |
| A-05 | Connect/READY после входа | missing | P0 | Network | NET-E2 |
| A-06 | `auth://state` события | missing | P1 | Backend | events + UI |
| A-07 | Logout → stop supervisor | missing | P1 | Backend/Network | тест + live |
| A-08 | QR-вход | missing | P2 | Network/RE | протокол |
| A-09 | WebAuthn | missing | P2 | Frontend/Core | нативный |
| A-10 | Live `/auth/*` + keyring | missing | P0 | QA | согласованное окно |
| A-11 | 429 retry_after в UI | partial | P1 | Frontend | таймер |
| A-12 | Keyring prompt timeout | missing | P1 | Backend | тест |

### 2.2 Чат и сообщения

| ID | Функция | Статус | P | Владелец |
|---|---|---|---|---|
| C-01 | Виртуализация 100k, высоты 26/48/+22 | done | P0 | Frontend |
| C-02 | Группировка, day/unread, reply, embeds, реакции (вид) | done | P1 | Frontend |
| C-03 | Отправка сообщений (REST) | missing | P0 | Network/Backend |
| C-04 | Оптимистичная отправка + ошибки | missing | P0 | Frontend |
| C-05 | Вложения: upload/прогресс/превью | missing | P1 | Frontend/Network |
| C-06 | Реакции: поставить/снять/счётчики | missing | P1 | Frontend/Network |
| C-07 | Меню сообщения: правка/удаление/копия/ответ | missing | P1 | Frontend |
| C-08 | Треды/reply-панель | missing | P2 | Frontend/Network |
| C-09 | Пины | missing | P2 | Frontend |
| C-10 | Реальный поиск по серверу | missing | P1 | Network/Frontend |
| C-11 | Day divider по времени (7 мин) | partial | P2 | Core/Frontend |

### 2.3 Участники, профиль, навигация

| ID | Функция | Статус | P | Владелец |
|---|---|---|---|---|
| M-01 | Виртуализация 24 229, панель/экран | done | P0 | Frontend |
| M-02 | Группы по ролям/статусу, полный поиск | partial | P1 | Frontend/Backend |
| M-03 | Profile-popout | missing | P2 | Frontend |
| M-04 | QuickSwitcher Ctrl+K | missing | P1 | Frontend |
| M-05 | DM-список | missing | P2 | Frontend/Network |
| M-06 | Меню гильдии | missing | P2 | Frontend |

### 2.4 Настройки, темы, i18n, звуки

| ID | Функция | Статус | P | Владелец |
|---|---|---|---|---|
| S-01 | 18 разделов, 38 тумблеров (вид) | done | P1 | Frontend |
| S-02 | Сохранение настроек в store | missing | P1 | Backend/Core |
| S-03 | Реальные разделы (appearance/notifications) | partial | P2 | Frontend |
| S-04 | Темы light/cozy | missing | P2 | Design/Frontend |
| S-05 | i18n-рантайм en/ru | missing | P2 | Core/Frontend |
| S-06 | Звуки событий | missing | P2 | Frontend/Media |
| S-07 | Полные шорткаты | partial | P2 | Frontend |

### 2.5 Голос, медиа, ОС

| ID | Функция | Статус | P | Владелец |
|---|---|---|---|---|
| V-01 | Голосовой UI: устройства, PTT, mute/deafen | missing | P2 | Frontend/Platform |
| V-02 | Статистика RTT/потери | missing | P2 | Frontend/Voice |
| V-03 | Уведомления (D-Bus) | partial (headless) | P1 | Platform/Frontend |
| V-04 | Трей, бейджи, автозапуск, deep links | partial (headless) | P2 | Platform/Frontend |
| V-05 | Updater UI | partial (headless) | P2 | Updater/Frontend |

## 3. Данные для реализации

- `DESIGN/FUNCTION-MATRIX.md` — UI → Tauri → BFF → core.
- `NET/NET-FUNCTIONS.md` — сетевые функции/endpoints.
- `NET/CORE-AUDIT.md` — 16 gaps ядра.
- `NET/AUTH-CONTRACT.md` — контракт входа.
- `QA/PLAN-E9.md` — матрица проверок кнопок.
- `docs/rust-client/PLAN.md` — P3/P4/P5 карточки.

## 4. Критерий готовности продукта

- вход (токен/пароль+MFA+captcha) → keyring → `/users/@me` → READY;
- отправка/получение сообщений, участники, поиск, настройки — на живых данных;
- голос/ОС — по статусу P2, без блокировки P0;
- 0 мёртвых кнопок; P0/P1=0; QA-матрица пройдена; live-прогон записан.
