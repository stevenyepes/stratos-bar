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

## License

By contributing, you agree that your contributions will be licensed under its MIT License.
