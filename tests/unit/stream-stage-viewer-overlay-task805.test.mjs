// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");

describe("TASK-805: Устранение оверлея «Подключение к видеопотоку…» у ведущего в Stage Viewer", () => {
  const appJs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/ui/app.js"), "utf8");

  it("T805-1: emptyNotice has pointer-events: none and display: none when isSelfStream is true", () => {
    const fnIdx = appJs.indexOf("function openLiveStreamStageViewer(");
    assert.ok(fnIdx > 0, "openLiveStreamStageViewer must exist in app.js");
    const fnEndIdx = appJs.indexOf("function renderLiveVoicePanel(", fnIdx);
    const fnBody = appJs.slice(fnIdx, fnEndIdx > 0 ? fnEndIdx : fnIdx + 45000);

    // emptyNotice must not intercept mouse clicks and be hidden on isSelfStream
    assert.match(
      fnBody,
      /emptyNotice\.style\.pointerEvents\s*=\s*["']none["']/,
      "emptyNotice must set pointerEvents = 'none' for isSelfStream",
    );
  });

  it("T805-2: if isSelfStream is true, emptyNotice is immediately hidden and cached or canvas frame is immediately shown", () => {
    const fnIdx = appJs.indexOf("function openLiveStreamStageViewer(");
    assert.ok(fnIdx > 0, "openLiveStreamStageViewer must exist in app.js");
    const fnEndIdx = appJs.indexOf("function renderLiveVoicePanel(", fnIdx);
    const fnBody = appJs.slice(fnIdx, fnEndIdx > 0 ? fnEndIdx : fnIdx + 45000);

    const renderIdx = fnBody.indexOf("const renderSelfStreamFallback =");
    assert.ok(renderIdx > 0, "renderSelfStreamFallback must exist in openLiveStreamStageViewer");
    const selfStreamSection = fnBody.slice(renderIdx - 200, renderIdx + 1500);

    assert.match(
      selfStreamSection,
      /emptyNotice\.style\.display\s*=\s*["']none["']/,
      "openLiveStreamStageViewer must immediately set emptyNotice.style.display = 'none' when isSelfStream",
    );

    // Check cachedLiveScreenDataUrl calls showFrameDataUrl
    assert.match(
      selfStreamSection,
      /showFrameDataUrl\(cachedLiveScreenDataUrl\)/,
      "must immediately call showFrameDataUrl(cachedLiveScreenDataUrl) if cachedLiveScreenDataUrl exists",
    );

    // Check fallback immediately calls drawLiveCaptureCanvasFrame and sets stageCanvas.style.display = 'block'
    assert.match(
      selfStreamSection,
      /drawLiveCaptureCanvasFrame\(stageCanvas,\s*srcId,\s*badgeTick,\s*true\)/,
      "must immediately call drawLiveCaptureCanvasFrame if cachedLiveScreenDataUrl not present",
    );
    assert.match(
      selfStreamSection,
      /stageCanvas\.style\.display\s*=\s*["']block["']/,
      "must set stageCanvas.style.display = 'block' when drawing live capture canvas frame",
    );
  });

  it("T805-3: immediately renders cachedLiveScreenDataUrl or /dev/shm/discord_live_preview_thumb.jpg around line 8960 for isSelfStream", () => {
    const fnIdx = appJs.indexOf("function openLiveStreamStageViewer(");
    assert.ok(fnIdx > 0, "openLiveStreamStageViewer must exist in app.js");
    const fnEndIdx = appJs.indexOf("function renderLiveVoicePanel(", fnIdx);
    const fnBody = appJs.slice(fnIdx, fnEndIdx > 0 ? fnEndIdx : fnIdx + 45000);

    const emptyNoticeIdx = fnBody.indexOf("const emptyNotice = document.createElement(\"div\");");
    assert.ok(emptyNoticeIdx > 0, "emptyNotice must exist in openLiveStreamStageViewer");
    const emptyNoticeSection = fnBody.slice(emptyNoticeIdx, emptyNoticeIdx + 1200);

    assert.match(
      emptyNoticeSection,
      /stageImg\.src\s*=\s*cachedLiveScreenDataUrl/,
      "must set stageImg.src = cachedLiveScreenDataUrl when cachedLiveScreenDataUrl is present",
    );
    assert.match(
      emptyNoticeSection,
      /stageImg\.src\s*=\s*["']\/dev\/shm\/discord_live_preview_thumb\.jpg["']/,
      "must set stageImg.src = '/dev/shm/discord_live_preview_thumb.jpg' fallback for isSelfStream",
    );
  });
});
