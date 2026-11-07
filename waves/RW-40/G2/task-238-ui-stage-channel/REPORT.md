# task-238 — Voice Stage Channel & Speaker Queue Controller (FE-P1-26)

## Сделано
- `rust/gui-spike/ui/features/stage-channel.js` (`// @ts-check`):
  - Экспортирован замороженный словарь ролей `STAGE_ROLES = Object.freeze({ SPEAKER: "speaker", AUDIENCE: "audience", REQUESTING: "requesting" })`.
  - Экспортирована чистая функция `partitionStageParticipants(participants)`: разбивает массив участников на `{ speakers, requesting, audience }` с сортировкой по `displayName.localeCompare("ru")` (и фолбэком по `userId`), неизвестные роли по умолчанию направляет в `audience`, входной массив не мутирует.
  - Экспортирован контроллер `createStageChannelController(options)`:
    - рендерит корень `[data-stage-channel]` с `role="region"` и `aria-label="Трибуна"`;
    - рендерит заголовок `[data-stage-topic]` с темой или фолбэком `"Трибуна без темы"`;
    - рендерит секцию `[data-stage-speakers]` (`"Спикеры (N)"`) с элементами `[data-stage-speaker-id="<userId>"]`, индикатором `[data-speaking="true"]` при `speaking === true` и кнопкой `[data-move-to-audience="<userId>"]` при `isModerator === true`;
    - рендерит секцию `[data-stage-requests]` (`"Запросы на выступление (N)"`) с кнопкой `[data-approve-speaker="<userId>"]` при `isModerator === true`;
    - рендерит секцию `[data-stage-audience]` (`"Слушатели (N)"`) с элементами `[data-stage-listener-id="<userId>"]`;
    - рендерит нижнюю панель управления с кнопками `[data-stage-hand-btn]` (`aria-pressed="true"` для `role === "requesting"`, переключение вызывает `onToggleHand(nextRaised)`) и `[data-stage-leave-btn]` (`"Покинуть трибуну"`, клик вызывает `onLeaveStage()`);
    - возвращает `{ setState(patch), getState(), destroy() }`.
- `tests/unit/stage-channel.test.mjs`: 8 unit-тестов по контракту (Strict TDD: Red -> Green).

## Прогоны
- **Red**: `node --test tests/unit/stage-channel.test.mjs` -> `exit=1` (`ERR_MODULE_NOT_FOUND`, `pass 0 / fail 1`) — лог сохранён в `red.log`.
- **Green**: `node --test tests/unit/stage-channel.test.mjs` -> `exit=0` (`tests 8 / pass 8 / fail 0`) — лог сохранён в `green.log`.
- **Typecheck**: `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` -> `exit=0` (0 ошибок).

## Файлы и md5
- `rust/gui-spike/ui/features/stage-channel.js` — `7ff1263e780bf4dcd71e4c0349957a0a`
- `tests/unit/stage-channel.test.mjs` — `c492e4317e3497b66c7c0024beeb34cd`

## Риски
- Контроллер не зависит от глобального `window.document`, если `options.container` предоставляет `ownerDocument.createElement` или `createElement`.
- `git commit` не выполнялся в соответствии с правилами воркера; изменения находятся в рабочем дереве ветки `feature/task-238-ui-stage-channel`.
