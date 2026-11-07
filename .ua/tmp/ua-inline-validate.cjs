const fs = require("fs");
const g = JSON.parse(fs.readFileSync("/home/sanya/orca/project/.ua/knowledge-graph.json", "utf8"));
const issues = [];
const ids = new Set();
const fileTypes = new Set(["file","config","document","service","pipeline","table","schema","resource","endpoint"]);
for (const n of g.nodes || []) {
  if (!n.id || !n.type || !n.name) issues.push("node missing id/type/name: " + JSON.stringify(n).slice(0,120));
  if (ids.has(n.id)) issues.push("duplicate node id: " + n.id);
  ids.add(n.id);
  if (!n.summary || !String(n.summary).trim()) issues.push("empty summary: " + n.id);
  if (!Array.isArray(n.tags) || n.tags.length === 0) issues.push("empty tags: " + n.id);
}
for (const e of g.edges || []) {
  if (!ids.has(e.source)) issues.push("dangling edge source: " + e.source);
  if (!ids.has(e.target)) issues.push("dangling edge target: " + e.target);
}
const layered = new Map();
for (const l of g.layers || []) {
  for (const id of l.nodeIds || []) {
    if (!ids.has(id)) issues.push("layer " + l.id + " references missing node " + id);
    layered.set(id, (layered.get(id) || 0) + 1);
  }
}
for (const n of g.nodes || []) {
  if (fileTypes.has(n.type) && (layered.get(n.id) || 0) !== 1) {
    issues.push("file-level node " + n.id + " layer count = " + (layered.get(n.id) || 0));
  }
}
console.log(JSON.stringify({ nodes: g.nodes.length, edges: g.edges.length, layers: (g.layers||[]).length, tour: (g.tour||[]).length, issues }, null, 2));
if (issues.length) process.exit(2);
