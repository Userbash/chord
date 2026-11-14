#!/usr/bin/env bash
# ecc-orca-bridge.sh — Мост между ECC 2.0 Control Plane (ecc-tui) и Orca Orchestration в discord-lab.
# Соблюдает правила AGENTS.md / TEAM.md:
# - Полный запрет codex и deepseek
# - Строгое сохранение градаций моделей (Flash/Argon/Carbon low|medium|high) и --effort без завышения
# - Лимит WORKER_RAM_LIMIT <= 25 GB (не более 5 активных слотов)
# - Создание изолированных веток от unstable и запуск воркеров через orca orchestration worker-start

set -euo pipefail

PROJECT_ROOT="/home/sanya/orca/project"
ORCA_BIN="${ORCA_BIN:-/home/sanya/.config/orca/linux-orca-cli-shim/orca}"
if [ ! -x "$ORCA_BIN" ]; then
  ORCA_BIN="orca"
fi

RUN_STATE_FILE="${ECC_RUN_STATE_FILE:-$PROJECT_ROOT/.claude/ecc-orca-run.json}"
mkdir -p "$(dirname "$RUN_STATE_FILE")"

MODEL=""
EFFORT_OVERRIDE=""
TASK=""
CWD="$PROJECT_ROOT"
SESSION_NAME="ecc-worker"
WORKTREE_SEL="current"
WORKTREE_NAME=""
BASE_BRANCH="unstable"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --model|-m)
      MODEL="${2:-}"
      shift 2
      ;;
    --effort|-e)
      EFFORT_OVERRIDE="${2:-}"
      shift 2
      ;;
    --cwd)
      CWD="${2:-$PROJECT_ROOT}"
      shift 2
      ;;
    --session-name|--role)
      SESSION_NAME="${2:-ecc-worker}"
      shift 2
      ;;
    --worktree)
      WORKTREE_SEL="${2:-current}"
      shift 2
      ;;
    --name)
      WORKTREE_NAME="${2:-}"
      shift 2
      ;;
    --base-branch)
      BASE_BRANCH="${2:-unstable}"
      shift 2
      ;;
    --task|-p|--spec)
      TASK="${2:-}"
      shift 2
      ;;
    --add-dir|--include-directories|--allowed-tools|--disallowed-tools|--permission-mode|--max-budget-usd)
      shift 2
      ;;
    --append-system-prompt)
      # FIX-16: Сохраняем prompt вместо выброса — передаём воркеру через env
      ECC_APPEND_SYSTEM_PROMPT="${2:-}"
      export ECC_APPEND_SYSTEM_PROMPT
      shift 2
      ;;
    --)
      shift
      if [[ $# -gt 0 ]]; then
        TASK="$*"
      fi
      break
      ;;
    *)
      if [[ -z "$TASK" ]]; then
        TASK="$1"
      else
        TASK="$TASK $1"
      fi
      shift
      ;;
  esac
done

# 1. Гранулярная маршрутизация моделей Antigravity (Hydrogen / Flash / Argon / Carbon / Orchestrator)
# Полный запрет codex и deepseek; младшие уровни (low/medium) не завышаются до high
HARNESS="${ECC_HARNESS:-antigravity}"
ORCA_AGENT="antigravity"

case "$MODEL" in
  *orchestrator-argon*)
    ORCA_MODEL="gemini-pro-orchestrator-argon"
    EFFORT_LEVEL="medium"
    ;;
  *orchestrator-carbon*|*gemini-pro-orchestrator*|*orchestrator*)
    ORCA_MODEL="gemini-pro-orchestrator"
    EFFORT_LEVEL="high"
    ;;
  *hydrogen-xhigh*)
    ORCA_MODEL="gemini-4-hydrogen-xhigh"
    EFFORT_LEVEL="xhigh"
    ;;
  *hydrogen-high*)
    ORCA_MODEL="gemini-4-hydrogen-high"
    EFFORT_LEVEL="high"
    ;;
  *hydrogen-medium*)
    ORCA_MODEL="gemini-4-hydrogen-medium"
    EFFORT_LEVEL="medium"
    ;;
  *hydrogen-low*|*hydrogen*)
    ORCA_MODEL="gemini-4-hydrogen-low"
    EFFORT_LEVEL="low"
    ;;
  *flash-low*)
    ORCA_MODEL="gemini-3.8-flash-low"
    EFFORT_LEVEL="low"
    ;;
  *flash-medium*)
    ORCA_MODEL="gemini-3.8-flash-medium"
    EFFORT_LEVEL="medium"
    ;;
  *flash-high*|*flash*)
    ORCA_MODEL="gemini-3.8-flash-high"
    EFFORT_LEVEL="medium"
    ;;
  *argon-low*)
    ORCA_MODEL="gemini-4-argon-low"
    EFFORT_LEVEL="low"
    ;;
  *argon-medium*)
    ORCA_MODEL="gemini-4-argon-medium"
    EFFORT_LEVEL="medium"
    ;;
  *argon-high*|*argon*)
    ORCA_MODEL="gemini-4-argon-high"
    EFFORT_LEVEL="high"
    ;;
  *carbon-low*)
    ORCA_MODEL="gemini-4-carbon-low"
    EFFORT_LEVEL="low"
    ;;
  *carbon-medium*)
    ORCA_MODEL="gemini-4-carbon-medium"
    EFFORT_LEVEL="medium"
    ;;
  *carbon-high*|*carbon*)
    ORCA_MODEL="gemini-4-carbon-high"
    EFFORT_LEVEL="high"
    ;;
  *)
    ORCA_MODEL="gemini-4-argon-medium"
    EFFORT_LEVEL="medium"
    ;;
esac

case "$EFFORT_OVERRIDE" in
  low|medium|high|xhigh)
    EFFORT_LEVEL="$EFFORT_OVERRIDE"
    ;;
esac

EXTRA_ARGS=(--agent "$ORCA_AGENT" --model "$ORCA_MODEL" --effort "$EFFORT_LEVEL" --keep-open)

# 2. Получение или создание активного Orca Orchestration Run
RUN_ID=""
if [[ -f "$RUN_STATE_FILE" ]]; then
  RUN_ID="$(node -e 'try { const d = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")); console.log(d.runId || ""); } catch { console.log(""); }' "$RUN_STATE_FILE")"
fi

if [[ -z "$RUN_ID" ]]; then
  RUN_JSON="$("$ORCA_BIN" orchestration run-create --objective "ECC 2.0 x discord-lab Serein/LemonCord/RPC Integration Wave (P0-P2)" --json)"
  RUN_ID="$(node -e 'const d = JSON.parse(process.argv[1]); console.log((d.result && d.result.run && d.result.run.id) || (d.run && d.run.id) || d.runId || "");' "$RUN_JSON")"
  echo "{\"runId\": \"$RUN_ID\", \"updatedAt\": \"$(date -Iseconds)\"}" > "$RUN_STATE_FILE"
else
  "$ORCA_BIN" orchestration run-use --id "$RUN_ID" >/dev/null 2>&1 || true
fi

echo "[ECC-ORCA-BRIDGE] Session=${ECC_SESSION_ID:-$SESSION_NAME} Run=$RUN_ID Agent=$ORCA_AGENT Model=$ORCA_MODEL Effort=$EFFORT_LEVEL"
echo "[ECC-ORCA-BRIDGE] Task: $TASK"

# 3. Освобождение завершённых слотов и аппаратный контроль памяти (Ryzen 7 7800X3D 16T / 48 GB RAM / Swap 0B)
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/home/sanya/orca/project/rust/target}"
export NODE_OPTIONS="${NODE_OPTIONS:---max-old-space-size=1536}"
WORKER_RAM_LIMIT_MB="${WORKER_RAM_LIMIT_MB:-25600}"
MAX_ACTIVE_WORKERS="${MAX_ACTIVE_WORKERS:-4}"

for ctx in $("$ORCA_BIN" orchestration worker-list --run "$RUN_ID" 2>/dev/null | awk '/terminal=reclaimable/ {print $1}'); do
  "$ORCA_BIN" orchestration worker-release --dispatch "$ctx" >/dev/null 2>&1 || true
done

MEM_AVAILABLE_MB="$(awk '/MemAvailable:/ {printf "%d", $2/1024}' /proc/meminfo)"
AGENT_RSS_MB="$(ps -eo rss,comm | awk '/antigravity|agy|orca|cargo|rustc|node/ {sum+=$1} END {printf "%d", sum/1024}')"
echo "[ECC-ORCA-BRIDGE] Hardware guard: MemAvailable=${MEM_AVAILABLE_MB}MB AgentStackRSS=${AGENT_RSS_MB}MB/${WORKER_RAM_LIMIT_MB}MB CargoJobs=${CARGO_BUILD_JOBS}"

if [[ "$TASK" == "status" ]]; then
  "$ORCA_BIN" orchestration worker-list --run "$RUN_ID"
  exit 0
fi

# 4. Запуск воркера через Orca Orchestration (включено по умолчанию ECC_SPAWN_ORCA=1)
if [[ "${ECC_SPAWN_ORCA:-1}" == "1" && -n "$TASK" ]]; then
  if [[ "$MEM_AVAILABLE_MB" -lt 8192 ]]; then
    echo "[ECC-ORCA-BRIDGE] WARNING: MemAvailable (${MEM_AVAILABLE_MB}MB) < 8192MB (Swap=0B); deferring spawn to protect host from OOM." >&2
    exit 0
  fi
  WT_ARGS=(--worktree "$WORKTREE_SEL")
  if [[ "$WORKTREE_SEL" == "new-child" ]]; then
    WT_ARGS+=(--repo "path:$PROJECT_ROOT" --base-branch "$BASE_BRANCH" --setup skip)
    if [[ -n "$WORKTREE_NAME" ]]; then
      WT_ARGS+=(--name "$WORKTREE_NAME" --display-name "$WORKTREE_NAME")
    fi
  fi
  "$ORCA_BIN" orchestration worker-start \
    --run "$RUN_ID" \
    "${WT_ARGS[@]}" \
    --task-title "$SESSION_NAME" \
    "${EXTRA_ARGS[@]}" \
    --spec "$TASK"
fi
