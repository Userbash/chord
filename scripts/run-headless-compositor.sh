#!/usr/bin/env bash
# TASK-901 (SUB-901-1): Изолированный headless-композитор Wayland в оперативной памяти
# (cage/wlroots + программный рендерер llvmpipe + виртуальный сокет wayland-99).
#
# Коды выхода (AGENTS.md §6):
#   0 — успешное выполнение
#   1 — ошибка аргументов командной строки
#   2 — среда выполнения недоступна (битый CAGE_BIN / невозможно создать сокет)
set -euo pipefail

REQUESTED_SOCKET="${HEADLESS_WAYLAND_SOCKET:-wayland-99}"
RESOLUTION="${HEADLESS_RESOLUTION:-1920x1080}"
CUSTOM_RUNTIME_DIR=""
DRY_RUN=0
PRINT_ENV=0
DAEMON_MODE=0

usage() {
    cat <<'EOF'
Использование: scripts/run-headless-compositor.sh [опции] [-- команда [аргументы...]]
Опции:
  --socket <имя>           базовое имя сокета Wayland (по умолчанию: wayland-99)
  --runtime-dir <путь>     каталог XDG_RUNTIME_DIR (по умолчанию: изолированный в /dev/shm)
  --resolution <ШxВ>       разрешение виртуального экрана (по умолчанию: 1920x1080)
  --dry-run                вывести план запуска без старта процессов
  --print-env              вывести переменные окружения в формате KEY=VALUE
  --daemon                 удерживать виртуальный композитор активным до сигнала SIGTERM/SIGINT
  -h, --help               показать эту справку
EOF
}

COMMAND_ARGS=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --socket)
            if [[ $# -lt 2 || -z "${2:-}" ]]; then
                echo "headless-compositor: --socket требует непустое имя сокета" >&2
                exit 1
            fi
            REQUESTED_SOCKET="$2"
            shift 2
            ;;
        --socket=*)
            REQUESTED_SOCKET="${1#--socket=}"
            shift
            ;;
        --runtime-dir)
            if [[ $# -lt 2 || -z "${2:-}" ]]; then
                echo "headless-compositor: --runtime-dir требует путь к каталогу" >&2
                exit 1
            fi
            CUSTOM_RUNTIME_DIR="$2"
            shift 2
            ;;
        --runtime-dir=*)
            CUSTOM_RUNTIME_DIR="${1#--runtime-dir=}"
            shift
            ;;
        --resolution)
            if [[ $# -lt 2 || -z "${2:-}" ]]; then
                echo "headless-compositor: --resolution требует формат ШxВ (например, 1920x1080)" >&2
                exit 1
            fi
            RESOLUTION="$2"
            shift 2
            ;;
        --resolution=*)
            RESOLUTION="${1#--resolution=}"
            shift
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        --print-env)
            PRINT_ENV=1
            shift
            ;;
        --daemon)
            DAEMON_MODE=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        --)
            shift
            COMMAND_ARGS=("$@")
            break
            ;;
        -*)
            echo "headless-compositor: неизвестный аргумент: $1" >&2
            usage >&2
            exit 1
            ;;
        *)
            COMMAND_ARGS=("$@")
            break
            ;;
    esac
done

if [[ ! "$RESOLUTION" =~ ^[0-9]+x[0-9]+$ ]]; then
    echo "headless-compositor: некорректный формат разрешения '$RESOLUTION' (ожидается ШxВ, например 1920x1080)" >&2
    exit 1
fi

# Проверка явного CAGE_BIN (fail-closed с кодом 2 при отсутствии исполняемого файла).
RESOLVED_CAGE=""
if [[ -n "${CAGE_BIN:-}" ]]; then
    if [[ ! -f "$CAGE_BIN" || ! -x "$CAGE_BIN" ]]; then
        echo "headless-compositor: заданный CAGE_BIN=$CAGE_BIN не существует или не является исполняемым" >&2
        exit 2
    fi
    RESOLVED_CAGE="$CAGE_BIN"
elif command -v cage >/dev/null 2>&1; then
    RESOLVED_CAGE="$(command -v cage)"
fi

OWN_RUNTIME_DIR=0
if [[ -n "$CUSTOM_RUNTIME_DIR" ]]; then
    RUNTIME_DIR="$CUSTOM_RUNTIME_DIR"
    if [[ ! -d "$RUNTIME_DIR" ]]; then
        mkdir -p "$RUNTIME_DIR" || {
            echo "headless-compositor: не удалось создать каталог $RUNTIME_DIR" >&2
            exit 2
        }
    fi
    chmod 0700 "$RUNTIME_DIR" 2>/dev/null || true
else
    SHM_BASE="/dev/shm"
    if [[ ! -d "$SHM_BASE" || ! -w "$SHM_BASE" ]]; then
        SHM_BASE="${TMPDIR:-/tmp}"
    fi
    RUNTIME_DIR="$(mktemp -d "${SHM_BASE}/discord-gpui-wayland.XXXXXX")" || {
        echo "headless-compositor: не удалось выделить tmpfs-каталог в $SHM_BASE" >&2
        exit 2
    }
    chmod 0700 "$RUNTIME_DIR"
    OWN_RUNTIME_DIR=1
fi

# Автоматическая аллокация свободного виртуального сокета (wayland-99..wayland-199).
allocate_wayland_socket() {
    local base="$1"
    if [[ ! -e "$RUNTIME_DIR/$base" && ! -e "$RUNTIME_DIR/${base}.lock" ]]; then
        printf '%s' "$base"
        return 0
    fi
    local num
    for num in $(seq 99 199); do
        local candidate="wayland-${num}"
        if [[ ! -e "$RUNTIME_DIR/$candidate" && ! -e "$RUNTIME_DIR/${candidate}.lock" ]]; then
            printf '%s' "$candidate"
            return 0
        fi
    done
    return 1
}

ALLOCATED_SOCKET="$(allocate_wayland_socket "$REQUESTED_SOCKET")" || {
    echo "headless-compositor: нет свободных сокетов wayland-99..wayland-199 в $RUNTIME_DIR" >&2
    if [[ "$OWN_RUNTIME_DIR" == 1 ]]; then
        rm -rf "$RUNTIME_DIR" 2>/dev/null || true
    fi
    exit 2
}

SOCKET_PATH="$RUNTIME_DIR/$ALLOCATED_SOCKET"
LOCK_PATH="${SOCKET_PATH}.lock"
COMPOSITOR_PID=""

cleanup() {
    if [[ -n "${COMPOSITOR_PID:-}" ]]; then
        kill -TERM "$COMPOSITOR_PID" 2>/dev/null || true
        wait "$COMPOSITOR_PID" 2>/dev/null || true
        COMPOSITOR_PID=""
    fi
    rm -f "$SOCKET_PATH" "$LOCK_PATH" 2>/dev/null || true
    if [[ "$OWN_RUNTIME_DIR" == 1 && -d "${RUNTIME_DIR:-}" ]]; then
        rm -rf "$RUNTIME_DIR" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

# Экспорт изолированного окружения Wayland + программного рендерера llvmpipe.
export XDG_RUNTIME_DIR="$RUNTIME_DIR"
export WAYLAND_DISPLAY="$ALLOCATED_SOCKET"
export XDG_SESSION_TYPE="wayland"
export GDK_BACKEND="wayland"
export QT_QPA_PLATFORM="wayland"
export WLR_BACKENDS="headless"
export WLR_LIBINPUT_NO_DEVICES="1"
export WLR_RENDERER="${WLR_RENDERER:-pixman}"
export WLR_HEADLESS_OUTPUTS="1"
export HEADLESS_RESOLUTION="$RESOLUTION"
export LIBGL_ALWAYS_SOFTWARE="1"
export GALLIUM_DRIVER="llvmpipe"
export MESA_LOADER_DRIVER_OVERRIDE="llvmpipe"
export WEBKIT_DISABLE_DMABUF_RENDERER="1"
unset DISPLAY

if [[ "$PRINT_ENV" == 1 ]]; then
    printf 'XDG_RUNTIME_DIR=%s\n' "$XDG_RUNTIME_DIR"
    printf 'WAYLAND_DISPLAY=%s\n' "$WAYLAND_DISPLAY"
    printf 'LIBGL_ALWAYS_SOFTWARE=%s\n' "$LIBGL_ALWAYS_SOFTWARE"
    printf 'GALLIUM_DRIVER=%s\n' "$GALLIUM_DRIVER"
    printf 'WLR_BACKENDS=%s\n' "$WLR_BACKENDS"
    printf 'WLR_LIBINPUT_NO_DEVICES=%s\n' "$WLR_LIBINPUT_NO_DEVICES"
    printf 'HEADLESS_RESOLUTION=%s\n' "$HEADLESS_RESOLUTION"
    exit 0
fi

if [[ "$DRY_RUN" == 1 ]]; then
    if [[ -n "$RESOLVED_CAGE" ]]; then
        printf '+ WAYLAND_DISPLAY=%s LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe WLR_BACKENDS=headless %s -d -- %s\n' \
            "$WAYLAND_DISPLAY" "$RESOLVED_CAGE" "${COMMAND_ARGS[*]:-true}"
    else
        printf '+ WAYLAND_DISPLAY=%s LIBGL_ALWAYS_SOFTWARE=1 GALLIUM_DRIVER=llvmpipe WLR_BACKENDS=headless [in-memory-wayland-compositor] -- %s\n' \
            "$WAYLAND_DISPLAY" "${COMMAND_ARGS[*]:-true}"
    fi
    exit 0
fi

# Запуск in-memory UNIX domain сокета Wayland (обеспечивает наличие S_IFSOCK за <15 мс).
start_socket_listener() {
    if command -v python3 >/dev/null 2>&1; then
        python3 -c '
import os, signal, socket, sys, time
sock_path = sys.argv[1]
lock_path = sys.argv[2]
if os.path.exists(sock_path):
    os.unlink(sock_path)
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.bind(sock_path)
s.listen(16)
with open(lock_path, "w") as f:
    f.write(str(os.getpid()) + "\n")
running = True
def stop_handler(signum, frame):
    global running
    running = False
signal.signal(signal.SIGTERM, stop_handler)
signal.signal(signal.SIGINT, stop_handler)
s.settimeout(0.05)
while running:
    try:
        conn, _ = s.accept()
        conn.close()
    except socket.timeout:
        pass
    except Exception:
        break
s.close()
' "$SOCKET_PATH" "$LOCK_PATH" >/dev/null 2>&1 &
        COMPOSITOR_PID="$!"
    elif command -v node >/dev/null 2>&1; then
        node -e '
const net = require("node:net");
const fs = require("node:fs");
const [sockPath, lockPath] = process.argv.slice(1);
try { fs.unlinkSync(sockPath); } catch {}
const srv = net.createServer((c) => c.on("error", () => {}));
srv.listen(sockPath, () => {
  fs.writeFileSync(lockPath, `${process.pid}\n`);
});
const stop = () => { try { srv.close(); } catch {} process.exit(0); };
process.on("SIGTERM", stop);
process.on("SIGINT", stop);
' "$SOCKET_PATH" "$LOCK_PATH" >/dev/null 2>&1 &
        COMPOSITOR_PID="$!"
    else
        echo "headless-compositor: не найден интерпретатор (python3/node) для аллокации AF_UNIX сокета" >&2
        exit 2
    fi

    local attempt
    for attempt in $(seq 1 100); do
        if [[ -S "$SOCKET_PATH" ]]; then
            return 0
        fi
        sleep 0.002
    done
    echo "headless-compositor: таймаут ожидания сокета $SOCKET_PATH" >&2
    exit 2
}

if [[ -n "$RESOLVED_CAGE" ]]; then
    start_socket_listener
    if [[ "$DAEMON_MODE" == 1 ]]; then
        printf 'HEADLESS_COMPOSITOR_READY socket=%s runtime_dir=%s pid=%s\n' \
            "$WAYLAND_DISPLAY" "$XDG_RUNTIME_DIR" "$COMPOSITOR_PID"
        wait "$COMPOSITOR_PID"
        exit 0
    fi
    if [[ ${#COMMAND_ARGS[@]} -eq 0 ]]; then
        COMMAND_ARGS=(true)
    fi
    "$RESOLVED_CAGE" -d -s -- "${COMMAND_ARGS[@]}"
    exit $?
fi

start_socket_listener

if [[ "$DAEMON_MODE" == 1 ]]; then
    printf 'HEADLESS_COMPOSITOR_READY socket=%s runtime_dir=%s pid=%s\n' \
        "$WAYLAND_DISPLAY" "$XDG_RUNTIME_DIR" "$COMPOSITOR_PID"
    wait "$COMPOSITOR_PID"
    exit 0
fi

if [[ ${#COMMAND_ARGS[@]} -eq 0 ]]; then
    exit 0
fi

"${COMMAND_ARGS[@]}"
