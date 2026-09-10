# Contributing to StratosBar

First off, thanks for taking the time to contribute! 🎉

The following is a set of guidelines for contributing to StratosBar. These are mostly guidelines, not rules. Use your best judgment, and feel free to propose changes to this document in a pull request.

## How Can I Contribute?

### Reporting Bugs

This section guides you through submitting a bug report. Following these guidelines helps maintainers and the community understand your report, reproduce the behavior, and find related reports.

- **Use a clear and descriptive title** for the issue to identify the problem.
- **Describe the steps to reproduce** the problem in as much detail as possible.
- **Include screenshots or animated GIFs** which show you following the steps and demonstrate the problem.

### Suggesting Enhancements

This section guides you through submitting an enhancement suggestion, including completely new features and minor improvements to existing functionality.

- **Use a clear and descriptive title** for the issue to identify the suggestion.
- **Provide a step-by-step description of the suggested enhancement** in as much detail as possible.
- **Explain why this enhancement would be useful** to most users.

## Development Setup

1.  **Prerequisites**: Ensure you have Node.js, npm, and Rust (cargo) installed.
2.  **Install dependencies**:
    ```bash
    npm install
    ```
3.  **Run Development Server**:
    ```bash
    npm run tauri dev
    ```

## Styleguides

### Git Commit Messages

- Use the present tense ("Add feature" not "Added feature")
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...")
- Limit the first line to 72 characters or less

### Code Style

- **TypeScript/Vue**: We follow the standard Vue.js style guide.
- **Rust**: We follow `rustfmt` standard. Please run `cargo fmt` before submitting.

## Security Audits

CI runs `cargo audit` (in `test-backend`) and `npm audit` (in `test-frontend`) on every
push and pull request. Both are checked against `security/audit-ignore.toml` by
`scripts/check-audit-ignores.py`, so a build only stays green if every reported advisory
is either fixed or has a live ignore entry.

### Adding an ignore entry

If an advisory has no direct fix available in this repo (e.g. it's pulled in
transitively, or fixing it requires a coordinated major bump that's out of scope for
your change), add an entry to `security/audit-ignore.toml`:

```toml
[[ignore]]
id = "RUSTSEC-2026-0007"   # or a GHSA-... id for npm advisories
tool = "cargo"             # "cargo" or "npm"
reason = "short explanation of why this can't be fixed right now"
expires = "2026-10-15"     # ISO date; near-term, not far-future
```

### What the expiry date obligates you to

An ignore entry is a deferral, not a silent, permanent suppression. Once `expires`
passes, `check-audit-ignores.py` fails the build for everyone until the entry is
renewed with a fresh `expires` date (after re-checking that the advisory still can't be
fixed) or the underlying dependency is actually upgraded/patched. Don't set a
far-future date to make the problem go away — pick a date you're actually willing to
revisit.

## Releasing

The release version has a single source of truth enforced by CI: `package.json`,
`src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` must all carry the same version
string, and the git tag used to trigger a release must match it too. The
`version-agreement` job in `test.yml` checks this on every push, pull request, and tag
push.

To cut a release:

1. Bump the version in `package.json`, `src-tauri/Cargo.toml`, and
   `src-tauri/tauri.conf.json` to the same value, in a commit on `master`.
2. Push that commit and let CI go green — this confirms the three files agree.
3. Tag the resulting commit `vX.Y.Z` and push the tag. This triggers `release.yml` and
   re-runs `version-agreement` against the tag itself.

Bumping the files without tagging, and tagging without bumping the files, are both
caught by the `version-agreement` check — a build won't stay green (or a tag push won't
pass) until all four sources agree.

## License

By contributing, you agree that your contributions will be licensed under its MIT License.
