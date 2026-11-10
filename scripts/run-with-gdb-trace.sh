#!/usr/bin/env bash
# run-with-gdb-trace.sh — Запуск финальной (release) версии discord-gui-spike под контролем GDB с полной трассировкой

set -euo pipefail

REPO_DIR="/home/sanya/orca/project"
# Автоматическое определение финальной исполняемой сборки
if [[ -n "${1:-}" ]]; then
    BIN="$1"
elif [[ -f "$REPO_DIR/rust/gui-spike/target/release/discord-gui-spike" ]]; then
    BIN="$REPO_DIR/rust/gui-spike/target/release/discord-gui-spike"
elif [[ -f "$REPO_DIR/rust/dist/discord-gui-spike" ]]; then
    BIN="$REPO_DIR/rust/dist/discord-gui-spike"
elif [[ -f "$REPO_DIR/rust/discord-gpui/target/release/discord-gpui" ]]; then
    BIN="$REPO_DIR/rust/discord-gpui/target/release/discord-gpui"
else
    BIN="$REPO_DIR/rust/target/release/discord-gui-spike"
fi
GDB_PY="/tmp/discord-gdb-trace.py"
GDB_CMD="/tmp/discord-trace.gdb"
GDB_LOG="/tmp/gdb-native-trace.log"
UI_LOG="/tmp/live-ui-telemetry.jsonl"
USER_SHARE_LOG="/home/sanya/.local/share/discord-client/user-telemetry.jsonl"
UNIFIED_LOG="/tmp/discord-full-user-activity.log"

if [[ ! -f "$BIN" ]]; then
    echo "Ошибка: Бинарник $BIN не найден."
    exit 1
fi

# 1. Подготовка GDB Python скрипта трассировки
cat << 'EOF' > "$GDB_PY"
import gdb
import time

class TraceBreakpoint(gdb.Breakpoint):
    def __init__(self, spec, label=None):
        super().__init__(spec, gdb.BP_BREAKPOINT)
        self.silent = True
        self.label = label or spec

    def stop(self):
        now = time.strftime("%H:%M:%S")
        thread = gdb.selected_thread()
        tid = thread.num if thread else "?"
        print(f"[GDB-NATIVE-CALL] [{now}] Thread #{tid} -> {self.label}", flush=True)
        return False

# Список отслеживаемых функций для финальной версии (Release) и отладочной (Debug)
functions_to_trace = [
    # Core Data & Telemetry
    ("discord_gui_spike::data::record_user_action_telemetry", "data::record_user_action_telemetry() [Click/Action Recorder]"),
    ("discord_gui_spike::data::channels", "data::channels() [Channel Tree Cache & Sorter]"),
    ("discord_gui_spike::data::members_page", "data::members_page() [Guild/DM Members Filter]"),
    ("discord_gui_spike::data::search_page", "data::search_page() [Message Search Engine]"),
    ("discord_gui_spike::data::set_system_audio_device", "data::set_system_audio_device() [Audio / Voice Switch]"),
    ("discord_gui_spike::data::ingest_live_dispatch", "data::ingest_live_dispatch() [Gateway Event Ingestion]"),
    ("discord_gui_spike::data::fetch_rest_channel_messages", "data::fetch_rest_channel_messages() [REST Messages Ingestion]"),
    ("discord_gui_spike::settings_store::SettingsStore::load", "settings_store::load() [Load User Settings]"),
    ("discord_gui_spike::settings_store::SettingsStore::save", "settings_store::save() [Save User Settings]"),
    # IPC commands
    ("discord_gui_spike::ready", "ready() [BFF Core State]"),
    ("discord_gui_spike::read_state", "read_state() [Read Markers & Mentions]"),
    ("discord_gui_spike::channels", "channels() [Channel Tree Loader]"),
    ("discord_gui_spike::messages_page", "messages_page() [Chat Messages Virtual List]"),
    ("discord_gui_spike::members_page", "members_page() [Guild/DM Members List]"),
    ("discord_gui_spike::settings", "settings() [User Settings Store]"),
    ("discord_gui_spike::search_page", "search_page() [Local Messages Search]"),
    ("discord_gui_spike::search_remote", "search_remote() [Remote Guild Search]"),
    ("discord_gui_spike::send_message", "send_message() [Composer Send]"),
    ("discord_gui_spike::edit_message", "edit_message() [Message Edit]"),
    ("discord_gui_spike::delete_message", "delete_message() [Message Delete]"),
    ("discord_gui_spike::reply_message", "reply_message() [Message Reply]"),
    ("discord_gui_spike::ack_message", "ack_message() [Mark As Read]"),
    ("discord_gui_spike::toggle_reaction", "toggle_reaction() [Emoji Reaction]"),
    ("discord_gui_spike::send_typing", "send_typing() [Typing Indicator]"),
    ("discord_gui_spike::join_guild", "join_guild() [Guild Join]"),
    ("discord_gui_spike::leave_guild", "leave_guild() [Guild Leave]"),
    ("discord_gui_spike::resolve_invite", "resolve_invite() [Invite Preview]"),
    ("discord_gui_spike::accept_invite", "accept_invite() [Invite Accept]"),
    ("discord_gui_spike::pins_list", "pins_list() [Pinned Messages]"),
    ("discord_gui_spike::threads_list", "threads_list() [Active Threads]"),
    ("discord_gui_spike::voice_set_audio_device", "voice_set_audio_device() [Audio/Device Route]"),
    ("discord_gui_spike::voice_connect", "voice_connect() [Voice/DM Call Connect]"),
    ("discord_gui_spike::voice_disconnect", "voice_disconnect() [Voice/DM Call Disconnect]"),
    ("discord_gui_spike::voice_stream_start", "voice_stream_start() [Voice Stream Broadcast Start]"),
    ("discord_gui_spike::voice_stream_stop", "voice_stream_stop() [Voice Stream Broadcast Stop]"),
    ("discord_gui_spike::voice_stream_watch", "voice_stream_watch() [Voice Stream Watch/Subscribe]"),
    ("discord_core::ws::queue_live_stream_watch", "ws::queue_live_stream_watch() [Gateway OP 20 STREAM_WATCH]"),
    ("discord_core::ws::queue_live_stream_delete", "ws::queue_live_stream_delete() [Gateway OP 19 STREAM_DELETE]"),
    ("discord_core::ws::queue_live_voice_state", "ws::queue_live_voice_state() [Gateway OP 4 VOICE_STATE_UPDATE]"),
    ("discord_gui_spike::data::remove_deleted_message", "data::remove_deleted_message() [Message Deletion Filter/Prune]"),
    ("discord_gui_spike::data::connect_or_update_live_voice_ext", "data::connect_or_update_live_voice_ext() [Live Voice/Stream State]"),
    ("discord_gui_spike::data::join_or_switch_voice_channel_with_message", "data::join_or_switch_voice_channel_with_message() [S4+V1 Voice Guard & FSM]"),
    ("discord_gui_spike::guilds::accept_invite_and_join_guild", "guilds::accept_invite_and_join_guild() [S1+S3 Invite & Screening]"),
    ("discord_gui_spike::permissions::check_allowed_channel_access", "permissions::check_allowed_channel_access() [S4 Channel Access Guard]"),
    ("discord_core::ws::build_guild_subscriptions_bulk_op37", "ws::build_guild_subscriptions_bulk_op37() [S2 Op 37 RangedGuilds]"),
    ("discord_core::ws::queue_live_dm_call_sync", "ws::queue_live_dm_call_sync() [OP 13 DM Call Sync]"),
    ("discord_voice::VoiceCallStateMachine::begin_signaling", "voice::VoiceCallStateMachine::begin_signaling() [V1 Call Signaling]"),
    ("discord_voice::srtp::DaveMlsEngine::on_session_description", "voice::DaveMlsEngine::on_session_description() [V3 DAVE Op 4 -> Op 26]"),
    # GPUI & Native Net / Media Hooks
    ("discord_gpui::state::AppState::toggle_reaction", "gpui::AppState::toggle_reaction() [Reaction Toggle]"),
    ("discord_gpui::state::AppState::pin_message", "gpui::AppState::pin_message() [Message Pin/Unpin Toggle]"),
    ("discord_gpui::state::AppState::delete_message", "gpui::AppState::delete_message() [Message Delete]"),
    ("discord_gpui::state::AppState::send_message", "gpui::AppState::send_message() [Message Send]"),
    ("discord_gpui::state::AppState::select_channel", "gpui::AppState::select_channel() [Channel Switch & History Ingestion]"),
    ("discord_gpui::state::AppState::select_guild", "gpui::AppState::select_guild() [Guild Switch & Channel Tree Sync]"),
    ("discord_gpui::net::delete_live_message_blocking", "gpui::net::delete_live_message_blocking() [REST DELETE /messages]"),
    ("discord_gpui::net::pin_live_message_blocking", "gpui::net::pin_live_message_blocking() [REST PUT /pins]"),
    ("discord_gpui::net::unpin_live_message_blocking", "gpui::net::unpin_live_message_blocking() [REST DELETE /pins]"),
    ("discord_gpui::net::fetch_channel_messages", "gpui::net::fetch_channel_messages() [REST GET /messages]"),
    ("discord_gpui::net::join_live_discord_invite", "gpui::net::join_live_discord_invite() [REST POST /invites]"),
    ("discord_gpui::video::ScreenCastManager::start_screencast", "gpui::video::ScreenCastManager::start_screencast() [Portal PipeWire Stream]"),
    ("discord_gpui::audio::VoiceUdpStreamer::bind_and_stream", "gpui::audio::VoiceUdpStreamer::bind_and_stream() [Voice UDP & Keepalive]"),
    ("discord_gpui::dispatch::DesktopNotificationManager::send_message_notification", "gpui::dispatch::DesktopNotificationManager::send_message_notification() [D-Bus Notification]"),
    ("discord_gpui::dispatch::SystemTrayManager::create_default_controller", "gpui::dispatch::SystemTrayManager::create_default_controller() [System Tray Menu]"),
    ("discord_gpui::dispatch::LocalRpcIpcServer::create_handshake_frame", "gpui::dispatch::LocalRpcIpcServer::create_handshake_frame() [Local RPC IPC 0..9]"),
]

installed = 0
for spec, label in functions_to_trace:
    try:
        TraceBreakpoint(spec, label)
        installed += 1
    except Exception:
        short_name = spec.split("::")[-1]
        try:
            TraceBreakpoint(short_name, label)
            installed += 1
        except Exception:
            pass

print(f"GDB Tracer: Успешно установлено {installed} нативных хуков/брейкпоинтов для финальной версии.", flush=True)
EOF

# 2. Подготовка GDB команд
cat << EOF > "$GDB_CMD"
set pagination off
set confirm off
set debuginfod enabled off
handle SIGPIPE nostop noprint pass
handle SIG32 nostop noprint pass
handle SIG33 nostop noprint pass
handle SIGCHLD nostop noprint pass
handle SIGALRM nostop noprint pass
file $BIN
source $GDB_PY
run
quit
EOF

# 3. Инициализация логов (сохранение накопленной UI-телеметрии между перезапусками)
mkdir -p "$(dirname "$USER_SHARE_LOG")"
> "$GDB_LOG"
touch "$UI_LOG"
touch "$USER_SHARE_LOG"
touch "$UNIFIED_LOG"

echo "================================================================="
echo "  🚀 Запуск ФИНАЛЬНОЙ ВЕРСИИ discord-gui-spike под контролем GDB"
echo "================================================================="
echo "  • Финальный бинарник: $BIN ($(du -h "$BIN" | cut -f1))"
echo "  • GDB нативный лог:   $GDB_LOG"
echo "  • UI телеметрия:      $USER_SHARE_LOG"
echo "  • Сводный лог:        $UNIFIED_LOG"
echo "  • Отчёт PM:           $REPO_DIR/waves/RW-41-ECC-INTEGRATION/PM-LIVE-TELEMETRY-REPORT.md"
echo "================================================================="

# 4. Запуск наблюдателей в фоне
node "$REPO_DIR/scripts/pm-telemetry-watcher.mjs" > /dev/null 2>&1 &
WATCHER_PID=$!

node "$REPO_DIR/scripts/unified-live-tracer.mjs" &
TRACER_PID=$!

cleanup() {
    echo ""
    echo "Завершение сессии трассировки..."
    kill $WATCHER_PID 2>/dev/null || true
    kill $TRACER_PID 2>/dev/null || true
    echo "Готово. Логи сохранены в $UNIFIED_LOG"
}
trap cleanup EXIT INT TERM

# 5. Запуск финального приложения в GDB
if [[ "${SPIKE_AUTH_DISABLED:-0}" == "1" ]]; then
    export SPIKE_AUTH_DISABLED=1
else
    unset SPIKE_AUTH_DISABLED
fi
export WEBKIT_SHOW_CONSOLE_MESSAGES=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1

gdb -q -x "$GDB_CMD" > "$GDB_LOG" 2>&1
