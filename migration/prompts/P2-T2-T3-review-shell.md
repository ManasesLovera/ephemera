You are reviewing a git diff (`git diff main...migration/P2-T1`) that adds the Slint
application shell. Read-only review — do not edit files.

Verify:

1. No Slint property or Rust struct in the UI layer holds actual file byte content
   (a `Vec<u8>` of file data, a `String` slurped from disk, etc.) — only metadata
   (name, size, status, tier). This is the same "frontend never holds authoritative
   bytes" rule from `CLAUDE.md`, now applying to Slint's Rust side since there's no
   IPC boundary to enforce it structurally anymore.
2. The property/state model actually reflects `docs/02-architecture.md`'s state
   shape — nothing renamed or restructured in a way that silently drops a field the
   real panels (Phase 3) will need.
3. `cargo check` output pasted at the end of the generation response is genuine and
   passing.
4. General Rust/Slint code quality: no obvious panics on the 4 Hz timer path (a
   panic there would crash the whole app, unlike a webview reload).

Output a markdown list of findings, ranked most-important first, each citing the
specific file/line. If nothing is wrong, say so explicitly.

---

End your response with exactly one line, verbatim, based on your findings: `VERDICT: PASS` if there are no correctness-blocking issues (style nits are fine to pass with), or `VERDICT: FAIL` if there is at least one issue that must be fixed before this can merge. This line is parsed by an automated gate — do not omit it, do not add extra text after it.
