Disposable git worktree/branch — safe to write freely. By this point all Phase 0-6
migration branches should already be merged into `main` (this task depends on the
Phase 6 compliance review, which only runs against a merged main).

Read `.github/workflows/ci.yml` and `.github/workflows/release.yml` in full before
changing anything. The migration is complete and the old `src/` (React) and
`src-tauri/` (Tauri) directories are being removed in the very next task — update
`ci.yml` to build and test **only** the Slint app (`crates/ephemera-app` +
`crates/ephemera-core`), dropping the Tauri/pnpm/vite steps entirely rather than
keeping both. Check `Cargo.toml`/`Cargo.lock` at the repo root for what the Slint
app actually needs (likely `cargo check`/`cargo test`/`cargo build --release`
plus whatever system libs Slint's winit backend needs on Linux CI runners — check
`docs/04-tech-stack.md`'s verified prereqs list). No Node/pnpm steps should remain.

Update `release.yml` only if it references Tauri-specific bundling steps
(`tauri-cli`, `tauri build`, AppImage/deb/msi bundle outputs) that no longer apply
to a Slint binary. Per this repo's `CLAUDE.md`, `release.yml` must keep
`prerelease: false` — do not change that.

Do not touch version numbers, tags, or trigger a release. This is a CI configuration
change only. Run `cargo check` at the repo root and paste the output at the end.
