#!/usr/bin/env python3
"""Install an already-built editor into a user-owned Linux prefix."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
APP_ID = "net.trelak.PaintRust"


def desktop_command(executable):
    value = str(executable)
    # Desktop Entry Specification forbids '=' in executable paths. Percent field
    # expansion inside quotes is undefined, so reject it rather than guessing.
    if any(character in value for character in "=\n\r\t%"):
        raise ValueError("The installation path cannot contain =, %, tabs or newlines")
    value = "".join("\\" + c if c in '\\"`$' else c for c in value)
    return '"' + value.replace("\\", "\\\\") + '" %f'


def artifacts(prefix, data_dir, binary):
    desktop = (ROOT / "packaging/linux" / f"{APP_ID}.desktop").read_text()
    desktop = desktop.replace("Exec=paint-rust %f", "Exec=" + desktop_command(prefix / "bin/paint-rust"))
    result = {
        prefix / "bin/paint-rust": (binary.read_bytes(), 0o755),
        data_dir / "applications" / f"{APP_ID}.desktop": (desktop.encode(), 0o644),
        data_dir / "mime/packages" / f"{APP_ID}.xml": ((ROOT / "packaging/linux" / f"{APP_ID}.xml").read_bytes(), 0o644),
        data_dir / "icons/hicolor/scalable/apps" / f"{APP_ID}.svg": ((ROOT / "assets/app-icon.svg").read_bytes(), 0o644),
        data_dir / "icons/hicolor/128x128/apps" / f"{APP_ID}.png": ((ROOT / "assets/app-icon.png").read_bytes(), 0o644),
    }
    for name in ("LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_NOTICES", "README.txt"):
        result[data_dir / "doc/paint-rust" / name] = ((ROOT / name).read_bytes(), 0o644)
    for source in (ROOT / "licenses").rglob("*"):
        if source.is_file():
            result[data_dir / "doc/paint-rust/licenses" / source.relative_to(ROOT / "licenses")] = (source.read_bytes(), 0o644)
    return result


def atomic_write(path, content, mode):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as output:
            temporary = Path(output.name)
            output.write(content)
            output.flush()
            os.fchmod(output.fileno(), mode)
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def refresh(data_dir):
    for tool, directory in (
        ("update-desktop-database", data_dir / "applications"),
        ("update-mime-database", data_dir / "mime"),
    ):
        executable = shutil.which(tool)
        if executable:
            subprocess.run([executable, str(directory)], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", type=Path, help="Install under this prefix (default: ~/.local)")
    parser.add_argument("--data-dir", type=Path, help="Override the desktop data directory")
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/paint-rust")
    parser.add_argument("--no-refresh", action="store_true", help="Skip desktop and MIME cache updates")
    args = parser.parse_args()
    prefix = (args.prefix or Path.home() / ".local").expanduser().absolute()
    default_data = Path(os.environ.get("XDG_DATA_HOME") or str(prefix / "share")) if args.prefix is None else prefix / "share"
    data_dir = (args.data_dir or default_data).expanduser().absolute()
    if not args.binary.is_file():
        parser.error("Build the application first with cargo build --release --locked")
    try:
        files = artifacts(prefix, data_dir, args.binary)
        for path, (content, mode) in files.items():
            atomic_write(path, content, mode)
        if not args.no_refresh:
            refresh(data_dir)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Installation failed: {error}\n")
    print(f"Installed {prefix / 'bin/paint-rust'}")
    print(f"Desktop entry: {data_dir / 'applications' / (APP_ID + '.desktop')}")


if __name__ == "__main__":
    main()
