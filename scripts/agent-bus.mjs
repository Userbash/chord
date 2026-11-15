#!/usr/bin/env node
// @ts-nocheck
/**
 * agent-bus.mjs — Zero-Token Inter-Agent Communication & Contract Exchange Bus
 *
 * Implements AMCP v1.0 §3 (File Ownership) and §5 (Contract Synchronization).
 * Enables seamless, lightweight structured data exchange between independent AI workers:
 *   1. Event Ledger (.collab/bus/events.jsonl) — broadcast interface changes & completions
 *   2. File Locking Registry (.collab/bus/locks.json) — prevents concurrent edits & collisions
 *   3. Contract Registry (.collab/bus/contracts/) — shares DTOs, schemas, and API signatures
 *   4. Direct Mailbox (.collab/bus/mailbox/) — peer-to-peer typed messages between agents
 *
 * Usage:
 *   node scripts/agent-bus.mjs status
 *   node scripts/agent-bus.mjs lock <file> <worker_id> <task_id>
 *   node scripts/agent-bus.mjs unlock <file>
 *   node scripts/agent-bus.mjs publish --from <worker> --type <EVENT_TYPE> --summary <text> [--data <json>]
 *   node scripts/agent-bus.mjs read [--limit 10]
 *   node scripts/agent-bus.mjs send --from <sender> --to <receiver> --subject <subj> --body <text>
 *   node scripts/agent-bus.mjs inbox <worker_id>
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const PROJECT_ROOT = path.resolve(__dirname, '..');

const BUS_DIR = path.join(PROJECT_ROOT, '.collab/bus');
const EVENTS_FILE = path.join(BUS_DIR, 'events.jsonl');
const LOCKS_FILE = path.join(BUS_DIR, 'locks.json');
const CONTRACTS_DIR = path.join(BUS_DIR, 'contracts');
const MAILBOX_DIR = path.join(BUS_DIR, 'mailbox');

// Ensure directories exist
function ensureBusDirs() {
  if (!fs.existsSync(BUS_DIR)) fs.mkdirSync(BUS_DIR, { recursive: true });
  if (!fs.existsSync(CONTRACTS_DIR)) fs.mkdirSync(CONTRACTS_DIR, { recursive: true });
  if (!fs.existsSync(MAILBOX_DIR)) fs.mkdirSync(MAILBOX_DIR, { recursive: true });
  if (!fs.existsSync(LOCKS_FILE)) fs.writeFileSync(LOCKS_FILE, JSON.stringify({ activeLocks: [] }, null, 2), 'utf8');
}

export function publishEvent(sender, eventType, summary, data = {}) {
  ensureBusDirs();
  const event = {
    id: `evt_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
    timestamp: new Date().toISOString(),
    from: sender,
    type: eventType,
    summary,
    data,
  };

  fs.appendFileSync(EVENTS_FILE, JSON.stringify(event) + '\n', 'utf8');
  console.log(`📡 [AGENT-BUS] Event Published [${eventType}] from @${sender}: ${summary}`);
  return event;
}

export function readEvents(limit = 10, filterType = null) {
  ensureBusDirs();
  if (!fs.existsSync(EVENTS_FILE)) return [];
  const lines = fs.readFileSync(EVENTS_FILE, 'utf8').split('\n').filter(Boolean);
  let events = lines.map((l) => {
    try {
      return JSON.parse(l);
    } catch {
      return null;
    }
  }).filter(Boolean);

  if (filterType) {
    events = events.filter((e) => e.type === filterType);
  }

  return events.slice(-limit);
}

export function acquireFileLock(filePath, workerId, taskId) {
  ensureBusDirs();
  const locksData = JSON.parse(fs.readFileSync(LOCKS_FILE, 'utf8'));
  const normPath = path.normalize(filePath);

  const existing = locksData.activeLocks.find((l) => l.file === normPath);
  if (existing) {
    if (existing.worker === workerId) {
      console.log(`🔒 File already locked by you (@${workerId}): ${normPath}`);
      return { success: true, lock: existing };
    }
    console.error(`❌ CONFLICT BLOCKED: File ${normPath} is already locked by @${existing.worker} for task ${existing.task}!`);
    return { success: false, conflictWith: existing };
  }

  const newLock = {
    file: normPath,
    worker: workerId,
    task: taskId,
    acquiredAt: new Date().toISOString(),
  };
  locksData.activeLocks.push(newLock);
  fs.writeFileSync(LOCKS_FILE, JSON.stringify(locksData, null, 2), 'utf8');
  console.log(`🔒 [LOCK ACQUIRED] File: ${normPath} -> @${workerId} (${taskId})`);
  return { success: true, lock: newLock };
}

export function releaseFileLock(filePath, workerId = null) {
  ensureBusDirs();
  const locksData = JSON.parse(fs.readFileSync(LOCKS_FILE, 'utf8'));
  const normPath = path.normalize(filePath);

  const beforeCount = locksData.activeLocks.length;
  locksData.activeLocks = locksData.activeLocks.filter((l) => {
    if (l.file === normPath) {
      if (workerId && l.worker !== workerId) return true; // not owner
      return false; // remove
    }
    return true;
  });

  const removed = beforeCount - locksData.activeLocks.length;
  fs.writeFileSync(LOCKS_FILE, JSON.stringify(locksData, null, 2), 'utf8');
  if (removed > 0) {
    console.log(`🔓 [LOCK RELEASED] File: ${normPath}`);
    return { success: true, released: normPath };
  } else {
    console.log(`⚠️  No active lock found for: ${normPath}`);
    return { success: false, message: 'Not locked' };
  }
}

export function sendAgentMessage(sender, receiver, subject, body, payload = {}) {
  ensureBusDirs();
  const receiverDir = path.join(MAILBOX_DIR, receiver);
  if (!fs.existsSync(receiverDir)) fs.mkdirSync(receiverDir, { recursive: true });

  const msgId = `msg_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`;
  const message = {
    id: msgId,
    timestamp: new Date().toISOString(),
    from: sender,
    to: receiver,
    subject,
    body,
    payload,
    read: false,
  };

  const msgPath = path.join(receiverDir, `${msgId}.json`);
  fs.writeFileSync(msgPath, JSON.stringify(message, null, 2), 'utf8');
  console.log(`✉️  [MAILBOX] Message sent from @${sender} -> @${receiver}: "${subject}"`);

  // Also publish notification event to bus
  publishEvent(sender, 'AGENT_MESSAGE_SENT', `Direct message to @${receiver}: ${subject}`, { to: receiver, msgId });
  return message;
}

export function readAgentInbox(workerId) {
  ensureBusDirs();
  const receiverDir = path.join(MAILBOX_DIR, workerId);
  if (!fs.existsSync(receiverDir)) return [];

  const files = fs.readdirSync(receiverDir).filter((f) => f.endsWith('.json'));
  const messages = [];

  for (const f of files) {
    try {
      const p = path.join(receiverDir, f);
      const data = JSON.parse(fs.readFileSync(p, 'utf8'));
      messages.push(data);
    } catch {}
  }

  return messages.sort((a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());
}

export function showBusStatus() {
  ensureBusDirs();
  const locksData = JSON.parse(fs.readFileSync(LOCKS_FILE, 'utf8'));
  const recentEvents = readEvents(5);

  console.log(`\n================================================================`);
  console.log(`📡 AI MULTI-AGENT COLLABORATION BUS (.collab/bus/)`);
  console.log(`================================================================`);
  console.log(`🔒 Active File Locks (${locksData.activeLocks.length}):`);
  if (locksData.activeLocks.length === 0) {
    console.log(`   (No active file locks — all files available)`);
  } else {
    for (const l of locksData.activeLocks) {
      console.log(`   • ${l.file.padEnd(40)} | @${l.worker.padEnd(16)} | Task: ${l.task}`);
    }
  }

  console.log(`\n📢 Recent Bus Events (last ${recentEvents.length}):`);
  if (recentEvents.length === 0) {
    console.log(`   (No events published yet)`);
  } else {
    for (const e of recentEvents) {
      const timeStr = new Date(e.timestamp).toLocaleTimeString();
      console.log(`   [${timeStr}] @${e.from} [${e.type}]: ${e.summary}`);
    }
  }
  console.log(`================================================================\n`);
}

// CLI handler
async function main() {
  const args = process.argv.slice(2);
  const cmd = args[0] || 'status';

  switch (cmd) {
    case 'status':
      showBusStatus();
      break;

    case 'lock': {
      const file = args[1];
      const worker = args[2] || 'worker_unknown';
      const task = args[3] || 'task_unknown';
      if (!file) {
        console.error('Usage: agent-bus lock <file> [worker] [task]');
        process.exit(1);
      }
      const res = acquireFileLock(file, worker, task);
      process.exit(res.success ? 0 : 1);
      break;
    }

    case 'unlock': {
      const file = args[1];
      const worker = args[2] || null;
      if (!file) {
        console.error('Usage: agent-bus unlock <file> [worker]');
        process.exit(1);
      }
      releaseFileLock(file, worker);
      break;
    }

    case 'publish': {
      const sender = args.find((a, i) => args[i - 1] === '--from') || 'system';
      const eventType = args.find((a, i) => args[i - 1] === '--type') || 'NOTICE';
      const summary = args.find((a, i) => args[i - 1] === '--summary') || args.slice(1).join(' ');
      publishEvent(sender, eventType, summary);
      break;
    }

    case 'read': {
      const limit = parseInt(args.find((a, i) => args[i - 1] === '--limit') || '10', 10);
      const events = readEvents(limit);
      console.log(JSON.stringify(events, null, 2));
      break;
    }

    case 'send': {
      const from = args.find((a, i) => args[i - 1] === '--from') || 'system';
      const to = args.find((a, i) => args[i - 1] === '--to');
      const subject = args.find((a, i) => args[i - 1] === '--subject') || 'Inter-agent message';
      const body = args.find((a, i) => args[i - 1] === '--body') || 'No content';
      if (!to) {
        console.error('Specify receiver: agent-bus send --from <sender> --to <receiver> --subject <subj> --body <text>');
        process.exit(1);
      }
      sendAgentMessage(from, to, subject, body);
      break;
    }

    case 'inbox': {
      const worker = args[1];
      if (!worker) {
        console.error('Usage: agent-bus inbox <worker_id>');
        process.exit(1);
      }
      const messages = readAgentInbox(worker);
      console.log(`\n📬 INBOX for @${worker} (${messages.length} messages):`);
      for (const m of messages) {
        console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
        console.log(`From: @${m.from} | Date: ${m.timestamp}`);
        console.log(`Subject: ${m.subject}`);
        console.log(`Body: ${m.body}`);
      }
      console.log();
      break;
    }

    default:
      console.log('Commands: status, lock, unlock, publish, read, send, inbox');
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main();
}
