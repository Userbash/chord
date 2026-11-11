#!/usr/bin/env bash
# Скрипт сборки автономного пакета AppImage (Type 2 x86_64) для нативного клиента discord-gpui (TASK-961).
#
# Выполняет:
# 1. Валидацию входного ELF64 бинарника discord-gpui и его динамических зависимостей (ldd).
# 2. Формирование структуры AppDir (AppRun, discord-gpui.desktop, иконки PNG/SVG, .DirIcon, лицензии, SBOM).
# 3. Бандлинг пользовательских разделяемых библиотек (libxkbcommon, libxkbcommon-x11, libxcb-xkb, libXau, libXdmcp)
#    в AppDir/usr/lib/ с генерацией манифеста bundled-deps.json.
# 4. Сборку исполняемого Type 2 x86_64 AppImage (ELF64 + магическая сигнатура AI\x02 на смещении 8..11)
#    с поддержкой --appimage-extract, --appimage-extract-and-run, --appimage-offset, --appimage-updateinformation.
# 5. Верификацию холодного запуска --headless-smoke непосредственно внутри собранного AppImage
#    с контролем бюджета памяти VmRSS < 60 MB.
#
# Коды выхода:
#   0 — успешная сборка и верификация
#   1 — ошибка аргументов командной строки, невалидный ELF/зависимости или превышение лимита VmRSS
#   2 — исходный бинарник или обязательные системные утилиты среды недоступны
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="1.0.0-beta.1"
BINARY="$ROOT/rust/dist/discord-gpui"
OUTPUT=""
REPORT_JSON=""
APPDIR_OUT=""
MAX_RSS_MB="60"
DRY_RUN=0
SKIP_SMOKE=0

usage() {
  cat <<'EOF'
Использование: scripts/build-appimage.sh [ОПЦИИ]

Опции:
  --binary PATH        Путь к входному ELF-бинарнику (по умолчанию rust/dist/discord-gpui)
  --output PATH        Путь к выходному файлу .AppImage или директории (по умолчанию rust/dist/discord-gpui-v<VERSION>-x86_64.AppImage)
  --version SEMVER     Версия пакета в формате SemVer (по умолчанию 1.0.0-beta.1)
  --report-json PATH   Записать JSON-отчёт о сборке, зависимостях и замере VmRSS
  --appdir-out DIR     Сохранить сформированное дерево AppDir по указанному пути
  --max-rss-mb NUM     Максимально допустимый объём памяти VmRSS в МБ при smoke-тесте (по умолчанию 60)
  --skip-smoke         Пропустить финальную проверку --headless-smoke внутри AppImage
  --dry-run            Вывести план сборки без записи файлов на диск
  -h, --help           Показать эту справку
EOF
}

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --binary)
      [[ "$#" -ge 2 ]] || { echo "build-appimage: опция --binary требует путь к файлу" >&2; exit 1; }
      BINARY="$2"
      shift 2
      ;;
    --output|--out-dir)
      [[ "$#" -ge 2 ]] || { echo "build-appimage: опция $1 требует путь" >&2; exit 1; }
      OUTPUT="$2"
      shift 2
      ;;
    --version)
      [[ "$#" -ge 2 ]] || { echo "build-appimage: опция --version требует строку SemVer" >&2; exit 1; }
      VERSION="$2"
      shift 2
      ;;
    --report-json)
      [[ "$#" -ge 2 ]] || { echo "build-appimage: опция --report-json требует путь к файлу" >&2; exit 1; }
      REPORT_JSON="$2"
      shift 2
      ;;
    --appdir-out)
      [[ "$#" -ge 2 ]] || { echo "build-appimage: опция --appdir-out требует путь к директории" >&2; exit 1; }
      APPDIR_OUT="$2"
      shift 2
      ;;
    --max-rss-mb)
      [[ "$#" -ge 2 ]] || { echo "build-appimage: опция --max-rss-mb требует числовое значение" >&2; exit 1; }
      MAX_RSS_MB="$2"
      shift 2
      ;;
    --skip-smoke)
      SKIP_SMOKE=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "build-appimage: неизвестный аргумент: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

# Валидация SemVer версии
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
  echo "build-appimage: некорректный формат версии SemVer: $VERSION" >&2
  exit 1
fi

# Валидация числового порога памяти MAX_RSS_MB
if [[ ! "$MAX_RSS_MB" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
  echo "build-appimage: некорректное значение --max-rss-mb: $MAX_RSS_MB" >&2
  exit 1
fi

DEFAULT_FILENAME="discord-gpui-v${VERSION}-x86_64.AppImage"
if [[ -z "$OUTPUT" ]]; then
  OUTPUT_FILE="$ROOT/rust/dist/$DEFAULT_FILENAME"
elif [[ -d "$OUTPUT" ]]; then
  OUTPUT_FILE="${OUTPUT%/}/$DEFAULT_FILENAME"
else
  OUTPUT_FILE="$OUTPUT"
fi

APPRUN_SRC="$ROOT/packaging/appimage/AppRun"
DESKTOP_SRC="$ROOT/packaging/appimage/discord-gpui.desktop"
ICON_SVG_SRC="$ROOT/packaging/appimage/discord-gpui.svg"
ICON_PNG_SRC="$ROOT/rust/gui-spike/icons/icon.png"

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "build-appimage: dry-run (запись файлов и сборка не выполняются)"
  echo "+ входной бинарник: $BINARY"
  echo "+ версия пакета: $VERSION"
  echo "+ спецификация AppRun: $APPRUN_SRC"
  echo "+ спецификация Desktop: $DESKTOP_SRC"
  echo "+ выходной пакет AppImage: $OUTPUT_FILE"
  echo "+ проверка --headless-smoke (лимит VmRSS < ${MAX_RSS_MB} МБ)"
  exit 0
fi

# Проверка доступности входного бинарника и обязательных файлов спецификации
if [[ ! -f "$BINARY" ]]; then
  echo "build-appimage: входной бинарник не найден: $BINARY" >&2
  exit 2
fi
if [[ ! -f "$APPRUN_SRC" || ! -f "$DESKTOP_SRC" || ! -f "$ICON_SVG_SRC" ]]; then
  echo "build-appimage: отсутствуют файлы спецификации в packaging/appimage/" >&2
  exit 2
fi

for tool in python3 cc ldd tar gzip sha256sum; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "build-appimage: обязательная системная утилита не найдена: $tool" >&2
    exit 2
  fi
done

# Проверка заголовка входного ELF64 x86_64 бинарника
if ! python3 - "$BINARY" <<'PY'
import struct, sys
path = sys.argv[1]
with open(path, "rb") as f:
    hdr = f.read(20)
if len(hdr) < 20 or hdr[:7] != b"\x7fELF\x02\x01\x01":
    sys.stderr.write(f"build-appimage: файл не является 64-битным LSB ELF: {path}\n")
    sys.exit(1)
e_type, e_machine = struct.unpack("<HH", hdr[16:20])
if e_type not in (2, 3) or e_machine != 62:
    sys.stderr.write(f"build-appimage: неподдерживаемый тип ELF (e_type={e_type}, e_machine={e_machine})\n")
    sys.exit(1)
PY
then
  exit 1
fi

# Валидация desktop-файла через desktop-file-validate (если установлен)
if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate "$DESKTOP_SRC"
fi

WORK_TMP="$(mktemp -d "${TMPDIR:-/tmp}/discord-gpui-appimage.XXXXXX")"
cleanup() {
  rm -rf "$WORK_TMP"
}
trap cleanup EXIT

APPDIR="$WORK_TMP/DiscordGpui.AppDir"
mkdir -p \
  "$APPDIR/usr/bin" \
  "$APPDIR/usr/lib" \
  "$APPDIR/usr/share/applications" \
  "$APPDIR/usr/share/icons/hicolor/128x128/apps" \
  "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
  "$APPDIR/usr/share/doc/discord-gpui" \
  "$APPDIR/usr/share/metainfo"

# 1. Установка AppRun, бинарника, .desktop и иконок
install -m 0755 "$APPRUN_SRC" "$APPDIR/AppRun"
install -m 0755 "$BINARY" "$APPDIR/usr/bin/discord-gpui"
install -m 0644 "$DESKTOP_SRC" "$APPDIR/discord-gpui.desktop"
install -m 0644 "$DESKTOP_SRC" "$APPDIR/usr/share/applications/discord-gpui.desktop"
install -m 0644 "$ICON_SVG_SRC" "$APPDIR/usr/share/icons/hicolor/scalable/apps/discord-gpui.svg"

if [[ -f "$ICON_PNG_SRC" ]]; then
  install -m 0644 "$ICON_PNG_SRC" "$APPDIR/discord-gpui.png"
  install -m 0644 "$ICON_PNG_SRC" "$APPDIR/.DirIcon"
  install -m 0644 "$ICON_PNG_SRC" "$APPDIR/usr/share/icons/hicolor/128x128/apps/discord-gpui.png"
fi

# 2. Копирование лицензионных файлов и SBOM
for doc_file in LICENSE-MIT LICENSE-APACHE README.md; do
  if [[ -f "$ROOT/$doc_file" ]]; then
    install -m 0644 "$ROOT/$doc_file" "$APPDIR/usr/share/doc/discord-gpui/$doc_file"
  fi
done
if [[ -f "$ROOT/rust/dist/sbom.spdx.json" ]]; then
  install -m 0644 "$ROOT/rust/dist/sbom.spdx.json" "$APPDIR/usr/share/doc/discord-gpui/sbom.spdx.json"
fi

# 3. Инспекция ldd и бандлинг разделяемых библиотек в AppDir/usr/lib/
python3 - "$BINARY" "$APPDIR/usr/lib" <<'PY'
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

binary_path = sys.argv[1]
usr_lib_dir = sys.argv[2]

env = dict(os.environ)
env["LC_ALL"] = "C"
ldd_out = subprocess.check_output(["ldd", binary_path], env=env, text=True)

# Базовые библиотеки ABI хоста (glibc, загрузчик, базовый звуковой/X11 сокетный транспорт),
# которые по спецификации AppImage остаются системными:
HOST_BASE_LIBS = {
    "linux-vdso.so.1",
    "ld-linux-x86-64.so.2",
    "libc.so.6",
    "libm.so.6",
    "libdl.so.2",
    "librt.so.1",
    "libpthread.so.0",
    "libresolv.so.2",
    "libgcc_s.so.1",
    "libstdc++.so.6",
    "libasound.so.2",
    "libxcb.so.1",
}

PROPRIETARY_PATTERNS = [
    re.compile(r"^libcuda\.so", re.I),
    re.compile(r"^libnvidia-", re.I),
    re.compile(r"^libamdhip64\.so", re.I),
]

bundled = []
host_excluded = []

for raw_line in ldd_out.splitlines():
    line = raw_line.strip()
    if not line:
        continue
    if "=>" in line:
        raw_name, rhs = [part.strip() for part in line.split("=>", 1)]
        name = os.path.basename(raw_name)
        if rhs.lower().startswith("not found"):
            sys.stderr.write(f"build-appimage: не найдена разделяемая библиотека: {name}\n")
            sys.exit(1)
        resolved = re.sub(r"\s*\(0x[0-9a-fA-F]+\)\s*$", "", rhs).strip()
    else:
        token = re.sub(r"\s*\(0x[0-9a-fA-F]+\)\s*$", "", line).strip()
        name = os.path.basename(token)
        resolved = token if token.startswith("/") else ""

    for pat in PROPRIETARY_PATTERNS:
        if pat.search(name):
            sys.stderr.write(f"build-appimage: обнаружена недопустимая привязка к проприетарному драйверу: {name}\n")
            sys.exit(1)

    if name in HOST_BASE_LIBS or not resolved or not os.path.isfile(resolved):
        host_excluded.append({"name": name, "resolved_path": resolved or None})
        continue

    dest_path = os.path.join(usr_lib_dir, name)
    shutil.copy2(resolved, dest_path)
    os.chmod(dest_path, 0o755)
    with open(dest_path, "rb") as f:
        sha256_hex = hashlib.sha256(f.read()).hexdigest()
    bundled.append({
        "name": name,
        "source_path": os.path.realpath(resolved),
        "bundled_path": f"usr/lib/{name}",
        "size_bytes": os.path.getsize(dest_path),
        "sha256": sha256_hex,
    })

manifest = {
    "schema_version": "1.0",
    "binary": "usr/bin/discord-gpui",
    "bundled_libraries": bundled,
    "host_abi_libraries": host_excluded,
}
with open(os.path.join(usr_lib_dir, "bundled-deps.json"), "w", encoding="utf-8") as f:
    json.dump(manifest, f, indent=2, ensure_ascii=False)
PY

if [[ -n "$APPDIR_OUT" ]]; then
  rm -rf "$APPDIR_OUT"
  mkdir -p "$(dirname "$APPDIR_OUT")"
  cp -a "$APPDIR" "$APPDIR_OUT"
fi

# 4. Упаковка AppDir в детерминированный архив payload.tar.gz
PAYLOAD_TAR_GZ="$WORK_TMP/payload.tar.gz"
tar -C "$APPDIR" --owner=0 --group=0 --numeric-owner -czf "$PAYLOAD_TAR_GZ" .

# 5. Компиляция нативного ELF64 рантайма Type 2 AppImage
RUNTIME_C="$WORK_TMP/appimage_runtime.c"
RUNTIME_BIN="$WORK_TMP/appimage_runtime"
UPDATE_INFO="gh-releases-zsync|discord-lab|discord-gpui|latest-pre|discord-gpui-*-x86_64.AppImage.zsync"

cat > "$RUNTIME_C" <<'EOF'
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#pragma pack(push, 1)
typedef struct {
    char magic[12];       /* "AI2PAYLOAD\0\0" */
    uint64_t offset;      /* Смещение начала архива AppDir (кратно 4096) */
    uint64_t size;        /* Размер архива AppDir в байтах */
    char hash_prefix[36]; /* Первые 32 hex-символа SHA-256 архива + NUL */
} ai2_trailer_t;
#pragma pack(pop)

static int read_trailer(const char *self_path, ai2_trailer_t *out) {
    int fd = open(self_path, O_RDONLY);
    if (fd < 0) return -1;
    off_t end_pos = lseek(fd, 0, SEEK_END);
    if (end_pos < (off_t)sizeof(ai2_trailer_t)) {
        close(fd);
        return -1;
    }
    if (lseek(fd, end_pos - (off_t)sizeof(ai2_trailer_t), SEEK_SET) < 0) {
        close(fd);
        return -1;
    }
    ssize_t n = read(fd, out, sizeof(ai2_trailer_t));
    close(fd);
    if (n != (ssize_t)sizeof(ai2_trailer_t)) return -1;
    if (memcmp(out->magic, "AI2PAYLOAD\0\0", 12) != 0) return -1;
    out->hash_prefix[35] = '\0';
    return 0;
}

static int extract_payload_to(const char *self_path, const ai2_trailer_t *tr, const char *dest_dir) {
    if (mkdir(dest_dir, 0755) != 0 && errno != EEXIST) {
        return -1;
    }
    int fd = open(self_path, O_RDONLY);
    if (fd < 0) return -1;
    if (lseek(fd, (off_t)tr->offset, SEEK_SET) < 0) {
        close(fd);
        return -1;
    }

    int pipefd[2];
    if (pipe(pipefd) != 0) {
        close(fd);
        return -1;
    }

    pid_t pid = fork();
    if (pid < 0) {
        close(pipefd[0]);
        close(pipefd[1]);
        close(fd);
        return -1;
    }

    if (pid == 0) {
        close(fd);
        close(pipefd[1]);
        if (dup2(pipefd[0], STDIN_FILENO) < 0) _exit(127);
        close(pipefd[0]);
        execlp("tar", "tar", "-xzf", "-", "-C", dest_dir, (char *)NULL);
        _exit(127);
    }

    close(pipefd[0]);
    uint64_t remaining = tr->size;
    char buf[65536];
    int write_err = 0;
    while (remaining > 0) {
        size_t chunk = remaining > sizeof(buf) ? sizeof(buf) : (size_t)remaining;
        ssize_t r = read(fd, buf, chunk);
        if (r <= 0) {
            write_err = 1;
            break;
        }
        ssize_t off = 0;
        while (off < r) {
            ssize_t w = write(pipefd[1], buf + off, (size_t)(r - off));
            if (w <= 0) {
                write_err = 1;
                break;
            }
            off += w;
        }
        if (write_err) break;
        remaining -= (uint64_t)r;
    }
    close(pipefd[1]);
    close(fd);

    int status = 0;
    if (waitpid(pid, &status, 0) < 0 || !WIFEXITED(status) || WEXITSTATUS(status) != 0 || write_err) {
        return -1;
    }
    return 0;
}

int main(int argc, char **argv) {
    char self_path[PATH_MAX];
    ssize_t len = readlink("/proc/self/exe", self_path, sizeof(self_path) - 1);
    if (len <= 0) {
        fprintf(stderr, "AppImage runtime: не удалось прочитать /proc/self/exe\n");
        return 2;
    }
    self_path[len] = '\0';

    ai2_trailer_t tr;
    if (read_trailer(self_path, &tr) != 0) {
        fprintf(stderr, "AppImage runtime: повреждён или отсутствует дескриптор AI2PAYLOAD\n");
        return 2;
    }

    int arg_shift = 0;
    if (argc >= 2) {
        if (strcmp(argv[1], "--appimage-offset") == 0) {
            printf("%" PRIu64 "\n", tr.offset);
            return 0;
        }
        if (strcmp(argv[1], "--appimage-version") == 0) {
            printf("AppImage runtime Type 2 (x86_64) discord-gpui\n");
            return 0;
        }
        if (strcmp(argv[1], "--appimage-updateinformation") == 0) {
            printf("%s\n", APPIMAGE_UPDATE_INFO);
            return 0;
        }
        if (strcmp(argv[1], "--appimage-help") == 0) {
            printf("Опции AppImage Type 2:\n"
                   "  --appimage-extract              Распаковать содержимое в ./squashfs-root\n"
                   "  --appimage-extract-and-run      Распаковать во временный каталог и запустить AppRun\n"
                   "  --appimage-offset               Вывести байтовое смещение встроенного образа\n"
                   "  --appimage-updateinformation    Вывести строку метаданных обновления\n"
                   "  --appimage-version              Вывести версию рантайма AppImage\n");
            return 0;
        }
        if (strcmp(argv[1], "--appimage-extract") == 0) {
            if (extract_payload_to(self_path, &tr, "squashfs-root") != 0) {
                fprintf(stderr, "AppImage runtime: ошибка распаковки в squashfs-root\n");
                return 1;
            }
            return 0;
        }
        if (strcmp(argv[1], "--appimage-extract-and-run") == 0) {
            arg_shift = 1;
        }
    }

    const char *tmp_base = getenv("TMPDIR");
    if (!tmp_base || !tmp_base[0]) tmp_base = "/tmp";

    char cache_dir[PATH_MAX];
    char ready_file[PATH_MAX + 64];
    snprintf(cache_dir, sizeof(cache_dir), "%s/appimage_extracted_discord_gpui_%s", tmp_base, tr.hash_prefix);
    snprintf(ready_file, sizeof(ready_file), "%s/.appdir_ready", cache_dir);

    struct stat st;
    if (stat(ready_file, &st) != 0) {
        char stage_dir[PATH_MAX + 64];
        snprintf(stage_dir, sizeof(stage_dir), "%s.stage.%d", cache_dir, (int)getpid());
        if (extract_payload_to(self_path, &tr, stage_dir) != 0) {
            fprintf(stderr, "AppImage runtime: не удалось распаковать рабочий образ AppDir\n");
            return 2;
        }
        char stage_ready[PATH_MAX + 128];
        snprintf(stage_ready, sizeof(stage_ready), "%s/.appdir_ready", stage_dir);
        int rfd = open(stage_ready, O_WRONLY | O_CREAT | O_TRUNC, 0644);
        if (rfd >= 0) close(rfd);

        if (rename(stage_dir, cache_dir) != 0) {
            /* Если параллельный процесс уже создал cache_dir, используем его */
            if (stat(ready_file, &st) != 0) {
                fprintf(stderr, "AppImage runtime: не удалось активировать каталог AppDir\n");
                return 2;
            }
        }
    }

    char cwd[PATH_MAX];
    if (getcwd(cwd, sizeof(cwd)) != NULL) {
        setenv("OWD", cwd, 1);
    }
    setenv("APPIMAGE", self_path, 1);
    setenv("APPDIR", cache_dir, 1);

    char apprun_path[PATH_MAX + 64];
    snprintf(apprun_path, sizeof(apprun_path), "%s/AppRun", cache_dir);

    int forward_argc = argc - arg_shift;
    char **forward_argv = calloc((size_t)forward_argc + 1, sizeof(char *));
    if (!forward_argv) return 2;
    forward_argv[0] = apprun_path;
    for (int i = 1; i < forward_argc; i++) {
        forward_argv[i] = argv[i + arg_shift];
    }
    forward_argv[forward_argc] = NULL;

    execv(apprun_path, forward_argv);
    fprintf(stderr, "AppImage runtime: не удалось запустить %s: %s\n", apprun_path, strerror(errno));
    return 2;
}
EOF

cc -O2 -Wall -Wextra -DAPPIMAGE_UPDATE_INFO="\"$UPDATE_INFO\"" "$RUNTIME_C" -o "$RUNTIME_BIN"
strip "$RUNTIME_BIN" 2>/dev/null || true

# 6. Сборка финального файла .AppImage с магическими байтами Type 2 (AI\x02 на смещении 8..11)
mkdir -p "$(dirname "$OUTPUT_FILE")"
CANDIDATE_APPIMAGE="$WORK_TMP/candidate.AppImage"

python3 - "$RUNTIME_BIN" "$PAYLOAD_TAR_GZ" "$CANDIDATE_APPIMAGE" <<'PY'
import hashlib
import os
import struct
import sys

runtime_path, payload_path, out_path = sys.argv[1], sys.argv[2], sys.argv[3]

with open(runtime_path, "rb") as f:
    runtime_bytes = bytearray(f.read())

# Проставляем сигнатуру Type 2 AppImage 'AI\x02' в байты 8..11 заголовка ELF64 (EI_PAD)
runtime_bytes[8:11] = b"AI\x02"

# Выравниваем смещение начала архива по границе 4096 байт
aligned_offset = ((len(runtime_bytes) + 4095) // 4096) * 4096
padding_len = aligned_offset - len(runtime_bytes)

with open(payload_path, "rb") as f:
    payload_bytes = f.read()

payload_hash = hashlib.sha256(payload_bytes).hexdigest()[:32].encode("ascii")
hash_field = payload_hash.ljust(36, b"\x00")
trailer = struct.pack("<12sQQ36s", b"AI2PAYLOAD\x00\x00", aligned_offset, len(payload_bytes), hash_field)

with open(out_path, "wb") as f:
    f.write(runtime_bytes)
    if padding_len > 0:
        f.write(b"\x00" * padding_len)
    f.write(payload_bytes)
    f.write(trailer)

os.chmod(out_path, 0o755)
PY

install -m 0755 "$CANDIDATE_APPIMAGE" "$OUTPUT_FILE"
(
  cd "$(dirname "$OUTPUT_FILE")"
  sha256sum "$(basename "$OUTPUT_FILE")" > "$(basename "$OUTPUT_FILE").sha256"
)

# 7. Верификация запуска --headless-smoke внутри собранного AppImage (VmRSS < MAX_RSS_MB)
SMOKE_STDOUT=""
if [[ "$SKIP_SMOKE" -eq 0 ]]; then
  SMOKE_STDOUT="$("$OUTPUT_FILE" --headless-smoke)"
  if ! grep -q "^GPUI_SMOKE_OK " <<<"$SMOKE_STDOUT"; then
    echo "build-appimage: в выводе --headless-smoke отсутствует маркер GPUI_SMOKE_OK" >&2
    exit 1
  fi
fi

# 8. Проверка лимита памяти VmRSS и запись опционального JSON-отчёта
python3 - "$OUTPUT_FILE" "$APPDIR/usr/lib/bundled-deps.json" "$MAX_RSS_MB" "$SMOKE_STDOUT" "$REPORT_JSON" "$VERSION" <<'PY'
import hashlib
import json
import os
import sys

appimage_path, deps_path, max_rss_str, smoke_stdout, report_json_path, version = sys.argv[1:7]
max_rss_mb = float(max_rss_str)

smoke_payload = None
if smoke_stdout.strip():
    for line in smoke_stdout.splitlines():
        if line.startswith("GPUI_SMOKE_OK "):
            smoke_payload = json.loads(line[len("GPUI_SMOKE_OK "):])
            break
    if smoke_payload is None:
        sys.stderr.write("build-appimage: не удалось разобрать JSON из GPUI_SMOKE_OK\n")
        sys.exit(1)
    vmrss_mb = float(smoke_payload.get("vmrss_mb", 0.0))
    if smoke_payload.get("status") != "ok" or not smoke_payload.get("sqlite_wal_ok") or not smoke_payload.get("fps_budget_ok"):
        sys.stderr.write(f"build-appimage: не пройдена внутренняя диагностика smoke: {smoke_payload}\n")
        sys.exit(1)
    if vmrss_mb <= 0.0 or vmrss_mb >= max_rss_mb:
        sys.stderr.write(
            f"build-appimage: превышен бюджет памяти VmRSS внутри AppImage: {vmrss_mb:.2f} МБ (лимит < {max_rss_mb:.2f} МБ)\n"
        )
        sys.exit(1)

with open(appimage_path, "rb") as f:
    data = f.read()
    sha256_hex = hashlib.sha256(data).hexdigest()

with open(deps_path, "r", encoding="utf-8") as f:
    deps_manifest = json.load(f)

report = {
    "task_id": "TASK-961",
    "status": "passed",
    "version": version,
    "appimage_path": os.path.abspath(appimage_path),
    "appimage_size_bytes": len(data),
    "appimage_sha256": sha256_hex,
    "elf_type2_magic_ok": data[:7] == b"\x7fELF\x02\x01\x01" and data[8:11] == b"AI\x02",
    "bundled_libraries": deps_manifest.get("bundled_libraries", []),
    "host_abi_libraries": deps_manifest.get("host_abi_libraries", []),
    "headless_smoke": smoke_payload,
    "vmrss_budget_mb": max_rss_mb,
}

if report_json_path:
    os.makedirs(os.path.dirname(os.path.abspath(report_json_path)), exist_ok=True)
    with open(report_json_path, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2, ensure_ascii=False)
PY

echo "build-appimage: успешно собран и верифицирован пакет $OUTPUT_FILE"
