# REVIEW-153A — task-238 / FE-P1-26 Stage Channel & Speaker Queue Controller

**Карточка:** task-238 (FE-P1-26)
**Ревьюер:** Technical Reviewer (DeepSeek, токен-лимитный проход)
**Дата:** 2026-10-31
**Ветка:** `feature/task-238-ui-stage-channel`

## Verdict: APPROVED

## Область проверки

| Файл | md5 в REPORT.md | md5 по факту | Статус |
| --- | --- | --- | --- |
| `rust/gui-spike/ui/features/stage-channel.js` | `7ff1263e780bf4dcd71e4c0349957a0a` | `7ff1263e780bf4dcd71e4c0349957a0a` | совпадает |
| `tests/unit/stage-channel.test.mjs` | `c492e4317e3497b66c7c0024beeb34cd` | `c492e4317e3497b66c7c0024beeb34cd` | совпадает |

## Targeted-прогон

```
node --test tests/unit/stage-channel.test.mjs
tests 8 / pass 8 / fail 0 / skipped 0 / todo 0
```

## Мутационное тестирование 5/5 — убито 5 из 5

Мутации вносились точечно в `stage-channel.js`, после каждой — прогон `node --test tests/unit/stage-channel.test.mjs`.
Файл восстановлен из резервной копии, итоговый md5 совпал с `REPORT.md` побайтно.

| # | Мутация | Правка | Результат |
| --- | --- | --- | --- |
| M1 | Фолбэк темы всегда вместо реальной темы | `resolveStageTopicText`: `return trimmed;` -> `return DEFAULT_STAGE_TOPIC_RU;` | KILLED (`fail 2`) |
| M2 | Удалена сортировка очереди запросов | убрана строка `requesting.sort(compareStageParticipantsRu);` | KILLED (`fail 1`) |
| M3 | Инвертирован индикатор речи | `if (speaker.speaking === true)` -> `if (speaker.speaking !== true)` | KILLED (`fail 1`) |
| M4 | `aria-pressed` всегда `"false"` | `handBtn.setAttribute("aria-pressed", "false");` без тернарника | KILLED (`fail 1`) |
| M5 | Неизвестные роли уходят в спикеры | `normalizeStageRole`: `AUDIENCE` -> `SPEAKER` в фолбэке | KILLED (`fail 4`) |

Каждая мутация ломает минимум один независимый ассерт, тесты не «зеленят» код: набор чувствителен к сортировке, фолбэку темы, `data-speaking`, доступности кнопки руки и нормализации ролей.

## Соответствие ТЗ (выборочно)

- `STAGE_ROLES` заморожен, значения `speaker`/`audience`/`requesting` — совпадают с контрактом.
- `partitionStageParticipants` не мутирует вход (копирование через `{ ...item }`), сортирует `localeCompare("ru")` с фолбэком по `userId`.
- Регион `[data-stage-channel]` с `role="region"` и `aria-label="Трибуна"`; фолбэк темы `"Трибуна без темы"`.
- Секции `[data-stage-speakers]`, `[data-stage-requests]`, `[data-stage-audience]` со счётчиками `(N)`.
- Кнопки модератора `[data-move-to-audience]`, `[data-approve-speaker]` рендерятся только при `isModerator === true` и вызывают колбэки после обновления состояния.
- `[data-stage-hand-btn]` c `aria-pressed`, переключение вызывает `onToggleHand(nextRaised)`; `[data-stage-leave-btn]` вызывает `onLeaveStage()`.
- `setState`/`getState`/`destroy` ведут себя как в отчёте; `destroy` идемпотентно очищает контейнер.

## Замечания

Блокирующих нет (P0 = 0, P1 = 0).

- P3: `destroy()` после повторного `setState` опирается на `root.parentNode === container`; для контейнеров без `removeChild` срабатывает ветка `replaceElementChildren`, поведение корректное, но ветка не покрыта отдельным тестом.
- P3: дублирование логики построения строк `displayName` для трёх групп — кандидат на рефакторинг в следующем проходе, не блокирует приёмку.

## Ограничения прогона

Выполнялись только targeted-тесты модуля и мутации. `gate:worker`, `cargo`, `tsc` и `git commit` не запускались в этом проходе — по условиям токен-лимитного ревью; полный гейт остаётся за integration-лейном.
