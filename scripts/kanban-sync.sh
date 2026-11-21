#!/usr/bin/env bash
# ==============================================================================
# scripts/kanban-sync.sh — Zero-Touch Kanban Board Auto-Sync
#
# Synchronizes waves/kanban-board.json with Git commits, PR merges, and file locks.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
KANBAN_FILE="$PROJECT_ROOT/waves/kanban-board.json"

command_help() {
  cat << 'EOF'
Usage:
  bash scripts/kanban-sync.sh status
  bash scripts/kanban-sync.sh move <CARD_ID> <backlog|in_dev|in_review|done>
  bash scripts/kanban-sync.sh lock <CARD_ID> [assignee]
  bash scripts/kanban-sync.sh done <CARD_ID>
  bash scripts/kanban-sync.sh auto
EOF
}

CMD="${1:-status}"

python3 -c '
import sys
import json
import os
import re

kanban_path = "'"$KANBAN_FILE"'"
cmd = "'"$CMD"'"
args = sys.argv[1:]

if not os.path.exists(kanban_path):
    print(f"❌ Kanban board not found at {kanban_path}")
    sys.exit(1)

with open(kanban_path, "r", encoding="utf-8") as f:
    board = json.load(f)

cards = board.get("cards", [])

if cmd == "status":
    counts = {}
    for c in cards:
        st = c.get("status", "unknown")
        counts[st] = counts.get(st, 0) + 1
    
    proj_name = board.get("project", "Chord")
    wave_name = board.get("wave", "RW-XX")
    total_cards = len(cards)
    print("\n📋 [KANBAN-SYNC] Kanban Board Status (waves/kanban-board.json)")
    print(f"   Project : {proj_name}")
    print(f"   Wave    : {wave_name}")
    print(f"   Total   : {total_cards} card(s)")
    print("----------------------------------------------------------------")
    for col in board.get("columns", []):
        cid = col["id"]
        title = col["title"]
        print(f"   {title.ljust(30)} : {counts.get(cid, 0)} cards")
    print("----------------------------------------------------------------\n")

elif cmd == "move":
    if len(args) < 2:
        print("Usage: kanban-sync move <CARD_ID> <STATUS>")
        sys.exit(1)
    card_id = args[0].upper()
    target_status = args[1].lower()
    
    valid_cols = [c["id"] for c in board.get("columns", [])]
    if target_status not in valid_cols:
        print(f"❌ Invalid status: {target_status}. Allowed: {valid_cols}")
        sys.exit(1)

    found = False
    for c in cards:
        if c.get("id", "").upper() == card_id:
            old_st = c.get("status")
            c["status"] = target_status
            c["column"] = target_status
            found = True
            print(f"✅ Moved {card_id}: {old_st} -> {target_status}")
            break
    
    if not found:
        print(f"⚠️  Card {card_id} not found on Kanban board.")
        sys.exit(1)
        
    with open(kanban_path, "w", encoding="utf-8") as f:
        json.dump(board, f, ensure_ascii=False, indent=2)

elif cmd == "lock":
    if len(args) < 1:
        print("Usage: kanban-sync lock <CARD_ID> [assignee]")
        sys.exit(1)
    card_id = args[0].upper()
    assignee = args[1] if len(args) > 1 else "worker"
    
    found = False
    for c in cards:
        if c.get("id", "").upper() == card_id:
            c["status"] = "in_dev"
            c["column"] = "in_dev"
            c["assignee"] = assignee
            found = True
            print(f"⚡ [KANBAN] Locked {card_id} -> status: in_dev, assignee: @{assignee}")
            break

    if found:
        with open(kanban_path, "w", encoding="utf-8") as f:
            json.dump(board, f, ensure_ascii=False, indent=2)
    else:
        print(f"⚠️  Card {card_id} not found.")

elif cmd == "done":
    if len(args) < 1:
        print("Usage: kanban-sync done <CARD_ID>")
        sys.exit(1)
    card_id = args[0].upper()
    for c in cards:
        if c.get("id", "").upper() == card_id:
            c["status"] = "done"
            c["column"] = "done"
            print(f"✅ [KANBAN] Marked {card_id} as DONE!")
            break
    with open(kanban_path, "w", encoding="utf-8") as f:
        json.dump(board, f, ensure_ascii=False, indent=2)

elif cmd == "auto":
    import subprocess
    # Read last 15 commit messages
    commits = subprocess.check_output(["git", "log", "-n", "15", "--oneline"], text=True).splitlines()
    updated = 0
    for line in commits:
        match = re.search(r"\b(TASK-[A-Z0-9_-]+)\b", line, re.IGNORECASE)
        if match:
            cid = match.group(1).upper()
            for c in cards:
                if c.get("id", "").upper() == cid and c.get("status") != "done":
                    c["status"] = "done"
                    c["column"] = "done"
                    updated += 1
                    print(f"🔄 Auto-synced from git: {cid} -> done")
    if updated > 0:
        with open(kanban_path, "w", encoding="utf-8") as f:
            json.dump(board, f, ensure_ascii=False, indent=2)
        print(f"✅ [KANBAN-AUTO] Synced {updated} card(s) from git history.")
    else:
        print("✅ [KANBAN-AUTO] Board is already in sync with git history.")
' "${@:2}"
