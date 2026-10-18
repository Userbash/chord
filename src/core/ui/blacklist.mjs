// @ts-check
// Запрещённые пункты UI-прогонов. Контракт: tests/unit/blacklist.test.mjs.

/**
 * Пункт запрещён к клику в UI-прогонах.
 *
 * Правило — объединение чёрных списков всех прогонов 2026-10-18
 * (ui-walk-settings, ui-walk-menus, ui-menu-extra): подстрока без учёта
 * регистра — необратимые и опасные действия: выход/покинуть сервер,
 * удалить, заглушить/mute/deafen, скрыть каналы, блокировка, статус,
 * отключиться/отписаться, reset/сбросить, очистить, забыть/forget.
 *
 * Обычные пункты («Мой аккаунт», «Профиль», «Копировать ID», «Пригласить», …)
 * не блокируются.
 * @param {string} label
 * @returns {boolean}
 */
export function isBlacklisted(label) {
  return /выйти|log ?out|sign ?out|покинуть сервер|leave server|удалить|delete|заблокировать|block|заглушить|mute|deafen|скрыт|hide|статус|status|отключиться|отписаться|unsubscribe|reset|сбросить|очистить|clear|забыть|forget/i.test(label);
}
