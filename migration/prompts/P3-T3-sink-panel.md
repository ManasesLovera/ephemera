Disposable git worktree/branch — safe to write freely.

Port `src/components/SinkPanel.tsx` (Postgres DB tier + GCS tier) to Slint, wired
into the shell from `migration/P2-T1` (merge or rebase onto that branch first).

Read `src/components/SinkPanel.tsx`, `docs/08-database-tier.md`, and
`docs/09-gcs-tier.md` before writing anything.

Hard invariant from `CLAUDE.md`: RAM→DB and RAM→GCS are one-way. Nothing in this
panel should offer or imply a path back to RAM. Match existing visual semantics
(100 MB caps for both DB and GCS) rather than redesigning.

Do not touch RAM/Disk panels or the dashboard charts. Run `cargo check` and paste
output at the end.
