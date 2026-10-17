import hashlib
import pathlib
import subprocess
import tarfile
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


class GentooOverlayPackagingTest(unittest.TestCase):
    def test_release_overlay_contains_both_archives_and_matching_manifest_hashes(self):
        with tempfile.TemporaryDirectory() as directory:
            work = pathlib.Path(directory)
            archives = {
                "fyle-0.10.0-linux-x86_64.tar.gz": b"x86 release payload",
                "fyle-0.10.0-linux-aarch64.tar.gz": b"arm release payload",
            }
            paths = []
            for name, contents in archives.items():
                path = work / name
                path.write_bytes(contents)
                paths.append(path)
            output = work / "overlay.tar.gz"
            subprocess.run(
                [
                    "python3",
                    str(ROOT / "scripts" / "package_gentoo_overlay.py"),
                    "0.10.0",
                    *(str(path) for path in paths),
                    str(output),
                ],
                check=True,
            )

            with tarfile.open(output, "r:gz") as bundle:
                names = set(bundle.getnames())
                self.assertIn("fyle/profiles/repo_name", names)
                self.assertIn("fyle/app-misc/fyle-bin/fyle-bin-0.10.0.ebuild", names)
                self.assertIn("fyle/app-misc/fyle-bin/Manifest", names)
                ebuild = bundle.extractfile(
                    "fyle/app-misc/fyle-bin/fyle-bin-0.10.0.ebuild"
                ).read().decode()
                manifest = bundle.extractfile(
                    "fyle/app-misc/fyle-bin/Manifest"
                ).read().decode()

            self.assertIn('KEYWORDS="~amd64 ~arm64"', ebuild)
            self.assertIn("SRC_URI=", ebuild)
            for name, contents in archives.items():
                self.assertIn(name, manifest)
                self.assertIn(hashlib.sha512(contents).hexdigest(), manifest)
                self.assertIn(hashlib.blake2b(contents).hexdigest(), manifest)


if __name__ == "__main__":
    unittest.main()
