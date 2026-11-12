# BRIEF W39a — RE-референсы дизайна (Phase 1a, добить артефакты)

Владелец: W39a (RE/REA). PM: DeepSeek. Итерация: 1.
Проблема: `DESIGN/SPEC.md`, `tokens.css`, `DOM-CONTRACT.md` есть, а `REFERENCES.md`,
`tokens.json`, `layout.json` — **нет** (DoD §5.1 не выполнен).

## Файлы (только эти)

- `waves/RW-39/DESIGN/REFERENCES.md`
- `waves/RW-39/DESIGN/tokens.json`
- `waves/RW-39/DESIGN/layout.json`
- `waves/RW-39/DESIGN/README.md`
- логи/скрипты: `waves/RW-39/PM/w39a/`

## Источники (read-only, не копировать ассеты)

| Источник | Что взять |
|---|---|
| `reference-oracle/ui/00-app.png`, `05-dm-list.png`, `06-profile-popout.png`, `07-guild-menu.png` | замеры: rail/tree/header/composer/members, цвета |
| `reference-oracle/ui8/*` (JPG/HTML/JSON) | поиск, фильтры, эмодзи, настройки, участники |
| `reference-oracle/ui/tokens.json` | 4795 CSS-переменных, тема dark, сборка 1.0.162 |
| `waves/RW-37/RE/layout.json` | schema `rw37/re-layout-v1` (уже проверен) |
| `docs/re/frontend-d03.md` | контракты компонент |
| `waves/RW-39/DESIGN/SPEC.md`, `tokens.css` | уже принятые числа (сверить, не противоречить) |

REA: если нужен рантайм/бинарь — `open_binary` по локальному пути, `Evidence` в отчёт.
Для PNG/JSON дампа обычное чтение дешевле — фиксируй, чем именно мерил.

## Выход

1. `REFERENCES.md` — таблица: id референса → путь → что измерено → метод (px/dpr/токен) →
   статус `[M]`/`[T]`/`[L]`/`[A]`/`[F]` (те же коды, что в SPEC §1). Плюс раздел «чего нет».
2. `tokens.json` — машиночитаемая выжимка **используемых** токенов (имя → значение → источник
   `[T]`/`[M]`), согласованная с `tokens.css` 1:1. Полные 4795 не тащить.
3. `layout.json` — геометрия shell и 4 экранов (rail 64–72, tree ~240, header ~48,
   composer ~56, members ~240, строки 22–26/48, участник 42), schema `rw39/layout-v1`,
   поле `source` у каждого блока.
4. `README.md` — как воспроизвести замеры (команды), юр. рамка: значения переносим, ассеты нет.

## Проверка (обязательна)

```bash
node -e "for (const f of ['tokens.json','layout.json']) JSON.parse(require('fs').readFileSync('waves/RW-39/DESIGN/'+f));" 
rg -n "rail|tree|header|composer|members" waves/RW-39/DESIGN/layout.json | head
# сверка: каждое имя из tokens.json есть в tokens.css, значение совпадает
```

## Запрещено

- Копировать PNG/шрифты/иконки/спрайты Discord в репозиторий.
- Менять `SPEC.md`/`tokens.css`/`DOM-CONTRACT.md` (владелец W39b) — расхождения только
  списком «на возврат PM».
- Выдумывать числа без кода статуса `[A]` и обоснования.

## Отчёт

`waves/RW-39/PM/w39a/REPORT.md`: что измерено, чем, сколько блоков, расхождения с SPEC
(если есть — PM решает), файлы + md5. Без commit/push.
