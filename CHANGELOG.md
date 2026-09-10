# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-09-10

### Added
- **Apps**: Live app discovery — a filesystem watcher keeps the application list current without a restart, backed by the `freedesktop-desktop-entry` crate for locale-aware names, correct field-code handling and `Type=Application` filtering. Entries now dedupe by desktop-file ID rather than display name, and `Hidden=true` is honored alongside `NoDisplay`.
- **Apps**: `AppEntry` carries the metadata the desktop-entry spec exposes — `id`, `generic_name`, `description`, `keywords`, `try_exec`, `categories`, `startup_wm_class` — plus an `AppSource` enum distinguishing Desktop, Flatpak, Snap, AppImage, Nix and Other.
- **Search**: The omnibar matches against generic name, description and keywords, not just the app name.
- **Settings**: New App Discovery section with a "Rescan Apps" button showing a live count, and add/remove of custom application directories. The empty state documents the default scan coverage (XDG, Flatpak, Snap, `~/Applications`).
- **Diagnostics**: `stratos-bar --diagnose` prints the decisions made before the window opens — GPU vendor and the evidence for it, session type and its source, which WebKit quirk branch was taken, launcher backend, resolved terminal emulator, icon theme and a sample lookup, and app count — then exits without starting Tauri. Environment deltas are reported as key name and changed/unchanged only; no values are printed.
- **Icons**: Icons are served through a validated `stratos-icon` protocol handler in Rust that checks each already-canonicalized path against independently computed icon roots. This replaces the `assetProtocol` scope, whose globs granted the webview recursive read over `$HOME` while still missing Flatpak and Snap icon export paths. The resolved ACL now contains no `$HOME` reference at all.
- **Currency**: Flag images are vendored as 32 circular SVGs under `src-tauri/assets/flags/` and embedded in the binary, so the converter renders offline instead of fetching from a third-party GitHub Pages host at render time.
- **Packaging**: `pkg/arch/install.sh` pre-flights required commands and runtime packages before starting a build that takes minutes.

### Fixed
- **Apps**: Desktop entries with `Terminal=true` (btop, htop, vim, …) launch inside a terminal emulator instead of silently doing nothing. Emulator resolution follows `$TERMINAL` → `AppConfig.terminal_emulator` → an ordered probe, with a per-terminal argument table since `-e` versus `--` is not uniform. An entry that resolves to no usable emulator returns a visible error.
- **Apps**: Launches go through `systemd-run --user --scope`, so each app gets its own cgroup. Previously an app stayed inside stratos-bar's cgroup and was killed whenever the bar's scope went away. A hardened `setsid` + double-fork fallback covers systems without a systemd user instance.
- **Apps**: `launch_app` takes a desktop-file-id and resolves the entry from the repository, instead of a pre-mangled exec string that was field-code-stripped twice and truncated at `--file-forwarding`. Quoted arguments containing literal `%` now survive, and `Path=` is honored as the child's working directory.
- **Apps**: `AppEntry.id` is derived from the spec-compliant XDG desktop-file-id instead of the bare filename, fixing id collisions between desktop files of the same name in different nested subdirectories.
- **Apps**: The filesystem watcher starts after `manage()` rather than from the repository constructor, fixing a startup race that panicked with `state() called before manage()`.
- **History**: App history and frequency data is keyed by app id instead of the `exec` string, so entries no longer split or merge when a launch command changes. Existing `history.json` entries are migrated non-destructively on first startup.
- **Icons**: Icon lookup uses the `freedesktop-icons` crate with spec-correct size, scale and theme fallback, replacing ~100 lines of hand-rolled fallback that ignored the user's icon theme and searched hicolor directly. The cache key is now `(name, size, scale, theme)`. The unspecced Steam `librarycache` mapping was dropped rather than reimplemented.
- **WebKit**: The NVIDIA black-window workaround is gated on detected GPU vendor and session type instead of being applied unconditionally, which had forced blur and animations onto the CPU for every AMD and Intel user. See the detection matrix in BUILD.md.
- **Environment**: Child processes are spawned from an environment snapshot captured before stratos-bar mutates its own environment, so launched apps, scripts and the ffmpeg thumbnailer no longer inherit the WebKit workaround and fall back to software rendering.
- **Previews**: Added `media-src` to the CSP; `FilePreview.vue`'s `<video>` element was loading over `asset:` with no matching directive and silently failing.

### Changed
- **Dependencies**: Direct dependencies converged onto the majors already present in the lockfile. `linicon` and its duplicate desktop-entry parser were dropped; `notify`, `notify-debouncer-full`, `which`, `freedesktop-icons` and `freedesktop-desktop-entry` added.

### Security
- Bumped vitest to 4.1.11 to clear the npm audit gate, and recorded the nested esbuild tree it pulls in.
- `cargo audit` and `npm audit` are gated in CI behind an ignore list whose entries carry expiry dates.

### CI
- New `version-agreement` job asserts `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` carry the same version, and that a pushed `v*` tag matches it. The release order is documented in CONTRIBUTING.md.
- New Arch package build workflow (`arch.yml`), running `makepkg` as a non-root builder with the cargo build cached.

### Documentation
- **BUILD.md / README.md**: Replaced the outdated claim of an unconditional `WEBKIT_DISABLE_DMABUF_RENDERER=1` fix with the actual GPU/session/Hyprland/egl-wayland2 detection matrix used by `webkit_nvidia_quirk.rs`, including its fail-open behavior and the WebKit bug 262607 (RESOLVED WONTFIX) context. Removed the stale `WEBKIT_DISABLE_COMPOSITING_MODE=1` Wayland tip.
- **README.md**: Removed the "From AUR (recommended)" install path (`yay -S stratos-bar`) — no AUR package exists or can exist given the PKGBUILD's local-path source. Replaced with a git-clone + `./pkg/arch/install.sh` path, documenting the script's pre-flight checks and its several-minutes build time.

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
