import { readFileSync, writeFileSync } from "node:fs";
const p = "rust/gui-spike/ui/features/dispatch-model.js";
let s = readFileSync(p, "utf8");
const from = '  "MESSAGE_REACTION_REMOVE",\n]);';
if (!s.includes(from)) throw new Error("m3 anchor missing");
writeFileSync(p, s.replace(from, "]);"));
