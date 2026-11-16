// @ts-check
/**
 * tests/unit/core-ui-adapters-contract.test.mjs
 *
 * Automated QA Contract Test for `src/core/ui/adapters.mjs`.
 * Validates:
 * 1. ADAPTERS registry integrity: fixture adapter presence and properties.
 * 2. resolveAdapter: returns adapter or throws helpful error on unknown app.
 * 3. clickNavItemExpression: produces valid executable JavaScript IIFE.
 * 4. Normalization: handles whitespaces and trimmed labels.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  ADAPTERS,
  resolveAdapter,
  clickNavItemExpression,
} from '../../src/core/ui/adapters.mjs';

describe('QA Contract — Core UI Adapters', () => {
  it('1. ADAPTERS: Registry contains fixture adapter with required selectors', () => {
    assert.ok(ADAPTERS.fixture);
    assert.equal(ADAPTERS.fixture.name, 'fixture');
    assert.equal(ADAPTERS.fixture.navSelector, '#nav button');
    assert.equal(ADAPTERS.fixture.screenshotPrefix, 'ui-fixture');
  });

  it('2. resolveAdapter: Resolves existing adapter', () => {
    const adapter = resolveAdapter('fixture');
    assert.equal(adapter.name, 'fixture');
  });

  it('3. resolveAdapter: Throws descriptive error for unknown app', () => {
    assert.throws(
      () => resolveAdapter('non_existent_app_42'),
      /неизвестное приложение «non_existent_app_42»/
    );
  });

  it('4. clickNavItemExpression: Generates valid IIFE string', () => {
    const expr = clickNavItemExpression('#nav button', '  Настройки  ');
    assert.ok(typeof expr === 'string');
    assert.ok(expr.startsWith('(() => {'));
    assert.ok(expr.includes('"#nav button"'));
    assert.ok(expr.includes('"Настройки"'));
  });
});
