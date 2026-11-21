#!/usr/bin/env node
// @ts-check
/**
 * R4-08: Cross-Platform Distribution Packaging Orchestrator & Verifier.
 * Verifies and/or stages packages for:
 *   - Linux: Debian/Ubuntu (.deb), RPM (.rpm), AppImage (.AppImage), Flatpak
 *   - Windows: WiX MSI (.msi) + Portable
 *   - macOS: .app + .dmg
 * Including desktop entries, icons, and MIME types / URI scheme handlers.
 */
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const VALID_PLATFORMS = new Set(["all", "linux", "windows", "macos"]);

/**
 * Parse CLI arguments.
 * @param {string[]} argv
 */
function parseArgs(argv) {
  let platform = "all";
  let verifyOnly = false;
  let dryRun = false;
  let outDir = join(ROOT, "rust", "dist");
  let reportJson = "";

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--platform") {
      platform = argv[i + 1] ?? "";
      i += 1;
    } else if (arg === "--verify-only") {
      verifyOnly = true;
    } else if (arg === "--dry-run") {
      dryRun = true;
    } else if (arg === "--out-dir" || arg === "--output") {
      outDir = argv[i + 1] ?? outDir;
      i += 1;
    } else if (arg === "--report-json") {
      reportJson = argv[i + 1] ?? "";
      i += 1;
    } else if (arg === "-h" || arg === "--help") {
      console.log(
        "Usage: node scripts/package-dist.mjs [--platform all|linux|windows|macos] [--verify-only] [--dry-run] [--out-dir DIR] [--report-json PATH]",
      );
      process.exit(0);
    } else {
      console.error(`package-dist: unknown option: ${arg}`);
      process.exit(1);
    }
  }

  if (!VALID_PLATFORMS.has(platform)) {
    console.error(`package-dist: invalid platform '${platform}'. Expected: all, linux, windows, macos`);
    process.exit(1);
  }

  return { platform, verifyOnly, dryRun, outDir, reportJson };
}

/**
 * Verify a file exists and matches all required regex patterns.
 * @param {string} relPath
 * @param {RegExp[]} patterns
 * @returns {{ ok: boolean, path: string, missingPatterns: string[] }}
 */
function verifySpec(relPath, patterns) {
  const fullPath = join(ROOT, relPath);
  if (!existsSync(fullPath)) {
    return { ok: false, path: relPath, missingPatterns: ["FILE_MISSING"] };
  }
  const content = readFileSync(fullPath, "utf8");
  const missingPatterns = patterns
    .filter((re) => !re.test(content))
    .map((re) => re.toString());
  return { ok: missingPatterns.length === 0, path: relPath, missingPatterns };
}

/**
 * Verify all 6 distribution targets.
 * @param {string} platform
 */
export function verifyAllTargets(platform = "all") {
  /** @type {Record<string, any>} */
  const targets = {};

  if (platform === "all" || platform === "linux") {
    const debSpec = verifySpec("packaging/deb/DEBIAN/control", [
      /Package:\s*discord-gpui/,
      /shared-mime-info/,
      /desktop-file-utils/,
    ]);
    const debDesktop = verifySpec("packaging/deb/usr/share/applications/discord-gpui.desktop", [
      /\[Desktop Entry\]/,
      /Icon=discord-gpui/,
      /MimeType=.*x-scheme-handler\/discord/,
      /application\/x-discord-gpui-invite/,
    ]);
    const debMime = verifySpec("packaging/deb/usr/share/mime/packages/discord-gpui.xml", [
      /x-scheme-handler\/discord/,
      /x-scheme-handler\/disclip/,
      /application\/x-discord-gpui-invite/,
    ]);
    const debIcon = existsSync(
      join(ROOT, "packaging/deb/usr/share/icons/hicolor/scalable/apps/discord-gpui.svg"),
    );
    targets.deb = {
      ok: debSpec.ok && debDesktop.ok && debMime.ok && debIcon,
      spec: debSpec,
      desktop: debDesktop,
      mime: debMime,
      icon: debIcon,
    };

    const rpmSpec = verifySpec("packaging/rpm/discord-gpui.spec", [
      /Name:\s*discord-gpui/,
      /discord-gpui\.desktop/,
      /discord-gpui\.svg/,
      /discord-gpui\.png/,
      /MimeType=x-scheme-handler\/discord;x-scheme-handler\/disclip/,
    ]);
    targets.rpm = { ok: rpmSpec.ok, spec: rpmSpec };

    const appImageScript = verifySpec("scripts/build-appimage.sh", [/AppRun/, /\.DirIcon/]);
    const appImageDesktop = verifySpec("packaging/appimage/discord-gpui.desktop", [
      /Icon=discord-gpui/,
      /MimeType=x-scheme-handler\/discord/,
    ]);
    const appImageIcon = existsSync(join(ROOT, "packaging/appimage/discord-gpui.svg"));
    targets.appimage = {
      ok: appImageScript.ok && appImageDesktop.ok && appImageIcon,
      script: appImageScript,
      desktop: appImageDesktop,
      icon: appImageIcon,
    };

    const flatpak = verifySpec("packaging/flatpak/com.discord_lab.DiscordGpui.yml", [
      /app-id:\s*com\.discord_lab\.DiscordGpui/,
      /\/app\/share\/applications\/com\.discord_lab\.DiscordGpui\.desktop/,
      /\/app\/share\/icons\/hicolor\/scalable\/apps\/com\.discord_lab\.DiscordGpui\.svg/,
      /\/app\/share\/mime\/packages\/com\.discord_lab\.DiscordGpui\.xml/,
    ]);
    targets.flatpak = { ok: flatpak.ok, manifest: flatpak };
  }

  if (platform === "all" || platform === "windows") {
    const wix = verifySpec("packaging/windows/wix.xml", [
      /InstallScope="perUser"/,
      /DiscordGpuiIcon\.ico/,
      /Software\\Classes\\discord/,
      /Software\\Classes\\disclip/,
      /\.discord-invite/,
      /application\/x-discord-gpui-invite/,
      /ApplicationStartMenuShortcut/,
    ]);
    targets["wix-msi"] = { ok: wix.ok, manifest: wix };
  }

  if (platform === "all" || platform === "macos") {
    const dmg = verifySpec("packaging/macos/build-dmg.sh", [
      /CFBundleIconFile/,
      /DiscordGPUI\.icns/,
      /CFBundleURLTypes/,
      /CFBundleDocumentTypes/,
      /application\/x-discord-gpui-invite/,
    ]);
    targets["macos-dmg"] = { ok: dmg.ok, script: dmg };
  }

  const ok = Object.values(targets).every((t) => Boolean(t.ok));
  return { ok, platform, targets };
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const verification = verifyAllTargets(args.platform);

  if (!args.verifyOnly && !args.dryRun && (args.platform === "all" || args.platform === "linux")) {
    const debVerify = spawnSync("node", ["packaging/deb/deb-package.mjs", "--verify"], {
      cwd: ROOT,
      encoding: "utf8",
    });
    if (debVerify.status !== 0) {
      verification.ok = false;
      verification.targets.deb.verifyError = debVerify.stderr || debVerify.stdout;
    }
  }

  if (args.reportJson) {
    mkdirSync(dirname(args.reportJson), { recursive: true });
    writeFileSync(args.reportJson, `${JSON.stringify(verification, null, 2)}\n`, "utf8");
  }

  if (!verification.ok) {
    console.error("package-dist: verification failed", JSON.stringify(verification, null, 2));
    process.exit(1);
  }
  console.log(`package-dist: verified targets [${Object.keys(verification.targets).join(", ")}]`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
