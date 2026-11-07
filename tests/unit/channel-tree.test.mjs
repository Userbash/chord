// @ts-check
/**
 * Unit tests for the guild channel category & position tree sorter (task-234 / FE-P2-26).
 * Contract oracle: Discord channel types and category tree ordering rules.
 */
import test from 'node:test';
import assert from 'node:assert/strict';

import {
  CHANNEL_TYPES,
  isVoiceLikeChannel,
  buildGuildChannelTree,
} from '../../rust/gui-spike/ui/features/channel-tree.js';

/**
 * @param {string} id
 * @param {string} name
 * @param {number} type
 * @param {number} position
 * @param {string|null} parentId
 * @param {boolean} [unread]
 * @returns {{id:string,name:string,type:number,position:number,parentId:string|null,unread?:boolean}}
 */
const ch = (id, name, type, position, parentId, unread) => ({
  id,
  name,
  type,
  position,
  parentId,
  ...(unread === undefined ? {} : { unread }),
});

test('CHANNEL_TYPES matches the Discord gateway enum and is frozen', () => {
  assert.deepEqual({ ...CHANNEL_TYPES }, {
    GUILD_TEXT: 0,
    DM: 1,
    GUILD_VOICE: 2,
    GROUP_DM: 3,
    GUILD_CATEGORY: 4,
    GUILD_ANNOUNCEMENT: 5,
    GUILD_STAGE_VOICE: 13,
    GUILD_FORUM: 15,
  });
  assert.equal(Object.isFrozen(CHANNEL_TYPES), true);
});

test('isVoiceLikeChannel is true only for voice and stage-voice channels', () => {
  assert.equal(isVoiceLikeChannel(CHANNEL_TYPES.GUILD_VOICE), true);
  assert.equal(isVoiceLikeChannel(CHANNEL_TYPES.GUILD_STAGE_VOICE), true);
  for (const type of [0, 1, 3, 4, 5, 15, 99]) {
    assert.equal(isVoiceLikeChannel(type), false, `type ${type} must not be voice-like`);
  }
});

test('buildGuildChannelTree separates categories and keeps an empty tree stable', () => {
  const tree = buildGuildChannelTree([]);
  assert.deepEqual(tree, { uncategorized: [], categories: [] });
});

test('categories are sorted by position then id and get both child lists', () => {
  const tree = buildGuildChannelTree([
    ch('cat-b', 'Beta', 4, 2, null),
    ch('cat-c', 'Charlie', 4, 1, null),
    ch('cat-a', 'Alpha', 4, 1, null),
    ch('txt-1', 'general', 0, 0, 'cat-a'),
  ]);

  assert.deepEqual(tree.categories.map((c) => c.id), ['cat-a', 'cat-c', 'cat-b']);
  assert.deepEqual(tree.categories.map((c) => c.name), ['Alpha', 'Charlie', 'Beta']);
  assert.equal(tree.categories[0]?.collapsed, false);
  assert.deepEqual(tree.categories[0]?.allChildren.map((c) => c.id), ['txt-1']);
  assert.deepEqual(tree.categories[0]?.visibleChildren.map((c) => c.id), ['txt-1']);
  assert.deepEqual(tree.categories[1]?.allChildren, []);
});

test('non-voice channels sort before voice-like ones, then by position, then by id', () => {
  const tree = buildGuildChannelTree([
    ch('voice-early', 'Lobby', 2, 0, null),
    ch('stage', 'Stage', 13, 0, null),
    ch('text-late', 'Random', 0, 9, null),
    ch('text-same-b', 'Bee', 0, 3, null),
    ch('text-same-a', 'Ay', 0, 3, null),
    ch('uncategorized-z', 'Zeta', 15, 1, null),
  ]);

  assert.deepEqual(
    tree.uncategorized.map((c) => c.id),
    ['uncategorized-z', 'text-same-a', 'text-same-b', 'text-late', 'stage', 'voice-early'],
  );
  assert.deepEqual(tree.categories, []);
});

test('collapsed category hides read channels but keeps allChildren intact', () => {
  const tree = buildGuildChannelTree(
    [
      ch('cat-1', 'Team', 4, 0, null),
      ch('unread-text', 'announcements', 0, 0, 'cat-1', true),
      ch('read-text', 'chatter', 0, 1, 'cat-1', false),
      ch('unread-voice', 'Standup', 2, 2, 'cat-1', true),
      ch('read-voice', 'Lounge', 2, 3, 'cat-1'),
      ch('other', 'general', 0, 0, null),
    ],
    ['cat-1'],
  );

  const cat = tree.categories[0];
  assert.ok(cat);
  assert.equal(cat.collapsed, true);
  assert.deepEqual(cat.allChildren.map((c) => c.id), [
    'unread-text',
    'read-text',
    'unread-voice',
    'read-voice',
  ]);
  assert.deepEqual(cat.visibleChildren.map((c) => c.id), ['unread-text', 'unread-voice']);
  // Uncategorized channels are never filtered by collapse state.
  assert.deepEqual(tree.uncategorized.map((c) => c.id), ['other']);
});

test('collapsed category with no unread channels has an empty visible list', () => {
  const tree = buildGuildChannelTree(
    [ch('cat-1', 'Quiet', 4, 0, null), ch('a', 'quiet-a', 0, 0, 'cat-1', false)],
    ['cat-1'],
  );
  assert.equal(tree.categories[0]?.collapsed, true);
  assert.equal(tree.categories[0]?.allChildren.length, 1);
  assert.deepEqual(tree.categories[0]?.visibleChildren, []);
});

test('channels pointing at an unknown parent fall back to uncategorized', () => {
  const tree = buildGuildChannelTree([
    ch('cat-1', 'Known', 4, 0, null),
    ch('orphan', 'orphan', 0, 0, 'cat-missing'),
    ch('kept', 'kept', 0, 1, 'cat-1'),
  ]);
  assert.deepEqual(tree.uncategorized.map((c) => c.id), ['orphan']);
  assert.deepEqual(tree.categories[0]?.allChildren.map((c) => c.id), ['kept']);
});

test('input array is not mutated', () => {
  const input = [
    ch('cat-1', 'Team', 4, 0, null),
    ch('voice', 'Voice', 2, 5, 'cat-1'),
    ch('text', 'Text', 0, 6, 'cat-1'),
  ];
  const snapshot = input.map((c) => c.id);
  buildGuildChannelTree(input, ['cat-1']);
  assert.deepEqual(input.map((c) => c.id), snapshot);
});
