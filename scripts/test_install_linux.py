"""Check installation metadata and isolated staging without changing the desktop."""

import os
import json
import time
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

from install_linux import APP_ID, artifacts, atomic_write, desktop_command, refresh


class InstallationTests(unittest.TestCase):
    def test_staged_installation_and_replacement(self):
        with tempfile.TemporaryDirectory() as temporary:
            prefix = Path(temporary) / 'Editor space $cash "quoted"'
            binary = Path(temporary) / "fake-binary"
            binary.write_bytes(b"test executable")
            data_dir = prefix / "share"
            files = artifacts(prefix, data_dir, binary)
            for path, (content, mode) in files.items():
                atomic_write(path, content, mode)
                self.assertEqual(path.read_bytes(), content)
            self.assertTrue(os.access(prefix / "bin/paint-rust", os.X_OK))
            atomic_write(prefix / "bin/paint-rust", b"updated executable", 0o755)
            self.assertEqual((prefix / "bin/paint-rust").read_bytes(), b"updated executable")
            desktop = data_dir / "applications" / f"{APP_ID}.desktop"
            if shutil.which("desktop-file-validate"):
                subprocess.run(["desktop-file-validate", str(desktop)], check=True)
            refresh(data_dir)
            if shutil.which("update-mime-database"):
                self.assertIn("application/x-paint-rust", (data_dir / "mime/globs2").read_text())
            self.assertTrue((data_dir / "doc/paint-rust/LICENSE-MIT").is_file())

    @unittest.skipUnless(shutil.which("gio"), "GIO is needed for launcher validation")
    def test_launcher_passes_file_as_one_argument_without_shell_expansion(self):
        with tempfile.TemporaryDirectory() as temporary:
            prefix = Path(temporary) / 'Editor space $cash "quoted"'
            binary = Path(temporary) / "fake-binary"
            binary.write_text(
                "#!/usr/bin/python3\nimport sys, json\nfrom pathlib import Path\n"
                "Path(__file__).with_suffix('.args').write_text(json.dumps(sys.argv[1:]))\n"
            )
            data_dir = prefix / "share"
            for path, (content, mode) in artifacts(prefix, data_dir, binary).items():
                atomic_write(path, content, mode)
            image = Path(temporary) / "image with spaces;literal.prust"
            image.touch()
            subprocess.run(["gio", "launch", str(data_dir / "applications" / f"{APP_ID}.desktop"), str(image)], check=True)
            captured = prefix / "bin/paint-rust.args"
            deadline = time.monotonic() + 5
            while not captured.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertEqual(json.loads(captured.read_text()), [str(image)])

    def test_unsupported_paths_are_rejected_before_any_writes(self):
        for value in ("/tmp/a=b/editor", "/tmp/a%f/editor", "/tmp/a\nb/editor"):
            with self.assertRaises(ValueError):
                desktop_command(Path(value))

    def test_desktop_quoting_has_both_escape_layers(self):
        command = desktop_command(Path('/tmp/a b/$cash/"quoted"/editor'))
        self.assertIn('\\\\$', command)
        self.assertIn('\\\\"', command)
        self.assertTrue(command.endswith('" %f'))


if __name__ == "__main__":
    unittest.main()
