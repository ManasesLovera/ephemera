Disposable git worktree/branch — safe to write freely.

Port `src/components/RamPane.tsx` (and its use of `Meter.tsx`/`StatTile.tsx` for the
RAM tier specifically) to Slint, wired into the shell from `migration/P2-T1` (merge
or rebase onto that branch first — `git log --all --oneline`).

Read `src/components/RamPane.tsx`, `src/components/Meter.tsx`,
`src/components/StatTile.tsx`, and the RAM section of `docs/03-ui-and-visualization.md`
before writing anything. Match the existing visual semantics (fill level = bytes used
/ 10 MB cap, whatever status states the meter currently shows) rather than
redesigning the panel — this is a port, not a redesign.

Do not touch Disk/Sink panels or the dashboard charts — those are separate tasks.
Run `cargo check` and paste output at the end.
