import hashlib
from pathlib import Path
import tempfile
import unittest

from scripts.release import check_version, checksums, set_version, validate_version


class ReleaseTests(unittest.TestCase):
    def test_version_change_preserves_dependency_versions_and_validates_tag(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[package]\nname = "skarva"\nversion = "0.1.0"\n[dependencies]\nexample = "2.0"\n')
            (root / "Cargo.lock").write_text('version = 4\n[[package]]\nname = "example"\nversion = "2.0.0"\n[[package]]\nname = "skarva"\nversion = "0.1.0"\n')
            set_version("0.2.0-beta.1", root)
            self.assertEqual(check_version(root, "v0.2.0-beta.1"), "0.2.0-beta.1")
            self.assertIn('version = "2.0.0"', (root / "Cargo.lock").read_text())
            self.assertIn('example = "2.0"', (root / "Cargo.toml").read_text())
            with self.assertRaises(ValueError):
                check_version(root, "v0.2.0")

    def test_invalid_versions_and_lockfile_mismatch_fail_before_release(self):
        for version in ["1.2", "v1.2.3", "01.2.3", "1.2.3-beta.01", "1.2.3;echo bad"]:
            with self.assertRaises(ValueError):
                validate_version(version)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text('[package]\nversion = "0.2.0"\n')
            (root / "Cargo.lock").write_text('[[package]]\nname = "skarva"\nversion = "0.1.0"\n')
            with self.assertRaises(ValueError):
                check_version(root)

    def test_checksum_manifest_is_repeatable_and_excludes_itself(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "app.zip").write_bytes(b"package")
            checksums(root)
            expected = f"{hashlib.sha256(b'package').hexdigest()}  app.zip\n"
            self.assertEqual((root / "SHA256SUMS").read_text(), expected)
            checksums(root)
            self.assertEqual((root / "SHA256SUMS").read_text(), expected)

    def test_missing_platform_package_blocks_release(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "skarva-0.1.0-macos-arm64.zip").write_bytes(b"package")
            with self.assertRaisesRegex(ValueError, "Incomplete"):
                checksums(root, "0.1.0")
            self.assertFalse((root / "SHA256SUMS").exists())
            for suffix in ("macos-arm64.dmg", "macos-x64.zip", "macos-x64.dmg", "windows-x64.zip", "windows-x64-setup.exe", "linux-x64.tar.gz", "linux-x64.deb"):
                (root / f"skarva-0.1.0-{suffix}").write_bytes(b"package")
            checksums(root, "0.1.0")
            self.assertEqual(len((root / "SHA256SUMS").read_text().splitlines()), 8)


if __name__ == "__main__":
    unittest.main()
