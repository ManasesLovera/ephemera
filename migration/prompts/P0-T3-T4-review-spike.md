You are reviewing a git diff (shown above, `git diff main...migration/P0-T2`) for the
`ephemera` Tauri→Slint migration spike. This is a read-only review — do not edit
files, only report findings.

## Parallel-safety and scope

This same prompt is run concurrently as `P0-T3` and `P0-T4` by independent review
agents. Treat the checkout and the `migration/P0-T2` branch as immutable review input.

- Inspect only `git diff main...migration/P0-T2` and files required to understand that
  diff. Do not inspect, modify, stage, commit, reset, checkout, merge, rebase, or push
  any branch.
- Do not run commands that write files, create worktrees, change dependencies, build
  artifacts in the repository, or alter processes. Read-only inspection commands such
  as `git diff`, `git show`, and file reads are allowed.
- Do not edit `migration/tasks.json`, review prompts, logs, outputs, or any source file.
  The dispatcher owns task status and captures your stdout separately for this task.
- Do not assume the other reviewer is present, and do not coordinate through shared
  files, the tracker, git refs, or the working tree. Produce a self-contained review
  from the supplied branch diff.
- Do not review the current checkout's unrelated or uncommitted changes as part of
  this task. Findings must concern the P0-T2 branch only.

Check specifically:

1. Is the RSS measurement methodology apples-to-apples? Same build profile
   (release vs debug), same workload size, same measurement point (idle vs
   post-allocation) on both sides of the comparison?
2. Does the spike's buffer allocation genuinely reflect real memory use (a real
   `Vec<u8>` or equivalent), or does it fake the number the meter displays? Per this
   project's rules (`CLAUDE.md`), a displayed number must be measured, never
   fabricated — flag any violation.
3. Is the comparison's conclusion (RAM savings from dropping the webview) actually
   supported by the numbers it presents, or does it conflate unrelated savings (e.g.
   smaller binary size, different allocator) with the webview removal specifically?
4. Any Slint API misuse or anti-pattern that would make the spike's numbers
   unrepresentative of a real production build (e.g. running in a debug/dev mode with
   extra debug allocations).

Output a markdown list of findings, ranked most-important first. If nothing is wrong,
say so explicitly rather than inventing filler feedback.

---

End your response with exactly one line, verbatim, based on your findings: `VERDICT: PASS` if there are no correctness-blocking issues (style nits are fine to pass with), or `VERDICT: FAIL` if there is at least one issue that must be fixed before this can merge. This line is parsed by an automated gate — do not omit it, do not add extra text after it.
