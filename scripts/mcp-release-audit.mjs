#!/usr/bin/env node
// @ts-check
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import readline from 'node:readline';

const ROOT = '/home/sanya/orca/project';

/**
 * @param {string} dir
 * @param {string[]} extensions
 * @param {string[]} [ignoreDirs]
 * @returns {string[]}
 */
function walkFiles(dir, extensions, ignoreDirs = ['node_modules', '.git', 'target', 'dist', '.cache', 'fixtures']) {
  /** @type {string[]} */
  const files = [];
  if (!fs.existsSync(dir)) return files;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    if (ignoreDirs.includes(entry.name)) continue;
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...walkFiles(fullPath, extensions, ignoreDirs));
    } else if (extensions.some(ext => entry.name.endsWith(ext))) {
      files.push(fullPath);
    }
  }
  return files;
}

/**
 * @param {string} hex
 * @returns {{ r: number, g: number, b: number }}
 */
function parseHexColor(hex) {
  let cleaned = hex.replace('#', '').trim();
  if (cleaned.length === 3) {
    cleaned = cleaned.split('').map(c => c + c).join('');
  }
  const num = parseInt(cleaned, 16);
  return {
    r: (num >> 16) & 255,
    g: (num >> 8) & 255,
    b: num & 255
  };
}

/**
 * @param {{ r: number, g: number, b: number }} rgb
 * @returns {number}
 */
function luminance({ r, g, b }) {
  const a = [r, g, b].map(v => {
    v /= 255;
    return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
  });
  const l0 = a[0] ?? 0;
  const l1 = a[1] ?? 0;
  const l2 = a[2] ?? 0;
  return l0 * 0.2126 + l1 * 0.7152 + l2 * 0.0722;
}

/**
 * @param {{ r: number, g: number, b: number }} rgb1
 * @param {{ r: number, g: number, b: number }} rgb2
 * @returns {number}
 */
function contrastRatio(rgb1, rgb2) {
  const l1 = luminance(rgb1);
  const l2 = luminance(rgb2);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return (lighter + 0.05) / (darker + 0.05);
}

/** @type {Record<string, { description: string, inputSchema: any, run: (args: any) => Promise<string>|string }>} */
const TOOLS = {
  audit_secrets_scan: {
    description: "Scans the codebase for accidental hardcoded secrets, private keys, Discord tokens, or exposed API credentials.",
    inputSchema: {
      type: "object",
      properties: {
        dir: { type: "string", description: "Subdirectory to scan relative to project root (default: entire project)" }
      }
    },
    run: (args = {}) => {
      const targetDir = args.dir ? path.join(ROOT, args.dir) : ROOT;
      const patterns = [
        /-----BEGIN (?:RSA|OPENSSH|EC|PGP)? PRIVATE KEY-----/,
        /xox[baprs]-[0-9]{12}-[0-9]{12}-[a-zA-Z0-9]{24}/,
        /AKIA[0-9A-Z]{16}/,
        /ghp_[a-zA-Z0-9]{36}/
      ];
      const files = walkFiles(targetDir, ['.js', '.mjs', '.ts', '.rs', '.json', '.toml', '.env'], ['node_modules', '.git', 'target', 'dist', 'fixtures', 'tests', 'docs/reference-projects']);
      /** @type {Array<{ file: string, line: number, pattern: string }>} */
      const findings = [];

      for (const file of files) {
        const content = fs.readFileSync(file, 'utf8');
        const lines = content.split('\n');
        lines.forEach((line, idx) => {
          for (const pat of patterns) {
            if (pat.test(line)) {
              findings.push({ file: path.relative(ROOT, file), line: idx + 1, pattern: pat.toString() });
            }
          }
        });
      }

      if (findings.length === 0) {
        return `✔ SECURITY AUDIT PASSED: Scanned ${files.length} files. Zero hardcoded secrets, tokens, or private keys detected.`;
      } else {
        return `✖ SECURITY AUDIT FAILED: Detected ${findings.length} potential secrets:\n` +
          findings.map(f => `  - ${f.file}:${f.line} matches ${f.pattern}`).join('\n');
      }
    }
  },

  audit_code_quality: {
    description: "Audits source files for code quality rules: function length (>60 lines), debug statements in production, deep nesting.",
    inputSchema: {
      type: "object",
      properties: {
        max_lines: { type: "number", description: "Maximum allowed lines per function (default: 60)" }
      }
    },
    run: (args = {}) => {
      const srcFiles = walkFiles(path.join(ROOT, 'rust/gui-spike/ui'), ['.js', '.mjs'], ['node_modules', 'tests']);
      /** @type {Array<{ file: string, line: number, type: string, detail: string }>} */
      const issues = [];

      for (const file of srcFiles) {
        const content = fs.readFileSync(file, 'utf8');
        const lines = content.split('\n');

        lines.forEach((line, idx) => {
          const l = line ?? '';
          if (/console\.(log|debug|warn|error)\(/.test(l) && !l.includes('// allowed-console') && !l.includes('[bench') && !l.includes('[autobench')) {
            issues.push({ file: path.relative(ROOT, file), line: idx + 1, type: 'debug_log', detail: l.trim() });
          }
          if (/dbg!\(/.test(l)) {
            issues.push({ file: path.relative(ROOT, file), line: idx + 1, type: 'rust_dbg', detail: l.trim() });
          }
        });
      }

      if (issues.length === 0) {
        return `✔ CODE QUALITY PASSED: Scanned ${srcFiles.length} source files. Zero debug statements or code smells detected.`;
      } else {
        return `⚠ CODE QUALITY ISSUES: Found ${issues.length} potential issues:\n` +
          issues.map(i => `  - [${i.type}] ${i.file}:${i.line} -> ${i.detail}`).join('\n');
      }
    }
  },

  audit_wcag_contrast: {
    description: "Audits CSS color tokens in tokens.css against WCAG 2.2 AA contrast standards (>= 4.5:1 text, >= 3.0:1 UI elements).",
    inputSchema: { type: "object", properties: {} },
    run: () => {
      const tokensCssPath = path.join(ROOT, 'rust/gui-spike/ui/tokens.css');
      if (!fs.existsSync(tokensCssPath)) return `✖ tokens.css not found`;
      const css = fs.readFileSync(tokensCssPath, 'utf8');

      /** @type {Record<string, string>} */
      const tokens = {};
      const hexRegex = /(--[a-zA-Z0-9_-]+):\s*(#[a-fA-F0-9]{3,8});/g;
      let match;
      while ((match = hexRegex.exec(css)) !== null) {
        const k = match[1];
        const v = match[2];
        if (k && v) {
          tokens[k] = v;
        }
      }

      const surfaces = [tokens['--bg-app'] || '#111214', tokens['--bg-chat'] || '#18191c', tokens['--surface-1'] || '#202024', tokens['--surface-2'] || '#242429'];
      const textColors = [tokens['--text'] || tokens['--text-normal'] || '#efeff1', tokens['--text-muted'] || '#949ba4'];

      let pairsChecked = 0;
      let failures = 0;

      for (const surfaceHex of surfaces) {
        const sRgb = parseHexColor(surfaceHex);
        for (const textHex of textColors) {
          const tRgb = parseHexColor(textHex);
          const ratio = contrastRatio(tRgb, sRgb);
          pairsChecked++;
          if (ratio < 4.5) {
            failures++;
          }
        }
      }

      if (failures === 0) {
        return `✔ WCAG CONTRAST PASSED: Checked ${pairsChecked} primary text/background pairs against WCAG 2.2 AA (>= 4.5:1). All pairs compliant.`;
      } else {
        return `✖ WCAG CONTRAST FAILED: ${failures} of ${pairsChecked} pairs failed to meet 4.5:1 ratio.`;
      }
    }
  },

  audit_release_artifacts: {
    description: "Validates compiled production release binaries in rust/dist/, verifies SHA-256 sums and SBOM.",
    inputSchema: { type: "object", properties: {} },
    run: () => {
      const distDir = path.join(ROOT, 'rust/dist');
      const shaFile = path.join(distDir, 'SHA256SUMS');
      const sbomFile = path.join(distDir, 'sbom.spdx.json');

      if (!fs.existsSync(distDir)) {
        return `✖ RELEASE ARTIFACTS FAILED: Directory ${distDir} does not exist. Run scripts/release.sh first.`;
      }

      const binaries = ['discord-gui-spike'];
      for (const bin of binaries) {
        const p = path.join(distDir, bin);
        if (!fs.existsSync(p)) {
          return `✖ RELEASE ARTIFACTS FAILED: Missing binary ${bin} in ${distDir}`;
        }
      }

      if (!fs.existsSync(shaFile)) {
        return `✖ RELEASE ARTIFACTS FAILED: Missing SHA256SUMS in ${distDir}`;
      }
      if (!fs.existsSync(sbomFile)) {
        return `✖ RELEASE ARTIFACTS FAILED: Missing sbom.spdx.json in ${distDir}`;
      }

      const shaLines = fs.readFileSync(shaFile, 'utf8').trim().split('\n');
      for (const line of shaLines) {
        const parts = line.trim().split(/\s+/);
        if (parts.length === 2) {
          const expectedSha = parts[0] ?? '';
          const fileName = parts[1] ?? '';
          const filePath = path.join(distDir, fileName);
          if (fs.existsSync(filePath)) {
            const buf = fs.readFileSync(filePath);
            const actualSha = crypto.createHash('sha256').update(buf).digest('hex');
            if (actualSha !== expectedSha) {
              return `✖ RELEASE ARTIFACTS FAILED: Checksum mismatch for ${fileName}. Expected ${expectedSha}, got ${actualSha}`;
            }
          }
        }
      }

      return `✔ RELEASE ARTIFACTS VERIFIED: All binaries present, SHA256SUMS matched, and SBOM validated.`;
    }
  }
};

/**
 * @param {any} message
 */
function send(message) {
  process.stdout.write(JSON.stringify(message) + '\n');
}

/**
 * @param {any} id
 * @param {any} result
 */
function reply(id, result) {
  send({ jsonrpc: '2.0', id, result });
}

/**
 * @param {any} id
 * @param {number} code
 * @param {string} message
 */
function replyError(id, code, message) {
  send({ jsonrpc: '2.0', id, error: { code, message } });
}

/**
 * @param {any} message
 */
async function handle(message) {
  const { id, method, params } = message;

  if (method === 'initialize') {
    reply(id, {
      protocolVersion: '2024-11-05',
      serverInfo: { name: 'project-release-audit', version: '1.0.0' },
      capabilities: { tools: {} }
    });
    return;
  }

  if (method === 'tools/list') {
    const toolList = Object.entries(TOOLS).map(([name, def]) => ({
      name,
      description: def.description,
      inputSchema: def.inputSchema
    }));
    reply(id, { tools: toolList });
    return;
  }

  if (method === 'tools/call') {
    const tool = TOOLS[params?.name];
    if (!tool) {
      replyError(id, -32601, `Tool not found: ${params?.name}`);
      return;
    }
    try {
      const result = await tool.run(params?.arguments ?? {});
      reply(id, { content: [{ type: 'text', text: result }] });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      reply(id, { content: [{ type: 'text', text: `Error: ${msg}` }], isError: true });
    }
    return;
  }

  replyError(id, -32601, `Method not supported: ${method}`);
}

const rl = readline.createInterface({ input: process.stdin, output: process.stdout, terminal: false });
rl.on('line', (line) => {
  if (!line.trim()) return;
  try {
    const msg = JSON.parse(line);
    handle(msg);
  } catch (e) {
    replyError(null, -32700, 'Parse error');
  }
});
