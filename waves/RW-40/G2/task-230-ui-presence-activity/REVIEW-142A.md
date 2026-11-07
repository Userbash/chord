# REVIEW-142A — PR-113 (T-230, FE-P2-25)

Ветка: `feature/task-230-ui-presence-activity`. Режим: token-lean (targeted unit + мутации, без `gate:worker`).

## Verdict: APPROVED

## Проверки

| Проверка | Результат |
| --- | --- |
| `node --test tests/unit/presence-activity.test.mjs` | exit 0, `tests 8 / pass 8 / fail 0` |
| Мутации | 5/5 killed |
| Восстановление файла после мутаций | md5 `404673c83e872004da699f8a88251bf0` = REPORT.md, повторный прогон exit 0 |
| md5 теста | `5ffd9d2f7dd8934a03bbdef8bbb8a06d` = REPORT.md |

## Мутации (5/5)

| # | Мутация | Файл/место | pass/fail | Итог |
| --- | --- | --- | --- | --- |
| M1 | префикс PLAYING `"Играет в "` → `"Играет "` | `formatActivityLineRu`, case PLAYING | 6/2 | KILLED |
| M2 | `MAX_CUSTOM_STATUS_CHARS` 128 → 64 | константа | 6/2 | KILLED |
| M3 | убран приоритет CUSTOM в `selectPrimaryActivity` (`const custom = null`) | `selectPrimaryActivity` | 7/1 | KILLED |
| M4 | убран `.slice(0, MAX_CUSTOM_STATUS_CHARS)` у CUSTOM | `formatActivityLineRu`, case CUSTOM | 7/1 | KILLED |
| M5 | `hasCustomContent`: `\|\|` → `&&` (emoji-only и state-only ломаются) | `hasCustomContent` | 7/1 | KILLED |

После каждой мутации файл возвращался из побайтовой копии; финальный md5 совпал с записанным в REPORT.md.

## Замечания (не блокирующие)

- P2: `selectPrimaryActivity` фильтрует `CUSTOM` без содержимого и затем ищет `custom` по всему списку — логика верна, но двойной проход; читаемость можно упростить при следующем касании файла.
- P3: подключение форматтера к строке участника (`members.js`) вне write-scope T-230 — остаётся на отдельную карточку.
- Полный `gate:worker` (fmt/clippy/workspace/e2e) не запускался по условиям token-lean режима; нужен в integration-лейне.

P0 = 0, P1 = 0. Мерж в `unstable` возможен после прогона полного гейта в integration-лейне.
