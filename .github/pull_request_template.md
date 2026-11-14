## Summary

<!-- What does this change do, and why? -->

## Checklist

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `npm run typecheck` and `npm test` pass
- [ ] New files start with the SPDX header `// SPDX-License-Identifier: GPL-3.0-or-later` (or `# SPDX-License-Identifier: GPL-3.0-or-later` for TOML and YAML)
- [ ] User-visible text is added in both `src/i18n/en.ts` and `src/i18n/ru.ts`
- [ ] CHANGELOG.md is updated if the change is visible to users
