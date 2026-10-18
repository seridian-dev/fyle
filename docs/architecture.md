# Boundaries by volatility

Fyle is one Rust package with two executables and a shared library. The core
builds without egui, a display server or a desktop session. Avoid a workspace
of tiny crates until separate dependency or release requirements justify one.

| Component | Responsibility | Expected change |
| --- | --- | --- |
| `src/types.rs` | Shared values and errors | Infrequent; keep backward compatibility |
| `src/core/file_ops/operations.rs` | Filesystem mutations | Safety and filesystem behavior |
| `src/core/archive.rs` | ZIP/7z creation and safe archive extraction | Archive format and path-safety policy |
| `src/core/file_ops/listing.rs` | Directory queries, sorting and filtering | Query policy |
| `src/core/file_ops/presentation.rs` | File labels, icons and sizes | Visual conventions |
| `src/core/loader.rs` | Background scans and stale-result rejection | Filesystem query execution |
| `src/core/job.rs` | Typed mutation intent and background execution | Operation scheduling |
| `src/core/bookmarks.rs` | Persistent bookmarks | Storage format and migrations |
| `src/desktop.rs` | Pure desktop policy and command construction | Desktop and terminal differences |
| `src/core/opener.rs` | Process execution and executable discovery | OS integration |
| `src/startup.rs` | CLI path/URL resolution | Launcher contracts |
| `src/app.rs` | GUI state and navigation | Interaction state |
| `src/app/actions.rs` | Input and action dispatch | Shortcuts and commands |
| `src/app/render.rs`, `src/ui/` | egui rendering and widgets | Frequent visual changes |
| `src/main.rs` | GUI composition root | Window/backend initialization |
| `src/bin/fyle-cli.rs` | Headless diagnostics | Support tooling |

Dependency direction: binaries → library; library never imports GUI modules.
Process execution stays at the boundary. Desktop policy takes executable
availability as an argument so tests need no installed terminals. Command
construction returns `Command` so tests inspect argv and cwd without launching
an application. No init-system-specific service or daemon is required.

`TERMINAL` accepts a single executable or executable path; use a wrapper script
for arguments. The parent process cwd never changes when launching a terminal.
Local file URLs are decoded using `url`; remote protocols are rejected rather
than treated as local paths. A single URL field code lets desktop launchers
invoke separate instances for multiple locations.

Bookmarks currently retain the legacy `cyberpunk_fm/bookmarks.json` location
under the OS config directory. Changing that requires a migration and tests.
The current clipboard is internal to Fyle. D-Bus FileManager1, remote mounts,
and session-wide clipboard integration are future adapters, not implied support.

Rendering emits typed `FileOperation` requests. The composition root starts a
single `OperationJob`, disables conflicting controls until completion, preserves
errors and refreshes the directory afterward. Only successful Move jobs clear
the cut clipboard. Trash behavior is selected by the feature at this boundary;
there is no destructive fallback. No filesystem mutation runs in a view renderer.
Archive jobs follow this same path. ZIP and 7z output are exclusive; extraction
stages regular files/directories, rejects links and escaping paths, and publishes
only to a new destination folder. Native readers handle ZIP, 7z, TAR and
gzip-compressed TAR. RAR extraction uses an installed `bsdtar` or 7-Zip tool.

## Native desktop validation

`scripts/native_desktop_smoke.py` attaches only to an explicitly launched Fyle
GUI session through the authenticated local harness. It records active distro,
desktop/session, init, display backend and scaling evidence, then exercises
space/Unicode file URLs, app text routing, internal copy/paste, renderer capture
and PTY lifecycle. It tests the current host only; Xvfb CI and installed desktop
packages are not evidence for another running desktop or init system.

DirectoryLoader owns one scan worker. Requests have generation numbers, queued
superseded requests are skipped, and the UI discards old responses. Workers are
not forcibly cancelled mid-I/O. A window owns its cache, loader and operation;
there are no process-wide environment or working-directory mutations.

## Managed update boundary

`scripts/updater.py` is a Python-standard-library Unix installation adapter, kept
outside the Rust filesystem library. Installed Python launchers execute the
`current` version directly and may start a detached update check. The canonical
repository and `main` branch are constants, not arbitrary downloaded commands.
Subprocess arguments are passed without shell interpretation. Cargo builds from
a dedicated checkout with a separate, reusable target directory.

An advisory lock serializes installation, updates and rollback. Binaries are
staged in a new version directory before a single symlink replacement activates
the pair. Failed fetches/builds never change `current`; the previous version is
retained for rollback. No updater changes a running executable in place. Status
writes use atomic replacement, automatic attempts are throttled, and failed
background checks cannot prevent the GUI from launching. The updater itself is
refreshed from the successfully built source for subsequent runs.

The distribution adapter has its own temporary-directory and local-Git tests.
System-wide installation and package-manager ownership remain administrator
concerns; automatic updates never prompt for or request elevated privileges.

## Diagnostics boundary

`src/diagnostics.rs` owns local report persistence, bounded logs, environment
allowlisting and bundle redaction. The library remains headless. GUI startup
installs a panic hook that persists a forced backtrace before invoking the
previous hook; startup errors and background-operation failures are logged.
`build.rs` embeds the Git revision and watches actual Git metadata paths, including
worktree refs. Release builds retain symbols; debug builds provide richer detail.

The managed launcher supervises the GUI with bounded stdout/stderr capture and
records abnormal native exits without copying command-line arguments into the
report. It forwards SIGINT/SIGTERM to its child. These reports complement Rust
panic reports and may describe the same incident with the same PID. The launcher
passes diagnostic/update paths through the child's environment only; it does not
mutate global application environment or install an init-system service.

Bundles are explicit local JSON exports with bounded tails and report counts.
The exporter never overwrites a user file. Raw local logs are private and may
contain paths/error text; shareable exports redact home paths and credential-like
lines. This is deliberately independent of any hosted crash analytics SDK.

## Agent GUI boundary

`src/harness.rs` owns authenticated loopback transport, private session discovery
and PNG encoding; it builds without egui. `src/app/automation.rs` applies requests
to the application and the real `raw_input_hook`, then replies after rendering.
`src/bin/support/gui.rs` is the JSON CLI client. Semantic commands use existing
application handlers; pointer/key/text events run through egui, not global OS
input. Optional control instrumentation exposes actual rendered control bounds.

A session starts only by explicit automation opt-in. A random capability token
lives in a private descriptor; CLI output and screenshots never include it. The
server binds loopback only, authenticates before queuing, and bounds messages.
Replies contain no executable shell commands. One screenshot request is in flight
at a time; its state is latched when capture is requested so PNG metadata describes
that rendered frame. Input epochs reject stale pixel actions, and image coordinates
are converted using pixels-per-point. Closing removes the descriptor.

`scripts/gui_smoke.py` validates actual app behavior with temporary fixtures and
saves screenshots plus structured state. It also runs in CI with a software GL
backend. This proves Fyle's own rendering/input path and does not assert native
integration correctness on every window manager or permission environment.

## Native source editor boundary

`core/editor.rs` owns bounded UTF-8 document reads and conflict-aware saves. It
rejects symlink/special-file entries, binary bytes and documents over 2 MiB. Save
compares current bytes with the opened snapshot, writes and syncs a sibling temp
file, and atomically renames it into place. This protects against ordinary
outside edits but cannot promise safety against a writer racing the final rename.
`app/editor.rs` owns session-local document tabs, edits, search, line-number
gutter, save state and Syntect language highlighting. Syntax work is capped at 256 KiB per document;
larger documents remain editable as plain text. Text/source paths open in Fyle;
binary/media paths continue through the desktop opener.

`core/git_review.rs` owns read-only status and single-file diff queries. It
passes argument vectors directly to Git, disables pagers/external diff/textconv,
uses literal pathspecs, bounds output and refuses unsafe untracked entries.
`GitReviewWorker` keeps process and file reads off the render thread. The GUI
shows staged/working layers, rename/deletion/untracked state and line-colored
diffs. No staging, commit or other repository mutation is exposed. LSP support
and semantic code intelligence remain separate future work.

## Local discovery boundary

`core/discovery.rs` compiles query terms into a headless predicate and ranks fuzzy
names without I/O. The directory cache supplies lens results; invalid syntax
matches nothing and exposes an inline explanation. Extension lists are ORed,
other terms are ANDed and may be negated. `app/discovery.rs` owns palette/lens
interaction and emits existing typed toolbar actions. Palette results are ranked
only when opened or edited and limited to ten visible rows.

`core/preview.rs` owns a single preview worker per window and bounded plain UTF-8
reads. It rejects non-regular entries before opening; Unix additionally uses
O_NOFOLLOW and O_NONBLOCK to reject a leaf symlink or a raced-in FIFO. An opened
handle is checked again before reading. Ancestor directories may be symlinks;
this is a read-only preview policy, not a filesystem sandbox. No content is
executed, decoded as HTML/SVG, or sent to a network service. A generation change
on selection/visibility changes discards stale replies. Worker I/O is not forcibly
cancelled. The GUI shows loading/unsupported/error states rather than blocking
rendering. Harness state includes palette/query/preview status and bounded preview
text, so the actual interaction is inspectable by an explicitly attached agent.

## Browsing workspace and vector UI

`app/workspace.rs` owns session-local tab snapshots and location entry. Switching
saves navigation/query/view policy, restores a snapshot and asks the existing
single directory worker for a fresh generation. Inactive tabs do not own worker
threads or directory caches. Selection is cleared and preview generations are
invalidated; clipboard remains shared within the window. The last tab cannot be
closed and the tab count is capped at 24. Mutation and file-dialog guards prevent
switching context during an operation or confirmation.

Workspace Ctrl+Tab chords are consumed in raw input before egui focus traversal.
Processing them only during rendering can leave an old focus ID in the native
accessibility tree when tab content disappears. Custom vector buttons, file rows,
cards and sidebar items provide labeled widget metadata for accessibility.
`ui/icons.rs` draws a shared vector set without fonts or raster assets. Visible-row
virtualization remains in both views; a geometry regression also verifies that
adjacent card hit targets do not overlap.

## Appearance and GPU boundaries

`appearance.rs` is a headless, version-tolerant settings model/store. Defaults
preserve the original neon palette; numeric settings are clamped and malformed or
oversized files report errors. Saves use an exclusive temporary file and rename.
`theme/palette.rs` owns semantic colors, `theme/mod.rs` applies styles/preferences,
`theme/shaders/` owns GLSL and `theme/shader.rs` owns GL compilation, a single
fullscreen-triangle callback and explicit shutdown cleanup. GL calls run only with
eframe's context current. Compilation errors retain a static background.
`app/appearance.rs` owns live settings interactions and persistence requests.
Animation is capped at 20 FPS, runs only while focused and is frozen in automation
sessions. Effects/motion can be disabled independently; UI scale and highlight
changes do not replace the original file-category colors.

## Embedded terminal boundary

`core/terminal.rs` is an optional `terminal` feature using portable PTY and VT
screen-model dependencies. GUI builds enable it; the headless CLI does not link
those dependencies. `app/terminal.rs` owns GUI sessions, rendering and the
small agent-harness command surface. Each PTY owns a shell child, child-local
working directory, writer and reader thread; closing the session sends HUP to
that PTY's process group on Unix and terminates its shell. It does not signal
other Fyle binaries or desktop processes. Up to eight independent PTYs are held
as session tabs; hiding the panel retains all sessions, and closing one/all
sessions ends only those PTY process groups. No background service, global cwd/environment
mutation, or cross-binary IPC is involved. Existing GUI automation remains the
only binary-to-GUI IPC, using authenticated loopback and explicit opt-in.

The VT parser maintains a 2,000-row scrollback with bounded reads and resizable
screen dimensions. Focused GUI input forwards text, control keys and common
navigation sequences to the PTY, so shell editing and keyboard-driven TUIs can
run in the embedded surface. ANSI foreground/background colors are painted as
cell runs and the cursor is visible. Terminal mouse reports and clipboard
selection remain follow-up work; this is not complete emulator compatibility
yet.
