// @ts-check
/**
 * tests/unit/core-ui-engine-contract.test.mjs
 *
 * Automated QA Contract Test for `src/core/ui/engine.mjs`.
 * Validates:
 * 1. Module export integrity: runWalk is an async function.
 * 2. Exit Code 2 on Missing Navigation: Returns code 2 if 0 nav items found.
 * 3. Exit Code 2 on CDP Error: Gracefully catches CDP exceptions without unhandled rejection.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { runWalk } from '../../src/core/ui/engine.mjs';

describe('QA Contract — Core UI Walk Engine', () => {
  it('1. Export Integrity: runWalk is an exported async function', () => {
    assert.equal(typeof runWalk, 'function');
  });

  it('2. Missing Nav Contract: Returns code 2 when 0 nav items are discovered', async () => {
    /** @type {any} */
    const mockCdp = {
      events: [],
      send: async (/** @type {string} */ cmd) => {
        if (cmd === 'Runtime.evaluate') {
          return { result: { value: [] } };
        }
        return {};
      },
    };
    /** @type {any} */
    const mockAdapter = {
      name: 'test-fixture',
      navSelector: '#nav button',
      screenshotPrefix: 'test-prefix',
    };

    let warned = '';
    const res = await runWalk({
      cdp: mockCdp,
      adapter: mockAdapter,
      outPath: '/tmp/test-walk-zero.jsonl',
      screensDir: '/tmp',
      maxSteps: 10,
      settleMs: 0,
      profile: false,
      sleep: async () => {},
      log: () => {},
      warn: (msg) => { warned += msg; },
    });

    assert.equal(res.code, 2, 'Must exit with code 2 on empty navigation');
    assert.ok(warned.includes('0 пунктов навигации'));
  });

  it('3. CDP Error Handling: Catches broken CDP session and returns code 2', async () => {
    /** @type {any} */
    const brokenCdp = {
      events: [],
      send: async () => {
        throw new Error('CDP connection reset by peer');
      },
    };
    /** @type {any} */
    const mockAdapter = {
      name: 'test-fixture',
      navSelector: 'button',
      screenshotPrefix: 'test-prefix',
    };

    const res = await runWalk({
      cdp: brokenCdp,
      adapter: mockAdapter,
      outPath: '/tmp/test-walk-broken.jsonl',
      screensDir: '/tmp',
      profile: false,
      warn: () => {},
    });

    assert.equal(res.code, 2, 'Must fail safely with code 2 on CDP failure');
  });
});
