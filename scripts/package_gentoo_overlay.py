#!/usr/bin/env python3
"""Create a small installable Gentoo overlay artifact for a binary release."""
import hashlib
import pathlib
import sys
import tarfile
import tempfile


def digest(path: pathlib.Path, algorithm: str) -> str:
    hasher = hashlib.new(algorithm)
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def main() -> int:
    if len(sys.argv) != 5:
        raise SystemExit("usage: package_gentoo_overlay.py VERSION X86_64_TARBALL AARCH64_TARBALL OUTPUT")
    version, *arguments = sys.argv[1:]
    archives = [pathlib.Path(item) for item in arguments[:2]]
    output = pathlib.Path(arguments[2])
    for path in archives:
        if not path.is_file():
            raise SystemExit(f"missing release archive: {path}")

    root = pathlib.Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory(prefix="fyle-gentoo-") as temporary:
        overlay = pathlib.Path(temporary) / "fyle"
        package = overlay / "app-misc" / "fyle-bin"
        package.mkdir(parents=True)
        (overlay / "profiles").mkdir()
        (overlay / "profiles" / "repo_name").write_text("fyle\n", encoding="utf-8")
        metadata = overlay / "metadata"
        metadata.mkdir()
        (metadata / "layout.conf").write_text("repo-name = fyle\nmasters = gentoo\nthin-manifests = true\n", encoding="utf-8")

        ebuild_name = f"fyle-bin-{version}.ebuild"
        template = (root / "packaging" / "gentoo" / "fyle-bin.ebuild.in").read_text(encoding="utf-8")
        ebuild = package / ebuild_name
        ebuild.write_text(template, encoding="utf-8")

        entries = []
        for archive in sorted(archives):
            entries.append(
                f"DIST {archive.name} {archive.stat().st_size} "
                f"BLAKE2B {digest(archive, 'blake2b')} SHA512 {digest(archive, 'sha512')}"
            )
        entries.append(
            f"EBUILD {ebuild_name} {ebuild.stat().st_size} "
            f"BLAKE2B {digest(ebuild, 'blake2b')} SHA512 {digest(ebuild, 'sha512')}"
        )
        (package / "Manifest").write_text("\n".join(entries) + "\n", encoding="utf-8")

        output.parent.mkdir(parents=True, exist_ok=True)
        with tarfile.open(output, "w:gz", format=tarfile.PAX_FORMAT) as archive:
            archive.add(overlay, arcname="fyle")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
