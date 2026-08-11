Disposable git worktree/branch — safe to write freely.

Port `src/components/StreamModal.tsx` to Slint — this is the RAM-bypass streaming
path's UI: a modal that reports measured peak memory for the stream-to-disk copy
against the buffered alternative. Read `src/components/StreamModal.tsx` and
`docs/07-streaming.md` in full before starting.

Hard invariant: the comparison this modal shows (streamed peak memory vs buffered
peak memory) must be built from real measurements taken during the actual copy, not
computed/estimated after the fact. If the current implementation already measures
correctly, preserve the measurement path exactly when porting — don't accidentally
replace a live measurement with a computed estimate because it's easier to wire up
in Slint's modal/dialog model.

Run `cargo check` and paste output at the end.
