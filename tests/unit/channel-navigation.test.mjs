// @ts-nocheck
import test from 'node:test';
import assert from 'node:assert/strict';

import {
  CHANNEL_TYPES,
  isVoiceLikeChannel,
  buildGuildChannelTree,
  buildServerGuildRail,
  resolveChannelUnreadIndicator,
  toggleCategoryCollapsed,
  buildVoiceParticipantPills,
} from '../../rust/gui-spike/ui/features/channel-tree.js';

test('buildGuildChannelTree sorts uncategorized, categories, and keeps unread children when collapsed', () => {
  const channels = [
    { id: 'v1', name: 'Voice', type: CHANNEL_TYPES.GUILD_VOICE, position: 0, parentId: 'cat1' },
    { id: 't1', name: 'general', type: CHANNEL_TYPES.GUILD_TEXT, position: 1, parentId: 'cat1', unread: true },
    { id: 'u1', name: 'rules', type: CHANNEL_TYPES.GUILD_TEXT, position: 0, parentId: null },
    { id: 'cat1', name: 'Text & Voice', type: CHANNEL_TYPES.GUILD_CATEGORY, position: 0 },
  ];

  const expanded = buildGuildChannelTree(channels, []);
  assert.equal(expanded.uncategorized.length, 1);
  assert.equal(expanded.uncategorized[0].id, 'u1');
  assert.equal(expanded.categories.length, 1);
  assert.equal(expanded.categories[0].collapsed, false);
  assert.deepEqual(
    expanded.categories[0].visibleChildren.map((c) => c.id),
    ['t1', 'v1']
  );

  const collapsed = buildGuildChannelTree(channels, ['cat1']);
  assert.equal(collapsed.categories[0].collapsed, true);
  assert.deepEqual(
    collapsed.categories[0].visibleChildren.map((c) => c.id),
    ['t1']
  );
  assert.equal(isVoiceLikeChannel(CHANNEL_TYPES.GUILD_VOICE), true);
  assert.equal(isVoiceLikeChannel(CHANNEL_TYPES.GUILD_TEXT), false);
});

test('toggleCategoryCollapsed immutably adds and removes category ids', () => {
  const initial = ['cat-1'];
  const added = toggleCategoryCollapsed(initial, 'cat-2');
  assert.deepEqual(initial, ['cat-1']);
  assert.deepEqual(added, ['cat-1', 'cat-2']);

  const removed = toggleCategoryCollapsed(added, 'cat-1');
  assert.deepEqual(removed, ['cat-2']);
});

test('buildServerGuildRail marks active guild, unread pill, and caps mention badges at 99+', () => {
  const guilds = [
    { id: 'g1', name: 'Alpha', icon: 'a.png', unread: true, mentionCount: 0 },
    { id: 'g2', name: 'Beta', icon: null, unread: false, mentionCount: 120 },
    { id: 'g3', name: 'Gamma', icon: 'g.png', unread: false, mentionCount: 0 },
  ];

  const rail = buildServerGuildRail(guilds, 'g1');
  assert.equal(rail.length, 3);

  assert.equal(rail[0].active, true);
  assert.equal(rail[0].pillState, 'active');
  assert.equal(rail[0].badgeText, '');

  assert.equal(rail[1].active, false);
  assert.equal(rail[1].unread, true);
  assert.equal(rail[1].pillState, 'unread');
  assert.equal(rail[1].badgeText, '99+');

  assert.equal(rail[2].active, false);
  assert.equal(rail[2].unread, false);
  assert.equal(rail[2].pillState, 'none');
});

test('resolveChannelUnreadIndicator computes unread pill, bold title, and mention badge', () => {
  const unreadMention = resolveChannelUnreadIndicator({ id: 'c1', unread: true, mentionCount: 5 });
  assert.deepEqual(unreadMention, {
    hasUnread: true,
    mentionCount: 5,
    badgeText: '5',
    boldTitle: true,
    pillVisible: true,
  });

  const readChannel = resolveChannelUnreadIndicator({ id: 'c2', unread: false, mentionCount: 0 });
  assert.deepEqual(readChannel, {
    hasUnread: false,
    mentionCount: 0,
    badgeText: '',
    boldTitle: false,
    pillVisible: false,
  });
});

test('buildVoiceParticipantPills formats participant pills and caps visible count with overflow', () => {
  const participants = Array.from({ length: 8 }, (_, i) => ({
    userId: `u${i + 1}`,
    displayName: `User ${i + 1}`,
    avatarUrl: null,
    muted: i % 2 === 0,
    deafened: false,
    speaking: i === 1,
  }));

  const result = buildVoiceParticipantPills(participants, 6);
  assert.equal(result.totalCount, 8);
  assert.equal(result.visiblePills.length, 6);
  assert.equal(result.overflowCount, 2);
  assert.equal(result.visiblePills[1].speaking, true);
  assert.equal(result.visiblePills[0].muted, true);
});
