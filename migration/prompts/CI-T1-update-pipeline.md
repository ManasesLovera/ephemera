Disposable git worktree/branch — safe to write freely. By this point all Phase 0-6
migration branches should already be merged into `main` (this task depends on the
Phase 6 compliance review, which only runs against a merged main).

Read `.github/workflows/ci.yml` and `.github/workflows/release.yml` in full before
changing anything. Update `ci.yml` to build and test the Slint app instead of (or,
if you judge it safer to keep both temporarily, alongside) the old Tauri/pnpm/vite
steps — check `Cargo.toml`/`Cargo.lock` at the repo root for what the Slint app
actually needs (likely just `cargo check`/`cargo test`/`cargo build --release`, no
Node/pnpm steps unless something in the repo still needs them).

Update `release.yml` only if it references Tauri-specific bundling steps
(`tauri-cli`, `tauri build`, AppImage/deb/msi bundle outputs) that no longer apply
to a Slint binary. Per this repo's `CLAUDE.md`, `release.yml` must keep
`prerelease: false` — do not change that.

Do not touch version numbers, tags, or trigger a release. This is a CI configuration
change only. Run `cargo check` at the repo root and paste the output at the end.
