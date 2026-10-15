# Agent-visible Fyle GUI

The harness controls Fyle itself. It never injects input into unrelated desktop
windows or captures the desktop. Use an agent-owned instance with fixture files.
No external browser driver or accessibility service is required.

## Launch and choose an exact session

```sh
fyle-cli gui launch --path /path/to/fixture
fyle-cli gui sessions
fyle-cli gui state --session SESSION_ID
```

`launch` returns JSON with `session`, PID, initial state and a log path. It waits
for the first directory scan to finish. `sessions` lists session IDs/builds and
liveness, without capability tokens. With one active session, `--session` can be
omitted; multiple active sessions require explicit selection. Source builds work
with `target/debug/fyle-cli` after `cargo build --bins`. Ordinary GUI launches have
no endpoint; `FYLE_AUTOMATION=1 fyle` or `fyle --automation` opts in explicitly.

The endpoint is authenticated loopback TCP. Discovery lives under the OS local
Fyle data directory (`harness/`), or FYLE_HARNESS_DIR for isolated tests. Unix
descriptors are mode 0600 inside a 0700 directory. Do not copy descriptor files
into artifacts: they contain the session capability token. Same-user processes
with that capability can exercise the app's normal file actions.

## Inspect, act, verify

```sh
fyle-cli gui state --session SESSION_ID --offset 0 --limit 100
fyle-cli gui screenshot --session SESSION_ID --output frame.png
fyle-cli gui action search --query Cargo --session SESSION_ID
fyle-cli gui action view --mode grid --session SESSION_ID
fyle-cli gui action select --name Cargo.toml --session SESSION_ID
fyle-cli gui action open_folder --name src --session SESSION_ID
fyle-cli gui wait --session SESSION_ID
```

State includes path, view, query, visible control bounds, up to 200 entry records,
entry counts/offset, selection, error, active dialog and input values, ready/loading/
busy flags, frame, pixel scale and `snapshot_id`. Controls are exposed only when
rendered (virtualized offscreen entries have no bounds). Entries may be paginated
independently of the visible controls. No document contents are read for state.

Semantic actions also include navigate (`--path`), back, forward, up, refresh,
toggle_hidden, new_folder, new_file, dismiss_error and cancel. Mutation operations
still use the GUI's normal confirmations and background execution. Most actions
reply after their next render; asynchronous scans/jobs may still be loading.
Use `gui wait` before asserting completed results. A ready state can still have an
error field, so check it. Folder activation does not launch a default file app.

## Real mouse, text and keyboard events

Get control `rect_pixels: [x,y,width,height]` and `snapshot_id` from fresh state or
screenshot JSON. Click the center using numeric coordinates and that snapshot:

```sh
fyle-cli gui click --x X --y Y --snapshot SNAPSHOT_ID --session SESSION_ID
fyle-cli gui text --text "search text" --snapshot SNAPSHOT_ID --session SESSION_ID
fyle-cli gui key --key Ctrl+A --session SESSION_ID
fyle-cli gui key --key Backspace --session SESSION_ID
fyle-cli gui scroll --x X --y Y --dy -400 --snapshot SNAPSHOT_ID --session SESSION_ID
```

Each action returns updated state with a new snapshot when state/input changed.
Use that returned snapshot for the next input. Stale IDs are rejected. Pixel
input always requires an ID, and loading folders reject pixel input. Coordinates
are native PNG image pixels, not screen coordinates; the app converts them to
logical points. Use the original PNG dimensions if your image viewer resizes it.
Automation instances retain app-local logical focus even when another desktop
window is focused; the harness does not change OS focus. Right click uses `--button right`. Key names are egui names such as Enter, Escape,
F5 and ArrowUp, with Ctrl/Alt/Shift modifiers. Text targets the focused widget.

## Image and state evidence

A screenshot is the actual framebuffer captured by eframe's renderer. Its JSON
contains PNG path/dimensions and the state/snapshot latched for that captured
render frame. It can work on Wayland even when desktop capture cannot identify a
window. The app must be rendering; a blocked/minimized/backend-failed viewport
can time out. Capture is capped at 16 megapixels and exports never overwrite an
existing file. Snapshots can become stale if another actor changes the window.

```sh
python3 scripts/gui_smoke.py --cli target/debug/fyle-cli --output-dir gui-artifacts
fyle-cli gui close --session SESSION_ID
```

The smoke script creates temporary fixtures, validates pixel search/text, keyboard
editing, pixel view toggles, dialog/Enter folder creation, navigation/empty state,
and saves PNG/JSON pairs plus result.json. Its finally block closes its own session.
Run with a new/empty artifact directory. CI executes the same script on Xvfb/Mesa
and uploads `gui-state-and-screenshots`, including partial evidence on failures.
This is GUI regression coverage, not a substitute for testing native desktop
integration on KDE, GNOME, Hyprland, OpenRC or systemd sessions.
