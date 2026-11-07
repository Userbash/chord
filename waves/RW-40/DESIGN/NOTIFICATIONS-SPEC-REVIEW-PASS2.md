# G3-NOTIFICATIONS-SPEC review — Pass 2

Вердикт: **PASS** (P0=0, P1=0, P2=0).

## Проверка прежних замечаний

- **P1 закрыт.** В `NOTIFICATIONS-SPEC.md` §1.5 (строки 218–232) `allowNoMessages` явно включает эффективный `resolvedMessageNotifications === 2` (`NO_MESSAGES`), а условия звука/Desktop для `@user`, `@role` и `@everyone`/`@here` проверяют `!allowNoMessages`. Условия счётчиков отдельно сохраняют увеличение mention badge при `NO_MESSAGES`; role/everyone badges также учитывают соответствующие `suppress_*`.
- **P2 закрыт.** В §5.4 (строки 1349–1358) описаны клавиатурное открытие `#channel-notif-menu`, Escape с возвратом фокуса к сохранённому opener, а также roving `tabindex` для radio-контролов каждой строки `.notif-override-table` с ArrowLeft/ArrowRight, активация checkbox клавишей Space и возврат фокуса при закрытии диалога.
- **CSS-токены проверены.** Все 66 уникальных custom properties, упомянутых через `var(--...)` в спецификации, объявлены в `rust/gui-spike/ui/tokens.css`; отсутствующих свойств нет.

Проверка выполнена чтением назначенных файлов и поиском объявлений токенов. Продуктовый код, тесты и спецификация не изменялись.
