# Отчёт воркера — TASK-989 (`G4 QA`)

- **Карточка:** `TASK-989` — Автоматизированный нагрузочный стенд на 50 одновременных ботов/клиентов (`5000 событий/с`)
- **Волна:** `RW-46` (`QUALITY`)
- **Задача Orca:** `task_ce857c1e5d67` (`ctx_3384a6c972d7`)
- **Ветка:** `feature/sprints-1-5-build-004`
- **Статус:** `Ready for Product Review`

---

## 1. Сделано

1. **Строгий TDD (`RED` → `GREEN` → `5/5 Mutations`):**
   - Сначала разработан контрактный и модульный тест [`tests/unit/multi-client-stress-989.test.mjs`](file:///home/sanya/orca/project/tests/unit/multi-client-stress-989.test.mjs) (`SUB-989-0`..`SUB-989-5`) и зафиксирован `RED`-прогон до создания скрипта (`waves/RW-46/QUALITY/task-989-multi-client-stress/red.log` — `6 fail`).
2. **Нагрузочный стенд 50 пиров ([`scripts/stress-test-50-peers.mjs`](file:///home/sanya/orca/project/scripts/stress-test-50-peers.mjs)):**
   - Реализована детерминированная топология из `50` одновременных ботов/клиентов (`createPeerRoster`, уникальные `Snowflake` ID и `SSRC 17843..17992`), генерирующая поток `5000 событий/с` в 4 категориях: всплески сообщений (`MESSAGE_CREATE` с кольцевым буфером `MAX_CHANNEL_MESSAGES_WINDOW = 200`), синхронные реакции 50 участников (`MESSAGE_REACTION_ADD` / `MESSAGE_REACTION_REMOVE` с защитой от дублей), индикаторы набора текста (`TYPING_START` с `TYPING_TTL_MS = 10_000` и автосбросом при отправке сообщения) и голосовое присутствие / переходы говорящих (`VOICE_STATE_UPDATE`, `SPEAKING` op `5` с блокировкой при `selfMute`/`selfDeaf`/`suppress`).
   - Проверен нулевой jank UI-потока при 120 FPS (`frameBudgetMs = 8.33`, `jankFrames = 0`, `p99FrameTimeMs <= 8.33`), соблюдение бюджета памяти `RSS <= 60 МБ` (`rssDeltaMb = 6.36 МБ`, `estimatedClientRssMb = 11.37 МБ`), сверка с независимыми RE-оракулами (`waves/kanban-board.json`, `fixtures/gateway/by-type/023-MESSAGE_CREATE.json`, `fixtures/gateway/summary.json`, `fixtures/voice/rtc-frames.jsonl`), все 9 измерений `TEST-POLICY.md` и гейт мутаций `5/5` (`100%` kill rate).

---

## 2. RED-прогон

Команда:
```sh
node --test tests/unit/multi-client-stress-989.test.mjs
```
Лог: [`waves/RW-46/QUALITY/task-989-multi-client-stress/red.log`](file:///home/sanya/orca/project/waves/RW-46/QUALITY/task-989-multi-client-stress/red.log) (`tests 6 | pass 0 | fail 6`).

---

## 3. GREEN-прогон и Targeted Gate (`T0`)

Команды:
```sh
node scripts/stress-test-50-peers.mjs --out-json waves/RW-46/QUALITY/task-989-multi-client-stress/stress-summary.json --out-md waves/RW-45/QUALITY/MULTI-CLIENT-STRESS-REPORT.md
node --test tests/unit/multi-client-stress-989.test.mjs
```
Лог: [`waves/RW-46/QUALITY/task-989-multi-client-stress/green.log`](file:///home/sanya/orca/project/waves/RW-46/QUALITY/task-989-multi-client-stress/green.log):
- `[TASK-989] 50-Peer Stress Test PASS: peers=50, rate=5000 ev/s, jank=0, rssDelta=6.36MB`
- `tests 6 | suites 1 | pass 6 | fail 0` (`138.57 ms`)

---

## 4. Мутационное тестирование (`5/5 KILLED`)

Лог: [`waves/RW-46/QUALITY/task-989-multi-client-stress/mutation.log`](file:///home/sanya/orca/project/waves/RW-46/QUALITY/task-989-multi-client-stress/mutation.log)
- `M1` (`STRESS_DEFAULT_CONFIG.peerCount: 50 -> 25`): **KILLED** (`SUB-989-1`, `SUB-989-2`, `SUB-989-4`, `SUB-989-5`)
- `M2` (`STRESS_DEFAULT_CONFIG.targetEventsPerSec: 5_000 -> 2_500`): **KILLED** (`SUB-989-1`, `SUB-989-4`)
- `M3` (`#handleSpeaking: allow speaking while muted`): **KILLED** (`SUB-989-3`, `SUB-989-5`)
- `M4` (`evaluateStressSimulationResult: rssDeltaMb > maxRssMb -> >= maxRssMb`): **KILLED** (`SUB-989-4`, `SUB-989-5`)
- `M5` (`#handleReactionAdd: bypass duplicate reaction check`): **KILLED** (`SUB-989-3`, `SUB-989-5`)
- Итог: **`5/5` мутаций убито (`100%`)**, исходный файл восстановлен побайтово (`MD5 match=true`).

---

## 5. Файлы и MD5

| Файл | MD5 |
|---|---|
| `scripts/stress-test-50-peers.mjs` | `b1b5e54f6a596679fe9c1ccc49023d18` |
| `tests/unit/multi-client-stress-989.test.mjs` | `8f9cb9365d8a40bc58c3c8b758f72ba4` |
| `waves/RW-45/QUALITY/MULTI-CLIENT-STRESS-REPORT.md` | `6bbabf712d2aef2c0a0742592c73d4b2` |
| `waves/RW-46/QUALITY/task-989-multi-client-stress/stress-summary.json` | `243457e8c6053bf8c48903d11b2c905c` |
| `waves/RW-46/QUALITY/task-989-multi-client-stress/red.log` | `03f2a4c87a79048b71900eee2ebff743` |
| `waves/RW-46/QUALITY/task-989-multi-client-stress/green.log` | `219f457883b6ed44428a3683c83f10cb` |
| `waves/RW-46/QUALITY/task-989-multi-client-stress/mutation.log` | `6f4f4b83940709b480479c4c131610cd` |

---

## 6. Риски и ограничения

- Кольцевое окно сообщений (`MAX_CHANNEL_MESSAGES_WINDOW = 200`) и TTL-очистка индикаторов набора текста (`10 000 мс`) предотвращают неограниченный рост кучи при длительном потоке `5000 событий/с`.
- Чужие файлы в рабочем дереве не затрагивались.
