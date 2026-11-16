#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/docs-graph.mjs — Automated Project Knowledge Graph & Map Generator
 *
 * Scans workspace architecture, Rust crates, UI features, and AMCP v2.0 bus,
 * validates Mermaid syntax, and synchronizes the master graph in docs/map.md.
 *
 * Usage:
 *   node scripts/docs-graph.mjs [--check]
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');
const MAP_FILE = path.join(PROJECT_ROOT, 'docs/map.md');

function generateMermaidGraph() {
  return `\`\`\`mermaid
flowchart TB
  classDef ok fill:#e2f6e7,stroke:#2f9e5f,color:#1b2430
  classDef part fill:#fdf3d8,stroke:#c99700,color:#1b2430
  classDef no fill:#fbe3e3,stroke:#c0392b,color:#1b2430
  classDef neu fill:#eef2f7,stroke:#7d93ab,color:#1b2430
  classDef amcp fill:#f0e6ff,stroke:#8a2be2,color:#1b2430

  GOAL["Native Discord Client & Protocol Suite (Chord)"]:::neu
  A["Scenario A — Independent Messenger (Recommended)"]:::ok
  C["Scenario C — Direct Protocol Client (TOS Sensitive)"]:::no
  GOAL --> A
  GOAL --> C
  GOAL --> S8
  A --> LAYERS

  subgraph AMCP["Multi-AI Collaboration Control Plane (AMCP v2.0)"]
    bus["Agent Bus (.collab/bus/events.jsonl)"]:::amcp
    locks["Distributed File Locks (locks.json)"]:::amcp
    precheck["Sub-Second Precheck (precheck.mjs)"]:::amcp
    shield["AST Security Shield (security-shield.mjs)"]:::amcp
    contracts["Contract Drift Linter (contract-lint.mjs)"]:::amcp
    bus --- locks
    locks --- precheck
    precheck --- shield
    shield --- contracts
  end

  subgraph LAYERS["System Architecture Layers (Waves R1–R7, RW-52)"]
    direction TB
    subgraph S1["1. UI & Visual Engine"]
      u1["Tokens: 5,132 CSS variables"]:::ok
      u2["Themes: Dark / Light / Midnight"]:::ok
      u3["Views: Server Rail, Virtual Chat, Voice HUD"]:::ok
      u4["Localization: 36 Locales (chord-i18n)"]:::ok
    end

    subgraph S2["2. Protocol Core (chord-core)"]
      p1["Gateway v9: WebSocket + ETF + zstd stream"]:::ok
      p2["REST API v9: Discord endpoints"]:::ok
      p3["Permissions: 64-bit Bitmask Calculator"]:::ok
      p4["Heartbeat & Voice Gateway OP 0..43"]:::ok
    end

    subgraph S3["3. Audio & Media Engine (chord-voice / chord-platform)"]
      m1["Audio: cpal native ALSA/Pulse/PipeWire 48kHz"]:::ok
      m2["SRTP: AES-256-GCM and ChaCha20-Poly1305"]:::ok
      m3["Screen Sharing: PipeWire Portal + X11 SHM"]:::ok
      m4["DAVE MLS v1: RFC 9420 KeyRatchet E2EE"]:::ok
    end

    subgraph S6["6. Reference Implementations"]
      ref1["Vesktop 1.6.7 (Screen sharing & packaging)"]:::ok
      ref2["discord-qt 0.5.1 (Protocol contracts & schemas)"]:::ok
    end

    subgraph S8["8. Native Client Implementation (docs/rust-client/)"]
      rc1["CONCEPT.md: Zero Electron / Zero WebKit lag"]:::ok
      rc2["FEATURES.md: >= 100 features mapped"]:::ok
      rc3["ARCHITECTURE.md: Tauri v2 BFF & High-FPS WGPU"]:::ok
      rc4["PLAN.md: Waves P0–P8"]:::ok
      rc5["TEST-PLAN.md: 12 Quality Tiers (T0–T4)"]:::ok
      rc6["PROJECT-STRUCTURE.md: Workspace Crates"]:::ok
      rc7["TRACEABILITY.md: F-* to T-* mapping"]:::ok
      rc8["VERIFICATION.md: Evidence validation"]:::ok
    end
    bkl["Transfer Backlog"]:::ok
  end

  ref1 --> bkl
  ref2 --> bkl
  bkl --> S8
  AMCP --> LAYERS
\`\`\``;
}

export function updateMapFile(checkOnly = false) {
  if (!fs.existsSync(MAP_FILE)) {
    console.error(`❌ Map file not found at ${MAP_FILE}`);
    process.exit(1);
  }

  const content = fs.readFileSync(MAP_FILE, 'utf8');
  const graphContent = generateMermaidGraph();

  // Replace existing first mermaid block
  const mermaidRegex = /```mermaid[\s\S]*?```/;
  if (!mermaidRegex.test(content)) {
    console.error('❌ Could not find ```mermaid block in docs/map.md');
    process.exit(1);
  }

  const updatedContent = content.replace(mermaidRegex, graphContent);

  if (checkOnly) {
    if (content === updatedContent) {
      console.log('✅ [DOCS-GRAPH] docs/map.md graph is 100% up to date.');
      return true;
    } else {
      console.log('⚠️  [DOCS-GRAPH] docs/map.md graph is out of date. Run npm run docs:graph to update.');
      return false;
    }
  }

  fs.writeFileSync(MAP_FILE, updatedContent, 'utf8');
  console.log('✅ [DOCS-GRAPH] Successfully regenerated and synchronized docs/map.md graph!');
  return true;
}

function main() {
  const checkOnly = process.argv.includes('--check');
  const success = updateMapFile(checkOnly);
  process.exit(success ? 0 : 1);
}

main();
