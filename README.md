# Discord Function Lab

Локальная лаборатория для разбора клиента Discord на Linux: **данные по функциям**
(символьные таблицы и рантайм-инвентарь), **логи вызовов** и статические Evidence-дампы REA.

Всё локально, ничего никуда не отправляется. Discord здесь — цель анализа:
это ваш собственный установленный клиент, инструментирование выполняется только
по вашей команде.

## Что уже собрано

| Каталог | Содержимое |
| --- | --- |
| `evidence/js-app-graph.json` | REA-граф JavaScript-приложения (app.asar): 976 модулей, 3208 findings, 504k AST-узлов |
| `evidence/asar-inventory.json` | Инвентарь содержимого app.asar |
| `functions/pid-*.json/.csv` | Рантайм-инвентарь по процессам: модули, символы функций, экспорты (адрес + смещение) |
| `functions/static/*.csv` | Полные таблицы символов из ELF-файлов всех `*.node` (без движка анализа): 115 158 функций |
| `functions/domains/*.csv|.tsv` | Классификация функций по доменам: webrtc, rtp-rtcp, rtt, jitter, latency, audio, video, camera, network, voice, krisp-noise |
| `functions/domains/REPORT.md` | Человекочитаемый отчёт: объёмы по доменам, модули, примеры имён |
| `logs/calls-*.jsonl` | Логи вызовов функций (Frida), по строке на вызов |

Актуальные цели и объёмы данных — в конце файла (раздел «Результаты»).

## Установленный клиент

- Пакет: `discord` (extra) → `1:1.0.162`, установлен через yay.
- Файлы приложения: `~/.config/discord/app-1.0.162/`
  - `resources/app.asar` (3.5 МБ) — загрузчик/бутстрап;
  - `modules/discord_desktop_core-1/discord_desktop_core/core.asar` — ядро;
  - `modules/<name>-1/<name>/<name>.node` — нативные модули:
    `discord_voice` (121 МБ), `discord_krisp` (60 МБ), `discord_utils` (19 МБ),
    `discord_game_utils` (11 МБ), `discord_erlpack` (7.4 МБ), `discord_zstd` (853 КБ),
    плюс `updater.node`.

## Быстрые команды

```bash
# CLI discord-lab (основной интерфейс; bin/discord-lab.mjs, после npm link — команда discord-lab)
node bin/discord-lab.mjs --help                # справка и список команд
node bin/discord-lab.mjs pids                  # процессы Discord, видимые Frida
node bin/discord-lab.mjs inventory --pid <PID> # инвентарь модулей/символов → functions/pid-*.json/.csv
node bin/discord-lab.mjs hook --pid <PID> --pattern 'ExecutePending|PollMetrics' \
  --limit 32 --duration-ms 60000 --arg-count 2 # запись вызовов → logs/calls-<PID>.jsonl
node bin/discord-lab.mjs summarize logs/calls-<PID>.jsonl   # сводка → <лог>.by-function.csv
node bin/discord-lab.mjs cdp targets           # CDP-цели (env CDP_ENDPOINT, по умолчанию :9222)

# статика, типы, тесты
bash   scripts/extract-static-functions.sh   # статические таблицы функций из ELF-файлов
bash   scripts/analyze.sh                    # статический разбор app.asar через REA
npm run typecheck                            # tsc: типы scripts/, src/ и tests/ (см. «Типизация»)
npm test                                     # тесты unit+meta+smoke+e2e (см. «Тесты (TDD)»; зелёные, e2e — skip-aware)

# UI-прогоны (CDP; нужен запуск discord --remote-debugging-port=9222):
node scripts/ui-walk-settings.mjs     # все разделы настроек
node scripts/ui-walk-settings2.mjs    # добивающий проход по пропущенным
node scripts/ui-walk-menus.mjs        # меню сервера/канала/почты
node scripts/ui-menu-extra.mjs        # оставшиеся пункты + голосовая панель
node scripts/cdp.mjs rclick "текст"   # доверенный правый клик (меню канала)
node scripts/cdp.mjs lclick "текст"   # доверенный левый клик по координатам

# легаси-логирование (паритет с CLI hook; выводится из эксплуатации в M6-WRAP-01)
node scripts/logger.mjs hook --pid <PID> --kind exports --module discord_utils.node
```

Полезные опции хука: `--module <имя>.node` (можно повторять), `--pattern REGEX`,
`--limit N` (максимум хуков), `--arg-count 0..6` (сколько указателей-аргументов писать),
`--duration-ms N` (0 = до Ctrl-C), `--out FILE`, `--spawn` (запустить Discord под Frida).

### Команды CLI `discord-lab` (M1–M2)

Точка входа — `bin/discord-lab.mjs` (bin-имя `discord-lab`; после `npm link` доступна как команда).
Коды выхода: `0` — успех; `1` — неизвестная команда/ошибка аргументов; `2` — недоступная среда
(Frida/CDP) или ещё не реализованная команда.

| Команда | Назначение | Опции | Результат |
| --- | --- | --- | --- |
| `pids` | процессы Discord, видимые Frida | `--all` — включить все процессы | `<pid>\t<имя>` в stdout |
| `inventory` | рантайм-инвентарь модулей/символов/экспортов | `--pid N`, `--max-symbols N` (по умолчанию 50 000), повторяемый `--module M` | `functions/pid-<PID>-<имя>.json/.csv` |
| `hook` | запись вызовов функций через Frida | `--pid N` (или `--spawn`), повторяемый `--module M`, `--kind symbols\|exports`, `--pattern REGEX`, `--limit N` (по умолчанию 64), `--arg-count 0..6` (2), `--duration-ms N` (0 = до Ctrl-C), `--backtrace`, `--out FILE` | `logs/calls-<PID>.jsonl` (строка = вызов) |
| `summarize [лог]` | сводка вызовов из JSONL | без аргумента — первый `calls-*.jsonl` в cwd | `<лог>.by-function.csv` + счётчики в stdout |
| `cdp targets` | список целей CDP | env `CDP_ENDPOINT` (по умолчанию `http://127.0.0.1:9222`) | JSON в stdout |
| `walk <app>` | обход интерфейса | — | в разработке: M3 (сейчас код 2) |
| `domains`, `static` | отчёты по доменам функций и статическим символам | — | в разработке: M4 (сейчас код 2) |

Примеры: `discord-lab inventory --pid 28170`; `discord-lab hook --pid 28170 --pattern 'SendRtp|OnIncomingRtp'
--arg-count 3 --duration-ms 30000`; `discord-lab summarize logs/calls-28170.jsonl`.

## Типизация (JSDoc + tsc)

Скрипты остаются обычными `.mjs`/`.js` — запускаются `node` напрямую, без шага сборки.
Типы описаны JSDoc-аннотациями и проверяются компилятором:

```bash
npm run typecheck        # = tsc -p tsconfig.json && tsc -p tsconfig.agent.json
```

- `tsconfig.json` — хост-скрипты (`logger.mjs`, `cdp.mjs`, `ui-*.mjs`), типы Node;
  общие типы CDP-клиента и записей UI-прогонов — в `scripts/cdp-types.d.ts`
  (файл только с типами, в рантайме не подключается).
- `tsconfig.agent.json` — только `frida-log-calls.js`, типы агента
  (`@types/frida-gum`); две области типов не смешиваются — у Node и Frida
  пересекаются глобальные `setTimeout`/`setInterval` и т.п.
- Каждый типизированный файл помечен `// @ts-check`; включены `strict`,
  `noUncheckedIndexedAccess`, `useUnknownInCatchVariables`.
- Правки при типизации — эквивалентные: `?? ""` вместо веры в `pop()`/индексацию,
  `?? 0` для `hitCount` профиля, `error instanceof Error ? error.message : String(error)`
  в `catch`, `frida.MessageType.Send` вместо литерала `"send"`, проверка
  `page.webSocketDebuggerUrl` перед `new WebSocket(...)`.
- devDependencies: `typescript`, `@types/node`, `@types/frida-gum`.

## Тесты (строгий TDD)

Приложение `discord-lab` (каркас — `bin/`, `src/`) пишется строго по тестам:
сначала красный тест, потом минимальная реализация. Раннер — встроенный
`node:test`, зависимостей нет. План, правила и дорожная карта: `TDD-PLAN.md`;
микро-декомпозиция задач и волны воркеров: `TASKS.md`.

```bash
npm test              # unit + meta + smoke + e2e (e2e сами пропускаются без браузера/live-endpoint)
npm run test:unit     # быстрые unit-тесты чистого ядра src/core
npm run test:meta     # мета: план §8 ↔ файлы тестов ↔ экспорты модулей
npm run test:smoke    # реальный CLI: help/version/ошибки, summarize на фикстурах, pids (Frida)
npm run test:e2e      # CDP: headless-браузер на fixture-странице (нужен CHROME_BIN)
npm run test:watch    # unit в режиме наблюдения
npm run test:coverage # покрытие src/** с порогами: строки ≥ 90, функции ≥ 80, ветки ≥ 90
npm run check         # typecheck + все тесты — «одна кнопка» перед приёмкой шага
```

- Статус (2026-10-19): **M1 (ядро) и M2 (Frida-слой) приняты** — 176 тестов (172 pass / 0 fail /
  4 e2e-skip), покрытие 99.3/90.5/97.6, `npm run check` зелёный; впереди M3–M6 (walk-движок,
  отчёты, CI-гейты, паритет). Текущий полный прогон (после волн R7/RC) — **215 тестов
  (211 pass / 0 fail / 4 e2e-skip)**; гейт воркера — passed. Golden-данные построены из
  реальных артефактов (`logs/**`, `functions/**`) независимыми оракулами (`jq`/`awk`/`c++filt`),
  провенанс — в комментариях тестов.
- Уровни: unit (без процессов/сети), meta (связность плана и тестов),
  smoke (реальный процесс CLI; Frida — skip-aware), e2e (реальный CDP; без браузера —
  skip с подсказкой).
- Переменные e2e: `CHROME_BIN` (путь к chromium/chrome/brave),
  `DISCORD_LAB_LIVE_CDP` (`http://127.0.0.1:9223` — живой Discord, только чтение).

## Документация проекта (`docs/`)

- `docs/vesktop-analysis.md` — сравнение с Vesktop 1.6.7 (Electron-оболочка): оболочка ОС,
  стриминг, обновления — что перенять и чего нам не хватает.
- `docs/discord-qt-analysis.md` — сравнение с discord-qt 0.5.1 (самостоятельный клиент):
  контракты, патчи, голос.
- `docs/feature-backlog.md` — **бэклог переноса недостающих функций**: 41 карточка с описанием
  и критериями готовности (R4 — оболочка ОС, R5 — стриминг/аудио, R6 — платформа
  и расширяемость, D — данные/контракты); статусы волн — в `TASKS.md`.
- `docs/map.md` — карта проекта (Mermaid; GitHub рендерит граф прямо в браузере). Растровые
  версии и генератор жили в лаборатории `/tmp/opencode/r2-net/` (tmpfs — не сохранилась; карта
  воспроизводится из этой копии).
- `docs/re/` — **глубокий RE (волна R7, 2026-10-19)**: нормативные документы по шлюзу,
  медиа/голосу, бэкенду (`main`-процесс, IPC, нативные модули), REST, UI и i18n. Индекс —
  `docs/re/README.md`, графы процессов/вызовов/функций — `docs/re/architecture.md`. Сырые
  доказательства (pcap, strace, декоды, 1,7 ГБ) — в лаборатории `/home/sanya/re-deep/`
  (в git не попадают, как и остальные данные сессий).
- `docs/rust-client/` — **план создания полноценного нативного клиента Discord на Rust**
  (без Electron и Chromium): концепт (`CONCEPT.md`), каталог функций — 14 групп (`FEATURES.md`),
  архитектура с 8 графами (`ARCHITECTURE.md`), волны P0–P8 с 90 карточками (`PLAN.md`),
  тест-план с ~130 контрактными тестами на фикстурах RE (`TEST-PLAN.md`); строение объединённого
  проекта (`PROJECT-STRUCTURE.md`), трассировка функция→спека→фикстура→тест (`TRACEABILITY.md`),
  журнал сверки данных (`VERIFICATION.md`), рабочий промт координатора/воркеров
  (`DEVELOPMENT-PROMPT.md`); индекс — `docs/rust-client/README.md`.
  Курированные фикстуры — `fixtures/` (12 записей, md5-манифест), каркас — `rust/` (cargo,
  10 крейтов, офлайн), оракул — `docs/rust-client/verify/verify-re.mjs`.
- Мета-контроль: `tests/meta/backlog-sync.test.mjs` стережёт, что бэклог, анализы, карта
  и ссылки в README/TASKS не расходятся; `tests/meta/rust-client-sync.test.mjs` — синхронность
  плана Rust-клиента (файлы, графы, уникальные ID, живые ссылки, фикстуры `fixtures/`, каркас
  `rust/`, отчёт верификации `verify/report.json`).

## Предпосылки

- **ptrace**: attach к чужому процессу требует
  `sudo sysctl kernel.yama.ptrace_scope=0` (по умолчанию 1). Вернуть обратно:
  `sudo sysctl kernel.yama.ptrace_scope=1`. Альтернатива без изменения системы —
  `--spawn`.
- **frida** установлена в этом проекте (`npm install` здесь уже выполнен).
- Полный статический разбор REA (`analyze.sh`) работает без движков анализа.
  Для внутренних функций **без символов** (не экспортируемых и не входящих в `.symtab`)
  нужен движок: `rea setup --install-hopper` (или Ghidra + JDK 21).
  Discord-модули сейчас везут `.symtab` — поэтому `nm`/Frida дают полный список функций.

## Что важно знать

- У нативных модулей Discord **динамические экспорты скрыты** (11–20 штук), но
  **таблица символов полная** (тысячи функций) — отсюда источник данных по функциям.
- Адрес символа в `functions/` — рантайм-адрес на момент снятия; `offset`
  (адрес минус база модуля) переносится между запусками.
- Хуки ставятся по адресу; при обновлении Discord адреса меняются — переснимите инвентарь.
- Осторожно с массовыми хуками: сотни интерцепторов на горячих функциях заметно
  замедляют Discord. Начинайте с `--limit 32` и узкого `--pattern`.
- Для снятия хуков просто завершите сессию (Ctrl-C) — скрипт сам снимает интерцепторы.

## Скрытые модули (и что не попало в таблицы)

- **JS-ядро** (`app.asar` + `core.asar`): 976 webpack-модулей под номерами
  (`webpack-bootstrap@0:module` key `26500`, `71793`, …) — имён/путей у них нет,
  это следствие минификации.
- **Все нативные модули**: динамические экспорты скрыты (9–20 на модуль), символы
  лежат в `.symtab` и видны только статически/Frida-перечислением символов.
- **`discord_rpc` и `discord_spellcheck` — вообще без `.node` бинарника**:
  в сборке только JS (`ws` для rpc, JS-пакеты для spellcheck), поэтому в таблицах
  функций их нет; в рантайме они никогда не появляются как нативные модули.
- **`updater.node`** — stripped: остались только 9 динамических символов.
- **Рантайм-инвентарь voice/krisp урезан лимитом** (`--max-symbols 50000`),
  но статические таблицы полные: voice 69 621, krisp 37 001.
- В рантайме нативные модули грузятся только в двух процессах: главном
  (`discord_utils`) и voice-utility (`voice`, `krisp`, `erlpack`, `zstd`,
  `game_utils`, `utils`); renderer-процессы без них.

## WebRTC / сеть / RTT / медиа — что найдено (статика)

Полный отчёт: `functions/domains/REPORT.md`. Ключевое (числа — функции с адресами):

| Домен | Функций | Примеры |
| --- | ---: | --- |
| `webrtc` | 11 049 | их форк libwebrtc целиком |
| `rtp-rtcp` | 1 742 | `PlaybackTransport::SendRtp`, `RtcpTransport::SendRtcp`, `RemoteUser::OnIncomingRtpAudio/Video` |
| `rtt` | 58 | `RtpTransportControllerSend::OnRttUpdate`, `RTCPReceiver::RttStats`, `NonSenderRttStats` |
| `jitter` / `latency` | 29 / 71 | NetEq (буфер воспроизведения) |
| `audio` | 4 176 | `OpusDecInst`, `MuteSendStream::SendAudioData`, `AudioRecorder::Record`, G711/G722 |
| `video` | 5 045 | Opus/VP8/VP9/H264/H265/AV1 (Opus 174, H264 216, H265 107, VP8/9 по 80/84, AV1 74) |
| `camera` | 161 | `Engine::CreateCameraVideoSource`, `VideoCaptureModuleV4L2::StartCapture`, `BaseCapturerPipeWire` (демонстрация экрана через PipeWire) |
| `network` | 3 743 | `Connection`, `Transport`, `Udp`, bandwidth/pacer, NACK/FEC/Feedback |
| `krisp-noise` | 551 | шумодав на onnxruntime |

Наблюдения:

- Символов `ICE`/`STUN`/`DTLS`/`SRTP`/`cricket` **нет** — в этой сборке Discord
  не использует P2P ICE-стек: подключение к голосовым серверам идёт через их
  собственный транспорт (`discord::media::Connection/Transport`).
- «Демки» в рантайме (реальные вызовы RTP/RTT/кодеков) снимаются хуками
  в voice-процессе — во время звонка/теста микрофона:
  `node scripts/logger.mjs hook --pid <voice-pid> --pattern 'OnRttUpdate|SendRtp|SendRtcp|OnIncomingRtp'`

## Тест интерфейса: прогон по всему меню (CDP + профайлер)

Discord запускается с `--remote-debugging-port=9222`; управление идёт
по Chrome DevTools Protocol (`scripts/cdp.mjs`, команды `eval/list/click/clicktext/
lclick/rclick/menu/key/shot/dump-clicks`). Для контекстного меню каналов
используются **доверенные** клики (`Input.dispatchMouseEvent`), остальное — DOM-клики.

Прогон (2026-10-18):

| Что | Объём |
| --- | --- |
| Настройки (все разделы + подпункты) | **56 пунктов** (`scripts/ui-walk-settings.mjs` + `ui-walk-settings2.mjs`) |
| Меню сервера | 13 пунктов (Буст, Пригласить, Настройки, Активные ветки, Каталог, Показать все каналы, Параметры уведомлений, Конфиденциальность, Профиль сервера, Изоляция, Пожаловаться на рейд, Копировать ID; «Покинуть сервер» — пропущен) |
| Меню канала | Пометить как прочитанное, Пригласить, Закрепить вверху, Копировать ссылку, Параметры уведомлений, Копировать ID; «Заглушить» — пропущен |
| Почта (inbox) | 4 вкладки: Непрочитанные, Упоминания, Закладки, Напоминания |
| Панель голоса | меню «Настройки ввода» и «Настройки вывода» (устройства не переключались) |
| Меню «Помощь» | открывает страницу поддержки в системном браузере (Brave) |

На каждый клик: короткая запись в `logs/ui-clicks-*.jsonl` (aria/текст/координаты),
скриншот `evidence/screens/ui-*.png`, профиль JS за ~1 с вокруг клика
(CDP Profiler → топ функций) в `logs/ui-walk-*.jsonl`; сводка —
`logs/ui-clicks-summary.csv` (фаза, шаг, пункт, топ-3 функции, скриншот).

Ограничения:

- Имена функций минифицированы (`callback`, `rr`, `o8`), но видно файлы-бандлы
  (`web.*.js`, `192743.*.js`, `501428.*.js` — голос/видео-панели).
- Сознательно не нажимались: «Выйти», «Покинуть сервер», «Заглушить канал»,
  «Отписаться от категории», переключение устройств, mute/deafen, кнопки окна.
- «Скрыть заглушенные каналы» не появился после переключения «Показать все каналы».
- Само окно зафиксировано скриншотом рабочего стола:
  `evidence/screens/window-discord.png` (окно поднято KWin-скриптом),
  `window-desktop-full.png`.

## Сеть: fake-IP ↔ хосты (из §5 `evidence/network/endpoints.md`)

DNS отдаёт адреса из `198.18.0.0/15` (прокси). Реальные имена восстанавливаются
запросом: `gateway.discord.gg → .13`, `cdn.discordapp.com → .14` (UDP/443, **QUIC**),
`images-ext-1.discordapp.net → .24`, `dl.discordapp.net → .25`, `discord.com → .5`.
Голос шёл на реальный `104.29.155.201:19309` (opus 48 kHz stereo, payload 120,
RED+NACK, RTP-расширения Discord MID/RID).

## Разведка gateway (R1, 2026-10-19)

Снято с изолированной копии клиента (CDP 9223; живой клиент не трогался). Артефакты — в
`/tmp/opencode/r2-net/` (данные сессии в репозиторий не коммитятся).

- **Шлюз**: `wss://gateway-us-east1-d.discord.gg/?encoding=etf&v=9&compress=zstd-stream`, поток
  zstd с первого кадра; heartbeat 41 250 мс; `op40` QoS (`ver:31`) ↔ `op11` ACK.
- **Кадры**: 70 recv-термов — HELLO(10), RESUMED, SESSIONS_REPLACE, CHANNEL_INFO,
  GUILD_MEMBER_LIST_UPDATE (ленивый список участников), PRESENCE_UPDATE, VOICE_STATE_UPDATE,
  MESSAGE_POLL_VOTE_*, USER_SETTINGS_PROTO_UPDATE; sent — RESUME(6), PRESENCE(3), VOICE_STATE(4),
  SUBSCRIPTIONS(37, диапазоны), QoS(40), REQUEST_CHANNEL_INFO(43).
- **ETF**: 16 тегов (таблица — `REPORT.md` §11 в каталоге волны); особенности: `nil` — строка «nil»,
  булевы — «true»/«false», снежинки — строки с суффиксом `n`.
- **REST**: 231 URL / 88 эндпоинтов `api/v9` восстановлены из кэша (`rest-endpoints.md`).
- **Медиа**: режимы шифрования 0–6 (`aead_aes256_gcm_rtpsize`, `aead_xchacha20poly1305_rtpsize`);
  AAD = заголовок (RTP 12+4·CSRC+4, RTCP 8), nonce — 4-байтовый LE-счётчик в хвосте пакета; голосовой
  сокет — `op1` каждые 13,75 с (`VOICE-CRYPTO.md`).
- **Ассеты кэша**: 4640 записей — 15 mp3 (UI-звуки), 22 woff2, 137 svg, 206 png, 429 webp, 13 видео.
- **Вывод**: сценарий A (свой мессенджер с UX Discord) — GO (≈85–90 %); сценарий C (клиент к реальному
  Discord) — юридический NO-GO. Оценка — `READINESS.md`, карта — `MAP.md`.

## Глубокий RE: шлюз, медиа, бэкенд, UI (R7, 2026-10-19)

Продолжение разведки на изолированной копии (карточки — `TASKS.md` §R7; нормативные доки —
`docs/re/`, индекс — `docs/re/README.md`, графы — `docs/re/architecture.md`). Секреты вырезаны,
публичные IP — `<PUB_IP>`, исходящие WS-кадры — только длина + sha256.

- **Шлюз**: 11 фактических ETF-тегов; исправлено кодирование big-int (знак **до** цифр);
  `op7 RECONNECT → RESUME → RESUMED` — серия `s` без разрывов; 1340 диспатчей на 7 сокетах.
- **READY/GUILD_CREATE (D-01)**: 4 полных READY (40 ключей, 7 гильдий, 17 ЛС, 146 users, 157 read_state);
  GUILD_CREATE при `data_mode: full` не приходит — гильдии целиком в `READY.guilds[]`
  (ITSKIDON: 83 канала, 34 роли, 24 229 участников).
- **REST (D-02)**: 47 маршрутов / 133 запроса `/api/v9` из живого захвата; auth — 221 запрос,
  6 заголовков, `X-Super-Properties` (22 поля, build 641004); независимая сверка —
  `docs/re/rest-d02-verify.sh` (exit 0).
- **UI (D-03)**: поиск (7 секций фильтров, сортировка), эмодзи-пикер (3 вкладки, 10 категорий),
  участники, настройки, поповер пользователя; 12 контрактов компонент с токенами и цитатами.
- **Голос**: полная сигнализация RTC-WS (ops 8/0/1/2/4/15/16/3/6); UDP-байты
  (discovery/keepalive/RTP/SRTCP), ротация медиа-сокета +60,08 с; DTX,
  `aead_aes256_gcm_rtpsize`, DAVE/MLS v1.
- **Бэкенд**: 5 полных IPC-цепочек, 168 каналов `DISCORD_*`, мост `DiscordNative` (35 записей,
  47 call-sites), 9 нативных модулей (`discord_voice.node` 126 МБ), апдейтер, окна, фичефлаги.
- **i18n**: собственный рантайм (не IntlMessageFormat); ключ = `xxh64 LE → base64[0..6]` (36 бит);
  плурализация ru + `=N`; 23 правила rich-разметки; 31/36 локалей; 23 836 ключей.

## План Rust-клиента (2026-10-19)

Результаты RE собраны в план создания **полноценного клиента Discord на Rust** — нативный
бэкенд в оболочке Tauri (системный WebView; без Electron и встроенного Chromium). Пакет
`docs/rust-client/`: концепт
(`CONCEPT.md` — цель, стек, сценарии, критерии, риски), каталог функций из 14 групп
(`FEATURES.md` — что делает каждая подсистема, зачем и как по RE), архитектура с 8 графами
Mermaid (`ARCHITECTURE.md` — процессы, крейты, шина, хранилище, замена 168 IPC-каналов,
ADR по UI), волны P0–P8 с 60+ карточками (`PLAN.md`), тест-план L1–L5 с ~130 контрактными
тестами на фикстурах лаборатории (`TEST-PLAN.md`).

**Единый проект собран:** курированные конформанс-фикстуры — `fixtures/` (12 записей, 1,59 МБ,
`MANIFEST.json` с md5 и провенансом), каркас клиента — `rust/` (cargo-воркспейс, 10 крейтов,
собирается офлайн: `cd rust && cargo test`), автоматическая сверка сырья с документами —
`node docs/rust-client/verify/verify-re.mjs` (20/20; журнал — `VERIFICATION.md`), строение —
`PROJECT-STRUCTURE.md`, трассировка функций — `TRACEABILITY.md`. Синхронность стережёт
`tests/meta/rust-client-sync.test.mjs`; карточки — `TASKS.md` §RC.

**Реализация идёт (2026-10-19):** волна RW-1 — ETF-кодек шлюза (`discord-codec`, карточка
RW-01, план P0-02): 11 фактических тегов, big-int «знак до цифр», 5 реальных термов захвата,
round-trip байт-в-байт, fuzz-lite; приёмка пройдена (независимое ревью — 95/95 сверок с сырьём).
Волна RW-2 — zstd-stream декодер (`discord-codec`, карточка RW-02, план P0-03) поверх `ruzstd`:
непрерывный кадр без `end()`, магия только в кадре 0 (сокет 1389: 137 кадров, 104 066 → 824 816 Б
по оракулу), удержание окна 64 КиБ задокументировано; приёмка пройдена (внешний тест — 390
проверок, 4/4 мутации пойманы). На 2026-10-21 приняты RW-01…RW-21 — **25/90 карточек**
(P0-02, P0-03, P0-04, P0-07, P1-01, P1-02, P1-03…P1-09, P1-11, P1-12, P2-01, P2-02,
P2-04, P2-05, P2-06, P2-07, P2-08, P2-09, P2-10, P2-11); RW-10…RW-21 прошли повторное
ревью, P2-01…P2-09 — по несколько раундов, legacy `ready_data` удалён. B02 принят:
settings 18, codec 43, conformance 6, 11/11 + 13/13 мутаций, P0/P1/P2=0; следующий
batch B03/RW-22.
`cd rust && cargo test --offline` — **580/580** (ядро 524/524).
Карточки и статусы — `TASKS.md`
§RW, факт — `docs/rust-client/STATUS.md`, план — `docs/rust-client/ROADMAP.md`,
команда и batch-поток — `docs/rust-client/TEAM.md`.

## GPU-кодировщики (проба Discord, `discord-webrtc_0`)

Discord сам печатает результат пробы (`video_linux_interop::encode::ffi`):

| Vendor | API | h264 | h265 | av1 |
| --- | --- | --- | --- | --- |
| Nvidia RTX 5070 Ti (615.78.08) | cuda + vulkan | ✅ | ✅ | ✅ |
| Amd Radeon 610M | vaapi | ✅ | ❌ | ❌ |
| Intel | vaapi | ❌ | ❌ | ❌ |

Голосовой процесс получает три GPU-хелпера (Nvidia/Amd/Intel). Рендер Chromium идёт
через iGPU (`--render-node-override=/dev/dri/renderD129`), NVIDIA = `renderD128`.

Хуки NVIDIA (Frida):

- NVDEC (`libnvcuvid`, GPU-процесс): 18 функций встали, **0 вызовов** за 90 с —
  в этот момент GPU-декодирование не работало.
- NVENC (`libnvidia-encode`, голосовой процесс): модуль грузится лениво —
  сторож `scripts/` ждёт его появления и ставит хуки `NvEncodeAPI`.
  Для живой демонстрации нужно включить «Трансляцию» (Go Live) в звонке.
- Камеры в системе нет (`/dev/video*` отсутствует), поэтому тест камеры NVENC не даст.


## Правовая рамка

Реверс-инжиниринг и модификация клиента Discord ограничены его ToS; анализ
собственного установленного клиента на своей машине вы выполняете под свою
ответственность. Не используйте эти инструменты для обхода ограничений, читов
или распространения изменённых сборок. Для «легального» разбора модов проще
смотреть открытые исходники [Vencord](https://github.com/Vencord) (AGPL).

## Результаты

Собрано 2026-10-18:

| Данные | Значение |
| --- | --- |
| `evidence/js-app-graph.json` | 976 модулей, 3 208 findings, 504 358 AST-узлов, 5 файлов (app.asar + core.asar) |
| `functions/static/*.csv` | 115 158 функций: `discord_voice` 69 621, `discord_krisp` 37 001, `discord_utils` 3 599, `discord_game_utils` 2 642, `discord_erlpack` 1 449, `discord_zstd` 838, `updater` (только dynsym: 9) |
| `functions/pid-*.json/.csv` | рантайм-инвентарь: главный процесс — 3 807 симв./11 эксп.; процесс с voice/krisp — 93 433 симв./39 824 эксп. (лимит 50k на модуль для `discord_voice`/`discord_krisp`) |
| `logs/calls-23548.jsonl` (+`.by-function.csv`) | 123 029 вызовов, 1 уникальная горячая функция (деструктор задач `InputEvents::OnSystemInputEvent`) |
| точечный прогон по обработчику ввода | 13 хуков встали, но 0 вызовов за 45с: события идут через Task-лямбды, а не через сам `OnSystemInputEvent` |
| `logs/ui-walk-settings.jsonl` | 56 разделов/подпунктов настроек — скриншот + топ-функции на каждый клик |
| `logs/ui-walk-menus.jsonl` | 52 шага меню/панелей: сервер, канал, почта, голосовая панель, «Помощь» |
| `logs/ui-clicks-settings.jsonl` + `ui-clicks-menus.jsonl` | 113 кликов с aria/текстом/координатами |
| `logs/ui-clicks-summary.csv` | сводка «пункт → топ-3 JS-функции» (108 шагов) |
| `evidence/screens/window-discord.png` | окно Discord целиком (KWin-скрипт + spectacle), с тайтлбаром и панелью голоса (1:04:14 в звонке) |
| NVDEC-хук (`libnvcuvid`, GPU-процесс) | 18 функций встали, 0 вызовов за 90 с |
| GPU-проба Discord | Nvidia cuda+vulkan: h264/h265/av1 ✅, Amd vaapi: h264, Intel: — |

Наблюдения:

- За 90 секунд в главном процессе активна ровно одна нативная функция — задача ввода
  `discord_utils`; Discord в простое почти не дёргает нативный код (всё в JS-процессах).
- X11-события мыши (xdotool) дают сотни вызовов в секунду — это самый дешёвый способ
  разогреть обработчики ввода.
- Во время прогона Discord перезапустил главный процесс (pid сменился: 23548 → 28032);
  после этого инвентарь нужно переснимать, адреса символов меняются вместе с базой.
- `discord_voice`/`discord_krisp` грузятся в отдельный utility-процесс — их функции
  активируются в голосовых сессиях.
- «Помощь» в тайтлбаре открывает страницу поддержки в системном браузере (DOM не меняется).
- «Почта» — правая панель с 4 вкладками: Непрочитанные, Упоминания, Закладки, Напоминания.
- Страницы «Голос/Камера/Стриминг/Звуки/Звуковая панель» собираются из чанков
  `501428.*.js` и `web.*.js` (имена функций минифицированы).
- Для перехвата NVENC надо, чтобы пользователь запустил трансляцию в звонке:
  фоновый сторож сам поставит хуки `NvEncodeAPI` и запишет `logs/calls-nvenc.jsonl`.

