#!/usr/bin/env node
// @ts-check
// Phase 1: structural analysis of the project graph for tour design.
import fs from "node:fs";

const [, , inPath, outPath] = process.argv;
if (!inPath || !outPath) {
  console.error("usage: node ua-tour-analyze.js <input.json> <results.json>");
  process.exit(1);
}

try {
  const data = JSON.parse(fs.readFileSync(inPath, "utf8"));
  const nodes = Array.isArray(data.nodes) ? data.nodes : [];
  const edges = Array.isArray(data.edges) ? data.edges : [];
  const layers = Array.isArray(data.layers) ? data.layers : [];

  const byId = new Map(nodes.map((n) => [n.id, n]));
  const fanIn = new Map(nodes.map((n) => [n.id, 0]));
  const fanOut = new Map(nodes.map((n) => [n.id, 0]));

  for (const e of edges) {
    if (byId.has(e.target)) fanIn.set(e.target, (fanIn.get(e.target) || 0) + 1);
    if (byId.has(e.source)) fanOut.set(e.source, (fanOut.get(e.source) || 0) + 1);
  }

  const rank = (m) =>
    [...m.entries()]
      .map(([id, v]) => ({ id, score: v, name: byId.get(id)?.name || id }))
      .sort((a, b) => b.score - a.score || a.id.localeCompare(b.id));

  const fanInRanking = rank(fanIn).slice(0, 20).map((r) => ({ id: r.id, fanIn: r.score, name: r.name }));
  const fanOutRanking = rank(fanOut).slice(0, 20).map((r) => ({ id: r.id, fanOut: r.score, name: r.name }));

  // C. Entry point candidates
  const CODE_NAMES = new Set([
    "index.ts", "index.js", "main.ts", "main.js", "app.ts", "app.js", "server.ts", "server.js",
    "mod.rs", "main.go", "main.py", "main.rs", "manage.py", "app.py", "wsgi.py", "asgi.py",
    "run.py", "__main__.py", "Application.java", "Main.java", "Program.cs", "config.ru",
    "index.php", "App.swift", "Application.kt", "main.cpp", "main.c",
  ]);
  const outVals = [...fanOut.values()].sort((a, b) => b - a);
  const inVals = [...fanIn.values()].sort((a, b) => a - b);
  const fanOutTop = outVals[Math.max(0, Math.ceil(outVals.length * 0.1) - 1)] ?? 0;
  const fanInBottom = inVals[Math.max(0, Math.ceil(inVals.length * 0.25) - 1)] ?? 0;

  const entryPointCandidates = [];
  for (const n of nodes) {
    let score = 0;
    const depth = String(n.filePath || "").split("/").length;
    if (n.type === "document") {
      if (n.filePath === "README.md") score += 5;
      else if (!String(n.filePath || "").includes("/") && String(n.filePath || "").endsWith(".md")) score += 2;
    } else if (n.type === "file") {
      if (CODE_NAMES.has(n.name)) score += 3;
      if (depth <= 2) score += 1;
      if ((fanOut.get(n.id) || 0) >= fanOutTop && fanOutTop > 0) score += 1;
      if ((fanIn.get(n.id) || 0) <= fanInBottom) score += 1;
    }
    if (score > 0) entryPointCandidates.push({ id: n.id, score, name: n.name, summary: n.summary || "" });
  }
  entryPointCandidates.sort((a, b) => b.score - a.score);

  // D. BFS from top code entry point following imports/calls forward.
  const codeEntry = entryPointCandidates.find((c) => byId.get(c.id)?.type === "file");
  const adj = new Map();
  for (const e of edges) {
    if (e.type !== "imports" && e.type !== "calls") continue;
    if (!adj.has(e.source)) adj.set(e.source, []);
    adj.get(e.source).push(e.target);
  }
  const order = [];
  const depthMap = {};
  let startNode = codeEntry?.id || nodes[0]?.id || null;
  if (startNode) {
    depthMap[startNode] = 0;
    order.push(startNode);
    const queue = [startNode];
    while (queue.length) {
      const cur = queue.shift();
      for (const nxt of adj.get(cur) || []) {
        if (!byId.has(nxt) || nxt in depthMap) continue;
        depthMap[nxt] = depthMap[cur] + 1;
        order.push(nxt);
        queue.push(nxt);
      }
    }
  }
  const byDepth = {};
  for (const [id, d] of Object.entries(depthMap)) {
    (byDepth[d] ||= []).push(id);
  }

  // E. Non-code inventory
  const nonCodeFiles = { documentation: [], infrastructure: [], data: [], config: [] };
  for (const n of nodes) {
    const rec = { id: n.id, name: n.name, summary: n.summary || "", type: n.type };
    if (n.type === "document") nonCodeFiles.documentation.push(rec);
    else if (["service", "pipeline", "resource"].includes(n.type)) nonCodeFiles.infrastructure.push(rec);
    else if (["table", "schema", "endpoint"].includes(n.type)) nonCodeFiles.data.push(rec);
    else if (n.type === "config") nonCodeFiles.config.push(rec);
  }

  // F. Tightly coupled clusters (bidirectional pairs, expand by 2+ links)
  const pairKey = (a, b) => (a < b ? a + "|" + b : b + "|" + a);
  const pairCount = new Map();
  const pairTypes = new Map();
  for (const e of edges) {
    const k = pairKey(e.source, e.target);
    pairCount.set(k, (pairCount.get(k) || 0) + 1);
    if (!pairTypes.has(k)) pairTypes.set(k, new Set());
    pairTypes.get(k).add(e.type);
  }
  const neighbors = new Map(nodes.map((n) => [n.id, new Set()]));
  for (const e of edges) {
    if (neighbors.has(e.source) && neighbors.has(e.target) && e.source !== e.target) {
      neighbors.get(e.source).add(e.target);
      neighbors.get(e.target).add(e.source);
    }
  }
  const seedPairs = [];
  for (const [k, c] of pairCount.entries()) {
    const [a, b] = k.split("|");
    if (!(neighbors.get(a)?.has(b) && neighbors.get(b)?.has(a))) continue;
    if (c >= 2) seedPairs.push([a, b, c]);
  }
  seedPairs.sort((x, y) => y[2] - x[2]);
  const seenClusters = new Set();
  const clusters = [];
  for (const [a, b] of seedPairs) {
    if (seenClusters.has(a) && seenClusters.has(b)) continue;
    const cluster = new Set([a, b]);
    let changed = true;
    while (changed && cluster.size < 5) {
      changed = false;
      for (const n of nodes) {
        if (cluster.has(n.id)) continue;
        let links = 0;
        for (const m of cluster) if (neighbors.get(n.id)?.has(m)) links++;
        if (links >= 2) {
          cluster.add(n.id);
          changed = true;
          if (cluster.size >= 5) break;
        }
      }
    }
    const arr = [...cluster];
    const key = arr.slice().sort().join(",");
    if (seenClusters.has(key)) continue;
    seenClusters.add(key);
    arr.forEach((x) => seenClusters.add(x));
    let edgeCount = 0;
    for (const e of edges) if (cluster.has(e.source) && cluster.has(e.target)) edgeCount++;
    clusters.push({ nodes: arr, edgeCount });
    if (clusters.length >= 10) break;
  }
  clusters.sort((x, y) => y.edgeCount - x.edgeCount);

  const nodeSummaryIndex = {};
  for (const n of nodes) {
    nodeSummaryIndex[n.id] = { name: n.name, type: n.type, summary: n.summary || "" };
  }

  const result = {
    scriptCompleted: true,
    entryPointCandidates: entryPointCandidates.slice(0, 5),
    fanInRanking,
    fanOutRanking,
    bfsTraversal: { startNode, order, depthMap, byDepth },
    nonCodeFiles,
    clusters,
    layers: { count: layers.length, list: layers.map((l) => ({ id: l.id, name: l.name, description: l.description })) },
    nodeSummaryIndex,
    totalNodes: nodes.length,
    totalEdges: edges.length,
  };
  fs.writeFileSync(outPath, JSON.stringify(result, null, 2));
  process.exit(0);
} catch (err) {
  console.error(err && err.stack ? err.stack : String(err));
  process.exit(1);
}
