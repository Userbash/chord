# RW-39 — TZ: дизайн-паритет с оригиналом + frontend/backend на RE-данных

Статус: Phase 1 (design references + design system). PM: DeepSeek. Дата: 2026-10-23.
Цель: привести Tauri-UI к раскладке/токенам оригинального Discord-клиента, с легальными
ассетами, и реализовать frontend/backend-функции на данных RE. Это foundation P3-01/P3-02.

## 0. Юридические границы (LEGAL.md)

- **Нельзя** распространять шрифты, иконки, звуки, emoji и иные ассеты Discord без лицензии.
- Используем: системные шрифты (`system-ui`), системные emoji, **собственные inline-SVG**
  или открытые иконки с совместимой лицензией (источник фиксируется).
- Название/логотип Discord не используются как бренд проекта; в UI — нейтральный бренд `DL`.
- Скриншоты/токены из `reference-oracle/` — **референс**, не копируются в репозиторий;
  допускаются измерения (цвета, отступы, размеры) как значения дизайн-системы.

## 1. Референсы

- `reference-oracle/ui/00-app.png` — общий экран (рейл, дерево каналов, чат, участники).
- `ui/05-dm-list.png`, `ui/06-profile-popout.png`, `ui/07-guild-menu.png` — панели.
- `reference-oracle/ui8/` — поиск, фильтры, эмодзи, настройки, участники (JPG/HTML/JSON).
- `reference-oracle/ui/tokens.json` — 4795 CSS-переменных (значения — референс).
- `docs/re/frontend-d03.md` — контракты компонент, размеры, поведение.
- `fixtures/` — санитизированные RE-данные для backend (READY, сообщения, участники).

## 2. Роли и этапы

| Этап | Воркер | Задача | Выход |
|---|---|---|---|
| **1a** | **W39a (REA)** | Извлечь раскладку/токены/компоненты/экраны из дампа | `DESIGN/REFERENCES.md`, `tokens.json`, `layout.json` |
| **1b** | **W39b (Design)** | Дизайн-система и спецификация экранов, DOM-контракт | `DESIGN/SPEC.md`, `tokens.css`, `DOM-CONTRACT.md` |
| **2a** | **W39c (Markup/CSS)** | Верстка: shell, rail, tree, chat, members, composer, 4 экрана | `ui/index.html`, `ui/style.css`, `ui/components/*.css` |
| **2b** | **W39d (Frontend)** | Интеракции: виртуализация, emoji, settings, клавиши, поиск | `ui/app.js`, `ui/features/*.js` |
| **3** | **W39e (Backend)** | BFF на `fixtures/`: READY, страницы сообщений/участников, настройки; Tauri-команды | `src/data.rs`, `tests/data_contract.rs` |
| QA | QA-1 | Мутации по новым инвариантам + AT-SPI/DOM | `mutations*`, логи |
| REV | REV-1 | Ревью P0/P1, дизайн-паритет, юр. границы | `REVIEW*.md` |

Одновременно — не более 2 фоновых воркеров; этапы 2a/2b запускаются после приёмки Phase 1.

## 3. Файловая матрица

| Владелец | Файлы |
|---|---|
| W39a | `waves/RW-39/DESIGN/{REFERENCES.md,tokens.json,layout.json,README.md}` |
| W39b | `waves/RW-39/DESIGN/{SPEC.md,tokens.css,DOM-CONTRACT.md}` |
| W39c | `rust/gui-spike/ui/index.html`, `ui/style.css`, `ui/components/*.css` |
| W39d | `rust/gui-spike/ui/app.js`, `ui/features/*.js` |
| W39e | `rust/gui-spike/src/data.rs`, `src/lib.rs`, `src/main.rs`, `tests/data_contract.rs` |
| QA-1 | `waves/RW-39/{run-mutations*.py,mutations*.log}` |
| REV-1 | `waves/RW-39/REVIEW*.md` |
| PM | `waves/RW-39/{TZ,RUNBOOK,METRICS,REPORT}.md`, интеграция, гейт |

`ui/model.js`, `ui/index.html`/`style.css` до Phase 2 не трогаются. `waves/RW-36/37/38/*` —
read-only.

## 4. DOM-контракт (W39b фиксирует до кода)

Обязательные id/роли (сохранить совместимость с тестами RW-38):
`#screen-chat|emoji|members|settings`, `#chat-list`, `#member-list`, `#emoji-grid`,
`#emoji-tabs`, `#emoji-search`, `#settings`, `#bench`, `.rail-btn[data-screen]`,
`.screen`, `.screen-title`; ARIA: `role=list/listitem/tab/gridcell/region`, `aria-label`,
`aria-selected`, `aria-current`. Новые элементы shell: `#guild-rail`, `#channel-tree`,
`#chat-header`, `#composer`, `#member-panel`, `#user-bar`.

## 5. DoD

1. `REFERENCES.md` + `tokens.json` + `layout.json` с провенансом; токены — значения, не ассеты.
2. `SPEC.md` + `tokens.css` + `DOM-CONTRACT.md`; 4 экрана + shell; состояния/размеры.
3. Верстка визуально соответствует референсу (rail 64–72, tree ~240, chat, members ~240,
   header ~48, composer ~56; цвета/типографика из токенов), системные шрифты/emoji.
4. Frontend-функции работают на новых элементах; DOM/AT-SPI тесты зелёные; `loadedRows≤500`.
5. Backend BFF отдаёт данные из `fixtures/` без сети; контрактные тесты; ошибки типизированы.
6. `npm run gate:worker` passed (0 skip на дисплее), `cargo test --workspace` 0 failed,
   clippy/fmt 0; мутации по новым инвариантам; ревью P0/P1=0.
