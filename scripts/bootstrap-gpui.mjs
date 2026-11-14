#!/usr/bin/env node
// @ts-check
import { spawnSync } from "node:child_process";
import { existsSync, lstatSync, readdirSync, realpathSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const upstreamUrl = "https://github.com/longbridge/gpui-kit.git";
const repositoryRoot = fileURLToPath(new URL("../", import.meta.url));
const dependencyName = "gpui-kit";

/** @param {string} root @param {string[]} args @param {NodeJS.ProcessEnv} env */
function git(root, args, env) {
  const result = spawnSync("git", ["-C", root, ...args], {
    env: { ...env, GIT_TERMINAL_PROMPT: "0", GIT_OPTIONAL_LOCKS: "0" },
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    const detail = result.error?.message ?? (result.stderr || result.stdout).trim();
    throw new Error(`git ${args.join(" ")} failed: ${detail || `exit ${result.status}`}`);
  }
  return result.stdout.trim();
}

/** @param {string} root @param {NodeJS.ProcessEnv} env */
function readCommittedPin(root, env) {
  const topLevel = git(root, ["rev-parse", "--show-toplevel"], env);
  if (realpathSync(topLevel) !== realpathSync(root)) throw new Error("Expected the Chord repository root");

  const entry = git(root, ["ls-tree", "HEAD", "--", dependencyName], env);
  const pin = /^160000 commit ([a-f0-9]{40,64})\tgpui-kit$/.exec(entry)?.[1];
  if (!pin) throw new Error("gpui-kit must be a committed gitlink (mode 160000)");
  return pin;
}

/** @param {string} root @param {NodeJS.ProcessEnv} env */
function verifySubmoduleConfig(root, env) {
  const modulesFile = join(root, ".gitmodules");
  if (!existsSync(modulesFile)) throw new Error("Missing .gitmodules for the gpui-kit authoritative upstream");
  const modulePath = git(root, ["config", "--file", modulesFile, "--get-all", "submodule.gpui-kit.path"], env);
  const moduleUrl = git(root, ["config", "--file", modulesFile, "--get-all", "submodule.gpui-kit.url"], env);
  if (modulePath !== dependencyName || moduleUrl !== upstreamUrl) {
    throw new Error(`gpui-kit must use the authoritative upstream ${upstreamUrl} at path ${dependencyName}`);
  }
}

/** @param {string} root @param {string} dependency @param {boolean} checkOnly @param {NodeJS.ProcessEnv} env */
function initializeDependency(root, dependency, checkOnly, env) {
  const dependencyInfo = lstatSync(dependency, { throwIfNoEntry: false });
  if (dependencyInfo && (!dependencyInfo.isDirectory() || dependencyInfo.isSymbolicLink())) {
    throw new Error("gpui-kit is not an initialized submodule directory; preserve local work and inspect it manually");
  }
  const initialized = existsSync(join(dependency, ".git"));
  if (!initialized) {
    if (existsSync(dependency) && readdirSync(dependency).length > 0) {
      throw new Error("gpui-kit is not an initialized submodule; preserve local work and inspect it manually");
    }
    if (checkOnly) throw new Error("gpui-kit is not initialized; run node scripts/bootstrap-gpui.mjs");
    // Override a stale local submodule URL without rewriting tracked metadata.
    // Git's submodule checkout uses the gitlink SHA, never --remote or a branch.
    git(root, ["-c", `submodule.gpui-kit.url=${upstreamUrl}`, "submodule", "update", "--init", "--checkout", "--depth", "1", "--", dependencyName], env);
  }
}

/** @param {string} dependency @param {string} pin @param {NodeJS.ProcessEnv} env */
function verifyCheckout(dependency, pin, env) {
  const dependencyTopLevel = git(dependency, ["rev-parse", "--show-toplevel"], env);
  if (realpathSync(dependencyTopLevel) !== realpathSync(dependency)) {
    throw new Error("gpui-kit is not an initialized submodule repository");
  }
  const revision = git(dependency, ["rev-parse", "--verify", "HEAD"], env);
  if (revision !== pin) throw new Error(`gpui-kit revision mismatch: expected ${pin}, found ${revision}; existing checkout was preserved`);
  const origin = git(dependency, ["config", "--get-all", "remote.origin.url"], env);
  if (origin !== upstreamUrl) throw new Error(`gpui-kit must use the authoritative upstream ${upstreamUrl}`);
  if (git(dependency, ["status", "--porcelain=v1", "--untracked-files=all", "--ignore-submodules=none"], env)) {
    throw new Error("gpui-kit has local changes; preserve or commit them before building");
  }
  for (const name of ["kit", "component", "webview"]) {
    const manifest = join(dependency, "crates", name, "Cargo.toml");
    if (!existsSync(manifest) || !statSync(manifest).isFile()) throw new Error(`gpui-kit is missing ${manifest}`);
  }
}

/**
 * Verify a clean, exact GPUI checkout. Initialize it only when it is absent.
 * The committed gitlink selects the source, independently of upstream's branch.
 * Existing work is never reset, cleaned, removed, or moved to another revision.
 * @param {{ root?: string, checkOnly?: boolean, env?: NodeJS.ProcessEnv }} [options]
 * @returns {string} The verified gitlink commit.
 */
export function bootstrapGpui({ root = repositoryRoot, checkOnly = false, env = process.env } = {}) {
  root = resolve(root);
  const pin = readCommittedPin(root, env);
  verifySubmoduleConfig(root, env);
  const dependency = join(root, dependencyName);
  initializeDependency(root, dependency, checkOnly, env);
  verifyCheckout(dependency, pin, env);
  return pin;
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  const args = process.argv.slice(2);
  if (args.length === 1 && (args[0] === "--help" || args[0] === "-h")) {
    console.log("Usage: node scripts/bootstrap-gpui.mjs [--check]\nInitialize the committed GPUI gitlink, or verify it without writes with --check.");
  } else if (args.length > 1 || (args.length === 1 && args[0] !== "--check")) {
    console.error("gpui-bootstrap: expected no arguments or --check (use --help for usage)");
    process.exitCode = 1;
  } else {
    try {
      const pin = bootstrapGpui({ checkOnly: args[0] === "--check" });
      console.log(`gpui-bootstrap: verified clean gpui-kit at ${pin}`);
    } catch (error) {
      console.error(`gpui-bootstrap: ${error instanceof Error ? error.message : String(error)}`);
      process.exitCode = 2;
    }
  }
}
