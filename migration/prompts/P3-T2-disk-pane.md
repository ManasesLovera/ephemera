Disposable git worktree/branch — safe to write freely.

Port `src/components/DiskPane.tsx` to Slint, wired into the shell from
`migration/P2-T1` (merge or rebase onto that branch first).

Read `src/components/DiskPane.tsx` and the disk section of
`docs/03-ui-and-visualization.md` before writing anything.

Hard invariant from `CLAUDE.md`: if the current UI shows a simulated disk-latency
throttle, it must stay visibly labelled as simulated in the ported version — never
let a simulated value look like a measured one. Match existing visual semantics
(fill level = bytes used / 20 MB cap) rather than redesigning.

Do not touch RAM/Sink panels or the dashboard charts. Run `cargo check` and paste
output at the end.
