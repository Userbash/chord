# C3a — light/cozy темы: отчёт G3 Design

Дата: 2026-10-24. Воркер: G3 Design. PM: DeepSeek.
Карточка: C3a (`waves/RW-40/TASKS.md`, «темы light/cozy»). Только документы; код не менялся.
Вход: `/home/sanya/re-deep/ui/tokens.json` (4795 переменных), `ui/00-app.png`, `ui/log.json`,
`waves/RW-39/DESIGN/{tokens.css,SPEC.md,REFERENCES.md}`, `waves/RW-40/DESIGN/DESIGN-MSG.md`,
`waves/RW-37/RE/layout.json`, `rust/gui-spike/ui/{tokens.css,index.html,features/settings.js}`.

## 1. Статус

**done (design, docs-only).** Спецификация light-темы и плотностей cozy/compact написана и
машинно проверена: JSON валиден, значения либо совпадают с токенами дампа, либо помечены
`[A]`, контраст AA пересчитан (160 пар). Окно/сеть/cargo/commit не запускались; `ui/**`,
`src/**`, `tests/**`, `package.json` не тронуты; ассеты RE не копировались.

## 2. Что найдено

| Вопрос | Ответ | Метка/источник |
|-|-|-|
| Light-тема в дампе есть? | **Нет.** `tokens.json` — плоская карта 4795 вычисленных свойств одного тёмного состояния | `[T]`, `scan-tokens.log` |
| Сколько light-имён? | 41, все компонентные (`chip-*` 16, `role-*` 6, `premium-*` 4, `lol-text-*` 2, `text-/icon-overlay-light`, `font-weight-light`, `custom-*-shadow-light`, `bg-gradient-*`) | `[T]` |
| Theme-level light-токены? | 0 (surface/text/border/status наборов нет) | `[T]` |
| Скриншоты | `ui/*.png` — 10 шт., все тёмные; light-скриншотов 0 | `[M]` |
| `00-app.png` | тёмный: средняя яркость 37.7/255, rail/tree `(18,18,20)`, chat `(26,26,30)`, composer `(34,35,39)` | `[M]` Pillow |
| Базовые роли дампа | `--background-base-low` L 13.333%, `--background-base-lowest` 7.451%, `--text-default` 94.118% — dark | `[T]` |
| `log.json` | поля темы нет, снимок один | `[D]` |

Вывод: light-палитра не измерена, а **выведена инверсией ролей** на нейтральной рампе дампа
(`neutral-1…100`, `red/green/yellow/blue/brand-*`); всё, что не выражается токеном, помечено
`[A]` и перечислено в §5. Это честный gap, а не выдача расчёта за замер.

## 3. Сколько [T] и [A]

Всего 57 переопределений light-темы (из них `[T]` 36, `[A]` 21) + 10 density-значений.

| Группа | Всего | `[T]` | `[A]` |
|-|-|-|-|
| Поверхности (8) + строки (2) | 10 | 10 | 0 |
| Текст | 11 | 11 | 0 |
| Акцент | 4 | 3 | 1 |
| Статусы/обратная связь | 14 | 10 | 4 |
| Границы/состояния/фокус/скроллбар | 12 | 2 | 10 |
| Тени | 6 | 0 | 6 |
| **Итого** | **57** | **36** | **21** |

Density: `[L]` 8 (cozy 5, compact 3), `[A]` 1 (compact `--row-h-group-start: 26px`),
`[F]` 1 (`--row-message-track: 44px`, не меняется).

`[A]`-значения — это осознанные решения, а не «забыли токен»: альфы бордеров/состояний
(в дампе они светлые), тени, `--accent-soft` 12%, `--danger-fill*` (AA), `--status-offline`,
`--status-streaming`. Единственное новое имя — `--warning-text` (`[T]` `yellow-460`);
в `tokens.css` его не было, в §4.3 описано, как добавить в dark-базу.

## 4. Контраст AA

Пересчёт по sRGB, 160 пар. Итоги:

- текст: минимум по разрешённым поверхностям — `--text` 8.42, `--text-muted` 4.54,
  `--text-faint` 4.53 (только `bg-app`/`bg-chat`), `--text-link`/`--info` 4.82,
  `--danger-text` 4.86, `--warning-text` 4.91, `--positive-text` 5.01, `--text-accent` 5.22;
- non-text (≥3): `--status-idle` 3.08, `--scrollbar-thumb` 3.12, `--status-online` 3.26,
  `--border-strong` 3.55, `--danger` 3.61, `--accent` 3.77;
- пары: белый на `--accent` 4.607, белый на `--danger-fill` 4.85,
  `--text-on-warning` на `--warning` 7.242.

Известные ограничения (задокументированы, не «зелёные»): `--text-muted` на `--surface-4`
даёт 4.39 — там использовать `--text-subtle`/`--text`; `--text-faint` ниже `--bg-app`/`--bg-chat`
не применять; `--warning` только как заливка (1.74 на белом), текст — `--warning-text`.

## 5. Пробелы

1. Light-тема и light-скриншоты в дампе отсутствуют; `[M]` только для факта тёмного экрана.
2. compact group start 26 — `[A]`: в `layout.json` есть только `message_compact=26` и
   `message_indent_compact=80`; отдельного group_start для compact нет.
3. D-04 (переменная высота 26/48/+22) не реализован в виртуализаторе: трек 44 заморожен
   тестами RW-38 (`[F]`), правка — W39d. Плотность сейчас меняет gutter/контент, но не шаг списка.
4. Нет `theme=system` (`prefers-color-scheme`): белый список настроек — только `dark/light`.
5. FOUC: тема применяется после async `settings_load`; pre-paint решение — вне C3a.
6. `--status-streaming` отсутствует в дампе (обе темы), значение `[A]`.
7. Тени light расчётные (`[A]`), пиксельно не проверялись.
8. Геометрия заливки hover/selected в compact (от gutter или на всю ширину) не измерена — `[A]`.
9. `--warning-text` — новое имя (нужно добавить в dark-базу); `--text-faint` и `--text-muted`
   имеют ограничения по поверхностям (4.53 на `bg-app`; 4.39 на `surface-4`).

## 6. Проверки

| Проверка | Результат | Лог |
|-|-|-|
| JSON валиден, схема `rw40/theme-spec-v1` | ok | `validate.log` |
| `[T]`-значения совпадают с дампом (36) | ok | `validate.log` |
| `[L]`-значения совпадают с `layout.json` (8) | ok | `validate.log` |
| Имена существуют в `tokens.css` (кроме `--warning-text`) | ok | `validate.log` |
| Контраст: 160 пар, text ≥4.5 / non-text ≥3 | ok | `validate.log`, `contrast.log` |
| Нет внешних ссылок/шрифтовых подключений в MD и JSON | пусто | `rg`, exit 1 |
| Негативный self-test (6 мутаций ловятся) | 6/6 | `selftest.log` |
| Всего проверок валидатора | **325 ok, 0 fail** | `validate.log` |

Self-test доказывает, что валидатор не пустой: он падает на подмене токена, неизвестном имени,
провале AA, расхождении с layout, внешней ссылке в документе и шрифтовом подключении в JSON.

## 7. Файлы и md5

| Файл | md5 |
|-|-|
| `waves/RW-40/THEME-SPEC.md` | `2d4cf3d8758de702d1b8620884a80d3a` |
| `waves/RW-40/theme-spec.json` | `4bce1692512bd288667a032de1bd2f5c` |
| `waves/RW-40/PM/c3a/validate.py` | `acbf688d1b6a09cb55a1a926ea1096f7` |
| `waves/RW-40/PM/c3a/contrast.py` | `4b129376a1b29a68f8509e00bb6f9b5e` |
| `waves/RW-40/PM/c3a/selftest.py` | `331a1847b96ad1ab9ccdf0e07f6bff25` |
| `waves/RW-40/PM/c3a/scan-tokens.log` | `b2e7423dc2467cb6fe47d4773bcf81f6` |
| `waves/RW-40/PM/c3a/validate.log` | `b1d20dfb3f405f8754cdac0b9f03f5c2` |
| `waves/RW-40/PM/c3a/contrast.log` | `c9c45fa65f914bd611463575912b97c5` |
| `waves/RW-40/PM/c3a/selftest.log` | `148b8d65f93597ef19d988cd6ab80058` |

md5 этого отчёта и всех артефактов — `waves/RW-40/PM/c3a/md5.txt` (снимается после записи
REPORT.md; сам себя файл не хеширует).

## 8. Что делает Frontend (вне C3a)

1. `:root[data-theme="light"]` с 57 переопределениями и `color-scheme: light` в `tokens.css`.
2. Блоки `:root[data-density="cozy"|"compact"]` по §3.3 спецификации.
3. Убрать `html { color-scheme: dark }` из `primitives.css`; `meta color-scheme` → `dark light`.
4. Добавить имя `--warning-text` в dark-базу (`#fdb833`).
5. Плотность сообщений (аватар, инлайн имени, высоты) — после D-04/W39d, не подгонкой тестов.
