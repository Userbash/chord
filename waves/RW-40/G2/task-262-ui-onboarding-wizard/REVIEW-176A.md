# REVIEW-176A — Technical Review PR-147 (task-262 / FE-P0-19)

- **Ревьюер**: Технический ревьюер (DeepSeek), token-lean режим
- **Ветка / коммит**: `feature/task-262-ui-onboarding-wizard` @ `94b91b1`
- **Объём**: `rust/gui-spike/ui/features/onboarding-wizard.js`, `tests/unit/onboarding-wizard.test.mjs`
- **Режим**: targeted-тест + 5 ручных мутаций. Полный `gate:worker` не запускался (по ТЗ), `git commit` не выполнялся, тестовый файл не изменялся.

## Вердикт: APPROVED

`P0 = 0`, `P1 = 0`, мутаций убито `5/5` (100%). Целевой тест зелёный, MD5 продуктового файла совпадает с `REPORT.md`.

## 1. Целевой прогон и сверка MD5

```
$ node --test tests/unit/onboarding-wizard.test.mjs
ℹ tests 8   ℹ suites 3   ℹ pass 8   ℹ fail 0   exit=0
```

| Файл | MD5 | Сверка с `REPORT.md` |
|---|---|---|
| `rust/gui-spike/ui/features/onboarding-wizard.js` | `18ba4d9a9404c5979b814363ed1e8de2` | совпадает |
| `tests/unit/onboarding-wizard.test.mjs` | `b47a7a25207dd47cec83e584260afe76` | совпадает |

## 2. Мутации (M1..M5)

Методика: исходник скопирован в бэкап, каждая мутация применялась отдельным `perl -0pi -e` точечно по одной строке, затем `node --test tests/unit/onboarding-wizard.test.mjs`. После каждой мутации файл восстанавливался из бэкапа побайтово (`cp`), MD5 сверялся с эталоном `18ba4d9a9404c5979b814363ed1e8de2` — расхождений нет ни на одном шаге. MD5 мутанта фиксировался отдельно, чтобы «убийство» не путалось с неприменённой правкой.

| # | Мутация | MD5 мутанта | exit | Вердикт | Убившие тесты |
|---|---|---|---|---|---|
| M1 | `MAX_WELCOME_CHANNELS = 5` → `6` | `d2a7609a2b09dd92fa74eecff1c3baa8` | 1 | KILLED | T-262-01, T-262-05 |
| M2 | `isValidWelcomeDescription`: верхняя граница `<= 140` → `< 140` | `d58017f6e3b658c2f58d0d2b4cf2955f` | 1 | KILLED | T-262-02 |
| M3 | `areRequiredPromptsSatisfied`: `matchedCount < 1` → `< 0` (обязательность не проверяется) | `51874b4b15456b549f8217a9f6c474dd` | 1 | KILLED | T-262-04, T-262-07 |
| M4 | `handleToggleOption`: убран сброс прежнего выбора при `singleSelect` | `382d389d94cbbc5af6a39e716c4fa823` | 1 | KILLED | T-262-06 |
| M5 | Кнопка завершения: `if (!areRequiredPromptsSatisfied(...))` → `if (false)` | `fd5a1f841838f8d27800f340bb32bcba` | 1 | KILLED | T-262-07 |

Итог: `5/5 KILLED`. Ни одна мутация не выжила, ни одна не осталась неприменённой.

## 3. Финальное состояние после ревью

```
$ node --test tests/unit/onboarding-wizard.test.mjs   → exit=0 (8/8)
18ba4d9a9404c5979b814363ed1e8de2  rust/gui-spike/ui/features/onboarding-wizard.js
b47a7a25207dd47cec83e584260afe76  tests/unit/onboarding-wizard.test.mjs
```

Рабочее дерево по целевым файлам чистое относительно исходного состояния: продуктовый файл побайтово восстановлен (`md5sum` сверен после каждой из 5 итераций и в конце), тестовый файл не открывался на запись.

## 4. Замечания (не блокирующие)

**P2 — деструктивный fallback в `destroy()`** (`onboarding-wizard.js:544-553`). Если `root.parentNode !== container`, ветка `else` вызывает `replaceElementChildren(container, [])` и стирает всё содержимое контейнера, включая чужие узлы. По контракту `options.container` принадлежит контроллеру, поэтому это не дефект текущего сценария, но при повторном использовании контейнера поведение неожиданное. Предложение: заменить `else`-ветку на no-op с явной причиной.

**P2 — доступность группы опций**. Блоки опций рендерятся в `div[data-onboarding-prompt-options]` без `role="group"`/`role="radiogroup"` и без связи `aria-labelledby` с заголовком `h3[data-onboarding-prompt-title]`. `aria-pressed` на кнопках есть и покрыт тестом, но для скринридера вопрос и его опции не образуют единую группу (при `singleSelect` уместнее `radiogroup` + `role="radio"`/`aria-checked`).

**P3 — покрытие**. Нет прямого теста на создание контроллера с `selectedOptionIds` в виде `Set` (функция `areRequiredPromptsSatisfied` с `Set` покрыта в T-262-03); в T-262-08 не проверяется обновление текста `[data-welcome-description]` после `setState`, только каналы и `aria-pressed`. Также отсутствует негативный тест на `createOnboardingWizardController()` без `container` (ожидается `TypeError`) — проверка `resolveDocument`/валидации `options` тестами не закреплена.

**P3 — нет UI-сигнала о невалидном описании**. `isValidWelcomeDescription` экспортирована, но контроллер не использует её и не показывает ошибку для `welcomeDescription` длиннее 140 символов; ограничение живёт только в чистой функции. Если это осознанное разделение слоёв (валидация на уровне вызывающего кода), замечание снимается.

## 5. Проверка контракта с оракулом

Константы соответствуют `reference-oracle/rest-static.json` (`GUILD_WELCOME_CHANNELS` = 5, описание ≤ 140 символов): `MAX_WELCOME_CHANNELS = 5`, `MAX_WELCOME_DESCRIPTION_CHARS = 140`. Лимит каналов защищён дважды — в `normalizeWelcomeChannels` (break на 5-м) и в `renderWelcomeSection` (`slice(0, MAX_WELCOME_CHANNELS)`), тест T-262-05 проверяет отсутствие 6-го и 7-го канала в DOM. Обязательность вопросов и `singleSelect` реализованы в чистой функции и повторно применяются обработчиком кнопки завершения, поэтому UI и валидация не расходятся.

## 6. Артефакты

Логи прогонов: `waves/RW-40/G2/task-262-ui-onboarding-wizard/MUTATION-176A/`
(`M1.log`..`M5.log`, `results.txt`, `post-restore.log`).
Скрипт мутаций: `/tmp/opencode/tr176a/mutate.sh`.
