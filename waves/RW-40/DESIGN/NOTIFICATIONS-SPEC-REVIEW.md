# G3-NOTIFICATIONS-SPEC review

Вердикт: **RETURN**

## P1

- **Условия звукового/desktop-уведомления для упоминаний неполны.** В матрице в [NOTIFICATIONS-SPEC.md](/home/user/orca/project/waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md:228) для `@user`, `@role` и `@everyone`/`@here` условие уведомления сводится к `!isMuted` (для ролей и everyone также к соответствующему `suppress_*`). Оно не учитывает `resolvedMessageNotifications === 2` / `allowNoMessages`. Выше тот же документ определяет `NO_MESSAGES` как отсутствие звуковых/desktop-уведомлений, а RE перечисляет `NO_MESSAGES` среди условий `allowNoMessages` (`NOTIFICATIONS-RE.md` §3.1, §6). Уточнить матрицу так, чтобы упоминания не порождали звуковое/desktop-уведомление при эффективном уровне `NO_MESSAGES`; бейдж упоминаний при этом может продолжать увеличиваться.

## P2

- **Клавиатурный контракт не покрывает все интерактивные элементы.** Таблица ARIA назначает радио- и checkbox-роли контролам строк `.notif-override-table` ([спецификация, строка 1333](/home/user/orca/project/waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md:1333)), но раздел клавиатурной модели задаёт стрелочную навигацию только для `.notif-level-list` модалки ([строка 1352](/home/user/orca/project/waves/RW-40/DESIGN/NOTIFICATIONS-SPEC.md:1352)). Не определены фокусирование и навигация по radio-контролам таблицы, Space/активация checkbox и открытие/закрытие `#channel-notif-menu` с его триггера. Дополнить контракт ожидаемыми клавишами, roving `tabindex` либо нативным поведением и возвратом фокуса.

## Проверенные пункты

- Per-guild и per-channel overrides, каскад channel → category → guild и поля подавления `suppress_everyone` / `suppress_roles` описаны и трассируются к `NOTIFICATIONS-RE.md`.
- Все шесть mute-пресетов присутствуют в том же порядке и со значениями `900`, `3600`, `10800`, `28800`, `86400`, `-1`.
- Спецификация содержит unread/mention индикаторы канала и гильдии, включая числовые бейджи, mute-состояние и доступные имена.
- CSS-правила раздела 3.2 используют только объявленные в `tokens.css` custom properties: проверено 59 уникальных `var()` токенов, пропущенных токенов нет. В CSS-правилах нет сырых hex/rgb/hsl цветов и `!important`.

Проверка выполнена только чтением назначенных исходников; исходники и спецификация не изменялись.
