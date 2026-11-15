// @ts-check
// Independent contracts for a lossless, dependency-safe modernization handoff.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync, lstatSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

/**
 * Structural contracts consumed by these tests; additional task annotations are
 * intentionally preserved by the board's lossless import/export contract.
 * @typedef {object} Card
 * @property {string} id
 * @property {string} epic_id
 * @property {string} summary
 * @property {string} coding_instructions
 * @property {string} rollback
 * @property {string} status
 * @property {string} dispatch_status
 * @property {string[]} dependencies
 * @property {string[]} read_scope
 * @property {string[]} write_scope
 * @property {string[]} acceptance_criteria
 * @property {string[]} exclusive_resources
 * @property {{owner: string, write_scope: string[]}[]} integration_subtasks
 * @property {{independent_oracle: string, process_checks: {argv: string[]}[]}} tests
 * @property {string} [accepted_slice_manifest]
 * @property {string} [runner_requirement]
 * @property {boolean} [release_blocker]
 * @property {boolean} [runtime_crash_observed]
 * @property {{requires_target_evidence: boolean, cross_compile_alone_proves_runtime: boolean}} [arm_admission]
 */

/** @typedef {{base_commit: string, cards: Card[], epics: {id: string}[]}} Plan */
/** @typedef {{cards: {id: string}[]}} Board */
/**
 * @typedef {object} Handoff
 * @property {string} base_commit
 * @property {{max_active_worker_slots: number, max_concurrent_cargo_in_shared_target: number}} resource_policy
 * @property {{id: string, resolved: boolean}[]} platform_blockers
 */
/** @typedef {{task_id: string, session_id?: string, actual_model?: string, heartbeat_at?: string}} Receipt */

const root = fileURLToPath(new URL('../../', import.meta.url));
/** @template T @param {string} name @returns {T} */
const load = name => JSON.parse(readFileSync(path.join(root, name), 'utf8'));
/** @type {Plan} */
const plan = load('waves/RW-51/TASKS.json');
/** @type {Board} */
const board = load('waves/kanban-board.json');
/** @type {Handoff} */
const handoff = load('waves/RW-51/HANDOFF.json');
const cards = plan.cards;
const byId = new Map(cards.map(card => [card.id, card]));
const base = '2b7cfb52024be0453e3b9333aba292b2372e610e';

/** @param {string} id @returns {Card} */
function requiredCard(id) {
  const card = byId.get(id);
  assert.ok(card, `Missing task card: ${id}`);
  return card;
}

/** @param {string} id @param {Set<string>} [seen] */
function ancestors(id, seen = new Set()) {
  if (seen.has(id)) return seen;
  seen.add(id);
  for (const dependency of requiredCard(id).dependencies) ancestors(dependency, seen);
  return seen;
}

test('The plan names the measured base and resolves every unique epic/leaf/dependency', () => {
  assert.equal(plan.base_commit, base);
  assert.equal(handoff.base_commit, base);
  assert.equal(byId.size, cards.length);
  assert.ok(cards.length >= 119, 'Keep the complete audited decomposition.');
  const epicIds = new Set(plan.epics.map(epic => epic.id));
  assert.equal(epicIds.size, plan.epics.length);
  assert.ok(epicIds.size >= 25);
  for (const card of cards) {
    assert.match(card.id, /^LIB-[A-Z][A-Z0-9]*-\d{2}$/);
    assert.ok(epicIds.has(card.epic_id), `${card.id}: unresolved epic`);
    for (const dependency of card.dependencies) assert.ok(byId.has(dependency), `${card.id}: ${dependency}`);
  }
});

test('The dependency graph is acyclic and every leaf reaches final acceptance', () => {
  const active = new Set();
  const complete = new Set();
  /** @param {string} id */
  function visit(id) {
    assert.ok(!active.has(id), `Dependency cycle at ${id}`);
    if (complete.has(id)) return;
    active.add(id);
    for (const dep of requiredCard(id).dependencies) visit(dep);
    active.delete(id);
    complete.add(id);
  }
  for (const id of byId.keys()) visit(id);
  assert.equal(complete.size, cards.length);
  assert.equal(ancestors('LIB-QA-06').size, cards.length);
});

test('All 220 historical card payloads survive unchanged and new rich cards match the manifest', () => {
  const original = JSON.parse(execFileSync('git', ['show', `${base}:waves/kanban-board.json`], { cwd: root, encoding: 'utf8' }));
  const historical = board.cards.filter(card => !byId.has(card.id));
  assert.equal(original.cards.length, 220);
  assert.deepEqual(historical, original.cards);
  const imported = board.cards.filter(card => byId.has(card.id));
  assert.deepEqual(imported, cards);
  assert.equal(board.cards.length, 220 + cards.length);
  assert.equal(new Set(board.cards.map(card => card.id)).size, board.cards.length);
});

test('Every annotation includes bounded ownership, an independent oracle and process checks', () => {
  for (const card of cards) {
    assert.ok(card.summary.length > 30, card.id);
    assert.ok(card.coding_instructions.length > 300, card.id);
    assert.ok(card.write_scope.length, card.id);
    assert.ok(card.acceptance_criteria.length >= 4, card.id);
    assert.ok(card.tests.independent_oracle.length > 25, card.id);
    assert.ok(card.tests.process_checks.length, card.id);
    for (const check of card.tests.process_checks) {
      assert.ok(Array.isArray(check.argv) && check.argv.length >= 2, card.id);
      assert.ok(check.argv.every(arg => typeof arg === 'string' && arg.length > 0), card.id);
    }
  }
});

test('Read paths exist or are produced by an explicit prerequisite', () => {
  for (const card of cards) {
    const previous = [...ancestors(card.id)].filter(id => id !== card.id).map(id => requiredCard(id));
    for (const file of card.read_scope) {
      let exists = false;
      try { lstatSync(path.join(root, file)); exists = true; } catch { /* Check an owned prerequisite output below. */ }
      if (exists) continue;
      assert.ok(previous.some(producer => producer.write_scope.some(output => file === output || file.startsWith(output + '/'))),
        `${card.id}: unowned read input ${file}`);
    }
  }
});

test('Write paths are checkout-relative, avoid historical goldens and all carry resource leases', () => {
  for (const card of cards) {
    for (const file of card.write_scope) {
      assert.ok(!path.posix.isAbsolute(file) && !file.split('/').includes('..'), `${card.id}: ${file}`);
      assert.ok(!/^(logs|functions|evidence|rust\/dist)\//.test(file), `${card.id}: golden write`);
      assert.ok(!file.startsWith('waves/') || file.startsWith('waves/RW-51/'), `${card.id}: historical wave write`);
      assert.ok(card.exclusive_resources.includes(file), `${card.id}: missing lease for ${file}`);
    }
    for (const integration of card.integration_subtasks) {
      assert.equal(integration.owner, 'RW-51 coordinator / named integrator');
      for (const file of integration.write_scope) assert.ok(card.write_scope.includes(file) && card.exclusive_resources.includes(file), `${card.id}: unowned integration`);
    }
  }
});

test('Dependency integration owns actual member manifests, not only workspace tables', () => {
  const expected = {
    'LIB-DEP-02': 'rust/crates/chord-voice/Cargo.toml',
    'LIB-DEP-03': 'rust/crates/chord-core/Cargo.toml',
    'LIB-DEP-04': 'rust/crates/chord-store/Cargo.toml',
    'LIB-DEP-06': 'rust/crates/chord-platform/Cargo.toml',
    'LIB-DEP-08': 'rust/crates/chord-json/Cargo.toml',
    'LIB-DEP-12': 'rust/crates/chord-ui/Cargo.toml',
    'LIB-DEP-15': 'rust/crates/chord-i18n/Cargo.toml',
  };
  for (const [id, manifest] of Object.entries(expected)) assert.ok(requiredCard(id).write_scope.includes(manifest), id);
});

test('RTC cutover has a real producer; IPC tests use the actual builder; AST transport is explicit', () => {
  assert.ok(ancestors('LIB-EVENT-02').has('LIB-EVENT-03'));
  assert.ok(requiredCard('LIB-EVENT-03').write_scope.includes('rust/gui-spike/src/rtc_events.rs'));
  assert.ok(requiredCard('LIB-IPC-15').write_scope.includes('rust/gui-spike/src/app_builder.rs'));
  assert.ok(ancestors('LIB-IPC-02').has('LIB-IPC-15'));
  assert.ok(requiredCard('LIB-IPC-03').write_scope.includes('rust/gui-spike/src/settings_store.rs'));
  assert.ok(ancestors('LIB-MARK-03').has('LIB-MARK-05'));
  assert.match(requiredCard('LIB-EVENT-02').coding_instructions, /intervalMs=1000|intervalMs: 1000/);
});

test('Security rollback cannot restore known RSA factors and eight-byte GCM has an explicit limit gate', () => {
  for (const card of cards.filter(card => card.epic_id === 'LIB-RSA')) assert.match(card.rollback, /never restore.*known-factor/);
  assert.match(requiredCard('LIB-CRYPTO-04').coding_instructions, /per-key authenticated-decryption invocation limits/);
  assert.ok(requiredCard('LIB-RSA-05').write_scope.includes('rust/gui-spike/src/biguint.rs'));
});

test('Native acceptance is tied to an accepted source revision and all target gates', () => {
  assert.ok(requiredCard('LIB-QA-07').write_scope.includes('waves/RW-51/ACCEPTED-SLICES.json'));
  for (const n of [2, 3, 4, 5]) {
    const card = requiredCard(`LIB-QA-0${n}`);
    assert.ok(card.dependencies.includes('LIB-QA-07'));
    assert.equal(card.accepted_slice_manifest, 'waves/RW-51/ACCEPTED-SLICES.json');
    assert.equal(typeof card.runner_requirement, 'string');
    assert.ok(card.runner_requirement && card.runner_requirement.length > 15);
    assert.ok(requiredCard('LIB-QA-06').dependencies.includes(card.id));
  }
});

test('An imported queue never masquerades as running model workers', () => {
  /** @type {{receipts: Receipt[]}} */
  const receiptSource = load('waves/RW-51/DISPATCH-RECEIPTS.json');
  const receipts = receiptSource.receipts;
  for (const card of cards) {
    if (card.dispatch_status === 'queued_not_dispatched') assert.equal(card.status, 'backlog', card.id);
    if (card.status === 'in_dev') {
      const receipt = receipts.find(receipt => receipt.task_id === card.id);
      assert.ok(receipt?.session_id && receipt.actual_model && receipt.heartbeat_at, `${card.id}: missing real execution receipt`);
    }
  }
  assert.equal(handoff.resource_policy.max_active_worker_slots, 3);
  assert.equal(handoff.resource_policy.max_concurrent_cargo_in_shared_target, 1);
});

test('The operator ARM blocker has explicit scoped work before native release acceptance', () => {
  assert.ok(plan.epics.some(epic => epic.id === 'LIB-ARM'));
  for (const number of [1, 2, 3, 4, 5, 6, 7, 8]) {
    const card = requiredCard(`LIB-ARM-0${number}`);
    assert.ok(card, `Missing ARM leaf ${number}`);
    assert.equal(card.release_blocker, true);
    assert.ok(ancestors('LIB-QA-06').has(card.id));
  }
  assert.ok(handoff.platform_blockers.some(blocker => blocker.id === 'ARM64_RELEASE_NOT_QUALIFIED' && blocker.resolved === false));
});

test('Library intake must consult the ARM target and CPU/ABI contract', () => {
  for (const card of cards.filter(card => card.epic_id === 'LIB-DEP')) {
    assert.ok(card.dependencies.includes('LIB-ARM-03'), `${card.id}: missing early ARM intake gate`);
    assert.ok(card.arm_admission, `${card.id}: missing ARM admission contract`);
    assert.equal(card.arm_admission.requires_target_evidence, true);
    assert.equal(card.arm_admission.cross_compile_alone_proves_runtime, false);
  }
  assert.ok(ancestors('LIB-ARM-03').has('LIB-ARM-02'));
});

test('ARM release evidence distinguishes genuine artifacts, CPU compatibility and native execution', () => {
  const cpu = requiredCard('LIB-ARM-02');
  assert.ok(cpu);
  assert.match(cpu.coding_instructions, /AES|aes/);
  assert.match(cpu.coding_instructions, /CRC|crc/);
  assert.match(cpu.coding_instructions, /SIGILL/);
  assert.equal(cpu.runtime_crash_observed, false, 'Source risk must not masquerade as an observed native crash.');
  for (const prerequisite of ['LIB-ARM-05', 'LIB-ARM-06', 'LIB-ARM-07']) {
    assert.ok(requiredCard('LIB-QA-05').dependencies.includes(prerequisite));
  }
  assert.ok(requiredCard('LIB-ARM-06').write_scope.includes('tests/unit/arm64-packaging-990.test.mjs'));
});
