Disposable git worktree/branch — safe to write freely.

Port `src/components/FileCard.tsx` and its drag-and-drop interaction to Slint.
Slint has no native HTML5 drag-and-drop equivalent — this needs to be built from
Slint's lower-level pointer-event primitives (`TouchArea`, pointer position tracking,
manual drag-state modeling). Read `src/components/FileCard.tsx` and the drag & drop
section of `docs/03-ui-and-visualization.md` before starting.

Preserve the actual semantics of a drag: which tier-to-tier moves are legal (RAM →
disk/db/gcs is fine; nothing moves back to RAM — `CLAUDE.md`'s one-way rule applies
to the drag targets you make available, not just the backend). Don't offer a drop
target that the backend would reject anyway.

Run `cargo check` and paste output at the end. If Slint's pointer primitives can't
cleanly express some interaction the current DnD has (e.g. drag previews, drop
zone highlighting), say so explicitly rather than shipping a degraded interaction
silently.
