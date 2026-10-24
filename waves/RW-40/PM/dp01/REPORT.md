# DP-01 — responsive breakpoints 1200/960/720 (G2 Frontend)

Карточка: RW-40 / DP-01 (P1), источник — `waves/RW-39/DESIGN/SPEC.md` §5.1,
`waves/RW-40/PARITY/DESIGN-PARITY.md` §3–§4. Дата: 2026-10-24. PM: DeepSeek.
Live/окно/сеть/cargo/gate/commit не запускались — только targeted `node`, `tsc`,
static geometry.

## 1. Сделано

| Ширина | Поведение | Где |
|---|---|---|
| ≥1600px | tree 311, member 256 | без изменений (`tokens.css` min-width:1600px) |
| 1200–1599px | tree 240, member 240 | базовый `:root` (не менялся) |
| 960–1199px | member panel `display:none`, tree 240 | `style.css` `@media (max-width:1199px)` |
| 720–959px | tree 200, member скрыт | `tokens.css` `@media (max-width:959px)` + тот же hide |
| <720px | tree `display:none`, rail-only | `style.css` `@media (max-width:719px)` |

- Ширина дерева — существующий токен `--layout-tree-min-w` (200px), новых
  значений и магических px не добавлено.
- Скрытие панели — `body[data-active-screen="chat"] #member-panel` (та же
  специфичность id+attr, что у базового `display:flex`, и позже в файле),
  иначе базовое правило выигрывало каскад. `#channel-tree` — id-селектор,
  поэтому перекрывает `.tree` из `tree.css` независимо от порядка подключения.
- `aria-hidden`/переключение экранов не тронуты: скрытие только CSS, DOM не
  меняется, `role="complementary"`/`role="list"` и `aria-label` остались;
  `features/screens.js` по-прежнему ставит `aria-hidden` только на `.screen`.
  Inline-display из `features/actions.js` — только `""`/`"none"`, поэтому
  media-query не перебивается.
- `chat ≥ 520` на 960: 960 − rail 64 − tree 240 = 656; `.workspace/.content/
  .chat-main` имеют `min-width:0`, `html,body { overflow:hidden }` — горизонтального
  скролла нет (проверяется meta-тестом).
- Широкий shell ≥1600 не изменён (регресс-строки в meta-тесте на 1600/2048).

## 2. Red (до правки CSS)

`waves/RW-40/PM/dp01/red.log`, exit 1, 3/6 fail по делу:

```
✖ токены по ширинам: 311/256 → 240/240 → 240 → 200
  AssertionError: tree при 959px            (ожидалось 200px)
✖ style.css: member panel скрыт на 960–1199, tree — на <720
  AssertionError: member-panel не скрыт при 1199px   ('flex' !== 'none')
✖ скрытие панели перекрывает body[data-active-screen=chat]
  AssertionError: нет media-query, покрывающего 960px
```

(3 зелёных строки — регресс-якоря: базовые 240/240 и aria/screen-switch уже
выполнялись до фикса.)

## 3. Green

`waves/RW-40/PM/dp01/green.log`, exit 0:

```
node --test tests/unit/gui-spike-{dom,model,bench,features,auth}.test.mjs
  ℹ tests 212  ℹ pass 212  ℹ fail 0
node --test tests/meta/design-breakpoints.test.mjs
  ℹ tests 6    ℹ pass 6    ℹ fail 0
npx tsc -p rust/gui-spike/ui/tsconfig.json
  exit 0 (без вывода)
node waves/RW-39/PM/w39c/geometry-check.mjs
  все 31 геометрических значений в допуске
```

`unit.log` — отдельный прогон 212 (те же 212/212).
Дополнительно: `node --test` по остальным meta-файлам — 34/34 pass.

## 4. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/tokens.css` | `b3c9d0782502c6815ac9e02ddccb2766` |
| `rust/gui-spike/ui/style.css` | `a9062b1cb3a2c0f06f93fec06ae5fe9b` |
| `tests/meta/design-breakpoints.test.mjs` (new) | `8d430d977d6071a002866dfd95fb2099` |

PROCESS §4.2: batch RW-40/DP-01, red/green выше, targeted — 212 unit + 6 meta,
gate — не запускался (запрещён PM; требуется integration-лейн), риски — §5.

## 5. Остаток / что не сделано (сознательно)

1. **TDD-PLAN.md §8 — нужна строка (shared, правит Team Lead).**
   `tests/meta/plan-sync.test.mjs` красный ровно из-за нового файла
   (`tests/meta/design-breakpoints.test.mjs` без строки в §8). Предлагаемая строка:

   ```
   | `tests/meta/design-breakpoints.test.mjs` | — | мета: адаптив shell 1600/1200/960/720 (`--layout-tree-w`, `--layout-tree-min-w`, `#member-panel`, `#channel-tree`) |
   ```

   До её добавления полный meta-гейт не зелёный; остальные meta-тесты 34/34.
2. **DESIGN/tokens.css не синхронизирован** (файл RW-39/G3, вне моего мандата):
   добавленный блок `@media (max-width:959px)` нужно перенести в источник и
   отразить в SPEC §6 при следующей сверке. Базовые пары property/value не
   менялись, шапка `ui/tokens.css` обновлена (теперь перечисляет два отличия от
   источника, чтобы md5-сверка была честной).
3. **DP-02 не трогал** (`--row-message-track` 44 / `--row-member-track` 40,
   D-04 26/48/42) — отдельная карточка.
4. **DP-05 (runtime resize-тест)** — статический meta-тест проверяет каскад по
   ширинам, но не `getBoundingClientRect` в окне; live/окно запрещены.
5. **Мелкий UX-остаток вне allowed-файлов:** на ≤1199 кнопка «участники»
   (`features/actions.js`, `toggleMemberPanel`) меняет `aria-expanded`, хотя
   панель скрыта media-query. Это affordance, не поломка aria-hidden/экранов;
   фикс — в JS (нужен мандат на `ui/features/**`), лучше вместе с DP-05.
