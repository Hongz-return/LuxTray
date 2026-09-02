# Contributing

Thanks for looking at LuxTray. Small, focused changes are easiest to review.

## Development

Requirements:

- Windows 10/11
- Rust **1.85+** (`rust-version` in `Cargo.toml`)
- Optional: [Inno Setup 6](https://jrsoftware.org/isinfo.php) for the installer (Chinese language file: `compiler:Languages\ChineseSimplified.isl`)

```powershell
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
cargo run --release
```

Package portable + installer (version is read from `Cargo.toml`):

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\package.ps1
```

## Pull requests

- One concern per PR when practical.
- Match existing style; do not reformat unrelated files.
- Add or extend `#[cfg(test)]` for pure logic (`clamp_level`, config parse, etc.).
- Update `CHANGELOG.md` under **Unreleased**.

Use the pull request template and describe how you tested on real hardware if the change touches DDC/CI or WMI.
