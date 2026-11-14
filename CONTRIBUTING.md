# Contributing to DownRust

Thank you for your interest in DownRust. This guide explains how to set up the project, which checks to run and how the code is organized.

## Set up

You need:

- Rust (stable), with the MSVC toolchain from Visual Studio Build Tools
- Node.js 22 or newer

```bash
npm install
npm run tauri dev
```

## Checks

Run these before you open a pull request. CI runs the same checks on Windows.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run typecheck
npm test
npm run build
npm run extension:lint
```

## Code style

- Each crate has one error enum. Use it for all errors of that crate.
- Keep files small. Split a file when it covers more than one job.
- Start every new source file with the license header:
  - Rust, TypeScript and JavaScript: `// SPDX-License-Identifier: GPL-3.0-or-later`
  - TOML, YAML and shell scripts: `# SPDX-License-Identifier: GPL-3.0-or-later`
- Keep the existing layout of the workspace. The engine goes in `crates/downrust-core`, and the desktop shell goes in `src-tauri`.
- Add every user-visible text to both `src/i18n/en.ts` and `src/i18n/ru.ts`.
- Use the Astryx components and tokens for the interface. Do not add raw colors or inline styles.

## Pull requests

1. Fork the repository and create a branch for your change.
2. Make the change and run the checks above.
3. Open a pull request. Fill in the template and describe what the change does and why.
4. Add a line to `CHANGELOG.md` when users can see the change.

## License

DownRust is free software under the GNU General Public License, version 3 or later. By contributing, you agree that your changes are released under the same license. See [LICENSE](LICENSE).
