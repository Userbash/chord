# REVIEW-129A — T-218 (PR-100) Markdown Spoiler / Codeblock / Timestamp Helper

Ревьюер: техническое ревью (DeepSeek, token-lean). Ветка: `feature/task-218-ui-markdown-helper`.
Дата: 2026-10-31.

## Verdict: APPROVED

## Проверка

| Шаг | Результат |
| --- | --- |
| `node --test tests/unit/markdown-helper.test.mjs` | exit 0, `tests 9 / pass 9 / fail 0` |
| md5 `markdown-helper.js` | `e28a346b7190efd9ba6f07f58ea2c117` — совпадает с REPORT.md |
| md5 `markdown-helper.test.mjs` | `e24ada3176fd228a1111d86f44900778` — совпадает с REPORT.md |
| Восстановление после мутаций | `cmp` с бэкапом идентичен, md5 исходника снова `e28a346b7190efd9ba6f07f58ea2c117` |

## Мутации (5/5 killed)

Каждая мутация применялась к `rust/gui-spike/ui/features/markdown-helper.js` по одной,
затем `node --test tests/unit/markdown-helper.test.mjs`, затем файл восстановлен из бэкапа.

| # | Мутация | Убивший тест | Итог |
| --- | --- | --- | --- |
| M1 | `DEFAULT_TIMESTAMP_STYLE` `"f"` → `"t"` | parseDiscordTimestampTag разбирает валидные теги | killed (fail 1) |
| M2 | Порядок `TIMESTAMP_STYLES`: `"d","D"` → `"D","d"` | TIMESTAMP_STYLES заморожен и совпадает с клиентом | killed (fail 1) |
| M3 | Убран `.trim()` у спойлера (`text.slice(i+2, closeStart)`) | extractSpoilerSpans собирает непустые спойлеры | killed (fail 1) |
| M4 | Граница секунд `abs < 60` → `abs <= 60` | formatRelativeTimestampRu — минуты и часы | killed (fail 1) |
| M5 | Дни `Math.floor` → `Math.ceil` | formatRelativeTimestampRu — дни | killed (fail 1) |

Итог: **5/5 mutations killed**, каждая падает ровно на 1 тесте, остальные 8 проходят.

## Замечания (не блокирующие, P2/P3)

- Codeblock (` ``` `) не парсится отдельно — в ТЗ заявлена только независимость спойлеров от inline-кода; зафиксировано в REPORT.md как ограничение.
- Модуль пока не подключён к рантайму рендера — вне write-scope задачи.
- Полный `gate:worker`, `tsc`, `cargo`, e2e не прогонялись (token-lean режим) — нужен прогон в integration-лейне.

## Ограничения проверки

Мутации проверяют только поведение, покрытое 9 unit-тестами; контракт с живым клиентом Discord
и интеграция в WebView этим ревью не подтверждены.

Блокирующих замечаний: `P0 = 0`, `P1 = 0`.
