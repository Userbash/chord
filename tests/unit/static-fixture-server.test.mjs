// @ts-check
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { startUiServer } from "../helpers/ui-fixture.mjs";

const fixture = fileURLToPath(new URL("../fixtures/app/index.html", import.meta.url));

test("explicit static mode serves the real fixture bytes over HTTP without a Tauri injection", async (t) => {
  const server = await startUiServer({ root: dirname(fixture), bootstrap: null });
  t.after(() => server.close());
  const response = await fetch(server.url);
  assert.equal(response.status, 200);
  assert.equal(await response.text(), await readFile(fixture, "utf8"));
});

test("a non-null bootstrap still rejects an HTML page missing the Tauri module marker", async (t) => {
  const server = await startUiServer({ root: dirname(fixture), bootstrap: "" });
  t.after(() => server.close());
  const response = await fetch(server.url);
  assert.equal(response.status, 404);
  assert.equal(await response.text(), "not found");
});
