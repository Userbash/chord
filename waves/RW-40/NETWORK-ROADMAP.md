# RW-40 NETWORK-ROADMAP — live-сеть, серверы, голос, видео

Дата: 2026-10-24. Автор: PM (DeepSeek). Статус: план на утверждение/исполнение.
Связанные документы: `docs/rust-client/PLAN.md` (P1, P4, P4.5, P5, P6),
`docs/re/voice-deep.md`, `waves/RW-40/ATTACH-RE.md`,
`waves/RW-40/TASKS.md`, `waves/RW-39/PM/DECISIONS.md`.

## 0. Короткий ответ

Да, направление в планах есть: в `PLAN.md` это фазы **P4 (голос)**, **P4.5 (DAVE/MLS)**,
**P5 (видео/стриминг)** и **P6 (платформа)**. Но в текущей волне RW-40 выполнялись
авторизация, gateway и текстовая продуктовая часть; **live-сеть, серверы, голос и видео
ещё не подключены к UI и не проверены на живом Discord**.

Сейчас правильно писать не «клиент умеет голос/видео», а так:

- есть **headless-примитивы** RTC-сигнализации, UDP, SRTP, RTP/RTCP, регионов, reconnect;
- **нет** собранной голосовой сессии, кодека Opus, устройств/PTT, голосового UI;
- **нет** видео-конвейера, захвата экрана/камеры, Go Live и зрителей;
- **нет** UI добавления/выхода с сервера;
- голос/видео **не проверялись** live-окном.

## 1. Что уже реализовано

| Слой | Что есть | Доказательство | Чего нет |
|---|---|---|---|
| Auth/session | login/password+MFA+captcha, token import, keyring, logout, `auth://state` | auth **43/43**, D-39…D-62 | live-проверки |
| Gateway WS | identify/resume/heartbeat, supervisor Ready/Stopped, `net_connect/status/disconnect` | core 574, gui 132, D-42…D-45 | live READY, полный dispatch в UI |
| REST text | send/edit/delete/reply, reactions, guild message search | D-50…D-65 | join/leave server, attachments, typing, presence, DMs, friends |
| State/history | `PageStore` ≤500, virtual list, истории/курсоры | model/bench tests | полная синхронизация гильдий |
| Voice primitives | RTC op0/1/2/4/8, endpoint/region, UDP discovery/keepalive, RTP/RTCP, SRTP KDF/AES-GCM/XChaCha, jitter, stats, reconnect policy | voice **141/0**; `docs/re/voice-deep.md` | реальная сессия, Opus, устройства, UI |
| Media | только каркас `discord-media` (константы Opus/AV1/H264/VP8) | 1 тест | захват, энкод/декод, RTP-видео, Go Live |
| Platform | D-Bus notifications есть в `discord-platform` | platform tests | подключение к UI/DND |
| UI | 4 экрана, чат, поиск, настройки, участники | frontend **212/0** | голосовая панель, меню сервера, стрим-UI |

## 2. Сетевые части: полная карта

### 2.1 Уже есть

1. **HTTP auth**: login, MFA, captcha, token import; 401/429/сеть типизированы.
2. **Gateway**: identify, heartbeat, resume, READY, dispatch/reducer, reconnect policy.
3. **REST сообщения**: отправка, правка, удаление, ответ, реакции, поиск по гильдии.
4. **Локальная модель**: сообщения/каналы/участники, страницы, виртуализация.
5. **Голосовые примитивы**: RTC-парсинг, регионы, UDP-discovery/keepalive, RTP/RTCP,
   SRTP-крипто, jitter/reception, reconnect-политика.

### 2.2 Отсутствует и требует кода

| # | Функция | Состояние | Контракт/источник |
|---|---|---|---|
| 1 | **Live READY** | не проверено | один live-прогон |
| 2 | **Live send** | не проверено | один live-прогон |
| 3 | **Вход на сервер по инвайту** | нет | `POST /invites/{code}`, payload — RE-gap |
| 4 | **Выход с сервера** | нет | `DELETE /users/@me/guilds/{id}` (`{lurking}`), `confirmed:false` |
| 5 | **Список серверов/переключение** | частично (демо-рейл) | REST + `GUILD_CREATE` |
| 6 | **Создание сервера** | нет | endpoint не подтверждён |
| 7 | **Инвайты: создать/посмотреть** | нет | `GET /guilds/{id}/invites`, `POST /channels/{id}/invites` |
| 8 | **Участники: полный поиск** | локально | `POST /guilds/{id}/members-search` (`confirmed:false`) |
| 9 | **Presence/typing** | reducer есть | `POST /channels/{id}/typing`, gateway events |
| 10 | **Вложения** | blocked | upload endpoint отсутствует в дампе |
| 11 | **DM/друзья** | нет | REST/ gateway, отдельная фаза |
| 12 | **Голос: вход/выход** | headless | RTC WS + VOICE_STATE_UPDATE |
| 13 | **UDP-медиа live** | примитивы | discovery/select/keepalive/rotation |
| 14 | **SRTP live** | примитивы | ключи из op4, nonce/tag/tail |
| 15 | **Opus encode/decode** | нет | libopus, 48 кГц, PT120, DTX/CN |
| 16 | **Устройства/PTT/mute/deafen** | нет | PipeWire/ALSA, 38 настроек |
| 17 | **Голосовой UI** | нет | панель, участники, ping, speaking |
| 18 | **DAVE/MLS E2EE** | нет | P45-01…03 |
| 19 | **Захват экрана/камеры** | нет | PipeWire/X11, v4l2 |
| 20 | **Видео-энкод/декод** | нет | AV1/H264, vaapi/vulkan |
| 21 | **RTP-видео/Go Live** | нет | streams/rid/quality, зрители |
| 22 | **Системный звук** | нет | PipeWire monitor |
| 23 | **Уведомления/DND** | backend есть | D-Bus + UI |
| 24 | **Трей/deep links/updater** | частично | P6 |

## 3. Приоритеты

### P0 — без этого нет live-клиента

- **N0-01 Live READY** (окно 1, тестовый аккаунт).
- **N0-02 Live send** (окно 1, один канал, один текст).
- **N0-03 Reconnect/offline state в UI** (resume/4013/4014, без сети в тестах).

### P1 — «клиент как Discord»: серверы и голос

- **N1-01** join/leave/list серверов (backend) — после RE-payload.
- **N1-02** меню сервера и модалка инвайта/выхода (UI).
- **N1-03** переключение гильдий/каналов, восстановление выбора.
- **N1-04** typing/presence в UI.
- **N2-01** RTC session manager (WS live).
- **N2-02** UDP media transport live.
- **N2-03** SRTP live.
- **N2-04** Opus pipeline.
- **N2-05** устройства/PTT/mute/deafen.
- **N2-06** голосовой UI (join/leave, участники, ping, speaking).

### P2 — устойчивость и расширение

- **N1-05** полный поиск участников (`members-search`, после подтверждения).
- **N1-06** вложения (после upload-контракта; сейчас blocked).
- **N2-07** voice stats/reconnect/шумоподавление/soundboard.
- **N2-08** security-hardening голоса.
- **N3-01…03** DAVE/MLS.
- **N3-04…07** screen capture → encode → RTP → Go Live.
- **N4-01** уведомления DND в UI.

### P3 — полировка

- **N3-08…11** камера, системный звук, декод зрителей, clips/quality.
- **N4-02…03** трей, автозапуск, deep links, updater.
- DM/друзья/полный social graph — отдельная волна после голоса.

## 4. Очередь задач по фазам

### N0 — live-сертификация (P0)

| ID | Задача | Владелец | DoD | Зависит |
|---|---|---|---|---|
| N0-01 | READY на тестовом аккаунте | PM+G4+G1 | `READY` в логе, окно 1, evidence, без токена | тестовый токен + окно |
| N0-02 | live send | PM+G4+G1 | сообщение видно в канале, 1 окно, скрин/лог | N0-01 |
| N0-03 | reconnect/offline UI | G1+G2 | `Reconnecting/Stopped`, resume, 4013/4014 | N0-01 |

### N1 — серверы и социальные функции (P1)

| ID | Задача | Владелец | DoD | Зависит |
|---|---|---|---|---|
| N1-01 | REST join/leave/list | G1 | `PUT /guilds/{id}/members/@me?lurker=true` 204 ([L]); `POST /invites/{code}` ([S]); `DELETE /users/@me/guilds/{id}` ([S]); список из `READY`; 401/403/404/429; invite-коды — `[A]` | RE/live |
| N1-02 | меню сервера | G3+G2 | список, «Присоединиться», инвайт, «Выйти» с confirm, ошибки | N1-01, дизайн |
| N1-03 | переключение гильдий | G2 | выбор гильдии/канала, восстановление после reload | N1-01 |
| N1-04 | typing/presence | G1+G2 | индикатор набора, статусы, 401/429, таймаут | N0-01 |
| N1-05 | members-search | G1+G2 | полный поиск после подтверждения endpoint; иначе deferred | RE |
| N1-06 | вложения | G1+G2 | upload/multipart/прогресс/превью | upload-контракт (blocked) |
| N1-07 | DM/друзья | G1+G2 | список, открыть DM, отправка; отдельная волна | контракты |

### N2 — голос MVP (P1)

| ID | Задача | Владелец | DoD | Зависит |
|---|---|---|---|---|
| N2-01 | RTC session manager | G1 | WS connect, op8→op0→op2→op4, heartbeat 13.75 с, close 1000 | live voice окно |
| N2-02 | UDP transport | G1 | discovery 74 Б, select protocol, keepalive 5 с, rotation +60 с | N2-01 |
| N2-03 | SRTP live | G1+G4b | ключи из op4, encrypt/decrypt, replay/tag/tail, нет plaintext | N2-02 |
| N2-04 | Opus pipeline | G5+G1 | 48 кГц, PT120, jitter, DTX/CN, loopback-тест | N2-03 |
| N2-05 | devices/PTT | G5 | выбор in/out, hot-plug, PTT, mute/deafen, 38 настроек | N2-04 |
| N2-06 | voice UI | G2 | join/leave, участники, speaking, ping, устройства; 1 live-окно | N2-05, дизайн |
| N2-07 | robustness | G1+G5 | reconnect, смена устройств, stats, soundboard | N2-06 |
| N2-08 | security | G4b | токен/ключи не в логах, zeroize, permission prompts | N2-06 |

### N3 — DAVE и видео/стриминг (P2)

| ID | Задача | Владелец | DoD | Зависит |
|---|---|---|---|---|
| N3-01 | DAVE framing | G1 | Welcome/Commit/App, пороги | N2-04 |
| N3-02 | openmls | G1 | групповые ключи, ротация | N3-01 |
| N3-03 | interop | G4 | два клиента слышат друг друга под E2EE | N3-02 |
| N3-04 | screen capture | G5 | PipeWire/X11, окно/монитор/регион, preview, consent | P0 |
| N3-05 | video encode | G5 | AV1/H264, keyframe/bitrate, vaapi/vulkan | N3-04 |
| N3-06 | RTP video | G1 | rid/quality, simulcast, rtx | N3-05, N2-03 |
| N3-07 | Go Live | G1+G2 | streams, зрители, качество, stop | N3-06 |
| N3-08 | camera | G5 | v4l2, preview, mic separate | N3-05 |
| N3-09 | system audio | G5 | PipeWire monitor | N3-04 |
| N3-10 | viewer decode | G2+G5 | тайлы участников, звук, fullscreen | N3-07 |
| N3-11 | soundboard/clips | G5+G2 | sounds, playback, clips | N2-04 |

### N4 — платформа (P2/P3)

| ID | Задача | Владелец | DoD |
|---|---|---|---|
| N4-01 | notifications/DND | G5+G2 | D-Bus, mention, mute, click→канал |
| N4-02 | tray/autostart/badge | G5 | закрытие в трей, XDG autostart |
| N4-03 | deep links/updater | G5 | `discord://`, single-instance, update check |

## 5. Очередь по отделам

| Отдел | Очередь |
|---|---|
| **G1 Core/Network** | N0-01 → N0-02 → N0-03 → N1-01 → N1-04 → N2-01 → N2-02 → N2-03 → N2-07 → N3-01 → N3-02 → N3-06 → N3-07 |
| **G2 Product UI** | N1-02 → N1-03 → N2-06 → N1-04 (UI) → N4-01 → N3-10 → N1-07 |
| **G3 Design** | `DESIGN/SERVER-MENU.md` → `DESIGN/VOICE.md` → `DESIGN/STREAM.md` → `DESIGN/NOTIFICATIONS.md` (параллельно, до кода) |
| **G4 QA** | live-харнесс N0-01/02 → voice loopback → screen-capture fixture → QA-матрица N1/N2/N3 → e2e без live |
| **G4b Security** | SRTP/DAVE review → token/key logs → screen/voice permissions → no hidden capture |
| **G5 Platform** | N2-05 → N3-04 → N3-05 → N3-08 → N3-09 → N3-11 → N4-01/02/03 |
| **PM** | окна live, бюджет 1 окно/приёмка, приоритеты, RE-гейты, merge |

Правило параллелизма: **≤2 кодовых воркера**, один `cargo` на target-dir; live-окно — только
по согласованию и один раз на приёмку.

## 6. Дизайн: что нужно до кода

| Артефакт | Содержание | Блокирует |
|---|---|---|
| `waves/RW-40/DESIGN/SERVER-MENU.md` | список серверов, «Присоединиться» (инвайт), «Выйти» (confirm), ошибки 401/403/404/429, права, пустые состояния | N1-02 |
| `waves/RW-40/DESIGN/VOICE.md` | панель join/leave, участники, speaking, mute/deafen, ping, устройства, PTT, 38 настроек, состояния reconnect | N2-06 |
| `waves/RW-40/DESIGN/STREAM.md` | Go Live, выбор экрана/окна, preview, качество, камера, зрители, stop, consent | N3-07/10 |
| `waves/RW-40/DESIGN/NOTIFICATIONS.md` | DND, mention, click→канал, настройки | N4-01 |

Дизайн обязан отделить **[T]** (из дампа), **[M]** (модель/контракт) и **[A]** (авторское
решение), как в `DESIGN-MSG.md`; ассеты не копировать.

## 7. RE-гейты и блокеры

До кода обязательны подтверждения. Уровни: **[L]** live, **[S]** call-site в коде эталона,
**[D]** только Enum-запись дампа, **[M]** модель-инференс, **[A]** нет подтверждения.
Поле `confirmed` из `rest-static.json` **не использовать**: оно `false` у всех 904 записей.

1. **Join в сервер** — уже подтверждён live: `PUT /guilds/{id}/members/@me?lurker=true`,
   тело `{}`, ответ **204** (`[L]`, `D02:101,138`). Это первый шаг N1-01.
2. **Join по инвайту** — `POST /invites/{code}` с `{session_id, invite_instance_id,
   received_installation_id}` (`[S]`); HTTP-коды/TTL/ошибки — `[A]`, нужен live capture.
3. **Guild leave** — `DELETE /users/@me/guilds/{id}` + `{lurking:bool}` (`[S]`); response
   не читается, коды и семантика lurker/guest — `[A]`. Минимум фикстуры 204/403.
4. **Список серверов** — REST-ручки нет; источник `READY.guilds[]` + `READY_SUPPLEMENTAL`
   (`[S]+[L]`); `GET /users/@me/guilds` не выдумывать.
5. **Guild create** — endpoint не подтверждён (`[A]`); не кодировать.
6. **Attachments upload** — endpoint/multipart/лимиты отсутствуют → B2 blocked.
7. **members-search** — `POST /guilds/{id}/members-search` есть (`[S]`), 202 `retry_after`;
   пагинация запроса неизвестна (`[A]`).
8. **Voice live** — рукопожатие/кодеки/UDP/SRTP-конверт подтверждены (`[L]/[S]`), но
   входящее медиа, расшифровка (AAD/nonce), триггер ротации, op5/11/13 — gap (`[A]`).
9. **Video/Go Live** — rid/ssrc/quality/AV1 частично (`[L]/[S]`), события `STREAM_*`,
   enum quality и Linux screen previews — gap (`[A]`).
10. **DAVE/MLS** — op21/22/24/25/27/29/30 и mlspp видны (`[S]`); пороги/epoch/2-клиентный
    тест — `[A]`. Без live-сессии не объявлять E2EE готовым.

## 8. MCP и скиллы: что подключено и что делает

### MCP (`.opencode/opencode.json`)

| MCP | Статус | Назначение | Использование |
|---|---|---|---|
| `rea` | доступен | REA: RE нативных/JS/Electron/браузерных артефактов, строки, decompile, evidence | **используется** для дампа `/home/sanya/re-deep`, voice/gateway |
| `basic-memory` | **healthy** (0.23.2, sqlite) | долговременные заметки/контекст проекта | диагностика; основной источник — файлы `waves/` |
| `context7` | доступен | актуальная документация библиотек (Tauri, React, Rust-крейты) | по необходимости; без секретов |
| `github` | доступен | read-only metadata репозитория/PR/issues | не пишет код; коммиты — координатор |
| `socraticode` | **DEGRADED** | семантический поиск/граф кода | **не используется**: Docker/Qdrant отсутствует; fallback — `rg` |

### Скиллы

| Скилл | Что делает | Статус |
|---|---|---|
| `discord-lab-context` | карта репозитория, что читать первым | активен |
| `discord-lab-process` | волны, роли, гейты, live-окна | активен |
| `discord-lab-worker` | strict TDD, только свои файлы, отчёт | активен |
| `discord-lab-review` | приёмка, границы, безопасность, red-first | активен |
| `reverse-engineer-anything` | REA-воркфлоу для дампа/бинарей | активен для RE-гейтов |
| `feature-change-workflow` | крупные изменения API/архитектуры | для N1/N2 |
| `shared-agent-memory` | чтение/запись Basic Memory | опционально |
| `codebase-exploration` / `codebase-management` | SocratiCode | **недоступны** без Docker |
| `orchestration` / `orca-cli` / `computer-use` | воркеры, терминалы, GUI | только для оркестрации, не продукт |
| `token-efficiency` | экономия контекста | применяется |
| game/music/tts-скиллы | не относятся к Discord | не используются |

## 9. Критерии приёмки live-функций

1. **Live READY**: `READY` + guild/channel в логе, токен не в логах, окно 1.
2. **Live send**: сообщение в канале, optimistic→ok, 401/429 обработаны.
3. **Join/leave**: вход по инвайту и выход подтверждены на тестовом аккаунте; список серверов
   обновился; ошибки видимы.
4. **Voice**: два тестовых клиента (или второй участник с согласия) слышат друг друга;
   mute/deafen/PTT работают; reconnect; ключи не в логах.
5. **Video/Go Live**: свой экран/камера, consent, stop; зритель видит поток; без скрытого захвата.
6. **Security**: P0/P1=0, нет токенов/паролей/ключей в логах, permissions prompts.
7. **Quality**: targeted + gate + `cargo test --workspace`; live-окно ровно 1 на приёмку.

## 10. Немедленные следующие шаги

1. Утвердить этот план; заморозить N0/N1 контракты по `NETWORK-CONTRACTS.md`.
2. **N1-01a** — `PUT /guilds/{id}/members/@me?lurker=true` (204, [L]) как первый безопасный
   backend-шаг; **N1-01b** — invite join ([S]) только после live-кодов; **N1-01c** — leave ([S]).
3. Подготовить live-харнесс N0-01/N0-02 (тестовый аккаунт, окно, evidence).
4. Дизайн: `SERVER-MENU.md` → `VOICE.md` (до кода UI); параллельно — design-parity аудит
   (`PARITY/G6-CHARTER.md` §13, DP-01…DP-05).
5. После N1-01: N1-02/N1-03; параллельно N2-01/N2-02 (headless→live).
6. Не начинать видео/DAVE до зелёного voice MVP: иначе распыление и ложные «готово».

## 11. Что не делаем

- не обходим captcha/anti-detect/fingerprint/прокси-ротацию;
- не делаем массовые аккаунты, спам, скрытый захват;
- не пишем endpoint «по догадке» без `[L]`/`[S]`/`[D]` и RE-проверки;
- не объявляем функцию готовой по headless-парсеру без live-сессии;
- не трогаем `logs/`, `functions/`, `evidence/`, `/home/sanya/re-deep` на запись.
