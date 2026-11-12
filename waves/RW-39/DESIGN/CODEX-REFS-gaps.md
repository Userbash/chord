## 1) Verdict

Подтверждено:

- `07-guild-menu` не содержит пригодного захвата: `log.json` сообщает `items: 0`, HTML имеет 41 байт; PNG показывает другой экран.
- `05-dm-list` содержит только shell: `05-dm-channels.json` равен `[]`, `log.json` сообщает `channels: 0`.
- `04b-search-results.json` пуст (`{}`), но PNG визуально содержит счётчики результатов.
- Геометрия меню сообщения и QuickSwitcher измерима непосредственно по PNG, поэтому текущие `[A]`-значения можно заменить на `[M]`.
- Day divider и light theme в дампе отсутствуют.
- Cozy подтверждается токенами и `data-density="cozy"`; отдельного PNG для сравнения плотности нет.

Основное расхождение: SPEC сейчас задаёт меню сообщения и QuickSwitcher как assumptions, хотя их внешние поверхности видны на PNG. Для QuickSwitcher измеренная ширина также расходится с текущим `--modal-w-md = 602px`.

## 2) Таблица

| id | элемент | измерено | источник | рекомендация | priority |
|---|---|---:|---|---|---|
| G-01 | Меню сообщения: внешняя поверхность | примерно `354×619 PNG` = `283.2×495.2 CSS px`; bbox около `x=1065.6, y=189.6` CSS `[M]` | `reference-oracle/ui/01-msg-menu.png`; PIL RGB-профиль по строкам/столбцам; dpr `1.25` из `reference-oracle/ui/log.json` | Заменить assumption `32px × 188px` на измеренные размеры контейнера для reference-профиля. Сохранить отдельные разделители и 15 пунктов | P1 |
| G-02 | Состав меню сообщения | `15` пунктов: `4` реакции + `11` действий `[M]` | `reference-oracle/ui/01-msg-menu-items.json`; `log.json` | Оставить состав как data-driven; текстовые пункты уже подтверждены JSON | P1 |
| G-03 | QuickSwitcher: внешняя поверхность | примерно `829×496 PNG` = `663.2×396.8 CSS px`; bbox около `x=684.8, y=355.2` CSS `[M]` | `reference-oracle/ui/03-quickswitcher.png`; PIL-профиль поверхности; dpr `1.25` из `ui/log.json` | Не использовать `--modal-w-md = 602px` для этого экрана; завести отдельную ширину около `663px` либо зафиксировать диапазон с допуском на rounded corners | P1 |
| G-04 | QuickSwitcher: результаты | `8` результатов `[M]` | `reference-oracle/ui/03-quickswitcher-results.json`; `log.json` | Сохранить 8 строк и scroll-capacity; геометрию контейнера обновить по G-03 | P1 |
| G-05 | Guild menu | gap: данных меню нет | `reference-oracle/ui/log.json` (`guild-menu items: 0`), `07-guild-menu.html` (`41` байт), `07-guild-menu.png` | Не выводить пункты, ширину или высоту из PNG; оставить Phase 2 как unresolved/gap | P0 |
| G-06 | DM list | gap: список пуст | `reference-oracle/ui/05-dm-channels.json` = `[]`; `reference-oracle/ui/log.json` (`dm-list channels: 0`) | Реализовывать только shell; данные, высоту строк и наполненность списка не выводить из этого захвата | P0 |
| G-07 | Search results data | JSON `{}` `[M]` как факт отсутствия данных; PNG показывает счётчики `3 302` и `49 887` `[M]` | `reference-oracle/ui/04b-search-results.json`, `reference-oracle/ui/04b-search-results.png`, `REFERENCES.md` R-18 | Счётчики можно использовать только как fixture/reference text; структуру строк и payload не считать измеренными | P1 |
| G-08 | Day divider | gap: PNG/JSON не содержит измерения | `SPEC-P3-02.md` DIV-05…DIV-07; `REFERENCES.md` G-10 | Оставить `24px`, линию `1px`, `--divider` как `[A]`; пометить как ожидающее отдельного захвата | P2 |
| G-09 | Cozy density | `data-density="cozy"` `[M]`; вертикальный spacing token `0.125rem` = `2px` `[T]`; compact gutter `80px` `[T]` | `DOM-CONTRACT.md` строка 76; `reference-oracle/ui/tokens.json`; `tokens.css` строки 320–325 | Cozy считать базовым режимом; не заявлять независимую pixel-проверку cozy без отдельного PNG | P1 |
| G-10 | Light theme | gap: light-theme capture отсутствует | `SPEC.md` §3 помечен `dark theme`; `tokens.json` содержит отдельные light-named токены, но не экранный reference | Не добавлять светлые цвета/геометрию по названию токенов; вынести в отдельный capture/task | P1 |

Метод измерения: PNG читались через `PIL.Image`; поверхности определялись по устойчивым RGB-полосам в строках и столбцах, координаты переводились из PNG в CSS через `PNG px / 1.25`. Antialiasing и скруглённые углы дают погрешность примерно 1–2 PNG px.

## 3) Пробелы и `[A]`-предложения

- `07-guild-menu`: честный gap. PNG показывает голосовой экран, а не меню сервера. Нельзя надёжно получить ни пункты, ни bbox.
- `05-dm-list`: честный gap по содержимому. Shell измерим, но список ЛС отсутствует.
- `04b-search-results`: честный gap по данным. Видимые числа результатов не заменяют JSON-модель сообщений.
- Меню сообщения: внешняя геометрия теперь `[M]`; внутренние высоты пунктов остаются `[A]`, если не измерять каждый row отдельно.
- QuickSwitcher: внешняя геометрия `[M]`; высота строки `32px` остаётся `[A]`, несмотря на наличие 8 результатов.
- Day divider: текущие `24px` и `1px` линия остаются `[A]`.
- Light theme: полностью unresolved.
- Cozy: есть контракт density и токены, но нет парного PNG для независимой pixel-сверки.

## 4) Что менять в SPEC-P3-02/вёрстке

1. В `SPEC-P3-02.md` около `MSG-28` и gap `G-01`:
   - заменить формулировку «геометрия не снята»;
   - добавить `[M]` для внешнего bbox меню: около `283.2×495.2px CSS`;
   - `--row-menu-item: 32px` оставить `[A]`, пока не сняты row-level rect.

2. В блоке `QS-04…QS-06` и `G-03`:
   - заменить `--modal-w-md = 602px` как единственную ширину;
   - указать измеренное reference-значение около `663.2px CSS`;
   - позиционирование `top: 20vh` оставить `[A]`, поскольку это не измерялось как CSS rect.

3. В `G-04`:
   - явно разделить `[M]` видимых счётчиков `3 302` и `49 887` и gap JSON-данных;
   - не задавать структуру результата по пустому `04b-search-results.json`.

4. В `DIV-05…DIV-07` и `G-10`:
   - сохранить `24px`, `1px`, `--divider`, но пометить все значения `[A]` до отдельного day-divider capture.

5. В `G-13`:
   - оставить light theme unresolved;
   - не расширять `tokens.css` светлой палитрой по косвенным `*-light-*` переменным из `tokens.json`.

6. Для cozy/compact:
   - оставить `data-density="cozy|compact"` на `<html>`;
   - сохранить `--message-gutter: 64px` и compact `80px`;
   - не утверждать pixel parity cozy без отдельного изображения.