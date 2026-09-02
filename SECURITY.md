# Security Policy

## Supported versions

Fixes land on the latest release published at <https://github.com/Hongz-return/LuxTray/releases>.

## Reporting a vulnerability

Please use [GitHub Security Advisories](https://github.com/Hongz-return/LuxTray/security/advisories/new) for issues that could affect user machines (for example unexpected process launch, path traversal in config/autostart, or privilege issues).

If advisories are unavailable, open a private report via a GitHub issue **without** exploit details and mention that you want to coordinate disclosure.

We will acknowledge reports as soon as practical and prefer a fix or clear mitigation before public detail.

## Known limitations (not vulnerabilities)

- Release binaries are **not code-signed**. Windows SmartScreen may warn; this is documented in the README.
- Config lives in `%APPDATA%\LuxTray\` (per-user). Corrupt JSON is backed up, not silently destroyed.
- Autostart uses `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` only.
