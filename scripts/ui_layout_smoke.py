#!/usr/bin/env python3
"""Launch isolated editor sessions and verify native floating-layout persistence."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("/tmp/paint-rust-layout-smoke"))
    args = parser.parse_args()
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
        for key in ("XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_STATE_HOME"):
            env[key] = str(Path(base) / key)
            Path(env[key]).mkdir()
        settings = Path(env["XDG_CONFIG_HOME"]) / "net.trelak.PaintRust/workspace.txt"
        settings.parent.mkdir()
        settings.write_text("workspace-v1\n1\n1\n1\n1\n1\n")
        saved = []
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
        (args.output / "workspace.txt").write_text(saved[0])
    print(f"Two native launches and clean exits; layout stable. Artifacts: {args.output}")


if __name__ == "__main__":
    main()
