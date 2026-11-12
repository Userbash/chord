# task-262 — Guild Onboarding Prompts & Welcome Screen Wizard Controller (`onboarding-wizard.js`, FE-P0-19)

- **Карточка / Ветка**: `task-262` (`feature/task-262-ui-onboarding-wizard`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракул**: `reference-oracle/rest-static.json` (`GUILD_ONBOARDING`, `GUILD_WELCOME_SCREEN`; до 5 каналов приветствия, описание до 140 символов, обязательные вопросы адаптации)

## Сделано
1. `tests/unit/onboarding-wizard.test.mjs` (`// @ts-check`, Strict TDD: Red → Green, 8 unit-тестов в 3 suites):
   - проверка экспорта констант `MAX_WELCOME_CHANNELS = 5` и `MAX_WELCOME_DESCRIPTION_CHARS = 140`;
   - проверка `isValidWelcomeDescription(text)` на граничных длинах (`1`, `140`, `0`, `141`), пробельных строках и нестроковых значениях;
   - проверка `areRequiredPromptsSatisfied(prompts, selectedOptionIds)` для `Array` и `Set`, обязательных/необязательных вопросов и ограничения `singleSelect`;
   - проверка `createOnboardingWizardController(options)`:
     - рендер корневого региона `[data-onboarding-wizard]` (`role="region"`, `aria-label="Приветствие и адаптация сервера"`);
     - рендер до 5 приветственных каналов `[data-welcome-channel="<channelId>"]` и вызов `onOpenWelcomeChannel(channelId)` при клике;
     - рендер вопросов адаптации `[data-onboarding-prompt="<promptId>"]` и кнопок опций `[data-onboarding-option="<optionId>"]` с `aria-pressed="true"|"false"`, переключение выбора при клике и снятие предыдущего выбора внутри вопроса при `singleSelect === true`;
     - рендер кнопки завершения `[data-onboarding-complete-btn]` и блока ошибки `[data-onboarding-error]`: вывод ошибки при невыполнении `areRequiredPromptsSatisfied` и очистка ошибки + вызов `onCompleteOnboarding(selectedOptionIds)` при успешной проверке;
     - проверка `{ setState(patch), getState(), destroy() }` и защиты состояния от внешней мутации.
2. `rust/gui-spike/ui/features/onboarding-wizard.js` (`// @ts-check` + JSDoc, ESM):
   - реализованы и экспортированы `MAX_WELCOME_CHANNELS`, `MAX_WELCOME_DESCRIPTION_CHARS`, `isValidWelcomeDescription`, `areRequiredPromptsSatisfied` и `createOnboardingWizardController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/onboarding-wizard.test.mjs` → `exit=1` (`ERR_MODULE_NOT_FOUND`, `tests 1, pass 0, fail 1`), лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-262-ui-onboarding-wizard/red.log`.
- **Green**: `node --test tests/unit/onboarding-wizard.test.mjs` → `exit=0` (`tests 8, suites 3, pass 8, fail 0`), лог сохранён в `/home/user/orca/project/waves/RW-40/G2/task-262-ui-onboarding-wizard/green.log`.
- **Typecheck**:
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `exit=0` (0 ошибок);
  - `./node_modules/.bin/tsc --ignoreConfig --noEmit --strict --allowJs --checkJs --noUncheckedIndexedAccess --target es2022 --module node20 --moduleResolution node16 --types node rust/gui-spike/ui/features/onboarding-wizard.js tests/unit/onboarding-wizard.test.mjs` → `exit=0` (0 ошибок).
- **Gate**: полный `gate:worker` не запускался (режим ТЗ воркера в batch); чужие файлы не затронуты.

## Файлы и md5
```
18ba4d9a9404c5979b814363ed1e8de2  rust/gui-spike/ui/features/onboarding-wizard.js
b47a7a25207dd47cec83e584260afe76  tests/unit/onboarding-wizard.test.mjs
381bfcd66112704578350a18c9c8d50b  /home/user/orca/project/waves/RW-40/G2/task-262-ui-onboarding-wizard/red.log
f634619fc679a4fbb44ea99af90b655d  /home/user/orca/project/waves/RW-40/G2/task-262-ui-onboarding-wizard/green.log
```

## Риски
- Контроллер не зависит от глобального `window.document`, если `options.container` предоставляет `ownerDocument.createElement` или `createElement`.
- `git commit` не выполнялся в соответствии с правилами воркера.
