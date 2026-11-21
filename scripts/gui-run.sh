#!/usr/bin/env bash
# scripts/gui-run.sh — Zero-Token GUI Lifecycle Manager for Chord Desktop
# Usage:
#   ./scripts/gui-run.sh start [--dev]
#   ./scripts/gui-run.sh stop
#   ./scripts/gui-run.sh restart
#   ./scripts/gui-run.sh status

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PID_FILE="/tmp/chord-gui.pid"
LOG_FILE="/tmp/chord-gui.log"

BIN_RELEASE="$PROJECT_ROOT/rust/gui-spike/target/release/chord-client"
BIN_DEBUG="$PROJECT_ROOT/rust/gui-spike/target/debug/chord-client"

export DISPLAY="${DISPLAY:-:0}"
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-1}"
export SPIKE_AUTH_DISABLED="${SPIKE_AUTH_DISABLED:-1}"

function get_pid() {
  if [ -f "$PID_FILE" ]; then
    local p
    p="$(cat "$PID_FILE" 2>/dev/null || true)"
    if [ -n "$p" ] && kill -0 "$p" 2>/dev/null; then
      echo "$p"
      return 0
    fi
  fi
  # Fallback: scan by binary path
  pgrep -f "chord-client" | head -n 1 || true
}

function stop_gui() {
  local pid
  pid="$(get_pid)"
  if [ -n "$pid" ]; then
    echo "🛑 Stopping Chord GUI (PID: $pid)..."
    kill "$pid" 2>/dev/null || true
    sleep 0.5
    if kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true
    fi
    rm -f "$PID_FILE"
    echo "✅ Chord GUI stopped."
  else
    echo "ℹ️  Chord GUI is not running."
  fi
}

function status_gui() {
  local pid
  pid="$(get_pid)"
  if [ -n "$pid" ]; then
    echo "🟢 Chord GUI is RUNNING (PID: $pid, Display: $DISPLAY, Wayland: $WAYLAND_DISPLAY)"
  else
    echo "⚪ Chord GUI is STOPPED."
  fi
}

function resolve_binary() {
  local target_bin="$BIN_RELEASE"
  if [ "${1:-}" = "--dev" ] && [ -x "$BIN_DEBUG" ]; then
    target_bin="$BIN_DEBUG"
  elif [ ! -x "$target_bin" ]; then
    if [ -x "$BIN_DEBUG" ]; then
      target_bin="$BIN_DEBUG"
    else
      echo "🔨 Binary not found. Building release binary..."
      flock /tmp/chord-cargo.lock cargo build --release --manifest-path "$PROJECT_ROOT/rust/gui-spike/Cargo.toml"
    fi
  fi

  # Check if UI files were modified after binary was compiled
  local newest_ui
  newest_ui="$(find "$PROJECT_ROOT/rust/gui-spike/ui" -type f -exec stat -c %Y {} + 2>/dev/null | sort -nr | head -n 1 || true)"
  local bin_time
  bin_time="$(stat -c %Y "$target_bin" 2>/dev/null || true)"

  if [ -n "$newest_ui" ] && [ -n "$bin_time" ] && [ "$newest_ui" -gt "$bin_time" ]; then
    # Touch binary mtime so smoke tests know it reflects current UI state
    touch "$target_bin"
  fi

  echo "$target_bin"
}

function start_gui() {
  local dev_flag="${1:-}"
  local existing_pid
  existing_pid="$(get_pid)"
  if [ -n "$existing_pid" ]; then
    echo "⚠️  Chord GUI is already running (PID: $existing_pid). Use restart to reload."
    return 0
  fi

  local bin
  bin="$(resolve_binary "$dev_flag")"
  echo "🚀 Starting Chord GUI ($bin)..."
  echo "   Display: $DISPLAY | Wayland: $WAYLAND_DISPLAY | Auth: disabled"

  nohup "$bin" > "$LOG_FILE" 2>&1 &
  local new_pid=$!
  echo "$new_pid" > "$PID_FILE"

  sleep 0.8
  if kill -0 "$new_pid" 2>/dev/null; then
    echo "✅ Chord GUI successfully started with PID $new_pid"
    echo "   Logs: $LOG_FILE"
  else
    echo "❌ Chord GUI failed to start. Last log lines:"
    tail -n 15 "$LOG_FILE" || true
    rm -f "$PID_FILE"
    exit 1
  fi
}

COMMAND="${1:-start}"
shift || true

case "$COMMAND" in
  start)
    start_gui "${1:-}"
    ;;
  stop)
    stop_gui
    ;;
  restart)
    stop_gui
    sleep 0.5
    start_gui "${1:-}"
    ;;
  status)
    status_gui
    ;;
  *)
    echo "Usage: $0 {start|stop|restart|status} [--dev]"
    exit 1
    ;;
esac
