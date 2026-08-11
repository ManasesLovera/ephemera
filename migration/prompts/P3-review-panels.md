Read-only review — do not edit files. Three generation branches need review together
since they touch the same shell: run these yourself and read the output before
reviewing:

```bash
git diff main...migration/P3-T1   # RamPane port
git diff main...migration/P3-T2   # DiskPane port
git diff main...migration/P3-T3   # SinkPanel port
```

For each, check against `CLAUDE.md`:

1. RamPane: fill level genuinely reflects the 10 MB cap tracked by the core module,
   not a hardcoded/faked percentage.
2. DiskPane: any simulated latency throttle is still visibly labelled as simulated,
   not presented as a measured number.
3. SinkPanel: no UI affordance implies bytes can move DB/GCS → RAM (one-way rule).
4. All three: no panel holds actual file byte content in a Slint property — metadata
   only (see the shell review criteria from `migration/P2-T1`'s review).
5. Do the three branches conflict with each other in ways that will make merging them
   into one shell painful (e.g. incompatible edits to the same shared `.slint` layout
   file)? Flag merge-order recommendations if so.

Output one markdown section per branch, findings ranked most-important first.

---

End your response with exactly one line, verbatim, based on your findings: `VERDICT: PASS` if there are no correctness-blocking issues (style nits are fine to pass with), or `VERDICT: FAIL` if there is at least one issue that must be fixed before this can merge. This line is parsed by an automated gate — do not omit it, do not add extra text after it.
