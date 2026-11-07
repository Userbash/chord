# C-02-UI-OLDER-FIX — отчёт по ревью Codex (P1 + P2)

- **Карточка:** `C-02-UI-OLDER-FIX`, отряд G2 Frontend, batch RW-40/PM.
- **Вход:** `waves/RW-40/PM/c02-ui-older/REVIEW.md` (P1 ×1, P2 ×1), вердикт RETURN.
- **HEAD:** `4e240481e59ff4d49732ac8bd5ba08a006363748`.
- **Дата:** 2026-10-30.
- **Не запускалось:** `cargo`, `npm run gate:*`, e2e, live/окно/Discord, сеть, `git commit/push`.
- **Свои файлы:** `rust/gui-spike/ui/features/chat.js`,
  `tests/unit/channel-paging.test.mjs`, этот отчёт и логи
  `waves/RW-40/PM/c02-ui-older/fix-{red,green}.log`. Чужие файлы не менялись.

## 1. Что сделано

### P1 — prepend-компенсация мерила неполный DOM

**Было** (`chat.js:1470`): `beforeHeight` снимался, когда в `#chat-list` уже
лежали строки, но обе полосы истории были сняты (`clearOlderBar()` в
`renderChannelWindow`, `clearHistoryFooter()` в `appendChannelRows`), а
`syncHistoryView()` добавлял их обратно **после** записи `scrollTop`. Дельта
включала удалённые полосы и не включала восстановленные, поэтому строка под
курсором уезжала на их суммарную высоту.

**Стало:** два хелпера рядом с `scrollChannelToEnd()`:

```js
function captureScroll() { return { top: options.list.scrollTop, height: options.list.scrollHeight }; }
function restoreScroll(snapshot) {
  options.list.scrollTop = snapshot.top + (options.list.scrollHeight - snapshot.height);
}
```

- `loadOlderHistory` (`chat.js:1496`, `:1514`): снимок берётся перед
  `renderChannelWindow`, `restoreScroll` — после `syncHistoryView(win)` **и**
  после ветки `limit` (там полосы добавляются явно). К этому моменту DOM
  содержит строки и обе полосы, дельта равна высоте вставленного.
- `loadMoreHistory` (`chat.js:1573`, `:1588`): тот же дефект в защитной ветке
  `merged.prepended > 0` (forward-ответ, вернувший строки старше окна). Правка
  симметричная: снимок до перестройки, восстановление после
  `historyFooterNode(win.hasMore ? "idle" : "end")`. Без неё тот же шов остался
  бы в forward-функции; отдельного требования ревью не было, но это тот же
  класс ошибки — отмечено как расширение объёма в §4.

Геометрический тест: `C-02-UI-OLDER: геометрия prepend` в
`tests/unit/channel-paging.test.mjs`. Шим не считает layout, поэтому тест
подменяет `#chat-list.scrollHeight` собственной моделью высот (строка `420 px`,
полоса `30 px`, контент выше окна 800 px) и проверяет два инварианта:

1. `scrollTop` после prepend равен `beforeTop + высота вставленной строки`
   (то есть компенсация идёт по полной дельте, а не по частичной);
2. смещение якорной строки в окне (`offsetOf(row) - scrollTop`) не изменилось.

Старое поведение: `424` вместо `484` — ровно `60 px` двух полос (`2 × 30`),
которые были сняты при измерении и восстановлены после. Второй кейс
`пересечение страниц: повтор граничной строки не даёт дубля` переносит проверку
дедупа из контрактного теста (см. P2).

### P2 — фикстура older-ответа нарушала контракт `before`

- `olderPage()` (`tests/unit/channel-paging.test.mjs:187`) теперь отдаёт при
  `before = HISTORY[1].id` только `[HISTORY[0]]` — строки строго до якоря
  (`C-02-BEFORE §1.2`, `before` exclusive). Раньше возвращались
  `[HISTORY[0], HISTORY[1]]`, то есть сам якорь.
- Тест раздельности направлений (`:735`) использовал тот же inclusive-ответ;
  теперь там `[HISTORY[0]]`, а forward-метаданные older-страницы
  (`hasMore`/`nextCursor`) остались — именно их подмену и проверяет кейс.
- Проверка дедупа вынесена в отдельный кейс `пересечение страниц` с явной
  пометкой «контракт такого не обещает, тест страхует UI». Основной контрактный
  кейс больше не доказывает dedup через невалидный ответ.

## 2. Red-first

`waves/RW-40/PM/c02-ui-older/fix-red.log` — тест добавлен, `chat.js` не тронут:

```
# md5 chat.js: 7098d413402c7cc6035f825bc347a286
# command: node --test --test-reporter=spec --test-name-pattern="геометрия prepend" tests/unit/channel-paging.test.mjs
▶ C-02-UI-OLDER: геометрия prepend
  ✖ prepend компенсирует высоту строки после восстановления обеих полос
  ✔ пересечение страниц: повтор граничной строки не даёт дубля
ℹ tests 2 / pass 1 / fail 1
AssertionError: prepend не скомпенсировал высоту вставленной строки: измерение шло без восстановленных полос
  424 !== 484
exit=1
```

Разница `484 − 424 = 60` — высота двух полос (`2 × 30 px`). Это и есть P1:
дельта посчитана без восстановленных полос.

Хеш теста в red-логе — `ae6c0df6…`: на момент прогона был добавлен только
геометрический кейс, правки фикстур P2 (§1) ещё не применялись. `chat.js` в
red-логе — исходный `7098d413…`.

## 3. Green

`waves/RW-40/PM/c02-ui-older/fix-green.log`:

```
# command: node --test --test-reporter=spec tests/unit/channel-paging.test.mjs tests/unit/channel-switch.test.mjs
ℹ tests 26 / suites 14 / pass 26 / fail 0
exit=0
```

`channel-paging.test.mjs` — 16/16 (было 14, добавлены геометрический кейс и
кейс пересечения), `channel-switch.test.mjs` — 10/10 (не менялся).

Типы:

```
npx tsc -p tsconfig.json   # exit=0, вывод пуст
```

## 4. Риски и границы

- **Расширение объёма.** Исправлена защитная ветка `loadMoreHistory`
  (`merged.prepended > 0`) — тот же шов, что в P1, но в forward-функции. Отдельного
  теста на неё нет: фикстура forward-страницы, возвращающей строки старше окна,
  в наборе отсутствует. Если нужен явный кейс — это ещё один мок `cursor`-ответа.
- **Геометрия только модельная.** DOM-шим не считает layout; тест подменяет
  `scrollHeight` моделью высот. Реальный браузерный скролл (шрифты, переносы,
  картинки) этой проверкой не покрыт, как и раньше — live/окно не запускались.
- **Крайние положения скролла.** При `scrollTop` у самого верха или у низа
  браузер клэмпит позицию; компенсация тогда ограничена диапазоном. Тест
  выбирает позицию с запасом с обеих сторон, чтобы различать ошибку измерения,
  а не клэмп.
- **Порядок обновления.** `restoreScroll` стоит после `aria-busy`/полос, но до
  `finally`; запись `scrollTop` не отменяет уже прошедшие guard-проверки
  (`stalled`, `hasOlder`, `limit`). Состояния полос и курсоров не менялись.

## 5. Файлы и контрольные суммы

| Файл | До | После |
|---|---|---|
| `rust/gui-spike/ui/features/chat.js` | `7098d413402c7cc6035f825bc347a286` | `6444e9d94a6daa9e1d7b036525295398` |
| `tests/unit/channel-paging.test.mjs` | `5feceaadeee6cab66c60d9bd45c61fe3` | `4ef50982a5de2847765bd1bc64d8dedf` |

`git diff --numstat`: `chat.js` +33/−11, `channel-paging.test.mjs` +144/−10.

## 6. Команды

```bash
node --test --test-reporter=spec --test-name-pattern="геометрия prepend" \
  tests/unit/channel-paging.test.mjs        # red: 424 !== 484
node --test --test-reporter=spec tests/unit/channel-paging.test.mjs \
  tests/unit/channel-switch.test.mjs        # green: 26/26
npx tsc -p tsconfig.json                    # exit 0
```

## 7. Вывод

P1 закрыт измерением полного DOM после восстановления полос (older-ветка и
симметричная forward-ветка). P2 закрыт контрактной фикстурой `before` и
отдельным кейсом пересечения. Приёмка — за Team Lead; live-паритет и
браузерная геометрия по-прежнему не проверены.
