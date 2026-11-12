# RW-39 / HANDOFF — дизайн, frontend, backend (P3-02 «чат-вью»)

Дата: 2026-10-23. PM: DeepSeek. Передаётся: дизайнеру (W39g), frontend (W39h),
backend (W39i). Все числа — из `DESIGN/tokens.css` / `layout.json` / `SPEC.md`; здесь только
навигация по данным и границы.

## 1. Референсы (read-only, не копировать ассеты)

| ID | Файл | Что смотреть | Куда идёт |
|---|---|---|---|
| R-01 | `reference-oracle/ui/00-app.png` | главный экран: рейл, дерево, чат, участники; 20+ новых, composer | shell + chat |
| R-02 | `ui/00-app.png` | цвета поверхностей (точечные пробы) | tokens (уже снято) |
| R-04 | `ui/00-app.png` | полоса «20+ новых сообщений» 32 px, `#5865f2` | unread divider |
| R-06 | `ui/06-profile-popout.png` | попаут профиля 268.8×408, `#2c2d32` | profile popout |
| R-08 | `ui8/22-members.jpg` + `22-members.json` | панель 256×1025, 35 строк, **6 групп**, шаг 43.2→42 | member groups |
| R-09/10 | `ui8/20a-search*.jpg`, `20b-search-filters.*` | поле 244×28; **7 секций**, 5 input, 9 опций | search overlay |
| R-11 | `ui8/20c-search-sort.*` | 3 пункта сортировки, `role=menu` | search overlay |
| R-12/13 | `ui8/21b-emoji-full.*`, `21d-emoji-categories.*` | 42 ячейки, 7 nav, **10 категорий**, placeholder | emoji |
| R-14/15 | `ui8/23-settings-voice.*`, `23-settings-categories.*` | «Голос» 1020×865, **38 switch**; модалка 1398×961, 63 строки | settings |
| R-16 | `ui/01-msg-menu.png` + `01-msg-menu-items.json` | **15 пунктов** меню сообщения (4 реакции + 11 действий) | message actions |
| R-17 | `ui/03-quickswitcher*.png` + results.json | 8 результатов, contenteditable | QuickSwitcher |
| R-18 | `ui/04-search-open.png`, `04b-search-results.png` | панель результатов («3 302», «49 887») | search results |

Полная таблица замеров и пробелов: `DESIGN/REFERENCES.md` §1–§7.
Машиночитаемый манифест (id → путь → размер → md5 → назначение): `DESIGN/ref-manifest.json`.
Ассеты не копируются: манифест ссылается на read-only дамп.

## 2. Замороженные данные (не переизмерять)

| Файл | Что внутри | Проверка |
|---|---|---|
| `DESIGN/tokens.css` | 202 токена, dark | `verify-tokens.mjs` → 57 match / 0 mismatch |
| `DESIGN/tokens.json` | машиночитаемая выжимка | `gen-tokens --check` |
| `DESIGN/layout.json` | 65 блоков, 4 экрана, source у каждого | `verify-layout.mjs` |
| `DESIGN/SPEC.md` | компоненты §7.1–7.15, высоты | — |
| `DESIGN/DOM-CONTRACT.md` | канонический `index.html`, id/роли, §9.5 D-04 | DOM 21/21 |
| `DESIGN/RE-EXTRA.md` | поведение из дампа (готовит W39f) | провенанс `[M]/[T]/[D]/[A]` |

## 3. Текущая реализация

- `rust/gui-spike/ui/index.html` — 173 строки, каноническое дерево.
- `ui/style.css` + `ui/components/*.css` — 9 модулей.
- `ui/app.js` + `ui/features/*.js` — 11 модулей (chat/emoji/members/settings/search/actions/…).
- `ui/model.js` — `PageStore` ≤500, переменные высоты 26/48/+22.
- `src/data.rs` — BFF `ready/messages_page/members_page/settings`, ошибки, пагинация.
- Тесты: **102/102** (dom 21, model 26, bench 45, features 10).

## 4. Что дизайнеру (W39g)

Собрать `DESIGN/SPEC-P3-02.md` по референсам R-01/R-04/R-08/R-16/R-18:

1. Анатомия сообщения: cozy/compact, group start 48 / continuation 26 / +22, hover-панель,
   gutter, avatar 32, имя/время, edited, reply-quote, reactions-row, embeds (link/image).
2. Day divider и unread divider (32 px, `#5865f2`), notice-полоса.
3. Композер: состояния empty/typing/disabled, кнопки, лимит, вложения (affordance).
4. Участники: 6 групп (роль/статус), заголовок группы 24, шаг 43, поиск.
5. Search overlay: 7 секций фильтров, сортировка, результаты, переход к сообщению.
6. QuickSwitcher: 8 результатов, клавиши.
7. Профиль-popout 268.8×408.

Правило: каждое число — с `[M]/[T]/[L]/[A]/[F]` и ссылкой; ассеты не копировать.

## 5. Что frontend (W39h)

Реализовать по `SPEC-P3-02.md` на текущем `model.js`:
- `features/chat.js` + новые модули (rows/embeds/reactions), `features/members.js` (группы),
  `features/search.js` (оверлей), `features/actions.js` (меню сообщения), `app.js` (wiring).
- CSS-правки — `components/chat.css`, `members.css`, `composer.css`.
- Тесты: расширить DOM/features без ослабления; 102 → больше, 0 skip.
- Не трогать `src/**`, `index.html` (только если новый id — через PM), `tokens.css`.

## 6. Что backend (W39i)

Расширить BFF под новые поля (только `fixtures/`, без сети):
- `messages_page`: `embeds`, `attachments`, `reactions`, `reply_to`, `edited_at`, day/cursor.
- `members_page`: `roles[]`, `group` (role/status), стабильный порядок.
- `search_page` (локальный по фикстурам) — если W39g зафиксирует контракт.
- Тесты: `data_contract.rs`, негативные кейсы, лимиты (см. D-10).
- Старт — **после** W39e2 (сейчас правит `data.rs`).

## 7. Порядок и гейты

1. W39f отдаёт `RE-EXTRA.md` → W39g спека (design gate: числа с source, ассетов нет).
2. W39g accepted → W39h (frontend) и W39e2/W39i (backend) параллельно, файлы не пересекаются.
3. Каждый lane: targeted-тесты + red/green логи. Общий gate — один раз на приёмке (E7).
4. Живое окно — **один** прогон на приёмку, кэш `window-budget.mjs`.

## 8. Пробелы дампа (честно)

- `07-guild-menu` — захват неудачный (0 items) → меню гильдии проектируем по `[A]`.
- `05-dm-list` — список ЛС пуст в дампе.
- `04b-search-results.json` пуст → числа результатов только с PNG.
- Геометрия меню сообщения и QuickSwitcher не снята (только состав).
- Светлая тема и компакт-режим в дампе не сняты → `[A]` до отдельной RE-сессии.
