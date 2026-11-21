// @ts-check
/**
 * tests/unit/typed-pins-commands-contract.test.mjs
 *
 * Strict TDD Contract Test: Typed Pins & Messages Operations in `bindings.js`.
 *
 * Requirements:
 * 1. `createTypedCommands` must export `pinMessage`, `unpinMessage`, and `listPins`.
 * 2. Boundary Value Analysis (BVA): rejects empty/null/non-string snowflakes with TypeError.
 * 3. Exact IPC Invocation: passes { channel_id, message_id } to Rust BFF handlers.
 * 4. Error Propagation: passes backend error responses without swallowing.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createTypedCommands } from '../../rust/gui-spike/ui/bindings.js';

describe('Strict TDD — Typed Pins Commands Contract', () => {
  it('1. Method Presence: createTypedCommands exports pinMessage, unpinMessage, listPins', () => {
    const cmds = createTypedCommands(async () => {});
    assert.equal(typeof cmds.pinMessage, 'function', 'pinMessage must be a function');
    assert.equal(typeof cmds.unpinMessage, 'function', 'unpinMessage must be a function');
    assert.equal(typeof cmds.listPins, 'function', 'listPins must be a function');
  });

  it('2. BVA Input Validation: Rejects invalid channelId or messageId', async () => {
    const cmds = createTypedCommands(async () => {});

    // Empty or non-string channelId
    await assert.rejects(
      async () => cmds.pinMessage('', '12345'),
      /channelId must be a non-empty string/
    );
    await assert.rejects(
      async () => cmds.pinMessage(/** @type {any} */ (null), '12345'),
      /channelId must be a non-empty string/
    );

    // Empty or non-string messageId
    await assert.rejects(
      async () => cmds.pinMessage('12345', ''),
      /messageId must be a non-empty string/
    );
    await assert.rejects(
      async () => cmds.unpinMessage('12345', /** @type {any} */ (null)),
      /messageId must be a non-empty string/
    );

    // Empty channelId in listPins
    await assert.rejects(
      async () => cmds.listPins(''),
      /channelId must be a non-empty string/
    );
  });

  it('3. Exact IPC Dispatch: pinMessage dispatches pin_channel_message with payload', async () => {
    /** @type {{ cmd: string, args: Record<string, unknown> | undefined } | null} */
    let recorded = null;
    /** @param {string} cmd @param {Record<string, unknown>} [args] */
    const mockInvoke = async (cmd, args) => {
      recorded = { cmd, args };
      return { ok: true };
    };

    const cmds = createTypedCommands(mockInvoke);
    await cmds.pinMessage('987654321012345678', '112233445566778899');

    assert.ok(recorded);
    assert.equal(/** @type {{ cmd: string, args: Record<string, unknown> }} */ (recorded).cmd, 'pin_channel_message');
    assert.deepEqual(/** @type {{ cmd: string, args: Record<string, unknown> }} */ (recorded).args, {
      channel_id: '987654321012345678',
      message_id: '112233445566778899',
    });
  });

  it('4. Exact IPC Dispatch: unpinMessage dispatches unpin_channel_message', async () => {
    /** @type {{ cmd: string, args: Record<string, unknown> | undefined } | null} */
    let recorded = null;
    /** @param {string} cmd @param {Record<string, unknown>} [args] */
    const mockInvoke = async (cmd, args) => {
      recorded = { cmd, args };
      return { ok: true };
    };

    const cmds = createTypedCommands(mockInvoke);
    await cmds.unpinMessage('987654321012345678', '112233445566778899');

    assert.ok(recorded);
    assert.equal(/** @type {{ cmd: string, args: Record<string, unknown> }} */ (recorded).cmd, 'unpin_channel_message');
    assert.deepEqual(/** @type {{ cmd: string, args: Record<string, unknown> }} */ (recorded).args, {
      channel_id: '987654321012345678',
      message_id: '112233445566778899',
    });
  });

  it('5. Exact IPC Dispatch: listPins dispatches list_channel_pins', async () => {
    /** @type {{ cmd: string, args: Record<string, unknown> | undefined } | null} */
    let recorded = null;
    /** @param {string} cmd @param {Record<string, unknown>} [args] */
    const mockInvoke = async (cmd, args) => {
      recorded = { cmd, args };
      return [{ id: 'msg-1', channel_id: 'ch-1' }];
    };

    const cmds = createTypedCommands(mockInvoke);
    const pins = await cmds.listPins('987654321012345678');

    assert.ok(recorded);
    assert.equal(/** @type {{ cmd: string, args: Record<string, unknown> }} */ (recorded).cmd, 'list_channel_pins');
    assert.deepEqual(/** @type {{ cmd: string, args: Record<string, unknown> }} */ (recorded).args, {
      channel_id: '987654321012345678',
    });
    assert.equal(pins.length, 1);
  });

  it('6. Error Propagation: Propagates backend errors to caller', async () => {
    const mockInvoke = async () => {
      throw new Error('403 Forbidden: Missing MANAGE_MESSAGES permission');
    };

    const cmds = createTypedCommands(mockInvoke);
    await assert.rejects(
      async () => cmds.pinMessage('ch-1', 'msg-1'),
      /Missing MANAGE_MESSAGES permission/
    );
  });
});
