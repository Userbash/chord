// @ts-check
import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const gate = readFileSync(join(root, 'scripts/gate.mjs'), 'utf8');
const workflow = readFileSync(join(root, '.github/workflows/ci.yml'), 'utf8');

/** @param {string} fixtureRoot @returns {string[]} */
function fixtureE2eFiles(fixtureRoot) {
  const body = /^function fixtureE2eFiles\(\) \{[\s\S]*?^\}/m.exec(gate)?.[0];
  assert.ok(body, 'the actual gate fixtureE2eFiles function must exist');
  return new Function('root', 'join', 'existsSync', 'readdirSync', `${body}; return fixtureE2eFiles();`)(
    fixtureRoot, join, existsSync, readdirSync,
  );
}

test('fixtureE2eFiles includes BUILD006 and ordinary test files while excluding live files and directories', t => {
  const fixture = mkdtempSync(join(tmpdir(), 'fixture selection '));
  t.after(() => {
    assert.equal(dirname(fixture), resolve(tmpdir()));
    assert.ok(basename(fixture).startsWith('fixture selection '));
    rmSync(fixture, { recursive: true, force: true });
  });
  const directory = join(fixture, 'tests/e2e');
  mkdirSync(directory, { recursive: true });
  for (const name of ['m2-search-error.e2e.test.mjs', 'e2e-build006.test.mjs', 'ordinary.test.mjs',
    'live-discord.e2e.test.mjs', 'live-other.test.mjs', 'helper.mjs']) writeFileSync(join(directory, name), '');
  mkdirSync(join(directory, 'directory.test.mjs'));
  assert.deepEqual(fixtureE2eFiles(fixture), [
    'tests/e2e/e2e-build006.test.mjs', 'tests/e2e/m2-search-error.e2e.test.mjs', 'tests/e2e/ordinary.test.mjs',
  ]);
});

test('the shipped BUILD006 scenarios are selected by the real gate', () => {
  assert.ok(existsSync(join(root, 'tests/e2e/e2e-build006.test.mjs')));
  const selected = fixtureE2eFiles(root);
  assert.ok(selected.includes('tests/e2e/e2e-build006.test.mjs'));
  assert.ok(selected.every(file => !basename(file).startsWith('live-')));
});

test('CI uses the same nonlive test-file rule and validates actual JUnit cases', () => {
  const browserJob = workflow.slice(workflow.indexOf('  qa-browser:'), workflow.indexOf('  qa-live:'));
  assert.ok(browserJob.includes("-name '*.test.mjs' ! -name 'live-*'"));
  assert.ok(browserJob.includes('tests/meta/fixture-e2e-selection.test.mjs'));
  assert.ok(browserJob.includes('python3 tests/meta/fixture-junit-validator.test.py'));
  const reporter = browserJob.indexOf('--test-reporter-destination=.gate/qa-browser-junit.xml');
  const validator = browserJob.indexOf('python3 scripts/validate-fixture-junit.py .gate/qa-browser-junit.xml');
  assert.ok(reporter >= 0 && validator > reporter, 'validate the generated report after the actual fixture run');
  assert.match(browserJob, /env -u DISCORD_LAB_LIVE_CDP -u CHROME_PATH/);
  assert.match(browserJob, /--test-concurrency=1/);
  assert.match(gate, /failOnSkips: true/);
  assert.match(gate, /minPass: 1/);
  assert.match(gate, /retries: 1/);
});
