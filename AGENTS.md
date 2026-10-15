# Fyle maintenance

Canonical upstream: https://github.com/seridian-dev/fyle (MIT).
Read docs/architecture.md and CONTRIBUTING.md before changing module boundaries.

Use a failing regression test for behavior changes, then implement and refactor.
Keep GUI modules out of the headless library. Renderers emit typed intents;
filesystem mutations execute in core job workers. Do not add shell commands for
file operations or mutate the application's working directory or environment.

Preserve explicit collision errors and the absence of permanent-delete fallback
when Trash fails. Do not claim transactional or concurrent mutation guarantees.
Keep view work bounded by visible rows; retain the 10,000-entry rendering test.

When the Kanban CLI is available and authenticated, use project FYLE to track
work. Inspect the card before editing and mark done only after validation.
Contributors do not need a Kanban account to build or submit changes.

Required checks for relevant changes:
- cargo fmt --check
- cargo test --locked --all-features
- cargo test --locked --no-default-features
- cargo test --locked --no-default-features --features gui
- cargo clippy --locked --all-targets --all-features -- -D warnings
- python3 -m unittest discover -s tests -p 'test_*.py'
- bash -n install.sh and desktop-file-validate fyle.desktop

Describe manual desktop coverage honestly. Cross-platform headless CI is not
proof of native GUI behavior on every desktop or init system.
