// @ts-check
/**
 * TASK-779 (G4 QA): Регрессионный сьют тестов стриминга:
 * 1. Проверка контракта WebRTC: `sourceId: '-1'`, `type: 'screen'`, исключение сброса захватчика в `nil`.
 * 2. Проверка жизненного цикла Stage Viewer и PIP: минимизация в PIP vs полное прекращение стрима.
 * 3. Проверка горячей смены окна через `/dev/shm/discord_capture_target.txt`.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");

const CAPTURE_TARGET_SHM_PATH = "/dev/shm/discord_capture_target.txt";
const CAPTURE_KICK_SHM_PATH = "/dev/shm/discord_capture_kick.bin";

describe("TASK-779: Комплексный регрессионный сьют трансляции экрана (WebRTC, Stage Viewer/PIP, Hot Window Switch)", () => {
  const wsRs = fs.readFileSync(path.join(REPO_ROOT, "rust/crates/chord-core/src/ws.rs"), "utf8");
  const dataRs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/src/data.rs"), "utf8");
  const mainRs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/src/main.rs"), "utf8");
  const appJs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/ui/app.js"), "utf8");

  describe("1. Контракт WebRTC: sourceId: '-1', type: 'screen', исключение сброса захватчика в nil", () => {
    it("T779-1a: ws.rs applyScreenBroadcast и set_stream_source всегда передают type: 'screen', sourceId: '-1' и не сбрасывают захватчик в nil", () => {
      const bcastIdx = wsRs.indexOf("function applyScreenBroadcast(");
      assert.ok(bcastIdx > 0, "applyScreenBroadcast должна существовать в ws.rs");
      const bcastBody = wsRs.slice(bcastIdx, bcastIdx + 5000);

      assert.match(
        bcastBody,
        /conn\.setDesktopSourceWithOptions\(\{\s*type:\s*"screen",\s*sourceId:\s*"-1",\s*width:\s*1280,\s*height:\s*720,\s*frameRate:\s*60/,
        "applyScreenBroadcast(true) обязан вызывать setDesktopSourceWithOptions с type: 'screen', sourceId: '-1' (kFullDesktopScreenId)",
      );
      assert.doesNotMatch(
        bcastBody,
        /conn\.setDesktopSourceWithOptions\(\{\s*type:\s*"screen",\s*sourceId:\s*"0"/,
        "applyScreenBroadcast не должен передавать sourceId: '0', вызывающий сбой ScreenCapturerX11::SelectSource и сброс в (nil)",
      );

      // Ветка active === true не должна вызывать clearDesktopSource()
      const activeBranchEnd = bcastBody.indexOf("} else {");
      assert.ok(activeBranchEnd > 0, "В applyScreenBroadcast должна быть ветка active / else");
      const activeBranch = bcastBody.slice(0, activeBranchEnd);
      assert.doesNotMatch(
        activeBranch,
        /clearDesktopSource\s*\(/,
        "Ветка запуска applyScreenBroadcast(true) не должна вызывать clearDesktopSource()",
      );

      const switchIdx = wsRs.indexOf('else if (cmd === "set_stream_source")');
      assert.ok(switchIdx > 0, "Обработчик set_stream_source должен существовать в ws.rs");
      const switchEndIdx = wsRs.indexOf('else if (cmd === "stop")', switchIdx);
      const switchBody = wsRs.slice(switchIdx, switchEndIdx > switchIdx ? switchEndIdx : switchIdx + 2400);

      assert.match(
        switchBody,
        /conn\.setDesktopSourceWithOptions\(\{\s*type:\s*"screen",\s*sourceId:\s*"-1"/,
        "set_stream_source обязан удерживать type: 'screen', sourceId: '-1' для сохранения активного XShm-захватчика",
      );
      assert.doesNotMatch(
        switchBody,
        /clearDesktopSource\s*\(/,
        "set_stream_source при горячей смене окна никогда не должен вызывать clearDesktopSource() (исключение сброса в nil)",
      );
      assert.match(
        switchBody,
        /conn\.setVideoBroadcast\(true\)/,
        "set_stream_source должен подтверждать setVideoBroadcast(true)",
      );
      assert.match(
        switchBody,
        /conn\.setTransportOptions\(\{[\s\S]*?keyframeInterval:\s*1000[\s\S]*?\}\)/,
        "set_stream_source должен запрашивать ускоренный ключевой кадр после смены источника",
      );
    });

    it("T779-1b: поведенческая модель WebRTC-моста исключает переход захватчика в nil при старте и многократной смене окон", () => {
      /** @type {Array<Record<string, any>>} */
      const desktopSourceCalls = [];
      /** @type {boolean[]} */
      const videoBroadcastCalls = [];
      /** @type {Array<Record<string, any>>} */
      const transportOptionCalls = [];
      let clearDesktopSourceCount = 0;
      /** @type {{ type: string, sourceId: string } | null} */
      let activeNativeCapturer = null;
      let capturerResetToNilCount = 0;

      // Эмуляция нативного Linux WebRTC ScreenCapturerX11 внутри discord_voice.node:
      // SelectSource принимает только kFullDesktopScreenId = "-1" и type = "screen".
      // Любой другой sourceId ("0", "{uuid}") или clearDesktopSource во время эфира сбрасывает захватчик в nil.
      const mockConn = {
        setDesktopSourceWithOptions(/** @type {Record<string, any>} */ opts) {
          desktopSourceCalls.push(opts);
          if (opts.type === "screen" && opts.sourceId === "-1") {
            activeNativeCapturer = { type: opts.type, sourceId: opts.sourceId };
          } else {
            activeNativeCapturer = null;
            capturerResetToNilCount += 1;
          }
        },
        clearDesktopSource() {
          clearDesktopSourceCount += 1;
          activeNativeCapturer = null;
        },
        setVideoBroadcast(/** @type {boolean} */ active) {
          videoBroadcastCalls.push(active);
        },
        setTransportOptions(/** @type {Record<string, any>} */ opts) {
          transportOptionCalls.push(opts);
        },
      };

      function runBridgeCommand(/** @type {{ cmd: string, broadcasting?: boolean, sourceType?: string, sourceId?: string }} */ msg) {
        if (msg.cmd === "set_stream") {
          const isBcast = Boolean(msg.broadcasting);
          if (!isBcast) {
            mockConn.clearDesktopSource();
            mockConn.setVideoBroadcast(false);
            return;
          }
          mockConn.setDesktopSourceWithOptions({
            type: "screen",
            sourceId: "-1",
            width: 1280,
            height: 720,
            frameRate: 60,
            useVideoHook: false,
            useGraphicsCapture: false,
            useQuartzCapturer: false,
            allowScreenCaptureKit: false,
            hdrCaptureMode: "never",
          });
          mockConn.setVideoBroadcast(true);
        } else if (msg.cmd === "set_stream_source") {
          mockConn.setDesktopSourceWithOptions({
            type: "screen",
            sourceId: "-1",
            width: 1280,
            height: 720,
            frameRate: 60,
            useVideoHook: false,
            useGraphicsCapture: false,
            useQuartzCapturer: false,
            allowScreenCaptureKit: false,
            hdrCaptureMode: "never",
          });
          mockConn.setVideoBroadcast(true);
          mockConn.setTransportOptions({ keyframeInterval: 1000 });
        }
      }

      // 1. Старт стрима
      runBridgeCommand({ cmd: "set_stream", broadcasting: true });
      assert.deepEqual(activeNativeCapturer, { type: "screen", sourceId: "-1" });
      assert.equal(capturerResetToNilCount, 0);
      assert.equal(clearDesktopSourceCount, 0);

      // 2. Серия горячих переключений: окно с UUID -> другое окно -> полный экран
      for (const src of [
        { sourceType: "window", sourceId: "{6ad0aefd-103b-4fa1-b650-3e016c818a00}" },
        { sourceType: "window", sourceId: "{11223344-5566-7788-99aa-bbccddeeff00}" },
        { sourceType: "screen", sourceId: "screen:0" },
      ]) {
        runBridgeCommand({ cmd: "set_stream_source", ...src });
        assert.deepEqual(
          activeNativeCapturer,
          { type: "screen", sourceId: "-1" },
          "Захватчик обязан оставаться активным с sourceId: '-1', type: 'screen'",
        );
        assert.equal(capturerResetToNilCount, 0, "Захватчик ни разу не должен сбрасываться в nil");
        assert.equal(clearDesktopSourceCount, 0, "clearDesktopSource не должен вызываться при смене окна");
      }

      assert.equal(desktopSourceCalls.length, 4);
      for (const call of desktopSourceCalls) {
        assert.equal(call.type, "screen");
        assert.equal(call.sourceId, "-1");
      }
      assert.equal(transportOptionCalls.length, 3);

      // 3. Остановка стрима — только здесь захватчик штатно освобождается
      runBridgeCommand({ cmd: "set_stream", broadcasting: false });
      assert.equal(clearDesktopSourceCount, 1);
      assert.equal(activeNativeCapturer, null);
      assert.equal(videoBroadcastCalls.at(-1), false);
    });
  });

  describe("2. Жизненный цикл Stage Viewer и PIP: минимизация в PIP vs полное прекращение стрима", () => {
    it("T779-2a: статический контракт Stage Viewer и PIP разграничивает свертывание в PIP (stopStream=false) и остановку стрима (stopStream=true)", () => {
      const fnIdx = appJs.indexOf("function openLiveStreamStageViewer(");
      assert.ok(fnIdx > 0, "openLiveStreamStageViewer должна существовать в app.js");
      const fnBody = appJs.slice(fnIdx, fnIdx + 42000);

      // Кнопки управления для стримера (isSelfStream)
      assert.match(fnBody, /changeSourceBtn\.id\s*=\s*"stage-change-source-btn"/);
      assert.match(fnBody, /minimizeStageBtn\.id\s*=\s*"stage-minimize-viewer-btn"/);
      assert.match(fnBody, /stopStageBtn\.id\s*=\s*"stage-stop-stream-btn"/);
      assert.match(fnBody, /closeStageBtn\.id\s*=\s*"stage-close-viewer-btn"/);

      // Minimize и крестик ✕ у стримера вызывают closeStageViewer(false) (свернуть в PIP, стрим продолжается)
      assert.match(
        fnBody,
        /minimizeStageBtn\.addEventListener\("click",\s*\(\)\s*=>\s*\{\s*closeStageViewer\(false\);\s*\}\)/,
        "Кнопка 'Свернуть 🗕' должна вызывать closeStageViewer(false)",
      );
      assert.match(
        fnBody,
        /closeStageBtn\.textContent\s*=\s*"✕"[\s\S]{0,400}?closeStageBtn\.addEventListener\("click",\s*\(\)\s*=>\s*\{\s*closeStageViewer\(false\);\s*\}\)/,
        "Крестик закрытия Stage Viewer у стримера должен сворачивать окно в PIP (closeStageViewer(false)), а не обрывать стрим",
      );

      // Кнопка 'Прекратить стрим' вызывает closeStageViewer(true)
      assert.match(
        fnBody,
        /stopStageBtn\.textContent\s*=\s*"Прекратить стрим"[\s\S]{0,400}?stopStageBtn\.addEventListener\("click",\s*\(\)\s*=>\s*\{\s*closeStageViewer\(true\);\s*\}\)/,
        "Кнопка 'Прекратить стрим' должна вызывать closeStageViewer(true)",
      );

      // В closeStageViewer: при !stopStream вызывается syncFloatingStreamPipPreview(), а при stopStream=true удаляется PIP и вызывается voice_stream_stop
      const closeIdx = fnBody.indexOf("const closeStageViewer = (stopStream = false) =>");
      assert.ok(closeIdx > 0, "closeStageViewer должна быть определена в openLiveStreamStageViewer");
      const closeBody = fnBody.slice(closeIdx, closeIdx + 3500);

      assert.match(
        closeBody,
        /if\s*\(stopStream\s*&&\s*activeVoiceConnection\)[\s\S]*?document\.getElementById\("live-stream-pip-preview"\)\?\.remove\(\)[\s\S]*?invoke\("voice_stream_stop"/,
        "При stopStream=true должны удаляться #live-stream-pip-preview и вызываться voice_stream_stop",
      );
      assert.match(
        closeBody,
        /else\s+if\s*\(!stopStream\)\s*\{[\s\S]{0,250}?syncFloatingStreamPipPreview\(\);\s*\}/,
        "При stopStream=false должен вызываться syncFloatingStreamPipPreview() для показа PIP",
      );
    });

    it("T779-2b: поведенческий тест жизненного цикла Stage Viewer <-> PIP: минимизация сохраняет стрим и открывает PIP, остановка уничтожает оба элемента", () => {
      /** @type {Map<string, any>} */
      const domById = new Map();
      /** @type {Array<{ cmd: string, args: any }>} */
      const ipcCalls = [];

      /**
       * @type {{
       *   guildId: string,
       *   channelId: string,
       *   channelName: string,
       *   screenShareOn: boolean,
       *   hideStreamPreview: boolean,
       *   streamSourceId: string,
       *   streamSourceName: string,
       *   activeStreamKey: string | null,
       * }}
       */
      const activeVoiceConnection = {
        guildId: "1001",
        channelId: "2002",
        channelName: "Основной",
        screenShareOn: true,
        hideStreamPreview: false,
        streamSourceId: "windows:{6ad0aefd-103b-4fa1-b650-3e016c818a00}:YouTube - Brave",
        streamSourceName: "YouTube - Brave",
        activeStreamKey: "guild:1001:2002:9001",
      };

      function syncFloatingStreamPipPreview() {
        domById.delete("live-stream-pip-preview");
        if (!activeVoiceConnection.screenShareOn || activeVoiceConnection.hideStreamPreview) {
          return null;
        }
        const pip = {
          id: "live-stream-pip-preview",
          sourceId: activeVoiceConnection.streamSourceId,
          sourceName: activeVoiceConnection.streamSourceName,
          expandToStage: () => openStageViewer({ isSelfStream: true }),
          stopFromPip: () => {
            activeVoiceConnection.screenShareOn = false;
            activeVoiceConnection.hideStreamPreview = true;
            domById.delete("live-stream-stage-viewer");
            domById.delete("live-stream-pip-preview");
            ipcCalls.push({
              cmd: "voice_stream_stop",
              args: {
                stream_key: activeVoiceConnection.activeStreamKey,
                streamKey: activeVoiceConnection.activeStreamKey,
              },
            });
            activeVoiceConnection.activeStreamKey = null;
          },
        };
        domById.set("live-stream-pip-preview", pip);
        return pip;
      }

      function openStageViewer(/** @type {{ isSelfStream: boolean }} */ { isSelfStream }) {
        domById.delete("live-stream-stage-viewer");
        const closeStageViewer = (stopStream = false) => {
          domById.delete("live-stream-stage-viewer");
          if (isSelfStream) {
            if (stopStream) {
              activeVoiceConnection.screenShareOn = false;
              activeVoiceConnection.hideStreamPreview = true;
              domById.delete("live-stream-pip-preview");
              domById.delete("live-stream-stage-viewer");
              ipcCalls.push({
                cmd: "voice_stream_stop",
                args: {
                  stream_key: activeVoiceConnection.activeStreamKey,
                  streamKey: activeVoiceConnection.activeStreamKey,
                },
              });
              activeVoiceConnection.activeStreamKey = null;
            } else {
              syncFloatingStreamPipPreview();
            }
          }
        };

        const stage = {
          id: "live-stream-stage-viewer",
          clickMinimizeBtn: () => closeStageViewer(false),
          clickCloseCrossBtn: () => closeStageViewer(false),
          pressEscape: () => closeStageViewer(false),
          clickStopStreamBtn: () => closeStageViewer(true),
        };
        domById.set("live-stream-stage-viewer", stage);
        return stage;
      }

      // 1. Открываем Stage Viewer собственного стрима
      const stage1 = openStageViewer({ isSelfStream: true });
      assert.equal(domById.has("live-stream-stage-viewer"), true);

      // 2. Нажимаем крестик '✕' (или 'Свернуть 🗕') -> Stage Viewer закрывается, появляется PIP, стрим НЕ останавливается
      stage1.clickCloseCrossBtn();
      assert.equal(domById.has("live-stream-stage-viewer"), false, "Stage Viewer должен скрыться при минимизации");
      assert.equal(domById.has("live-stream-pip-preview"), true, "Окно PIP обязано остаться/появиться при минимизации");
      assert.equal(activeVoiceConnection.screenShareOn, true, "screenShareOn обязан оставаться true при минимизации в PIP");
      assert.equal(ipcCalls.length, 0, "voice_stream_stop не должен вызываться при минимизации");

      // 3. Разворачиваем обратно из PIP и проверяем клавишу Escape (тоже сворачивает в PIP)
      domById.get("live-stream-pip-preview").expandToStage();
      assert.equal(domById.has("live-stream-stage-viewer"), true);
      domById.get("live-stream-stage-viewer").pressEscape();
      assert.equal(domById.has("live-stream-stage-viewer"), false);
      assert.equal(domById.has("live-stream-pip-preview"), true);
      assert.equal(ipcCalls.length, 0);

      // 4. Разворачиваем Stage Viewer и нажимаем 'Прекратить стрим' (stopStageBtn) -> полное прекращение
      domById.get("live-stream-pip-preview").expandToStage();
      domById.get("live-stream-stage-viewer").clickStopStreamBtn();
      assert.equal(domById.has("live-stream-stage-viewer"), false, "#live-stream-stage-viewer обязан быть удалён");
      assert.equal(domById.has("live-stream-pip-preview"), false, "#live-stream-pip-preview обязан быть удалён");
      assert.equal(activeVoiceConnection.screenShareOn, false, "screenShareOn обязан стать false");
      assert.equal(activeVoiceConnection.hideStreamPreview, true, "hideStreamPreview обязан стать true");
      assert.equal(ipcCalls.length, 1, "voice_stream_stop должен быть вызван ровно 1 раз");
      assert.ok(ipcCalls[0], "Первый вызов IPC должен существовать");
      assert.equal(ipcCalls[0]?.cmd, "voice_stream_stop");
      assert.equal(ipcCalls[0]?.args?.streamKey, "guild:1001:2002:9001");
    });
  });

  describe("3. Горячая смена окна через /dev/shm/discord_capture_target.txt", () => {
    it("T779-3a: сквозной контракт app.js -> data.rs -> main.rs -> ws.rs синхронизирует /dev/shm/discord_capture_target.txt и KWin CaptureWindow/CaptureWorkspace", () => {
      // 1. app.js switchLiveStreamSource вызывает set_capture_source без разрыва соединения
      const switchFnIdx = appJs.indexOf("function switchLiveStreamSource(");
      assert.ok(switchFnIdx > 0, "switchLiveStreamSource должна быть определена в app.js");
      const switchFnBody = appJs.slice(switchFnIdx, switchFnIdx + 900);
      assert.match(switchFnBody, /kind:\s*"set_capture_source"/);
      assert.match(switchFnBody, /deviceId:\s*picked\.id/);
      assert.doesNotMatch(switchFnBody, /voice_stream_stop|voice_disconnect/);

      // 2. data.rs set_capture_source пишет в /dev/shm/discord_capture_target.txt и /dev/shm/discord_capture_kick.bin
      const dataIdx = dataRs.indexOf('if kind == "set_capture_source"');
      assert.ok(dataIdx > 0, "set_capture_source должен быть в data.rs");
      const dataEnd = dataRs.indexOf('    if kind == "list_capture_windows"', dataIdx);
      assert.ok(dataEnd > dataIdx, "set_capture_source must have its expected closing boundary");
      const dataBody = dataRs.slice(dataIdx, dataEnd);
      assert.match(
        dataBody,
        /std::fs::write\("\/dev\/shm\/discord_capture_target\.txt",\s*target_handle\.as_bytes\(\)\)/,
        "data.rs set_capture_source обязан записывать target_handle в /dev/shm/discord_capture_target.txt",
      );
      assert.match(
        dataBody,
        /std::fs::write\("\/dev\/shm\/discord_capture_kick\.bin",\s*&\[1u8\]\)/,
        "data.rs set_capture_source обязан писать сигнал в /dev/shm/discord_capture_kick.bin",
      );
      assert.match(
        dataBody,
        /chord_core::ws::send_native_stream_cmd\(&ucmd\)/,
        "data.rs set_capture_source обязан отправлять команду set_stream_source в нативный мост",
      );

      // 3. main.rs voice_stream_start также инициализирует /dev/shm/discord_capture_target.txt
      const startIdx = mainRs.indexOf("async fn voice_stream_start");
      assert.ok(startIdx > 0, "voice_stream_start должна быть в main.rs");
      const startBody = mainRs.slice(startIdx, mainRs.indexOf("\n}", startIdx) + 2);
      assert.match(
        startBody,
        /std::fs::write\("\/dev\/shm\/discord_capture_target\.txt",\s*clean_src\.as_bytes\(\)\)/,
        "voice_stream_start должна записывать начальный источник в /dev/shm/discord_capture_target.txt",
      );

      // 4. ws.rs capture_kwin_dbus_to_ring читает /dev/shm/discord_capture_target.txt и выбирает CaptureWindow vs CaptureWorkspace
      const kwinIdx = wsRs.indexOf("static int capture_kwin_dbus_to_ring(");
      assert.ok(kwinIdx > 0, "capture_kwin_dbus_to_ring должна быть в ws.rs");
      const kwinBody = wsRs.slice(kwinIdx, kwinIdx + 2000);
      assert.match(
        kwinBody,
        /fopen\("\/dev\/shm\/discord_capture_target\.txt",\s*"r"\)/,
        "C-хук capture_kwin_dbus_to_ring обязан читать /dev/shm/discord_capture_target.txt",
      );
      assert.match(
        kwinBody,
        /if\s*\(target_buf\[0\]\s*==\s*'\{'\)\s*\{\s*method\s*=\s*"CaptureWindow";/,
        "При UUID окна '{...}' C-хук обязан вызывать метод KWin ScreenShot2 CaptureWindow",
      );
      assert.match(
        kwinBody,
        /const\s+char\*\s+method\s*=\s*"CaptureWorkspace";/,
        "По умолчанию (для экрана) C-хук обязан использовать метод CaptureWorkspace",
      );
    });

    it("T779-3b: файловый тест горячей смены цели в /dev/shm/discord_capture_target.txt (окно {uuid} <-> экран)", () => {
      // Сохраняем текущее состояние /dev/shm файлов, чтобы не затронуть живое окружение
      const hadTargetFile = fs.existsSync(CAPTURE_TARGET_SHM_PATH);
      const prevTargetContent = hadTargetFile ? fs.readFileSync(CAPTURE_TARGET_SHM_PATH) : null;
      const hadKickFile = fs.existsSync(CAPTURE_KICK_SHM_PATH);
      const prevKickContent = hadKickFile ? fs.readFileSync(CAPTURE_KICK_SHM_PATH) : null;

      /**
       * Точная копия парсера `set_capture_source` из `rust/gui-spike/src/data.rs`.
       * @param {string} rawInput
       */
      function applyCaptureSourceToShm(rawInput) {
        const raw = rawInput.trim();
        let targetKind = "screen";
        let targetHandle = "0";
        let targetLabel = "";
        if (raw.length > 0) {
          if (raw.startsWith("{")) {
            targetHandle = raw;
            targetKind = "window";
          } else if (raw.startsWith("screen:")) {
            targetHandle = raw;
            targetKind = "screen";
          } else if (raw.includes(":")) {
            const firstColon = raw.indexOf(":");
            const secondColon = raw.indexOf(":", firstColon + 1);
            const tab = raw.slice(0, firstColon);
            if (tab === "windows" || tab === "window") {
              targetKind = "window";
            }
            if (secondColon >= 0) {
              targetHandle = raw.slice(firstColon + 1, secondColon).trim();
              targetLabel = raw.slice(secondColon + 1).trim();
            } else {
              targetHandle = raw.slice(firstColon + 1).trim();
            }
          } else {
            targetHandle = raw;
            if (raw.startsWith("win")) {
              targetKind = "window";
            }
          }
        }
        if (targetHandle.startsWith("{")) {
          targetKind = "window";
        }
        fs.writeFileSync(CAPTURE_TARGET_SHM_PATH, Buffer.from(targetHandle, "utf8"));
        fs.writeFileSync(CAPTURE_KICK_SHM_PATH, Buffer.from([1]));
        return { targetKind, targetHandle, targetLabel };
      }

      /**
       * Точная модель C-хука `capture_kwin_dbus_to_ring` из `rust/crates/chord-core/src/ws.rs`.
       */
      function readKwinTargetFromShm() {
        const raw = fs.readFileSync(CAPTURE_TARGET_SHM_PATH, "utf8");
        const line = raw.split(/[\r\n]/)[0] || "";
        if (line.startsWith("{")) {
          return { dbusMethod: "CaptureWindow", windowUuid: line };
        }
        return { dbusMethod: "CaptureWorkspace", windowUuid: null };
      }

      try {
        // Шаг 1: Горячее переключение на окно браузера с префиксом вкладки `windows:{uuid}:Title`
        const win1 = applyCaptureSourceToShm("windows:{6ad0aefd-103b-4fa1-b650-3e016c818a00}:YouTube - Brave");
        assert.equal(win1.targetKind, "window");
        assert.equal(win1.targetHandle, "{6ad0aefd-103b-4fa1-b650-3e016c818a00}");
        assert.equal(win1.targetLabel, "YouTube - Brave");
        assert.equal(
          fs.readFileSync(CAPTURE_TARGET_SHM_PATH, "utf8"),
          "{6ad0aefd-103b-4fa1-b650-3e016c818a00}",
        );
        assert.deepEqual(fs.readFileSync(CAPTURE_KICK_SHM_PATH), Buffer.from([1]));
        assert.deepEqual(readKwinTargetFromShm(), {
          dbusMethod: "CaptureWindow",
          windowUuid: "{6ad0aefd-103b-4fa1-b650-3e016c818a00}",
        });

        // Шаг 2: Горячее переключение на другое окно по прямому KWin UUID `{...}` без префикса
        const win2 = applyCaptureSourceToShm("{9f8e7d6c-5b4a-3210-fedc-ba9876543210}");
        assert.equal(win2.targetKind, "window");
        assert.equal(win2.targetHandle, "{9f8e7d6c-5b4a-3210-fedc-ba9876543210}");
        assert.deepEqual(readKwinTargetFromShm(), {
          dbusMethod: "CaptureWindow",
          windowUuid: "{9f8e7d6c-5b4a-3210-fedc-ba9876543210}",
        });

        // Шаг 3: Горячее переключение обратно на полный экран `screen:0`
        const scr = applyCaptureSourceToShm("screen:0");
        assert.equal(scr.targetKind, "screen");
        assert.equal(scr.targetHandle, "screen:0");
        assert.deepEqual(readKwinTargetFromShm(), {
          dbusMethod: "CaptureWorkspace",
          windowUuid: null,
        });
      } finally {
        if (hadTargetFile && prevTargetContent !== null) {
          fs.writeFileSync(CAPTURE_TARGET_SHM_PATH, prevTargetContent);
        } else {
          try {
            fs.unlinkSync(CAPTURE_TARGET_SHM_PATH);
          } catch {}
        }
        if (hadKickFile && prevKickContent !== null) {
          fs.writeFileSync(CAPTURE_KICK_SHM_PATH, prevKickContent);
        } else {
          try {
            fs.unlinkSync(CAPTURE_KICK_SHM_PATH);
          } catch {}
        }
      }
    });
  });
});
