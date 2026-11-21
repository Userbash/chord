// @ts-check
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { existsSync } from 'node:fs';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { cleanupUiFixture } from '../helpers/ui-fixture.mjs';
import { launchBrowser } from '../helpers/browser.mjs';

/** @param {import('node:test').TestContext} t @param {boolean} [ignoreTermination] */
async function resources(t, ignoreTermination = false) {
  const dir = await mkdtemp(join(tmpdir(), 'fixture cleanup '));
  const script = (ignoreTermination ? 'process.on("SIGTERM", () => {}); ' : '')
    + 'process.stdout.write("ready"); setInterval(() => {}, 1000);';
  const child = spawn(process.execPath, ['-e', script], {
    stdio: ['ignore', 'pipe', 'ignore'],
  });
  t.after(async () => {
    if (child.exitCode === null && child.signalCode === null) {
      const exited = once(child, 'exit');
      child.kill('SIGKILL');
      await exited;
    }
    assert.equal(dirname(dir), resolve(tmpdir()));
    assert.ok(basename(dir).startsWith('fixture cleanup '));
    await rm(dir, { recursive: true, force: true });
  });
  await once(child.stdout, 'data');
  await writeFile(join(dir, 'profile-data'), 'must survive until browser exit');
  return { dir, browser: { child } };
}

test('fixture cleanup waits for its owned child and server before removing the profile', { timeout: 10000 }, async t => {
  const { dir, browser } = await resources(t);
  let closed = false;
  await cleanupUiFixture({
    opened: null, browser, dir,
    server: { close: async () => {
      assert.ok(browser.child.exitCode !== null || browser.child.signalCode !== null, 'browser is still alive');
      assert.equal(existsSync(dir), true, 'profile removed before server shutdown');
      closed = true;
    } },
  });
  assert.equal(closed, true);
  assert.equal(existsSync(dir), false);
});

test('a failing fixture close still terminates its owned child and removes the profile', { timeout: 10000 }, async t => {
  const { dir, browser } = await resources(t);
  const failure = new Error('fixture close failed');
  let closed = false;
  await assert.rejects(cleanupUiFixture({
    opened: { close: async () => { throw failure; } }, browser, dir,
    server: { close: async () => {
      assert.ok(browser.child.exitCode !== null || browser.child.signalCode !== null, 'browser is still alive');
      assert.equal(existsSync(dir), true);
      closed = true;
    } },
  }), failure);
  assert.equal(closed, true);
  assert.equal(existsSync(dir), false);
});

test('forced browser termination also completes before profile cleanup', { timeout: 10000 }, async t => {
  const { dir, browser } = await resources(t, true);
  await cleanupUiFixture({
    opened: null, browser, dir,
    server: { close: async () => {
      assert.ok(browser.child.exitCode !== null || browser.child.signalCode !== null, 'forced termination is still pending');
      assert.equal(existsSync(dir), true);
    } },
  });
  assert.equal(existsSync(dir), false);
});

test('server shutdown failure remains visible and still releases the profile', { timeout: 10000 }, async t => {
  const { dir, browser } = await resources(t);
  const failure = new Error('server close failed');
  await assert.rejects(cleanupUiFixture({
    opened: null, browser, dir, server: { close: async () => { throw failure; } },
  }), failure);
  assert.ok(browser.child.exitCode !== null || browser.child.signalCode !== null);
  assert.equal(existsSync(dir), false);
});

test('browser launch failure waits for its owned child before the caller can remove its profile', { timeout: 25000 }, async t => {
  const { dir, browser } = await resources(t);
  await assert.rejects(launchBrowser({
    bin: process.execPath, url: 'about:blank', userDataDir: dir, timeoutMs: 20,
    spawnProcess: () => browser.child,
  }), /DevToolsActivePort/);
  assert.ok(browser.child.exitCode !== null || browser.child.signalCode !== null, 'launch failure left its owned child alive');
  assert.equal(existsSync(dir), true, 'caller still owns the profile');
  await rm(dir, { recursive: true, force: true });
  assert.equal(existsSync(dir), false);
});

/** @param {'transient' | 'persistent'} mode */
function profileConflict(mode) {
  const source = String.raw`
    import fs from 'node:fs';
    import assert from 'node:assert/strict';
    import { mkdtemp, rm, writeFile } from 'node:fs/promises';
    import { spawn } from 'node:child_process';
    import { once } from 'node:events';
    import { tmpdir } from 'node:os';
    import { join } from 'node:path';
    const dir = await mkdtemp(join(tmpdir(), 'late chromium profile '));
    const child = spawn(process.execPath, ['-e', 'process.stdout.write("ready");setInterval(()=>{},1000)']);
    await once(child.stdout, 'data');
    await writeFile(join(dir, 'Preferences'), 'initial profile state');
    const original = fs.rmdir;
    let attempts = 0, conflicts = 0, active = true, serverClosed = false, errorCode;
    fs.rmdir = function(path, ...args) {
      const target = String(path) === dir;
      if (active && target) {
        assert.ok(child.exitCode !== null || child.signalCode !== null, 'browser still alive');
        assert.equal(serverClosed, true, 'server still open');
        attempts++;
        if (attempts === 2 || (process.argv[1] === 'persistent' && attempts > 2))
          fs.writeFileSync(join(dir, '.chromium-late-write'), 'late profile state');
      }
      const callback = args.pop();
      return original.call(this, path, ...args, error => {
        if (active && target && error?.code === 'ENOTEMPTY') conflicts++;
        callback(error);
      });
    };
    try {
      const { cleanupUiFixture } = await import(process.argv[2]);
      try { await cleanupUiFixture({ opened:null, browser:{child}, dir,
        server:{close:async()=>{serverClosed=true;}} }); } catch (error) { errorCode = error.code; }
      console.log(JSON.stringify({ attempts, conflicts, errorCode, exists:fs.existsSync(dir), serverClosed }));
    } finally {
      active = false; fs.rmdir = original;
      if (child.exitCode === null && child.signalCode === null) {
        const exited = once(child, 'exit'); child.kill('SIGKILL'); await exited;
      }
      await rm(dir, {recursive:true, force:true});
    }
  `;
  const result = spawnSync(process.execPath, ['--input-type=module', '-e', source, mode,
    new URL('../helpers/ui-fixture.mjs', import.meta.url).href], { encoding: 'utf8', timeout: 10000 });
  assert.equal(result.status, 0, result.stderr || result.error?.message);
  return JSON.parse(result.stdout);
}

test('a late profile write after browser exit is removed before cleanup succeeds', { timeout: 15000 }, () => {
  const result = profileConflict('transient');
  assert.ok(result.conflicts >= 2, 'the filesystem did not return the induced late ENOTEMPTY');
  assert.ok(result.attempts >= 3, 'cleanup did not retry the real conflicting directory');
  assert.equal(result.errorCode, undefined);
  assert.equal(result.exists, false);
  assert.equal(result.serverClosed, true);
});

test('a persistent profile writer still fails cleanup instead of being ignored', { timeout: 15000 }, () => {
  const result = profileConflict('persistent');
  assert.equal(result.errorCode, 'ENOTEMPTY');
  assert.equal(result.exists, true, 'a failed cleanup must not claim removal');
  assert.ok(result.conflicts >= 2);
  assert.ok(result.attempts <= 20, 'cleanup retries are unbounded');
});
