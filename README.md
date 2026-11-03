# achlydesa-strata

## Building

The toolchain is pinned in `rust-toolchain.toml`; `rustup` installs it on first use.

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
```

`cargo build` and `cargo test` cover only the crates that do not depend on Bevy (the workspace `default-members`). Use `--workspace` or `-p strata_client` to include the client; CI runs `cargo test --workspace`.

## Faster local builds (Windows)

All of these are optional, machine-level, and PowerShell-based.

- **Defender exclusions** (run as administrator). Real-time scanning of `target` is a large part of build time:
  ```powershell
  Add-MpPreference -ExclusionPath "$env:USERPROFILE\.cargo", "$env:USERPROFILE\.rustup", "<repo>\target"
  ```
- **Dev Drive** (Windows 11): put the repository, and so `target`, on a Dev Drive (Settings > System > Storage > Advanced storage settings > Disks & volumes > Create dev drive).
- **rust-analyzer**: in VS Code set `"rust-analyzer.cargo.targetDir": true` so rust-analyzer builds into its own directory and stops locking `target`.
- **Client iteration**: `cargo run -p strata_client --features dev` links Bevy dynamically. Never use `dev` in CI or release builds.
- **Finding slow tests**: `cargo install cargo-nextest --locked`, then `cargo nextest run --workspace`.
