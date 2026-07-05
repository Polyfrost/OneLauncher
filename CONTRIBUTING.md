# Contributing to OneLauncher/OneClient

Welcome!

Please read our [Code of Conduct] to keep our community approachable and respectable.

## New Contributor Guide

To familiarize yourself with the project, please read the [README]. Here are some resources to help you get started with open-source contributions:

- [Finding ways to contribute to open-source on GitHub]
- [Setting up Git]
- [GitHub flow]
- [Collaborating with pull requests]
- [The Rust Book]
- [The `cargo` CLI]

## Getting Started

### Issues

#### Creating a New Issue

If you come across an issue or have a feature request for OneLauncher/OneClient, please [search if a related issue has already been reported]. If no relevant issue exists, you can open a new issue using the appropriate [issue form].

#### Solving an Issue

To find an issue that interests you, you can browse through our [existing issues] and use the available `labels` to narrow down your search (See [Labels] for more information). As a general rule, if you find an issue you want to work on, you are welcome to open a PR with a fix.

### Making Changes

#### Making Changes Locally

This project is a pure Rust [`Cargo`] workspace (edition 2024). Make sure you have a
recent Rust toolchain (`rustc` **1.85+**) installed before proceeding — the easiest way
is via [rustup].

To make changes locally, follow these steps:

1. Clone the repository: `git clone https://github.com/Polyfrost/OneLauncher`
2. Navigate to the project directory: `cd OneLauncher`
3. Configure your system environment for OneLauncher/OneClient development:
   1. Install a Rust toolchain that supports edition 2024: `rustup toolchain install stable`
   2. Freya renders with [Skia]; on Linux you may also need system libraries such as a C
      compiler, `pkg-config`, and the usual GUI/graphics dev packages. See the
      [Freya setup guide] for platform-specific prerequisites.

### Running

The app crate is `oneclient_app`:

- `cargo run -p oneclient_app` - Runs the **OneClient** desktop application.
- `cargo build --release -p oneclient_app` - Builds an optimized release binary.

After you finish making your changes and committed them to your branch, run
`cargo fmt` and `cargo clippy` to fix style inconsistencies and catch lints.

### Pull Request

Once you have finished making your changes, create a pull request (PR) to submit them.

- Fill out the template to help reviewers understand your changes and the purpose of your PR.
- If you are addressing an existing issue, don't forget to [link your PR to the issue].
- Enable the checkbox to [allow maintainer edits] so that the branch can be updated for merging.
- Once you submit your PR, a team member will review your proposal. They may ask questions or request additional information.
- You may be asked to make changes before the PR can be merged, either through [suggested changes] or pull request comments. You can apply suggested changes directly through the UI. For other changes, you can make them in your fork and commit them to your branch.
- As you update your PR and apply changes, [mark each conversation as resolved].
- If you run into any merge issues, refer to this [git tutorial] to help you resolve merge conflicts and other issues.

### Your PR is Merged!

Congratulations! 🎉🎉 Polyfrost thanks you for your contribution! ✨
Once your PR is merged, your changes will be included in the next release.

### Common Errors

#### macOS errors with Xcode command line tools

Build errors can occur when the Xcode command line tools are not installed or not in your `PATH`.

Run `xcode-select --install` in the terminal to install them. If they are already installed, ensure that macOS is updated to the latest version available.

### Credits

This CONTRIBUTING.md file was inspired by the [`github/docs` CONTRIBUTING.md] file, and we extend our gratitude to the original authors.

[Skia]: https://skia.org/
[Freya setup guide]: https://book.freyaui.dev/setup.html
[`cargo`]: https://doc.rust-lang.org/cargo/getting-started/installation.html
[rustup]: https://rustup.rs/
[Labels]: https://github.com/Polyfrost/OneLauncher/labels
[The Rust Book]: https://doc.rust-lang.org/book/
[The `cargo` CLI]: https://doc.rust-lang.org/cargo/
[Collaborating with pull requests]: https://docs.github.com/en/github/collaborating-with-pull-requests
[GitHub flow]: https://docs.github.com/en/get-started/quickstart/github-flow
[Setting up Git]: https://docs.github.com/en/get-started/quickstart/set-up-git
[Finding ways to contribute to open-source on GitHub]: https://docs.github.com/en/get-started/exploring-projects-on-github/finding-ways-to-contribute-to-open-source-on-github
[Code of Conduct]: ./CODE_OF_CONDUCT.md
[README]: ./README.md
[search if a related issue has already been reported]: https://docs.github.com/en/github/searching-for-information-on-github/searching-on-github/searching-issues-and-pull-requests#search-by-the-title-body-or-comments
[issue form]: https://github.com/Polyfrost/OneLauncher/issues/new/choose
[existing issues]: https://github.com/Polyfrost/OneLauncher/issues
[`github/docs` CONTRIBUTING.md]: https://github.com/github/docs/blob/main/CONTRIBUTING.md
[link your PR to the issue]: https://docs.github.com/en/issues/tracking-your-work-with-issues/linking-a-pull-request-to-an-issue
[allow maintainer edits]: https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/allowing-changes-to-a-pull-request-branch-created-from-a-fork
[suggested changes]: https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/incorporating-feedback-in-your-pull-request
[mark each conversation as resolved]: https://docs.github.com/en/github/collaborating-with-issues-and-pull-requests/commenting-on-a-pull-request#resolving-conversations
[git tutorial]: https://lab.github.com/githubtraining/managing-merge-conflicts
