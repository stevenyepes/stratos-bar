# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- **Apps**: `AppEntry.id` is now derived from the spec-compliant XDG desktop-file-id instead of the bare filename, fixing id collisions between desktop files with the same name in different nested subdirectories.
- **History**: App history/frequency data is now keyed by app id instead of the `exec` string, preventing entries from splitting or merging incorrectly when an app's launch command changes. Existing `history.json` entries are automatically and non-destructively migrated to the new key on first startup, preserving frequency and last-accessed data.

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
