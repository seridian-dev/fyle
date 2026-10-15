# Rendering workload evidence

The headless egui regression test constructs 10,000 entries, renders a 1024×768
viewport, and counts entry widgets actually built. It does not merely count
visible paint output: offscreen widget construction was the original cost.

```sh
cargo test --bin fyle large_directory_only -- --nocapture
```

Observed on the development Linux host, debug build, first frame:

| View | Before | After |
| --- | --- | --- |
| List | 10,000 entries built; ~1,003 ms | 29 entries built; ~16 ms |
| Grid | Previously iterated every entry | 56 entries built; ~21 ms |

Times are individual diagnostic observations, not a statistical benchmark or a
latency guarantee. The regression asserts fewer than 100 entries are built,
not a machine-dependent time threshold. Both views use `ScrollArea::show_rows`.

Other changes remove work rather than relying on timing:

- Rendering borrows entries instead of cloning the directory every frame.
- Search filters an in-memory snapshot; no metadata calls per keystroke.
- Name sorting computes lowercase keys once per entry.
- Disk probes occur at most once per five seconds per window.
- Directory scanning and file operations use background workers.

Initial scans still read metadata for every included entry. Tree copies preflight
contents before mutation, adding a traversal in exchange for detecting unsupported
entries before copying. Copy progress currently reports a working state, not bytes
or a cancellation control. External filesystem changes require F5. GUI session
validation across KDE, GNOME, Hyprland and OpenRC remains manual; the local Cua
adapter could not enumerate the Wayland window during this pass.
