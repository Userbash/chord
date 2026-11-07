// @ts-check
/**
 * Guild channel category & position tree sorter (task-234 / FE-P2-26).
 *
 * Discord renders guild channels as: uncategorized channels first, then each
 * category (sorted by position) with its children. Inside a bucket the client
 * puts text-like channels above voice/stage-voice ones, and within the same
 * kind orders by position and finally by id.
 */

/**
 * @typedef {Object} GuildChannel
 * @property {string} id
 * @property {string} name
 * @property {number} type Discord channel type
 * @property {number} position Sort position inside its parent
 * @property {string|null} [parentId] Category id, or null/undefined when uncategorized
 * @property {boolean} [unread] Whether the channel currently has unread messages
 */

/**
 * @typedef {Object} ChannelCategoryNode
 * @property {string} id
 * @property {string} name
 * @property {boolean} collapsed
 * @property {GuildChannel[]} allChildren
 * @property {GuildChannel[]} visibleChildren
 */

/**
 * @typedef {Object} GuildChannelTree
 * @property {GuildChannel[]} uncategorized
 * @property {ChannelCategoryNode[]} categories
 */

/** Discord channel type enum, frozen so callers cannot mutate the contract. */
export const CHANNEL_TYPES = Object.freeze({
  GUILD_TEXT: 0,
  DM: 1,
  GUILD_VOICE: 2,
  GROUP_DM: 3,
  GUILD_CATEGORY: 4,
  GUILD_ANNOUNCEMENT: 5,
  GUILD_STAGE_VOICE: 13,
  GUILD_FORUM: 15,
});

const GUILD_CATEGORY = CHANNEL_TYPES.GUILD_CATEGORY;

/**
 * Voice-like channels render below text-like ones in a category.
 * @param {number} type
 * @returns {boolean}
 */
export function isVoiceLikeChannel(type) {
  return type === CHANNEL_TYPES.GUILD_VOICE || type === CHANNEL_TYPES.GUILD_STAGE_VOICE;
}

/**
 * Sort a bucket of sibling channels: non-voice first, then position, then id.
 * @param {GuildChannel} a
 * @param {GuildChannel} b
 * @returns {number}
 */
function compareChannels(a, b) {
  const voiceA = isVoiceLikeChannel(a.type) ? 1 : 0;
  const voiceB = isVoiceLikeChannel(b.type) ? 1 : 0;
  if (voiceA !== voiceB) return voiceA - voiceB;

  const posA = Number.isFinite(a.position) ? a.position : 0;
  const posB = Number.isFinite(b.position) ? b.position : 0;
  if (posA !== posB) return posA - posB;

  return String(a.id).localeCompare(String(b.id));
}

/**
 * Compare two categories: position ascending, then id.
 * @param {GuildChannel} a
 * @param {GuildChannel} b
 * @returns {number}
 */
function compareCategories(a, b) {
  const posA = Number.isFinite(a.position) ? a.position : 0;
  const posB = Number.isFinite(b.position) ? b.position : 0;
  if (posA !== posB) return posA - posB;
  return String(a.id).localeCompare(String(b.id));
}

/**
 * Build the renderable guild channel tree.
 *
 * @param {GuildChannel[]} channels Flat channel list from the gateway
 * @param {string[]} [collapsedCategoryIds] Categories the user has collapsed
 * @returns {GuildChannelTree}
 */
export function buildGuildChannelTree(channels, collapsedCategoryIds = []) {
  const list = Array.isArray(channels) ? channels : [];
  const collapsed = new Set(Array.isArray(collapsedCategoryIds) ? collapsedCategoryIds : []);

  const categoryChannels = list.filter((c) => c.type === GUILD_CATEGORY).slice().sort(compareCategories);
  const knownCategoryIds = new Set(categoryChannels.map((c) => c.id));

  /** @type {Map<string|null, GuildChannel[]>} */
  const buckets = new Map();
  buckets.set(null, []);

  for (const channel of list) {
    if (channel.type === GUILD_CATEGORY) continue;
    const parentId = channel.parentId ?? null;
    // Channels whose category is missing from the payload fall back to the
    // uncategorized bucket instead of disappearing from the tree.
    const bucketKey = parentId !== null && knownCategoryIds.has(parentId) ? parentId : null;
    const bucket = buckets.get(bucketKey);
    if (bucket) bucket.push(channel);
    else buckets.set(bucketKey, [channel]);
  }

  /** @type {GuildChannel[]} */
  const uncategorized = (buckets.get(null) ?? []).slice().sort(compareChannels);

  const categories = categoryChannels.map((category) => {
    const allChildren = (buckets.get(category.id) ?? []).slice().sort(compareChannels);
    const isCollapsed = collapsed.has(category.id);
    /** @type {GuildChannel[]} */
    const visibleChildren = isCollapsed
      ? allChildren.filter((child) => child.unread === true)
      : allChildren.slice();

    return {
      id: category.id,
      name: category.name,
      collapsed: isCollapsed,
      allChildren,
      visibleChildren,
    };
  });

  return { uncategorized, categories };
}

export default buildGuildChannelTree;
