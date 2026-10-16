# Release binaries and Linux packages

Push a version tag such as `v0.10.0` after updating `[package].version` in
`Cargo.toml`. The Binary releases workflow checks that the tag and package
version match, builds the desktop and CLI binaries, packages them, then creates
a GitHub release with generated notes and SHA-256 checksums. The Linux package
metadata and staging logic live in `.nfpm.yaml` and `scripts/package_linux.sh`.
GitHub's artifact attestation service records build provenance for each release
asset; verify one with `gh attestation verify <asset> --repo seridian-dev/fyle`.

The matrix publishes:

| Platform | Architecture | Files |
| --- | --- | --- |
| Debian and Ubuntu | x86_64, ARM64 | `.deb`, `.tar.gz` |
| Fedora and openSUSE | x86_64, ARM64 | `.rpm`, `.tar.gz` |
| Arch Linux | x86_64, ARM64 | pacman `.pkg.tar.zst`, `.tar.gz` |
| macOS | Intel, Apple Silicon | `.tar.gz` |
| Windows | x86_64, ARM64 | `.zip` |

Linux packages declare the GUI runtime libraries and `xdg-utils`. Linux binaries
are built on Ubuntu 22.04 (glibc 2.35), so the portable archives and distro
packages require glibc 2.35 or newer. They use the host's X11/Wayland and OpenGL
libraries; the archives do not bundle a desktop stack. Alpine/musl is not part of
this build. No service is installed, so the same binaries work under systemd,
OpenRC, or another init system when run inside a supported graphical session.

Install a Debian package with `sudo apt install ./fyle_*.deb`, an RPM with
`sudo dnf install ./fyle-*.rpm` (or the equivalent package-manager command), or
an Arch package with `sudo pacman -U ./fyle-*.pkg.tar.zst`. Package upgrades and
removal remain owned by the distribution package manager. The tarball is for
manual installs; its `bin/` directory contains `fyle` and `fyle-cli`, and its
`share/applications/` directory contains the desktop entry. The source-based
`install.sh` updater is a separate installation path.

Before installing, verify an asset against `SHA256SUMS`, for example:

```sh
sha256sum --check SHA256SUMS
```

The build uses native GitHub-hosted runners for Linux x86_64/ARM64, macOS
Intel/Apple Silicon, and Windows x86_64/ARM64. Linux package jobs use nFPM pinned
to v2.48.0 so package formats and metadata are built consistently from the same
staged binaries. CI checks workflow syntax with actionlint and builds all three
Linux package formats on each change before a version can be tagged.
