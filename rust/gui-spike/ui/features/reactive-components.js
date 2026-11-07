// @ts-check
/**
 * @fileoverview Декларативные реактивные компоненты (`Preact + @preact/signals` архитектура)
 * поверх `reactive-store.js`, `ipc-bindings.js` и `virtualizer.js`:
 * - `<GuildRail />` (`createGuildRailComponent`)
 * - `<ChannelTree />` (`createChannelTreeComponent`)
 * - `<DmCallStage />` (`createDmCallStageComponent`)
 * - `<MessageList />` (`createMessageListComponent`)
 * - `<InviteModal />` (`createInviteModalComponent`)
 */

import { computed, effect } from "./reactive-store.js";
import {
  normalizeGuildPayload,
  filterVisibleGuildChannels,
  canSendInGuildChannel,
} from "./ipc-bindings.js";
import { createVirtualizer } from "./virtualizer.js";

/**
 * Декларативный компонент `<GuildRail />`.
 * @param {any} store
 */
export function createGuildRailComponent(store) {
  const viewModel = computed(() => {
    const selectedId = store.selectedGuildId.value;
    const items = [];
    for (const rawG of store.guildsById.value.values()) {
      const g = normalizeGuildPayload(rawG);
      items.push({
        id: g.id,
        name: g.name,
        icon: g.icon,
        selected: g.id === selectedId,
        roleCount: Array.isArray(g.roles) ? g.roles.length : 0,
        myRoleCount: Array.isArray(g.myRoles) ? g.myRoles.length : 0,
      });
    }
    return {
      selectedGuildId: selectedId,
      isHomeSelected: selectedId === "@me",
      items,
    };
  });

  return {
    viewModel,
    getSnapshot: () => viewModel.value,
    /** @param {(vm: any) => void} fn */
    subscribe: (fn) => effect(() => fn(viewModel.value)),
  };
}

/**
 * Декларативный компонент `<ChannelTree />` с фильтрацией каналов по ролям (`PERM_VIEW_CHANNEL`).
 * @param {any} store
 */
export function createChannelTreeComponent(store) {
  const viewModel = computed(() => {
    const gid = store.selectedGuildId.value;
    const selectedCid = store.selectedChannelId.value;
    const selfId = String(store.selfUser.value?.id ?? "");
    const rawGuild = store.guildsById.value.get(gid);
    const guild = rawGuild ? normalizeGuildPayload(rawGuild) : null;
    const allChannels = store.activeGuildChannels.value;
    const visibleChannels = guild
      ? filterVisibleGuildChannels(guild, allChannels, selfId)
      : allChannels;
    const activeCh =
      visibleChannels.find((/** @type {any} */ c) => String(c?.id ?? "") === selectedCid) ?? null;
    const canSend =
      guild && activeCh ? canSendInGuildChannel(guild, activeCh, allChannels, selfId) : true;

    return {
      guildId: gid,
      selectedChannelId: selectedCid,
      visibleChannels,
      canSendInSelectedChannel: canSend,
    };
  });

  return {
    viewModel,
    getSnapshot: () => viewModel.value,
    /** @param {(vm: any) => void} fn */
    subscribe: (fn) => effect(() => fn(viewModel.value)),
  };
}

/**
 * Декларативный компонент `<DmCallStage />`.
 * @param {any} store
 */
export function createDmCallStageComponent(store) {
  const viewModel = computed(() => {
    const isDm = store.isDmMode.value;
    const cid = store.selectedChannelId.value;
    const participants = store.activeDmCallParticipants.value;
    return {
      visible: isDm && Boolean(cid) && participants.length > 0,
      channelId: cid,
      participants,
    };
  });

  return {
    viewModel,
    getSnapshot: () => viewModel.value,
    /** @param {(vm: any) => void} fn */
    subscribe: (fn) => effect(() => fn(viewModel.value)),
  };
}

/**
 * Декларативный компонент `<MessageList />` с подключённым виртуализатором (`createVirtualizer`).
 * @param {any} store
 * @param {{ viewportSize?: number, itemSize?: number, overscan?: number }} [opts]
 */
export function createMessageListComponent(store, opts = {}) {
  const itemSize = Number(opts.itemSize) || 48;
  const virtualizer = createVirtualizer({
    count: 0,
    estimateSize: () => itemSize,
    overscan: Number(opts.overscan ?? 4),
    viewportSize: Number(opts.viewportSize ?? 600),
  });

  const viewModel = computed(() => {
    const messages = store.activeChannelMessages.value;
    virtualizer.setCount(messages.length);
    const virtualItems = virtualizer.getVirtualItems();
    return {
      totalMessages: messages.length,
      totalHeightPx: virtualizer.getTotalSize(),
      virtualItems: virtualItems.map((vi) => ({
        ...vi,
        message: messages[vi.index],
      })),
    };
  });

  return {
    viewModel,
    virtualizer,
    getSnapshot: () => viewModel.value,
    /** @param {(vm: any) => void} fn */
    subscribe: (fn) => effect(() => fn(viewModel.value)),
  };
}

/**
 * Декларативный компонент карточки приглашения `<InviteModal />`.
 * @param {{
 *   code: string,
 *   guildId?: string,
 *   guildName?: string,
 *   channelName?: string,
 *   onlineCount?: number,
 *   memberCount?: number,
 *   joined?: boolean,
 * }} initialProps
 */
export function createInviteModalComponent(initialProps) {
  let state = { ...initialProps };
  return {
    getSnapshot() {
      const joined = Boolean(state.joined);
      return {
        code: String(state.code || ""),
        guildId: String(state.guildId || ""),
        guildName: String(state.guildName || "Сервер Discord"),
        channelName: String(state.channelName || "general"),
        onlineCount: Number(state.onlineCount || 0),
        memberCount: Number(state.memberCount || 1),
        joined,
        ctaLabel: joined ? "Открыть сервер" : "Присоединиться",
      };
    },
    /** @param {Partial<typeof initialProps>} patch */
    update(patch) {
      state = { ...state, ...patch };
    },
  };
}
