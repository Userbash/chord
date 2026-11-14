// @ts-check
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { evaluate, openPage } from "../../src/core/cdp/client.mjs";
import { launchBrowser } from "./browser.mjs";
import { cleanupUiFixture, startUiServer } from "./ui-fixture.mjs";
import { waitFor } from "./tmp.mjs";

/**
 * Serve a local fixture and wait for its actual document before exercising CLI commands.
 * @param {import("node:test").TestContext} t
 * @param {{ bin: string, file: string }} options
 */
export async function openStaticFixture(t, options) {
  const server = await startUiServer({ root: dirname(options.file), bootstrap: null });
  /** @type {string | null} */
  let dir = null;
  /** @type {Awaited<ReturnType<typeof launchBrowser>> | null} */
  let browser = null;
  t.after(() => cleanupUiFixture({ opened: null, browser, server, dir }));
  dir = await mkdtemp(join(tmpdir(), "discord-lab-static-"));
  browser = await launchBrowser({ bin: options.bin, url: server.url, userDataDir: join(dir, "profile") });
  const page = await openPage({ endpoint: browser.endpoint, hint: server.url });
  try {
    await waitFor(async () => {
      try {
        return Boolean(await evaluate(page.cdp,
          `location.href === ${JSON.stringify(server.url)} && document.readyState === "complete"`));
      } catch { return false; } // Navigation may replace the initial execution context.
    }, { timeoutMs: 15000 });
  } finally {
    page.cdp.close();
  }
  return { browser, dir, url: server.url };
}
