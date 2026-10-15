# Debug Fyle from the CLI

Start with:

```sh
fyle-cli doctor --json
fyle-cli crashes --latest
fyle-cli logs --tail 100
```

Doctor reports build version/revision, feature flags, OS/architecture, kernel
where available, desktop/session, detected init process, display/D-Bus presence,
terminal and opener availability. It does not dump the environment or D-Bus
address. `crashes --json` is a structured list; no reports yields an empty array.
`crashes --latest --json` yields a report or JSON null.

## Reproduce a problem

```sh
cargo build --bins
target/debug/fyle-cli debug run -- --home
```

The CLI locates the GUI beside itself and launches it with `RUST_BACKTRACE=full`
and `RUST_LOG=warn,fyle=debug` (an existing RUST_LOG overrides the default). Logs
appear on stderr and in local session logs. `debug run` blocks until the GUI exits
and returns an error for a failed run. No installed updater/build is required for
source debugging. The installed `fyle-cli debug run` works the same way.

For isolated data, pass FYLE_DIAGNOSTICS_DIR to the command. In development tests,
set it on the child command rather than modifying the parent process environment.
Debug builds have better symbol/source information than optimized releases.

## Collect a support bundle

```sh
fyle-cli debug bundle --output fyle-debug.json
# For automation:
fyle-cli debug bundle --output -
```

A bundle contains doctor output, up to ten reports, up to five bounded session-log
tails, updater status and the last 16 KiB of updater output. Individual reports
are bounded; logs rotate at 1 MiB per session segment. Bundles do not read browsed
files or upload anything. Home paths and lines resembling credentials are
redacted; this is best-effort and cannot identify every secret in arbitrary error
messages. Review the JSON before sharing. A path export uses exclusive creation.

`fyle-cli debug paths` shows the current directories. Managed Unix installations
put diagnostics below the per-user Fyle data directory; a custom
FYLE_DIAGNOSTICS_DIR takes precedence. Raw source builds use the OS local-data
directory. Files are private on Unix. Reports are retained by count (20), not
as a lifetime crash-rate metric.

## What a report means

- `panic`: the Rust panic hook ran, captured a backtrace and persisted it. A worker
  panic can be recoverable while the GUI stays open.
- `startup`: GUI initialization returned an error.
- `native_exit`: the installed supervisor observed an abnormal GUI exit or signal.
  The report links its stderr log. A Rust abort may produce both panic and native
  reports; correlate PID and timestamp rather than counting these as two crashes.
- `process_exit`: `debug run` observed an unsuccessful GUI exit.

Native reports cannot supply a native stack by themselves. Use a debugger or OS
core dump when needed. SIGKILL/OOM can prevent reporting if the supervisor is
also killed. Report persistence is best-effort: a full/unwritable disk must not
prevent ordinary app startup. This is local diagnostics, not Firebase Crashlytics
or another hosted collection service.
