#!/usr/bin/env node
// @ts-nocheck
import test from 'node:test';
import assert from 'node:assert/strict';
import { getEmbedding, ensureCollection, searchKnownFailure } from '../../scripts/qdrant-qa-memory.mjs';
import { checkDoomLoop, recordTestRun, resetDoomLoop } from '../../scripts/doom-loop-guard.mjs';
import { getImpactedTests } from '../../scripts/qa-auto-suite.mjs';

test('1. Qdrant & Ollama: getEmbedding produces normalized 768-dim vector', async () => {
  const vector = await getEmbedding('test failure pattern with ipc path');
  assert.equal(Array.isArray(vector), true, 'Vector must be an array');
  assert.equal(vector.length, 768, 'Vector dimension must be 768');

  // Verify vector is not all zeros
  const sumAbs = vector.reduce((acc, v) => acc + Math.abs(v), 0);
  assert.ok(sumAbs > 0.1, 'Vector must not be empty or all zeros');
});

test('2. Qdrant Collection: ensureCollection verifies chord_qa_test_memory on node2', async () => {
  const res = await ensureCollection();
  assert.equal(res.ok, true, 'Collection check must return ok=true');
});

test('3. Qdrant Semantic Search: retrieves resolution for historical failure query', async () => {
  const hits = await searchKnownFailure('Forbidden parameter name path in command', 2, 0.5);
  assert.ok(hits.length >= 1, 'Should find at least 1 match for path error');
  const top = hits[0];
  assert.match(top.solution, /endpoint/i, 'Solution should recommend renaming to endpoint');
});

test('4. Doom Loop Guard: records runs and trips after 3 consecutive failures', () => {
  resetDoomLoop();

  // Initial check: clear
  const initial = checkDoomLoop();
  assert.equal(initial.tripped, false);

  // Record 2 failures: still not tripped
  recordTestRun({ testName: 'mock_test_failing', status: 'fail' });
  recordTestRun({ testName: 'mock_test_failing', status: 'fail' });
  assert.equal(checkDoomLoop().tripped, false);

  // Record 3rd failure: trips breaker
  recordTestRun({ testName: 'mock_test_failing', status: 'fail' });
  const trippedCheck = checkDoomLoop();
  assert.equal(trippedCheck.tripped, true, 'Breaker must trip on 3rd consecutive failure');
  assert.match(trippedCheck.reason, /3 times consecutively/i);

  // Reset clears breaker
  resetDoomLoop();
  assert.equal(checkDoomLoop().tripped, false);
});

test('5. SocratiCode Impact Mapper: maps changed Rust file to targeted contract test', async () => {
  const tests = await getImpactedTests('rust/gui-spike/src/main.rs');
  assert.ok(Array.isArray(tests), 'Must return an array of tests');
  assert.ok(tests.some((t) => t.includes('ipc-path') || t.includes('ipc_wiring_contract')), 'Must include IPC test');
});
