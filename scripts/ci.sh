#!/usr/bin/env bash
# T-HARN-06 / P8-03 / TASK-621: локальный CI-контракт + headless Xvfb E2E runner.
#
# По умолчанию работает офлайн (репозиторий уже собран, сеть не нужна) и не публикует
# артефакты. `--online` разрешает cargo-сеть только для CI-раннера; `--dry-run` печатает
# план, ничего не запуская; `--skip-release` оставляет сборку бинарника отдельной джобе;
# `--scope <worker|release|qualify>` дополнительно запускает гейт с авто-обнаружением
# браузера и виртуальным дисплеем (`xvfb-run -a` при отсутствии активного DISPLAY).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_DIR="$ROOT/rust"
OFFLINE=(--offline)
DRY_RUN=0
SKIP_RELEASE=0
SCOPE=""

usage() {
    cat <<'EOF'
Использование: scripts/ci.sh [--dry-run] [--online] [--skip-release] [--scope worker|release|qualify]
  --dry-run                напечатать план и выйти (cargo/сеть не запускаются)
  --online                 не добавлять --offline к cargo-командам (только для CI-раннера)
  --skip-release           не собирать release-артефакт в этой джобе
  --scope <scope>          запустить гейт (worker|release|qualify) с авто-поиском браузера и xvfb-run -a
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=1; shift ;;
        --online) OFFLINE=(); shift ;;
        --skip-release) SKIP_RELEASE=1; shift ;;
        --scope)
            if [[ $# -lt 2 || -z "${2:-}" ]]; then
                echo "ci: --scope требует аргумент (worker|release|qualify)" >&2
                usage >&2
                exit 1
            fi
            SCOPE="$2"
            shift 2
            ;;
        --scope=*)
            SCOPE="${1#--scope=}"
            shift
            ;;
        -h|--help) usage; exit 0 ;;
        *) echo "ci: неизвестный аргумент: $1" >&2; usage >&2; exit 1 ;;
    esac
done

if [[ -n "$SCOPE" && "$SCOPE" != "worker" && "$SCOPE" != "release" && "$SCOPE" != "qualify" ]]; then
    echo "ci: неизвестный scope: $SCOPE (ожидается worker|release|qualify)" >&2
    usage >&2
    exit 1
fi

# Автоматическое обнаружение браузера по стандартным путям и flatpak (TASK-621).
detect_browser() {
    local explicit="${CHROME_BIN:-${CHROME_PATH:-}}"
    if [[ -n "$explicit" && -x "$explicit" ]]; then
        export CHROME_BIN="$explicit"
        return 0
    fi
    local candidates=(
        "/home/sanya/.local/bin/chromium"
        "/usr/bin/chromium"
        "/usr/bin/chromium-browser"
        "/usr/bin/google-chrome"
        "/usr/bin/google-chrome-stable"
        "/usr/bin/brave-browser"
        "/usr/bin/brave"
    )
    local candidate
    for candidate in "${candidates[@]}"; do
        if [[ -x "$candidate" ]]; then
            export CHROME_BIN="$candidate"
            return 0
        fi
    done
    local bin_name found
    for bin_name in chromium chromium-browser google-chrome google-chrome-stable chrome brave-browser brave; do
        if found="$(command -v "$bin_name" 2>/dev/null)" && [[ -n "$found" && -x "$found" ]]; then
            export CHROME_BIN="$found"
            return 0
        fi
    done
    if command -v flatpak >/dev/null 2>&1; then
        local app_id
        for app_id in com.brave.Browser org.chromium.Chromium com.google.Chrome; do
            if flatpak info "$app_id" >/dev/null 2>&1; then
                local wrapper="$ROOT/.gate/flatpak-browser-wrapper.sh"
                if [[ "$DRY_RUN" != 1 ]]; then
                    mkdir -p "$ROOT/.gate"
                    printf '#!/usr/bin/env bash\nexec flatpak run --filesystem=host %s "$@"\n' "$app_id" > "$wrapper"
                    chmod +x "$wrapper"
                fi
                export CHROME_BIN="$wrapper"
                return 0
            fi
        done
    fi
    return 1
}

# Проверка наличия активного X11 DISPLAY (TASK-621).
has_active_display() {
    if [[ -z "${DISPLAY:-}" ]]; then
        return 1
    fi
    if [[ "$DISPLAY" =~ ^:([0-9]+)(\.[0-9]+)?$ ]] && [[ -d /tmp/.X11-unix ]]; then
        [[ -S "/tmp/.X11-unix/X${BASH_REMATCH[1]}" ]]
        return $?
    fi
    return 0
}

run() {
    if [[ "$DRY_RUN" == 1 ]]; then
        printf '+ %s\n' "$*"
    else
        printf 'ci: %s\n' "$*"
        "$@"
    fi
}

cargo_run() {
    local subcommand="$1"
    shift
    if [[ "$DRY_RUN" == 1 ]]; then
        printf '+ cargo %s %s %s\n' "$subcommand" "${OFFLINE[*]}" "$*"
    else
        printf 'ci: cargo %s %s %s\n' "$subcommand" "${OFFLINE[*]}" "$*"
        cargo "$subcommand" "${OFFLINE[@]}" "$@"
    fi
}

if [[ "$DRY_RUN" == 1 ]]; then
    echo "ci: dry-run (сеть и cargo не запускаются)"
fi

if detect_browser; then
    echo "ci: browser detected: CHROME_BIN=$CHROME_BIN"
else
    echo "ci: browser: не обнаружен в стандартных путях" >&2
fi

if has_active_display; then
    echo "ci: display: активен DISPLAY=${DISPLAY}"
else
    echo "ci: display: активный DISPLAY отсутствует, используется xvfb-run -a (при наличии)"
fi

# 1. Форматирование
run cargo fmt --manifest-path "$RUST_DIR/Cargo.toml" --all -- --check

# 2. Линты: warnings — ошибки, все targets/features
cargo_run clippy --manifest-path "$RUST_DIR/Cargo.toml" \
    --workspace --all-targets --all-features -- -D warnings

# 3. Тесты workspace
cargo_run test --manifest-path "$RUST_DIR/Cargo.toml" --workspace

# 4. Микробенчмарк ETF (T-HARN-04 — отдельная карточка, здесь smoke производительности)
cargo_run run --manifest-path "$RUST_DIR/Cargo.toml" \
    --release -p discord-codec --example etf_bench -- --iterations 2000

# 5. cargo-deny: если инструмент не установлен, это явный skip, а не молчаливый успех.
if [[ "$DRY_RUN" == 1 ]]; then
    echo '+ cargo deny check (cwd=rust, если cargo-deny установлен; иначе SKIP)'
elif command -v cargo-deny >/dev/null 2>&1; then
    echo "ci: cargo deny check (cwd=rust)"
    (cd "$RUST_DIR" && cargo deny check)
else
    echo "ci: SKIP cargo-deny: инструмент не установлен; обязательный SBOM/license-check выполняется ниже" >&2
fi

# 6. Воспроизводимый SBOM и обязательная локальная проверка лицензий.
if [[ "$DRY_RUN" == 1 ]]; then
    echo "+ node scripts/sbom.mjs --check --output $RUST_DIR/dist/sbom.spdx.json (offline)"
else
    echo "ci: SBOM/license check (offline)"
    node "$ROOT/scripts/sbom.mjs" --check --output "$RUST_DIR/dist/sbom.spdx.json"
fi

# 7. Release-артефакт: сборка + sha256, без публикации.
if [[ "$SKIP_RELEASE" == 1 ]]; then
    echo "ci: SKIP release artifact (--skip-release)"
elif [[ "$DRY_RUN" == 1 ]]; then
    echo '+ bash scripts/release.sh --dry-run'
else
    release_args=()
    if [[ "${#OFFLINE[@]}" == 0 ]]; then
        release_args+=(--online)
    fi
    bash "$ROOT/scripts/release.sh" "${release_args[@]}"
fi

# 8. Опциональный запуск гейта (--scope worker|release|qualify) с xvfb-run -a при отсутствии DISPLAY.
if [[ -n "$SCOPE" ]]; then
    if [[ "$DRY_RUN" == 1 ]]; then
        if has_active_display; then
            echo "+ node scripts/gate.mjs --scope $SCOPE (CHROME_BIN=${CHROME_BIN:-auto})"
        else
            echo "+ xvfb-run -a node scripts/gate.mjs --scope $SCOPE (CHROME_BIN=${CHROME_BIN:-auto})"
        fi
    else
        if ! has_active_display && command -v xvfb-run >/dev/null 2>&1; then
            echo "ci: xvfb-run -a node scripts/gate.mjs --scope $SCOPE"
            xvfb-run -a node "$ROOT/scripts/gate.mjs" --scope "$SCOPE"
        else
            echo "ci: node scripts/gate.mjs --scope $SCOPE"
            node "$ROOT/scripts/gate.mjs" --scope "$SCOPE"
        fi
    fi
fi

echo "ci: OK"
