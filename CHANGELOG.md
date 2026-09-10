# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **Apps**: `AppEntry.id` is now derived from the spec-compliant XDG desktop-file-id instead of the bare filename, fixing id collisions between desktop files with the same name in different nested subdirectories.
- **History**: App history/frequency data is now keyed by app id instead of the `exec` string, preventing entries from splitting or merging incorrectly when an app's launch command changes. Existing `history.json` entries are automatically and non-destructively migrated to the new key on first startup, preserving frequency and last-accessed data.

### Documentation
- **BUILD.md / README.md**: Replaced the outdated claim of an unconditional `WEBKIT_DISABLE_DMABUF_RENDERER=1` fix with the actual GPU/session/Hyprland/egl-wayland2 detection matrix used by `webkit_nvidia_quirk.rs`, including its fail-open behavior and the WebKit bug 262607 (RESOLVED WONTFIX) context. Removed the stale `WEBKIT_DISABLE_COMPOSITING_MODE=1` Wayland tip and cross-linked README's Development Tips and Troubleshooting sections to BUILD.md.
- **README.md**: Removed the "From AUR (recommended)" install path (`yay -S stratos-bar`) for Arch Linux / Manjaro / CachyOS / Endeavour — no AUR package exists or can exist given the PKGBUILD's local-path source. Replaced with a single git-clone + `./pkg/arch/install.sh` path, and documented the script's pre-flight command checks and its several-minutes full Tauri build time.

## [0.1.2] - 2026-01-10

### Added
- **CI/CD**: Added GitHub Actions workflow for automated testing (`test.yml`).
- **Documentation**: Added pipeline status and test coverage badges to README.
- **Features**:
    - Translation support.
    - Recent actions history.
    - File and video previsualizations.

### Changed
- **Currency Converter**: Improved quality of life and usability.
- **Apps**: Improved icon resolution logic.

### Fixed
- **Launcher**: Fixed issue where closing the launcher would terminate launched apps.
- **Backend**: Refactored library structure.
- **Testing**: Added extensive unit tests (backend coverage).

### Security
- Updated dependencies.
