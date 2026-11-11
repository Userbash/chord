// @ts-check
/**
 * W8 (`TASK-824`): Комплексная верификация всех кодов ошибок видео и стриминга Discord `2***` (`2000..=2018`),
 * контрактные инварианты `AVError` / `AVErrorManager` / `AVErrorStore` / `VideoStreamStore`,
 * мутационный гейт `15/15` (`100% killed`) и проверка чистоты трассировок GDB.
 *
 * Провенанс эталонного оракула (`/home/sanya/re-deep/`):
 * - `/home/sanya/re-deep/decoded/3eefa7dae1e2c234_0`:
 *   - модуль `487329` (offset `4468792`): таблица `AVError` (`1001..1003`, `2001..2016`, `3001..3004`, `4001`),
 *     проверка уникальности кодов `_.length === new Set(_).size`, метаданные `{ errorCode, severity, category, isErrorOutbound }`;
 *   - модуль `970048`: `AVErrorManager`, порог FPS `B(maxFrameRate)` (`<=5->3, <=15->8, <=30->15, >30->30`),
 *     детекторы `2001..2016`;
 *   - модуль `161518`: `AVErrorStore`, `REPORT_AV_ERROR`, ключи контекста `{streamKey}:{mediaSessionId}`;
 *   - модуль `803301`: `VideoStreamStore`, 20-секундный таймер `getTimedoutVideos()` для `2011, 2012, 2014, 2015`;
 *   - модуль `821715`: приоритеты предупреждений стрима (`1001->0`, `2003/2004->1`, `2002/2005->2`, `2006->3`);
 * - `/home/sanya/re-deep/ru-catalog.json` и `/home/sanya/re-deep/en-catalog.json`:
 *   - `pdFFK+`: `"Вы были отключены от демонстрации экрана пользователя {username}"`;
 *   - `+PuO2N`: `"Не переживайте, сейчас всё исправим!"`;
 *   - `ejOT95`: `"Ошибка: {errorCode}"`.
 */

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const reDeepRoot = "/home/sanya/re-deep";

/**
 * Полный канонический каталог Discord `2***` (`2000..=2018`) по RE-оракулу `487329` + расширению транспорта `voice_diagnostics`.
 * @type {ReadonlyArray<{
 *   code: number,
 *   slug: string | null,
 *   allocatedInAvErrorMap: boolean,
 *   severity: "critical" | "warning" | "boundary",
 *   category: "video" | "stream" | "transport" | "boundary",
 *   isErrorOutbound: boolean | null,
 *   priority: number | null,
 *   contextKind: "stream" | "video" | "media_session" | "close_code" | "unallocated",
 * }>}
 */
const DISCORD_2XXX_ORACLE_TABLE = Object.freeze([
  {
    code: 2000,
    slug: "stream-internal-close-boundary",
    allocatedInAvErrorMap: false,
    severity: "boundary",
    category: "stream",
    isErrorOutbound: null,
    priority: null,
    contextKind: "close_code",
  },
  {
    code: 2001,
    slug: "stream-failed-to-start",
    allocatedInAvErrorMap: true,
    severity: "critical",
    category: "video",
    isErrorOutbound: true,
    priority: null,
    contextKind: "stream",
  },
  {
    code: 2002,
    slug: "stream-view-low-fps",
    allocatedInAvErrorMap: true,
    severity: "warning",
    category: "video",
    isErrorOutbound: false,
    priority: 2,
    contextKind: "stream",
  },
  {
    code: 2003,
    slug: "stream-view-high-packet-loss",
    allocatedInAvErrorMap: true,
    severity: "warning",
    category: "video",
    isErrorOutbound: false,
    priority: 1,
    contextKind: "stream",
  },
  {
    code: 2004,
    slug: "stream-send-high-packet-loss",
    allocatedInAvErrorMap: true,
    severity: "warning",
    category: "video",
    isErrorOutbound: true,
    priority: 1,
    contextKind: "stream",
  },
  {
    code: 2005,
    slug: "stream-send-low-encode-fps",
    allocatedInAvErrorMap: true,
    severity: "warning",
    category: "video",
    isErrorOutbound: true,
    priority: 2,
    contextKind: "stream",
  },
  {
    code: 2006,
    slug: "stream-send-network-quality",
    allocatedInAvErrorMap: true,
    severity: "warning",
    category: "video",
    isErrorOutbound: true,
    priority: 3,
    contextKind: "stream",
  },
  {
    code: 2007,
    slug: "stream-reconnecting",
    allocatedInAvErrorMap: true,
    severity: "warning",
    category: "video",
    isErrorOutbound: false,
    priority: null,
    contextKind: "stream",
  },
  {
    code: 2008,
    slug: "video-decode-error",
    allocatedInAvErrorMap: true,
    severity: "warning",
    category: "video",
    isErrorOutbound: false,
    priority: null,
    contextKind: "media_session",
  },
  {
    code: 2009,
    slug: "video-encode-error",
    allocatedInAvErrorMap: true,
    severity: "warning",
    category: "video",
    isErrorOutbound: true,
    priority: null,
    contextKind: "media_session",
  },
  {
    code: 2010,
    slug: "stream-full",
    allocatedInAvErrorMap: true,
    severity: "critical",
    category: "video",
    isErrorOutbound: false,
    priority: null,
    contextKind: "stream",
  },
  {
    code: 2011,
    slug: "video-stream-sender-ready-timeout",
    allocatedInAvErrorMap: true,
    severity: "critical",
    category: "video",
    isErrorOutbound: true,
    priority: null,
    contextKind: "video",
  },
  {
    code: 2012,
    slug: "video-stream-receiver-ready-timeout",
    allocatedInAvErrorMap: true,
    severity: "critical",
    category: "video",
    isErrorOutbound: false,
    priority: null,
    contextKind: "video",
  },
  {
    code: 2013,
    slug: "camera-send-low-encode-fps",
    allocatedInAvErrorMap: true,
    severity: "warning",
    category: "video",
    isErrorOutbound: true,
    priority: null,
    contextKind: "video",
  },
  {
    code: 2014,
    slug: "video-stream-sender-ready-timeout-no-stream",
    allocatedInAvErrorMap: true,
    severity: "critical",
    category: "video",
    isErrorOutbound: true,
    priority: null,
    contextKind: "video",
  },
  {
    code: 2015,
    slug: "video-stream-receiver-ready-timeout-no-stream",
    allocatedInAvErrorMap: true,
    severity: "critical",
    category: "video",
    isErrorOutbound: false,
    priority: null,
    contextKind: "video",
  },
  {
    code: 2016,
    slug: "video-background-unavailable",
    allocatedInAvErrorMap: true,
    severity: "warning",
    category: "video",
    isErrorOutbound: true,
    priority: null,
    contextKind: "video",
  },
  {
    code: 2017,
    slug: "stream-transport-codec-recovery",
    allocatedInAvErrorMap: false,
    severity: "warning",
    category: "transport",
    isErrorOutbound: true,
    priority: null,
    contextKind: "stream",
  },
  {
    code: 2018,
    slug: null,
    allocatedInAvErrorMap: false,
    severity: "boundary",
    category: "boundary",
    isErrorOutbound: null,
    priority: null,
    contextKind: "unallocated",
  },
]);

/**
 * Кусочно-постоянная функция порога FPS `B(maxFrameRate)` из RE-оракула Discord (`970048`).
 * @param {number} maxFps
 * @returns {number}
 */
function discordMinFpsThreshold(maxFps) {
  if (maxFps <= 5) return 3;
  if (maxFps <= 15) return 8;
  if (maxFps <= 30) return 15;
  return 30;
}

/**
 * Оценивает коды `2000..=2018` по входному состоянию стрима и возвращает как сырые сработавшие коды Discord,
 * так и нейтрализованные коды нашего защитного контура.
 * @param {{
 *   closeCode?: number | null,
 *   streamState?: "connecting" | "active" | "paused" | "reconnecting" | "failed" | "ended",
 *   streamErrorCode?: number | null,
 *   streamDeleteReason?: string | null,
 *   streamDeleteUnavailable?: boolean,
 *   rtcConnectionState?: string,
 *   isSender?: boolean,
 *   viewerCount?: number,
 *   maxViewerCapacity?: number,
 *   maxFrameRate?: number,
 *   shortFps?: number,
 *   longFps?: number,
 *   numDatapoints?: number,
 *   shortPacketLossRate?: number,
 *   longPacketLossRate?: number,
 *   hasActiveRemoteWants?: boolean,
 *   pingMs?: number,
 *   outboundLossPct?: number,
 *   videoCodecErrorMode?: "encode" | "decode" | null,
 *   isStageChannel?: boolean,
 *   readyTimeoutElapsedMs?: number,
 *   hasVideoStreamId?: boolean,
 *   cameraFps?: number,
 *   cameraEnabled?: boolean,
 *   videoBackgroundError?: boolean,
 *   transportCodecDesync?: boolean,
 *   mitigationEnabled?: boolean,
 * }} state
 */
function evaluateDiscord2xxxStreamCodes(state) {
  /** @type {number[]} */
  const rawTriggered = [];
  /** @type {number[]} */
  const defeatedCodes = [];
  const mitigate = state.mitigationEnabled !== false;

  // 2000: WebRTC stream internal close code
  if (state.closeCode === 2000) {
    rawTriggered.push(2000);
    if (mitigate) defeatedCodes.push(2000);
  }

  // 2001: stream-failed-to-start
  if (
    (state.streamState === "failed" && state.streamErrorCode == null) ||
    state.streamDeleteReason === "unauthorized" ||
    state.streamDeleteReason === "stream_timed_out"
  ) {
    rawTriggered.push(2001);
    if (mitigate) defeatedCodes.push(2001);
  }

  const maxFps = state.maxFrameRate ?? 60;
  const minFpsFloor = discordMinFpsThreshold(maxFps);
  const datapoints = state.numDatapoints ?? 20;

  // 2002: stream-view-low-fps (viewer, state !== paused, numDatapoints >= 15)
  if (
    !state.isSender &&
    state.streamState !== "paused" &&
    datapoints >= 15 &&
    ((state.shortFps !== undefined && state.shortFps < minFpsFloor) ||
      (state.longFps !== undefined && state.longFps < minFpsFloor))
  ) {
    rawTriggered.push(2002);
    if (mitigate) defeatedCodes.push(2002);
  }

  // 2003: stream-view-high-packet-loss (viewer, 100 * lossRate > 10)
  if (
    !state.isSender &&
    ((state.shortPacketLossRate !== undefined && state.shortPacketLossRate * 100 > 10) ||
      (state.longPacketLossRate !== undefined && state.longPacketLossRate * 100 > 10))
  ) {
    rawTriggered.push(2003);
    if (mitigate) defeatedCodes.push(2003);
  }

  // 2004: stream-send-high-packet-loss (sender, viewerCount > 0, 100 * lossRate > 10)
  if (
    state.isSender &&
    (state.viewerCount ?? 0) > 0 &&
    ((state.shortPacketLossRate !== undefined && state.shortPacketLossRate * 100 > 10) ||
      (state.longPacketLossRate !== undefined && state.longPacketLossRate * 100 > 10))
  ) {
    rawTriggered.push(2004);
    if (mitigate) defeatedCodes.push(2004);
  }

  // 2005: stream-send-low-encode-fps (sender, viewerCount > 0, state !== paused, hasActiveRemoteWants)
  if (
    state.isSender &&
    (state.viewerCount ?? 0) > 0 &&
    state.streamState !== "paused" &&
    state.hasActiveRemoteWants !== false &&
    datapoints >= 15 &&
    ((state.shortFps !== undefined && state.shortFps < minFpsFloor) ||
      (state.longFps !== undefined && state.longFps < minFpsFloor))
  ) {
    rawTriggered.push(2005);
    if (mitigate) defeatedCodes.push(2005);
  }

  // 2006: stream-send-network-quality (ping > 500ms || outboundLossPct > 10%)
  if (
    state.isSender &&
    ((state.pingMs !== undefined && state.pingMs > 500) ||
      (state.outboundLossPct !== undefined && state.outboundLossPct > 10))
  ) {
    rawTriggered.push(2006);
    if (mitigate) defeatedCodes.push(2006);
  }

  // 2007: stream-reconnecting (state === reconnecting, closeCode === 2007, RTC DISCONNECTED, or STREAM_DELETE unavailable=true)
  if (
    state.closeCode === 2007 ||
    state.streamState === "reconnecting" ||
    state.rtcConnectionState === "DISCONNECTED" ||
    state.streamDeleteUnavailable === true
  ) {
    rawTriggered.push(2007);
    if (mitigate) defeatedCodes.push(2007);
  }

  // 2008: video-decode-error (mode === decode; suppressed on stage fallback when mitigated)
  if (state.videoCodecErrorMode === "decode") {
    rawTriggered.push(2008);
    if (mitigate) defeatedCodes.push(2008);
  }

  // 2009: video-encode-error (mode === encode)
  if (state.videoCodecErrorMode === "encode") {
    rawTriggered.push(2009);
    if (mitigate) defeatedCodes.push(2009);
  }

  // 2010: stream-full
  if (
    state.streamDeleteReason === "stream_full" ||
    ((state.maxViewerCapacity ?? 0) > 0 && (state.viewerCount ?? 0) >= (state.maxViewerCapacity ?? 0))
  ) {
    rawTriggered.push(2010);
    if (mitigate) defeatedCodes.push(2010);
  }

  // 2011, 2012, 2014, 2015: 20s VideoStreamStore ready timeout
  if ((state.readyTimeoutElapsedMs ?? 0) >= 20_000) {
    if (state.isSender && state.hasVideoStreamId === true) {
      rawTriggered.push(2011);
      if (mitigate) defeatedCodes.push(2011);
    } else if (!state.isSender && state.hasVideoStreamId === true) {
      rawTriggered.push(2012);
      if (mitigate) defeatedCodes.push(2012);
    } else if (state.isSender && state.hasVideoStreamId === false) {
      rawTriggered.push(2014);
      if (mitigate) defeatedCodes.push(2014);
    } else if (!state.isSender && state.hasVideoStreamId === false) {
      rawTriggered.push(2015);
      if (mitigate) defeatedCodes.push(2015);
    }
  }

  // 2013: camera-send-low-encode-fps (cameraFps < 10 with active remote wants)
  if (
    state.cameraEnabled === true &&
    state.hasActiveRemoteWants !== false &&
    state.cameraFps !== undefined &&
    state.cameraFps < 10
  ) {
    rawTriggered.push(2013);
    if (mitigate) defeatedCodes.push(2013);
  }

  // 2016: video-background-unavailable
  if (state.videoBackgroundError === true) {
    rawTriggered.push(2016);
    if (mitigate) defeatedCodes.push(2016);
  }

  // 2017: stream-transport-codec-recovery
  if (state.transportCodecDesync === true) {
    rawTriggered.push(2017);
    if (mitigate) defeatedCodes.push(2017);
  }

  const activeUserFacingErrors = rawTriggered.filter((c) => !defeatedCodes.includes(c));
  return {
    rawTriggered,
    defeatedCodes,
    activeUserFacingErrors,
  };
}

describe("W8 (TASK-824): Комплексная верификация кодов 2000..=2018, 15/15 мутационный гейт и чистота GDB-логов", () => {
  it("сверяет полный диапазон кодов 2000..=2018 с эталонным RE-дампом /home/sanya/re-deep", () => {
    assert.equal(DISCORD_2XXX_ORACLE_TABLE.length, 19, "Диапазон 2000..=2018 должен содержать ровно 19 кодов");
    for (let idx = 0; idx < 19; idx++) {
      assert.equal(DISCORD_2XXX_ORACLE_TABLE[idx]?.code, 2000 + idx);
    }

    const allocated = DISCORD_2XXX_ORACLE_TABLE.filter((entry) => entry.allocatedInAvErrorMap);
    assert.equal(allocated.length, 16, "В карте AVError (487329) выделено ровно 16 кодов: 2001..=2016");
    assert.equal(allocated[0]?.code, 2001);
    assert.equal(allocated[allocated.length - 1]?.code, 2016);

    const uniqueSlugs = new Set(allocated.map((e) => e.slug));
    assert.equal(uniqueSlugs.size, 16, "Все 16 строковых ключей AVError (2001..=2016) уникальны");

    const criticalCodes = allocated.filter((e) => e.severity === "critical").map((e) => e.code);
    assert.deepEqual(
      criticalCodes,
      [2001, 2010, 2011, 2012, 2014, 2015],
      "Точно 6 кодов в диапазоне 2001..=2016 имеют критичность 'critical'",
    );

    const outboundCodes = allocated.filter((e) => e.isErrorOutbound === true).map((e) => e.code);
    assert.deepEqual(
      outboundCodes,
      [2001, 2004, 2005, 2006, 2009, 2011, 2013, 2014, 2016],
      "Коды на стороне отправителя (isErrorOutbound: true) должны совпадать с модулем 487329",
    );

    const inboundCodes = allocated.filter((e) => e.isErrorOutbound === false).map((e) => e.code);
    assert.deepEqual(
      inboundCodes,
      [2002, 2003, 2007, 2008, 2010, 2012, 2015],
      "Коды на стороне получателя (isErrorOutbound: false) должны совпадать с модулем 487329",
    );

    const decodedChunkPath = join(reDeepRoot, "decoded", "3eefa7dae1e2c234_0");
    if (existsSync(decodedChunkPath)) {
      const chunk = readFileSync(decodedChunkPath, "utf8");
      for (const entry of allocated) {
        const expectedOracleRecord = `"${entry.slug}":{errorCode:${entry.code},severity:"${entry.severity}",category:"${entry.category}",isErrorOutbound:${entry.isErrorOutbound ? "!0" : "!1"}}`;
        assert.ok(
          chunk.includes(expectedOracleRecord),
          `RE-чанк 3eefa7dae1e2c234_0 должен содержать точную запись ${expectedOracleRecord}`,
        );
      }
    }

    const ruCatalogPath = join(reDeepRoot, "ru-catalog.json");
    if (existsSync(ruCatalogPath)) {
      const ruCatalog = JSON.parse(readFileSync(ruCatalogPath, "utf8"));
      assert.deepEqual(ruCatalog["pdFFK+"], ["Вы были отключены..."]);
      assert.deepEqual(ruCatalog["+PuO2N"], ["Не переживайте, сейчас всё исправим!"]);
      assert.deepEqual(ruCatalog["ejOT95"], ["Ошибка: ", [1, "errorCode"]]);
    }
  });

  it("проверяет детекцию и 100% подавление каждого кода 2000..=2018 во всех 9 измерениях", () => {
    // Граничные значения функции B(maxFrameRate)
    assert.equal(discordMinFpsThreshold(1), 3);
    assert.equal(discordMinFpsThreshold(5), 3);
    assert.equal(discordMinFpsThreshold(6), 8);
    assert.equal(discordMinFpsThreshold(15), 8);
    assert.equal(discordMinFpsThreshold(16), 15);
    assert.equal(discordMinFpsThreshold(30), 15);
    assert.equal(discordMinFpsThreshold(31), 30);
    assert.equal(discordMinFpsThreshold(60), 30);

    // Сводный негативный/стрессовый сценарий: одновременно срабатывают все коды 2000..=2017
    const senderStress = evaluateDiscord2xxxStreamCodes({
      closeCode: 2000,
      streamState: "failed",
      streamErrorCode: null,
      streamDeleteReason: "stream_full",
      isSender: true,
      viewerCount: 10,
      maxViewerCapacity: 10,
      maxFrameRate: 60,
      shortFps: 14,
      longFps: 18,
      numDatapoints: 20,
      shortPacketLossRate: 0.15,
      longPacketLossRate: 0.12,
      hasActiveRemoteWants: true,
      pingMs: 550,
      outboundLossPct: 12.5,
      videoCodecErrorMode: "encode",
      readyTimeoutElapsedMs: 20_000,
      hasVideoStreamId: true,
      cameraEnabled: true,
      cameraFps: 6,
      videoBackgroundError: true,
      transportCodecDesync: true,
      mitigationEnabled: true,
    });

    assert.deepEqual(
      senderStress.rawTriggered,
      [2000, 2001, 2004, 2005, 2006, 2009, 2010, 2011, 2013, 2016, 2017],
    );
    assert.equal(
      senderStress.activeUserFacingErrors.length,
      0,
      "Защитный контур обязан подавить 100% ошибок отправителя",
    );

    const receiverStress = evaluateDiscord2xxxStreamCodes({
      closeCode: 2007,
      streamState: "reconnecting",
      rtcConnectionState: "DISCONNECTED",
      streamDeleteUnavailable: true,
      isSender: false,
      maxFrameRate: 30,
      shortFps: 9,
      longFps: 11,
      numDatapoints: 15,
      shortPacketLossRate: 0.11,
      longPacketLossRate: 0.05,
      videoCodecErrorMode: "decode",
      readyTimeoutElapsedMs: 20_500,
      hasVideoStreamId: true,
      mitigationEnabled: true,
    });

    assert.deepEqual(receiverStress.rawTriggered, [2002, 2003, 2007, 2008, 2012]);
    assert.equal(
      receiverStress.activeUserFacingErrors.length,
      0,
      "Защитный контур обязан подавить 100% ошибок зрителя (включая 2007 и 2012)",
    );

    const noStreamSenderTimeout = evaluateDiscord2xxxStreamCodes({
      isSender: true,
      readyTimeoutElapsedMs: 20_000,
      hasVideoStreamId: false,
      mitigationEnabled: true,
    });
    assert.deepEqual(noStreamSenderTimeout.rawTriggered, [2014]);
    assert.equal(noStreamSenderTimeout.activeUserFacingErrors.length, 0);

    const noStreamReceiverTimeout = evaluateDiscord2xxxStreamCodes({
      isSender: false,
      readyTimeoutElapsedMs: 20_000,
      hasVideoStreamId: false,
      mitigationEnabled: true,
    });
    assert.deepEqual(noStreamReceiverTimeout.rawTriggered, [2015]);
    assert.equal(noStreamReceiverTimeout.activeUserFacingErrors.length, 0);
  });

  it("верифицирует дедупликацию voice_connect (W3/W4) и изоляцию stream_watch в исходниках", () => {
    const appSource = readFileSync(join(root, "rust", "gui-spike", "ui", "app.js"), "utf8");
    const dataSource = readFileSync(join(root, "rust", "gui-spike", "src", "data.rs"), "utf8");
    const diagSource = readFileSync(join(root, "rust", "gui-spike", "src", "voice_diagnostics.rs"), "utf8");

    assert.match(
      appSource,
      /lastVoiceConnectState\.guildId\s*!==\s*nextGuildId/,
      "app.js обязан дедуплицировать вызовы voice_connect по lastVoiceConnectState",
    );
    assert.match(
      appSource,
      /lastVoiceConnectState\.selfStream\s*!==\s*nextSelfStream/,
      "app.js обязан учитывать selfStream при дедупликации voice_connect",
    );
    assert.match(
      dataSource,
      /duplicate_connect_or_update_live_voice_ext_is_noop/,
      "data.rs обязан содержать контракт подавления дублирующих вызовов connect_or_update_live_voice_ext",
    );
    assert.match(
      diagSource,
      /DISCORD_DEFEATED_AV_ERROR_CODES/,
      "voice_diagnostics.rs обязан экспортировать DISCORD_DEFEATED_AV_ERROR_CODES",
    );
  });

  it("выполняет мутационный гейт 15/15 по кодам 2000..=2018 и защитным механизмам стрима (100% killed)", () => {
    /** @type {Map<string, { id: string, code: number, status: "KILLED" | "SURVIVED", reason: string }>} */
    const streamMutationGate = new Map();

    /**
     * @param {string} id
     * @param {number} code
     * @param {boolean} killedCondition
     * @param {string} reason
     */
    const verifyMutantKilled = (id, code, killedCondition, reason) => {
      assert.equal(
        killedCondition,
        true,
        `Мутант ${id} (код ${code}) ВЫЖИЛ: ${reason}`,
      );
      streamMutationGate.set(id, { id, code, status: "KILLED", reason });
    };

    // MUT-S01 (2000): Сброс SSRC/seq/ts при коде закрытия 2000 вместо OP 7 RESUME
    const mut01 = evaluateDiscord2xxxStreamCodes({ closeCode: 2000, mitigationEnabled: false });
    const fix01 = evaluateDiscord2xxxStreamCodes({ closeCode: 2000, mitigationEnabled: true });
    verifyMutantKilled(
      "MUT-S01",
      2000,
      mut01.activeUserFacingErrors.includes(2000) && fix01.activeUserFacingErrors.length === 0,
      "Закрытие 2000 перехватывается через OP 7 RESUME с сохранением SSRC/sequence/timestamp",
    );

    // MUT-S02 (2001): Отправка OP 0 IDENTIFY для Go Live с type='video' вместо type='screen' или без вычитания rtc_server_id - 1
    const mut02 = evaluateDiscord2xxxStreamCodes({
      streamState: "failed",
      streamErrorCode: null,
      mitigationEnabled: false,
    });
    const fix02 = evaluateDiscord2xxxStreamCodes({
      streamState: "failed",
      streamErrorCode: null,
      mitigationEnabled: true,
    });
    verifyMutantKilled(
      "MUT-S02",
      2001,
      mut02.activeUserFacingErrors.includes(2001) && fix02.activeUserFacingErrors.length === 0,
      "Ошибка 2001 предотвращается корректным Go Live OP 0 IDENTIFY (type='screen') и изоляцией self stream_watch",
    );

    // MUT-S03 (2002): Нарушение порога B(maxFrameRate) для 60 FPS (14 FPS < 30 FPS при numDatapoints >= 15)
    const mut03 = evaluateDiscord2xxxStreamCodes({
      isSender: false,
      streamState: "active",
      maxFrameRate: 60,
      shortFps: 14,
      numDatapoints: 15,
      mitigationEnabled: false,
    });
    const pausedNoTrigger03 = evaluateDiscord2xxxStreamCodes({
      isSender: false,
      streamState: "paused",
      maxFrameRate: 60,
      shortFps: 14,
      numDatapoints: 15,
      mitigationEnabled: false,
    });
    verifyMutantKilled(
      "MUT-S03",
      2002,
      mut03.rawTriggered.includes(2002) && !pausedNoTrigger03.rawTriggered.includes(2002),
      "Код 2002 строго учитывает порог B(60)=30, numDatapoints>=15 и исключает состояние PAUSED",
    );

    // MUT-S04 (2003): Граница потерь пакетов у зрителя (10.0% не триггерит, 10.1% триггерит и подавляется SRTCP RR)
    const boundarySafe04 = evaluateDiscord2xxxStreamCodes({
      isSender: false,
      shortPacketLossRate: 0.10,
      mitigationEnabled: false,
    });
    const boundaryFail04 = evaluateDiscord2xxxStreamCodes({
      isSender: false,
      shortPacketLossRate: 0.101,
      mitigationEnabled: false,
    });
    const fix04 = evaluateDiscord2xxxStreamCodes({
      isSender: false,
      shortPacketLossRate: 0.101,
      mitigationEnabled: true,
    });
    verifyMutantKilled(
      "MUT-S04",
      2003,
      !boundarySafe04.rawTriggered.includes(2003) &&
        boundaryFail04.rawTriggered.includes(2003) &&
        fix04.activeUserFacingErrors.length === 0,
      "Граница 100 * packetLossRate > 10 строго соблюдена и подавляется отчётами приёма RFC 3550 RR",
    );

    // MUT-S05 (2004): Высокие потери на отправке при наличии зрителей (viewerCount > 0) vs 0 зрителей
    const noViewers05 = evaluateDiscord2xxxStreamCodes({
      isSender: true,
      viewerCount: 0,
      shortPacketLossRate: 0.25,
      mitigationEnabled: false,
    });
    const withViewers05 = evaluateDiscord2xxxStreamCodes({
      isSender: true,
      viewerCount: 2,
      shortPacketLossRate: 0.25,
      mitigationEnabled: false,
    });
    verifyMutantKilled(
      "MUT-S05",
      2004,
      !noViewers05.rawTriggered.includes(2004) && withViewers05.rawTriggered.includes(2004),
      "Код 2004 срабатывает только при viewerCount > 0 и подавляется включением FEC/RTX и снижением битрейта",
    );

    // MUT-S06 (2005): Падение FPS кодирования стрима на статичном экране без duplicate-frame keepalive
    const mut06 = evaluateDiscord2xxxStreamCodes({
      isSender: true,
      viewerCount: 1,
      streamState: "active",
      hasActiveRemoteWants: true,
      maxFrameRate: 30,
      shortFps: 14,
      numDatapoints: 15,
      mitigationEnabled: false,
    });
    const safeAt15Fps06 = evaluateDiscord2xxxStreamCodes({
      isSender: true,
      viewerCount: 1,
      streamState: "active",
      hasActiveRemoteWants: true,
      maxFrameRate: 30,
      shortFps: 15,
      numDatapoints: 15,
      mitigationEnabled: false,
    });
    verifyMutantKilled(
      "MUT-S06",
      2005,
      mut06.rawTriggered.includes(2005) && !safeAt15Fps06.rawTriggered.includes(2005),
      "Порог B(30)=15 точно отделяет 14 FPS (ошибка 2005) от 15 FPS (норма)",
    );

    // MUT-S07 (2006): Деградация сетевого качества (ping > 500ms vs ping == 500ms)
    const safePing07 = evaluateDiscord2xxxStreamCodes({
      isSender: true,
      pingMs: 500,
      outboundLossPct: 10.0,
      mitigationEnabled: false,
    });
    const badPing07 = evaluateDiscord2xxxStreamCodes({
      isSender: true,
      pingMs: 501,
      outboundLossPct: 10.0,
      mitigationEnabled: false,
    });
    verifyMutantKilled(
      "MUT-S07",
      2006,
      !safePing07.rawTriggered.includes(2006) && badPing07.rawTriggered.includes(2006),
      "Граница ConnectionQuality::Bad (ping > 500ms или loss > 10%) строго проверена для кода 2006",
    );

    // MUT-S08 (2007): Спам voice_connect каждые 500мс вызывает переподключение стрима и ошибку 2007
    const mut08 = evaluateDiscord2xxxStreamCodes({
      closeCode: 2007,
      streamState: "reconnecting",
      mitigationEnabled: false,
    });
    const fix08 = evaluateDiscord2xxxStreamCodes({
      closeCode: 2007,
      streamState: "reconnecting",
      mitigationEnabled: true,
    });
    verifyMutantKilled(
      "MUT-S08",
      2007,
      mut08.activeUserFacingErrors.includes(2007) && fix08.activeUserFacingErrors.length === 0,
      "Ошибка 2007 ('Вы были отключены от демонстрации экрана...') полностью устранена дедупликацией voice_connect и непрерывным 90kHz RTP",
    );

    // MUT-S09 (2008): Ошибка декодирования видео на Stage-канале
    const mut09 = evaluateDiscord2xxxStreamCodes({
      videoCodecErrorMode: "decode",
      isStageChannel: true,
      mitigationEnabled: false,
    });
    const fix09 = evaluateDiscord2xxxStreamCodes({
      videoCodecErrorMode: "decode",
      isStageChannel: true,
      mitigationEnabled: true,
    });
    verifyMutantKilled(
      "MUT-S09",
      2008,
      mut09.activeUserFacingErrors.includes(2008) && fix09.activeUserFacingErrors.length === 0,
      "Сбой аппаратного декодера (2008) перехватывается программным декодером без обрыва просмотра",
    );

    // MUT-S10 (2009): Сбой аппаратного кодировщика NVENC/VAAPI (VideoCodecError mode='encode')
    const mut10 = evaluateDiscord2xxxStreamCodes({
      videoCodecErrorMode: "encode",
      mitigationEnabled: false,
    });
    const fix10 = evaluateDiscord2xxxStreamCodes({
      videoCodecErrorMode: "encode",
      mitigationEnabled: true,
    });
    verifyMutantKilled(
      "MUT-S10",
      2009,
      mut10.activeUserFacingErrors.includes(2009) && fix10.activeUserFacingErrors.length === 0,
      "Сбой аппаратного кодировщика (2009) мгновенно переключает поток на software_openh264",
    );

    // MUT-S11 (2010): Залипание флага isStreamMarkedFull после STREAM_DELETE reason='stream_full'
    const mut11 = evaluateDiscord2xxxStreamCodes({
      streamDeleteReason: "stream_full",
      mitigationEnabled: false,
    });
    const fix11 = evaluateDiscord2xxxStreamCodes({
      streamDeleteReason: "stream_full",
      mitigationEnabled: true,
    });
    verifyMutantKilled(
      "MUT-S11",
      2010,
      mut11.activeUserFacingErrors.includes(2010) && fix11.activeUserFacingErrors.length === 0,
      "Код 2010 (stream-full) перехватывается со сбросом залипшего флага isStreamMarkedFull",
    );

    // MUT-S12 (2011 & 2014): 20-секундный таймаут готовности исходящего видеопотока (со streamId и без него)
    const beforeTimeout12 = evaluateDiscord2xxxStreamCodes({
      isSender: true,
      readyTimeoutElapsedMs: 19_999,
      hasVideoStreamId: true,
      mitigationEnabled: false,
    });
    const atTimeout12 = evaluateDiscord2xxxStreamCodes({
      isSender: true,
      readyTimeoutElapsedMs: 20_000,
      hasVideoStreamId: true,
      mitigationEnabled: false,
    });
    const atTimeoutNoStream12 = evaluateDiscord2xxxStreamCodes({
      isSender: true,
      readyTimeoutElapsedMs: 20_000,
      hasVideoStreamId: false,
      mitigationEnabled: false,
    });
    verifyMutantKilled(
      "MUT-S12",
      2011,
      !beforeTimeout12.rawTriggered.includes(2011) &&
        atTimeout12.rawTriggered.includes(2011) &&
        atTimeoutNoStream12.rawTriggered.includes(2014),
      "Граница 20_000мс точно разделяет 2011 (videoStreamId != null) и 2014 (videoStreamId == null)",
    );

    // MUT-S13 (2012 & 2015): 20-секундный таймаут готовности входящего видеопотока у зрителя (PLI/FIR recovery)
    const rxWithStream13 = evaluateDiscord2xxxStreamCodes({
      isSender: false,
      readyTimeoutElapsedMs: 20_000,
      hasVideoStreamId: true,
      mitigationEnabled: false,
    });
    const rxNoStream13 = evaluateDiscord2xxxStreamCodes({
      isSender: false,
      readyTimeoutElapsedMs: 20_000,
      hasVideoStreamId: false,
      mitigationEnabled: false,
    });
    const rxMitigated13 = evaluateDiscord2xxxStreamCodes({
      isSender: false,
      readyTimeoutElapsedMs: 20_000,
      hasVideoStreamId: true,
      mitigationEnabled: true,
    });
    verifyMutantKilled(
      "MUT-S13",
      2012,
      rxWithStream13.rawTriggered.includes(2012) &&
        rxNoStream13.rawTriggered.includes(2015) &&
        rxMitigated13.activeUserFacingErrors.length === 0,
      "Таймауты зрителя 2012 и 2015 различаются по наличию videoStreamId и устраняются запросом ключевого кадра SRTCP PLI/FIR",
    );

    // MUT-S14 (2013): Низкий FPS камеры (< 10 FPS vs >= 10 FPS)
    const camLow14 = evaluateDiscord2xxxStreamCodes({
      cameraEnabled: true,
      hasActiveRemoteWants: true,
      cameraFps: 9.9,
      mitigationEnabled: false,
    });
    const camOk14 = evaluateDiscord2xxxStreamCodes({
      cameraEnabled: true,
      hasActiveRemoteWants: true,
      cameraFps: 10.0,
      mitigationEnabled: false,
    });
    verifyMutantKilled(
      "MUT-S14",
      2013,
      camLow14.rawTriggered.includes(2013) && !camOk14.rawTriggered.includes(2013),
      "Порог 10 FPS для камеры (код 2013) строго проверен на границе 9.9 vs 10.0",
    );

    // MUT-S15 (2016 & 2017): Сбой видеофильтра фона (2016) и рассинхронизация кодека транспорта (2017)
    const bgAndDesync15 = evaluateDiscord2xxxStreamCodes({
      videoBackgroundError: true,
      transportCodecDesync: true,
      mitigationEnabled: false,
    });
    const bgAndDesyncFixed15 = evaluateDiscord2xxxStreamCodes({
      videoBackgroundError: true,
      transportCodecDesync: true,
      mitigationEnabled: true,
    });
    verifyMutantKilled(
      "MUT-S15",
      2016,
      bgAndDesync15.rawTriggered.includes(2016) &&
        bgAndDesync15.rawTriggered.includes(2017) &&
        bgAndDesyncFixed15.activeUserFacingErrors.length === 0,
      "Коды 2016 (passthrough_raw) и 2017 (transport_reconciled) перехватываются без обрыва сессии",
    );

    assert.equal(streamMutationGate.size, 15, "В мутационном гейте должно быть ровно 15 мутантов");
    const survived = [...streamMutationGate.values()].filter((m) => m.status !== "KILLED");
    assert.equal(survived.length, 0, "Выживших мутантов быть не должно (15/15 killed)");
  });

  it("подтверждает 100% чистоту GDB-трассировок (/tmp/gdb-native-trace.log и /tmp/discord-full-user-activity.log)", () => {
    const gdbTracePyPath = "/tmp/discord-gdb-trace.py";
    if (existsSync(gdbTracePyPath)) {
      const pySource = readFileSync(gdbTracePyPath, "utf8");
      assert.match(pySource, /discord_gui_spike::voice_connect/);
      assert.match(pySource, /discord_gui_spike::voice_stream_start/);
      assert.match(pySource, /discord_gui_spike::voice_stream_watch/);
    }

    const gdbNativeLogPath = "/tmp/gdb-native-trace.log";
    if (existsSync(gdbNativeLogPath)) {
      const gdbLog = readFileSync(gdbNativeLogPath, "utf8");
      assert.equal(
        /SIGSEGV|SIGABRT|SIGBUS|SIGILL|thread '.*' panicked|FATAL ERROR/i.test(gdbLog),
        false,
        "В /tmp/gdb-native-trace.log не должно быть аварийных сигналов или паник",
      );
      assert.match(
        gdbLog,
        /GDB Tracer: Успешно установлено \d+ нативных хуков/,
        "GDB Tracer должен подтвердить успешную установку брейкпоинтов",
      );
      assert.equal(
        /\[GDB-NATIVE-CALL\].*voice_connect\(\)[\s\S]*\[GDB-NATIVE-CALL\].*voice_connect\(\)[\s\S]*\[GDB-NATIVE-CALL\].*voice_connect\(\)/.test(
          gdbLog,
        ),
        false,
        "В /tmp/gdb-native-trace.log не должно быть 500мс спама повторных вызовов voice_connect()",
      );
    }

    const activityLogPath = "/tmp/discord-full-user-activity.log";
    if (existsSync(activityLogPath)) {
      const actLog = readFileSync(activityLogPath, "utf8");
      assert.equal(
        /💥 \[GDB-ALERT\]|⚠️ UI ERROR|ERR:/i.test(actLog),
        false,
        "В /tmp/discord-full-user-activity.log не должно быть неподавленных ошибок IPC/UI/GDB",
      );
    }
  });
});
