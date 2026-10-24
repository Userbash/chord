# STUB-MAP — карта заглушек и сверка с эталоном

Дата: 2026-10-24. PM. Основание: `PARITY/PARITY-MATRIX.md` (91 функция),
`QUALITY/G1-AUDIT.md`, `G2-AUDIT.md`, `TEST-AUDIT.md`, код `rust/gui-spike/ui`.
Источник истины о функции — только `[L]/[S]/[D]`; `[M]/[A]` — дизайн/неизвестно.

## 0. Легенда

| Код | Значение |
|---|---|
| `STUB-UI` | кнопка/панель видна, но обработчик — заглушка |
| `STUB-DATA` | экран работает на fixtures/synthetic, не на live |
| `STUB-ABSENT` | функции нет, UI отсутствует |
| `STUB-TEST` | тест/CLI/e2e — заглушка или skip |
| P | P0 stop-line, P1 daily parity, P2 secondary, P3 backlog |
| Urgency | Now (P0/P1), Next (P1/P2), Later (P2/P3) |

## 1. Видимые заглушки UI

| ID | Где | Сейчас | Эталон / evidence | Функция-цель | P | Urgency | Owner |
|---|---|---|---|---|---|---|---|
| STUB-01 | `#guild-rail` | 3 статичные кнопки | `G-01`, `UI6:01` [M] | список/выбор серверов из READY | P1 | Now | G2+G1 |
| STUB-02 | `data-action="guild-menu"` | только `aria-expanded` | `G-05`, `UI2:D1` [M] | поповер: 10 пунктов, права, клавиатура | P1 | Now | G2+G3 |
| STUB-03 | `+` сервера | нет поведения | `G-02`, `S162:1682` [D] | create/join; endpoint требует RE | P3 | Later | G1+G2+G3 |
| STUB-04 | preview/enter guild | backend `G-03a` готов, UI нет | `G-03a`, `D02:101,138` [L] 204 | кнопка входа из preview + список | P1 | Now | G2+G1 |
| STUB-05 | join by invite | нет UI/backend | `G-03`, `NC:1.1#1` [S]; коды [A] | модалка инвайта, ошибки | P1 | Next | G1+G2 |
| STUB-06 | leave server | backend в работе | `G-04`, `NC:1.1#4` [S] | confirm → leave → список | P1 | Now | G1+G2 |
| STUB-07 | invite create/list/revoke | нет | `G-06`, `NC:1.1#8-11` [S] | модалка/копирование/отзыв | P2 | Next | G1+G2 |
| STUB-08 | `#channel-tree` | 3 канала, 2 категории | `C-01`, `UI6:01` [M] | категории, text/voice/forum, live | P1 | Now | G2+G1 |
| STUB-09 | switch channel | фиксированный `news` | `C-02`, `D2:70,180` [L] | выбор → история → restore | P1 | Now | G2+G1 |
| STUB-10 | pins/threads | `aria-expanded` без панели | `C-04/M-10/M-11`, `D2:109-110` [L] | панели, empty/error, jump | P2 | Next | G2+G1 |
| STUB-11 | inbox `🔔` | `aria-disabled` | `N-03`, `CAT` [M] | inbox/mentions, счётчики, jump | P2 | Later | G2+G1 |
| STUB-12 | gift `🎁` | `aria-disabled` | **нет G6-строки**; кнопка есть в UI | RE: endpoint/поведение в дампе | P3 | Later | G6+G2 |
| STUB-13 | attach `＋` | `aria-disabled` | `AT-01…03`; upload [D]/[A] | upload/прогресс/превью | P2 | Blocked | G1+G2 |
| STUB-14 | search filters | 3 серверных, 6 локальных | `M-13`, `F162` [L] | подтверждённые фильтры + live | P1 | Next | G1+G2 |
| STUB-15 | members | 24k synthetic | `MB-01`, `F162:55-62` [M] | live-члены, роли, сортировка | P1 | Next | G1+G2 |
| STUB-16 | profile popover | stub aria | `MB-03`, `UI5:04` [M] | карточка, роли, действия | P2 | Next | G2 |
| STUB-17 | mute/deafen | локальный `aria-pressed` | `V-07/V-08`, `F162:63-83` [M] | реальные voice-state + UI | P1 | Later | G5+G2 |
| STUB-18 | voice panel | нет | `V-08`, `VUI:01` [M] | join/leave, участники, ping | P1 | Later | G2+G3 |
| STUB-19 | settings 15 разделов | disabled | `ST-01…ST-08` | контент по разделам | P2/P3 | Later | G2+G3 |
| STUB-20 | QR/passkey | disabled | `A-04/A-05` [S] | QR/WebAuthn | P2 | Later | G1+G5 |
| STUB-21 | read state/unread | статичные классы | `M-07/C-03/G-08` [L] | ack, счётчики, сброс | P1 | Next | G1+G2 |
| STUB-22 | typing/presence | core есть, UI нет | `P-01/P-02/P-03` [L]/[S] | статусы и «печатает…» | P1 | Next | G1+G2 |

## 2. Заглушки данных и сети

| ID | Слой | Сейчас | Эталон / evidence | Что заменить | P | Owner |
|---|---|---|---|---|---|---|
| STUB-23 | history | fixtures + PageStore | `M-01`, `D2:70,180` [L] | REST-страницы/курсоры | P1 | G1+G2 |
| STUB-24 | guild list | демо-рейл | `G-01`, `READY.guilds` [L] | READY → rail, restore | P1 | G1+G2 |
| STUB-25 | members | synthetic | `MB-01` [M] | member list/chunk, роли | P1 | G1+G2 |
| STUB-26 | voice session | только crate/parsers | `V-01…V-06` [L] | RTC/UDP/SRTP/Opus | P1 | G1+G5 |
| STUB-27 | video/stream | нет | `S-01…S-04` [L]/[A] | capture/encode/RTP/Go Live | P2 | G5+G1 |
| STUB-28 | notifications | D-Bus crate не подключён | `N-01/N-02` [S] | notify/DND/click→channel | P2 | G5+G2 |
| STUB-29 | settings sync | локальный JSON | `ST-08`, `D2:95,115` [L] | settings-proto sync | P2 | G1 |
| STUB-30 | i18n | crate не подключён | `ST-04`, `I162` [S] | t()/ru/en/plural | P2 | G1+G2 |

## 3. Заглушки тестов и инструментов

| ID | Где | Сейчас | Что нужно | P | Owner |
|---|---|---|---|---|---|
| STUB-31 | `src/cli/run.mjs` | `walk` — заглушка | fixture walk + JSONL/скриншоты | P1 | G4 |
| STUB-32 | `cdp` CLI | только `targets` | `eval/clicktext/list/shot` | P1 | G4 |
| STUB-33 | e2e | 4/4 skip | fixture-прогон без live; skip ≠ pass | P1 | G4 |
| STUB-34 | gate | нет workspace/CLI | `cargo test --workspace`, CLI coverage | P1 | G4 |
| STUB-35 | token test | source-scrape | поведенческий keyring→Network | P1 | G1+G4 |

## 4. Сверка с эталоном: что нельзя выдумывать

| Область | Подтверждено | Не подтверждено |
|---|---|---|
| Guild join | `PUT ...?lurker=true` 204 `[L]` | `POST /invites/{code}` коды/TTL `[A]` |
| Guild leave | `DELETE ...` + `{lurking}` `[S]` | коды/guest-семантика `[A]` |
| Members search | endpoint `[S]`, 202 retry | pagination `[A]` |
| Attachments | path `[D]` | multipart/поля/лимиты `[A]` |
| Voice | op2/op4/UDP/SRTP `[L]` | входящее медиа, AAD/nonce, op5/11/13 `[A]` |
| Video/Go Live | rid/quality/AV1 частично | `STREAM_*`, viewer, Linux capture `[A]` |
| DAVE | op21/22/24/25/27/29/30 `[S]` | пороги/epoch/2-client `[A]` |

## 5. Итог

- **Видимых UI-заглушек:** 22.
- **Сетевых/данных:** 8.
- **Тестовых/CLI:** 5.
- **Всего карточек:** 35; часть пересекается с G6 (G-01…V-10).
- Приоритет замены: **P1 — 22, P2 — 11, P3 — 2**; внутри них
  **blocked — 1** (attachments), **unknown — 1** (gift, нужен RE).
