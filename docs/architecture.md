# Boundaries by volatility

Fyle is one Rust package with two executables and a shared library. The core
builds without egui, a display server or a desktop session. Avoid a workspace
of tiny crates until separate dependency or release requirements justify one.

| Component | Responsibility | Expected change |
| --- | --- | --- |
| `src/types.rs` | Shared values and errors | Infrequent; keep backward compatibility |
| `src/core/file_ops/operations.rs` | Filesystem mutations | Safety and filesystem behavior |
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
