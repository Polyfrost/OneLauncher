# Contributing to OneLauncher

Welcome!

Please read our [Code of Conduct] to keep our community approachable and respectable.

## New Contributor Guide

To familiarize yourself with the project, please read the [README]. Here are some resources to help you get started with open-source contributions:

- [Finding ways to contribute to open-source on GitHub]
- [Setting up Git]
- [GitHub flow]
- [Collaborating with pull requests]
- [Tauri essentials]
- [the `pnpm` CLI]

## Getting Started

### Issues

#### Creating a New Issue

If you come across an issue or have a feature request for OneLauncher, please [search if a related issue has already been reported]. If no relevant issue exists, you can open a new issue using the appropriate [issue form].

#### Solving an Issue

To find an issue that interests you, you can browse through our [existing issues] and use the available `labels` to narrow down your search (See [Labels] for more information). As a general rule, if you find an issue you want to work on, you are welcome to open a PR with a fix.

### Making Changes

#### Making Changes Locally

This project uses [`Cargo`] and [`pnpm`]. Make ensure you have them installed before proceeding.

To make changes locally, follow these steps:

1. Clone the repository: `git clone https://github.com/Polyfrost/OneLauncher`
2. Navigate to the project directory: `cd OneLauncher`
3. Configure your system environment for OneLauncher development:
   1. Linux:
	   1. NixOS: Please use `nix develop`
	   2. For Other Linux users, run: `./packages/scripts/setup.sh`
		  > The [Unix script] will check if Rust and pnpm are installed then proceed to install Clang, NASM, LLVM, libvips, [Tauri essentials] and any other required dependencies for OneLauncher to build.
   2. For macOS users, run: `./packages/scripts/setup.sh`
      > The [Unix script] will check if Rust, pnpm and Xcode are installed and proceed to use Homebrew to install NASM, [Tauri essentials] and install any other required dependencies for OneLauncher to build.
   3. For Windows users, run in PowerShell: `powershell -ExecutionPolicy Bypass -File .\packages\scripts\setup.ps1`
      > The [Windows script] will install pnpm, LLVM, C++ build tools, NASM, Rust + Cargo, Rust tools, Edge Webview 2, [Tauri essentials] and any other required dependencies for OneLauncher to build.
4. Install dependencies: `pnpm i`
5. Prepare your cargo installation: `pnpm prep`

### Running
The most common scripts you will use are:

- `pnpm onelauncher:desktop dev` - Runs the **OneLauncher** desktop application with watch mode enabled and starts up the frontend's vite dev server.

- `pnpm oneclient:desktop dev` - Runs the **OneClient** desktop application with watch mode enabled and starts up the frontend's vite dev server.

If necessary, the webview devtools can be opened by pressing `Ctrl + Shift + I` (Linux and Windows) or `Command + Option + I` (macOS) in the desktop app.

After cleaning out your build artifacts using `pnpm clean`, `git clean`, or `cargo clean`, it is necessary to re-run the `setup` script.

After you finish making your changes and committed them to your branch, make sure to execute `pnpm format` to fix any style inconsistency in your code.

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

#### macOS errors with XCode tools & Rosetta

This error occurs when Xcode is not installed or when the Xcode command line tools are not in your `PATH`.

Run `xcode-select --install` in the terminal to install the command line tools. If they are already installed, ensure that you update macOS to the latest version available.

If that doesn't work, ensure that macOS is fully updated, and that you have Xcode installed (via the app store).

Also ensure that Rosetta is installed, as a few of our dependencies require it. You can install Rosetta with `softwareupdate --install-rosetta --agree-to-license`.

### Translations

Check out the [i18n README](apps/desktop/locales/README.md) for more information on how to contribute to translations.

### Credits

This CONTRIBUTING.md file was inspired by the [`github/docs` CONTRIBUTING.md] file, and we extend our gratitude to the original authors.

[Tauri essentials]: https://v2.tauri.app/start/prerequisites/
[Unix script]: https://github.com/Polyfrost/OneLauncher/blob/oneclient/main/packages/scripts/setup.sh
[Windows script]: https://github.com/Polyfrost/OneLauncher/blob/oneclient/main/packages/scripts/setup.ps1
[`cargo`]: https://doc.rust-lang.org/cargo/getting-started/installation.html
[`pnpm`]: https://pnpm.io/installation
[Labels]: https://github.com/Polyfrost/OneLauncher/labels
[the `pnpm` CLI]: https://pnpm.io/pnpm-cli
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
