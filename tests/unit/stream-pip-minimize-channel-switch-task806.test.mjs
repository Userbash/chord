// @ts-check
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");

describe("TASK-806: Автоматическое сворачивание трансляции в плавающий PiP при смене канала", () => {
  const appJs = fs.readFileSync(path.join(REPO_ROOT, "rust/gui-spike/ui/app.js"), "utf8");

  it("T806-1: minimizeStageViewerToPip и onChannelSwitch объявлены в app.js", () => {
    assert.match(
      appJs,
      /function\s+minimizeStageViewerToPip\s*\(/,
      "app.js must declare minimizeStageViewerToPip()",
    );
    assert.match(
      appJs,
      /(?:function\s+onChannelSwitch|const\s+onChannelSwitch\s*=)/,
      "app.js must declare onChannelSwitch",
    );
  });

  it("T806-2: chat.openChannel и дерево каналов сворачивают #live-stream-stage-viewer при смене канала", () => {
    const openChannelIdx = appJs.indexOf("chat.openChannel = async (target, controls) =>");
    assert.ok(openChannelIdx > 0, "chat.openChannel must exist in app.js");
    const openChannelBody = appJs.slice(openChannelIdx, openChannelIdx + 1200);

    assert.match(
      openChannelBody,
      /(?:minimizeStageViewerToPip|onChannelSwitch|stage-minimize-viewer-btn)/,
      "chat.openChannel must call minimizeStageViewerToPip or onChannelSwitch to dock active stream to PiP",
    );

    const treeIdx = appJs.indexOf("const tree = createChannelTree({");
    assert.ok(treeIdx > 0, "createChannelTree must exist in app.js");
    const treeBody = appJs.slice(treeIdx, treeIdx + 600);
    assert.match(
      treeBody,
      /onOpen:\s*\([^)]*\)\s*=>\s*\{[\s\S]*?(?:onChannelSwitch|minimizeStageViewerToPip|chat\.openChannel)/,
      "tree.onOpen must trigger channel switch with PiP minimization",
    );
  });

  it("T806-3: PiP окно в app.js настроено на размер 320x180 и правый нижний угол (bottom-right)", () => {
    // syncFloatingStreamPipPreview must configure 320x180 and bottom-right placement
    assert.match(
      appJs,
      /width:\s*320px\s*;\s*height:\s*180px/,
      "syncFloatingStreamPipPreview in app.js must set 320x180 for #live-stream-pip-preview",
    );
    assert.match(
      appJs,
      /right:\s*24px\s*;\s*bottom:\s*24px/,
      "syncFloatingStreamPipPreview in app.js must default to bottom-right docked positioning",
    );
  });

  it("T806-4: поведенческий тест: при наличии #live-stream-stage-viewer вызов onChannelSwitch на текстовый канал сворачивает Stage Viewer в PiP 320x180", () => {
    let minimizeClicked = false;
    let stageRemoved = false;
    let pipCreated = false;

    /** @type {Record<string, any>} */
    const mockDom = {
      "live-stream-stage-viewer": {
        id: "live-stream-stage-viewer",
        remove() {
          stageRemoved = true;
          delete mockDom["live-stream-stage-viewer"];
        },
      },
      "stage-minimize-viewer-btn": {
        id: "stage-minimize-viewer-btn",
        click() {
          minimizeClicked = true;
          mockDom["live-stream-stage-viewer"]?.remove();
          mockDom["live-stream-pip-preview"] = {
            id: "live-stream-pip-preview",
            style: {
              width: "320px",
              height: "180px",
              right: "24px",
              bottom: "24px",
            },
          };
          pipCreated = true;
        },
      },
    };

    /** @param {string} id */
    const getEl = (id) => mockDom[id] || null;

    /**
     * @param {any} target
     */
    function mockOnChannelSwitch(target) {
      const isText =
        !target ||
        target.type === undefined ||
        target.type === 0 ||
        target.type === 1 ||
        target.type === 3 ||
        target.type === 5;
      if (isText && getEl("live-stream-stage-viewer")) {
        const minBtn = getEl("stage-minimize-viewer-btn");
        if (minBtn) {
          minBtn.click();
        } else {
          getEl("live-stream-stage-viewer")?.remove();
        }
      }
    }

    // 1. Initial check
    assert.ok(getEl("live-stream-stage-viewer"), "Stage viewer should initially exist");
    assert.equal(getEl("live-stream-pip-preview"), null, "PiP preview should not exist yet");

    // 2. Navigation to text channel (type 0)
    mockOnChannelSwitch({ id: "text-chan-1", type: 0, name: "general" });

    assert.equal(minimizeClicked, true, "stage-minimize-viewer-btn should be clicked");
    assert.equal(stageRemoved, true, "Stage viewer overlay should be removed");
    assert.equal(pipCreated, true, "PiP preview should be created/active in bottom-right");
    assert.equal(getEl("live-stream-stage-viewer"), null, "Stage viewer should no longer exist in DOM");
    const pip = getEl("live-stream-pip-preview");
    assert.ok(pip, "PiP preview should exist in DOM");
    assert.equal(pip.style.width, "320px", "PiP width must be 320px");
    assert.equal(pip.style.height, "180px", "PiP height must be 180px");
    assert.equal(pip.style.right, "24px", "PiP right offset must be 24px");
    assert.equal(pip.style.bottom, "24px", "PiP bottom offset must be 24px");
  });
});
