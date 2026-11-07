# RW-40 / G2 / task-199 — UI invite-link parser

Карточка: task-199. Ветка: feature/task-199-ui-invite-link-parser.
Scope: `rust/gui-spike/ui/features/invite-parser.js`, `tests/unit/invite-parser.test.mjs`.

## Сделано
- `MAX_INVITE_EMBEDS = 3`.
- `extractInviteCodes(text)`: формы `https://discord.gg/<code>`, `http://discord.gg/<code>`,
  `discord.gg/<code>`, `https://discord.com/invite/<code>`, `https://discordapp.com/invite/<code>`,
  код `[A-Za-z0-9-]{2,32}`. Дедуп с сохранением порядка, обрезка до 3.
- `formatInviteSummaryRu(invite)`: недействительное/истёкшее приглашение даёт фиксированный
  русский набор строк; иначе title из `guildName` с fallback `Сервер Discord`, subtitle со
  счётчиками (нормализация `max(0, trunc(...))`, мусор -> 0), кнопка `Перейти`/`Вступить`.

## Прогоны
- red: `red.log` — `node --test tests/unit/invite-parser.test.mjs` падал (ERR_MODULE_NOT_FOUND, модуль ещё не создан).
- green: `green.log` — `tests 14, pass 14, fail 0` (T0, только этот файл).

## Замечание
Первый вариант теста «все формы ссылок» ожидал 5 кодов в одном тексте, что противоречит
лимиту 3. Тест переписан по формам + отдельный кейс на три формы; реализация не ослаблялась.

## Файлы и md5
- `rust/gui-spike/ui/features/invite-parser.js` — `8a311b18999df034b35ea1e26bd68b3f`
- `tests/unit/invite-parser.test.mjs` — `0099b770c15ca5d209bfbc1f1bc82148`

## Не сделано
- Полный `gate:worker` не запускался (TOKEN-LEAN, T0).
- Интеграция парсера в рендер сообщений не входила в scope. Git commit не делался.
