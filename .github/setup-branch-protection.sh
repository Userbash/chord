# Скрипт настройки Branch Protection Rules через GitHub API
# Запускать ОДИН РАЗ: bash .github/setup-branch-protection.sh
#
# Требования: gh CLI авторизован, права admin на репозиторий

set -euo pipefail

REPO="Userbash/chord"
echo "🔧 Настройка Branch Protection Rules для $REPO"
echo ""

# ─────────────────────────────────────────────────────────────
# Функция применения правил к ветке
# ─────────────────────────────────────────────────────────────
protect_branch() {
  local branch="$1"
  local required_reviews="$2"
  local strict="$3"

  echo "→ Защита ветки: $branch (reviews: $required_reviews, strict: $strict)"

  gh api \
    --method PUT \
    "/repos/$REPO/branches/$branch/protection" \
    --input - <<EOF
{
  "required_status_checks": {
    "strict": $strict,
    "contexts": [
      "L0 · Format (rustfmt)",
      "L0 · Clippy (zero warnings)",
      "L0 · Tests (workspace)",
      "L0 · Harness build",
      "L1 · File Boundaries",
      "L2 · Contract Compatibility",
      "L3 · Secret Scan (gitleaks)",
      "✅ Gate Summary"
    ]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": {
    "dismiss_stale_reviews": true,
    "require_code_owner_reviews": true,
    "required_approving_review_count": $required_reviews,
    "require_last_push_approval": false
  },
  "restrictions": null,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "block_creations": false,
  "required_conversation_resolution": true,
  "lock_branch": false,
  "allow_fork_syncing": false
}
EOF

  echo "  ✅ $branch защищена"
}

# ─────────────────────────────────────────────────────────────
# Создать ветки если не существуют
# ─────────────────────────────────────────────────────────────
for branch in unstable testing beta main; do
  if ! gh api "/repos/$REPO/branches/$branch" &>/dev/null; then
    echo "→ Создание ветки $branch..."
    # Создать от HEAD текущей ветки
    SHA=$(gh api /repos/$REPO/git/ref/heads/release/developer-alpha-preview --jq '.object.sha')
    gh api --method POST "/repos/$REPO/git/refs" \
      -f ref="refs/heads/$branch" \
      -f sha="$SHA" || echo "  (ветка уже существует)"
  fi
done

# ─────────────────────────────────────────────────────────────
# Применить Branch Protection
# ─────────────────────────────────────────────────────────────
# unstable: 1 ревьюер, обновление ветки перед merge
protect_branch "unstable" 1 true

# testing: 2 ревьюера (PM + Tech Reviewer)
protect_branch "testing" 2 true

# beta: 2 ревьюера, строгий
protect_branch "beta" 2 true

# main: 2 ревьюера, максимальная защита
protect_branch "main" 2 true

# ─────────────────────────────────────────────────────────────
# Создать Labels
# ─────────────────────────────────────────────────────────────
echo ""
echo "→ Создание меток..."

create_label() {
  local name="$1" color="$2" desc="$3"
  gh label create "$name" --color "$color" --description "$desc" --repo "$REPO" 2>/dev/null \
    || gh label edit "$name" --color "$color" --description "$desc" --repo "$REPO" 2>/dev/null \
    || true
}

# Scope labels
create_label "scope:backend"  "0075ca" "G1 — Rust Core/State/Gateway"
create_label "scope:frontend" "e4e669" "G2 — UI/Views"
create_label "scope:voice"    "d93f0b" "G5 — Voice/Platform"
create_label "scope:tests"    "0e8a16" "G4 — Tests/QA"
create_label "scope:harness"  "1d76db" "E2E Harness"
create_label "scope:devops"   "5319e7" "DevOps/CI/Scripts"
create_label "scope:ecc"      "f9d0c4" "ECC Config/.agents"
create_label "scope:re"       "bfd4f2" "G7 — Reverse Engineering"

# Status labels
create_label "ai-task"        "0052cc" "Task for AI worker"
create_label "backlog"        "c5def5" "In backlog, not started"
create_label "in-progress"    "fbca04" "Currently being worked on"
create_label "in-review"      "0075ca" "Waiting for review"
create_label "docs-only"      "cfd3d7" "Documentation changes only"
create_label "dependabot"     "0366d6" "Automated dependency update"
create_label "⚠️ shared-file" "e4e669" "Requires coordinator approval"

# Priority labels
create_label "P0-critical"    "b60205" "P0 — blocking"
create_label "P1-high"        "d93f0b" "P1 — high priority"
create_label "P2-medium"      "fbca04" "P2 — medium priority"
create_label "P3-low"         "0e8a16" "P3 — low priority"

echo "  ✅ Метки созданы"

# ─────────────────────────────────────────────────────────────
# Включить Auto-merge для репозитория
# ─────────────────────────────────────────────────────────────
echo ""
echo "→ Включение auto-merge..."
gh api --method PATCH "/repos/$REPO" \
  -f allow_auto_merge=true \
  -f delete_branch_on_merge=true \
  -f allow_squash_merge=true \
  -f allow_merge_commit=false \
  -f allow_rebase_merge=false \
  --jq '.full_name' | xargs -I{} echo "  ✅ auto-merge включён для {}"

echo ""
echo "✅ Настройка завершена!"
echo ""
echo "Итого:"
echo "  • Branch Protection: unstable, testing, beta, main"
echo "  • Required checks: 8 gate jobs"
echo "  • Auto-merge: включён (squash)"
echo "  • Delete branch on merge: включён"
echo "  • Labels: созданы"
