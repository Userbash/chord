# DP-02 — D-04: переменная высота строк 26/48/+22, member 42, height index

Карточка: RW-40 / DP-02 (P1), G2 Frontend. Источник — `waves/RW-39/DESIGN/SPEC.md`
§5.1, A-01/A-02; оракул — `waves/RW-37/RE/layout.json` (`row_heights`,
`row_heights_detail`, `line_height`). Дата: 2026-10-24. PM: DeepSeek.
Live/окно/сеть/cargo/gate/commit не запускались — только targeted `node`, `tsc`,
static geometry.

## 1. Сделано

`model.js` (ROW_HEIGHTS/HeightIndex/createChatHeights/createMemberHeights) уже
содержал D-04 после W39h — **не менялся** (md5 в §4). Закрыты остатки, из-за
которых аудит DP-02 считал модель «на старом треке»:

| Что | Было | Стало | Файл |
|---|---|---|---|
| Высота `.text` строки | читалась из `row.style.height` (трек 44) с fallback `26` и `−2` | `messageHeight(index) − 2×2` (`--space-1`), т.е. строки×22 + строка автора у `group_start`; не зависит от того, отрисована ли строка виртуализатором | `features/chat.js` |
| `data-row-height` чата | литерал `"26/48/+22"` | собирается из `ROW_HEIGHTS` (диагностика не разойдётся с моделью) | `features/chat.js` |
| Треки RW-38 | `[F] FROZEN … временный трек` | `[F] legacy RW-38 (44/40), модель не использует` + шапка и раздел 10 объясняют, что значения держит только `geometry-check.mjs` | `tokens.css` |
| Шапка provenance | «отличий ровно два» | «ровно три»: правка DP-02 чисто комментарийная, значения/имена не менялись | `tokens.css` |

Новых значений, магии и публичных API нет: `renderChatRow`/`ROW_HEIGHTS`
используются как есть, сигнатуры не менялись → дизайн-предложение по §5 ТЗ не
требуется.

Тесты-оракулы (аддитивно, 16 новых кейсов; оракул читается из `layout.json`,
не из `model.js`):

| Файл | +кейсов | Что фиксируют |
|---|---|---|
| `tests/unit/gui-spike-model.test.mjs` | 7 | ROW_HEIGHTS = RE; `messageHeight` = 26/48 + 22×перенос на 2 периодах (набор высот 26/48/70/92, «44» не входит); литеральные пары 0→48, 1→26, 9→70, 32→70, 400→92; prefix-sum/top/indexAt/totalHeight; `prefix.length = period+1` (не O(rows)); отказ непериодичным высотам; member 42 = rows×42 |
| `tests/unit/gui-spike-features.test.mjs` | 7 | высоты и `.text` в живом списке; `renderChatRow` без виртуализатора (индекс 9 → 66px, а не 24); `data-row-height` + отсутствие `--row-*-track` в model/chat/virtual-list/members; `scrollToIndex` → `scrollTop = top(i)` и `firstVisibleIndex = i`; tombstone уменьшает `totalHeight` ровно на высоту удалённой строки и смыкает соседей; member 42 / шаг RE 43 (`--row-member-pitch`) / панель 42; D-04 токены на месте, legacy-треки помечены |
| `tests/unit/gui-spike-bench.test.mjs` | 2 | prefix-sum = 1565 записей (≈12.5 КБ) на 100 000 строк; 20 000 пар `top/indexAt` в бюджете 5 с + `indexAt(top(i)) = i` |

Старые ожидания 44/40 **не заменялись подгонкой**: в тестах базы их не было, а
новые проверки прямо запрещают `--row-*-track` в коде модели/рендера. Ни один
тест не ослаблен и не удалён: 228 кейсов базы сохранены, добавлено 16 → 244.

## 2. Red (до правки продукта)

`waves/RW-40/PM/dp02/red.log` (md5 `ae4b0a0e43e26e779dd84d1139ff9429`), exit 1,
117/120 pass, 3 падения — ровно новые оракулы:

```
✖ высоты строк чата 26/48/+22, .text по тексту/переносам (не 44-трек)
  AssertionError: строка 0: .text не по числу переносов (1)
    '46px' !== '44px'
✖ renderChatRow считает высоту из текста, а не из строки/трека
  AssertionError: высота .text считается не по тексту
    '24px' !== '66px'
✖ tokens.css: D-04 значения на месте, треки RW-38 помечены legacy
  AssertionError: --row-message-track не помечен legacy (44 — приближение, не паритет)
```

Red получен на том же наборе тестов с откатанным продуктовым патчем (обратный
патч применён к `chat.js`/`tokens.css`, лог снят, патч возвращён) — это
воспроизводимо, а не «подгонка под уже исправленное».
Anchor-кейсы (высоты строк, scrollToIndex, tombstone, member 42) были зелёными
и в red-прогоне: это регресс-якоря существующей реализации D-04.

## 3. Green

`waves/RW-40/PM/dp02/green.log` (md5 `4d6b597e8dc1f71a6abbe3181ab1ab00`), exit 0:

```
node --test tests/unit/gui-spike-{dom,model,bench,features,auth,regressions}.test.mjs
  ℹ tests 244  ℹ pass 244  ℹ fail 0  ℹ cancelled 0  ℹ skipped 0
npx tsc -p rust/gui-spike/ui/tsconfig.json
  exit 0 (без вывода)
node waves/RW-39/PM/w39c/geometry-check.mjs
  все 31 геометрических значений в допуске
node --test tests/meta/design-breakpoints.test.mjs
  ℹ tests 6  ℹ pass 6  ℹ fail 0
```

`unit.log` (md5 `cf1690f5bc7c60c6ab6767dc7c9e5f51`) — отдельный повторный прогон
того же набора: 244/244. e2e/gate:worker не запускались (запрещены PM, нужен
integration-лейн).

## 4. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/model.js` (не менялся, D-04 уже был) | `642e1d8f9a35ca0bc6e0dced394ebd77` |
| `rust/gui-spike/ui/features/chat.js` | `a76fcbdecb1ca24c6533254a3fcc5858` |
| `rust/gui-spike/ui/tokens.css` | `21d570a6bf9c4b5d77b00d3d9b3d1d08` |
| `tests/unit/gui-spike-model.test.mjs` | `f014e702ca0a0195c9d87211e6218e26` |
| `tests/unit/gui-spike-features.test.mjs` | `db2095278f558e284232f3fae4a14d97` |
| `tests/unit/gui-spike-bench.test.mjs` | `687b5de9754b48cd61f4093c44be7904` |
| `waves/RW-40/PM/dp02/red.log` | `ae4b0a0e43e26e779dd84d1139ff9429` |
| `waves/RW-40/PM/dp02/green.log` | `4d6b597e8dc1f71a6abbe3181ab1ab00` |
| `waves/RW-40/PM/dp02/unit.log` | `cf1690f5bc7c60c6ab6767dc7c9e5f51` |

PROCESS §4.2: batch RW-40/DP-02, red/green выше, targeted — 244 unit + 6 meta,
gate — не запускался (запрещён PM), риски/остаток — §5.

## 5. Остаток / что не сделано (сознательно)

1. **`--row-message-track: 44px` / `--row-member-track: 40px` пока нельзя удалить.**
   Их проверяет `waves/RW-39/PM/w39c/geometry-check.mjs` (`/^44px$/`, `/^40px$/`),
   а этот скрипт вне файлового мандата DP-02 (и требование — geometry 31/31).
   Поэтому значения сохранены как `[F] legacy RW-38`, модель/рендер их не читают
   (это закреплено тестом). Снятие: правка скрипта RW-39 + удаление двух строк
   токенов одной карточкой (G2+G4, после согласования оракула geometry).
2. **Комментарии в read-only CSS описывают старый трек:** `components/chat.css:1,18`
   («виртуализация 44px», «Строка 44»), `components/members.css:3,99` («список
   участников 40»). Вне allowed-файлов; нужен мандат на остальной UI.
3. **DESIGN/tokens.css не синхронизирован по комментариям** (значения совпадают
   1:1, включая legacy 44/40 — источник RW-39). Шапка `ui/tokens.css` уже честно
   перечисляет три отличия; G3 переносит текст комментариев при следующей сверке.
4. **Member pitch 43 остаётся измерением, не шагом раскладки.** SPEC §5.1:
   строка 42 `[L]`, шаг 43 `[M]` (JPEG ±1). Раскладка — 42 (sizer 24 229×42 —
   оракул `gui-spike-dom`); токен `--row-member-pitch: 43px` проверяется тестом.
   Переход на 43 — дизайн-решение (ломает оракул sizer), нужен ответ дизайна.
5. **Строки с extras** (day-divider 24 + body 22 = 46) помещаются в 48 за счёт
   нижнего padding'а (2+44+2 = 48); высота строки остаётся 26/48/+22 по D-04.
   Если PNG-паритет потребует отдельной высоты под extras — это новое правило
   высот вне A-01/A-02 (нужен SPEC).
6. `tests/meta/plan-sync` и `gate:worker` не запускались (полный meta/гейт —
   integration-лейн); design-breakpoints 6/6 зелёный.
