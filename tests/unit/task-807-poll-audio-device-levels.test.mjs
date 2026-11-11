// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");

describe("TASK-807: Устранение спама polling аудиоустройств (voice_set_audio_device)", () => {
  const appJs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/ui/app.js"), "utf8");

  it("T807-1: pollAudioDeviceLevels и функции управления таймерами объявлены в app.js", () => {
    assert.match(
      appJs,
      /(?:function\s+pollAudioDeviceLevels|const\s+pollAudioDeviceLevels\s*=)/,
      "app.js must declare pollAudioDeviceLevels",
    );
    assert.match(
      appJs,
      /(?:function\s+startAudioDeviceLevelsPolling|const\s+startAudioDeviceLevelsPolling\s*=)/,
      "app.js must declare startAudioDeviceLevelsPolling",
    );
    assert.match(
      appJs,
      /(?:function\s+stopAudioDeviceLevelsPolling|const\s+stopAudioDeviceLevelsPolling\s*=)/,
      "app.js must declare stopAudioDeviceLevelsPolling",
    );
  });

  it("T807-2: pollAudioDeviceLevels увеличивает интервал в фоне до 10 секунд при стабильном звонке", () => {
    const fnIdx = appJs.indexOf("function pollAudioDeviceLevels(");
    assert.ok(fnIdx > 0, "pollAudioDeviceLevels must exist in app.js");
    const fnBody = appJs.slice(fnIdx, fnIdx + 3000);

    assert.match(
      fnBody,
      /10000|10_000/,
      "pollAudioDeviceLevels must enforce 10-second background throttling during stable voice calls",
    );
    assert.match(
      fnBody,
      /isStableCall|isVoiceSettingsOpen/,
      "pollAudioDeviceLevels must distinguish between open settings and stable background call",
    );
  });

  it("T807-3: опрос уровней устройств активируется при открытых настройках звука (createSettingsConsole)", () => {
    const fnIdx = appJs.indexOf("function pollAudioDeviceLevels(");
    const fnBody = appJs.slice(fnIdx, fnIdx + 3000);

    assert.match(
      fnBody,
      /voice-devices-settings|settings-voice-input-select|data-active-pane/,
      "pollAudioDeviceLevels must inspect voice settings console visibility",
    );
    assert.match(
      fnBody,
      /VOICE_AUDIO_DEVICES_IPC_CMD/,
      "pollAudioDeviceLevels must query audio devices when settings are open",
    );

    const startIdx = appJs.indexOf("function startAudioDeviceLevelsPolling(");
    assert.ok(startIdx > 0, "startAudioDeviceLevelsPolling must exist in app.js");
    const startBody = appJs.slice(startIdx, startIdx + 1500);
    assert.match(
      startBody,
      /500[\s\S]*?10000|intervalMs/,
      "startAudioDeviceLevelsPolling must use fast interval when settings are open and 10000ms in background",
    );
  });

  it("T807-4: индикатор активности голоса опирается на события speaking и localMicSpeaking без вызова voice_set_audio_device", () => {
    assert.match(
      appJs,
      /speakingSet\.has\(uid\)\s*\|\|\s*\(isSelf\s*&&\s*Boolean\(liveRtcStatus\?\.localMicSpeaking\)\)/,
      "Speaking indicator must be driven by WebRTC status and speaking events",
    );
    assert.match(
      appJs,
      /setAttribute\("data-speaking",\s*"true"\)/,
      "Speaking indicator must set data-speaking attribute",
    );
  });

  it("T807-5: syncVoiceMuteStateWithBackend кэширует конфигурацию и не спамит voice_set_audio_device при отсутствии изменений", () => {
    const syncIdx = appJs.indexOf("function syncVoiceMuteStateWithBackend(");
    assert.ok(syncIdx > 0, "syncVoiceMuteStateWithBackend must exist in app.js");
    const syncBody = appJs.slice(syncIdx, syncIdx + 5000);

    assert.match(
      syncBody,
      /lastAudioDeviceInputConfig/,
      "syncVoiceMuteStateWithBackend must track lastAudioDeviceInputConfig",
    );
    assert.match(
      syncBody,
      /lastAudioDeviceOutputConfig/,
      "syncVoiceMuteStateWithBackend must track lastAudioDeviceOutputConfig",
    );
    assert.match(
      syncBody,
      /VOICE_SET_AUDIO_DEVICE_IPC_CMD/,
      "syncVoiceMuteStateWithBackend must route through VOICE_SET_AUDIO_DEVICE_IPC_CMD only when changed or forced",
    );
  });

  it("T807-6: syncVoiceMuteStateWithBackend мемоизирует lastVoiceConnectState и не вызывает дублирующий VOICE_CONNECT_IPC_CMD без изменений", () => {
    const syncIdx = appJs.indexOf("function syncVoiceMuteStateWithBackend(");
    assert.ok(syncIdx > 0, "syncVoiceMuteStateWithBackend must exist in app.js");
    const syncBody = appJs.slice(syncIdx, syncIdx + 5000);

    assert.match(
      syncBody,
      /lastVoiceConnectState/,
      "syncVoiceMuteStateWithBackend must memoize lastVoiceConnectState to prevent duplicate VOICE_CONNECT_IPC_CMD dispatches",
    );
    assert.match(
      syncBody,
      /lastVoiceConnectState\.guildId[\s\S]*?lastVoiceConnectState\.channelId[\s\S]*?lastVoiceConnectState\.selfMute[\s\S]*?lastVoiceConnectState\.selfDeaf[\s\S]*?lastVoiceConnectState\.selfVideo[\s\S]*?lastVoiceConnectState\.selfStream/,
      "syncVoiceMuteStateWithBackend must compare guildId, channelId, selfMute, selfDeaf, selfVideo, and selfStream against lastVoiceConnectState",
    );
  });

  it("T807-7: isVoiceSettingsOpen проверяет реальное открытие экрана настроек и вкладки voice (интервал 10s при закрытых настройках)", () => {
    const fnIdx = appJs.indexOf("function pollAudioDeviceLevels(");
    assert.ok(fnIdx > 0, "pollAudioDeviceLevels must exist in app.js");
    const fnBody = appJs.slice(fnIdx, fnIdx + 3500);

    assert.doesNotMatch(
      fnBody,
      /\|\|\s*document\.getElementById\("voice-devices-mic-meter"\)/,
      "isVoiceSettingsOpen must not unconditionally match mounted #voice-devices-mic-meter when settings screen is closed",
    );
    assert.match(
      fnBody,
      /screen-settings|!settingsEl\.hidden/,
      "isVoiceSettingsOpen must verify that settings screen/element is not hidden",
    );
  });
});
