You are reviewing a git diff (`git diff main...migration/P4-T1`) that hand-rolls the
4 Hz dashboard charts in Slint. Read-only review — do not edit files.

Check specifically, these are the two rules most likely to get silently violated in
a hand-rolled chart:

1. **No dual-axis charts** — confirm RAM store bytes and process RSS render as two
   separate stacked charts sharing one x-axis, not one chart with two y-scales. This
   is explicit in `CLAUDE.md` and in `docs/03-ui-and-visualization.md`; treat any
   dual-axis rendering as a hard failure, not a style nit.
2. **No fabricated numbers** — every value plotted must trace back to a real
   measurement from the core module (buffer size, `/proc` RSS, etc.), never an
   interpolated/placeholder/mocked value standing in for a chart type that was too
   hard to hand-roll.
3. Does the 4 Hz redraw path allocate on every tick in a way that would show up as
   its own RSS growth over time (ironic given the app's purpose) — e.g. re-allocating
   a `Vec` for chart history every frame instead of reusing a ring buffer?
4. If the generation response describes a blocker it couldn't solve, is that
   surfaced honestly in the diff (e.g. a TODO/comment) rather than papered over?

Output a markdown list of findings, ranked most-important first, each citing the
specific file/line.

---

End your response with exactly one line, verbatim, based on your findings: `VERDICT: PASS` if there are no correctness-blocking issues (style nits are fine to pass with), or `VERDICT: FAIL` if there is at least one issue that must be fixed before this can merge. This line is parsed by an automated gate — do not omit it, do not add extra text after it.
