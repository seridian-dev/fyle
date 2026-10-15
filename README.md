# Fyle

[![CI](https://github.com/seridian-dev/fyle/actions/workflows/ci.yml/badge.svg)](https://github.com/seridian-dev/fyle/actions/workflows/ci.yml)

A [Seridian](https://github.com/seridian-dev) open-source project.

An MIT-licensed Rust file manager with a cyberpunk egui interface. Browse,
filter, sort, bookmark, copy, move, rename and delete local files.

## Build and install

Install a current stable Rust toolchain, Bash and Python 3. On Debian/Ubuntu,
install `build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev xdg-utils git python3`.
For desktop entry validation install `desktop-file-utils`.

```sh
cargo test --no-default-features
./install.sh
fyle --home
fyle /path/to/folder
fyle 'file:///path/with%20spaces'
fyle-cli doctor
```

`./install.sh --set-default` explicitly registers Fyle for directory MIME
associations. `--system` installs into `/usr/local` and requires write access;
`--uninstall` removes both executables and the desktop entry. `INSTALL_DIR`
and `DESKTOP_DIR` override user install paths; `XDG_DATA_HOME` is respected.
Uninstall does not choose a replacement default file manager.

## Seamless updates

`./install.sh` creates a managed user installation with automatic updates enabled.
Normal GUI launches check Seridian's **main** branch (the repository's default
branch, rather than `master`) at most once every six hours. Fetching and compiling
happen in a detached, lower-priority background process. A successful build becomes
active on the next launch; running windows keep their current binary. No OpenRC
or systemd service is needed. Git, Cargo/Rust, Python 3 and the build dependencies
must remain installed for source updates.

```sh
fyle-cli update --status       # current commit, update state and last error
fyle-cli update --check        # check upstream without building
fyle-cli update                # fetch, build and activate now
fyle-cli update --rollback     # return to previous version; disable automatic updates
fyle-cli update --disable-auto
fyle-cli update --enable-auto
```

The launcher starts immediately while updates build. Failed fetches/builds keep
the current version. Both binaries switch together through an atomic version
pointer. A dedicated checkout is used, so updates never pull into your development
checkout or discard local changes. Update logs and state live under
`${XDG_DATA_HOME:-~/.local/share}/fyle/`; `update.log` contains build output. Three
recent versions plus the rollback target are retained. Build caches are reused.
`FYLE_NO_UPDATE=1 fyle` skips background checking for one launch.

This source updater supports Unix user installations made by `install.sh`.
System-wide installs disable automatic updates; an administrator can update them
manually with `sudo /usr/local/bin/fyle-cli update`. Uninstall removes launchers
and the desktop entry, retaining the managed data/cache for reinstall or rollback.
The raw Cargo binaries have no updater; use the installed launchers for updates.

## Crash reports and developer diagnostics

```sh
fyle-cli doctor --json                  # version/commit, OS, desktop and session checks
fyle-cli crashes                       # recent local reports
fyle-cli crashes --latest               # newest report with backtrace
fyle-cli logs --tail 100 --follow        # bounded live log tails
fyle-cli debug bundle --output fyle-debug.json
fyle-cli debug run -- --home            # reproduce with debug logs and full backtraces
fyle-cli debug paths                    # locate diagnostics and updater data
```

Rust panics record a structured report and backtrace, including worker panics.
GUI startup errors are recorded too. The managed launcher supervises GUI exits,
keeps native stderr, and records abnormal exit codes/signals even when Rust cannot
run a panic hook. Normal closes, argument errors and ordinary SIGINT/SIGTERM exits
do not create native crash reports. Native fault reports contain the signal and
stderr; a native stack still requires a debugger/core dump.

Reports include version and Git revision. Local retention keeps 20 recent crash
reports, bounded per-session log segments, and a bounded selection in bundles.
No telemetry is uploaded. No document contents or full environment are scanned.
Raw local reports can contain paths and error text; bundles replace home paths
and credential-like lines. Review a bundle before sharing it. Exports refuse to
overwrite existing files. `--output -` writes bundle JSON to stdout.

Use `FYLE_DIAGNOSTICS_DIR=/path/to/test-logs` to isolate a reproduction. Build from
source with `cargo build --bins`, then use `target/debug/fyle-cli debug run` for
richer stack details. See the [debugging guide](docs/debugging.md).

## Agent GUI harness

Agents can inspect and control an explicitly enabled Fyle window without desktop
input/capture permissions. The harness supplies structured state, visible control
bounds, semantic actions, real egui input events and PNGs from the app renderer.

```sh
fyle-cli gui launch --path /path/to/test-folder
fyle-cli gui sessions
fyle-cli gui state
fyle-cli gui screenshot --output fyle-window.png
fyle-cli gui action search --query Cargo
fyle-cli gui action view --mode grid
fyle-cli gui close
```

Use `--session ID` when multiple sessions exist. State includes loading/busy flags,
dialog/input values, entries, selection, control bounds, pixel scale and a snapshot
ID. Pixel clicks/scrolls require that snapshot ID and reject stale state. Keyboard
and text input are delivered only to this app. Normal Fyle launches expose no
control endpoint; `gui launch` or `fyle --automation` explicitly enables one.

Run the real GUI regression harness (requires a graphical session):

```sh
cargo build --bins
python3 scripts/gui_smoke.py --cli target/debug/fyle-cli --output-dir gui-artifacts
```

It uses temporary fixture files, exercises clicks, typing, keys, view switching,
folder creation/navigation, and saves five rendered PNG/state pairs. CI runs it
under Xvfb/Mesa and uploads the evidence as `gui-state-and-screenshots`.
See the [agent protocol guide](docs/agent-harness.md).

## Linux desktops and init systems

Fyle launches directly in an existing graphical session on either systemd or
OpenRC; no service is installed. It uses eframe's Wayland/X11 backends. KDE
Plasma prefers Konsole, Ubuntu/GNOME prefers GNOME Terminal, and Hyprland/Sway
prefer Foot, with installed-terminal fallbacks. Set `TERMINAL` to an executable
path to override selection. All terminals inherit the selected directory
without changing Fyle's process working directory.

File opening uses `xdg-open`; install `xdg-utils` and configure MIME defaults
in your desktop. A functioning user D-Bus session may be needed by desktop
applications regardless of init system. On a minimal OpenRC/compositor setup,
start the graphical session with your distribution's D-Bus session setup
(e.g. `dbus-run-session Hyprland` if it is not already provided).

The desktop entry follows the [freedesktop specification](https://specifications.freedesktop.org/desktop-entry/latest-single/).
It accepts one local file URL per invocation, sets the window app ID to `fyle`,
and resolves Home/Downloads through OS user directories rather than shell `~`.
Remote URLs, D-Bus FileManager1 and desktop-wide clipboard are not implemented.
Actual desktop sessions still need manual smoke testing.

## Two binaries, one testable core

`fyle` is the GUI. `fyle-cli doctor` reports desktop integration diagnostics.
For a headless build without GUI dependencies:

```sh
cargo build --no-default-features --bin fyle-cli
cargo test --no-default-features
```

Stable filesystem APIs, desktop policy, process execution, GUI state, actions,
and rendering have separate modules. See [architecture](docs/architecture.md)
and the [TDD workflow](CONTRIBUTING.md). CI checks Linux/macOS/Windows core tests
and Linux full-feature tests, formatting, Clippy and desktop entry validation.

## Safety and responsiveness

Default builds move deleted files to Trash. A trash failure is reported and
never falls back to permanent deletion. To deliberately build without Trash,
use `cargo build --no-default-features --features gui`; its confirmation says
“Delete Permanently.”

Creating a file uses exclusive creation. Copy, move and rename reject existing
destinations. Batch copy/move checks predictable collisions before starting.
Names must be a single component; recursive self-copy, symlinks and special
files are rejected by copy/move instead of followed implicitly. Cross-device
moves currently return an error and preserve the source.

These operations are not transactions: disk errors can leave a partly completed
batch. Rename/move destination checks are not atomic against another program
changing the same paths concurrently. Reopen/refresh the folder after an error.

Directory scans and file mutations run off the UI thread. One mutation runs at
a time per window, and normal window close waits for it to finish. A stale scan
cannot replace a newer folder. Search uses cached entries; F5 refreshes from disk.
List/grid views build visible rows only, disk usage is cached for five seconds,
and case-insensitive name sorting caches keys once per entry.

The automated 10,000-entry rendering test builds fewer than 100 entries for a
1024×768 viewport. See [performance evidence](docs/performance.md).

Bookmarks retain their legacy location: the OS config directory followed by
`cyberpunk_fm/bookmarks.json`. The clipboard currently works within Fyle.

## License

[MIT](LICENSE). The bundled Feather font retains its [upstream license](assets/fonts/LICENSE.feather).
