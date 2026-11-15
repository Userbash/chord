"""Independent ELF/container/content checks; never execute the target GUI."""
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import tarfile


class IntegrityError(Exception):
    """An existing artifact or tool does not satisfy its content contract."""


class EnvironmentFailure(Exception):
    """A real prerequisite is unavailable; no success may be reported."""


def sha256_file(path):
    with open(path, "rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def check_pinned_file(path, pin):
    path = Path(path)
    if not path.is_file():
        raise EnvironmentFailure(f"required official file is unavailable: {path}")
    if sha256_file(path) != pin["sha256"] or path.stat().st_size != pin["size"]:
        raise IntegrityError(f"official file SHA-256/size mismatch: {path}")


def command_output(command, **kwargs):
    result = subprocess.run([str(arg) for arg in command], check=False,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, **kwargs)
    if result.returncode:
        raise IntegrityError(f"{command[0]} failed: {result.stderr.decode(errors='replace')}")
    return result.stdout


def elf_headers(path):
    with open(path, "rb") as stream:
        header = stream.read(64)
        if len(header) != 64 or header[:7] != b"\x7fELF\x02\x01\x01":
            raise IntegrityError(f"invalid or truncated ELF64 LSB: {path}")
        if struct.unpack_from("<HH", header, 16) not in ((2, 183), (3, 183)):
            raise IntegrityError(f"ELF must use EM_AARCH64=183: {path}")
        if struct.unpack_from("<I", header, 20)[0] != 1 or struct.unpack_from("<H", header, 52)[0] != 64:
            raise IntegrityError(f"invalid ELF header version/size: {path}")
        offset = struct.unpack_from("<Q", header, 32)[0]
        width, count = struct.unpack_from("<HH", header, 54)
        size = os.fstat(stream.fileno()).st_size
        if width != 56 or not count or offset < 64 or offset + width * count > size:
            raise IntegrityError(f"invalid ELF program headers: {path}")
        stream.seek(offset)
        segments = [struct.unpack("<IIQQQQQQ", stream.read(56)) for _ in range(count)]
        if any(segment[0] != 0 and segment[2] + segment[5] > size for segment in segments):
            raise IntegrityError(f"truncated ELF segment: {path}")
        validate_load_segments(segments)
        return segments


def validate_load_segments(segments):
    """Check gABI load ranges for both the GUI and the official runtime."""
    previous = -1
    for kind, _, offset, address, _, file_size, memory_size, alignment in segments:
        if kind != 1:
            continue
        if file_size > memory_size:
            raise IntegrityError("ELF PT_LOAD file size exceeds memory size")
        if memory_size > (1 << 64) - 1 - address:
            raise IntegrityError("ELF PT_LOAD address range overflows ELF64")
        # Linux ARM64 base pages are multiples of 4 KiB, even when p_align is 0/1.
        # This is a minimum loader constraint, not a measured native page size.
        if (address - offset) % 4096:
            raise IntegrityError("ELF PT_LOAD address and offset must be congruent to 4 KiB")
        if alignment > 1:
            if alignment & (alignment - 1):
                raise IntegrityError("ELF PT_LOAD alignment must be a power of two")
            if (address - offset) % alignment:
                raise IntegrityError("ELF PT_LOAD address and offset must be congruent")
        if address < previous:
            raise IntegrityError("ELF PT_LOAD addresses must be in ascending order")
        previous = address
    if previous == -1:
        raise IntegrityError("ELF executable requires a PT_LOAD segment")


def validate_elf(path, gui=True):
    segments = elf_headers(path)
    if not gui:
        return {"elf_class": "ELF64", "elf_machine_id": 183}
    loads = [segment[7] for segment in segments if segment[0] == 1]
    if not loads or any(align < 16384 or align % 16384 for align in loads):
        raise IntegrityError("ELF PT_LOAD segments must be aligned to 16 KiB")
    interpreters = [segment for segment in segments if segment[0] == 3]
    if len(interpreters) != 1:
        raise IntegrityError("ELF must contain one PT_INTERP")
    with open(path, "rb") as stream:
        stream.seek(interpreters[0][2])
        interpreter = stream.read(interpreters[0][5])
    if interpreter != b"/lib/ld-linux-aarch64.so.1\0":
        raise IntegrityError(f"unexpected ELF interpreter: {interpreter!r}")
    if segments.index(interpreters[0]) > next(index for index, segment in enumerate(segments) if segment[0] == 1):
        raise IntegrityError("ELF PT_INTERP must precede PT_LOAD segments")
    stack = [segment for segment in segments if segment[0] == 0x6474E551]
    relro = any(segment[0] == 0x6474E552 and segment[6] for segment in segments)
    if not relro or len(stack) != 1 or stack[0][1] & 1:
        raise IntegrityError("ELF requires GNU_RELRO and a non-executable GNU_STACK")
    details = command_output(["readelf", "--wide", "--dynamic", "--notes", path]).decode()
    if not re.search(r"BIND_NOW|FLAGS[^\n]*NOW", details) or "Build ID:" not in details:
        raise IntegrityError("ELF requires BIND_NOW and a linker Build ID")
    return {"elf_class": "ELF64", "elf_machine": "EM_AARCH64", "elf_machine_id": 183,
            "interpreter": interpreter[:-1].decode("ascii"), "load_segment_alignment_bytes": min(loads),
            "kernel_page_size_bytes": None, "full_relro": True}


def md5_section(runtime):
    offset = struct.unpack_from("<Q", runtime, 40)[0]
    width, count, strings_index = struct.unpack_from("<HHH", runtime, 58)
    if width != 64 or not 0 < strings_index < count or offset + width * count > len(runtime):
        raise IntegrityError("official runtime has invalid ELF section headers")
    sections = [struct.unpack_from("<IIQQQQIIQQ", runtime, offset + i * width) for i in range(count)]
    strings = sections[strings_index]
    names = runtime[strings[4]:strings[4] + strings[5]]
    for section in sections:
        name = names[section[0]:].split(b"\0", 1)[0]
        if name == b".digest_md5" and section[5] >= 16 and section[4] + 16 <= len(runtime):
            return section[4]
    raise IntegrityError("official runtime has no valid .digest_md5 section")


def hash_command(command):
    with subprocess.Popen([str(arg) for arg in command], stdout=subprocess.PIPE,
                          stderr=subprocess.DEVNULL) as process:
        digest = hashlib.file_digest(process.stdout, "sha256").hexdigest()
        if process.wait() != 0:
            raise IntegrityError(f"container extraction failed: {command[0]}")
        return digest


def appimage_binary_hash(path, runtime_path):
    runtime = bytearray(Path(runtime_path).read_bytes())
    with open(path, "rb") as stream:
        prefix = bytearray(stream.read(len(runtime)))
    validate_elf(path, gui=False)
    if len(prefix) != len(runtime) or prefix[8:11] != b"AI\x02":
        raise IntegrityError("invalid Type 2 AppImage header")
    # Official appimagetool changes exactly this MD5 field when embedding a runtime.
    digest_offset = md5_section(runtime)
    prefix[digest_offset:digest_offset + 16] = b"\0" * 16
    runtime[digest_offset:digest_offset + 16] = b"\0" * 16
    if prefix != runtime:
        raise IntegrityError("AppImage executable differs from the pinned official runtime")
    command_output(["unsquashfs", "-s", "-o", len(runtime), path])
    return hash_command(["unsquashfs", "-cat", "-o", len(runtime), path, "usr/bin/discord-gpui"])


def tar_binary_hash(archive, expected_name):
    matches = [member for member in archive if member.name.lstrip("./") == expected_name]
    if len(matches) != 1 or not matches[0].isfile() or not matches[0].mode & 0o111:
        raise IntegrityError(f"container must contain one executable {expected_name}")
    with archive.extractfile(matches[0]) as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def deb_binary_hash(path):
    architecture = command_output(["dpkg-deb", "--field", path, "Architecture"]).strip()
    if architecture != b"arm64":
        raise IntegrityError("Debian package Architecture must be arm64")
    with subprocess.Popen(["dpkg-deb", "--fsys-tarfile", str(path)],
                          stdout=subprocess.PIPE, stderr=subprocess.DEVNULL) as process:
        with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
            digest = None
            for member in archive:
                if member.name.lstrip("./") == "usr/bin/discord-gpui":
                    if digest is not None or not member.isfile() or not member.mode & 0o111:
                        raise IntegrityError("invalid Debian executable entry")
                    digest = hashlib.file_digest(archive.extractfile(member), "sha256").hexdigest()
        if process.wait() != 0 or digest is None:
            raise IntegrityError("Debian executable extraction failed")
        return digest


def artifact_names(version):
    return {"binary": "discord-gpui-aarch64",
            "tarball": f"discord-gpui-v{version}-linux-aarch64.tar.gz",
            "deb": f"discord-gpui_{version}_arm64.deb",
            "appimage": f"discord-gpui-v{version}-aarch64.AppImage"}


def read_sums(path):
    entries = {}
    for line in path.read_text().splitlines():
        match = re.fullmatch(r"([0-9a-f]{64})  ([^/\\]+)", line)
        if not match or match[2] in entries or match[2] in (".", ".."):
            raise IntegrityError("invalid or duplicate SHA256SUMS entry")
        entries[match[2]] = match[1]
    return entries


def verify_checksums(directory, required):
    entries = read_sums(directory / "SHA256SUMS")
    if any(name not in entries for name in required):
        raise IntegrityError("required ARM64 output is absent from SHA256SUMS")
    for name, digest in entries.items():
        path = directory / name
        if not path.is_file() or sha256_file(path) != digest:
            raise IntegrityError(f"SHA-256 mismatch: {name}")
    return entries


def verify_release(directory, version, runtime, pins):
    names = artifact_names(version)
    required = [*names.values(), "SHA256SUMS", "arm64-build-provenance.json", "cargo-config-aarch64.toml"]
    if any(not (directory / name).is_file() for name in required):
        raise EnvironmentFailure("required ARM64 release files/provenance are unavailable")
    check_pinned_file(runtime, pins["runtime"])
    sums = verify_checksums(directory, [name for name in required if name != "SHA256SUMS"])
    result = validate_elf(directory / names["binary"])
    digest = sums[names["binary"]]
    with tarfile.open(directory / names["tarball"], "r:gz") as archive:
        tar_hash = tar_binary_hash(archive, f"discord-gpui-v{version}-linux-aarch64/discord-gpui")
    if any(value != digest for value in (tar_hash, deb_binary_hash(directory / names["deb"]),
            appimage_binary_hash(directory / names["appimage"], runtime))):
        raise IntegrityError("packaged executable differs from the Cargo output")
    provenance = json.loads((directory / "arm64-build-provenance.json").read_text())
    command = provenance.get("cargo_command", [])
    if not all(arg in command for arg in ("build", "--locked", "chord-gpui", "aarch64-unknown-linux-gnu")):
        raise IntegrityError("provenance lacks the locked Chord GUI build command")
    if provenance.get("binary_sha256") != digest or provenance.get("runtime_sha256") != pins["runtime"]["sha256"]:
        raise IntegrityError("provenance does not match binary/runtime content")
    return {**result, "status": "passed", "packaging_integrity": "passed", "native_validation": "not_run",
            "version": version, "target": "aarch64-unknown-linux-gnu", "artifacts": {
                key: {"filename": name, "size_bytes": (directory / name).stat().st_size,
                      "sha256": sums[name]} for key, name in names.items()},
            "host_abi_requirements": provenance["host_abi_requirements"]}
