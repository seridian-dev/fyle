# Fyle

An MIT-licensed Rust file manager with a cyberpunk egui interface. Browse,
filter, sort, bookmark, copy, move, rename and delete local files.

## Build and install

Install a current stable Rust toolchain, Bash and Python 3. On Debian/Ubuntu,
install `build-essential pkg-config libxkbcommon-dev libwayland-dev xdg-utils`.
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

Bookmarks retain their legacy location: the OS config directory followed by
`cyberpunk_fm/bookmarks.json`. Enable optional trash support with
`cargo build --release --features trash`; otherwise deletion is permanent.
The clipboard currently works within Fyle.

## License

[MIT](LICENSE). The bundled Feather font retains its [upstream license](assets/fonts/LICENSE.feather).
