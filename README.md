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
node scripts/logger.mjs pids                 # процессы Discord, видимые Frida
node scripts/logger.mjs inventory            # инвентарь символов/экспортов по всем процессам
bash   scripts/extract-static-functions.sh   # статические таблицы функций из ELF-файлов
bash   scripts/analyze.sh                    # статический разбор app.asar через REA
npm run typecheck                            # tsc: типы scripts/, src/ и tests/ (см. «Типизация»)
npm test                                     # тесты unit+meta+smoke+e2e (см. «Тесты (TDD)»; сейчас red-базлайн)

# UI-прогоны (CDP; нужен запуск discord --remote-debugging-port=9222):
node scripts/ui-walk-settings.mjs     # все разделы настроек
node scripts/ui-walk-settings2.mjs    # добивающий проход по пропущенным
node scripts/ui-walk-menus.mjs        # меню сервера/канала/почты
node scripts/ui-menu-extra.mjs        # оставшиеся пункты + голосовая панель
node scripts/cdp.mjs rclick "текст"   # доверенный правый клик (меню канала)
node scripts/cdp.mjs lclick "текст"   # доверенный левый клик по координатам

# логирование вызовов (пример: планировщик задач discord_utils в главном процессе)
node scripts/logger.mjs hook --pid <PID> \
  --pattern 'ExecutePending|ExecuteInternal|ExecuteAfterInternal|PollMetrics' \
  --limit 32 --duration-ms 60000 --arg-count 2
# лог: logs/calls-<PID>.jsonl  (модуль, функция, адрес, поток, время, аргументы)

# хук по экспортам вместо символов (у Discord экспорты почти пустые)
node scripts/logger.mjs hook --pid <PID> --kind exports --module discord_utils.node
```

Полезные опции хука: `--module <имя>.node` (можно повторять), `--pattern REGEX`,
`--limit N` (максимум хуков), `--arg-count 0..6` (сколько указателей-аргументов писать),
`--duration-ms N` (0 = до Ctrl-C), `--out FILE`, `--spawn` (запустить Discord под Frida).

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

- Сейчас в репозитории **red-базлайн + углубление**: `src/**` — каркас интерфейсов
  без логики (каждая функция кидает `Не реализовано: …`); тесты показывают очередь
  работ. Golden-данные строятся из реальных артефактов (`logs/**`, `functions/**`)
  независимыми оракулами (`jq`/`awk`/`c++filt`), провенанс — в комментариях тестов.
- Уровни: unit (без процессов/сети), meta (связность плана и тестов),
  smoke (реальный процесс CLI; Frida — skip-aware), e2e (реальный CDP; без браузера —
  skip с подсказкой).
- Переменные e2e: `CHROME_BIN` (путь к chromium/chrome/brave),
  `DISCORD_LAB_LIVE_CDP` (`http://127.0.0.1:9223` — живой Discord, только чтение).

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

