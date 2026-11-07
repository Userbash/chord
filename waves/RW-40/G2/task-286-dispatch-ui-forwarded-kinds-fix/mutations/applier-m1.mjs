import { readFileSync, writeFileSync } from "node:fs";
const p = "rust/gui-spike/ui/features/dispatch-model.js";
let s = readFileSync(p, "utf8");
const from = '  "CHANNEL_CREATE",\n';
if (!s.includes(from)) throw new Error("m1 anchor missing");
writeFileSync(p, s.replace(from, ""));
