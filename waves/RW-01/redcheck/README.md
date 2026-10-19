# discord-rs — каркас нативного клиента (P0)

Рабочая область Cargo с границами крейтов из `docs/rust-client/ARCHITECTURE.md` §2.
Статус: **каркас P0** — структура, константы из RE и точки роста; логика появляется
волнами P1–P8 строго по `docs/rust-client/PLAN.md` (red-first по `TEST-PLAN.md`).

Внешних зависимостей нет: `cargo test` работает офлайн. Добавление крейтов (tokio,
winit, iced/slint и пр.) — отдельная задача P0-02 с проверкой сети и лицензий.

| Крейт | Ответственность | Спека | Тесты |
|---|---|---|---|
| `discord-codec` | ETF (erlpack) + zstd-стрим | `docs/re/gateway-protocol.md` §2–3 | T-ENC-* |
| `discord-core` | шлюз, REST v9, сессии, реконнект | `docs/re/gateway-protocol.md` §4–6 | T-GW-*, T-REST-* |
| `discord-store` | локальное хранилище и миграции | `docs/feature-backlog.md` (D-серия) | T-STORE-* |
| `discord-voice` | сигнализация RTC, SRTP, keepalive | `docs/re/voice-deep.md` | T-VOICE-* |
| `discord-media` | Opus/AV1, захват, скриншот-стрим | `docs/re/voice-deep.md` §4 | T-MEDIA-* |
| `discord-platform` | системная интеграция (tray, уведомления) | `docs/re/backend-ipc.md` | T-SYS-* |
| `discord-i18n` | ключи, плурализация, 31 локаль | `docs/re/i18n-deep.md` | T-I18N-* |
| `discord-ui` | дизайн-токены, компоненты, 12 контрактов | `docs/re/ui-rest-d03.md` | T-UI-* |
| `discord-app` | сборка процесса, конфиг, запуск | `docs/rust-client/ARCHITECTURE.md` | T-APP-* |
| `discord-conformance` | доступ к фикстурам `fixtures/` | `docs/rust-client/TEST-PLAN.md` §3 | T-HARN-* |

Запуск:

```sh
cd rust && cargo test        # офлайн, без зависимостей
node ../docs/rust-client/verify/verify-re.mjs   # сверка лаборатории и констант
```

Правила: константы — только с доказательством из RE (в докстрингах указан документ);
`unsafe` — запрещён до появления обоснованного FFI-слоя (P7); тесты не «зеленятся» под код.
