#!/usr/bin/env python3
"""Inject Linux write/sync/replace failures into an isolated save child only."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def main():
    if sys.platform != "linux":
        raise SystemExit("This fault injector requires Linux /proc and LD_PRELOAD")
    root = Path(__file__).resolve().parents[1]
    cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo")
    subprocess.run([cargo, "build", "--locked", "--example", "save_fault_probe"], cwd=root, check=True)
    with tempfile.TemporaryDirectory(prefix="paint-rust-save-faults-") as temporary:
        scratch = Path(temporary)
        library = scratch / "faults.so"
        subprocess.run(["cc", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror", "-o", str(library),
                        str(root / "tests/support/save_faults.c"), "-ldl"], check=True)
        for mode in ("write", "sync", "rename"):
            for format_ in ("prust", "pdn", "png", "encoded"):
                for existing in (False, True):
                    folder = scratch / f"{mode}-{format_}-{existing}"
                    folder.mkdir()
                    destination = folder / f"image.{format_}"
                    original = b"previous saved document\x00\xff"
                    if existing:
                        destination.write_bytes(original)
                    env = os.environ.copy()
                    env.update(LD_PRELOAD=str(library), PAINT_RUST_FAULT_ROOT=str(folder), PAINT_RUST_FAULT_MODE=mode)
                    result = subprocess.run([str(root / "target/debug/examples/save_fault_probe"),
                                             str(destination), format_], env=env, text=True,
                                            capture_output=True, timeout=30)
                    if result.returncode:
                        raise RuntimeError(f"{folder.name}: {result.stdout}\n{result.stderr}")
                    expected_error = {"write": "No space left", "sync": "Input/output error", "rename": "Permission denied"}[mode]
                    if expected_error not in result.stdout:
                        raise RuntimeError(f"Unexpected failure in {folder.name}: {result.stdout}")
                    if existing:
                        assert destination.read_bytes() == original, folder.name
                    else:
                        assert not destination.exists(), folder.name
                    assert set(folder.iterdir()) == ({destination} if existing else set()), folder.name
                    print(f"PASS {folder.name}: original destination preserved; temporary file removed")
    print("24 isolated save failures verified")


if __name__ == "__main__":
    main()
