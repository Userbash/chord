#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/qdrant-qa-memory.mjs — Semantic Test Failure & QA Knowledge Memory via Qdrant on node2
 *
 * Stores test failure patterns, root causes, and resolutions in Qdrant vector database
 * running in rootless Podman on node2 (user 'doom') via SSH tunnel (127.0.0.1:6333).
 * Uses Ollama 'nomic-embed-text' (768-dim) for semantic vector embeddings.
 *
 * Benefits:
 * - Instant sub-second lookup of previously solved test failures.
 * - Zero LLM token spend on diagnosing recurring errors or flaky tests.
 * - Automatically recommends concrete fixes and patches.
 *
 * Usage:
 *   node scripts/qdrant-qa-memory.mjs --status
 *   node scripts/qdrant-qa-memory.mjs --search "Forbidden parameter name path in command"
 *   node scripts/qdrant-qa-memory.mjs --seed
 */

import crypto from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

export const QDRANT_URL = process.env.QDRANT_URL || 'http://127.0.0.1:6333';
export const OLLAMA_URL = process.env.OLLAMA_URL || 'http://127.0.0.1:11434';
export const COLLECTION_NAME = 'chord_qa_test_memory';
export const VECTOR_SIZE = 768;

/**
 * Generate 768-dim embedding via local Ollama (nomic-embed-text)
 * with deterministic projection fallback if Ollama is unreachable.
 */
export async function getEmbedding(text) {
  if (!text || typeof text !== 'string') {
    text = 'unknown test error';
  }
  const cleanText = text.slice(0, 2048);

  try {
    const res = await fetch(`${OLLAMA_URL}/api/embeddings`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        model: 'nomic-embed-text',
        prompt: cleanText,
      }),
      signal: AbortSignal.timeout(4000),
    });

    if (res.ok) {
      const data = await res.json();
      if (Array.isArray(data.embedding) && data.embedding.length === VECTOR_SIZE) {
        return data.embedding;
      }
    }
  } catch {
    // Fallback to deterministic pseudo-embedding
  }

  // Fallback: deterministic hash projection to 768 floats normalized
  const vector = new Array(VECTOR_SIZE).fill(0);
  for (let i = 0; i < VECTOR_SIZE; i++) {
    const hash = crypto.createHash('sha256').update(`${cleanText}:${i}`).digest();
    vector[i] = (hash.readInt16LE(0) / 32768.0);
  }
  // Normalize vector
  const norm = Math.sqrt(vector.reduce((sum, val) => sum + val * val, 0)) || 1.0;
  return vector.map((v) => v / norm);
}

/**
 * Ensure Qdrant collection exists on node2
 */
export async function ensureCollection() {
  try {
    const checkRes = await fetch(`${QDRANT_URL}/collections/${COLLECTION_NAME}`, {
      signal: AbortSignal.timeout(3000),
    });

    if (checkRes.ok) {
      const info = await checkRes.json();
      return { ok: true, exists: true, info: info.result };
    }

    // Create collection
    const createRes = await fetch(`${QDRANT_URL}/collections/${COLLECTION_NAME}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        vectors: {
          size: VECTOR_SIZE,
          distance: 'Cosine',
        },
      }),
      signal: AbortSignal.timeout(5000),
    });

    if (createRes.ok) {
      return { ok: true, exists: false, created: true };
    }
    return { ok: false, error: await createRes.text() };
  } catch (err) {
    return { ok: false, error: err.message };
  }
}

/**
 * Index a known failure pattern into Qdrant
 */
export async function indexFailurePattern({
  id,
  testFile,
  errorPattern,
  rootCause,
  solution,
  tags = [],
}) {
  const pointId = id || crypto.randomUUID();
  const textToEmbed = `${errorPattern} ${rootCause} ${testFile} ${tags.join(' ')}`;
  const vector = await getEmbedding(textToEmbed);

  const payload = {
    testFile: testFile || 'unknown',
    errorPattern: errorPattern || '',
    rootCause: rootCause || '',
    solution: solution || '',
    tags: Array.isArray(tags) ? tags : [],
    indexedAt: new Date().toISOString(),
  };

  try {
    const res = await fetch(`${QDRANT_URL}/collections/${COLLECTION_NAME}/points?wait=true`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        points: [{
          id: pointId,
          vector,
          payload,
        }],
      }),
      signal: AbortSignal.timeout(5000),
    });

    if (res.ok) {
      return { ok: true, id: pointId };
    }
    return { ok: false, error: await res.text() };
  } catch (err) {
    return { ok: false, error: err.message };
  }
}

/**
 * Search Qdrant for known failure resolutions
 */
export async function searchKnownFailure(queryText, limit = 3, scoreThreshold = 0.65) {
  try {
    const vector = await getEmbedding(queryText);
    const res = await fetch(`${QDRANT_URL}/collections/${COLLECTION_NAME}/points/search`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        vector,
        limit,
        with_payload: true,
        score_threshold: scoreThreshold,
      }),
      signal: AbortSignal.timeout(4000),
    });

    if (res.ok) {
      const data = await res.json();
      return (data.result || []).map((hit) => ({
        score: hit.score,
        testFile: hit.payload?.testFile,
        errorPattern: hit.payload?.errorPattern,
        rootCause: hit.payload?.rootCause,
        solution: hit.payload?.solution,
        tags: hit.payload?.tags || [],
      }));
    }
    return [];
  } catch {
    return [];
  }
}

/**
 * Pre-seed historical failure patterns cataloged from discord-lab engineering
 */
export async function seedHistoricalPatterns() {
  await ensureCollection();

  const historicalData = [
    {
      id: '00000000-0000-0000-0000-000000000001',
      testFile: 'tests/meta/ipc-path.test.mjs',
      errorPattern: 'Forbidden parameter name path in command',
      rootCause: 'Meta test strictly prohibits any Tauri IPC command parameter named path to prevent path traversal.',
      solution: 'Rename argument path: String to endpoint: String or target_path: String in main.rs and update frontend bridge.',
      tags: ['ipc', 'meta', 'security', 'rust'],
    },
    {
      id: '00000000-0000-0000-0000-000000000002',
      testFile: 'tests/meta/plan-sync.test.mjs',
      errorPattern: 'Missing row in TDD-PLAN.md §8 table for test file',
      rootCause: 'Newly added test files were not registered in the §8 tracking table of TDD-PLAN.md.',
      solution: 'Add a markdown table row for the new test file in TDD-PLAN.md §8 specifying Phase, Card, Test Path, and Status.',
      tags: ['meta', 'tdd-plan', 'sync'],
    },
    {
      id: '00000000-0000-0000-0000-000000000003',
      testFile: 'rust/crates/chord-core/src/resume.rs',
      errorPattern: 'cannot call method default on type Ready',
      rootCause: 'Ready struct does not implement Default trait.',
      solution: 'Use state.clone() before apply() rather than std::mem::replace(state, Ready::default()).',
      tags: ['rust', 'compiler', 'resume', 'traits'],
    },
    {
      id: '00000000-0000-0000-0000-000000000004',
      testFile: 'rust/gui-spike/src/voice_plugin.rs',
      errorPattern: 'let-chains syntax is experimental in Rust 2021',
      rootCause: 'if let Some(...) && let Ok(...) is not supported in 2021 edition without unstable feature flags.',
      solution: 'Split chained let into nested if let statements: if let Some(x) = ... { if let Ok(y) = ... { ... } }.',
      tags: ['rust', 'syntax', 'let-chains', 'rust2021'],
    },
    {
      id: '00000000-0000-0000-0000-000000000005',
      testFile: 'rust/gui-spike/src/voice_plugin.rs',
      errorPattern: 'use of moved value cur_cid',
      rootCause: 'Option<String> cur_cid was moved into closure or function call before checking has_live_call.',
      solution: 'Evaluate let has_live_call = cur_cid.is_some(); before moving or taking ownership of cur_cid.',
      tags: ['rust', 'borrow-checker', 'ownership', 'move'],
    },
    {
      id: '00000000-0000-0000-0000-000000000006',
      testFile: 'rust/gui-spike/tests/ipc_wiring_contract.rs',
      errorPattern: 'command count mismatch in ALL_UI_IPC_COMMANDS',
      rootCause: 'Newly added Tauri commands in main.rs were not added to ALL_89_UI_IPC_COMMANDS in contract test.',
      solution: 'Update ALL_89_UI_IPC_COMMANDS array in ipc_wiring_contract.rs with all registered commands sorted alphabetically.',
      tags: ['rust', 'ipc', 'contract', 'parity'],
    },
    {
      id: '00000000-0000-0000-0000-000000000007',
      testFile: 'git commit push',
      errorPattern: 'Push rejected by secret scanner Discord Bot Token',
      rootCause: 'Mock test string matched Discord Bot Token regex (3-segment base64 token).',
      solution: 'Replace mock token with generic non-matching string: mock_dummy_secret_auth_token_for_redaction_test.',
      tags: ['security', 'git', 'push-protection', 'secrets'],
    },
    {
      id: '00000000-0000-0000-0000-000000000008',
      testFile: 'tests/unit/pipewire-portal-971.test.mjs',
      errorPattern: 'Cargo.toml workspace dependencies tokio missing in temporary harness',
      rootCause: 'Temporary crate created in /tmp references chord-platform which requires tokio from workspace.',
      solution: 'Inject [workspace.dependencies] tokio = { version = "1.53", features = [...] } into temporary Cargo.toml harness.',
      tags: ['cargo', 'temp-harness', 'dependencies'],
    },
  ];

  let seeded = 0;
  for (const item of historicalData) {
    const res = await indexFailurePattern(item);
    if (res.ok) seeded++;
  }
  return { seeded, total: historicalData.length };
}

// CLI Execution
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);

  if (args.includes('--status')) {
    console.log('🩺 [QDRANT-QA-MEMORY] Checking Qdrant on node2 (user: doom)...');
    const res = await ensureCollection();
    if (res.ok) {
      console.log(`✅ Qdrant Connected! Collection '${COLLECTION_NAME}' is ready.`);
      if (res.info) {
        console.log(`   Points Count: ${res.info.points_count || 0}`);
        console.log(`   Vectors Count: ${res.info.vectors_count || 0}`);
        console.log(`   Status: ${res.info.status}`);
      }
    } else {
      console.error(`❌ Qdrant Error: ${res.error}`);
    }
  } else if (args.includes('--seed')) {
    console.log('🌱 [QDRANT-QA-MEMORY] Seeding historical failure resolutions into Qdrant...');
    const result = await seedHistoricalPatterns();
    console.log(`✅ Successfully seeded ${result.seeded}/${result.total} knowledge points into Qdrant!`);
  } else if (args.includes('--search')) {
    const queryIdx = args.indexOf('--search') + 1;
    const query = args[queryIdx] || 'ipc error';
    console.log(`🔍 [QDRANT-QA-MEMORY] Searching semantic memory for: "${query}"...`);
    const hits = await searchKnownFailure(query);
    if (hits.length === 0) {
      console.log('   ℹ️ No direct match found in Qdrant memory.');
    } else {
      hits.forEach((hit, i) => {
        console.log(`\n[Match #${i + 1}] (Cosine Similarity: ${(hit.score * 100).toFixed(1)}%)`);
        console.log(`   📁 Target File: ${hit.testFile}`);
        console.log(`   ⚠️ Pattern:     ${hit.errorPattern}`);
        console.log(`   💡 Root Cause:  ${hit.rootCause}`);
        console.log(`   🛠️ Solution:    ${hit.solution}`);
      });
    }
  } else {
    console.log(`Usage:`);
    console.log(`  node scripts/qdrant-qa-memory.mjs --status`);
    console.log(`  node scripts/qdrant-qa-memory.mjs --seed`);
    console.log(`  node scripts/qdrant-qa-memory.mjs --search "<error text>"`);
  }
}
