"""Build the real GUI, then package it with explicitly supplied official tools.

No downloads or simulated platform measurements occur here. AppImage contents use
the target host's ABI libraries; portable GTK/WebKit deployment is a separate job.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile

from verify import (EnvironmentFailure, IntegrityError, artifact_names, check_pinned_file,
                    command_output, read_sums, sha256_file, validate_elf, verify_release)

ROOT = Path(__file__).resolve().parents[2]
TARGET = "aarch64-unknown-linux-gnu"
CPUS = {"generic": "generic", "rpi5": "cortex-a76", "asahi": "apple-m1"}
C_ARCH = {"generic": "-march=armv8-a+simd+crc+crypto",
          "rpi5": "-mcpu=cortex-a76+crc+crypto", "asahi": "-mcpu=apple-m1"}
PACKAGES = ["alsa", "fontconfig", "wayland-client", "xkbcommon", "xkbcommon-x11", "xcb",
            "gtk+-3.0", "webkit2gtk-4.1", "glib-2.0", "gio-2.0", "gobject-2.0",
            "libsoup-3.0", "libsecret-1", "libpipewire-0.3", "enchant-2", "hunspell", "openssl"]
PIN_PATH = ROOT / "packaging/arm64/toolchain-pins.json"


class Parser(argparse.ArgumentParser):
    def error(self, message):
        raise IntegrityError(message)


def arguments():
    parser = Parser(prog="scripts/build-arm64.sh", description=__doc__,
                    epilog="Использование: scripts/build-arm64.sh [ОПЦИИ]")
    parser.add_argument("--output", "--out-dir", dest="output", type=Path, default=ROOT / "rust/dist")
    parser.add_argument("--version", default="1.0.0-beta.1")
    parser.add_argument("--target", choices=[TARGET], default=TARGET)
    parser.add_argument("--profile", choices=CPUS, default="generic")
    parser.add_argument("--cargo-config", type=Path, help="copy the configuration actually used after success")
    parser.add_argument("--report-json", type=Path)
    parser.add_argument("--runtime-file", default=os.environ.get("CHORD_ARM64_RUNTIME_FILE"),
                        help="local official ARM64 runtime matching toolchain-pins.json; never downloaded")
    parser.add_argument("--appimagetool", default=os.environ.get("CHORD_ARM64_APPIMAGETOOL"),
                        help="local pinned official AppImage for this build host; never downloaded")
    parser.add_argument("--sysroot", default=os.environ.get("CHORD_ARM64_SYSROOT"),
                        help="complete ARM64 Linux development sysroot; native ARM64 defaults to /")
    parser.add_argument("--pkg-config-libdir", default=os.environ.get("CHORD_ARM64_PKG_CONFIG_LIBDIR"),
                        help="colon-separated ARM64 pkg-config directories inside the sysroot")
    parser.add_argument("--verify", action="store_true")
    parser.add_argument("--dry-run", action="store_true", help="print a JSON plan without running builds")
    args = parser.parse_args()
    if not re.fullmatch(r"\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?", args.version):
        raise IntegrityError("--version must be SemVer")
    args.output = args.output.resolve()
    validate_output_paths(args)
    return args


def cargo_command(args, config, cargo=None):
    return [cargo or os.environ.get("CARGO", "cargo"), "build", "--locked", "--manifest-path",
            str(ROOT / "rust/chord-gpui/Cargo.toml"), "--package", "chord-gpui", "--release",
            "--target", TARGET, "--config", str(config), "--message-format=json-render-diagnostics", "--target-dir",
            str(Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "rust/target")).resolve())]


def build_plan(args):
    return {"dry_run": True, "target": TARGET, "profile": args.profile, "target_cpu": CPUS[args.profile],
            "cargo": cargo_command(args, "<generated Cargo configuration>"),
            "appimagetool": [args.appimagetool or "<pinned local appimagetool>", "--runtime-file",
                             args.runtime_file or "<pinned local ARM64 runtime>", "<AppDir>",
                             artifact_names(args.version)["appimage"]],
            "output": str(args.output), "native_validation": "not_run",
            "host_abi_requirements": PACKAGES, "self_contained": False}


def require_tool(name, command=None):
    found = shutil.which(command or name)
    if not found:
        raise EnvironmentFailure(f"{name} executable is unavailable: {command or name}")
    # Preserve argv[0]: cargo/rustc and clang++ may be symlinks to multicall tools.
    return os.path.abspath(found)


def required_tools():
    tools = {"Cargo": require_tool("Cargo", os.environ.get("CARGO", "cargo")),
             "rustc": require_tool("rustc", os.environ.get("RUSTC", "rustc"))}
    for tool in ("clang", "clang++", "ld.lld", "llvm-ar", "pkg-config", "node", "readelf",
                 "dpkg-deb", "unsquashfs", "ar", "tar", "gzip", "flock", "git"):
        tools[tool] = require_tool(tool)
    return tools


def runtime_file(args, pins):
    if not args.runtime_file:
        raise EnvironmentFailure("supply --runtime-file/CHORD_ARM64_RUNTIME_FILE with the pinned official runtime")
    path = Path(args.runtime_file).resolve()
    check_pinned_file(path, pins["runtime"])
    validate_elf(path, gui=False)
    return path


def appimagetool_file(args, pins):
    host = {"arm64": "aarch64", "AMD64": "x86_64"}.get(platform.machine(), platform.machine())
    pin = pins["appimagetool"]["assets"].get(host)
    if not pin or not args.appimagetool:
        raise EnvironmentFailure("supply --appimagetool/CHORD_ARM64_APPIMAGETOOL for x86_64 or aarch64 Linux")
    path = Path(args.appimagetool).resolve()
    check_pinned_file(path, pin)
    if not os.access(path, os.X_OK):
        raise EnvironmentFailure("pinned appimagetool must be executable")
    return path


def cross_environment(args, tools):
    native_arm = platform.machine() in ("aarch64", "arm64")
    if not args.sysroot and not native_arm:
        raise EnvironmentFailure("cross-build requires an explicit complete CHORD_ARM64_SYSROOT")
    sysroot = Path(args.sysroot or "/").resolve()
    if not (sysroot / "usr/include").is_dir():
        raise EnvironmentFailure(f"ARM64 sysroot development headers are unavailable: {sysroot}")
    directories = args.pkg_config_libdir or ":".join(str(sysroot / suffix) for suffix in (
        "usr/lib/aarch64-linux-gnu/pkgconfig", "usr/lib/pkgconfig", "usr/share/pkgconfig"))
    for directory in directories.split(":"):
        if not Path(directory).resolve().is_relative_to(sysroot):
            raise EnvironmentFailure("ARM64 pkg-config directories must stay within the target sysroot")
    flags = [f"--target={TARGET}", f"--sysroot={sysroot}", C_ARCH[args.profile], "-O2", "-fPIC"]
    values = {"CC_aarch64_unknown_linux_gnu": tools["clang"],
              "CXX_aarch64_unknown_linux_gnu": tools["clang++"],
              "AR_aarch64_unknown_linux_gnu": tools["llvm-ar"],
              "CFLAGS_aarch64_unknown_linux_gnu": shlex.join(flags),
              "CXXFLAGS_aarch64_unknown_linux_gnu": shlex.join(flags),
              "BINDGEN_EXTRA_CLANG_ARGS_aarch64_unknown_linux_gnu": shlex.join(flags[:2]),
              "PKG_CONFIG_ALLOW_CROSS": "1", "PKG_CONFIG_SYSROOT_DIR": str(sysroot),
              "PKG_CONFIG_LIBDIR": directories, "PKG_CONFIG_PATH": ""}
    return values, flags


def check_sdk(args, tools, env, flags, work):
    libdir = Path(command_output([tools["rustc"], "--print", "target-libdir", "--target", TARGET]).decode().strip())
    if not libdir.is_dir() or not list(libdir.glob("libstd-*.rlib")):
        raise EnvironmentFailure(f"Rust standard library for {TARGET} is unavailable; install it with rustup")
    result = subprocess.run([tools["pkg-config"], "--cflags", "--libs", *PACKAGES],
                            env=env, capture_output=True, text=True)
    if result.returncode:
        raise EnvironmentFailure(f"ARM64 development libraries are unavailable: {result.stderr.strip()}")
    source, binary = work / "sdk-probe.cpp", work / "sdk-probe"
    source.write_text('#include <string>\nint main() { return std::string("chord").empty(); }\n')
    result = subprocess.run([tools["clang++"], *flags, "-fuse-ld=lld", str(source), "-o", str(binary),
                             *shlex.split(result.stdout)], env=env, capture_output=True, text=True)
    if result.returncode:
        raise EnvironmentFailure(f"ARM64 C++ linker/sysroot probe failed: {result.stderr.strip()}")
    validate_elf(binary, gui=False)


def cargo_configuration(args, tools, values, flags):
    rustflags = []
    for flag in [*flags[:2], "-fuse-ld=lld", "-Wl,--build-id=sha1",
                 "-Wl,-z,relro,-z,now,-z,noexecstack",
                 "-Wl,-z,max-page-size=16384,-z,common-page-size=16384"]:
        rustflags.extend(["-C", f"link-arg={flag}"])
    rustflags.extend(["-C", "target-feature=+neon,+crc,+aes,+sha2", "-C", f"target-cpu={CPUS[args.profile]}"])
    lines = ["# Generated for an actual locked Chord GUI build.", "[build]", f"target = {json.dumps(TARGET)}",
             f"[target.{TARGET}]", f"linker = {json.dumps(tools['clang'])}",
             f"rustflags = {json.dumps(rustflags)}", "[env]"]
    lines.extend(f"{key} = {{ value = {json.dumps(value)}, force = true }}" for key, value in values.items())
    return "\n".join(lines) + "\n"


def assert_untracked(path):
    try:
        relative = path.resolve().relative_to(ROOT)
    except ValueError:
        return
    tracked = command_output(["git", "-C", ROOT, "ls-files", "-z", "--", relative])
    if tracked:
        raise EnvironmentFailure("refusing to overwrite tracked release/source oracles; use a fresh --out-dir")


def validate_output_paths(args):
    reserved = {str(args.output / name) for name in (*artifact_names(args.version).values(),
                "SHA256SUMS", "arm64-build-provenance.json", "cargo-config-aarch64.toml")}
    if args.report_json and str(args.report_json.resolve()) in reserved:
        raise IntegrityError("--report-json must not overwrite a release artifact")
    if args.cargo_config and str(args.cargo_config.resolve()) in reserved - {str(args.output / "cargo-config-aarch64.toml")}:
        raise IntegrityError("--cargo-config must not overwrite a release artifact")
    if args.report_json:
        assert_untracked(args.report_json)


def assert_publishable(args):
    for path in filter(None, [args.output, args.cargo_config]):
        assert_untracked(path)


def atomic_copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=destination.parent, prefix=".chord-", delete=False) as output:
        temporary = Path(output.name)
        try:
            with open(source, "rb") as stream:
                shutil.copyfileobj(stream, output)
            os.fchmod(output.fileno(), source.stat().st_mode & 0o777)
            output.close()
            temporary.replace(destination)
        finally:
            temporary.unlink(missing_ok=True)


def atomic_write(destination, text):
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", dir=destination.parent, prefix=".chord-", delete=False) as output:
        temporary = Path(output.name)
        try:
            output.write(text)
            os.fchmod(output.fileno(), 0o644)
            output.close()
            temporary.replace(destination)
        finally:
            temporary.unlink(missing_ok=True)


def package_tar(args, stage, epoch):
    name = f"discord-gpui-v{args.version}-linux-aarch64"
    source = stage / name
    source.mkdir()
    shutil.copy2(stage / artifact_names(args.version)["binary"], source / "discord-gpui")
    shutil.copy2(stage / "cargo-config-aarch64.toml", source)
    for document in ("LICENSE-MIT", "LICENSE-APACHE", "RELEASE-NOTES-v1.0.0-beta.1.md"):
        if (ROOT / document).is_file():
            shutil.copy2(ROOT / document, source)
    def normalize(member):
        member.uid = member.gid = 0
        member.uname = member.gname = "root"
        member.mtime = epoch
        return member
    with open(stage / artifact_names(args.version)["tarball"], "wb") as output:
        with gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=epoch) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as archive:
                archive.add(source, arcname=name, filter=normalize)


def package_deb(args, stage, tools):
    code = ('import {pathToFileURL} from "node:url"; '
            'const {buildDebPackage}=await import(pathToFileURL(process.argv[1]).href); '
            'buildDebPackage({debRootDir:process.argv[2],binaryPath:process.argv[3],'
            'outputDebPath:process.argv[4],version:process.argv[5],architecture:"arm64"});')
    names = artifact_names(args.version)
    version = re.sub(r"-beta\.(\d+)", r"~beta\1", args.version) + "-1"
    subprocess.run([tools["node"], "--input-type=module", "-e", code,
                    str(ROOT / "packaging/deb/deb-package.mjs"), str(ROOT / "packaging/deb"),
                    str(stage / names["binary"]), str(stage / names["deb"]), version], check=True, cwd=ROOT)


def package_appimage(args, stage, tool, runtime):
    appdir = stage / "DiscordGpui.AppDir"
    (appdir / "usr/bin").mkdir(parents=True)
    for source, destination in ((ROOT / "packaging/appimage/AppRun", appdir / "AppRun"),
            (stage / artifact_names(args.version)["binary"], appdir / "usr/bin/discord-gpui"),
            (ROOT / "packaging/appimage/discord-gpui.desktop", appdir / "discord-gpui.desktop"),
            (ROOT / "rust/gui-spike/icons/icon.png", appdir / "discord-gpui.png"),
            (ROOT / "rust/gui-spike/icons/icon.png", appdir / ".DirIcon")):
        shutil.copy2(source, destination)
    env = {**os.environ, "ARCH": "aarch64", "APPIMAGE_EXTRACT_AND_RUN": "1"}
    subprocess.run([str(tool), "--runtime-file", str(runtime), str(appdir),
                    str(stage / artifact_names(args.version)["appimage"])], env=env, check=True)


def source_identity():
    files = command_output(["git", "-C", ROOT, "ls-files", "-z", "--cached", "--others",
                            "--exclude-standard", "--", "rust"]).decode().split("\0")
    content = [(name, sha256_file(ROOT / name)) for name in sorted(set(files))
               if name.endswith((".rs", ".toml", ".lock")) and (ROOT / name).is_file()]
    return {"commit": command_output(["git", "-C", ROOT, "rev-parse", "HEAD"]).decode().strip(),
            "rust_sources_sha256": hashlib.sha256(json.dumps(content).encode()).hexdigest(),
            "cargo_lock_sha256": sha256_file(ROOT / "rust/chord-gpui/Cargo.lock"),
            "gpui_gitlink": command_output(["git", "-C", ROOT, "rev-parse", "HEAD:gpui-kit"]).decode().strip()}


def write_metadata(args, stage, command, identity, runtime, tool, artifact):
    binary = stage / artifact_names(args.version)["binary"]
    dynamic = command_output(["readelf", "--dynamic", binary]).decode()
    requirements = {"pkg_config_packages": PACKAGES, "bundled": False,
                    "elf_needed": re.findall(r"\(NEEDED\).*?\[(.*?)\]", dynamic)}
    provenance = {"schema_version": 1, "cargo_command": command, "serialized_with": "/tmp/chord-cargo.lock",
                  "source": identity, "binary_sha256": sha256_file(binary),
                  "cargo_reported_artifact": {key: artifact[key] for key in ("executable", "package_id", "fresh")},
                  "runtime_sha256": sha256_file(runtime), "appimagetool_sha256": sha256_file(tool),
                  "profile": args.profile, "target_cpu": CPUS[args.profile], "native_validation": "not_run",
                  "host_abi_requirements": requirements}
    (stage / "arm64-build-provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    files = [*artifact_names(args.version).values(), "arm64-build-provenance.json", "cargo-config-aarch64.toml"]
    (stage / "SHA256SUMS").write_text("".join(f"{sha256_file(stage / name)}  {name}\n" for name in files))


def publish(args, stage):
    entries = read_sums(stage / "SHA256SUMS")
    old_path = args.output / "SHA256SUMS"
    if old_path.is_file():
        for name, expected in read_sums(old_path).items():
            if name not in entries:
                path = args.output / name
                if not path.is_file() or sha256_file(path) != expected:
                    raise IntegrityError(f"existing checksum oracle is missing/corrupt: {name}")
                entries[name] = expected
    args.output.mkdir(parents=True, exist_ok=True)
    for name in read_sums(stage / "SHA256SUMS"):
        atomic_copy(stage / name, args.output / name)
    atomic_write(old_path, "".join(f"{digest}  {name}\n" for name, digest in entries.items()))
    if args.cargo_config and args.cargo_config.resolve() != args.output / "cargo-config-aarch64.toml":
        atomic_copy(stage / "cargo-config-aarch64.toml", args.cargo_config)


def run_cargo(command, tools, env, expected_binary, lock_file="/tmp/chord-cargo.lock"):
    artifact = None
    with subprocess.Popen([tools["flock"], str(lock_file), *command], env=env, cwd=ROOT,
                          stdout=subprocess.PIPE, text=True) as process:
        for line in process.stdout:
            try:
                event = json.loads(line)
            except ValueError:
                print(line, end="")
                continue
            if event.get("reason") == "compiler-message" and event.get("message", {}).get("rendered"):
                print(event["message"]["rendered"], end="", file=sys.stderr)
            if (event.get("reason") == "compiler-artifact" and event.get("target", {}).get("name") == "chord-gpui"
                    and "bin" in event["target"].get("kind", []) and event.get("executable")
                    and Path(event.get("manifest_path", "")).resolve() == ROOT / "rust/chord-gpui/Cargo.toml"
                    and Path(event["executable"]).resolve() == expected_binary.resolve()):
                artifact = event
        if process.wait() != 0:
            raise IntegrityError(f"locked Cargo build failed with exit {process.returncode}")
    if artifact is None:
        raise IntegrityError("Cargo did not report the expected chord-gpui compiler artifact")
    return artifact


def build_release(args, pins):
    tools = required_tools()
    runtime, tool = runtime_file(args, pins), appimagetool_file(args, pins)
    assert_publishable(args)
    overrides = [key for key in os.environ if key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")
                 or key.startswith("CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_")]
    if any(os.environ[key] for key in overrides):
        raise EnvironmentFailure("remove Rust target/linker overrides so the declared ARM64 profile is actually used")
    values, flags = cross_environment(args, tools)
    env = {**os.environ, **values, "LC_ALL": "C"}
    subprocess.run([tools["node"], str(ROOT / "scripts/bootstrap-gpui.mjs"), "--check"], check=True, cwd=ROOT)
    with tempfile.TemporaryDirectory(prefix="chord-real-arm64-") as temporary:
        stage = Path(temporary)
        check_sdk(args, tools, env, flags, stage)
        config = stage / "cargo-config-aarch64.toml"
        config.write_text(cargo_configuration(args, tools, values, flags))
        command = cargo_command(args, config, tools["Cargo"])
        identity = source_identity()
        binary = Path(command[-1]) / TARGET / "release/chord-gpui"
        artifact = run_cargo(command, tools, env, binary)
        if source_identity() != identity:
            raise IntegrityError("source/dependency identity changed during the locked build")
        validate_elf(binary)
        shutil.copy2(binary, stage / artifact_names(args.version)["binary"])
        epoch = int(os.environ.get("SOURCE_DATE_EPOCH") or command_output(
            [tools["git"], "-C", ROOT, "show", "-s", "--format=%ct", "HEAD"]).decode().strip())
        package_tar(args, stage, epoch)
        package_deb(args, stage, tools)
        package_appimage(args, stage, tool, runtime)
        write_metadata(args, stage, command, identity, runtime, tool, artifact)
        report = verify_release(stage, args.version, runtime, pins)
        publish(args, stage)
        return report


def main():
    try:
        if sys.version_info < (3, 11) or sys.platform != "linux":
            raise EnvironmentFailure("Linux and Python 3.11 or newer are required")
        args = arguments()
        if args.dry_run:
            print(json.dumps(build_plan(args), indent=2))
            return 0
        pins = json.loads(PIN_PATH.read_text())
        if args.verify:
            for tool in ("readelf", "dpkg-deb", "unsquashfs"):
                require_tool(tool)
            report = verify_release(args.output, args.version, runtime_file(args, pins), pins)
        else:
            report = build_release(args, pins)
        if args.report_json:
            atomic_write(args.report_json, json.dumps(report, indent=2) + "\n")
        print("build-arm64: genuine package integrity passed; native ARM64 runtime validation: not run")
        return 0
    except EnvironmentFailure as error:
        print(f"build-arm64: EnvFail: {error}", file=sys.stderr)
        return 2
    except (IntegrityError, OSError, ValueError, tarfile.TarError, subprocess.CalledProcessError) as error:
        print(f"build-arm64: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
