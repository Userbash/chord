#!/usr/bin/env bash
# Восстановление четырёх файлов RW-07 из бэкапа приёмки + touch (инцидент залипшего rlib).
set -euo pipefail
cd /home/user/orca/project/rust/crates/discord-core
B=/tmp/opencode/review-rw-07/backup/final
cp "$B/dispatch.rs" src/dispatch.rs
cp "$B/ws.rs" src/ws.rs
cp "$B/lib.rs" src/lib.rs
cp "$B/dispatch_router.rs" tests/dispatch_router.rs
touch src/dispatch.rs src/ws.rs src/lib.rs tests/dispatch_router.rs
md5sum src/dispatch.rs src/ws.rs src/lib.rs tests/dispatch_router.rs
