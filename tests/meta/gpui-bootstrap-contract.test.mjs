// @ts-check
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, lstatSync, mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { describe, it } from "node:test";
import { bootstrapGpui } from "../../scripts/bootstrap-gpui.mjs";

const upstreamUrl = "https://github.com/longbridge/gpui-kit.git";
const projectRoot = fileURLToPath(new URL("../../", import.meta.url));
const requiredManifests = ["kit", "component", "webview"];

/** @param {string} cwd @param {string[]} args @param {NodeJS.ProcessEnv} env */
function git(cwd, args, env) {
  const result = spawnSync("git", ["-C", cwd, ...args], { env, encoding: "utf8" });
  assert.equal(result.status, 0, `git ${args.join(" ")}: ${result.error?.message ?? result.stderr}`);
  return result.stdout.trim();
}

/** @param {string} directory @param {string} upstream @returns {NodeJS.ProcessEnv} */
function fixtureEnvironment(directory, upstream) {
  return {
    ...process.env,
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: join(directory, "empty-gitconfig"),
    GIT_AUTHOR_NAME: "Bootstrap contract",
    GIT_AUTHOR_EMAIL: "bootstrap@example.invalid",
    GIT_COMMITTER_NAME: "Bootstrap contract",
    GIT_COMMITTER_EMAIL: "bootstrap@example.invalid",
    GIT_TERMINAL_PROMPT: "0",
    GIT_CONFIG_COUNT: "2",
    GIT_CONFIG_KEY_0: `url.${pathToFileURL(upstream).href}.insteadOf`,
    GIT_CONFIG_VALUE_0: upstreamUrl,
    GIT_CONFIG_KEY_1: "protocol.file.allow",
    GIT_CONFIG_VALUE_1: "always",
  };
}

/** @param {{ missingManifest?: string }} [options] */
function fixture(options = {}) {
  const directory = mkdtempSync(join(tmpdir(), "chord gpui bootstrap "));
  const root = join(directory, "project checkout");
  const upstream = join(directory, "fixture upstream");
  mkdirSync(root);
  mkdirSync(upstream);
  const env = fixtureEnvironment(directory, upstream);
  git(upstream, ["init", "--quiet"], env);
  for (const name of requiredManifests) {
    if (name === options.missingManifest) continue;
    const manifest = join(upstream, "crates", name, "Cargo.toml");
    mkdirSync(dirname(manifest), { recursive: true });
    writeFileSync(manifest, `[package]\nname = "fixture-${name}"\nversion = "0.0.0"\n`);
  }
  writeFileSync(join(upstream, "marker"), "pinned revision\n");
  git(upstream, ["add", "."], env);
  git(upstream, ["-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "Pinned fixture"], env);
  const pin = git(upstream, ["rev-parse", "HEAD"], env);
  writeFileSync(join(upstream, "marker"), "newer floating revision\n");
  git(upstream, ["-c", "commit.gpgsign=false", "commit", "--quiet", "-am", "Newer fixture"], env);
  const newer = git(upstream, ["rev-parse", "HEAD"], env);

  git(root, ["init", "--quiet"], env);
  writeFileSync(join(root, ".gitmodules"),
    `[submodule "gpui-kit"]\n\tpath = gpui-kit\n\turl = ${upstreamUrl}\n`);
  git(root, ["add", ".gitmodules"], env);
  git(root, ["update-index", "--add", "--cacheinfo", `160000,${pin},gpui-kit`], env);
  git(root, ["-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "Project gitlink"], env);
  const cleanup = () => {
    const target = resolve(directory);
    const relativeTarget = relative(resolve(tmpdir()), target);
    assert.ok(relativeTarget && !relativeTarget.startsWith("..") && !isAbsolute(relativeTarget));
    assert.ok(basename(target).startsWith("chord gpui bootstrap "));
    rmSync(target, { recursive: true, force: true });
  };
  return { directory, root, upstream, env, pin, newer, cleanup };
}

describe("GPUI bootstrap contract", () => {
  it("registers the authoritative upstream for the existing gitlink", () => {
    assert.match(git(projectRoot, ["ls-tree", "HEAD", "--", "gpui-kit"], process.env),
      /^160000 commit 957ae8b55f1d02ea0f322951a9fa3b3f0cbbb282\tgpui-kit$/);
    assert.equal(git(projectRoot, ["config", "--file", ".gitmodules", "--get", "submodule.gpui-kit.path"], process.env), "gpui-kit");
    assert.equal(git(projectRoot, ["config", "--file", ".gitmodules", "--get", "submodule.gpui-kit.url"], process.env), upstreamUrl);
  });

  it("bootstraps the committed SHA rather than the newer upstream branch", () => {
    const f = fixture();
    try {
      assert.equal(bootstrapGpui({ root: f.root, env: f.env }), f.pin);
      const dependency = join(f.root, "gpui-kit");
      assert.equal(git(dependency, ["rev-parse", "HEAD"], f.env), f.pin);
      assert.notEqual(f.pin, f.newer);
      assert.equal(readFileSync(join(dependency, "marker"), "utf8"), "pinned revision\n");
      assert.equal(git(dependency, ["config", "--get", "remote.origin.url"], f.env), upstreamUrl);
      assert.equal(spawnSync("git", ["-C", dependency, "symbolic-ref", "--quiet", "HEAD"], { env: f.env }).status, 1);
      assert.equal(git(f.root, ["status", "--short"], f.env), "");
    } finally { f.cleanup(); }
  });

  it("check-only fails for an uninitialized dependency without creating files", () => {
    const f = fixture();
    try {
      const before = git(f.root, ["status", "--short"], f.env);
      assert.throws(() => bootstrapGpui({ root: f.root, env: f.env, checkOnly: true }), /not initialized/);
      assert.equal(existsSync(join(f.root, "gpui-kit")), false);
      assert.equal(git(f.root, ["status", "--short"], f.env), before);
    } finally { f.cleanup(); }
  });

  it("verifies an exact clean checkout idempotently", () => {
    const f = fixture();
    try {
      bootstrapGpui({ root: f.root, env: f.env });
      const before = readFileSync(join(f.root, ".git", "config"), "utf8");
      assert.equal(bootstrapGpui({ root: f.root, env: f.env, checkOnly: true }), f.pin);
      assert.equal(bootstrapGpui({ root: f.root, env: f.env }), f.pin);
      assert.equal(readFileSync(join(f.root, ".git", "config"), "utf8"), before);
      assert.equal(git(f.root, ["status", "--short"], f.env), "");
    } finally { f.cleanup(); }
  });

  it("refuses an existing different revision without resetting it", () => {
    const f = fixture();
    try {
      bootstrapGpui({ root: f.root, env: f.env });
      const dependency = join(f.root, "gpui-kit");
      git(dependency, ["fetch", "origin", f.newer], f.env);
      git(dependency, ["checkout", "--detach", f.newer], f.env);
      assert.throws(() => bootstrapGpui({ root: f.root, env: f.env }), /revision mismatch/);
      assert.equal(git(dependency, ["rev-parse", "HEAD"], f.env), f.newer);
    } finally { f.cleanup(); }
  });

  for (const filename of ["marker", "untracked.txt"]) {
    it(`refuses local changes to ${filename} without discarding them`, () => {
      const f = fixture();
      try {
        bootstrapGpui({ root: f.root, env: f.env });
        const changed = join(f.root, "gpui-kit", filename);
        writeFileSync(changed, "keep local work\n");
        assert.throws(() => bootstrapGpui({ root: f.root, env: f.env }), /local changes/);
        assert.equal(readFileSync(changed, "utf8"), "keep local work\n");
      } finally { f.cleanup(); }
    });
  }

  it("refuses an unmanaged directory without modifying it", () => {
    const f = fixture();
    try {
      const dependency = join(f.root, "gpui-kit");
      mkdirSync(dependency);
      writeFileSync(join(dependency, "local-work"), "keep me\n");
      assert.throws(() => bootstrapGpui({ root: f.root, env: f.env }), /not an initialized submodule/);
      assert.equal(readFileSync(join(dependency, "local-work"), "utf8"), "keep me\n");
    } finally { f.cleanup(); }
  });

  it("rejects dangling directory links before any clone", () => {
    const f = fixture();
    try {
      const dependency = join(f.root, "gpui-kit");
      symlinkSync(join(f.directory, "unused target"), dependency, process.platform === "win32" ? "junction" : "dir");
      assert.throws(() => bootstrapGpui({ root: f.root, env: f.env }), /not an initialized submodule directory/);
      assert.equal(lstatSync(dependency).isSymbolicLink(), true);
      assert.equal(existsSync(join(f.directory, "unused target")), false);
    } finally { f.cleanup(); }
  });

  it("rejects a checkout missing a required manifest", () => {
    const f = fixture({ missingManifest: "webview" });
    try {
      assert.throws(() => bootstrapGpui({ root: f.root, env: f.env }), /crates[\\/]webview[\\/]Cargo.toml/);
    } finally { f.cleanup(); }
  });

  it("rejects an untrusted .gitmodules URL before any clone", () => {
    const f = fixture();
    try {
      git(f.root, ["config", "--file", ".gitmodules", "submodule.gpui-kit.url", "https://example.invalid/gpui-kit.git"], f.env);
      assert.throws(() => bootstrapGpui({ root: f.root, env: f.env }), /authoritative upstream/);
    } finally { f.cleanup(); }
  });

  it("rejects a normal tracked directory in place of a gitlink", () => {
    const f = fixture();
    try {
      git(f.root, ["update-index", "--force-remove", "gpui-kit"], f.env);
      git(f.root, ["-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "Remove gitlink"], f.env);
      assert.throws(() => bootstrapGpui({ root: f.root, env: f.env }), /committed gitlink/);
    } finally { f.cleanup(); }
  });

  it("supports a git worktree whose .git metadata is a file", () => {
    const f = fixture();
    try {
      const worktree = join(f.directory, "linked worktree");
      git(f.root, ["worktree", "add", "--quiet", "--detach", worktree, "HEAD"], f.env);
      assert.match(readFileSync(join(worktree, ".git"), "utf8"), /^gitdir:/);
      assert.equal(bootstrapGpui({ root: worktree, env: f.env }), f.pin);
      assert.equal(bootstrapGpui({ root: worktree, env: f.env, checkOnly: true }), f.pin);
      assert.equal(git(worktree, ["status", "--short"], f.env), "");
    } finally { f.cleanup(); }
  });
});
