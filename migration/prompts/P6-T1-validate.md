Research task, read-only — do not edit files unless explicitly building a
measurement harness under `migration/outputs/P6-T1/` (fine to write scratch
measurement scripts there, nowhere else).

By this point all migration branches (`migration/P1-T1` through `migration/P5-T2`)
should have been merged by a human into a single working branch. Confirm that's
true (`git log --oneline -20`) before proceeding; if it isn't, report that and stop.

Re-run the RSS/memory comparison methodology from `migration/prompts/P0-T2-spike.md`
and `migration/outputs/P0-T2/`, but against the *full* merged Slint app instead of
the standalone spike. Compare against a fresh build of the pre-migration Tauri app
(check out `main` in a separate worktree if needed) under the same workload:

- idle RSS
- RSS after loading a file up to the RAM cap (10 MB)
- RSS after moving that file through disk/db/gcs tiers

Report actual numbers, not estimates. If the full-app numbers don't show the same
improvement the Phase 0 spike suggested, say so plainly — that's a valid and
important finding, not a failure to hide.
