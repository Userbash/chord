# BRIEF W39g — дизайн P3-02 «чат-вью» по референсам

Владелец: W39g (Design). PM: DeepSeek. Вход: `waves/RW-39/HANDOFF.md` §1/§4 + принятые
`DESIGN/{tokens.css,layout.json,SPEC.md,DOM-CONTRACT.md}` + `DESIGN/RE-EXTRA.md` (если готов).

## Файлы (только эти)
- `waves/RW-39/DESIGN/SPEC-P3-02.md` (новый)
- `waves/RW-39/PM/w39g/` (логи)
Всё остальное read-only. Ассеты Discord не копировать.

## Что зафиксировать (каждое число — с кодом источника)
1. Сообщение: cozy/compact, group start 48 / continuation 26 / перенос +22, gutter, avatar 32,
   hover-панель (состав из `01-msg-menu-items.json`), reply-quote, reactions-row, edited.
2. Embeds: link и image (бордер/радиус/паддинги/макс. ширина), вложение-файл.
3. Day divider и unread divider (32 px, `#5865f2`), notice-полоса 24/34 (см. discrepancies).
4. Композер: empty/typing/disabled, кнопки, лимит 2000, attach/emoji affordance.
5. Участники: 6 групп (роль/статус), заголовок 24, шаг 43, поиск, счётчики.
6. Search overlay: 7 секций (R-10), сортировка 3 пункта (R-11), результаты (R-18), переход.
7. QuickSwitcher: 8 результатов (R-17), Ctrl+K, клавиши.
8. Profile-popout 268.8×408 (R-06).

## Формат
Таблица `id | элемент | значение | [M]/[T]/[L]/[A]/[F] | источник | примечание`; отдельный
раздел «состояния» и «чего нет в дампе». Ссылки на `tokens.css`/`layout.json` вместо
дублирования значений.

## Проверка
```bash
rg -n "url\(|@font-face" waves/RW-39/DESIGN/SPEC-P3-02.md || true
node -e "const fs=require('fs');const t=fs.readFileSync('waves/RW-39/DESIGN/tokens.css','utf8');const s=fs.readFileSync('waves/RW-39/DESIGN/SPEC-P3-02.md','utf8');const used=[...s.matchAll(/--[a-z0-9-]+/g)].map(m=>m[0]);const miss=[...new Set(used)].filter(v=>!t.includes(v+':'));if(miss.length){console.error(miss);process.exit(1)}console.log('tokens ok',new Set(used).size)"
```
Окно/probe/cargo/gate/сеть/commit — запрещены.

## Отчёт
`waves/RW-39/PM/w39g/REPORT.md`: сколько пунктов, какие значения `[M]` vs `[A]`, чего нет,
md5 `SPEC-P3-02.md`.
