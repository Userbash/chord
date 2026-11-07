# REVIEW-110A — task-199 UI invite-link parser

Вердикт: **APPROVED** (P0=0, P1=0, мутации 5/5).

## Проверено
- Code review `rust/gui-spike/ui/features/invite-parser.js`: парсер `INVITE_PATTERN` покрывает `discord.gg`,
  `discord.com/invite`, `discordapp.com/invite` со схемой и без; `{2,32}` по `[A-Za-z0-9-]`; дедуп с
  сохранением порядка; лимит `MAX_INVITE_EMBEDS = 3`; `formatInviteSummaryRu` даёт фиксированный набор
  строк для `null`/`expired`, fallback `Сервер Discord`, `max(0, trunc(...))`, кнопку `Перейти`/`Вступить`;
  чистые функции без DOM, ESM + `// @ts-check`, JSDoc.
- Targeted: `node --test tests/unit/invite-parser.test.mjs` — `tests 14, pass 14, fail 0`.
- md5 до/после мутаций совпадает с REPORT.md: `invite-parser.js 8a311b18999df034b35ea1e26bd68b3f`, `invite-parser.test.mjs 0099b770c15ca5d209bfbc1f1bc82148`; `git diff` по парсеру пуст.

## Мутации (каждая: правка → тест → падение → restore)
| # | Мутация | Результат |
|---|---|---|
| M1 | `MAX_INVITE_EMBEDS` 3→4 | KILLED, fail 2 |
| M2 | отключён дедуп (`if (true)`) | KILLED, fail 1 |
| M3 | код `{2,32}`→`{3,32}` | KILLED, fail 1 |
| M4 | `expired === true`→`expired === undefined` | KILLED, fail 6 |
| M5 | снят clamp `Math.max(0, ...)` | KILLED, fail 1 |

## Остаточные риски
- Полный `gate:worker` не запускался (TOKEN-LEAN, T0) — T1 остаётся за integration-лейном.
- Парсер не подключён к рендеру сообщений: интеграция вне scope карточки.
