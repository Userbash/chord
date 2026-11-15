// @ts-check
// Exercise the real CLI and JSON-RPC process in an isolated, relocated checkout.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const source = fileURLToPath(new URL('../../scripts/mcp-kanban-board.mjs', import.meta.url));
const seed = {
  id: 'SEED-01',
  title: 'Existing task stays intact',
  department: 'G1 Backend',
  priority: 'P1',
  status: 'backlog',
  summary: 'A pre-existing card from another workstream.',
  acceptance_criteria: ['Preserve this independent acceptance criterion.'],
  annotations: { owner: 'another-team', review: ['unreviewed'] },
};
const richCard = {
  id: 'LIB-01',
  title: 'Replace one infrastructure adapter',
  department: 'G5 Voice/Platform',
  priority: 'P1',
  status: 'backlog',
  summary: 'Keep the observable contract while reducing handwritten code.',
  dependencies: ['LIB-BASELINE'],
  coding_instructions: 'Write a failing behavior test before changing the adapter.',
  write_scope: ['rust/crates/chord-platform/src/example.rs'],
  dispatch_status: 'queued_not_dispatched',
  dispatch: { profile: 'g5_voice_platform', status: 'pending_runtime', run_id: null },
  future_annotation: { compatibility: ['linux', 'windows', 'macos'] },
  acceptance_criteria: ['Existing behavior remains available.'],
};

/**
 * The permission boundary prevents an unfixed absolute path from touching any
 * real repository, even when this test itself runs as root.
 * @param {import('node:test').TestContext} t
 * @param {Record<string, unknown>[]} [cards]
 */
async function checkout(t, cards = [seed]) {
  const base = await mkdtemp(path.join(tmpdir(), 'chord kanban relocated-'));
  t.after(() => rm(base, { recursive: true, force: true }));
  const root = path.join(base, 'checkout with spaces');
  const elsewhere = path.join(base, 'different working directory');
  await mkdir(path.join(root, 'scripts'), { recursive: true });
  await mkdir(path.join(root, 'waves'), { recursive: true });
  await mkdir(path.join(elsewhere, 'waves'), { recursive: true });
  const script = path.join(root, 'scripts', 'mcp-kanban-board.mjs');
  await copyFile(source, script);
  const board = {
    project: 'Relocated project marker',
    wave: 'RW-51',
    updated_at: '2026-11-15T00:00:00.000Z',
    columns: [
      { id: 'backlog', title: 'Backlog', color: '#4e5058' },
      { id: 'in_dev', title: 'In development', color: '#f0b232' },
    ],
    cards,
  };
  const boardFile = path.join(root, 'waves', 'kanban-board.json');
  const decoyFile = path.join(elsewhere, 'waves', 'kanban-board.json');
  const original = JSON.stringify(board, null, 2);
  const decoy = JSON.stringify({ ...board, project: 'Wrong checkout marker', cards: [] });
  await writeFile(boardFile, original);
  await writeFile(decoyFile, decoy);

  /** @param {string[]} args @param {string} [input] */
  function run(args, input = '') {
    const result = spawnSync(process.execPath, [
      '--permission',
      `--allow-fs-read=${base}`,
      `--allow-fs-write=${base}`,
      script,
      ...args,
    ], { cwd: elsewhere, input, encoding: 'utf8', timeout: 10_000 });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    return result.stdout;
  }

  /** @param {Record<string, unknown>[]} requests */
  function rpc(requests) {
    const output = run([], requests.map(request => JSON.stringify(request)).join('\n') + '\n');
    const responses = output.trim().split('\n').filter(Boolean).map(line => JSON.parse(line));
    assert.equal(responses.length, requests.length, 'Every RPC request must receive a response.');
    for (const response of responses) assert.equal(response.error, undefined, JSON.stringify(response.error));
    return responses;
  }

  return {
    root, boardFile, original, run, rpc,
    readBoard: async () => JSON.parse(await readFile(boardFile, 'utf8')),
    assertDecoyUnchanged: async () => assert.equal(await readFile(decoyFile, 'utf8'), decoy),
  };
}

/** @param {string} name @param {Record<string, unknown>} args @param {number} [id] */
function call(name, args, id = 1) {
  return { jsonrpc: '2.0', id, method: 'tools/call', params: { name, arguments: args } };
}

test('Kanban --export uses its relocated checkout independently of CWD', async t => {
  const fixture = await checkout(t);
  const output = fixture.run(['--export']);
  const htmlFile = path.join(fixture.root, 'waves', 'kanban-board.html');
  const html = await readFile(htmlFile, 'utf8');
  assert.ok(output.includes(htmlFile));
  assert.ok(html.includes('Relocated project marker'));
  assert.ok(html.includes(seed.title));
  assert.ok(!html.includes('Wrong checkout marker'));
  assert.ok(!/[ \t]+$/m.test(html), 'Generated HTML must not introduce trailing whitespace.');
  assert.equal(await readFile(fixture.boardFile, 'utf8'), fixture.original, 'Export must not rewrite the board.');
  await fixture.assertDecoyUnchanged();
});

test('Kanban stdio reads and filters only the relocated board', async t => {
  const fixture = await checkout(t);
  const responses = fixture.rpc([
    call('kanban_get_board', {}, 1),
    call('kanban_list_cards', { status: 'backlog', department: seed.department }, 2),
    call('kanban_list_cards', { status: 'done' }, 3),
  ]);
  const info = JSON.parse(responses[0].result.content[0].text);
  assert.equal(info.project, 'Relocated project marker');
  assert.equal(info.total_cards, 1);
  assert.equal(info.export_html_path, path.join(fixture.root, 'waves', 'kanban-board.html'));
  assert.deepEqual(JSON.parse(responses[1].result.content[0].text), [seed]);
  assert.deepEqual(JSON.parse(responses[2].result.content[0].text), []);
  assert.equal(await readFile(fixture.boardFile, 'utf8'), fixture.original);
  await fixture.assertDecoyUnchanged();
});

test('Kanban tool schema advertises planning annotations and extensible metadata', async t => {
  const fixture = await checkout(t);
  const [response] = fixture.rpc([{ jsonrpc: '2.0', id: 1, method: 'tools/list' }]);
  const tool = response.result.tools.find(item => item.name === 'kanban_create_card');
  assert.ok(tool);
  for (const key of ['dependencies', 'coding_instructions', 'write_scope', 'dispatch_status', 'dispatch']) {
    assert.ok(tool.inputSchema.properties[key], `Missing schema field: ${key}`);
  }
  assert.equal(tool.inputSchema.additionalProperties, true);
  await fixture.assertDecoyUnchanged();
});

test('Kanban create persists the complete worker handoff without changing existing cards', async t => {
  const fixture = await checkout(t);
  fixture.rpc([call('kanban_create_card', richCard)]);
  const board = await fixture.readBoard();
  assert.equal(board.cards.length, 2);
  assert.deepEqual(board.cards[0], seed);
  const created = board.cards[1];
  for (const [name, value] of Object.entries(richCard)) assert.deepEqual(created[name], value, name);
  assert.match(created.updated_at, /^\d{4}-\d{2}-\d{2}T/);
  assert.ok((await readFile(path.join(fixture.root, 'waves', 'kanban-board.html'), 'utf8')).includes(richCard.title));
  await fixture.assertDecoyUnchanged();
});

test('Kanban update preserves omitted annotations, instructions and optional design fields', async t => {
  const existing = {
    ...richCard,
    layout: { container: 'Existing layout', spacing: '12px' },
    design_tokens: { background: '#101010', text_primary: '#ffffff' },
    interactive_states: [{ element: 'button', default: 'ready', hover: 'hover', active: 'active' }],
    independent_annotation: { ticket: 'external-123', checked: false },
  };
  const fixture = await checkout(t, [existing]);
  const changed = {
    id: richCard.id, title: 'Revised scoped task', department: richCard.department,
    priority: 'P2', status: 'backlog', summary: 'Clarified task summary.',
  };
  fixture.rpc([call('kanban_create_card', changed)]);
  const board = await fixture.readBoard();
  assert.equal(board.cards.length, 1);
  const updated = board.cards[0];
  for (const [name, value] of Object.entries({ ...existing, ...changed })) {
    assert.deepEqual(updated[name], value, `Update must preserve ${name}.`);
  }
  await fixture.assertDecoyUnchanged();
});

test('Kanban update replaces explicitly supplied metadata and allows clearing arrays', async t => {
  const fixture = await checkout(t, [richCard]);
  const changed = {
    ...richCard, dependencies: [], coding_instructions: ['Use the revised parity fixture.'],
    write_scope: [], acceptance_criteria: [], dispatch_status: 'blocked',
    dispatch: { status: 'blocked', reason: 'ECC_RUNTIME_MISSING' },
    future_annotation: { reviewed: true },
  };
  fixture.rpc([call('kanban_create_card', changed)]);
  const updated = (await fixture.readBoard()).cards[0];
  for (const [name, value] of Object.entries(changed)) assert.deepEqual(updated[name], value, name);
  await fixture.assertDecoyUnchanged();
});

test('Kanban legacy minimal cards retain compatible defaults and move behavior', async t => {
  const fixture = await checkout(t, []);
  const minimal = { id: 'OLD-01', title: 'Legacy caller', department: 'G4 QA', priority: 'P2', status: 'backlog', summary: 'Minimal API payload.' };
  const responses = fixture.rpc([
    call('kanban_create_card', minimal, 1),
    call('kanban_move_card', { id: minimal.id, new_status: 'in_dev' }, 2),
  ]);
  assert.ok(responses[1].result.content[0].text.includes('in_dev'));
  const updated = (await fixture.readBoard()).cards[0];
  assert.equal(updated.status, 'in_dev');
  assert.deepEqual(updated.layout, { container: 'Flex', dimensions: 'auto', padding: '8px', spacing: '8px' });
  assert.equal(updated.design_tokens.background, '#2b2d31');
  assert.deepEqual(updated.acceptance_criteria, []);
  assert.deepEqual(updated.interactive_states, []);
  await fixture.assertDecoyUnchanged();
});

test('Kanban header reports actual task/audit metadata without invented test success', async t => {
  const fixture = await checkout(t, [seed, richCard]);
  fixture.run(['--export']);
  const file = path.join(fixture.root, 'waves', 'kanban-board.html');
  let html = await readFile(file, 'utf8');
  let header = /<header>([\s\S]*?)<\/header>/.exec(html)?.[1] ?? '';
  assert.ok(header.includes('Карточек: 2'), 'Fallback must report the actual board size.');
  assert.ok(!header.includes('61/61') && !header.includes('100%'), 'Do not assert unexecuted test success.');
  const board = await fixture.readBoard();
  board.modernization = { leaf_count: 2, completed_planning_sessions: 1 };
  await writeFile(fixture.boardFile, JSON.stringify(board));
  fixture.run(['--export']);
  html = await readFile(file, 'utf8');
  header = /<header>([\s\S]*?)<\/header>/.exec(html)?.[1] ?? '';
  assert.ok(header.includes('План: 2 задач'));
  assert.ok(header.includes('Аудитов ECC: 1'));
  assert.ok(!header.includes('61/61') && !header.includes('100%'));
  await fixture.assertDecoyUnchanged();
});
