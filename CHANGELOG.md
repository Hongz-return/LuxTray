# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Destroy DDC/CI physical-monitor handles when monitors are dropped or rescanned (handle leak).
- Avoid overlapping `&mut App` during nested Win32 modal loops (`TrackPopupMenu`).
- Show a message box and write `%APPDATA%\LuxTray\panic.log` on panic (Release still uses `panic = "abort"`).
- Collapse installer Startup-folder shortcuts and the in-app HKCU Run toggle into a single autostart mechanism.
- Refresh hardware brightness when the flyout opens and while it stays visible.
- Back up unreadable `config.json` instead of silently replacing it; write config via a temp file then rename.

### Added

- `rust-version = "1.85"` (MSRV matching the lockfile).
- GitHub Actions CI (fmt, clippy, test, release build) and tag-triggered release packaging.
- Unit tests for brightness mapping, config load/save, and flyout slider math.
- `CHANGELOG`, `CONTRIBUTING`, `SECURITY`, and issue/PR templates.

### Changed

- Direct `windows` crate dependency bumped to 0.62 to match `wmi`.
- Installer autostart writes `HKCU\...\Run` instead of a Startup-folder shortcut.
- Version strings in the installer, package script, and PE resources are derived from `Cargo.toml`.

## [0.1.0] - 2026-09-02

### Added

- Initial public release: tray brightness for DDC/CI external monitors and WMI laptop panels.

[Unreleased]: https://github.com/Hongz-return/LuxTray/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Hongz-return/LuxTray/releases/tag/v0.1.0
