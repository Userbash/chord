#!/usr/bin/env bash
# T-HARN-06 / P8-03: локальный CI-контракт.
#
# По умолчанию работает офлайн (репозиторий уже собран, сеть не нужна) и не публикует
# артефакты. `--online` разрешает cargo-сеть только для CI-раннера; `--dry-run` печатает
# план, ничего не запуская; `--skip-release` оставляет сборку бинарника отдельной джобе.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_DIR="$ROOT/rust"
OFFLINE=(--offline)
DRY_RUN=0
SKIP_RELEASE=0

usage() {
    cat <<'EOF'
Использование: scripts/ci.sh [--dry-run] [--online] [--skip-release]
  --dry-run       напечатать план и выйти (cargo/сеть не запускаются)
  --online        не добавлять --offline к cargo-командам (только для CI-раннера)
  --skip-release  не собирать release-артефакт в этой джобе
EOF
}

for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=1 ;;
        --online) OFFLINE=() ;;
        --skip-release) SKIP_RELEASE=1 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "ci: неизвестный аргумент: $arg" >&2; usage >&2; exit 1 ;;
    esac
done

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

echo "ci: OK"
