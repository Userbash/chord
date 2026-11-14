#!/usr/bin/env bash
# P8-03: release-артефакт без публикации.
#
# Кладёт бинарь и SHA256SUMS в rust/dist/. Сеть не нужна; по умолчанию cargo --offline.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_DIR="$ROOT/rust"
DIST="$RUST_DIR/dist"
OFFLINE=(--offline)
DRY_RUN=0

usage() {
    cat <<'EOF'
Использование: scripts/release.sh [--dry-run] [--online] [--output DIR]
  --dry-run    напечатать план, не собирать и не создавать файлы
  --online     не добавлять --offline к cargo build (CI-раннер)
  --output DIR каталог артефакта (по умолчанию rust/dist)
EOF
}

while [[ "$#" -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=1; shift ;;
        --online) OFFLINE=(); shift ;;
        --output)
            [[ "$#" -ge 2 ]] || { echo "release: --output требует путь" >&2; exit 1; }
            DIST="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "release: неизвестный аргумент: $1" >&2; usage >&2; exit 1 ;;
    esac
done

TARGET_DIR="${CARGO_TARGET_DIR:-$RUST_DIR/target}"
BIN="$TARGET_DIR/release/chord-rs"

if [[ "$DRY_RUN" == 1 ]]; then
    echo "release: dry-run (сеть, cargo и запись файлов не выполняются)"
    printf '+ cargo build %s--release -p chord-app --manifest-path %s\n' \
        "${OFFLINE[*]:+${OFFLINE[*]} }" "$RUST_DIR/Cargo.toml"
    echo "+ install -m 0755 $BIN $DIST/discord-rs"
    if [[ -f "$RUST_DIR/dist/sbom.spdx.json" ]]; then
        echo "+ install -m 0644 $RUST_DIR/dist/sbom.spdx.json $DIST/sbom.spdx.json"
        echo "+ sha256sum $DIST/discord-rs $DIST/sbom.spdx.json > $DIST/SHA256SUMS"
    else
        echo "+ sha256sum $DIST/discord-rs > $DIST/SHA256SUMS"
    fi
    exit 0
fi

mkdir -p "$DIST"
printf 'release: cargo build %s--release -p chord-app\n' "${OFFLINE[*]:+${OFFLINE[*]} }"
cargo build "${OFFLINE[@]}" --release -p chord-app --manifest-path "$RUST_DIR/Cargo.toml"

if [[ ! -x "$BIN" ]]; then
    echo "release: не найден бинарь $BIN" >&2
    exit 1
fi
install -m 0755 "$BIN" "$DIST/discord-rs"
SBOM_SRC="$RUST_DIR/dist/sbom.spdx.json"
SBOM_DEST="$DIST/sbom.spdx.json"
if [[ -f "$SBOM_SRC" ]]; then
    # -ef сравнивает реальные файлы, а не написание пути (rust/dist vs rust/./dist).
    if [[ ! -e "$SBOM_DEST" || ! "$SBOM_SRC" -ef "$SBOM_DEST" ]]; then
        install -m 0644 "$SBOM_SRC" "$SBOM_DEST"
    fi
    (
        cd "$DIST"
        files=()
        for f in discord-gpui discord-gpui-*.tar.gz discord-gui-spike discord-rs sbom.spdx.json; do
            [[ -f "$f" ]] && files+=("$f")
        done
        sha256sum "${files[@]}" > SHA256SUMS
    )
else
    (cd "$DIST" && sha256sum discord-rs > SHA256SUMS)
fi
echo "release: артефакт $DIST/discord-rs"
echo "release: checksum $DIST/SHA256SUMS"
