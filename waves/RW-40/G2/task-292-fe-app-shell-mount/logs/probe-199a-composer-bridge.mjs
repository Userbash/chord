import { createComposerPopoversBridge } from "/home/user/orca/workspaces/project/feature-task-292-fe-app-shell-mount/rust/gui-spike/ui/features/composer-popovers-bridge.js";

const ipcCalls = [];
const selected = [];
const played = [];

const bridge = createComposerPopoversBridge({
  onDispatchIpc: (command, payload) => {
    ipcCalls.push([command, payload]);
    return Promise.resolve("ipc");
  },
  commands: [{ id: "cmd-1", name: "shrug", description: "shrug" }],
  sounds: [{ id: "s1", name: "Boom" }],
  volume: 42,
  onSelectSlashCommand: (cmd) => selected.push(cmd.name),
  onPlaySoundboardSound: (id, volume) => played.push([id, volume]),
});

bridge.updateDraft("/shrug");
const picked = bridge.selectSlashCommand({ id: "cmd-1", name: "shrug" });
bridge.playSoundboardSound("s1");
bridge.setSoundboardVolume(120);

console.log("ipcCalls:", JSON.stringify(ipcCalls));
console.log("selected:", JSON.stringify(selected));
console.log("played:", JSON.stringify(played));
console.log("picked:", JSON.stringify(picked));
console.log("state:", JSON.stringify(bridge.getState()));
console.log("VERDICT:", ipcCalls.length === 0 ? "onDispatchIpc NEVER called (dead option)" : "onDispatchIpc used");
