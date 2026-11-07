# REVIEW — TR-188A / PR-159 / task-280 (`FE-FIX-04`: `tsc`-чистота `profile-modal.test.mjs` и `appearance-settings.test.mjs`)

- **Ревьюер:** Технический ревьюер (DeepSeek, TR-188A)
- **Ветка:** `bugfix/task-280-unit-tsc-fix-batch1` (HEAD `8f3bc24`)
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED**
- **Дефекты:** `P0=0`, `P1=0`, `P2=0`, `P3=2`
- **Мутации:** **5/5 убиты**, продуктовые модули восстановлены побайтно (MD5 = HEAD)

## 1. Проверяемые файлы

| Файл | MD5 (эталон ТЗ) | MD5 (факт) | Статус |
|---|---|---|---|
| `tests/unit/profile-modal.test.mjs` | `33330e8cd38d2aea4f7a7bf2fa64fd9e` | `33330e8cd38d2aea4f7a7bf2fa64fd9e` | совпал |
| `tests/unit/appearance-settings.test.mjs` | `0daf0811e1883af8b07c279c84b50af9` | `0daf0811e1883af8b07c279c84b50af9` | совпал |

Объём диффа: `+27 / −19` в двух файлах. Продуктовый код не тронут.

## 2. Scope и `git status`

```
 D node_modules
 M package-lock.json
 M tests/unit/appearance-settings.test.mjs
 M tests/unit/profile-modal.test.mjs
```

PR меняет ровно два тестовых файла из ТЗ. `D node_modules` (в индексе симлинк, в рабочем дереве реальный каталог) и `M package-lock.json` (`bin.discord-lab`, `engines.node >= 24` догоняют `package.json`) — состояние окружения, воспроизводится в несвязанных деревьях; при merge не коммитить. Продуктовые `profile-modal.js` и `appearance-settings.js` чистые: `git status` по `rust/gui-spike/ui/features/` пуст.

## 3. Прогоны

| Проверка | Команда | Результат |
|---|---|---|
| Typecheck целевых файлов | `npx tsc -p tsconfig.json --noEmit \| grep -E "profile-modal\.test\.mjs\|appearance-settings\.test\.mjs"` | **0 строк**, exit `1` у `grep` (совпадений нет) |
| Red-воспроизводимость | те же два файла на версии `HEAD` (временно `git checkout HEAD --` + возврат из копии) | **51 ошибка** ровно: `29` в `profile-modal`, `22` в `appearance-settings` — совпадает с `red.log` |
| Unit-тесты | `node --test tests/unit/profile-modal.test.mjs tests/unit/appearance-settings.test.mjs` | `tests 27, pass 27, fail 0, skipped 0`, exit `0` |
| По файлам | `node --test <файл>` | `profile-modal` `12/12`, `appearance-settings` `15/15` |
| Остаток по проекту | `npx tsc -p tsconfig.json --noEmit` | `88` строк `error TS` в других файлах — вне write-scope batch (подтверждает `REPORT.md`) |

После мутационных правок и восстановления оба прогона (tsc + 27 тестов) повторены: `0` ошибок в целевых файлах, `27/27` pass. MD5 обоих тестовых файлов после всех операций совпал с эталоном ТЗ.

## 4. Мутационное тестирование (3 мутации в `profile-modal.js`, 2 в `appearance-settings.js`)

Каждая мутация — точечная замена в продуктовом модуле, прогон `node --test` по двум файлам, затем восстановление исходного содержимого с проверкой MD5. Логи — `mutations/M<ID>-red.log`, машинный протокол — `mutations/mutations.json`.

| ID | Файл | Мутация | MD5 мутанта | Exit | Итог | Убившие тесты |
|---|---|---|---|---|---|---|
| M1 | `profile-modal.js` | `normalizeStatus`: fallback `"offline"` → `"online"` | `47dda5f8074e84528a05399cba8f6c5a` | 1 | **убита** | «корректно отображает все 4 статуса из STATUS_LABELS_RU и фоллбэчит неизвестный в offline» |
| M2 | `profile-modal.js` | `buildProfileHeader`: инверсия условия скрытия местоимений (`if (profile.pronouns)`) | `eb6c15b287621adc111d65eee8b324e1` | 1 | **убита** | «поддерживает поле about как синоним bio, скрывает пустые местоимения и отвергает невалидный containerEl» |
| M3 | `profile-modal.js` | `submitQuickDm`: инверсия guard пустого текста (`text.length !== 0`) | `004268cc7152113d23cc77e762d2ae6a` | 1 | **убита** | «рендерит быстрый инпут ЛС с placeholder 'Написать @' + username и вызывает onSendQuickMessage только для непустого текста» |
| M4 | `appearance-settings.js` | `isGroupSpacing`: валидация по `FONT_SCALE_STEPS` вместо `GROUP_SPACING_STEPS` | `54cba494002a2470aa727203969a7a84` | 1 | **убита** | «T-185-04: валидный JSON из storage восстанавливает все 4 контрола», «T-185-05», «T-185-11: setGroupSpacing принимает 0 и остальные шаги» |
| M5 | `appearance-settings.js` | `getState`: возврат внутренней ссылки `state` вместо копии `{ ...state }` | `b9bdeedf613aa1914ec1c178fb3726a9` | 1 | **убита** | «T-185-13: getState отдаёт копию, внешняя мутация не меняет контроллер» |

Базовые MD5 до прогона: `profile-modal.js` = `1518ab1b8972591a49746291b1e21400`, `appearance-settings.js` = `da96a0ee035e62d771dc741c02f2ce9c`. Финальные значения совпали с базовыми; `git show HEAD:<file> | md5sum` даёт те же хеши, то есть файлы идентичны HEAD, а не просто «похожи на исходное состояние».

MD5 красных логов: `M1 7fb400d0ff2377e54065a17b757f8428`, `M2 e4211baede16f539c8d83b6a4cbe0adc`, `M3 1ea5e8b69594b34089db4ddbeec5ba2d`, `M4 41201dcf88c6251a15e89e9ea0a369e7`, `M5 eb4dc5dbab9e82436818ca350c4160c0`.

## 5. Качество правки

- **Тесты не ослаблены.** В диффе `0` удалённых строк с `assert` и `0` добавленных `@ts-ignore` / `@ts-expect-error`. Все изменения — сужение типов, не ослабление проверок.
- **Приведения честные.** `const name = /** @type {string} */ (m[1])` в `matches()` опирается на регулярное выражение, которое гарантирует непустую первую группу; `value` по-прежнему берётся из `m[2] ?? m[3] ?? m[4]`.
- **`assert.ok` вместо `assert.equal` — усиление.** Новые `assert.ok(pills[0] && pills[1], ...)`, `assert.ok(aboutTab && guildsTab && friendsTab, ...)`, `assert.ok(guildItems[0] && guildItems[1], ...)`, `assert.ok(friendItems[0], ...)` и сужают тип для `noUncheckedIndexedAccess`, и дают явное сообщение о недорисованном DOM вместо `TypeError` на `undefined` в следующей строке.
- **JSDoc `makeStorage` исправлен по существу.** `@param {string|null} [seed]` соответствует телу функции (дефолт `null`, `data.set(...)` в `if (seed)`), раньше тип выводился как `null` и строки-JSON отвергались.
- **Границы.** Продуктовые модули, `tests/support/gui-dom-harness.mjs`, `tsconfig.json` не тронуты; конфиг глоб не менялся, `include`/`strict` не ослаблялись.

## 6. Замечания

### P3-1. `/** @type {any} */` на `ShimDocument` отключает проверку типа `document` в 14 вызовах

`createAppearanceController(/** @type {any} */ (doc), ...)` — рабочий и минимальный обход, но проверка типов аргумента на границе теряется полностью: при будущем изменении сигнатуры контроллера (например, третий параметр или другое имя поля) тест промолчит. Аккуратнее было бы типизировать шим как `Pick<Document, "documentElement">` с минимальной структурной совместимостью либо расширить `tests/support/gui-dom-harness.mjs` и объявить сигнатуру контроллера через узкий интерфейс (`{ documentElement: { setAttribute(...), style: { setProperty(...) } } }`). Не блокирует: сама сигнатура контроллера в `appearance-settings.js` осталась `@param {Document} doc`, продуктовый код не менялся.

### P3-2. `scripts/test-impact.mjs` не содержит записей для `profile-modal.js` / `appearance-settings.js`

`node scripts/test-impact.mjs --changed tests/unit/profile-modal.test.mjs` корректно выдаёт `T0` с targeted-тестом (правка тестов идёт от changed-файла), но при изменении продуктового модуля профильный тест в набор не попадёт — в `UI_MODULE_TESTS` записей для этих модулей нет. Риск ограничен: `gate.mjs` и `npm run test:unit` гоняют `tests/unit/**/*.test.mjs`. Рекомендация: карточка G4 QA (`scripts/` — не файлы PR-159).

## 7. Что осталось за пределами ревью

- Полный `npm run gate:worker` не запускался: ТЗ TR-188A ограничивает проверки two targeted-прогонами. Полный гейт выполняет integration-лейн на batch (правило §1.3 `AGENTS.md`).
- 88 остаточных ошибок `tsc` по другим файлам проекта — отдельные карточки серии (`task-279`, `task-281` и далее), не входят в scope `task-280`.
- `git commit`/`push` не выполнялись.

## 8. Итог

`51 → 0` ошибок `tsc` в двух целевых файлах воспроизведено независимо: на версии `HEAD` ровно `29 + 22`, на рабочей версии — ноль. Тесты зелёные (`27/27`) и сохранили силу: пять точечных мутаций в `profile-modal.js` и `appearance-settings.js` убиты, включая неочевидные (`getState` без копии, валидация отступов по чужому списку шагов). Продуктовые файлы восстановлены побайтно (`1518ab1b8972591a49746291b1e21400`, `da96a0ee035e62d771dc741c02f2ce9c` — совпадают с `HEAD`), тестовые — с эталоном ТЗ. Блокирующих дефектов нет.

**Вердикт: APPROVED** (`P0=0`, `P1=0`, мутации `5/5`). Замечания P3-1 и P3-2 не блокируют merge в `unstable`.
