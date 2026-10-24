# N1-D — отчёт: меню сервера / join / leave / invite

Карточка: **N1-D** (вход в N1-02). Владелец: G3 Design. Дата: 2026-10-24.
Режим: только документы, код не менялся. Версия: **после PM-ревью** (evidence-правки).

## 1. Что исправлено по PM-ревью

| # | Замечание | Исправление |
|-|-|-|
| 1 | поле `confirmed` — не аргумент | Убраны все значения `confirmed` (true/false) как аргумент и из success-condition. Добавлено пояснение: в `rest-static.json` поле стоит в `false` у всех 904 записей (`NETWORK-CONTRACTS.md` §0.3). Достоверность — только `[L]/[S]/[A]`. |
| 2 | Join частично live-подтверждён | Разведены два действия: **A. accept invite** `POST /invites/{code}` с `{session_id, invite_instance_id, received_installation_id}` — `[S]`, коды/TTL `[A]`; **B. enter guild** `PUT /guilds/{id}/members/@me?lurker=true`, тело `{}`, **204 ×2** — `[L]` (`D02:101,138`). В UI это разные состояния/кнопки; success одного не выдаётся за другой. |
| 3 | Leave | `DELETE /users/@me/guilds/{id}` + `{lurking: boolean}` — `[S]`; ответ не читается `[S]`; коды и семантика lurker/guest — `[A]`. Утверждения «тело неизвестно» удалены; добавлен G-15 (риск 403 при неверном lurker/guest). |
| 4 | Успех UI | Только **2xx**: accept — 2xx (200 ожидается `[A]`), enter 204, leave любой 2xx. Поле `confirmed` не используется. Таймаут = «результат неизвестен», рейл не мутируется. |
| 5 | Геометрия модалок | §4.3–4.5 помечены **`unmeasured`**: значения — токены проекта `[T]` + `[A]`-сборка, не замер и не паритет. В JSON `dialog.measurement="unmeasured"`, `join_preview.measurement="unmeasured"`. `D1-guild-menu.png` остаётся `[M]`. |
| 6 | Единственный снятый экран | Оставлено: 37 PNG/JPEG, экранов join/leave нет, меню сервера — `ui2/D1-guild-menu.png`. |
| 7 | JSON/REPORT/md5 | Обновлены evidence-модель, статус, тесты (SM-67…69), пробелы G-01…G-17, хэши. |

Дополнительно исправлено: preview — это `GET /invites/{code}` `[S]` (не «отсутствует»);
инвайты create/list — `[S]` (не «blocked»); список серверов — только
`READY.guilds[]` + `READY_SUPPLEMENTAL`, REST-ручки нет `[S]`, `GUILD_CREATE` live не наблюдался.

## 2. Что найдено в дампе

| Артефакт | Что даёт | Метка |
|-|-|-|
| `ui2/D1-guild-menu.png` (2560×1383, dpr 1.25) | меню сервера: ширина 288 CSS, фон `#28282d`, блок группы 48 = 8+32+8, destructive-текст, радиус ≈8 | `[M]` |
| `ui2/D1-guild-menu-items.json` | 10 пунктов, порядок и тексты | `[D]` |
| `ui2/00-start.fingerprint.json` | `NAV aria="Боковая панель серверов"`, `group aria="Серверы"`, `treeitem aria="Добавить сервер"` | `[D]` |
| `ru-catalog.json` (23 836 ключей) | строки `cs48bs`, `Ts/9Ac`, `AVazqJ`, `5AkWAd`, `FDsl+J`, `33M5bg`, `7iPyVW` и др. | `[D]` |
| `ui/tokens.json` + `tokens.css` | 4795 переменных; `--modal-w-sm` 442, `--menu-pad` 8, `--row-menu-item` 32 | `[T]` |
| `docs/re/rest-d02.md` | `PUT /guilds/{id}/members/@me?lurker=true`, тело `{}`, **204 ×2** (`ready|57992.6989`, `run2|95974.7409`) | `[L]` |
| `NETWORK-CONTRACTS.md` §1 | accept-invite `[S]`, leave `[S]`, resolve/create/list invites `[S]`, список из `READY` `[S]/[L]` | `[S]` |
| 4 capture-файла | 0 запросов `invites/`, 0 `"DELETE"`; accept/leave live не наблюдались | `[D]` |
| `out/bundles/*` | ключи модалки только в двух чанках i18n, JSX-компонента join/leave нет | `[D]` |

**Экранов join/leave в дампе нет** — только контекстное меню. Поэтому геометрия модалок
`unmeasured`; это зафиксировано в §4.3–4.5 и в G-01.

## 3. Метки

| Метка | В документе | Что означает |
|-|-|-|
| `[T]` | 62 | токен проекта `tokens.css` |
| `[L]` | 14 | live capture (`rest-d02`) |
| `[S]` | 48 | код клиента (`NETWORK-CONTRACTS` §1) |
| `[M]` | 17 | пиксельный замер (меню `D1`) |
| `[D]` | 135 | факт дампа (DOM/JSON/строки/capture) |
| `[A]` | 195 | решение / оценка / неподтверждённое |

Всего 471 маркер (по сумме меток). Success-condition — только `2xx`, без `confirmed`.

## 4. Оставшиеся `[A]`-гэпы

| # | Гэп | Что нужно |
|-|-|-|
| G-01 | Геометрия модалок `unmeasured` (нет экранов) | живое окно → сверка чисел |
| G-02 | `POST /invites/{code}`: body `[S]`, HTTP-коды/TTL/error body `[A]`, live нет | live-прогон accept-invite |
| G-03 | `GET /invites/{code}`: `[S]`, live/коды `[A]` | live-проверка preview |
| G-04 | Invite list/create: `[S]`, live/коды `[A]` | live-проверка |
| G-12 | Таймаут 15 с и `retry_after` — наши числа | согласовать с N1-01 |
| G-15 | `lurking`/guest: клиент вычисляет `isLurking || isGuest`; коды leave `[A]` | фикстуры 204/403, различать lurker/guest |
| G-16 | `GUILD_CREATE` не наблюдался; список из `READY`; fallback `GET /guilds/{id}` `[D]` | решение по обновлению рейла после accept |
| G-17 | `received_installation_id` может быть `null`, TTL очистки `[A]` | не выдумывать TTL |
| G-05…G-11, G-13, G-14 | токен ширины меню, surface-3/surface-2, 2-строчный пункт, deep links, owner-flow, screening, иконки, row-menu-item, экран create | по владельцам из §9 документа |

`confirmed` в списке гэпов больше нет — это артефакт схемы, а не контракт.

## 5. Проверки

| Проверка | Результат |
|-|-|
| JSON валиден (`python3 -m json.tool`) | OK |
| Нет `confirmed` как аргумента (ни true, ни false) | PASS |
| Нет `confirmed` в success-condition | PASS |
| Все join/leave утверждения имеют `[L]/[S]/[A]` | PASS (accept S, enter L, leave S, коды A) |
| Нет внешних CSS-ссылок / шрифтовых подключений | PASS |
| Токены из JSON существуют в `tokens.css` | 35/35 |
| Геометрия модалок помечена `unmeasured` | PASS |
| Контраст WCAG AA | 7/7: danger-text/surface-3 7.24; text-strong/surface-3 14.18; muted/surface-2 5.31; white/danger-fill 4.85; white/accent 4.61 |
| Меню: 10 пунктов, тексты = дамп | PASS |
| `PUT …?lurker=true` + 204 в `rest-d02` | PASS |
| Код не менялся | PASS: изменения только в `waves/RW-40/DESIGN/` и `waves/RW-40/PM/n1d/` |

Валидатор: `python3 waves/RW-40/PM/n1d/validate.py` → `checks=111 failures=0 RESULT: OK`.

## 6. md5

| Файл | md5 |
|-|-|
| `waves/RW-40/DESIGN/SERVER-MENU.md` | `49e5186e3ce759a6b6bfc063492f6dfa` |
| `waves/RW-40/DESIGN/server-menu.json` | `a165fe3bfdc4867a14aa6e6419775901` |
| `waves/RW-40/PM/n1d/validate.py` | `80017a88b615255aff0c47384e7f4041` |
| `waves/RW-40/PM/n1d/validate.log` | `d3c4f4e9990a5be058725220e6b9f84d` |
| вход `NETWORK-CONTRACTS.md` | см. `md5.txt` |
| вход `docs/re/rest-d02.md` | см. `md5.txt` |
| вход `ui2/D1-guild-menu.png` | `8c24a205e5a17f02c0d48a68c47e1dc7` |
| вход `ru-catalog.json` | `09272320cecfa79d479a2b03df7adce1` |

## 7. Риски

| Риск | Влияние | Mitigation |
|-|-|-|
| accept-invite не проверен live, коды неизвестны | нельзя гарантировать обработку 403/404/429 | N1-01 live-прогон; до этого — фикстуры, коды `[A]` |
| Неверная семантика `lurking`/guest | leave может дать 403 или выйти «как lurker» | фикстуры 204/403; G-15 |
| Модалки не сверены | визуальное расхождение | `unmeasured`; после окна — только числа |
| `GUILD_CREATE` не наблюдался | рейл может не обновиться после accept | обновление из ответа/`READY`, fallback `GET /guilds/{id}` `[D]` |
