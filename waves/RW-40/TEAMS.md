# RW-40 — TEAMS: группы, ответственные, зоны

Дата: 2026-10-24. PM: DeepSeek. Все группы работают по `COMMS.md` и `TEST-POLICY.md`.

## 1. Структура

```
Program Manager (PM DeepSeek)
├── G1 Auth & Network  — lead: Backend/Auth
├── G2 Product UI      — lead: Frontend
├── G3 Design          — lead: Design
├── G4 Quality         — lead: QA (+ Security)
└── G5 Platform/Voice  — lead: Platform
```

## 2. Группы и ответственные

| Группа | Ответственный | Зона | Файлы | KPI |
|---|---|---|---|---|
| **G1 Auth & Network** | **Backend/Auth lead** (роль NET-F1) | вход, keyring, captcha, gateway/REST, READY | `gui-spike/src/{auth,net}*.rs`, `core/rest|ws|gateway`, `tests/auth_contract.rs`, `tests/net_contract.rs` | login→READY, 0 утечек, 401/429 |
| **G2 Product UI** | **Frontend lead** (роль NET-D2/FE) | экраны, сообщения, настройки, auth UI | `ui/**`, `tests/unit/gui-spike-*` | 0 мёртвых кнопок, DOM/features green |
| **G3 Design** | **Design lead** (роль NET-DESIGN) | login-спека, темы, компоненты, паритет | `DESIGN/**`, `NET/LOGIN-SPEC.md` | все числа с источником, ассетов нет |
| **G4 Quality** | **QA lead** (роль QA-AUTH) | unit/meta/smoke/e2e, AT-SPI, матрица | `tests/**`, `QA/**` | P0/P1=0, 0 skip на дисплее |
| **G4b Security** | **Security reviewer** (Codex) | утечки, секреты, ToS-границы | `REVIEW*` | P0/P1=0 |
| **G5 Platform/Voice** | **Platform lead** | трей, уведомления, голос UI, updater | `core/platform|voice`, `ui/**` | headless→UI |

## 3. Текущие назначения (responsibility assignment)

| Задача | Ответственный | Группа | Статус |
|---|---|---|---|
| A1 окно входа (40/41, tsc) | **Frontend lead** (исполнитель: Codex CLI) | G2 | in work |
| A2 captcha session/rqdata/headers | **Backend/Auth lead** | G1 | queued |
| A3 connect/READY | **Network lead** | G1 | queued |
| A4 auth events/logout stop | Backend lead | G1 | queued |
| A5 QA-AUTH | **QA lead** | G4 | blocked (pool) |
| A6 Security review | Codex | G4b | queued |
| B1 отправка сообщений | Frontend+Network leads | G1/G2 | queued |
| B5 группы участников | Frontend lead | G2 | queued |
| C3 темы | Design+Frontend | G3/G2 | queued |

## 4. Правила группы

- У группы один ответственный; он отвечает за red/green и evidence.
- Чужие файлы не трогаются; стык — через PM и контракт.
- Внутри группы допускается параллельность, но ≤2 кодовых исполнителя.
- Любой блокер — сразу в `INBOX/<group>.md` и PM.

## 5. Исполнительный пул

| Пул | Доступность | Использование |
|---|---|---|
| Субагенты-воркеры | недоступны (баланс) | после восстановления — G1/G2/G4 |
| Codex CLI | ограниченно | критический путь A1, затем A2 |
| PM | доступен | приёмка/интеграция, не продукт-код |

## 6. RACI по функциям

`R` — делает, `A` — принимает решение/приёмка, `C` — консультирует, `I` — информируется.

| Функция | G1 Auth/Net | G2 Frontend | G3 Design | G4 QA | G4b Security | PM |
|---|---|---|---|---|---|---|
| Вход/токен/keyring | **R** | C | I | C | C | **A** |
| Окно входа | C | **R** | C | C | I | **A** |
| Login-спека/паритет | I | C | **R** | I | I | **A** |
| Gateway/READY | **R** | I | I | C | C | **A** |
| Чат/сообщения | C | **R** | C | C | I | **A** |
| Настройки/темы | C | **R** | C | C | I | **A** |
| Голос/ОС | C | **R** | C | C | I | **A** |
| Тест-план/матрица | C | C | I | **R** | C | **A** |
| Security/утечки | C | C | I | C | **R** | **A** |
| Merge/релиз | C | C | I | C | C | **A** |

## 7. Текущий стендап (2026-10-24)

| Группа | В работе | Блокер |
|---|---|---|
| G1 | A2 captcha-поля/headers | ждёт A2 (Codex) |
| G2 | A1 принят; ждёт A2/A3 для UI | нет |
| G3 | LOGIN-SPEC принят; ждёт W5-сверку | нет |
| G4 | QA-AUTH план готов | пул воркеров |
| G4b | Security-ревью после A2/A3 | нет |
| G5 | очередь P2 | P0 auth |
