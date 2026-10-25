#!/usr/bin/env python3
"""Build and package a native Linux release with notices and a user-local installer."""

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib

from install_linux import atomic_write

ROOT = Path(__file__).resolve().parents[1]


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def package_files(root, binary, build_info):
    files = {
        "bin/paint-rust": (binary.read_bytes(), 0o755),
        "install.py": ((root / "scripts/install_linux.py").read_bytes(), 0o755),
        "BUILD_INFO.json": ((json.dumps(build_info, indent=2, sort_keys=True) + "\n").encode(), 0o644),
    }
    for name in ("LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_NOTICES", "README.txt", "USER_GUIDE.txt"):
        files[name] = ((root / name).read_bytes(), 0o644)
    for directory in ("assets", "packaging/linux", "licenses"):
        for path in sorted((root / directory).rglob("*")):
            if path.is_symlink():
                raise ValueError(f"Package inputs must not be symlinks: {path.relative_to(root)}")
            if path.is_file():
                files[path.relative_to(root).as_posix()] = (path.read_bytes(), 0o644)
    files["INSTALL.txt"] = (b"""Paint Rust for Linux
===================

Application identifier: net.trelak.PaintRust

Run directly from this folder:
    ./bin/paint-rust

Install the launcher, icons and file type registration for your account:
    python3 install.py
The default executable location is ~/.local/bin/paint-rust. No administrator
password is needed. To install under another directory:
    python3 install.py --prefix /absolute/path/to/prefix

The installer requires Python 3. Desktop/MIME caches are refreshed when the host
provides the corresponding tools. Existing default file associations are kept.

This is a native build for the architecture recorded in BUILD_INFO.json. It uses
host system libraries (including the dav1d runtime for AVIF), an X11 or Wayland
desktop and working OpenGL drivers.
File dialogs use the desktop portal. This archive does not bundle a Linux runtime;
compatibility with older distributions is not guaranteed. BUILD_INFO.json records
the build system's glibc and directly linked libraries for diagnosis.

SHA256SUMS lists the package files. Run 'sha256sum -c SHA256SUMS' to check them.
This detects changes; it is not a signature or a proof of the publisher's identity.
Licenses and dependency notices are included. See README.txt for features and
current limitations. Full screen-reader workflow validation is deferred.
""", 0o644)
    files["SHA256SUMS"] = ("".join(f"{sha256(data)}  {name}\n" for name, (data, _) in sorted(files.items())).encode(), 0o644)
    return files


def write_archive(output, name, files, epoch):
    if not re.fullmatch(r"[A-Za-z0-9_.+-]+", name):
        raise ValueError("Invalid release directory name")
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=output.parent, delete=False) as raw:
            temporary = Path(raw.name)
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                    for relative, (data, mode) in sorted(files.items()):
                        path = Path(relative)
                        if path.is_absolute() or ".." in path.parts:
                            raise ValueError("Unsafe package member path")
                        info = tarfile.TarInfo(f"{name}/{relative}")
                        info.size, info.mode, info.mtime = len(data), mode, epoch
                        archive.addfile(info, io.BytesIO(data))
            raw.flush()
            os.fchmod(raw.fileno(), 0o644)
            os.fsync(raw.fileno())
        os.replace(temporary, output)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.error("This packager validates native Linux builds only")
    cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo")
    rustc = shutil.which("rustc") or str(Path.home() / ".cargo/bin/rustc")
    def command(*parts):
        return subprocess.check_output(parts, cwd=ROOT, text=True).strip()
    try:
        rust = command(rustc, "--version", "--verbose")
        target = next(line.removeprefix("host: ") for line in rust.splitlines() if line.startswith("host: "))
        subprocess.run([sys.executable, str(ROOT / "scripts/dependency_notices.py"), "--check", "--target", target], cwd=ROOT, check=True)
        build = subprocess.run([cargo, "build", "--release", "--locked", "--target", target,
                                "--message-format=json-render-diagnostics"], cwd=ROOT, check=True,
                               stdout=subprocess.PIPE, text=True)
        artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.startswith("{")]
        executables = [artifact["executable"] for artifact in artifacts
                       if artifact.get("reason") == "compiler-artifact" and artifact.get("executable")
                       and artifact["target"]["name"] == "paint-rust"
                       and "bin" in artifact["target"]["kind"]]
        if len(executables) != 1:
            raise ValueError("Cargo did not report exactly one editor executable")
        binary = Path(executables[0])
        if not binary.read_bytes().startswith(b"\x7fELF"):
            raise ValueError("The built executable is not an ELF binary")
        linked = command("ldd", str(binary))
        if "not found" in linked:
            raise ValueError("The build host cannot resolve all linked libraries")
        info = {
            "application_id": "net.trelak.PaintRust",
            "version": tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"],
            "target": target,
            "rustc": rust,
            "build_libc": list(platform.libc_ver()),
            "source_revision": command("git", "rev-parse", "HEAD"),
            "source_dirty": bool(command("git", "status", "--porcelain")),
            "cargo_lock_sha256": sha256((ROOT / "Cargo.lock").read_bytes()),
            "binary_sha256": sha256(binary.read_bytes()),
            "linked_libraries": sorted({Path(line.strip().split()[0]).name for line in linked.splitlines() if line.strip()}),
        }
        epoch = int(os.environ.get("SOURCE_DATE_EPOCH") or command("git", "log", "-1", "--format=%ct"))
        name = f'paint-rust-{info["version"]}-{target}'
        output = args.output_dir / f"{name}.tar.gz"
        write_archive(output, name, package_files(ROOT, binary, info), epoch)
        checksum = sha256(output.read_bytes())
        atomic_write(output.with_suffix(output.suffix + ".sha256"), f"{checksum}  {output.name}\n".encode(), 0o644)
        print(f"Created {output} ({output.stat().st_size / 1048576:.1f} MiB)")
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Packaging failed: {error}\n")


if __name__ == "__main__":
    main()
