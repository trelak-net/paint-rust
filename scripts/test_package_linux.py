"""Validate release archive contents and installation from an extracted bundle."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest

from package_linux import ROOT, package_files, write_archive


class PackagingTests(unittest.TestCase):
    def test_archive_is_reproducible_and_installs_without_a_source_checkout(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "fake-binary"
            binary.write_bytes(b"test executable")
            files = package_files(ROOT, binary, {"application_id": "net.trelak.PaintRust"})
            first, second = root / "first.tar.gz", root / "second.tar.gz"
            write_archive(first, "paint-rust-test", files, 123)
            write_archive(second, "paint-rust-test", files, 123)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            extracted = root / "extracted"
            with tarfile.open(first) as archive:
                archive.extractall(extracted, filter="data")
            bundle = extracted / "paint-rust-test"
            for line in (bundle / "SHA256SUMS").read_text().splitlines():
                checksum, name = line.split("  ", 1)
                self.assertEqual(hashlib.sha256((bundle / name).read_bytes()).hexdigest(), checksum)
            self.assertEqual(json.loads((bundle / "BUILD_INFO.json").read_text())["application_id"], "net.trelak.PaintRust")
            prefix = root / "installation space"
            subprocess.run([sys.executable, str(bundle / "install.py"), "--prefix", str(prefix), "--no-refresh"], cwd=root, check=True, stdout=subprocess.DEVNULL)
            self.assertEqual((prefix / "bin/paint-rust").read_bytes(), binary.read_bytes())
            self.assertTrue((prefix / "share/doc/paint-rust/licenses/dependencies/INDEX.json").is_file())
            self.assertTrue((prefix / "share/applications/net.trelak.PaintRust.desktop").is_file())

    def test_unsafe_members_do_not_replace_an_existing_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "release.tar.gz"
            output.write_bytes(b"previous archive")
            with self.assertRaises(ValueError):
                write_archive(output, "safe-name", {"../outside": (b"bad", 0o644)}, 0)
            self.assertEqual(output.read_bytes(), b"previous archive")
            self.assertEqual(sorted(p.name for p in output.parent.iterdir()), ["release.tar.gz"])


if __name__ == "__main__":
    unittest.main()
