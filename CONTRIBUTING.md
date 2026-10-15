# Contributing

Start with `docs/architecture.md` and keep changes in the component whose
reason for changing matches the feature. Preserve the public core facade.

For behavior changes, use a red → green → refactor loop:

1. Add a regression test that describes observable behavior; reproduce the
   failure before implementation. Use `tempfile` for filesystem tests.
2. Implement the smallest fix. Inject desktop availability and inspect command
   construction instead of spawning terminals or modifying global environment.
3. Refactor while keeping behavior tests green. Add a new boundary only when
   its dependencies or reasons for change differ.

Fast headless loop:

```sh
cargo test --no-default-features --test desktop_contract
cargo test --no-default-features
cargo run --no-default-features --bin fyle-cli -- doctor
```

Before a PR:

```sh
cargo fmt --check
cargo test --locked --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
python3 -m unittest discover -s tests -p 'test_*.py'
bash -n install.sh
desktop-file-validate fyle.desktop
```

CI tests the headless library on Linux, macOS and Windows, plus full-feature
compilation/tests on Linux. GUI behavior still needs manual smoke tests in
actual KDE, GNOME and compositor sessions; CI does not simulate those desktops.

For interactive debugging, read [the diagnostics guide](docs/debugging.md).
Use `cargo build --bins` followed by `target/debug/fyle-cli debug run -- --home`.
The CLI can collect JSON diagnostics without compiling the GUI using
`cargo run --no-default-features --bin fyle-cli -- doctor --json`.
Panic/native-exit tests run child processes with temporary diagnostics directories;
never inject intentional failures into a user's normal diagnostics history.

For changes affecting GUI behavior, run `scripts/gui_smoke.py` in a graphical
session after `cargo build --bins`. See docs/agent-harness.md for structured state,
real input, screenshot coordinates and session targeting. The CI artifact contains
PNG/JSON evidence for agent review. Never attach a normal user window implicitly:
launch an agent-owned automation session and explicitly target its ID.
