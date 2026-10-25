#!/usr/bin/env python3
"""Launch isolated editor sessions and verify native floating-layout persistence."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("/tmp/paint-rust-layout-smoke"))
    parser.add_argument("--theme", choices=("system", "light", "dark"), default="system")
    parser.add_argument("--scale", type=float)
    parser.add_argument("--rulers", action="store_true")
    args = parser.parse_args()
    if args.scale is not None and not 0.5 <= args.scale <= 3:
        parser.error("Scale must be between 0.5 and 3")
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
        preferences.write_text(f"preferences-v1\n{args.theme} 1 12 1 1 0 0 {int(args.rulers)}\n")
        saved = []
        saved_preferences = []
        effects = settings.with_name("effects.json")
        effects.write_text(json.dumps({"version": 1, "last": {"GaussianBlur": 3}, "parameters": [{"GaussianBlur": 3}]}))
        saved_effects = []
        with (args.output / "editor.log").open("w") as log:
            for attempt in range(2):
                app = subprocess.Popen(
                    [str(binary), "tests/fixtures/pdn/Untitled2.pdn", "tests/fixtures/pdn/FlattenBlendTest.pdn"],
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
                            time.sleep(2)
                            subprocess.run(["import", "-window", window, str(args.output / f"launch-{attempt}.png")], check=True, timeout=10)
                            # WM close preserves normal shutdown and settings persistence.
                            subprocess.run(["xdotool", "windowquit", window], check=True, timeout=5)
                            if app.wait(timeout=10) != 0:
                                raise RuntimeError("Editor failed during shutdown")
                            saved.append(settings.read_text())
                            saved_preferences.append(preferences.read_text())
                            saved_effects.append(effects.read_text())
                            if json.loads(saved_effects[-1])["last"] != {"GaussianBlur": 3}:
                                raise RuntimeError("Last effect settings were not preserved")
                            if not saved_preferences[-1].splitlines()[1].startswith(args.theme + " "):
                                raise RuntimeError("Theme preference was not preserved")
                            if len(saved[-1].splitlines()[2].split()) != 5:
                                raise RuntimeError("Floating panel geometry was not saved")
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
        if saved_preferences[0] != saved_preferences[1]:
            raise RuntimeError("Preferences or recent files changed after restart")
        if saved_effects[0] != saved_effects[1]:
            raise RuntimeError("Effect settings changed after restart")
        (args.output / "effects.json").write_text(saved_effects[0])
        (args.output / "preferences.txt").write_text(saved_preferences[0])
        (args.output / "workspace.txt").write_text(saved[0])
    print(f"Two native launches and clean exits; layout, preferences and recent files stable. Artifacts: {args.output}")


if __name__ == "__main__":
    main()
