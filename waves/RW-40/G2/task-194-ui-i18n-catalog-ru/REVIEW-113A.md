# REVIEW-113A — task-194 UI i18n catalog RU (G2)

Вердикт: **APPROVED**. P0=0, P1=0, мутации 5/5 killed. Проверял только `rust/gui-spike/ui/i18n.js` + `REPORT.md` (token-lean режим), затем targeted-тест и мутации.

## Проверка контракта
- `node --test tests/unit/i18n.test.mjs` → tests 11, suites 3, pass 11, fail 0, skip 0 (совпадает с `REPORT.md`).
- md5 до и после мутаций: `d12b967602657cbeb9a8362d49d3e623` — совпадает с `REPORT.md`, файл восстановлен, `git diff` пуст.
- Ключи и строки сверены с оракулами по ссылкам в шапке модуля (`i18n-deep.md`, `ru-catalog.json`); `Object.freeze` на каталогах, `hasOwnProperty.call` защищает от прототипных ключей.

## Мутации (каждая: mutate → node --test → restore)
| # | Мутация | Результат |
|---|---|---|
| M1 | `substituteParams` join(`{key}`) → `""` | KILLED (fail 2) |
| M2 | plural всегда ветвь `other` вместо `formatPluralRu` | KILLED (fail 2) |
| M3 | `hashedMap[hashKey]` → фиксированный `GSmTKJ` | KILLED (fail 3) |
| M4 | `FORBIDDEN_ENGLISH_LABELS` никогда не матчится | KILLED (fail 2) |
| M5 | `formatNumber` теряет `Intl.NumberFormat` | KILLED (fail 1) |

## Замечания (P2, не блокируют)
1. Ветка `fallbackBranch` (нет нужной формы в ICU) тестами не покрыта: мутация `category = "other"` выживает, т.к. при полном наборе форм результат даёт `formatPluralRu`. Покрыть fallback отдельным кейсом в следующей итерации.
2. `formatDate` с невалидной датой/локалью контрактом не проверен — тест на `""` и на `ru-RU` был бы полезен.
3. Подключение `tHashed`/`auditUntranslatedDomText` в `app.js`/`index.html` — отдельные интеграционные карточки (уже в рисках `REPORT.md`).

`git commit` не выполнялся. Полный `gate:worker` — на integration-лейне batch RW-40.
