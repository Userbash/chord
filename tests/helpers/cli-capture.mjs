// @ts-check
// Перехват stdout/stderr вызова CLI без запуска процесса (unit-уровень).

/**
 * Выполняет функцию, собирая console.log/console.error.
 * @param {() => Promise<number>} fn
 * @returns {Promise<{ code: number, stdout: string, stderr: string }>}
 */
export async function captureCli(fn) {
  const log = console.log;
  const error = console.error;
  let stdout = "";
  let stderr = "";
  console.log = (/** @type {unknown[]} */ ...args) => {
    stdout += `${args.map(String).join(" ")}\n`;
  };
  console.error = (/** @type {unknown[]} */ ...args) => {
    stderr += `${args.map(String).join(" ")}\n`;
  };
  try {
    const code = await fn();
    return { code, stdout, stderr };
  } finally {
    console.log = log;
    console.error = error;
  }
}
