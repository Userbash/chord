// @ts-check
/**
 * TASK-821 (W6): Unit & Integration tests for Voice Channel Stage Banner
 * (Voice Room Stage Preview) parity with official Discord (frame 032).
 *
 * RE Oracle:
 * - /home/sanya/re-deep/ru-catalog.json:
 *   - "96ANUN" / "9C444m": "Присоединиться к голосовому каналу"
 *   - "BUyJIG": "{a} сейчас в голосовом чате"
 *   - "dcyZf/": "{a} и {b} сейчас в голосовом чате"
 *   - "44/Obu": "В голосовом чате сейчас {a}, {b} и ещё {n} человек(а)"
 *   - "zSqdrS": "Сейчас в голосовом чате никого нет"
 * - waves/kanban-board.json & waves/RW-42-VIDEO-AUDIT-AND-TASKS.md (TASK-821):
 *   - Design tokens: background #2b2d31, surface #1e1f22, text_primary #ffffff,
 *     accent #eb459e, brand #5865f2, border #3f4147
 *   - Layout: 100% width x 280px banner inside chat view, padding 24px, spacing 16px,
 *     non-overlapping with #chat-list.
 */

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ShimDocument, loadApp } from "../support/gui-dom-harness.mjs";
import {
  VOICE_STAGE_PREVIEW_TOKENS,
  VOICE_STAGE_PREVIEW_TEXT,
  MAX_STAGE_PREVIEW_AVATARS,
  formatVoiceStageParticipantsSummary,
  resolveParticipantCardColor,
  renderVoiceStagePreview,
  createVoiceStagePreviewController,
} from "../../rust/gui-spike/ui/features/voice-stage-preview.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");

describe("TASK-821: Voice Channel Stage Banner Parity (frame 032)", () => {
  describe("1. Design Tokens, RE Strings & CSS Parity (TASK-821.1)", () => {
    it("exports exact Discord Dark design tokens from TASK-821 spec", () => {
      assert.equal(VOICE_STAGE_PREVIEW_TOKENS.background, "#2b2d31");
      assert.equal(VOICE_STAGE_PREVIEW_TOKENS.surface, "#1e1f22");
      assert.equal(VOICE_STAGE_PREVIEW_TOKENS.textPrimary, "#ffffff");
      assert.equal(VOICE_STAGE_PREVIEW_TOKENS.accent, "#eb459e");
      assert.equal(VOICE_STAGE_PREVIEW_TOKENS.brand, "#5865f2");
      assert.equal(VOICE_STAGE_PREVIEW_TOKENS.brandHover, "#4752c4");
      assert.equal(VOICE_STAGE_PREVIEW_TOKENS.brandActive, "#3c45a5");
      assert.equal(VOICE_STAGE_PREVIEW_TOKENS.border, "#3f4147");
      assert.equal(VOICE_STAGE_PREVIEW_TOKENS.liveBadge, "#da373c");
      assert.equal(VOICE_STAGE_PREVIEW_TOKENS.speakingRing, "#23a55a");
    });

    it("exports Russian strings matching /home/sanya/re-deep/ru-catalog.json and frame 032", () => {
      assert.equal(
        VOICE_STAGE_PREVIEW_TEXT.joinButton,
        "Присоединиться к голосовому каналу",
      );
      assert.equal(
        VOICE_STAGE_PREVIEW_TEXT.emptyParticipants,
        "Сейчас в голосовом чате никого нет",
      );
      assert.equal(VOICE_STAGE_PREVIEW_TEXT.liveBadge, "В ЭФИРЕ");
      assert.equal(VOICE_STAGE_PREVIEW_TEXT.openChatButton, "Открыть чат");
    });

    it("provides voice-stage-preview.css with 280px height, 24px padding, 16px gap, and non-overlapping layout", () => {
      const cssPath = path.join(
        REPO_ROOT,
        "rust/gui-spike/ui/components/voice-stage-preview.css",
      );
      assert.ok(fs.existsSync(cssPath), "components/voice-stage-preview.css must exist");
      const css = fs.readFileSync(cssPath, "utf8");
      assert.match(css, /#2b2d31/i);
      assert.match(css, /#1e1f22/i);
      assert.match(css, /#5865f2/i);
      assert.match(css, /#eb459e/i);
      assert.match(css, /280px/);
      assert.match(css, /24px/);
      assert.match(css, /16px/);
    });
  });

  describe("2. Participant Summary Formatting & Card Colors (TASK-821.2)", () => {
    it("formats 0, 1, 2, and 3+ participants according to ru-catalog.json (BUyJIG, dcyZf/, 44/Obu, zSqdrS)", () => {
      // 0 participants
      assert.equal(
        formatVoiceStageParticipantsSummary([]),
        "Сейчас в голосовом чате никого нет",
      );
      assert.equal(
        formatVoiceStageParticipantsSummary(null),
        "Сейчас в голосовом чате никого нет",
      );

      // 1 participant (frame 032: "Rustery сейчас в голосовом чате")
      assert.equal(
        formatVoiceStageParticipantsSummary([{ userId: "1", displayName: "Rustery" }]),
        "Rustery сейчас в голосовом чате",
      );

      // 2 participants
      assert.equal(
        formatVoiceStageParticipantsSummary([
          { userId: "1", displayName: "Rustery" },
          { userId: "2", displayName: "ShadowByte" },
        ]),
        "Rustery и ShadowByte сейчас в голосовом чате",
      );

      // 3 participants (extra = 1 -> человек)
      assert.equal(
        formatVoiceStageParticipantsSummary([
          { userId: "1", displayName: "Rustery" },
          { userId: "2", displayName: "ShadowByte" },
          { userId: "3", displayName: "Alex" },
        ]),
        "В голосовом чате сейчас Rustery, ShadowByte и ещё 1 человек",
      );

      // 4 participants (extra = 2 -> человека)
      assert.equal(
        formatVoiceStageParticipantsSummary([
          { userId: "1", displayName: "Rustery" },
          { userId: "2", displayName: "ShadowByte" },
          { userId: "3", displayName: "Alex" },
          { userId: "4", displayName: "Max" },
        ]),
        "В голосовом чате сейчас Rustery, ShadowByte и ещё 2 человека",
      );

      // 7 participants (extra = 5 -> человек)
      const seven = Array.from({ length: 7 }, (_, i) => ({
        userId: String(i + 1),
        displayName: `User${i + 1}`,
      }));
      assert.equal(
        formatVoiceStageParticipantsSummary(seven),
        "В голосовом чате сейчас User1, User2 и ещё 5 человек",
      );
    });

    it("resolves participant card background color with #eb459e default for primary participant (frame 032)", () => {
      assert.equal(
        resolveParticipantCardColor({ userId: "1", displayName: "Rustery" }, 0),
        "#eb459e",
      );
      assert.equal(
        resolveParticipantCardColor(
          { userId: "2", displayName: "Custom", bannerColor: "#23a55a" },
          1,
        ),
        "#23a55a",
      );
    });
  });

  describe("3. DOM Rendering of Rich Preview Card, Avatars, Badges & Join Button (TASK-821.2 & TASK-821.3)", () => {
    it("renders frame 032 preview card with channel title, participant avatar card, В ЭФИРЕ badge, muted badge, and Join button", () => {
      const doc = new ShimDocument();
      const chatMain = doc.createElement("div");
      chatMain.className = "chat-main";
      const chatList = doc.createElement("div");
      chatList.id = "chat-list";
      chatMain.appendChild(chatList);
      doc.body.appendChild(chatMain);

      /** @type {Array<string>} */
      const playedSounds = [];
      /** @type {Array<any>} */
      const joinedChannels = [];
      /** @type {Array<any>} */
      const watchedStreams = [];

      const banner = renderVoiceStagePreview(/** @type {any} */ (chatMain), {
        document: /** @type {any} */ (doc),
        guildId: "999001",
        guildName: "SISI 18+ PROJECT",
        channel: {
          id: "777032",
          name: "🔥 Стрим костра 🔥",
          type: 2,
        },
        participants: [
          {
            userId: "384299970632876032",
            displayName: "Rustery",
            muted: true,
            deafened: false,
            streaming: true,
            speaking: false,
          },
        ],
        isConnected: false,
        canConnect: true,
        onPlaySound: (kind) => playedSounds.push(kind),
        onJoinVoice: (ch) => joinedChannels.push(ch),
        onWatchStream: (p, ch) => watchedStreams.push({ p, ch }),
      });

      assert.ok(banner, "Banner element must be created");
      assert.equal(banner.id, "voice-stage-preview-banner");
      assert.equal(banner.getAttribute("data-voice-stage-preview"), "true");
      assert.equal(banner.getAttribute("data-channel-id"), "777032");
      assert.equal(banner.getAttribute("data-guild-id"), "999001");
      assert.equal(banner.getAttribute("data-participant-count"), "1");

      // Non-overlapping: banner must be prepended before #chat-list
      assert.equal(
        chatMain.children[0],
        banner,
        "Banner must be placed before #chat-list so it does not cover message history",
      );
      assert.equal(chatMain.children[1], chatList);

      // Channel title
      const titleEl = banner.querySelector("[data-voice-stage-title]");
      assert.ok(titleEl, "Channel title element must exist");
      assert.equal(titleEl.textContent, "🔥 Стрим костра 🔥");

      // Subtitle
      const subtitleEl = banner.querySelector("[data-voice-stage-subtitle]");
      assert.ok(subtitleEl, "Subtitle element must exist");
      assert.equal(subtitleEl.textContent, "Rustery сейчас в голосовом чате");

      // Participant tile
      const tileEl = banner.querySelector('[data-participant-id="384299970632876032"]');
      assert.ok(tileEl, "Participant avatar card for Rustery must exist");
      assert.equal(tileEl.getAttribute("data-streaming"), "true");

      // В ЭФИРЕ badge
      const liveBadge = /** @type {HTMLElement | null} */ (
        tileEl.querySelector('[data-badge="live"]')
      );
      assert.ok(liveBadge, "В ЭФИРЕ badge must be rendered for streaming participant");
      assert.equal(liveBadge.textContent, "В ЭФИРЕ");

      // Muted badge
      const mutedBadge = tileEl.querySelector('[data-badge="muted"]');
      assert.ok(mutedBadge, "Muted badge must be rendered for muted participant");

      // Click on В ЭФИРЕ badge triggers onWatchStream
      /** @type {any} */ (liveBadge).click();
      assert.equal(watchedStreams.length, 1);
      assert.equal(watchedStreams[0].p.userId, "384299970632876032");

      // Join button
      const joinBtn = /** @type {HTMLElement | null} */ (
        banner.querySelector('[data-action="join-voice-channel"]')
      );
      assert.ok(joinBtn, "Join voice channel button must exist");
      assert.equal(
        joinBtn.textContent,
        "Присоединиться к голосовому каналу",
      );

      // Click Join button -> visual feedback + sound + callback
      /** @type {any} */ (joinBtn).click();
      assert.deepEqual(playedSounds, ["join"]);
      assert.equal(joinedChannels.length, 1);
      assert.equal(joinedChannels[0].id, "777032");
      assert.equal(joinBtn.getAttribute("data-connecting"), "true");
    });

    it("renders overflow badge (+N) when more than MAX_STAGE_PREVIEW_AVATARS participants are in the channel", () => {
      const doc = new ShimDocument();
      const chatMain = doc.createElement("div");
      doc.body.appendChild(chatMain);

      const manyParticipants = Array.from(
        { length: MAX_STAGE_PREVIEW_AVATARS + 3 },
        (_, idx) => ({
          userId: `u_${idx + 1}`,
          displayName: `Участник ${idx + 1}`,
          speaking: idx === 0,
        }),
      );

      const banner = renderVoiceStagePreview(/** @type {any} */ (chatMain), {
        document: /** @type {any} */ (doc),
        guildId: "g1",
        channel: { id: "c1", name: "Общий голосовой", type: 2 },
        participants: manyParticipants,
      });

      const tiles = banner.querySelectorAll(".voice-stage-preview-tile");
      assert.equal(tiles.length, MAX_STAGE_PREVIEW_AVATARS);

      const overflowEl = banner.querySelector("[data-voice-stage-overflow]");
      assert.ok(overflowEl, "Overflow pill must be rendered when participants exceed max");
      assert.equal(overflowEl.textContent, "+3");
    });

    it("disables Join button and shows restriction notice when canConnect is false", () => {
      const doc = new ShimDocument();
      const chatMain = doc.createElement("div");
      doc.body.appendChild(chatMain);

      let joinCalled = false;
      const banner = renderVoiceStagePreview(/** @type {any} */ (chatMain), {
        document: /** @type {any} */ (doc),
        guildId: "g1",
        channel: { id: "c_locked", name: "Закрытый войс", type: 2 },
        participants: [],
        canConnect: false,
        restrictionReason: "У вашей роли нет прав для подключения к этому голосовому каналу",
        onJoinVoice: () => {
          joinCalled = true;
        },
      });

      const joinBtn = /** @type {any} */ (
        banner.querySelector('[data-action="join-voice-channel"]')
      );
      assert.ok(joinBtn);
      assert.equal(joinBtn.disabled, true);
      assert.equal(joinBtn.getAttribute("aria-disabled"), "true");
      joinBtn.click();
      assert.equal(joinCalled, false, "Disabled join button must not invoke onJoinVoice");
    });
  });

  describe("4. Controller Lifecycle & Real-Time VOICE_STATE_UPDATE Sync (TASK-821.4)", () => {
    it("updates participants in real time and toggles text chat visibility without overlapping", () => {
      const doc = new ShimDocument();
      const chatMain = doc.createElement("div");
      chatMain.className = "chat-main";
      const chatList = doc.createElement("div");
      chatList.id = "chat-list";
      chatMain.appendChild(chatList);
      doc.body.appendChild(chatMain);

      const ctrl = createVoiceStagePreviewController({
        document: /** @type {any} */ (doc),
        container: /** @type {any} */ (chatMain),
        onJoinVoice: () => {},
      });

      // Open preview for empty channel
      ctrl.open({
        guildId: "g1",
        guildName: "Test Guild",
        channel: { id: "vc1", name: "🔥 Стрим костра 🔥", type: 2 },
        participants: [],
      });

      assert.equal(ctrl.isOpen(), true);
      const banner = doc.getElementById("voice-stage-preview-banner");
      assert.ok(banner);
      assert.equal(
        banner.querySelector("[data-voice-stage-subtitle]")?.textContent,
        "Сейчас в голосовом чате никого нет",
      );

      // Real-time VOICE_STATE_UPDATE: Rustery joins and starts streaming
      ctrl.updateParticipants([
        {
          userId: "384299970632876032",
          displayName: "Rustery",
          streaming: true,
          muted: true,
        },
      ]);

      assert.equal(
        banner.querySelector("[data-voice-stage-subtitle]")?.textContent,
        "Rustery сейчас в голосовом чате",
      );
      const liveBadge = banner.querySelector('[data-badge="live"]');
      assert.ok(liveBadge);
      assert.equal(liveBadge.textContent, "В ЭФИРЕ");

      // Close preview when switching to a text channel
      ctrl.close();
      assert.equal(ctrl.isOpen(), false);
      assert.equal(doc.getElementById("voice-stage-preview-banner"), null);
    });
  });

  describe("5. Full App Integration (app.js): Click Voice Channel -> Preview Banner -> Join", () => {
    it("renders #voice-stage-preview-banner on clicking a voice channel, updates on VOICE_STATE_UPDATE, and connects on clicking 'Присоединиться к голосовому каналу'", async () => {
      const prevImage = globalThis.Image;
      const prevDisabled = /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__;
      /** @type {any} */
      let gui = null;
      let voiceConnectCalls = 0;

      const GUILD_ID = "7100";
      const SELF_ID = "9100";
      const RUSTERY_ID = "384299970632876032";
      const VOICE_CHANNEL_ID = "777032";

      const guildChannels = [
        {
          id: "c_news",
          guild_id: GUILD_ID,
          name: "новости-проекта",
          type: 0,
          kind: "text",
          position: 0,
          parentId: null,
        },
        {
          id: VOICE_CHANNEL_ID,
          guild_id: GUILD_ID,
          name: "🔥 Стрим костра 🔥",
          type: 2,
          kind: "voice",
          position: 1,
          parentId: null,
        },
      ];

      try {
        globalThis.Image = /** @type {any} */ (class FakeImage {});
        /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__ = false;

        gui = await loadApp({
          invokeHandler: (cmd, args) => {
            if (cmd === "auth_status") {
              return { status: "ready", stored: true, user: { id: SELF_ID, username: "ShadowByte" } };
            }
            if (cmd === "net_connect" || cmd === "ack_message") {
              return null;
            }
            if (cmd === "read_state") {
              return [];
            }
            if (cmd === "ready") {
              return {
                user: { id: SELF_ID, username: "ShadowByte", global_name: "ShadowByte" },
                guilds: [
                  {
                    id: GUILD_ID,
                    name: "SISI 18+ PROJECT",
                    owner_id: "1111",
                    channels: guildChannels,
                    voice_states: [
                      {
                        guild_id: GUILD_ID,
                        channel_id: VOICE_CHANNEL_ID,
                        user_id: RUSTERY_ID,
                        self_mute: true,
                        self_deaf: false,
                        self_stream: true,
                        member: {
                          user: {
                            id: RUSTERY_ID,
                            username: "rustery",
                            global_name: "Rustery",
                          },
                        },
                      },
                    ],
                  },
                ],
                relationships: [],
                private_channels: [],
                users: [
                  { id: SELF_ID, username: "ShadowByte", global_name: "ShadowByte" },
                  { id: RUSTERY_ID, username: "rustery", global_name: "Rustery" },
                ],
              };
            }
            if (cmd === "channels") {
              if (String(args?.guildId ?? "") === GUILD_ID) {
                return guildChannels;
              }
              return [];
            }
            if (cmd === "messages_page") {
              return {
                channelId: String(args?.channelId ?? "c_news"),
                items: [],
                hasMore: false,
                nextCursor: null,
                total: 0,
                limit: 100,
                loadedRowsLimit: 500,
              };
            }
            if (cmd === "voice_connect") {
              voiceConnectCalls += 1;
              return { ok: true, state: "connected", lastPingMs: 22 };
            }
            if (
              cmd === "voice_disconnect" ||
              cmd === "voice_set_audio_device" ||
              cmd === "voice_rtc_status" ||
              cmd === "voice_audio_devices" ||
              cmd === "voice_states_sync"
            ) {
              return { ok: true, state: "connected", lastPingMs: 22 };
            }
            if (cmd === "members_page") {
              return { items: [], groups: [], total: 2, onlineCount: 2 };
            }
            return undefined;
          },
        });

        const doc = gui.window.document;
        const app = gui.appModule.currentApp();
        assert.ok(app);

        await gui.settle(12);

        const voiceItem = [...doc.querySelectorAll("#channel-tree .channel-item")].find(
          (node) => node.getAttribute("data-channel-id") === VOICE_CHANNEL_ID,
        );
        assert.ok(voiceItem, "Voice channel item must be rendered in #channel-tree");

        // 1. Click on voice channel in tree -> opens Stage Preview Banner without forcing voice_connect
        voiceItem.click();
        await gui.settle(6);

        const previewBanner = doc.getElementById("voice-stage-preview-banner");
        assert.ok(
          previewBanner,
          "Clicking voice channel must render #voice-stage-preview-banner",
        );
        assert.equal(
          voiceConnectCalls,
          0,
          "First click on voice channel must open preview banner without calling voice_connect",
        );

        const titleEl = previewBanner.querySelector("[data-voice-stage-title]");
        assert.equal(titleEl?.textContent, "🔥 Стрим костра 🔥");

        const subtitleEl = previewBanner.querySelector("[data-voice-stage-subtitle]");
        assert.equal(subtitleEl?.textContent, "Rustery сейчас в голосовом чате");

        const liveBadge = previewBanner.querySelector('[data-badge="live"]');
        assert.ok(liveBadge, "В ЭФИРЕ badge must be visible on preview banner");
        assert.equal(liveBadge.textContent, "В ЭФИРЕ");

        // 2. Real-time VOICE_STATE_UPDATE adds a second participant to the preview banner
        app.dispatch.handle({
          kind: "VOICE_STATE_UPDATE",
          data: {
            guild_id: GUILD_ID,
            channel_id: VOICE_CHANNEL_ID,
            user_id: "555002",
            self_mute: false,
            self_deaf: false,
            member: {
              user: {
                id: "555002",
                username: "friend2",
                global_name: "Друг",
              },
            },
          },
        });
        await gui.settle(4);

        const updatedBanner = doc.getElementById("voice-stage-preview-banner");
        assert.ok(updatedBanner);
        assert.equal(
          updatedBanner.querySelector("[data-voice-stage-subtitle]")?.textContent,
          "Rustery и Друг сейчас в голосовом чате",
        );

        // 3. Click 'Присоединиться к голосовому каналу' button -> connects to voice channel
        const joinBtn = updatedBanner.querySelector('[data-action="join-voice-channel"]');
        assert.ok(joinBtn, "Join button must exist in #voice-stage-preview-banner");
        assert.equal(joinBtn.textContent, "Присоединиться к голосовому каналу");
        joinBtn.click();
        await gui.settle(6);

        assert.equal(
          voiceConnectCalls,
          1,
          "Clicking 'Присоединиться к голосовому каналу' must call voice_connect",
        );
      } finally {
        gui?.dispose?.();
        globalThis.Image = prevImage;
        /** @type {any} */ (globalThis).__SPIKE_AUTH_DISABLED__ = prevDisabled;
      }
    });
  });
});
