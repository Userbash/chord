#!/usr/bin/env bash
# Комплексный запуск фаззинга протоколов Discord и криптографии DAVE MLS v1
# через cargo-fuzz (libFuzzer) + AddressSanitizer / LeakSanitizer (TASK-903).
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

RUNS=1000000
SINGLE_TARGET=""
RUN_CMIN=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs)
      if [[ $# -lt 2 || ! "$2" =~ ^[0-9]+$ ]]; then
        echo "Ошибка аргументов: --runs требует положительное целое число" >&2
        exit 1
      fi
      RUNS="$2"
      shift 2
      ;;
    --target)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "Ошибка аргументов: --target требует имя fuzz-мишени" >&2
        exit 1
      fi
      SINGLE_TARGET="$2"
      shift 2
      ;;
    --cmin)
      RUN_CMIN=1
      shift
      ;;
    --help|-h)
      cat <<'EOF'
Использование: bash scripts/fuzz-ci.sh [--runs <N>] [--target <name>] [--cmin]

Опции:
  --runs <N>       Число итераций libFuzzer на каждую мишень (по умолчанию: 1000000)
  --target <name>  Запустить только одну указанную мишень
  --cmin           Минимизировать корпус после успешного прогона
EOF
      exit 0
      ;;
    *)
      echo "Неизвестный аргумент: $1" >&2
      exit 1
      ;;
  esac
done

ALL_TARGETS=(
  "fuzz_gateway_json_etf"
  "fuzz_voice_udp_packets"
  "fuzz_rtp_header_extensions"
  "fuzz_aes128_gcm_truncated8"
  "fuzz_anti_replay_window"
)

if [[ -n "$SINGLE_TARGET" ]]; then
  TARGETS=("$SINGLE_TARGET")
else
  TARGETS=("${ALL_TARGETS[@]}")
fi

if [[ -x "$ROOT_DIR/rust/.tools/bin/cargo-fuzz" ]]; then
  CARGO_FUZZ_BIN=("$ROOT_DIR/rust/.tools/bin/cargo-fuzz")
  export RUSTC_BOOTSTRAP="${RUSTC_BOOTSTRAP:-1}"
  export CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-true}"
elif command -v cargo-fuzz >/dev/null 2>&1; then
  CARGO_FUZZ_BIN=("cargo-fuzz")
else
  echo "Среда недоступна: бинарник cargo-fuzz не найден" >&2
  exit 2
fi

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/rust/target}"
LOG_DIR="$ROOT_DIR/.gate/fuzz-logs"
mkdir -p "$LOG_DIR"

OVERALL_RC=0

for target in "${TARGETS[@]}"; do
  CORPUS_DIR="$ROOT_DIR/tests/fuzz/corpus/$target"
  mkdir -p "$CORPUS_DIR"
  LOG_FILE="$LOG_DIR/${target}.log"

  MAX_LEN=2048
  if [[ "$target" == "fuzz_aes128_gcm_truncated8" ]]; then
    MAX_LEN=512
  fi

  echo "=== [fuzz-ci] Запуск мишени $target ($RUNS итераций, RSS <= 2048 МБ) ==="
  set +e
  "${CARGO_FUZZ_BIN[@]}" run --fuzz-dir "$ROOT_DIR/rust/fuzz" "$target" "$CORPUS_DIR" \
    -- -runs="$RUNS" -max_len="$MAX_LEN" -rss_limit_mb=2048 -timeout=5 2>&1 | tee "$LOG_FILE"
  TARGET_RC="${PIPESTATUS[0]}"
  set -e

  if [[ "$TARGET_RC" -ne 0 ]]; then
    echo "=== [fuzz-ci] СБОЙ на мишени $target (код выхода: $TARGET_RC) ===" >&2
    OVERALL_RC=1
  elif [[ "$RUN_CMIN" -eq 1 ]]; then
    echo "=== [fuzz-ci] Минимизация корпуса $target ==="
    "${CARGO_FUZZ_BIN[@]}" cmin --fuzz-dir "$ROOT_DIR/rust/fuzz" "$target" "$CORPUS_DIR" \
      -- -max_len="$MAX_LEN" -rss_limit_mb=2048 -timeout=5
  fi
done

node "$ROOT_DIR/scripts/fuzz-report.mjs" \
  --log-dir "$LOG_DIR" \
  --artifact-dir "$ROOT_DIR/rust/fuzz/artifacts" \
  --out-json "$ROOT_DIR/.gate/fuzz-report.json" \
  --out-md "$ROOT_DIR/.gate/fuzz-report.md" \
  --min-runs "$RUNS" || OVERALL_RC=1

exit "$OVERALL_RC"
