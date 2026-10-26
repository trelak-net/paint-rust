#!/usr/bin/env python3
"""Launch isolated editor sessions and verify native floating-layout persistence."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def press_key(window, keysym, modifiers=0):
    # Address the isolated test window with an explicit unmodified key event.
    # Duplicate F1 keysyms in XKB can confuse keysym-to-modifier automation.
    class KeyEvent(ctypes.Structure):
        _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong),
                    ("send_event", ctypes.c_int), ("display", ctypes.c_void_p),
                    ("window", ctypes.c_ulong), ("root", ctypes.c_ulong),
                    ("subwindow", ctypes.c_ulong), ("time", ctypes.c_ulong),
                    ("x", ctypes.c_int), ("y", ctypes.c_int),
                    ("x_root", ctypes.c_int), ("y_root", ctypes.c_int),
                    ("state", ctypes.c_uint), ("keycode", ctypes.c_uint),
                    ("same_screen", ctypes.c_int)]
    class Event(ctypes.Union):
        _fields_ = [("key", KeyEvent), ("pad", ctypes.c_long * 24)]
    x11 = ctypes.CDLL("libX11.so.6")
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
    x11.XKeysymToKeycode.restype = ctypes.c_ubyte
    x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
    x11.XDefaultRootWindow.restype = ctypes.c_ulong
    x11.XSendEvent.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_long, ctypes.POINTER(Event)]
    x11.XFlush.argtypes = [ctypes.c_void_p]
    x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
    display = x11.XOpenDisplay(None)
    if not display:
        raise RuntimeError("Could not connect to X11 for the help shortcut")
    try:
        code = x11.XKeysymToKeycode(display, keysym)
        if not code:
            raise RuntimeError("Requested key is not mapped in the active keyboard layout")
        event = Event()
        event.key.display = display
        event.key.window = int(window)
        event.key.root = x11.XDefaultRootWindow(display)
        event.key.keycode = code
        event.key.state = modifiers
        event.key.same_screen = 1
        for kind, mask in [(2, 1), (3, 2)]:
            event.key.type = kind
            if not x11.XSendEvent(display, int(window), 0, mask, ctypes.byref(event)):
                raise RuntimeError("Could not deliver the help shortcut")
        x11.XFlush(display)
    finally:
        x11.XCloseDisplay(display)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("/tmp/paint-rust-layout-smoke"))
    parser.add_argument("--images", nargs="+", default=["tests/fixtures/pdn/Untitled2.pdn", "tests/fixtures/pdn/FlattenBlendTest.pdn"], help="Input images for the isolated editor sessions")
    parser.add_argument("--theme", choices=("system", "light", "dark"), default="system")
    parser.add_argument("--scale", type=float)
    parser.add_argument("--rulers", action="store_true")
    parser.add_argument("--units", choices=("pixels", "inches", "centimeters"), default="pixels")
    parser.add_argument("--size", help="Requested native window size, e.g. 800x600")
    dialogs = parser.add_mutually_exclusive_group()
    dialogs.add_argument("--show-help", action="store_true")
    dialogs.add_argument("--show-errors", action="store_true")
    dialogs.add_argument("--show-new", action="store_true")
    dialogs.add_argument("--show-settings", action="store_true")
    dialogs.add_argument("--show-picker", action="store_true")
    dialogs.add_argument("--show-selection-options", action="store_true")
    dialogs.add_argument("--show-resize", action="store_true")
    dialogs.add_argument("--show-canvas", action="store_true")
    dialogs.add_argument("--show-selection-transform", action="store_true")
    dialogs.add_argument("--show-effect", choices=("Clouds", "Turbulence", "Mandelbrot", "Julia"))
    args = parser.parse_args()
    if args.scale is not None and not 0.5 <= args.scale <= 3:
        parser.error("Scale must be between 0.5 and 3")
    size = None
    if args.size:
        try:
            size = tuple(int(value) for value in args.size.lower().split("x"))
            if len(size) != 2 or not all(200 <= value <= 8192 for value in size):
                raise ValueError()
        except ValueError:
            parser.error("Size must be WIDTHxHEIGHT with dimensions from 200 to 8192")
    root = Path(__file__).resolve().parents[1]
    binary = root / "target/release/paint-rust"
    if not binary.is_file():
        parser.error("Build the release binary first: cargo build --locked --release")
    if not os.environ.get("DISPLAY"):
        parser.error("A running X11 or XWayland desktop is required")
    for command in ("xdotool", "import"):
        if shutil.which(command) is None:
            parser.error(f"Required desktop tool is missing: {command}")
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="paint-rust-ui-") as base:
        env = os.environ.copy()
        env.pop("WAYLAND_DISPLAY", None)
        if args.scale is not None:
            env["WINIT_X11_SCALE_FACTOR"] = str(args.scale)
        for key in ("XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME"):
            env[key] = str(Path(base) / key)
            Path(env[key]).mkdir()
        settings = Path(env["XDG_CONFIG_HOME"]) / "net.trelak.PaintRust/workspace.txt"
        settings.parent.mkdir()
        settings.write_text("workspace-v1\n1\n1\n1\n1\n1\n")
        preferences = settings.with_name("preferences.txt")
        units = ("pixels", "inches", "centimeters").index(args.units)
        preferences.write_text(f"preferences-v1\n{args.theme} 1 12 1 1 0 0 {int(args.rulers)} 0 {units}\n")
        if args.show_selection_options:
            settings.with_name("general.json").write_text(json.dumps({"selection_draw": {"mode": "Ratio", "width": 4, "height": 3}}))
        saved = []
        saved_preferences = []
        saved_general = []
        effects = settings.with_name("effects.json")
        last_effect = {"GaussianBlur": 3}
        if args.show_effect:
            colors = [[0, 0, 0, 255], [255, 255, 255, 255]]
            parameters = ({"scale": 40.0, "roughness": 0.5, "seed": 1, "colors": colors}
                          if args.show_effect in ("Clouds", "Turbulence") else
                          {"iterations": 100, "zoom": 1.0, "center": [-0.5, 0.0], "angle": 0.0,
                           "julia": [-0.7, 0.27015], "colors": colors})
            last_effect = {args.show_effect: parameters}
        effects.write_text(json.dumps({"version": 1, "last": last_effect, "parameters": [last_effect]}))
        saved_effects = []
        with (args.output / "editor.log").open("w") as log:
            for attempt in range(2):
                app = subprocess.Popen(
                    [str(binary), *args.images],
                    cwd=root, env=env, stdout=log, stderr=log,
                )
                try:
                    deadline = time.monotonic() + 15
                    while time.monotonic() < deadline:
                        found = subprocess.run(
                            ["xdotool", "search", "--onlyvisible", "--pid", str(app.pid)],
                            capture_output=True, text=True, timeout=5,
                        )
                        if found.returncode == 0:
                            window = found.stdout.splitlines()[-1]
                            if size:
                                subprocess.run(["xdotool", "windowsize", window, str(size[0]), str(size[1])], check=True, timeout=5)
                            time.sleep(2)
                            if args.show_selection_options or args.show_picker or args.show_settings or args.show_new or args.show_errors or args.show_help or args.show_effect or args.show_selection_transform or args.show_resize or args.show_canvas:
                                subprocess.run(["xdotool", "windowactivate", "--sync", window], check=True, timeout=5)
                                focused = subprocess.check_output(["xdotool", "getactivewindow"], text=True, timeout=5).strip()
                                if focused != window:
                                    raise RuntimeError("Editor does not own focus; no keyboard input sent")
                                if args.show_selection_options:
                                    press_key(window, ord("s"))
                                elif args.show_picker:
                                    press_key(window, ord("i"))
                                elif args.show_settings:
                                    press_key(window, ord("x"), 8)
                                elif args.show_new:
                                    press_key(window, ord("n"), 4)
                                elif args.show_errors:
                                    press_key(window, 0xFFBE, 4)
                                elif args.show_canvas:
                                    press_key(window, ord("r"), 5)
                                elif args.show_resize:
                                    press_key(window, ord("r"), 4)
                                elif args.show_selection_transform:
                                    press_key(window, ord("a"), 4)
                                    time.sleep(0.2)
                                    press_key(window, ord("m"), 12)
                                else:
                                    press_key(window, 0xFFBE if args.show_help else ord("f"), 0 if args.show_help else 12)
                                time.sleep(0.5)
                            subprocess.run(["import", "-window", window, str(args.output / f"launch-{attempt}.png")], check=True, timeout=10)
                            if args.show_settings or args.show_new or args.show_effect or args.show_selection_transform or args.show_resize or args.show_canvas:
                                press_key(window, 0xFF1B)  # Escape cancels the preview before closing.
                                time.sleep(0.3)
                            # WM close preserves normal shutdown and settings persistence.
                            subprocess.run(["xdotool", "windowquit", window], check=True, timeout=5)
                            if app.wait(timeout=10) != 0:
                                raise RuntimeError("Editor failed during shutdown")
                            saved.append(settings.read_text())
                            saved_preferences.append(preferences.read_text())
                            saved_general.append(settings.with_name("general.json").read_text())
                            saved_effects.append(effects.read_text())
                            if json.loads(saved_effects[-1])["last"] != last_effect:
                                raise RuntimeError("Last effect settings were not preserved")
                            if not saved_preferences[-1].splitlines()[1].startswith(args.theme + " "):
                                raise RuntimeError("Theme preference was not preserved")
                            if any(len(line.split()) not in (1, 5) for line in saved[-1].splitlines()[2:]):
                                raise RuntimeError("Invalid panel visibility or geometry settings")
                            break
                        if app.poll() is not None:
                            raise RuntimeError("Editor exited before showing a window")
                        time.sleep(0.25)
                    else:
                        raise RuntimeError("Editor did not show a window")
                finally:
                    if app.poll() is None:
                        app.terminate()
                        app.wait(timeout=5)
        if saved[0] != saved[1]:
            raise RuntimeError("Panel layout changed after restart")
        if saved_general[0] != saved_general[1]:
            raise RuntimeError("General preferences changed after restart")
        if saved_preferences[0] != saved_preferences[1]:
            raise RuntimeError("Preferences or recent files changed after restart")
        if saved_effects[0] != saved_effects[1]:
            raise RuntimeError("Effect settings changed after restart")
        (args.output / "effects.json").write_text(saved_effects[0])
        (args.output / "preferences.txt").write_text(saved_preferences[0])
        (args.output / "general.json").write_text(saved_general[0])
        (args.output / "workspace.txt").write_text(saved[0])
    print(f"Two native launches and clean exits; layout, preferences and recent files stable. Artifacts: {args.output}")


if __name__ == "__main__":
    main()
