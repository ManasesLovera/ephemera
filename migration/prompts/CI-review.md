You are reviewing a git diff (`git diff main...migration/CI-T1`) that updates the
GitHub Actions workflows for the post-migration Slint app. Read-only review — do
not edit files.

Check:

1. `ci.yml` actually builds/tests the Slint app (correct Rust toolchain setup,
   correct working directory, no leftover step that only made sense for the old
   Tauri/pnpm/vite build unless intentionally kept for a documented reason).
2. `release.yml` still has `prerelease: false` unchanged — this is a hard rule in
   this repo's `CLAUDE.md`; flag any change to it as a blocking failure regardless
   of anything else.
3. No workflow step got broader permissions/secrets access than it had before
   without a clear reason tied to the Slint build (e.g. don't wave through a new
   `secrets.GITHUB_TOKEN` write scope that wasn't needed for the old pipeline).
4. The diff doesn't touch version numbers, tags, or trigger any release action —
   this task is scoped to CI config only.
5. `cargo check` output pasted at the end of the generation response is genuine.

Output a markdown list of findings, ranked most-important first, each citing the
specific file/line.

---

End your response with exactly one line, verbatim, based on your findings:
`VERDICT: PASS` if there are no correctness-blocking issues (style nits are fine to
pass with), or `VERDICT: FAIL` if there is at least one issue that must be fixed
before this can merge. This line is parsed by an automated gate — do not omit it,
do not add extra text after it.
