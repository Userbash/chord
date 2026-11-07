// @ts-check
/**
 * Reactive Channel Tree & Server Preview Banner component (`@preact/signals` architecture).
 * Computes visible channels and read-only / preview badges from `evaluateChannelAccessDto`
 * (`twilight-util::permission_calculator` + `lurkerPermissionsMask`).
 */

import { evaluateChannelAccessDto, normalizeGuildDto } from "../bindings.js";

/**
 * Filters a guild's channel list and attaches computed `ChannelAccessDto` metadata per channel.
 *
 * @param {{
 *   guild: Record<string, any> | null | undefined,
 *   channels: Array<Record<string, any>>,
 *   currentUserId: string | null | undefined
 * }} params
 */
export function computeGuildChannelTreeModel({ guild, channels, currentUserId }) {
  const guildDto = normalizeGuildDto(guild);
  const byId = new Map();
  for (const ch of channels || []) {
    if (ch && ch.id) byId.set(String(ch.id), ch);
  }

  const visibleChannels = [];
  for (const ch of channels || []) {
    if (!ch || typeof ch !== "object") continue;
    const parentCategory = ch.parent_id ? byId.get(String(ch.parent_id)) : null;
    const access = evaluateChannelAccessDto({
      guild,
      channel: ch,
      parentCategory,
      currentUserId,
    });
    if (access.canViewChannel) {
      visibleChannels.push({
        channel: ch,
        access,
      });
    }
  }

  return {
    guildDto,
    isPreviewMode: Boolean(guildDto?.lurker),
    isPendingScreening: Boolean(guildDto?.pending),
    visibleChannels,
  };
}
