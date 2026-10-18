// Общие типы приложения discord-lab (src/**, bin/**, tests/**).
// Файл содержит только типы и в рантайме не подключается.
// Использование в JSDoc: /** @typedef {import("./types.js").CdpClient} CdpClient */

// --- CDP ---

/** Одна запись из GET /json/list. */
export interface CdpTarget {
  type: string;
  title?: string;
  url?: string;
  webSocketDebuggerUrl?: string;
}

/** Страница с гарантированным webSocketDebuggerUrl (результат pickTarget). */
export type PageTarget = CdpTarget & { webSocketDebuggerUrl: string };

/** Сообщение от CDP: ответ на команду (id) либо событие (method). */
export interface CdpMessage {
  id?: number;
  method?: string;
  params?: any;
  result?: any;
  error?: { code?: number; message: string };
}

/** Событие CDP, накопленное клиентом. */
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

/** WebSocket-подобный объект (Node WebSocket; в тестах — FakeWebSocket). */
export interface WebSocketLike {
  addEventListener(
    type: string,
    listener: (event: any) => void,
    options?: { once?: boolean },
  ): void;
  send(data: string): void;
  close(): void;
}

/** Конструктор WebSocket-подобного объекта. */
export interface WebSocketLikeConstructor {
  new (url: string): WebSocketLike;
}

/** Минимальная поверхность fetch, нужная CDP-клиенту. */
export type FetchLike = (
  url: string,
  init?: { signal?: AbortSignal },
) => Promise<{ ok: boolean; status: number; json(): Promise<any> }>;

/** Ответ page-скрипта на клик/открытие. */
export interface ClickOutcome {
  clicked?: boolean;
  opened?: boolean;
  reason?: string;
  aria?: string | null;
  text?: string;
  [key: string]: unknown;
}

// --- Профиль CPU ---

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

// --- Записи UI-прогона ---

/** Запись шага UI-прогона (строка в logs/ui-walk-*.jsonl). */
export interface WalkRecord {
  step: number;
  label: string;
  clicked: boolean;
  screenshot: string | null;
  consoleErrors: number;
  consoleErrorTexts: string[];
  topFunctions: TopFunction[];
  error: string | null;
}

/** Пункт навигации (меню), найденный на странице. */
export interface NavItem {
  text: string;
}

/** Итог восстановления прогресса прогона из журнала. */
export interface VisitedState {
  labels: Set<string>;
  maxStep: number;
}

/** Сводка ошибок консоли за шаг. */
export interface ConsoleIssues {
  count: number;
  texts: string[];
}

// --- Вызовы Frida ---

/** Запись о вызове функции (payload сообщения агента frida-log-calls.js). */
export interface CallRecord {
  type: "call";
  module: string;
  name: string;
  address: string;
  threadId: number;
  at: number;
  args: (string | null)[];
}

/** Итог сводки логов вызовов (паритет summarize-calls.sh). */
export interface CallsSummary {
  total: number;
  unique: number;
  csv: string;
}

// --- Инвентарь символов ---

/** Символ из таблицы символов модуля. */
export interface SymbolEntry {
  name: string;
  address: string;
  offset: string;
}

/** Экспорт модуля. */
export interface ExportEntry {
  name: string;
  type: string;
  address: string;
  offset: string;
}

/** Инвентарь одного модуля (ответ RPC inventory). */
export interface ModuleInfo {
  name: string;
  path: string;
  base: string;
  size: number;
  symbols: SymbolEntry[];
  exports: ExportEntry[];
  symbolsTruncated?: number;
  error: string | null;
}

/** Процесс, видимый Frida. */
export interface ProcessInfo {
  pid: number;
  name: string;
}

/** Счётчики инвентаря. */
export interface InventoryCounts {
  symbols: number;
  exports: number;
}

/** JSON-документ инвентаря (functions/pid-*.json). */
export interface InventoryJson {
  pid: number;
  process: string;
  at: string;
  modules: ModuleInfo[];
}

// --- Аргументы CLI ---

/** Разбор argv хост-скриптов (--key value / --key=value / --flag / --module). */
export interface ParsedArgs {
  _: string[];
  modules: string[];
  [key: string]: string | boolean | string[] | undefined;
}

// --- Отчёты ---

/** Запись nm --format=posix со значимым типом (T/t/W/w). */
export interface NmEntry {
  name: string;
  type: string;
  address: string;
  size: string;
}

/** Строка таблицы домена (functions/domains/<domain>.csv). */
export interface DomainRow {
  module: string;
  name: string;
  demangled: string;
  address: string;
  size: string;
}
