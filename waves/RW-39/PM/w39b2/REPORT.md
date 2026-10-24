# W39b2 — отчёт: выравнивание `DESIGN/tokens.css` по D-02/D-04/D-05

Карточка: RW-39 / W39b2 (Design, продолжение W39b). PM: DeepSeek. Итерация 1. Дата: 2026-10-24.
Статус: **выполнено, на приёмку PM**. Тесты, `rust/gui-spike/ui/**`, `src/**`, `REFERENCES.md`,
`layout.json` не изменялись. Без commit/push, без сети.

## 1. Исходное состояние (важно для приёмки)

Основная часть D-02/D-04 уже лежала на диске от прерванного W39b: `tokens.css` содержал
новые цвета, `--row-h-*`, шаг 30 и обновлённые контрастные комментарии; `SPEC.md` — числа
строк D-04 (§5.1/§5.2/§7.5/§7.6/§13/§14). `tokens.json` был перегенерирован (204 токена).

Первая проверка в сессии попала на снимок до перезапуска окружения (json 196 / css 198,
18 ошибок); после перезапуска фактическое состояние — 204 токена, md5 css `62fffb9c…`,
`gen-tokens --check` и `verify-tokens` зелёные, `verify-layout` падал 7×`discrepancy: нет id`
(в `layout.json` параллельно шла запись, файл мне недоступен для правки).

Мой дельта-вклад в этой карточке (то, чего не хватало):

1. новый токен `--positive-fact: #45a366` `[M]` + строка в SPEC §3.3;
2. SPEC §3.3: `--positive`/`--status-*` приведены к дампу и разделены (`--positive-fact`);
3. `DOM-CONTRACT.md` §9.2: зафиксированы числа строк D-04 и структура `#emoji-grid` (D-05);
4. `tokens.json` перегенерирован генератором (не руками).

## 2. Значения: было → стало → основание

Все строки ниже — против принятого D-01 состояния (`tokens.css` md5 `579a878a…`).

| Токен | Было | Стало | Основание |
|---|---|---|---|
| `--danger` | `#ed4245` | `#da3e44` | `[T] --status-danger` + `[M]` красная точка панели; `#ed4245` в дампе 1.0.162 нет (Δ19) |
| `--warning` | `#f0b232` | `#fdb833` | `[T] --status-warning` / `--yellow-300` (Δ13) |
| `--positive` | `#23a55a` | `#3d9e60` | `[T] --status-positive` / `--green-360` (Δ26) |
| `--positive-fact` | — | `#45a366` | `[M]` фактическая зелёная точка панели = `--green-new-38`; отдельный токен, только non-text |
| `--positive-text` | `#57d98a` | `#57d98a` (комментарий пересчитан: 7.65–8.60) | `[A]`; прежний диапазон не соответствовал поверхности |
| `--text-inverse` | `#1a1a1e` | `#2f3035` | `[T] --text-invert` (Δ23); на `--warning` 7.57, белый на нём 13.17 |
| `--accent-hover` | `#4752c4` | `#4654c0` | `[T] --brand-560` (Δ4) |
| `--accent-soft` | `rgb(88 101 242 / 14%)` | `rgb(88 101 242 / 16%)` | `[T] --message-highlight-background-default` (16 %) |
| `--border-subtle` | `rgb(142 146 157 / 12%)` | `rgb(151 151 159 / 12%)` | `[T] #97979f` (Δ9), альфа не менялась |
| `--border` | `rgb(142 146 157 / 20%)` | `rgb(151 151 159 / 20%)` | то же (Δ9) |
| `--border-strong` | `rgb(142 146 157 / 44%)` | `rgb(151 151 159 / 44%)` | то же (Δ9) |
| `--badge-secondary-bg` | `rgb(142 146 157 / 16%)` | `rgb(151 151 159 / 16%)` | `[T] --badge-secondary-background` (Δ9) |
| `--status-online` | `#23a55a` | `#3d9e60` | `[T] --status-positive` |
| `--status-idle` | `#f0b232` | `#fdb833` | `[T] --status-warning` |
| `--status-dnd` | `#ed4245` | `#da3e44` | `[T] --status-danger` |
| `--status-streaming` | `[T]` (ложная метка) | `[A]`, значение `#593695` не менялось | D-02: имени в дампе 1.0.162 нет; обоснование в SPEC §3.3 |
| `--radius-round` | `9999px` / `[T]` | `2147483647px` / `[A]`-эквивалент | D-02: эквивалент после клампа, не дефект |
| `--row-channel-pitch` | `30px` без пояснения | `30px`, комментарий «28 высота + 2px зазор» | D-02: RW-37 фиксирует 28 (gap), шаг 30 — `[M]` (n=19) |
| `--row-h-continuation` | — | `26px` | D-04 / `layout.json`: `message_cozy` / `message_cozy_continuation` |
| `--row-h-group-start` | — | `48px` | D-04 / `layout.json`: `message_cozy_group_start` |
| `--row-line-extra` | — | `22px` | D-04 / `layout.json`: перенос +22 |
| `--row-h-member` | — | `42px` | D-04 / `layout.json`: `member` (панель и виртуализация) |

Legacy-имена (`--row-message-cozy` 26, `--row-message-start` 48, `--row-member` 42) сохранены
как ссылки из `layout.json`; `--row-message-track` 44 / `--row-member-track` 40 остаются `[F]`
как приближение тестов RW-38 и помечены «временный трек до W39d/D-04» (дизайн-константой не
являются).

## 3. Контрастные комментарии (пересчёт, PY-скрипт по WCAG sRGB)

| Пара / комментарий | Было | Стало (факт) |
|---|---|---|
| `--text-on-warning` `#121214` на `#fdb833` | 9.91 (ложное) | **10.76** |
| `--positive` `#3d9e60` на `--bg-chat` | 5.45 (ложное) | **5.17** |
| `--positive-fact` `#45a366` | — | **5.52** на `--bg-chat`, **4.37** на `--surface-4` (только non-text) |
| `--positive-text` диапазон | без диапазона / 7.65–8.60 | **7.65–8.60** (по лестнице поверхностей) |
| `--text-accent` / `--focus-ring` `#a5b0ff` | 7.5–9.1 | **6.71–9.14** |
| `--danger-fill` `#cf3a3f` + белый | 4.85 | **4.85**; дамп `--danger` `#da3e44` + белый = **4.42** (поэтому заливка кнопки темнее) |
| `--text-inverse` `#2f3035` | — | на `--warning` **7.57**, белый на `--text-inverse` **13.17** |

Проверены также диапазоны `--text` (11.96–16.29), `--text-strong` (13.28–18.08),
`--text-muted` (4.72–6.43), `--text-subtle` (6.07–8.26), `--text-link` (4.52–6.16) —
в `tokens.css` и SPEC §3.4 числа совпадают с расчётом. Статус-точки как non-text на
`--bg-chat`: `--danger` 3.93, `--warning` 9.98, `--positive` 5.17 (≥ 3:1).

## 4. SPEC / DOM-CONTRACT

- `SPEC.md` §3.2: `--text-inverse` = `#2f3035` (D-02, Δ23), применение и контрасты.
- `SPEC.md` §3.3: `--accent-hover` `#4654c0`, `--accent-soft` 16 %, `--danger/--warning/--positive`
  = дамп, новый `--positive-fact`, `--status-*` с именами токенов дампа, `--status-streaming`
  `[A]` с обоснованием.
- `SPEC.md` §3.4: пересчитанные пары (см. §3) — ложных `9.91`/`5.45` больше нет.
- `SPEC.md` §5.1/§5.2: констант `44`/`40` нет; строка сообщения `26`/`48`/`+22`, участник `42`
  (D-04), шаг канала `30` = 28 + 2 зазор.
- `DOM-CONTRACT.md` §9.2 (аддитивно, без правки `[F]`):
  - «Числа строк (D-04)»: `data-row-height` 44/40 в §3 — legacy-атрибуты `[F]`, рантайм-высоты
    26/48/+22 и 42 задаёт `ui/model.js`, харнесс высоты не хардкодит;
  - «`#emoji-grid` (D-05)»: целевая структура `grid` + строки-обёртки `row` с
    `display: contents` (структурная часть OBS-01), контейнер добавляет `buildDom`, строки/ячейки —
    W39d; ARIA-роль на этом шаге остаётся `listbox` по D-07/`[F]` (тест RW-38), смена на `grid` —
    отдельное решение PM.
- `[F]`-числа DOM/ARIA (4/40/44, `role=listbox`, `data-row-height`) не тронуты.

## 5. Промер verify (до/после)

| Команда | До (снимок до перезапуска) | До (факт на диске) | После |
|---|---|---|---|
| `gen-tokens.mjs --check` | FAIL: рассинхронизирован | ok, 204 токена, md5 `62fffb9c…` | **ok, 205 токенов, md5 `7ce9c1f0…`** |
| `verify-tokens.mjs` | FAIL: 18 ошибок (json 196 ≠ css 198) | ok: 204, 57 match / 0 mismatch / 0 missing | **ok: 205, 57 match / 0 mismatch / 0 missing** |
| `verify-layout.mjs` | ok | FAIL: 7× `discrepancy: нет id` (внешняя правка `layout.json`) | **ok: 65 блоков, 22 ссылки, 4 экрана, rows 26/48 + member 42** |

`verify-layout` до правки падал не из-за моих файлов: `layout.json` (read-only для W39b2)
в момент 15:05 дописывался параллельным владельцем, записи `discrepancies[]` были с `key`
вместо `id`. К концу карточки файл дописан, проверка зелёная; я его не менял.

Полный лог: `waves/RW-39/PM/w39b2/check.log` (второй прогон после правки, стабилен).

```
ok: tokens.json актуален (205 токенов, md5 css 7ce9c1f03e26056243e13df3237b0a9d)
ok: 205 токенов 1:1 с tokens.css (md5 7ce9c1f0…); dump_refs: 57 match, 0 mismatch, 0 missing
ok: layout.json — 65 блоков с source, 22 ссылок на токены, экранов 4; rows 26/48 + member 42
```

Ассеты: `rg -n "url\(|@font-face" waves/RW-39/DESIGN/tokens.css` — совпадение только в
комментарии-запрете («никаких … url()»), реальных `url()`/`@font-face` нет.

## 6. Файлы + md5

| Файл | Байт | md5 |
|---|---|---|
| `waves/RW-39/DESIGN/tokens.css` | 21231 | `7ce9c1f03e26056243e13df3237b0a9d` |
| `waves/RW-39/DESIGN/tokens.json` | 50676 | `12be8c11a82abbe97ba544505ce75ccc` |
| `waves/RW-39/DESIGN/SPEC.md` | 58444 | `340eeac669096af92ccc62fd4a0f3693` |
| `waves/RW-39/DESIGN/DOM-CONTRACT.md` | 53466 | `a564f36fe46f68e6895b956c3a6a5a9d` |
| `waves/RW-39/PM/w39b2/check.log` | — | см. файл |

`tokens.json` — только вывод `gen-tokens.mjs` (руками не правился); `source_css.md5`
в нём = `7ce9c1f0…`, 205 токенов (13 alias), 6 оверрайдов.

## 7. Что осталось (вне W39b2)

1. `W39c`: перевендорить `DESIGN/tokens.css` → `rust/gui-spike/ui/tokens.css` и обновить
   `source-md5` (сейчас вендоренная копия отстала: в ней есть `--row-h-*`, но нет
   `--positive-fact` и md5 чужой). D-03 п. 2.
2. `W39d`: рантайм-высоты уже в модели; хвост — убрать legacy `--row-message-track`/`--row-member-track`
   44/40 и атрибуты `data-row-height` после снятия `[F]`-ожиданий (`chat.css` `.text{height:40px}`
   и `.text-body{white-space:nowrap}` — правка W39c2, зафиксирована в DOM-CONTRACT §9.5).
3. `#emoji-grid`: смена ARIA `listbox` → `grid` + `row` (клавиатура roving) — решение PM/W39d,
   тест RW-38 сейчас фиксирует `listbox`; в контракте помечено как отдельный шаг.
4. `--status-streaming` остаётся `[A]` (в дампе имени нет) — принято D-02, не дефект.
5. `layout.json` пишется параллельным владельцем; при повторной проверке после его дописывания
   `verify-layout` зелёный, но файл вне моей зоны — при следующей правке прогнать снова.

## 8. Границы

Изменены только `waves/RW-39/DESIGN/{tokens.css,tokens.json,SPEC.md,DOM-CONTRACT.md}` и
`waves/RW-39/PM/w39b2/*`. Тесты, `rust/gui-spike/ui/**`, `src/**`, `REFERENCES.md`,
`layout.json` — не тронуты. Сети, пакетов, `cargo`, окна, commit/push не было; `cargo` не
запускался (RUN BUDGET: `gen-tokens --check` и `verify-tokens` — по 2 прогона, `verify-layout` —
3, включая стартовый прогон до восстановления состояния окружения).
