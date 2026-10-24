// @ts-check
// W39f: проверка машиночитаемого вывода re-extra.json (структура, уникальность id,
// соответствие counts, наличие каждого id в RE-EXTRA.md). Только чтение артефактов.
import { readFileSync } from "node:fs";

const root = new URL("../../DESIGN/", import.meta.url);
const jsonPath = new URL("re-extra.json", root);
const mdPath = new URL("RE-EXTRA.md", root);
const data = JSON.parse(readFileSync(jsonPath, "utf8"));
const md = readFileSync(mdPath, "utf8");

const errors = [];
const ids = [];
let total = 0;
for (const [key, section] of Object.entries(data.sections)) {
  const facts = section.facts ?? [];
  total += facts.length;
  if (data.counts?.[key] !== facts.length) {
    errors.push(`counts.${key}=${data.counts?.[key]} != facts=${facts.length}`);
  }
  for (const fact of facts) {
    if (!/^[A-Z]-\d{2}$/.test(fact.id)) errors.push(`bad id ${fact.id}`);
    if (!["D", "M", "T", "A", "D/T", "D/A", "M/A", "D/M"].includes(fact.code)) {
      errors.push(`bad code ${fact.id}: ${fact.code}`);
    }
    if (!fact.source || !fact.method) errors.push(`no provenance ${fact.id}`);
    if (!md.includes(`| ${fact.id} |`)) errors.push(`id ${fact.id} missing in RE-EXTRA.md`);
    ids.push(fact.id);
  }
}
if (new Set(ids).size !== ids.length) errors.push("duplicate ids");
if (data.counts?.total !== total) errors.push(`counts.total=${data.counts?.total} != ${total}`);
for (const gap of data.gaps ?? []) {
  if (!/^G-\d{2}$/.test(gap.id)) errors.push(`bad gap id ${gap.id}`);
  if (!md.includes(`| ${gap.id} |`)) errors.push(`gap ${gap.id} missing in RE-EXTRA.md`);
}

if (errors.length) {
  console.error("FAIL");
  for (const e of errors) console.error(" -", e);
  process.exit(1);
}
console.log(`ok: ${total} фактов, ${data.gaps.length} пробелов, ${Object.keys(data.sections).length} разделов; ids уникальны и есть в MD`);
for (const [key, section] of Object.entries(data.sections)) {
  console.log(`  ${key}: ${section.facts.length}`);
}
