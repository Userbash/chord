# PROGRAM-AUTH — единая программа аутентификации и сетевого подключения

Дата: 2026-10-23. PM: DeepSeek. Статус: активна.
Цель: один сквозной путь «вход → сессия → шлюз → данные» силами дизайна, frontend, backend,
ядра, сети и QA — без дублирования сетевого кода.

## 1. Потоки работ (workstreams)

| ID | Поток | Владелец | Вход | Выход | Зависит |
|---|---|---|---|---|---|
| W1 | RE login-окна, captcha, 2FA | **NET-R2** | дамп `/home/sanya/re-deep` | `LOGIN-UI-RE.md` + json — **готово (D-38)** | — |
| W2 | Дизайн окна входа «как в оригинале» | **NET-DESIGN** | W1, `REFERENCES`, `tokens.css` | `LOGIN-SPEC.md` + json | W1 |
| W3 | Core-примитивы | NET-C1/E1 (готово) + NET-F1 (`post`) | `ADR-3` | `ApiClient::post`, `Me`, `GatewayUrl`, op40 | — |
| W4 | Backend auth-flow | **NET-F1** | `AUTH-RE`, W3 | Tauri: password/mfa/sms/token/status/logout — **готово (D-35)** | W3 |
| W5 | Frontend login UI | **NET-D2** | W2, W4, `AUTH-CONTRACT` | `ui/features/auth.js`, `auth.css`, тесты | W2, W4 |
| W6 | Сеть: подключение после входа | **NET-E2** | W4, supervisor | `connect`/`reader` + READY, статусы | W4 |
| W7 | QA/Security | **QA-AUTH**, Codex | W5, W6 | `QA/AUTH-REPORT.md`, P0/P1 | W5, W6 |

## 2. Единый контракт

Все потоки работают по `NET/AUTH-CONTRACT.md` (состояния, команды, ошибки, события, id).
Изменение контракта — только через PM и `DECISIONS.md`; иначе поток возвращается.

## 3. Файловая матрица (без пересечений)

| Владелец | Файлы |
|---|---|
| NET-R2 | `NET/LOGIN-UI-RE.md`, `NET/login-ui-re.json`, `NET/PM/netr2/**` |
| NET-DESIGN | `NET/LOGIN-SPEC.md`, `NET/login-spec.json`, `NET/PM/netdesign/**` |
| NET-F1 | `gui-spike/src/auth*.rs`, `src/main.rs`, `tests/auth_contract.rs`, `core/src/rest/client.rs`, `NET/PM/netf1/**` |
| NET-D2 | `gui-spike/ui/{index.html,app.js,style.css}`, `ui/features/{auth,screens}.js`, `ui/components/auth.css`, `tests/unit/gui-spike-auth.test.mjs`, `NET/PM/netd2/**` |
| NET-E2 | `gui-spike/src/net*.rs`, `src/main.rs` (только регистрация), `tests/net_contract.rs`, `NET/PM/nete2/**` |
| QA-AUTH | `QA/AUTH-*.md`, `QA/artifacts/**` |

`model.js`, `data.rs`, `fixtures/**` — read-only без отдельного решения PM.

## 4. DAG и порядок

```
W1 RE ──► W2 Design ──┐
W3 Core ──► W4 Backend ─┼─► W5 Frontend ─┐
                        └─► W6 Network ──┼─► W7 QA/Security ─► merge gate
                                          ┘
```

Параллельно: W1+W3 сейчас; W2 после W1; W4 после W3; W5 после W2+W4; W6 после W4; W7 после W5+W6.

## 5. Гейты

| Гейт | Критерий |
|---|---|
| W1 | провенанс `[D]/[M]/[A]`, нет ассетов, 0 выдуманных полей |
| W2 | все числа с источником; состояния/ошибки/a11y; токены существуют |
| W3/W4 | red-first, контрактные тесты, clippy/fmt, секреты редактируются |
| W5 | DOM/features/auth-тесты, a11y, токен не в DOM, мёртвых кнопок нет |
| W6 | supervisor/READY на стабе, close-коды, reconnect |
| W7 | P0/P1=0, утечек нет, QA-матрица пройдена |
| Merge | `gate:release` + `cargo test --workspace` + один live-прогон |

## 6. Риски

| Риск | Митигация |
|---|---|
| ToS/блокировка аккаунта | только тестовый аккаунт, ADR-3, письменное принятие |
| Утечка пароля/токена | нет хранения пароля, keyring, redaction, тесты |
| Captcha-дрейф | интерактивный виджет, фича-флаг, token-import fallback |
| 429/401 | backoff, понятные состояния, без циклов |
| Расхождение контрактов | `AUTH-CONTRACT.md`, PM-ревью, один owner на файл |

## 7. Definition of Done программы

- вход (токен или логин/пароль+2FA+captcha) → keyring → `/users/@me` → READY;
- перезапуск восстанавливает сессию; logout очищает токен и останавливает шлюз;
- ошибки 401/429/captcha/сеть — понятны, без утечек;
- окно входа соответствует `LOGIN-SPEC.md`;
- все слои зелёные, P0/P1=0, live-прогон записан в бюджет.
