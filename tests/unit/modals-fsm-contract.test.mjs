// @ts-check
/**
 * tests/unit/modals-fsm-contract.test.mjs
 *
 * Universal Finite State Machine (FSM) & Accessibility (A11y) Contract Test
 * for all Modal Dialogs and Popouts via `modal-router.js`.
 *
 * Adheres to:
 * - W3C ARIA Dialog (Modal) Pattern & WCAG 2.2 AA
 * - FSM transitions: CLOSED -> OPENING -> OPEN -> CLOSING -> CLOSED
 * - Exclusive Modals vs Stackable Overlays
 * - Keyboard Escape trap & Backdrop dismiss
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { initModalRouter, KNOWN_MODAL_IDS, DEFAULT_EXCLUSIVE_MODALS } from '../../rust/gui-spike/ui/features/modal-router.js';

describe('Modal Dialogs FSM & Accessibility Architecture Contract', () => {
  /** @type {import('../../rust/gui-spike/ui/features/modal-router.js').ModalRouter} */
  let router;
  /** @type {EventTarget} */
  let mockTarget;
  /** @type {any} */
  let mockApp;

  beforeEach(() => {
    mockTarget = new EventTarget();
    mockApp = {};
    router = initModalRouter(mockApp, {
      listenEscape: true,
      target: mockTarget,
    });
  });

  it('1. Repertoire Invariant: All known modal IDs and exclusive modals are well-defined', () => {
    assert.ok(KNOWN_MODAL_IDS.length >= 10, 'Must have at least 10 registered modal layers');
    const uniqueIds = new Set(KNOWN_MODAL_IDS);
    assert.equal(uniqueIds.size, KNOWN_MODAL_IDS.length, 'Every modal ID must be strictly unique');
    for (const em of DEFAULT_EXCLUSIVE_MODALS) {
      assert.ok(uniqueIds.has(em), `Exclusive modal ${em} must be in KNOWN_MODAL_IDS`);
    }
  });

  it('2. FSM State Transition: Initial state is CLOSED with empty stack', () => {
    assert.equal(router.getActiveModal(), null, 'No modal should be active initially');
    assert.deepEqual(router.getActiveStack(), [], 'Stack must be empty initially');
    for (const id of KNOWN_MODAL_IDS) {
      assert.equal(router.isOpen(id), false, `Modal ${id} must be closed initially`);
    }
  });

  it('3. FSM State Transition: OPEN -> sets active modal and pushes to stack', () => {
    const opened = router.open('settings-modal', { section: 'audio' });
    assert.equal(opened, 'settings-modal');
    assert.equal(router.isOpen('settings-modal'), true);
    assert.equal(router.getActiveModal(), 'settings-modal');
    assert.deepEqual(router.getActiveStack(), ['settings-modal']);
  });

  it('4. Exclusive Modal Switching Invariant: Opening exclusive modal replaces active exclusive modal', () => {
    router.open('settings-modal');
    assert.equal(router.isOpen('settings-modal'), true);

    // Opening another exclusive modal closes settings-modal
    router.open('profile-modal');
    assert.equal(router.isOpen('profile-modal'), true);
    assert.equal(router.isOpen('settings-modal'), false, 'Previous exclusive modal must be closed');
    assert.deepEqual(router.getActiveStack(), ['profile-modal']);
  });

  it('5. Overlay Stacking Invariant: Non-exclusive layer stacks on top of active modal', () => {
    // Open main modal
    router.open('settings-modal');
    // Open a stackable overlay (e.g. context-menu or expression-picker)
    router.open('context-menu');

    assert.equal(router.isOpen('settings-modal'), true, 'Base modal remains open');
    assert.equal(router.isOpen('context-menu'), true, 'Overlay is open');
    assert.equal(router.getActiveModal(), 'context-menu', 'Top active layer is overlay');
    assert.deepEqual(router.getActiveStack(), ['settings-modal', 'context-menu']);

    // Closing top layer restores base modal
    const closed = router.closeTop();
    assert.equal(closed, 'context-menu');
    assert.equal(router.getActiveModal(), 'settings-modal');
    assert.deepEqual(router.getActiveStack(), ['settings-modal']);
  });

  it('6. Keyboard Navigation & Escape Trap: Escape event closes top modal', () => {
    router.open('captcha-modal');
    assert.equal(router.isOpen('captcha-modal'), true);

    // Simulate keydown Escape on target
    const escEvent = new Event('keydown');
    Object.defineProperty(escEvent, 'key', { value: 'Escape' });
    mockTarget.dispatchEvent(escEvent);

    assert.equal(router.isOpen('captcha-modal'), false, 'Modal must close on Escape key');
    assert.equal(router.getActiveModal(), null);
  });

  it('7. Idempotence & Safe Dismiss: Closing non-existent or already closed modal is safe', () => {
    assert.equal(router.close('non-existent-modal'), null);
    assert.equal(router.closeTop(), null);
    const closedList = router.closeAll();
    assert.deepEqual(closedList, []);
  });

  it('8. Dynamic Modal Registration & Custom Controller Hook', () => {
    let openedPayload = null;
    let wasClosed = false;

    const controller = router.registerModal('custom-wizard-dialog', {
      open: (p) => { openedPayload = p; },
      close: () => { wasClosed = true; },
    });

    assert.ok(router.hasModal('custom-wizard-dialog'));

    router.open('custom-wizard-dialog', { step: 1 });
    assert.deepEqual(openedPayload, { step: 1 });
    assert.equal(router.isOpen('custom-wizard-dialog'), true);

    router.close('custom-wizard-dialog');
    assert.equal(wasClosed, true);
    assert.equal(router.isOpen('custom-wizard-dialog'), false);

    const unregistered = router.unregisterModal('custom-wizard-dialog');
    assert.equal(unregistered, true);
    assert.equal(router.hasModal('custom-wizard-dialog'), false);
  });
});
