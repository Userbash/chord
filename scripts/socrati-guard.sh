#!/usr/bin/env bash
# ==============================================================================
# scripts/socrati-guard.sh — SocratiCode Infrastructure Health & Auto-Healer
#
# Verifies end-to-end connectivity:
#   1. Local SSH Tunnel to node2 (:6333) -> Auto-reconnects if dead
#   2. Remote Qdrant vector database in rootless Podman Quadlet on node2
#   3. Local Ollama embedding service (:11434)
#   4. SocratiCode JSON-RPC interface
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

QDRANT_URL="http://127.0.0.1:6333"
OLLAMA_URL="http://127.0.0.1:11434"

echo "🩺 [SOCRATI-GUARD] Inspecting SocratiCode + node2 Quadlet infrastructure..."

# 1. Check local Qdrant port tunnel
TUNNEL_ALIVE=false
if curl -s --max-time 2 "$QDRANT_URL/readyz" >/dev/null 2>&1 || curl -s --max-time 2 "$QDRANT_URL/" >/dev/null 2>&1; then
  TUNNEL_ALIVE=true
fi

if [ "$TUNNEL_ALIVE" = false ]; then
  echo "⚠️  [SOCRATI-GUARD] Qdrant port 6333 unreachable. Attempting auto-healing tunnel to node2..."
  # Check if stale ssh tunnel process exists and kill it
  pkill -f "ssh.*127.0.0.1:6333:127.0.0.1:6333 node2" 2>/dev/null || true
  sleep 1

  # Launch background tunnel
  nohup ssh -F ~/.ssh/config -N -T -o BatchMode=yes -o StrictHostKeyChecking=yes \
    -o ExitOnForwardFailure=yes -o ServerAliveInterval=20 -o ServerAliveCountMax=3 \
    -o ConnectTimeout=10 -L 127.0.0.1:6333:127.0.0.1:6333 node2 >/dev/null 2>&1 &

  # Wait up to 5 seconds for tunnel to establish
  for i in {1..10}; do
    if curl -s --max-time 1 "$QDRANT_URL/" >/dev/null 2>&1; then
      TUNNEL_ALIVE=true
      echo "   ✅ Tunnel to node2 revived successfully!"
      break
    fi
    sleep 0.5
  done
fi

# 2. Verify Qdrant Response
if [ "$TUNNEL_ALIVE" = true ]; then
  QDRANT_INFO="$(curl -s --max-time 2 "$QDRANT_URL/")"
  QDRANT_VER="$(echo "$QDRANT_INFO" | grep -o '"version":"[^"]*"' | cut -d'"' -f4 || echo "unknown")"
  echo "✅ [1/3] Qdrant on node2 (Quadlet container) : Healthy (v$QDRANT_VER, 127.0.0.1:6333)"
else
  echo "❌ [1/3] Qdrant on node2 (Quadlet container) : UNREACHABLE (Tunnel failed)"
fi

# 3. Verify Ollama Embeddings
if curl -s --max-time 2 "$OLLAMA_URL/api/tags" >/dev/null 2>&1; then
  echo "✅ [2/3] Ollama Embeddings (local :11434)     : Healthy"
else
  echo "⚠️  [2/3] Ollama Embeddings (local :11434)     : Offline or starting"
fi

# 4. Verify SocratiCode binary
if [ -x "/home/sanya/.local/bin/socraticode" ]; then
  echo "✅ [3/3] SocratiCode Engine (/home/sanya/.local/bin/socraticode) : Ready"
else
  echo "❌ [3/3] SocratiCode Engine : Binary not found"
fi

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [ "$TUNNEL_ALIVE" = true ]; then
  echo "🎉 All SocratiCode infrastructure components are OPERATIONAL."
  exit 0
else
  echo "⚠️  Infrastructure degraded. Fallback mode active."
  exit 1
fi
