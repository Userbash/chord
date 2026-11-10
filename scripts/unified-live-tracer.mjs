#!/usr/bin/env node
// @ts-check
/**
 * Unified Live Activity & GDB Tracer for discord-gui-spike
 *
 * Объединяет потоки:
 * 1. UI Telemetry (/tmp/live-ui-telemetry.jsonl) - клики, ввод, DOM-события, вызовы Tauri IPC.
 * 2. GDB Native Trace (/tmp/gdb-native-trace.log) - точки останова и нативные вызовы Rust/BFF в GDB.
 *
 * Записывает сводный журнал в `/tmp/discord-full-user-activity.log` и выводит ключевые события в stdout.
 */

import fs from "node:fs";
import path from "node:path";
import { execFile } from "node:child_process";

const UI_TELEMETRY_PATHS = [
  "/home/sanya/.local/share/discord-client/user-telemetry.jsonl",
  "/tmp/live-ui-telemetry.jsonl"
];
const GDB_TRACE_PATH = "/tmp/gdb-native-trace.log";
const LOG_OUTPUT_PATH = "/tmp/discord-full-user-activity.log";
const CAPTURE_DIR = "/tmp/trace-captures";
const TRACE_MD_PATH = "/home/sanya/orca/project/waves/RW-41-ECC-INTEGRATION/LIVE-CALL-AND-CLICK-TRACE.md";

fs.mkdirSync(CAPTURE_DIR, { recursive: true });
fs.mkdirSync(path.dirname(TRACE_MD_PATH), { recursive: true });

// Очищаем / инициализируем итоговый лог
fs.writeFileSync(LOG_OUTPUT_PATH, `=== Unified Trace Session Started at ${new Date().toISOString()} ===\n`, "utf8");

let shotSeq = 0;
let lastShotMs = 0;
/** @type {Array<{ ts: string, kind: string, ui: string, code: string, status: string, screenshot: string }>} */
const correlatedEvents = [];
/** @type {{ timeStr: string, label: string, handler: string, shot: string } | null} */
let lastUiClick = null;
let lastGdbCall = "";

/**
 * Делает фоновый скриншот окна/экрана через spectacle (KDE Wayland) с защитой от частых вызовов.
 * @param {string} tag
 * @returns {string}
 */
function captureScreenshotAsync(tag) {
  const now = Date.now();
  if (now - lastShotMs < 1500) {
    return "";
  }
  lastShotMs = now;
  shotSeq += 1;
  const safeTag = String(tag || "event").replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 32);
  const outPath = path.join(CAPTURE_DIR, `shot-${String(shotSeq).padStart(4, "0")}-${safeTag}.png`);
  execFile(
    "/usr/bin/spectacle",
    ["-f", "-b", "-n", "-o", outPath],
    { timeout: 4000, env: { ...process.env, DISPLAY: process.env.DISPLAY || ":0", WAYLAND_DISPLAY: process.env.WAYLAND_DISPLAY || "wayland-1", XDG_RUNTIME_DIR: process.env.XDG_RUNTIME_DIR || "/run/user/1000" } },
    () => {}
  );
  return outPath;
}

function flushMarkdownReport() {
  const rows = correlatedEvents.slice(-60).reverse();
  const md = [
    "# Live Call, Click, Screenshot & Code Correlation Trace",
    "",
    `- **Обновлено:** \`${new Date().toISOString()}\``,
    `- **Лог активности:** \`${LOG_OUTPUT_PATH}\``,
    `- **Директория скриншотов:** \`${CAPTURE_DIR}\` (снято: \`${shotSeq}\`)`,
    `- **Последний нативный вызов GDB:** \`${lastGdbCall || "ожидание"}\``,
    "",
    "## Таблица корреляции: Клик / Событие UI -> Код (JS/Rust) -> Статус -> Скриншот",
    "",
    "| Время | Тип | Действие / Элемент UI | Вызов на уровне кода (JS -> IPC -> Rust) | Статус / Результат | Скриншот |",
    "|---|---|---|---|---|---|",
  ];
  if (rows.length === 0) {
    md.push("| — | `init` | Сессия трассировки запущена | `discord_gui_spike::main` | `ready` | — |");
  } else {
    for (const r of rows) {
      const shotCell = r.screenshot ? `\`${path.basename(r.screenshot)}\`` : "—";
      md.push(`| \`${r.ts}\` | \`${r.kind}\` | ${r.ui.replace(/\|/g, "/")} | \`${r.code.replace(/\|/g, "/")}\` | ${r.status.replace(/\|/g, "/")} | ${shotCell} |`);
    }
  }
  fs.writeFileSync(TRACE_MD_PATH, md.join("\n") + "\n", "utf8");
}

/**
 * @param {string} line
 */
function appendLog(line) {
  fs.appendFileSync(LOG_OUTPUT_PATH, line + "\n", "utf8");
  console.log(line);
}

const offsets = new Map();
for (const p of UI_TELEMETRY_PATHS) {
  if (fs.existsSync(p)) {
    offsets.set(p, fs.statSync(p).size);
  }
}
let gdbOffset = fs.existsSync(GDB_TRACE_PATH) ? fs.statSync(GDB_TRACE_PATH).size : 0;

function pollUiTelemetry() {
  for (const filePath of UI_TELEMETRY_PATHS) {
    try {
      if (!fs.existsSync(filePath)) continue;
      const stat = fs.statSync(filePath);
      let offset = offsets.get(filePath) || 0;
      if (stat.size < offset) offset = 0;
      if (stat.size > offset) {
        const fd = fs.openSync(filePath, "r");
        const len = stat.size - offset;
        const buf = Buffer.alloc(len);
        fs.readSync(fd, buf, 0, len, offset);
        fs.closeSync(fd);
        offsets.set(filePath, stat.size);

        const lines = buf.toString("utf8").split("\n").filter(Boolean);
        for (const line of lines) {
          try {
            const ev = JSON.parse(line);
            const timeStr = ev.ts ? new Date(ev.ts).toLocaleTimeString() : new Date().toLocaleTimeString();

            if (ev.type === "click" || (ev.type === "ui_interaction" && ev.event === "click")) {
              const label = (ev.ariaLabel || ev.text || ev.id || ev.className || ev.tag || "element").trim().slice(0, 60);
              const shot = captureScreenshotAsync(`click_${ev.id || ev.tag || "ui"}`);
              lastUiClick = { timeStr, label, handler: `${ev.module || "ui/app.js"}::${ev.handler || "click"}`, shot };
              appendLog(`[${timeStr}] 🖱️  UI CLICK -> <${ev.tag || "elem"}> "${label}" | handler: ${ev.handler || "n/a"} (pos: ${ev.x},${ev.y})${shot ? ` [📸 ${path.basename(shot)}]` : ""}`);
              correlatedEvents.push({
                ts: timeStr,
                kind: "UI_CLICK",
                ui: `<${ev.tag || "elem"}> "${label}" (${ev.x},${ev.y})`,
                code: `${ev.module || "ui/app.js"}::${ev.handler || "onClick"}`,
                status: "dispatched",
                screenshot: shot,
              });
              flushMarkdownReport();
            } else if (ev.type === "ui_interaction" && ev.event === "change") {
              const label = (ev.ariaLabel || ev.id || ev.className || ev.tag || "input").trim().slice(0, 60);
              appendLog(`[${timeStr}] 🎛️  UI CHANGE -> <${ev.tag || "elem"}> "${label}" | handler: ${ev.handler || "n/a"}`);
            } else if (ev.type === "ipc_invoke") {
              const statusIcon = ev.status === "ok" ? "✅" : "❌";
              const dur = ev.durationMs !== undefined ? `${ev.durationMs}ms` : "?ms";
              const isBgPoll =
                ev.cmd === "voice_set_audio_device" &&
                (ev.args?.kind === "dm_friends_sync" || ev.args?.kind === "voice_states_sync" || ev.args?.kind === "ui_telemetry");
              if (isBgPoll) continue;
              const needShot = ev.status !== "ok" || /voice_connect|voice_disconnect|guild|invite|send_message|delete_message/.test(String(ev.cmd || ""));
              const shot = needShot ? captureScreenshotAsync(`ipc_${ev.cmd}_${ev.status}`) : (lastUiClick?.shot || "");
              appendLog(`[${timeStr}] ⚡ IPC INVOKE -> "${ev.cmd}" ${statusIcon} [${dur}] target: ${ev.rustTarget || "rust-core"}${ev.error ? ` ERR: ${ev.error}` : ""}`);
              correlatedEvents.push({
                ts: timeStr,
                kind: ev.status === "ok" ? "IPC_OK" : "IPC_ERR",
                ui: lastUiClick ? `After click "${lastUiClick.label}"` : `IPC ${ev.cmd}`,
                code: `${(ev.callerChain || []).slice(0, 2).join("->") || "ui"} => ${ev.rustTarget || ev.cmd}`,
                status: `${statusIcon} ${ev.status} (${dur})${ev.error ? `: ${ev.error}` : ""}`,
                screenshot: shot,
              });
              flushMarkdownReport();
            } else if (ev.layer === "backend_rest") {
              const statusIcon = ev.status >= 200 && ev.status < 300 ? "🌐" : "⚠️";
              appendLog(`[${timeStr}] ${statusIcon} REST API -> ${ev.method} ${ev.path} HTTP ${ev.status} (${ev.rust_fn})`);
            } else if (ev.type === "error" || ev.type === "js_error" || ev.type === "js_unhandled_rejection" || ev.error) {
              const shot = captureScreenshotAsync("ui_error");
              const errMsg = JSON.stringify(ev.error || ev.message || ev.reason || ev);
              appendLog(`[${timeStr}] ⚠️ UI ERROR -> ${errMsg}${shot ? ` [📸 ${path.basename(shot)}]` : ""}`);
              correlatedEvents.push({
                ts: timeStr,
                kind: "ERROR",
                ui: lastUiClick ? `During "${lastUiClick.label}"` : "window.onerror",
                code: `${ev.filename || "ui/app.js"}:${ev.lineno || 0}`,
                status: `❌ ${errMsg.slice(0, 100)}`,
                screenshot: shot,
              });
              flushMarkdownReport();
            }
          } catch {
            // pass
          }
        }
      }
    } catch {
      // ignore
    }
  }
}

function pollGdbTrace() {
  try {
    if (!fs.existsSync(GDB_TRACE_PATH)) return;
    const stat = fs.statSync(GDB_TRACE_PATH);
    if (stat.size < gdbOffset) gdbOffset = 0;
    if (stat.size > gdbOffset) {
      const fd = fs.openSync(GDB_TRACE_PATH, "r");
      const len = stat.size - gdbOffset;
      const buf = Buffer.alloc(len);
      fs.readSync(fd, buf, 0, len, gdbOffset);
      fs.closeSync(fd);
      gdbOffset = stat.size;

      const lines = buf.toString("utf8").split("\n").filter(Boolean);
      for (const line of lines) {
        if (line.includes("[GDB-NATIVE-CALL]")) {
          lastGdbCall = line.replace(/^.*->\s*/, "").trim();
          appendLog(`🔧 ${line}`);
        } else if (line.includes("SIGSEGV") || line.includes("SIGABRT") || line.includes("panic")) {
          const shot = captureScreenshotAsync("gdb_crash");
          appendLog(`💥 [GDB-ALERT] ${line}`);
          correlatedEvents.push({
            ts: new Date().toLocaleTimeString(),
            kind: "GDB_ALERT",
            ui: "Native Process",
            code: lastGdbCall || "native",
            status: `💥 ${line.slice(0, 100)}`,
            screenshot: shot,
          });
          flushMarkdownReport();
        } else if (line.includes("GDB Tracer:")) {
          appendLog(`⚙️  ${line}`);
        }
      }
    }
  } catch {
    // ignore transient
  }
}

console.log("[TRACER] Unified tracer active. Logging to " + LOG_OUTPUT_PATH);
captureScreenshotAsync("startup");
flushMarkdownReport();
setInterval(pollUiTelemetry, 250);
setInterval(pollGdbTrace, 250);
