# REVIEW-126A — PR-97 / task-214 (UI channel gate, G2)

**Verdict: APPROVED**

Режим: token-lean. Проверен только целевой тест-файл, `gate:worker` не запускался.
Ветка: `feature/task-214-ui-channel-gate`, HEAD `fb7af0623387b60523b96c5ca71a8044887df951`.

## Проверка

| Шаг | Результат |
| --- | --- |
| `node --test tests/unit/channel-gate.test.mjs` | exit 0, `tests 10 / pass 10 / fail 0` |
| md5 `rust/gui-spike/ui/features/channel-gate.js` | `e523199dc9a77608e58b65fa8538e723` — совпал с REPORT.md |
| md5 `tests/unit/channel-gate.test.mjs` | `7b34389929079db6074eb34c52521744` — совпал с REPORT.md |
| md5 после restore | `e523199dc9a77608e58b65fa8538e723` — MATCH, файл восстановлен байт-в-байт |

## Мутации (5/5 убиты)

Каждая мутация: точечная правка `channel-gate.js`, прогон целевого теста, откат к оригиналу.

| # | Мутация | Прогон | Убита |
| --- | --- | --- | --- |
| M1 | инверсия подтверждения: `nsfw && !accepted` → `nsfw && accepted` | exit 1, pass 7 / fail 3 | да (age gate: NSFW без подтверждения; age gate: с подтверждением открыт) |
| M2 | утечка подтверждения между каналами: `accepted = id !== "" && acknowledged.includes(id)` → `accepted = acknowledged.length > 0` | exit 1, pass 9 / fail 1 | да (age gate: подтверждение другого канала не помогает) |
| M3 | отсутствующий `nsfw` считается гейтом: `channel?.nsfw === true` → `channel?.nsfw !== false` | exit 1, pass 9 / fail 1 | да (age gate: обычный канал и отсутствие канала открыты) |
| M4 | снят обход для модератора: условие `threadLocked === true && canManageThreads !== true` → `threadLocked === true` | exit 1, pass 9 / fail 1 | да (composer: модератор может писать в заблокированную ветку) |
| M5 | сломан приоритет: блокировка ветки пропускается при `canSendMessages === false` | exit 1, pass 9 / fail 1 | да (composer: приоритет блокировки ветки над отсутствием прав) |

Итог: **5/5 (100%)**, выживших мутаций нет.

## Замечания (неблокирующие, P2–P3)

- P2: строки захардкожены по-русски, i18n-ключей и сверки с RE-дампом нет — сам воркер зафиксировал это в
  разделе «Риски». Для G2-слоя приемлемо, но перед релизом нужна привязка к каталогу ключей.
- P3: `canSendMessages: undefined` трактуется как разрешение (блокирует только явный `false`). Контракт ТЗ это
  допускает, тест на `undefined` отсутствует — поведение стоит зафиксировать при появлении реальных данных capabilities.
- P3: проверен только целевой тест; полный `gate:worker` остаётся за интеграционной линией.

`git commit` не выполнялся. Файл `channel-gate.js` после мутаций восстановлен, md5 совпадает с REPORT.md.
