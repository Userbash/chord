# Technical Review 180A — PR-151 (`task-267` / FE-INT-10)

- **Ревьюер:** Технический ревьюер (DeepSeek, `deepseek/deepseek-v4.1-flash`, token-lean)
- **Дата:** 2026-10-31
- **Ветка/worktree:** `feature-task-267-ui-shell-controllers-registry`
- **Объект ревью:** `rust/gui-spike/ui/features/shell-controllers-registry.js`, `tests/unit/shell-controllers-registry.test.mjs`
- **Основание:** `REPORT.md` (G2), `ACCEPT.md` (PM)
- **Вердикт:** **APPROVED** — `P0 = 0`, `P1 = 0`, мутации `5/5` убиты, байтовая целостность модуля после мутаций подтверждена.

## 1. Targeted-тест и сверка MD5

```
$ node --test tests/unit/shell-controllers-registry.test.mjs
ℹ tests 16
ℹ pass 16
ℹ fail 0
EXIT=0
```

| Файл | md5 (факт) | md5 в REPORT.md | Совпадение |
| --- | --- | --- | --- |
| `rust/gui-spike/ui/features/shell-controllers-registry.js` | `5dac49c8f33ea25694f93625d7f7fd49` | `5dac49c8f33ea25694f93625d7f7fd49` | да |
| `tests/unit/shell-controllers-registry.test.mjs` | `1a2f84fb41dece94e337739079fba4a1` | `1a2f84fb41dece94e337739079fba4a1` | да |

Тестовый файл в ходе ревью не изменялся: md5 до и после мутаций одинаковый.

## 2. Мутационное тестирование (5/5)

Метод: одна правка продуктового модуля → `node --test tests/unit/shell-controllers-registry.test.mjs` → фиксация `exit` → побайтовое восстановление из бэкапа `cp -p` → сверка `md5sum` и `cmp -s` с эталоном. Патч накладывался python-скриптом со строгой проверкой `count(old) == 1`, поэтому ни одна мутация не задела лишний код.

| # | Мутация | Ожидаемое нарушение контракта | Результат | Убита | Тест-убийца | Файл после restore |
| --- | --- | --- | --- | --- | --- | --- |
| M1 | `"webhooks-editor"` → `"webhook-editor"` в `GUILD_ADMIN_PANELS` | состав/порядок реестра 6 панелей | `exit=1`, `fail 2` | да | «содержит ровно 6 канонических панелей в объявленном порядке» (+ каскад на открытии панели с изменённым id) | md5 `5dac49c8…`, `cmp` identical |
| M2 | `Object.freeze([` → `([` (снят freeze) | реестр мутабелен | `exit=1`, `fail 1` | да | «заморожен и не даёт мутировать себя» | md5 `5dac49c8…`, `cmp` identical |
| M3 | в `openPanel` условие `activePanel !== null && activePanel !== panelId` → `activePanel !== null` | повторное открытие панели закрывает её саму | `exit=1`, `fail 1` | да | «повторное открытие той же панели не закрывает её саму» | md5 `5dac49c8…`, `cmp` identical |
| M4 | `onDispatchAction?.(panelId, action.trim(), payload)` → `action` | action не обрезается | `exit=1`, `fail 1` | да | «обрезает action и передаёт payload в onDispatchAction» | md5 `5dac49c8…`, `cmp` identical |
| M5 | удалён `controllers.clear()` в `destroy()` | реестр не очищается | `exit=1`, `fail 1` | да | «закрывает активную панель и очищает реестр контроллеров» | md5 `5dac49c8…`, `cmp` identical |

Итог мутационного прогона: **5/5 (100%)**, ни одна мутация не выжила; `exit != 0` во всех пяти случаях.

## 3. Контроль восстановления

- Эталонная копия: `/tmp/opencode/tr180a/shell-controllers-registry.js.bak` (снята до первой мутации, `md5 5dac49c8f33ea25694f93625d7f7fd49`).
- После каждой мутации: `restore_md5=5dac49c8f33ea25694f93625d7f7fd49`, `diff_vs_backup=identical`.
- Финальная сверка: `cmp -s` → `BYTE_IDENTICAL`, `git status` показывает файлы как untracked, без посторонних правок.
- Повторный baseline-прогон после восстановления: `16/16 pass`, `exit=0`.

## 4. Оценка кода по существу

**P0 — нет.** Модуль не трогает DOM, IPC, сеть и файловую систему; импортов нет; секретов и внешних вызовов нет.

**P1 — нет.** Контракт закрыт тестами: замороженный реестр из 6 панелей, чистый предикат без сайд-эффектов, валидация `panelId` и объекта контроллера, взаимное исключение активной панели с закрытием предыдущей, `onPanelChange`/`onDispatchAction` как единственные точки эффектов, `trim()` действия, сортированная копия реестра, полная очистка в `destroy()`. Red-лог (`ERR_MODULE_NOT_FOUND`) и green-лог (`16/16`) в `REPORT.md` согласуются между собой и с фактическим прогоном.

Замечания уровня P2/P3 (не блокируют merge, кандидаты в отдельную карточку при интеграции):

1. **P3.** `registerPanel` проверяет `typeof controller !== "object"`, поэтому массив проходит как контроллер. Вреда нет (методы вызываются через `?.()`), но проверка `!Array.isArray(controller)` сделала бы сообщение об ошибке точнее.
2. **P3.** В `openPanel` поле `activePanel` присваивается до вызова `controllers.get(panelId)?.open?.(payload)`. Если `open()` бросит, роутер останется с новой активной панелью без `onPanelChange`. Для текущих колбэков не критично; при интеграции с реальными контроллерами стоит либо обернуть вызов, либо документировать порядок.
3. **P2 (унаследовано из REPORT).** Публичный API пока не подключён к `app.js`/`dispatch-bridge.js` — это отдельная задача интеграции, а не дефект этого PR.

## 5. Границы ревью

Полный `gate:worker`, `tsc` по workspace и `git commit` не запускались — по ТЗ 180A. Заявление REPORT о `0 tsc errors` принято как evidence PM/воркера и здесь не перепроверялось. Чужие файлы, тесты и общие документы не менялись.

## 6. Финальные контрольные суммы

```
5dac49c8f33ea25694f93625d7f7fd49  rust/gui-spike/ui/features/shell-controllers-registry.js
1a2f84fb41dece94e337739079fba4a1  tests/unit/shell-controllers-registry.test.mjs
```

Вердикт: **APPROVED**, PR-151 готов к merge в `unstable` после зелёного CI.
