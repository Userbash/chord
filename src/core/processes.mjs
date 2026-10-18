// @ts-check
// Отбор процессов Discord. Контракт: tests/unit/processes.test.mjs
// (паритет с scripts/logger.mjs: /discord/i, кроме vesktop).

/**
 * Процесс относится к Discord: имя содержит "discord" (без учёта регистра)
 * и не содержит "vesktop".
 * @param {string} name
 * @returns {boolean}
 */
export function isDiscordProcess(name) {
  return /discord/i.test(name) && !/vesktop/i.test(name);
}

/**
 * Оставляет только процессы Discord, сохраняя порядок.
 * @template {{ name: string }} T
 * @param {readonly T[]} processes
 * @returns {T[]}
 */
export function filterDiscordProcesses(processes) {
  return processes.filter((process) => isDiscordProcess(process.name));
}
