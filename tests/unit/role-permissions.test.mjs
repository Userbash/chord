// @ts-check
/**
 * Contract tests for task-219 (FE-P2-22): guild role color + permission bitfield helper.
 * Oracle: Discord permission bit positions and role color encoding (0xRRGGBB, 0 = no color).
 */
import assert from 'node:assert/strict';
import test from 'node:test';

import {
  PERMISSION_FLAGS,
  hasPermissionBit,
  intColorToHex,
  resolveHighestColoredRole,
} from '../../rust/gui-spike/ui/features/role-permissions.js';

test('PERMISSION_FLAGS exposes frozen Discord bit positions', () => {
  assert.equal(Object.isFrozen(PERMISSION_FLAGS), true);
  assert.equal(PERMISSION_FLAGS.ADMINISTRATOR, 1n << 3n);
  assert.equal(PERMISSION_FLAGS.MANAGE_CHANNELS, 1n << 4n);
  assert.equal(PERMISSION_FLAGS.MANAGE_GUILD, 1n << 5n);
  assert.equal(PERMISSION_FLAGS.VIEW_CHANNEL, 1n << 10n);
  assert.equal(PERMISSION_FLAGS.SEND_MESSAGES, 1n << 11n);
  assert.equal(PERMISSION_FLAGS.MANAGE_MESSAGES, 1n << 13n);
  assert.equal(PERMISSION_FLAGS.ATTACH_FILES, 1n << 15n);
  assert.equal(PERMISSION_FLAGS.CONNECT, 1n << 20n);
  assert.equal(PERMISSION_FLAGS.SPEAK, 1n << 21n);
  assert.equal(PERMISSION_FLAGS.MANAGE_THREADS, 1n << 34n);
  assert.throws(() => {
    // @ts-ignore -- frozen object must reject writes
    PERMISSION_FLAGS.ADMINISTRATOR = 0n;
  }, TypeError);
});

test('hasPermissionBit accepts decimal strings', () => {
  // 1024 = VIEW_CHANNEL, 2048 = SEND_MESSAGES
  assert.equal(hasPermissionBit('3072', PERMISSION_FLAGS.VIEW_CHANNEL), true);
  assert.equal(hasPermissionBit('3072', PERMISSION_FLAGS.SEND_MESSAGES), true);
  assert.equal(hasPermissionBit('1024', PERMISSION_FLAGS.SEND_MESSAGES), false);
});

test('hasPermissionBit accepts BigInt input', () => {
  assert.equal(hasPermissionBit(1n << 10n, PERMISSION_FLAGS.VIEW_CHANNEL), true);
  assert.equal(hasPermissionBit(0n, PERMISSION_FLAGS.VIEW_CHANNEL), false);
  assert.equal(hasPermissionBit((1n << 34n) | (1n << 10n), PERMISSION_FLAGS.MANAGE_THREADS), true);
});

test('hasPermissionBit grants everything for ADMINISTRATOR', () => {
  assert.equal(hasPermissionBit('8', PERMISSION_FLAGS.MANAGE_GUILD), true);
  assert.equal(hasPermissionBit(1n << 3n, PERMISSION_FLAGS.MANAGE_THREADS), true);
  assert.equal(hasPermissionBit('8', 1n << 62n), true);
});

test('hasPermissionBit returns false on invalid bitfield', () => {
  assert.equal(hasPermissionBit('abc', PERMISSION_FLAGS.VIEW_CHANNEL), false);
  assert.equal(hasPermissionBit('', PERMISSION_FLAGS.VIEW_CHANNEL), false);
  assert.equal(hasPermissionBit('12.5', PERMISSION_FLAGS.VIEW_CHANNEL), false);
  assert.equal(hasPermissionBit('0x400', PERMISSION_FLAGS.VIEW_CHANNEL), false);
  assert.equal(hasPermissionBit(1.5, PERMISSION_FLAGS.VIEW_CHANNEL), false);
  assert.equal(hasPermissionBit(null, PERMISSION_FLAGS.VIEW_CHANNEL), false);
  assert.equal(hasPermissionBit(undefined, PERMISSION_FLAGS.VIEW_CHANNEL), false);
  assert.equal(hasPermissionBit({}, PERMISSION_FLAGS.VIEW_CHANNEL), false);
});

test('intColorToHex formats valid colors as 6-digit hex', () => {
  assert.equal(intColorToHex(0x1abc9c), '#1abc9c');
  assert.equal(intColorToHex(0xff0000), '#ff0000');
  assert.equal(intColorToHex(0x00000f), '#00000f');
  assert.equal(intColorToHex(0xffffff), '#ffffff');
});

test('intColorToHex falls back to default blurple-grey for absent/invalid colors', () => {
  assert.equal(intColorToHex(0), '#99aab5');
  assert.equal(intColorToHex(-1), '#99aab5');
  assert.equal(intColorToHex(0x1000000), '#99aab5');
  assert.equal(intColorToHex(1.5), '#99aab5');
  assert.equal(intColorToHex(Number.NaN), '#99aab5');
  assert.equal(intColorToHex(null), '#99aab5');
  assert.equal(intColorToHex(undefined), '#99aab5');
  assert.equal(intColorToHex('16711680'), '#99aab5');
});

test('resolveHighestColoredRole picks the highest positioned role that has a color', () => {
  const roles = [
    { id: '1', name: 'Lowest', position: 1, color: 0x1abc9c },
    { id: '3', name: 'Highest colored', position: 3, color: 0xff0000 },
    { id: '2', name: 'Middle colorless', position: 2, color: 0 },
  ];
  const picked = resolveHighestColoredRole(roles);
  assert.ok(picked);
  assert.equal(picked.id, '3');
  assert.equal(picked.name, 'Highest colored');
  // input must not be reordered
  assert.deepEqual(roles.map((role) => role.id), ['1', '3', '2']);
});

test('resolveHighestColoredRole skips top roles without color', () => {
  const roles = [
    { id: 'a', name: 'Owner-ish', position: 9, color: 0 },
    { id: 'b', name: 'Manager', position: 5, color: 0x0000ff },
    { id: 'c', name: 'Member', position: 2, color: 0x00ff00 },
  ];
  const picked = resolveHighestColoredRole(roles);
  assert.ok(picked);
  assert.equal(picked.id, 'b');
});

test('resolveHighestColoredRole returns null when no role carries color', () => {
  assert.equal(resolveHighestColoredRole([]), null);
  assert.equal(
    resolveHighestColoredRole([
      { id: '1', name: 'A', position: 2, color: 0 },
      { id: '2', name: 'B', position: 1, color: 0 },
    ]),
    null,
  );
  assert.equal(resolveHighestColoredRole(null), null);
});
