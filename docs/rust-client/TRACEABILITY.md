# Трассируемость: функция → спека → фикстура → тест

Сверка каталога функций (141 функция, 14 групп — `FEATURES.md`) с нормативной спекой
(`docs/re/`), конформанс-фикстурами (`fixtures/`, лаборатория) и каталогом тестов
(133 теста, 13 групп — `TEST-PLAN.md`). Метод: каждая группа функций обязана иметь
(1) документ-первоисточник с числами, (2) фикстуру-оракул или явную пометку «только
лаборатория», (3) тесты в тест-плане, (4) карточки реализации в `PLAN.md`.

## 1. Матрица групп

| Функции | Кол-во | Спека (docs/re) | Фикстуры | Тесты | Волны |
|---|---|---|---|---|---|
| `F-GW` — шлюз событий | 11 | `gateway-protocol.md` §4–6, `gateway-schema.md` | `fixtures/gateway/by-type/`×164, `summary.json`; лаборатория: `capture-run2.jsonl`, 7 сокетов `gw-proto` | **T-GW 9** (+T-ENC 5) | P1, P2, P4 |
| `F-ENC` — кодеки транспорта | 3 | `gateway-protocol.md` §2–3 (ETF-теги, zstd-стрим) | `fixtures/gw-proto/facts-frozen.json`, `msgs-run2-…-13072.jsonl` | **T-ENC 5** | P1 |
| `F-AUTH` — вход и сессия | 5 | `gateway-protocol.md` §5 (IDENTIFY/RESUME), `main-arch.md` | лаборатория: `capture-ready/boot.jsonl`; `docs/re/gateway-protocol.json` | **T-REST 9** (часть кейсов), T-SEC 5 | P1, P6, P7 |
| `F-API` — REST v9 | 14 | `rest-d02.md`, `rest-static.json` | `fixtures/rest/rest-static.json`; `docs/re/rest-d02.json` (47 маршрутов); лаборатория: `d02-snapshot/` | **T-REST 9** | P1–P3, P6 |
| `F-DATA` — состояние и хранилище | 9 | `gateway-schema.md`, `feature-backlog.md` (D) | `fixtures/gateway/by-type/` (GUILD_*, MESSAGE_*), `ready-stats.json` | **T-DATA 13** | P2, P4, P6 |
| `F-MSG` — сообщения | 12 | `rest-d02.md` §messages, `gateway-protocol.md` §4 | `fixtures/gateway/by-type/` (MESSAGE_*, REACTION); лаборатория: `capture-run2.jsonl` | **T-MSG 10** | P3, P6, P7 |
| `F-SRCH` — поиск | 5 | `frontend-d03.md` §поиск | лаборатория: `ui*/`, `decoded/`, `frontend-d03.json` | **T-SRCH 3** | P3 |
| `F-PRES` — присутствие | 4 | `gateway-protocol.md` §4 (PRESENCE_UPDATE ×107) | `fixtures/gateway/by-type/010-PRESENCE_UPDATE.json`; лаборатория: `capture-run2.jsonl` | **T-DATA 13** (T-DATA-11) | P2, P3, P6 |
| `F-VOICE` — голос | 21 | `voice-deep.md` §1–3 (сессии s2/s3, SRTP, keepalive) | `fixtures/voice/*`; лаборатория: `strace-media-s3.jsonl`, `tcpdump*.log` | **T-VOICE 23** | P4, P4.5, P5, P6 |
| `F-MEDIA` — видео/трансляция | 7 | `voice-deep.md` §4 (движок кодеков) | лаборатория: `engine-log-session1.txt`, pcap | **T-MEDIA 8** | P3, P5, P7 |
| `F-UI` — интерфейс | 24 | `frontend-d03.md` §5 (12 контрактов) | `docs/re/frontend-d03.json`; лаборатория: `ui*/`, `decoded/` | **T-UI 21** | P3, P5, P6 |
| `F-I18N` — локализация | 7 | `i18n-deep.md` §1–3, §7 (36 кодов, 31 включён) | `fixtures/i18n/i18n-stats.json`; лаборатория: каталоги ru/en, `locale-chunk-hashes.json` | **T-I18N 8** | P3, P7 |
| `F-SYS` — платформенные функции | 13 | `backend-ipc.md`, `native-re.md` | лаборатория: `backend-ipc.json`, `native-re.json` | **T-SYS 12** | P6, P7, P8 |
| `F-SEC` — безопасность/приватность | 6 | `voice-deep.md` §2 (ключи), `gateway-protocol.md` §5 | санитизация всех фикстур; лаборатория: `strace-*` | **T-SEC 5** | сквозные |
| **Итого** | **141** | `docs/re/` (26 файлов) | `fixtures/` (12 записей) + лаборатория | **133** (+T-HARN 7 — харнесс) | P1–P8 |

Соответствие «группа функций → группа тестов» не 1:1 по построению: `T-REST` покрывает
`F-AUTH`+`F-API`, `T-DATA` покрывает `F-DATA`+`F-PRES`, часть `F-GW` проверяется в `T-ENC`
(оракул кодеков — те же кадры). Сводные взаимодействия подтверждены: функции 141 = Σ групп,
тесты 133 = Σ групп (мета-тест `tests/meta/rust-client-sync.test.mjs`).

## 2. Сквозные проверки

| Что | Где | Как проверяется |
|---|---|---|
| Карточки P0–P8 → тесты | `PLAN.md` (столбец «Тесты» каждой карточки) | мета-тест: ≥60 карточек, волны P0–P8, живые ссылки |
| Фикстуры → тесты | `fixtures/MANIFEST.json` (поле `usedBy`) | `verify-re.mjs` FIX-01: md5 копий = манифест |
| Числа в доках = сырьё | `docs/re/*.json`, `docs/rust-client/*` | `verify-re.mjs`: 20 проверок (см. `VERIFICATION.md`) |
| Спека → продукт | `docs/re/README.md` (таблица покрытия) | пересечение с 14 группами из §1 |
| Санитизация | все фикстуры | `verify-re.mjs` SAN-01; `T-SEC-03` |

## 3. Известные ограничения

- Матрица ведётся **на уровне групп**; привязка «функция → конкретный T-ID» живёт в карточках
  `PLAN.md` и будет перенесена в Rust-репозиторий задачей P0-05 (машиночитаемая форма).
- `F-SEC` — сквозная группа: у неё нет отдельной волны, кейсы распределены (P1 — валидация
  входа, P8 — аудит), поэтому «Волны» = сквозные.
- Часть оракулов намеренно остаётся в лаборатории (`d02-snapshot`, pcap, каталоги локалей):
  в git — только минимальный срез (`PROJECT-STRUCTURE.md` §3).
