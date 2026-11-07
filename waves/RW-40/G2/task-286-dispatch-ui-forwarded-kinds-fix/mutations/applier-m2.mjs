import { readFileSync, writeFileSync } from "node:fs";
const p = "rust/gui-spike/ui/features/dispatch-model.js";
let s = readFileSync(p, "utf8");
const from = '  "MESSAGE_DELETE",\n';
if (!s.includes(from)) throw new Error("m2 anchor missing");
writeFileSync(p, s.replace(from, '  "MESSAGE_DELETE_EXTRA",\n'));
