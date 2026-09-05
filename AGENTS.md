# AGENTS.md

Rustcharge monitors battery levels and sends desktop notifications with sound.

## Project conventions

- Keep the polling loop synchronous unless the task requires concurrency.
- Preserve CLI compatibility and update `README.md` when arguments or behavior change.
- For notification or audio changes, account for platform differences. Notification urgency is Linux-only; keep platform-specific code and imports behind matching `cfg` gates.
- For release changes, read `.github/workflows/release.yml` for supported targets, native dependencies, and packaging.
- Test notification decisions separately from battery hardware, desktop notifications, and audio playback. Focus on threshold boundaries, charging state, and session attempt limits when changing that logic.

## Validation

For Rust changes, run:

```sh
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Use `cargo check` during iteration. For release or packaging changes, also run `cargo build --release` and check the affected workflow targets where available.

Linux builds require ALSA development libraries and `pkg-config`; CI installs `libasound2-dev` and `pkg-config`.

For documentation-only changes, verify commands and file references against the repository and check the diff for whitespace errors. Rust checks are unnecessary.

Report which checks passed and which could not run, with the reason. Automated checks do not verify actual desktop notifications or sound playback; report any manual validation separately.
