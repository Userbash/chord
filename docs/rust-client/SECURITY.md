# SECURITY — модель угроз и правила безопасной разработки

Документ фиксирует, **что мы защищаем**, **где проходит граница доверия** и **какие проверки
обязательны до accepted**. Он дополняет `CODE-STANDARD.md` §5 и `LEGAL.md`; процесс приёмки —
`PROCESS.md` §4.3–§4.4.

Принцип: безопасность — не отдельная волна, а свойство каждого слоя. Для каждой карточки
указывается применимая часть модели; неприменимые пункты помечаются «не применимо» с причиной.

## 1. Модель угроз

### 1.1 Что защищаем (assets)

| Актив | Где живёт | Почему важен |
|---|---|---|
| Токен / session id / cookie | keyring, память, HTTP-заголовки | доступ к аккаунту |
| Содержимое сообщений и вложений | state, history, SQLite, UI | приватность переписки |
| Голосовые ключи и E2EE-материал | voice signaling, память | прослушивание/подмена медиа |
| Учётные данные и MFA | login flow, keyring | захват аккаунта |
| Локальная БД и кэши | `discord-store`, файлы | история и метаданные |
| Логи/дампы/ошибки | stderr, файлы, отчёты | утечка секретов и PII |
| Deep links / argv | `discord://`, CLI | подмена маршрута/токена |
| Updater-манифест и артефакт | сеть/диск | подмена кода |
| D-Bus/Secret Service/FFI | системные библиотеки | привилегии, fd, память |

### 1.2 От кого защищаемся ( adversaries )

- **Недоверенный сервер/сеть**: повреждённые, обрезанные, повторные, гигантские или
  противоречивые payload'ы Discord; MITM при отсутствии TLS-проверки.
- **Локальный недоверенный ввод**: argv, deep links, буфер обмена, файлы фикстур, env.
- **Скомпрометированная зависимость**: crate с уязвимостью/лицензионной ловушкой.
- **Ошибка самого клиента**: утечка секрета в `Debug`, path traversal, гонка, двойное
  освобождение fd, unbounded-очередь, silent fallback.
- **Не автор, а ошибка процесса**: тест «зеленит» дефект, skip маскирует live-утверждение,
  mutation-лог не соответствует коду.

### 1.3 Границы доверия

| Граница | Вход | Что обязано произойти на границе |
|---|---|---|
| Сеть → gateway | WS-кадры, zstd, ETF | лимиты размера/глубины, терминальные ошибки, unknown → raw, без паник |
| Сеть → REST | HTTP/1.1, TLS, JSON | проверка TLS/заголовков, лимит тела, 401/429/5xx-политика, без ретрая неидемпотентных |
| Голос → медиа | RTC JSON, UDP, RTP/SRTP | проверка op/полей, SSRC/peer, nonce/counter, tag/tail, bounded jitter |
| Диск → state/store | READY, SQLite, фикстуры | схема/типы, NUL/UTF-8, границы, миграции, provenance |
| Система → platform | D-Bus, logind, keyring, hunspell, libsqlite3 | auth/адрес, fd ownership, ABI, NUL, error-path cleanup |
| Пользователь → UI/CLI | argv, deep link, clipboard, файлы | канонизация пути, control chars, лимит длины, безопасный парсинг |
| Сеть → updater | манифест, артефакт | подпись Ed25519, SHA-256, downgrade guard, path traversal guard |
| Логи → наружу | `Debug`, ошибки, отчёты | redaction, отсутствие raw payload/токенов/PII |

Правило: **доверенные данные внутри ядра** — только после разбора на границе. Если слой
принимает уже разобранный тип, он не обязан повторять проверки, но обязан сохранять инварианты.

## 2. Модель STRIDE (сокращённо)

| Угроза | Пример в проекте | Контроль | Проверка |
|---|---|---|---|
| Spoofing | подмена voice endpoint/peer; deep link с чужим host | TLS, peer/SSRC, строгий разбор scheme/host | T-VOICE, T-SYS-04 |
| Tampering | подмена updater-манифеста; порча SQLite | Ed25519 + SHA-256; транзакции/миграции | T-SYS-11, T-DATA-05 |
| Repudiation | нет следов операции | структурная диагностика без секретов | unit/contract |
| Information disclosure | токен в `Debug`/логе; raw payload в ошибке | redaction, `Secret`-типы, сканер | T-SEC-02/03 |
| Denial of service | zstd-бомба, глубокая вложенность, 24k-список, fd-шторм | лимиты, bounded-очереди, таймауты, caps | T-ENC-05, T-DATA, T-SYS-06 |
| Elevation of privilege | path traversal в updater; shell в autostart; FFI | канонизация, без shell, safe wrapper | T-SYS-03/11, FFI-review |

## 3. Секреты: политика хранения и ротации

- Токены, session id, cookies, voice keys, E2EE-материал **не попадают** в `Debug`, `Display`,
  ошибки, логи, отчёты, имена файлов и commit-сообщения.
- Публичные типы с секретом обязаны иметь redacted `Debug`; для сырых байтов — длина/hash,
  не содержимое.
- Фикстуры и доки — только `REDACTED`/placeholder; сканер `T-SEC-02/03` обязателен.
- Keyring — единственное место хранения токена; файлы конфигурации и SQLite не используются
  как secret store.
- Ошибки внешних библиотек (libsecret, hunspell, SQLite, D-Bus) не печатают входные байты.
- При подозрении на утечку: остановить live-операцию, ротировать токен, очистить кэши,
  уведомить владельца, зафиксировать инцидент без секретов.

## 4. FFI и `unsafe`

`unsafe` разрешён только в изолированном модуле-обёртке; снаружи — безопасный API.
Для `discord-platform` и `discord-store` действует `deny(unsafe_code)` с точечными
`#[allow(unsafe_code)]` на FFI-модулях.

Чек-лист каждого блока:

- [ ] ABI и сигнатура сверены с заголовком/версией системной библиотеки;
- [ ] указатель не NULL, выровнен, жив на протяжении вызова; lifetime объяснён;
- [ ] длина передаётся вместе с указателем; нет чтения за границей; NUL-терминация проверена;
- [ ] владение (кто освобождает) и `Drop`/error-path cleanup определены;
- [ ] нет unwind через FFI; ошибки C-библиотеки преобразуются в typed error;
- [ ] thread-safety: можно ли вызывать из нескольких потоков; нужен ли мьютекс;
- [ ] `// SAFETY:` объясняет **почему** инвариант выполняется, а не повторяет код;
- [ ] есть тест на error-path, двойное освобождение/fd, лимит и отсутствие библиотеки;
- [ ] `cargo clippy -D warnings`, Miri/санитайзер (когда доступен) или явное ограничение.

Известные FFI-точки: `libsecret` (keyring), системный `libsqlite3` (store), `libhunspell`
(spellcheck), `libc` (`sendmsg/recvmsg`, SCM_RIGHTS, logind inhibitor). Для каждой — свой
контрактный тест и security-review.

## 5. Supply chain

- `Cargo.lock` — авторитетный набор; новые зависимости только с разрешения владельца.
- Офлайн-сборка по умолчанию; сеть — только в согласованном окне.
- `cargo deny`/SBOM: лицензии, запрещённые/уязвимые пакеты, version-pinned fallback для
  target-specific пакетов.
- Новая зависимость проходит проверку: maintenance, license, transitive tree, build scripts,
  `unsafe`, сетевые вызовы, размер; фиксируется в SBOM и отчёте волны.
- Системные библиотеки (`libsqlite3`, `libsecret`, `libhunspell`) проверяются по ABI/версии;
  отсутствие библиотеки — явный skip/код 2, не тихий fallback.
- Релизный артефакт подписан; updater проверяет подпись и хеш до перемещений.

## 6. Живые операции и golden

- Attach/spawn/walk на живом Discord — только по окну владельца; по умолчанию — код 2/skip.
- `logs/`, `functions/`, `evidence/`, `/home/sanya/re-deep` — только чтение; не перезаписывать,
  не «нормализовать», не коммитить секреты.
- Live-флаг при ошибке среды обязан падать, если тест заявляет live-контракт; обычный запуск
  может skip-ать с точной причиной.
- Никакого shell-спавна из недоверенных строк; пути — через канонизацию и `safeName`.

## 7. Security-тестирование по уровням

| Уровень | Что проверяем | Примеры |
|---|---|---|
| L0 Static | clippy/deny, SBOM, secret scan, unsafe inventory | CI, `scripts/ci.sh`, `scripts/sbom.mjs` |
| L1 Unit | redaction, лимиты, парсеры, crypto KAT | `Debug`-тесты, границы, NIST/RFC |
| L2 Contract | wire-форматы, NUL/control chars, schema | фикстуры RE, byte-exact |
| L3 Integration | FFI error-path, D-Bus auth/fd, SQLite | `keyring_contract`, `dbus_contract`, `store_contract` |
| L4 System | updater, deep links, autostart, loopback RTC | `updater_contract`, `deep_link_contract` |
| L5 Acceptance | login/logout, token storage, live smoke | ручной чек-лист + owner |
| L6 Non-functional | DoS-лимиты, fuzz, таймауты, утечки | fuzz/property/chaos |
| L7 Mutation | redaction/guard/fd-мутанты | mutation-ledger |

## 8. Триггеры обязательного security-review

- крипто, ключи, nonce/counter, E2EE;
- FFI/`unsafe`, работа с fd/памятью;
- auth/login/token/keyring;
- updater, подпись, path traversal;
- deep links, argv, clipboard, autostart;
- сетевые парсеры и лимиты (ETF/zstd/JSON/protobuf/RTP/SRTP);
- live-пути и операции с golden;
- новые зависимости.

Review выполняет агент, не автор; для live/FFI — отдельный security-reviewer
(`PROCESS.md` §4.4).

## 9. Известные ограничения (на 2026-10-23)

- Wire-совместимость SRTP/DAVE с Discord не заявлена: нет ключевого материала в capture.
- `discord-store` однопоточный (`!Send + !Sync`); WAL только для файла.
- `discord-platform` после FFI-модулей не может гарантировать «unsafe запрещён полностью»;
  каждый FFI-модуль требует отдельного ревью.
- D-Bus abstract/tcp, percent-encoding и часть hints — задокументированы как ограничения.
- Live-уведомления/ингибитор проверены на реальном logind, но не в промышленном окне.
- Полный threat-model review GUI/WebView (Tauri) ещё впереди — до P0-01.

## 10. Инцидент

1. Остановить операцию и не продолжать live-прогон.
2. Не логировать секрет; зафиксировать только категорию, время, затронутый компонент.
3. Ротировать токен/ключ, очистить кэши/логи, проверить, не ушёл ли артефакт в git.
4. Уведомить владельца; оформить инцидент и regression-тест.
5. Обновить чек-лист/правило, если причина — процесс, а не единичная ошибка.
