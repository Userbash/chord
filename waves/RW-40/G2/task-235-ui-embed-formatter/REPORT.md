# task-235 — FE-P2-27: Message Rich Embed Limits & Safe URL Validator

Batch: RW-40 / G2. Branch: feature/task-235-ui-embed-formatter (base unstable).
Status: implemented, tests green, no commit made.

## Что сделано
- `rust/gui-spike/ui/features/embed-formatter.js` (`// @ts-check` + JSDoc):
  - `EMBED_LIMITS` — frozen константы (10 embeds, 256 title, 4096 description, 25 fields,
    256 field name, 1024 field value, 2048 footer).
  - `isSafeEmbedUrl(url)` — только абсолютные `https:`/`http:` через `new URL()`;
    отклоняет `javascript:`, `data:`, `file:`, `mailto:`, относительные пути, пустые и не-строки.
  - `normalizeEmbedForRender(raw)` — обрезка title/description/footer.text, `url` → `null`
    при небезопасном значении, цвет `#rrggbb` при integer `0..0xFFFFFF` иначе `#2b2d31`,
    `fields` до 25 с обрезкой name/value, вход не мутируется, мусорный вход даёт пустой embed.
- `tests/unit/embed-formatter.test.mjs` — 10 тестов: лимиты, безопасные/опасные URL,
  не-строки, обрезка, цвет, поля, пустой вход, отсутствие мутаций.

## Red -> Green
- red.log: exit=1, `pass 0 / fail 1` (модуль ещё отсутствовал).
- green.log: exit=0, `tests 10 / pass 10 / fail 0`.

## Файлы и md5
- rust/gui-spike/ui/features/embed-formatter.js — 8571b347913aa320e2affdaac4b44ec3
- tests/unit/embed-formatter.test.mjs — ea130729e0557bd8d7dd0e869e0bf5c3

## Риски и заметки
- Гейт `gate:worker` и полный срез не запускались: режим token-lean, только targeted-тест.
- Не-строковые title/description отбрасываются в `''` (без коэрции числа в DOM-текст).
- Мутация `5/5` — на приёмке Technical Reviewer.
