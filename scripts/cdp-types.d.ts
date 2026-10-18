// Общие типы для scripts/*.mjs (Chrome DevTools Protocol и записи UI-прогонов).
// Файл содержит только типы и в рантайме не подключается.
// Использование в JSDoc: /** @typedef {import("./cdp-types.js").CdpClient} CdpClient */

/** Одна запись из GET /json/list. */
export interface CdpTarget {
  type: string;
  title?: string;
  url?: string;
  webSocketDebuggerUrl?: string;
}

/** Сообщение от CDP: ответ на команду (id) либо событие (method). */
export interface CdpMessage {
  id?: number;
  method?: string;
  params?: any;
  result?: any;
  error?: { code?: number; message: string };
}

/** Событие CDP, накопленное клиентом UI-прогонов. */
export interface CdpEvent {
  at: number;
  method: string;
  params?: any;
}

/** Минимальный клиент CDP: отправка команд и закрытие соединения. */
export interface CdpClient {
  send(method: string, params?: Record<string, unknown>): Promise<any>;
  close(): void;
}

/** Клиент UI-прогонов: дополнительно копит события страницы. */
export interface CdpEventClient extends CdpClient {
  events: CdpEvent[];
}

/** Ответ page-скрипта на клик/открытие. */
export interface ClickOutcome {
  clicked?: boolean;
  opened?: boolean;
  reason?: string;
  aria?: string | null;
  text?: string;
  [key: string]: unknown;
}

/** Узел профиля CPU (ответ Profiler.stop). */
export interface CpuProfileNode {
  hitCount?: number;
  callFrame: {
    functionName?: string;
    url?: string;
    lineNumber?: number;
    columnNumber?: number;
  };
}

/** Профиль CPU (ответ Profiler.stop). */
export interface CpuProfile {
  nodes: CpuProfileNode[];
}

/** Функция из топа профиля вокруг шага. */
export interface TopFunction {
  fn: string;
  url: string;
  hits: number;
}

/** Запись шага UI-прогона (строка в logs/ui-walk-*.jsonl). */
export interface WalkRecord {
  step: number;
  label: string;
  clicked: boolean;
  screenshot: string | null;
  topFunctions: TopFunction[];
  error: string | null;
  consoleErrors?: number;
  consoleErrorTexts?: string[];
}
