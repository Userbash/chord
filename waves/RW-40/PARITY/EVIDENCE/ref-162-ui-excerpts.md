# E1/E3 — выдержки из дампа 1.0.162 (live CDP + статика web-бандла)

Дамп снят с изолированной копии Discord 1.0.162; **не** живой аккаунт этой сессии.
Каждый артефакт — read-only, хеши в `hashes.txt`. Это `version-gap` к установленному 1.0.166:
UI-функции подтверждены на 1.0.162, IPC/asar — на 1.0.166.

## 1. Меню сервера (live DOM, `/home/sanya/re-deep/ui2/D1-guild-menu-items.json`)

10 пунктов, порядок дампа:
`Пометить как прочитанное` · `Пригласить на сервер` · `Заглушить сервер` ·
`Параметры уведомлений Только @упоминания` · `Скрыть заглушённые каналы` ·
`Настройки конфиденциальности` · `Редактировать личный профиль сервера` · `Переместить...` ·
`Покинуть сервер` · `Копировать ID сервера`.

Подтверждает: invite, mute, per-guild notification level, privacy, leave, copy ID.
Не подтверждает: точные endpoint/тела и модалку подтверждения выхода (RE-gap).

## 2. Каналы (live DOM, `/home/sanya/re-deep/ui6/01-guild-channels.json`)

`href=/channels/<guild>/<channel>`, `aria="💬welcome (текстовый канал)"`,
`"🔔афиша-стримов📺 (текстовый канал)"`, `"не прочитано, 🔤болталка (текстовый канал)"`,
`"💾dpakc_help🖥💻 (канал форума)"`.
Подтверждает: дерево каналов, типы text/forum, unread-маркер в доступном имени.

## 3. Голосовая панель (live DOM, `/home/sanya/re-deep/voice/ui/`)

`01-panel-text.json`:
```
93 ms · О голосовом звонке · Голосовая связь подключена ·
📞┃тестируем твики / ITSKIDON PC BOOST · Камера недоступна ·
Продемонстрируйте свой экран · Начать активность · ShadowByte pharos12510 Невидимый · Вкл. микрофон
```
`01-voice-panel.html` aria-label: `Заглушить`, `Откл. звук`, `Отключиться`,
`Настройки ввода`, `Настройки вывода`, `Настройки пользователя`,
`Управление профилем и статусом`, `Открыть звуковую панель`,
`Шумоподавление с помощью Krisp`, скрытая подпись `Продемонстрируйте свой экран`.
`01-candidates.json` — участник `ShadowByte` + текст `Этот микрофон отключён`,
кнопка `Пригласить в голосовой чат`.

Подтверждает: join/leave, ping, участники, mute/deafen, speaking/settings, screen share
affordance, camera disabled state, activities. Не подтверждает: Go Live-поток и зрителей.

## 4. Строки i18n-каталога (`/home/sanya/re-deep/ru-catalog.json`, 23 836 ключей)

| Ключ | Значение | Домен |
|---|---|---|
| `tYGJRO` | `Печатает…` | typing |
| `lJ9sZX` | `[[arg]] печатает...` | typing |
| `gH3Frd` | `Не беспокоить` | DND/status |
| `gYEF6m` | `Режим концентрации («Не беспокоить») включён` | DND |
| `l5WIbf`, `H9jxS1` | `Добавить сервер` | guilds |
| `HvoZQD` | `Пригласить на серверы` | invite |
| `fjBNo1` | `Продемонстрируйте свой экран` | Go Live/screen |
| `NRRxmz` | `Загрузить файл (` + types + `)` | attachments |
| `IJyOUf` | `Перетащите и отпустите или щёлкните, чтобы загрузить файл.` | attachments |
| `HVxmOD` | `Прикрепить вложения` | attachments |
| `K2sFfo` | `Светлая` | themes |
| `J/FISW` | `Включить компактный режим` | density |
| `JfHzXG` | `Отсканируйте этот QR-код ... чтобы мгновенно войти` | QR login |
| `gTH4Dp` | `Использовать ключ доступа или ключ безопасности` | passkey |

## 5. Настройки (live DOM, `docs/re/frontend-d03.md:63-83`)

- 63 строки навигации, включая «Учётная запись», «Уведомления», «Голос и видео»,
  «Внешний вид», «Язык и время», «Расширенные».
- Вкладка «Голос и видео»: 38 переключателей, микрофон/динамик/камера,
  профиль ввода (3 радио), шумоподавление Krisp, ползунки, кнопки проверки.
- Токены/классы — там же §5–6.

## 6. Шлюз (live capture, 1.0.162)

- `docs/re/gateway-protocol.md:152-154`: `PRESENCE_UPDATE 107`, `TYPING_START 28`,
  `VOICE_STATE_UPDATE 8`, `VOICE_SERVER_UPDATE 3`, `VOICE_CHANNEL_START_TIME_UPDATE 7`,
  `SOUNDBOARD_SOUNDS 21`, `MESSAGE_CREATE 31`, `MESSAGE_UPDATE 24`, `MESSAGE_ACK 8`.
- `docs/re/gateway-schema.md:76`: статусы `online 312`, `idle 116`, `dnd 92`, `invisible 2`.
- `docs/re/gateway-schema.md:397`: в READY нет `presences`; статусы — в
  `READY_SUPPLEMENTAL.merged_presences` и `PRESENCE_UPDATE`.
- `docs/re/gateway-schema.md:433-438`: `MESSAGE_CREATE` — 19 ключей, `attachments` есть,
  но `null` во всех 10 событиях; схему вложений по дампу строить нельзя.
- `STREAM_CREATE/UPDATE/SERVER_UPDATE` в дампе **не наблюдались** (Go Live — RE-gap).

## 7. REST: подтверждённые live-вызовы (`docs/re/rest-d02.md:63-141`)

| Метод | Путь | Коды | Что подтверждает |
|---|---|---|---|
| GET | `/channels/{id}/messages` | 200×11 | история |
| POST | `/channels/{id}/messages/{id}/ack` | 200×5 | read state, тело `{token,last_viewed}` |
| PUT | `/channels/{id}/messages/{id}/reactions/%F0%9F%91%8B/%40me` | 204 | реакции |
| GET | `/channels/{id}/messages/pins` | 200 | пины |
| GET | `/channels/{id}/threads/search` | 200 | ветки |
| GET/PATCH | `/users/@me/settings-proto/{num}` | 200 | настройки (PATCH — non-json) |
| GET | `/guilds/{id}/messages/search` | — | серверный поиск (D-03) |

## 8. REST: статическая карта web-бандла (`/home/sanya/re-deep/rest-static.md`)

| Строка | Метод | Путь | Тело/примечание | level |
|---|---|---|---|---|
| 172 | POST | `/auth/login` | `{login,password,undelete,login_source,gift_code_sku_id}` | [S] |
| 175 | POST | `/auth/mfa/{arg}` | `{code,ticket,login_source,gift_code_sku_id,login_instance_id}` | [S] |
| 176 | POST | `/auth/one-time-login` | QR/one-time | [S] |
| 166-167 | POST | `/auth/conditional/start`, `/finish` | WebAuthn | [S] |
| 181-182 | GET/POST | `/auth/sessions`, `/auth/sessions/logout` | сессии | [S] |
| 454 | DELETE/GET/POST | `/invites/{arg}` | POST body `{session_id,invite_instance_id,received_installation_id}` | [S] |
| 657 | DELETE | `/users/@me/guilds/{arg}` | body `{lurking}` | [S] |
| 371 | PUT | `/guilds/{arg}/members/@me` | guild join (live D-02: 204, пустое тело) | [L] |
| 220 | GET/POST | `/channels/{arg}/invites` | создать/список инвайтов | [S] |
| 367 | GET | `/guilds/{arg}/invites` | список инвайтов | [S] |
| 253 | POST | `/channels/{arg}/typing` | typing | [S] |
| 370 | POST | `/guilds/{arg}/members-search` | 202 `{retry_after}` | [S] |
| 817 | — | `/channels/{arg}/attachments` | `MESSAGE_CREATE_ATTACHMENT_UPLOAD`, метод не определён, call-site 0 | [D] |
| 705 | GET/PATCH | `/users/@me/settings-proto/{arg}` | настройки (live GET/PATCH — [L]) | [S] |
| 680 | PATCH | `/users/@me/notification-settings` | уведомления | [S] |
| 544-546 | POST/GET/PATCH | `/streams/{arg}/notify\|preview\|stream` | activity streams, **не** Go Live | [S] |

## 9. i18n (`docs/re/i18n-deep.md:14,205`)

- `ru-catalog.json` — 23 836 ключей / 2 443 841 Б; `en-catalog.json` — 23 903.
- Включённых локалей — 31 из 36; fallback → `en-US`.
- Собственный рантайм (`FormatBuilder`, не ICU), ключ = `xxh64 LE → base64[0..6]` (36 бит),
  CLDR-плурализация, 23 правила rich-разметки.

## 10. Deep links / tray / updater (1.0.162, `docs/re/main-arch.md`)

- §9.4 (841-860): `extractUrlFromArgs`, `open-url`, `second-instance`, `openOrQueueUrl`,
  `discord://`, `disclip://`; `setAsDefaultProtocolClient` в asar нет.
- §10.1 (861-869): main не грузит `discord_voice`; трей-команды mute/deafen/voice-settings.
- §5.4 (579-591): updater, `pinned_update.json`, `USE_PINNED_UPDATE_MANIFEST`.
- §11 (892+): `NOTIFICATIONS_SEND_NOTIFICATION` → native `discord_notifications`;
  `discord_clips`; `WEBAUTHN_*`.
