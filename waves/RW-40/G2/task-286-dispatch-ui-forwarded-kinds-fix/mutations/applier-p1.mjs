import { readFileSync, writeFileSync } from "node:fs";
const p = "rust/gui-spike/ui/features/dispatch-model.js";
let s = readFileSync(p, "utf8");
const tail = '  "VOICE_STATE_UPDATE",\n  "MESSAGE_REACTION_REMOVE",\n]);';
if (!s.includes(tail)) throw new Error("p1 tail anchor missing");
s = s.replace(tail, '  "VOICE_STATE_UPDATE",\n]);');
s = s.replace('  "CHANNEL_INFO",\n', '  "MESSAGE_REACTION_REMOVE",\n  "CHANNEL_INFO",\n');
writeFileSync(p, s);
