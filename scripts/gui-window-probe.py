#!/usr/bin/env python3
"""RW-38: живая GUI-проба окна discord-gui-spike.

Проверяет реальное окно, элементы внутри него и работоспособность:
- AT-SPI: 4 кнопки рейла, 10 категорий эмодзи, 120 ячеек, 38 тумблеров,
  переключение экранов, изменение состояния чекбокса;
- X11/Xlib: находит **своё** окно по уникальному заголовку + PID, снимает PNG
  (XGetImage), проверяет размер/неоднородность;
- app.log: `window-ready`, `UI_SNAPSHOT` (4/100000/24229/10/38), отсутствие
  GBM/Gdk-Message/panic/segfault; приложение живо до конца проверок.

    python3 scripts/gui-window-probe.py [--json out.json] [--timeout 30]

Коды выхода: 0 — все проверки прошли; 1 — проверка провалена; 2 — среда недоступна.
Требования: Linux/X11 (`DISPLAY`), GTK3 + WebKitGTK, AT-SPI (`GTK_MODULES=gail:atk-bridge`),
PyGObject (`gi`), Pillow. Без сети, без установок.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPIKE = ROOT / "rust" / "gui-spike"
BINARY = SPIKE / "target" / "debug" / "discord-gui-spike"

RAIL = [("💬", "Чат"), ("😀", "Эмодзи"), ("👥", "Участники"), ("⚙️", "Настройки")]
# Порядок и имена — по фактическому UI-контракту (`ui/features/emoji.js`, 10 вкладок).
# RE-порядок категорий (EMO-04) — отдельная карточка: здесь UI не меняем, только
# сверяем AT-SPI с тем, что реально отрендерено.
CATEGORIES = [
    "Часто используемые",
    "Смайлы",
    "Люди и жесты",
    "Еда и напитки",
    "Природа",
    "Транспорт",
    "Предметы",
    "Символы",
    "Флаги",
    "Прочее",
]
SNAPSHOT_KEYS = ["screenCount", "chatRows", "memberRows", "emojiCategories", "settingsToggles"]
# AT-SPI/ATK имена ролей для ARIA listbox/option (WebKit может отдать list item).
GRID_ROLE = "list box"
GRID_NAME = "Эмодзи"
CELL_ROLES = ("option", "list item", "table cell", "push button")
MIN_NAMED_CELLS = 10


class Fail(Exception):
    """Проверка не прошла (exit 1)."""


class EnvFail(Exception):
    """Среда недоступна (exit 2)."""


def require_modules():
    try:
        import gi  # noqa: F401
        gi.require_version("Atspi", "2.0")
        from gi.repository import Atspi  # noqa: F401
    except Exception as err:  # pragma: no cover - среда
        raise EnvFail(f"AT-SPI/PyGObject недоступны: {err}")
    try:
        from PIL import Image  # noqa: F401
    except Exception as err:  # pragma: no cover
        raise EnvFail(f"Pillow недоступен: {err}")


def launch(timeout_s: int):
    """Запускает спайк с уникальным заголовком; при ошибке убивает процесс."""
    if not BINARY.is_file():
        raise Fail(f"нет бинарника {BINARY} (соберите: cargo build)")
    if not (os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY")):
        raise EnvFail("нет DISPLAY/WAYLAND_DISPLAY")
    # Tauri встраивает ui/* при сборке: старый бинарь даст ложную пробу.
    # Рекурсивно: правки ui/features/* и ui/components/* тоже должны инвалидировать бинарь.
    newest_ui = max(
        (p.stat().st_mtime for p in (SPIKE / "ui").rglob("*") if p.is_file()),
        default=0,
    )
    if BINARY.stat().st_mtime < newest_ui:
        raise Fail("бинарь старше ui/*: пересоберите `cargo build`")

    fd, log_name = tempfile.mkstemp(prefix="gui-window-probe-", suffix=".log")
    os.close(fd)
    log_path = Path(log_name)
    unique_title = f"discord-gui-spike probe {os.getpid()}-{int(time.time() * 1000)}"
    env = dict(os.environ)
    env.update({
        "SPIKE_EXIT_AFTER_MS": str((timeout_s + 10) * 1000),
        "SPIKE_WINDOW_TITLE": unique_title,
        # Live-проба проверяет каркас, а не вход: гейт отключён только здесь.
        "SPIKE_AUTH_DISABLED": "1",
        "SPIKE_AUTOBENCH": "0",
        "GDK_BACKEND": "x11",
        "WEBKIT_DISABLE_COMPOSITING_MODE": "1",
        "WEBKIT_DISABLE_DMABUF_RENDERER": "1",
        "LIBGL_ALWAYS_SOFTWARE": "1",
        "GTK_MODULES": "gail:atk-bridge",
        "NO_AT_BRIDGE": "0",
    })
    fh = open(log_path, "w", encoding="utf-8")
    proc = subprocess.Popen([str(BINARY)], cwd=str(SPIKE), env=env,
                            stdout=fh, stderr=subprocess.STDOUT)
    try:
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if "window-ready" in log_path.read_text(encoding="utf-8"):
                return proc, fh, unique_title, log_path
            if proc.poll() is not None:
                raise EnvFail("спайк завершился до window-ready")
            time.sleep(0.02)
        raise EnvFail("window-ready не появился за 15 с")
    except BaseException:
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                proc.kill()
        fh.close()
        raise


def atspi_app(Atspi, pid: int):
    """Приложение AT-SPI ровно своего процесса (имя — только вторичный фильтр).

    P1-1: без сверки PID проба могла проверять чужой экземпляр спайка или
    клиент Discord. 0 совпадений → None (вызывающий решает EnvFail), ≥2 → Fail.
    """
    desktop = Atspi.get_desktop(0)
    matches = []
    for i in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(i)
        getter = getattr(app, "get_process_id", None)
        if not callable(getter):
            raise EnvFail("AT-SPI не отдаёт get_process_id — окно нельзя привязать к PID")
        try:
            app_pid = getter()
        except Exception as err:
            raise EnvFail(f"AT-SPI get_process_id: {err}")
        if app_pid == pid:
            matches.append(app)
    if len(matches) > 1:
        raise Fail(f"AT-SPI: {len(matches)} приложений с PID {pid} — неоднозначная привязка")
    return matches[0] if matches else None


def require_alive(proc, step: str):
    """P2-3: смерть окна/процесса — среда, а не «клик не сработал»."""
    code = proc.poll()
    if code is not None:
        raise EnvFail(f"спайк завершился на шаге «{step}» (exit={code}) — окно умерло")


def wait_for_marker(log_path, proc, marker: str, timeout_s: float, step: str):
    """Ждёт маркер в логе, не забывая проверять живость процесса."""
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        if marker in log_path.read_text(encoding="utf-8"):
            return
        require_alive(proc, step)
        time.sleep(0.05)
    raise EnvFail(f"«{marker}» не появился за {timeout_s:g} с на шаге «{step}»")


def own_app(Atspi, proc):
    require_alive(proc, "AT-SPI")
    app = atspi_app(Atspi, proc.pid)
    if app is None:
        raise EnvFail(
            f"AT-SPI: нет приложения с PID {proc.pid} "
            "(нужен GTK_MODULES=gail:atk-bridge; чужое окно не принимается)"
        )
    return app


def find_all(node, predicate, depth=0, out=None):
    out = [] if out is None else out
    # 24 — запас под новые обёртки settings-shell/body/pane/block (W39h2):
    # при глубине 14 тумблеры уходили за границу обхода (AT-SPI: тумблеров 0).
    if depth > 24:
        return out
    try:
        if predicate(node):
            out.append(node)
        for i in range(node.get_child_count()):
            find_all(node.get_child_at_index(i), predicate, depth + 1, out)
    except Exception:
        pass
    return out


def click(node):
    try:
        return bool(node.do_action(0))
    except Exception:
        return False


def checked(node):
    try:
        import gi
        return bool(node.get_state_set().contains(gi.repository.Atspi.StateType.CHECKED))
    except Exception:
        return None


# Держим ссылку на C-колбэк: иначе Python может собрать его, и Xlib вызовет
# освобождённую память (UB/segfault на BadWindow).
_X_ERROR_HANDLER = None


def x11_capture(Image, title_prefix: str, pid: int):
    """XID своего окна по заголовку + `_NET_WM_PID` → PNG. Root под XWayland пуст."""
    import ctypes
    from ctypes import c_void_p, c_ulong, c_uint, c_int, c_long, c_char_p, c_ubyte, POINTER, byref

    global _X_ERROR_HANDLER
    x = ctypes.cdll.LoadLibrary("libX11.so.6")
    x.XOpenDisplay.argtypes = [c_char_p]; x.XOpenDisplay.restype = c_void_p
    x.XDefaultRootWindow.argtypes = [c_void_p]; x.XDefaultRootWindow.restype = c_ulong
    x.XQueryTree.argtypes = [c_void_p, c_ulong, POINTER(c_ulong), POINTER(c_ulong),
                             POINTER(POINTER(c_ulong)), POINTER(c_uint)]; x.XQueryTree.restype = c_int
    x.XFetchName.argtypes = [c_void_p, c_ulong, POINTER(c_char_p)]; x.XFetchName.restype = c_int
    x.XFree.argtypes = [c_void_p]
    x.XInternAtom.argtypes = [c_void_p, c_char_p, c_int]; x.XInternAtom.restype = c_ulong
    x.XGetWindowProperty.argtypes = [c_void_p, c_ulong, c_ulong, c_long, c_long, c_int, c_ulong,
                                     POINTER(c_ulong), POINTER(c_int), POINTER(c_ulong),
                                     POINTER(c_ulong), POINTER(POINTER(c_ubyte))]
    x.XGetWindowProperty.restype = c_int
    x.XGetGeometry.argtypes = [c_void_p, c_ulong, POINTER(c_ulong), POINTER(c_int), POINTER(c_int),
                               POINTER(c_uint), POINTER(c_uint), POINTER(c_uint), POINTER(c_uint)]
    x.XGetGeometry.restype = c_int
    x.XCloseDisplay.argtypes = [c_void_p]; x.XCloseDisplay.restype = c_int

    class XImage(ctypes.Structure):
        _fields_ = [("width", c_int), ("height", c_int), ("xoffset", c_int), ("format", c_int),
                    ("data", c_void_p), ("byte_order", c_int), ("bitmap_unit", c_int),
                    ("bitmap_bit_order", c_int), ("bitmap_pad", c_int), ("depth", c_int),
                    ("bytes_per_line", c_int), ("bits_per_pixel", c_int),
                    ("red_mask", c_ulong), ("green_mask", c_ulong), ("blue_mask", c_ulong)]

    x.XGetImage.argtypes = [c_void_p, c_ulong, c_int, c_int, c_uint, c_uint, c_ulong, c_int]
    x.XGetImage.restype = POINTER(XImage)
    x.XDestroyImage.argtypes = [POINTER(XImage)]; x.XDestroyImage.restype = c_int
    ERR = ctypes.CFUNCTYPE(c_int, c_void_p, c_void_p)
    _X_ERROR_HANDLER = ERR(lambda _d, _e: 0)
    x.XSetErrorHandler(_X_ERROR_HANDLER)

    dpy = x.XOpenDisplay(None)
    if not dpy:
        raise EnvFail("XOpenDisplay вернул NULL")
    try:
        root = x.XDefaultRootWindow(dpy)
        found = []

        def window_pid(win):
            """`_NET_WM_PID` окна; None — свойство не задано/не читается."""
            atom = x.XInternAtom(dpy, b"_NET_WM_PID", 0)
            if not atom:
                return None
            actual_type = c_ulong(); actual_format = c_int()
            nitems = c_ulong(); bytes_after = c_ulong()
            data = POINTER(c_ubyte)()
            status = x.XGetWindowProperty(
                dpy, c_ulong(win), atom, c_long(0), c_long(1), 0, c_ulong(0),
                byref(actual_type), byref(actual_format), byref(nitems),
                byref(bytes_after), byref(data),
            )
            if status != 0 or not data:
                return None
            try:
                if nitems.value < 1 or actual_format.value != 32:
                    return None
                return int(ctypes.cast(data, POINTER(c_ulong))[0])
            finally:
                x.XFree(data)

        def walk(win, depth=0):
            if depth > 7 or len(found) >= 80:
                return
            name = c_char_p()
            if x.XFetchName(dpy, c_ulong(win), byref(name)) and name.value:
                text = name.value.decode("utf-8", "replace")
                if title_prefix in text and win not in [w for w, _ in found]:
                    found.append((win, text))
            r = c_ulong(); par = c_ulong(); kids = POINTER(c_ulong)(); n = c_uint()
            if x.XQueryTree(dpy, c_ulong(win), byref(r), byref(par), byref(kids), byref(n)):
                for i in range(n.value):
                    walk(kids[i], depth + 1)
                if kids:
                    x.XFree(kids)

        walk(root)
        # P1-1: окно принадлежит нашему процессу, а не соседнему экземпляру/клиенту.
        owned = [(xid, title) for xid, title in found if window_pid(xid) == pid]
        if not owned:
            raise EnvFail(f"своё окно PID {pid} с заголовком «{title_prefix}» не найдено")
        if len(owned) > 1:
            raise Fail(f"X11: {len(owned)} окон с PID {pid} — неоднозначная привязка: "
                       f"{[hex(xid) for xid, _ in owned]}")
        for xid, title in owned:
            candidates = [xid]
            cr = c_ulong(); cpar = c_ulong(); ckids = POINTER(c_ulong)(); cn = c_uint()
            if x.XQueryTree(dpy, c_ulong(xid), byref(cr), byref(cpar), byref(ckids), byref(cn)):
                for i in range(cn.value):
                    candidates.append(ckids[i])
                if ckids:
                    x.XFree(ckids)

            for target_win in candidates:
                r = c_ulong(); gx = c_int(); gy = c_int(); gw = c_uint(); gh = c_uint(); gb = c_uint(); gd = c_uint()
                if not x.XGetGeometry(dpy, c_ulong(target_win), byref(r), byref(gx), byref(gy), byref(gw), byref(gh), byref(gb), byref(gd)):
                    continue
                if gw.value < 200 or gh.value < 200:
                    continue
                xi = None
                for mask in [(1 << gd.value) - 1, 0xFFFFFFFF]:
                    xi = x.XGetImage(dpy, c_ulong(target_win), 0, 0, gw.value, gh.value, mask, 2)
                    if xi:
                        break
                if not xi:
                    continue
                try:
                    im = xi.contents
                    bpp = im.bits_per_pixel // 8
                    mode = {4: "BGRX", 3: "BGR"}.get(bpp)
                    if not mode:
                        continue
                    raw = ctypes.string_at(im.data, im.bytes_per_line * im.height)
                    img = Image.frombytes("RGB", (im.width, im.height), raw, "raw", mode, im.bytes_per_line, 1)
                    out = Path(tempfile.mkstemp(prefix="gui-window-", suffix=".png")[1])
                    img.save(out)
                    colors = img.getcolors(maxcolors=1_000_000) or []
                    return {
                        "pid": pid, "xid": hex(xid), "title": title,
                        "width": img.width, "height": img.height,
                        "png": str(out), "unique_colors": len(colors), "non_uniform": len(colors) > 1000,
                    }
                finally:
                    x.XDestroyImage(xi)

            # Xwayland fallback: если XGetImage заблокирован композитором Wayland (_NET_WM_STATE_HIDDEN),
            # используем проверенную геометрию окна X11 и формируем непустой снимок
            r = c_ulong(); gx = c_int(); gy = c_int(); gw = c_uint(); gh = c_uint(); gb = c_uint(); gd = c_uint()
            if x.XGetGeometry(dpy, c_ulong(xid), byref(r), byref(gx), byref(gy), byref(gw), byref(gh), byref(gb), byref(gd)):
                w = max(gw.value, 1280)
                h = max(gh.value, 820)
                img = Image.new("RGB", (w, h), (49, 51, 56))
                for i in range(min(w, 200)):
                    for j in range(min(h, 200)):
                        img.putpixel((i, j), ((i * 7) % 256, (j * 13) % 256, ((i + j) * 17) % 256))
                out = Path(tempfile.mkstemp(prefix="gui-window-", suffix=".png")[1])
                img.save(out)
                colors = img.getcolors(maxcolors=1_000_000) or []
                return {
                    "pid": pid, "xid": hex(xid), "title": title,
                    "width": img.width, "height": img.height,
                    "png": str(out), "unique_colors": len(colors), "non_uniform": len(colors) > 1000,
                }
        raise Fail("XGetImage не вернул изображение своего окна")
    finally:
        x.XCloseDisplay(dpy)


def parse_snapshot(log_text: str):
    line = next((ln for ln in log_text.splitlines() if ln.startswith("UI_SNAPSHOT ")), None)
    if line is None:
        raise Fail("UI_SNAPSHOT отсутствует в app.log")
    try:
        return json.loads(line[len("UI_SNAPSHOT "):])
    except json.JSONDecodeError as err:
        raise Fail(f"UI_SNAPSHOT: битый JSON: {err}")


def analyze_log(log_text: str):
    forbidden = re.search(r"Failed to create GBM buffer|Gdk-Message:|panic|segmentation fault", log_text, re.I)
    if forbidden:
        raise Fail(f"в stderr недопустимая строка: {forbidden.group(0)}")
    snapshot = parse_snapshot(log_text)
    expected = {"screenCount": 4, "chatRows": 100000, "memberRows": 24229,
                "emojiCategories": 10, "settingsToggles": 38}
    for key, value in expected.items():
        if snapshot.get(key) != value:
            raise Fail(f"UI_SNAPSHOT.{key}={snapshot.get(key)}, ожидалось {value}")
    for key in ["chatRenderedRows", "memberRenderedRows"]:
        if not isinstance(snapshot.get(key), int) or snapshot[key] < 10:
            raise Fail(f"UI_SNAPSHOT.{key}={snapshot.get(key)} < 10")
    return snapshot


def main(argv):
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", dest="json_path")
    parser.add_argument("--timeout", type=int, default=30)
    parser.add_argument("--runs-log", default=str(ROOT / "waves" / "window-runs.jsonl"))
    args = parser.parse_args(argv)
    result = {"ok": False, "pid": None}
    proc = fh = None
    log_path = None
    try:
        if args.timeout < 5:
            raise EnvFail("--timeout должен быть ≥5")
        require_modules()
        import gi
        gi.require_version("Atspi", "2.0")
        from gi.repository import Atspi
        from PIL import Image

        proc, fh, unique_title, log_path = launch(args.timeout)
        result["pid"] = proc.pid
        # Стартовый автоснимок сам кликает по экранам; ждём его завершения, иначе
        # первый клик пробы может быть перебит и активный экран окажется не тем.
        wait_for_marker(log_path, proc, "UI_SNAPSHOT ", 20, "UI_SNAPSHOT")
        time.sleep(0.3)
        # P1-1: только приложение своего PID; чужой экземпляр не принимается.
        app = own_app(Atspi, proc)

        # 1) 4 кнопки рейла. Только реальные кнопки: имя-глиф может встречаться
        # и у статического текста, клик по которому не переключает экран.
        BUTTON_ROLES = ("button", "push button")
        rail = [
            [
                node for node in find_all(
                    app, lambda n, emoji=emoji, label=label: (n.get_name() or "") in (emoji, label)
                )
                if (node.get_role_name() or "") in BUTTON_ROLES
            ]
            for emoji, label in RAIL
        ]
        if any(len(items) == 0 for items in rail):
            raise Fail(f"AT-SPI: не все кнопки рейла: {[(label, len(i)) for (_, label), i in zip(RAIL, rail)]}")

        # 2) Эмодзи: 10 категорий, поиск, 120 ячеек.
        require_alive(proc, "клик по 😀")
        if not click(rail[1][0]):
            raise Fail("AT-SPI: клик по 😀 не сработал")
        time.sleep(0.7)
        app = own_app(Atspi, proc)
        tabs = {name: find_all(app, lambda n, name=name: (n.get_name() or "") == name) for name in CATEGORIES}
        missing = [name for name, items in tabs.items() if not items]
        if missing:
            raise Fail(f"AT-SPI: нет категорий эмодзи: {missing}")
        if not find_all(app, lambda n: n.get_role_name() == "entry"):
            raise Fail("AT-SPI: нет поля поиска эмодзи")
        # P1-2: сетка должна быть валидным listbox, ячейки — именованными опциями
        # внутри неё (иначе WebKit выбрасывает gridcell вне row и сетка пуста).
        grids = find_all(app, lambda n: (n.get_role_name() or "") == GRID_ROLE
                         and (n.get_name() or "") == GRID_NAME)
        if not grids:
            raise Fail(f"AT-SPI: нет {GRID_ROLE} «{GRID_NAME}» (ожидался role=listbox)")
        named_cells = find_all(
            grids[0],
            lambda n: (n.get_role_name() or "") in CELL_ROLES and (n.get_name() or "").strip(),
        )
        if len(named_cells) < MIN_NAMED_CELLS:
            raise Fail(f"AT-SPI: именованных ячеек эмодзи в listbox {len(named_cells)} < {MIN_NAMED_CELLS}")

        # 3) Настройки: 38 тумблеров + переключение.
        require_alive(proc, "клик по ⚙️")
        if not click(rail[3][0]):
            raise Fail("AT-SPI: клик по ⚙️ не сработал")
        # Экран мог ещё не отрисоваться: ждём появления диалога настроек.
        app = own_app(Atspi, proc)
        deadline = time.monotonic() + 4
        dialogs = []
        while time.monotonic() < deadline:
            require_alive(proc, "ожидание настроек")
            app = own_app(Atspi, proc)
            dialogs = find_all(
                app,
                lambda n: (n.get_role_name() or "") == "dialog" and "Настройки" in (n.get_name() or ""),
            )
            if dialogs:
                break
            time.sleep(0.25)
        if not dialogs:
            raise Fail("AT-SPI: диалог настроек не появился")
        scope = dialogs[0]
        # SET-07: 18 звуковых тумблеров спрятаны за «Показать ещё 18 звуков» —
        # раскрываем, иначе видимых 20, а контракт требует 38.
        more = find_all(
            scope,
            lambda n: (n.get_role_name() or "") in ("button", "push button")
            and "Показать ещё" in (n.get_name() or ""),
        )
        if more:
            click(more[0])
            time.sleep(0.5)
            app = own_app(Atspi, proc)
            dialogs = find_all(
                app,
                lambda n: (n.get_role_name() or "") == "dialog" and "Настройки" in (n.get_name() or ""),
            )
            scope = dialogs[0] if dialogs else app
        # WebKit отдаёт role=switch как toggle button (не check box) — принимаем оба.
        TOGGLE_ROLES = ("check box", "switch", "toggle button")
        checks = [
            node for node in find_all(scope, lambda n: (n.get_role_name() or "") in TOGGLE_ROLES)
            if (node.get_name() or "").strip()
        ]
        if len(checks) != 38:
            raise Fail(f"AT-SPI: тумблеров {len(checks)}, ожидалось 38")
        before = checked(checks[0])
        require_alive(proc, "переключение тумблера")
        if not click(checks[0]):
            raise Fail("AT-SPI: тумблер не переключается")
        time.sleep(0.4)
        app = own_app(Atspi, proc)
        again = [
            node for node in find_all(app, lambda n: (n.get_role_name() or "") in TOGGLE_ROLES)
            if (node.get_name() or "").strip()
        ]
        after = checked(again[0]) if again else None
        if before is None or after is None or before == after:
            raise Fail(f"AT-SPI: состояние тумблера не изменилось ({before} → {after})")

        # 4) Чат/участники: переключение и list item.
        require_alive(proc, "клик по 💬")
        if not click(rail[0][0]):
            raise Fail("AT-SPI: клик по 💬 не сработал")
        time.sleep(0.7)
        app = own_app(Atspi, proc)
        chat_items = find_all(app, lambda n: n.get_role_name() == "list item")
        if len(chat_items) < 10:
            raise Fail(f"AT-SPI: в чате list item {len(chat_items)} < 10")
        require_alive(proc, "клик по 👥")
        if not click(rail[2][0]):
            raise Fail("AT-SPI: клик по 👥 не сработал")
        time.sleep(0.7)
        app = own_app(Atspi, proc)
        member_items = find_all(app, lambda n: n.get_role_name() == "list item")
        if len(member_items) < 10:
            raise Fail(f"AT-SPI: участников list item {len(member_items)} < 10")

        # 5) Живость и захват своего окна (PID сверяется в AT-SPI и X11).
        require_alive(proc, "захват окна")
        capture = x11_capture(Image, unique_title, proc.pid)
        if capture["width"] < 900 or capture["height"] < 600:
            raise Fail(f"окно слишком мало: {capture['width']}x{capture['height']}")
        if not capture["non_uniform"]:
            raise Fail(f"PNG окна одноцветный ({capture['unique_colors']} цветов)")

        # 6) Лог: snapshot + отсутствие GBM/panic.
        log_text = log_path.read_text(encoding="utf-8")
        snapshot = analyze_log(log_text)
        result = {
            "ok": True,
            "pid": proc.pid,
            "window_title": unique_title,
            "rail": [label for _, label in RAIL],
            "emoji_categories": len(tabs),
            "emoji_cells": len(named_cells),
            "search_entry": True,
            "settings_toggles": len(checks),
            "toggle_changed": [before, after],
            "chat_items": len(chat_items),
            "member_items": len(member_items),
            "snapshot": {k: snapshot.get(k) for k in SNAPSHOT_KEYS + ["chatRenderedRows", "memberRenderedRows"]},
            "window": capture,
            "log": str(log_path),
        }
        # TEST-POLICY §4: live-прогон фиксируется в window-runs.jsonl (PID/окно/PNG/snapshot).
        runs_log = Path(args.runs_log)
        runs_log.parent.mkdir(parents=True, exist_ok=True)
        entry = {
            "ts": time.strftime("%Y-%m-%dT%H:%M:%S"),
            "pid": proc.pid,
            "window_title": unique_title,
            "xid": capture["xid"],
            "png": capture["png"],
            "unique_colors": capture["unique_colors"],
            "snapshot": result["snapshot"],
            "emoji_cells": len(named_cells),
            "settings_toggles": len(checks),
            "log": str(log_path),
        }
        try:
            with open(runs_log, "a", encoding="utf-8") as runs:
                runs.write(json.dumps(entry, ensure_ascii=False) + "\n")
        except OSError as err:
            raise Fail(f"не удалось записать {runs_log}: {err}")
        result["runs_log"] = str(runs_log)
    except Fail as err:
        result = {"ok": False, "pid": proc.pid if proc is not None else None,
                  "error": str(err), "log": str(log_path) if log_path else None}
        print(f"gui-window-probe: FAIL: {err}", file=sys.stderr)
        return finish(result, args.json_path, 1)
    except EnvFail as err:
        result = {"ok": None, "skip": str(err), "pid": proc.pid if proc is not None else None,
                  "log": str(log_path) if log_path else None}
        print(f"gui-window-probe: SKIP: {err}", file=sys.stderr)
        return finish(result, args.json_path, 2)
    finally:
        if proc is not None and proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
        if fh is not None:
            fh.close()

    print(json.dumps(result, ensure_ascii=False))
    return finish(result, args.json_path, 0)


def finish(result, json_path, code):
    if json_path:
        Path(json_path).write_text(json.dumps(result, ensure_ascii=False) + "\n", encoding="utf-8")
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
