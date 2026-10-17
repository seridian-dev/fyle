# Native desktop evidence

The GUI harness validates Fyle's renderer and app-local input path. It does not
prove compositor, session-bus, scaling, launcher or init-system integration. To
record evidence from a real logged-in desktop, build the CLI and run:

```sh
cargo build --locked --bins
python3 scripts/native_desktop_smoke.py \
  --cli target/debug/fyle-cli \
  --output-dir /tmp/fyle-native-evidence
```

The output directory must be empty. The report captures `/etc/os-release`, the
CLI's desktop/session/init detection, a compositor scaling probe when available,
and pass/fail results for a real renderer screenshot, a spaced/Unicode file URL,
Fyle's text-editor routing, in-app copy/paste, an embedded PTY command and PTY
shutdown. `native-session.png` is the actual rendered window. Run the same check
under software Mesa with `LIBGL_ALWAYS_SOFTWARE=1` to check the fallback path.

## Recorded run: Gentoo, Hyprland and OpenRC

On 2026-10-17, the active host was Gentoo Linux 2.18, x86_64, kernel
`7.2.9-gentoo-dist-bin`, OpenRC PID 1, Hyprland/Wayland, a live D-Bus session,
and `foot` selected as the external terminal. Hyprland reported an active
1920×1080 monitor at 60 Hz and scale 1; `XCURSOR_SIZE=24`.

The native run passed session detection, scaling probe, a `file://` URL for a
folder with spaces and Unicode, a 1900×1001 Wayland renderer capture with the GL
shader available, text-file routing with `fyle.desktop` as the host's text MIME
default, internal copy/paste, embedded PTY output, and PTY shutdown. Repeating
the GUI check with `LIBGL_ALWAYS_SOFTWARE=1` also passed and compiled the shader.

These results cover this logged-in Gentoo/Hyprland/OpenRC combination only.
They do not establish KDE Plasma, GNOME, systemd, fractional scaling, or external
binary-MIME application behavior. The machine has Plasma executables installed,
but no Plasma session was running during these tests. The Fyle clipboard check is
its internal file copy/paste model; it does not claim desktop clipboard
integration.
