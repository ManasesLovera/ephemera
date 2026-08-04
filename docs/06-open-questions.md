# 06 — Open questions

Decisions still to make. Each carries a recommendation so the next session can proceed
by confirming rather than re-deriving.

## Answered in the specifying conversation

Recorded so they are not reopened by accident.

| Question | Decision |
| --- | --- |
| Frontend framework | React + TypeScript |
| Desktop shell | Tauri 2 (Rust) |
| Project name | **Ephemera** |
| RAM cap | 10 MB |
| Disk cap | 20 MB, deliberately 2× RAM |
| Pane layout | Two panes, side by side |
| Where the RAM store lives | Rust process heap, not the webview |
| Does a webview reload clear RAM? | **No** — state is in Rust; this is a deliberate demo |

## Open — needs a decision

### 1. Artificial disk latency: real, simulated, or both?

Real `fsync` on a modern NVMe drive writing 5 MB may complete in tens of milliseconds —
fast enough that the RAM/disk difference is real but not *felt*, especially on a
projector.

**Recommendation:** measure real timings always and display them as the truth. Add an
optional **"slow disk" throttle** with presets (NVMe / SATA SSD / 7200 rpm HDD / floppy)
that inserts a delay between chunks. It must be **off by default**, and while on, every
affected number must be visibly marked as simulated. A fabricated figure presented as
measured would undermine the app's whole premise — but a labelled simulation of a
spinning disk is genuinely instructive, since most learners have never used one.

### 2. Warn before quitting with unpersisted files?

**Recommendation:** show a dismissible warning that does **not** block the quit, and
whose primary button is "Quit anyway". Losing a file once is the lesson; an app that
rescues the user from it teaches nothing. Consider making the warning appear only from
the second session onward, so the first loss is clean.

### 3. Should the caps be user-configurable?

**Recommendation:** yes, but hidden in settings with 10/20 MB as defaults and a
prominent reset. A teacher may want a 1 MB RAM cap for a fast demo. Do not surface it in
the main UI — a limit that looks adjustable does not feel like a limit.

### 4. Language — English, Spanish, or both?

The user works in Spanish contexts. Teaching material is far more effective in the
learner's first language, and the copy in this app *is* the teaching material.

**Recommendation:** decide before writing UI copy, since retrofitting i18n is tedious.
If undecided, structure strings in a single `strings.ts` module from day one so adding
`es` later is mechanical. Ask the user.

### 5. Multiple vault folders / profiles?

**Recommendation:** no. One vault. Multiple would complicate the accounting and add
nothing pedagogically.

### 6. Should disk → RAM be a move or a copy?

Copying leaves the file in both places (matching how opening a file really works). Moving
would make the panes mutually exclusive, which is tidier but *wrong* — a real read does
not delete the file.

**Recommendation:** copy, and show the file in both panes with a "both" indicator in the
file table. The shared color makes the relationship visible.

### 7. Page-cache demo — build it or just describe it?

Demonstrable and genuinely surprising (see [`05-teaching-notes.md`](05-teaching-notes.md)),
but a repeatable cold read needs root to drop caches.

**Recommendation:** build the timing comparison, present it as "first read vs. repeat
read", and do not attempt to drop caches. Defer to after the core app works.

### 8. Persist the metrics history across restarts?

**Recommendation:** no. Writing the RAM-usage history to disk in an app about the
difference between RAM and disk would be a genuinely confusing thing to do. The chart
starts empty on every launch, which is itself on-message.

### 9. Packaging and distribution

Is this run from `pnpm tauri dev`, or does it need a real `.deb` / AppImage for others
to install?

**Recommendation:** dev-mode only until the app works, then produce an AppImage if it is
going to be handed to students. Note that `.deb` on Ubuntu 26.04 built against
webkit2gtk-4.1 will not install on older distros — AppImage is safer for sharing.

## Risks to watch

| Risk | Mitigation |
| --- | --- |
| Tauri drag-drop suppressing HTML5 DnD | Documented in [`02-architecture.md`](02-architecture.md); use Tauri events for OS drops, dnd-kit for in-app |
| Tauri 2 raw-IPC API details having drifted | Path-based file reading avoids the boundary entirely; treat raw IPC as fallback |
| `sysinfo` missing WebKit child processes | Walk the process tree; verify the number against `htop` early |
| 4 Hz metrics causing re-render storms | Zustand selectors; profile once the dashboard exists |
| Blank window under Wayland | `WEBKIT_DISABLE_DMABUF_RENDERER=1` — see [`04-tech-stack.md`](04-tech-stack.md) |
| Scope creep into a real file manager | The non-goals in [`00-vision.md`](00-vision.md) are the guardrail |

## Suggested build order

Each stage should be independently runnable and demonstrable.

1. Scaffold Tauri + React + Tailwind; confirm the window opens under Wayland.
2. Rust `RamStore` with quota enforcement; upload via the file picker only; plain list UI.
   **Milestone: the core lesson works** — files vanish on restart.
3. Vault config, `persist_to_disk` with `fsync`, disk list, disk quota.
   **Milestone: the app teaches its whole point.**
4. Metrics sampler + `metrics://tick`; stat tiles.
5. Segmented meters with per-file colors; palette validated for both modes.
6. Drag and drop, both mechanisms.
7. Time-series charts and throughput comparison.
8. Motion, "pull the plug", quota error states, the "why?" popovers.
9. Tier-3 extras: page-cache demo, latency throttle, guided walkthrough.

Stages 1–3 are the actual project. Everything after is what makes it teach well.
