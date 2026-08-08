# Slint RSS Spike Measurement

This spike answers a narrow question: what is the resident memory baseline of a
small Slint window compared with the existing Tauri release application? It is not
an implementation of the Ephemera port.

## Reproduce

Run these commands from `spike-slint/`:

```bash
cargo build --release
./target/release/spike-slint --auto-load-after 10
```

The release profile is required. Compare it only with the Tauri release build,
not with a debug binary. The window starts with an empty Rust `Vec<u8>`. Record
the PID from another terminal after the window is visible:

```bash
pgrep -n -x spike-slint
PID=<pid printed above>
grep '^VmRSS:' /proc/$PID/status
```

That is the idle point. Click **Load 50 MiB**, wait for the UI to show `Rust
Vec<u8>: 50.0 MiB`, then record the post-allocation point using the same PID:

```bash
grep '^VmRSS:' /proc/$PID/status
```

The allocation is a real Rust `Vec<u8>` and each 4 KiB page is touched before the
timer reports it. The UI update timer is 250 ms, matching the application’s 4 Hz
dashboard cadence. Click **Clear buffer** before exiting if repeating a run.

The optional `--auto-load-after <seconds>` argument leaves the buffer empty until
the delay expires, then performs the same real allocation as the button. Capture
the idle point before the delay and the post-allocation point afterward.

## Tauri comparison

From the repository root, install dependencies if needed and build the matching
release application:

```bash
pnpm install
pnpm tauri build
```

Start the produced Linux binary. The exact path depends on the bundle target; find
it without launching a debug build:

```bash
find src-tauri/target/release -maxdepth 3 -type f -executable -name 'ephemera*' -print
```

Use the same idle and 50 MiB workload points as the Slint run. In the existing
Tauri UI, use the RAM pane’s browse action to load a 50 MiB file. Do not use
`stream to disk`, because that deliberately bypasses the RAM allocation. Record
the application PID and its child WebKitGTK process PIDs:

```bash
pgrep -n -x ephemera
PID=<ephemera pid>
pgrep -P $PID -a
```

Repeat the idle reading after the window is shown and the post-allocation reading
after the RAM meter reports 50 MiB. For a single-process comparison, the primary
number is `/proc/$PID/status` for the main process. For the application’s own
dashboard meaning, also sum `VmRSS` for the Tauri PID and all descendants, because
WebKitGTK puts web content and networking in child processes:

```bash
python3 - <<'PY' "$PID"
import pathlib
import sys

root = int(sys.argv[1])
pending = [root]
pids = []
while pending:
    pid = pending.pop()
    pids.append(pid)
    children = pathlib.Path(f"/proc/{pid}/task/{pid}/children")
    if children.exists():
        pending.extend(int(child) for child in children.read_text().split())

total = 0
for pid in pids:
    try:
        for line in pathlib.Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith("VmRSS:"):
                total += int(line.split()[1]) * 1024
                break
    except FileNotFoundError:
        pass
print(f"pids={pids} tree_vm_rss_bytes={total}")
PY
```

Use the same workload size, profile, displayed-window state, and measurement
commands for both applications. RSS is affected by the desktop session and
allocator state, so repeat runs are useful. Do not claim a WebKitGTK saving from
the main-process number alone when the Tauri child processes are present.

Because the Tauri post-allocation point could not be captured in this run, these
measurements do not answer whether removing WebKitGTK lowers the baseline after a
50 MiB workload. The directly observed idle values are reported for follow-up,
not as a causal WebKitGTK savings claim.

## Measurements from this worktree

The binaries were built and launched in release mode on 2026-08-08. Commands and
readings below are retained verbatim so they can be checked against the worktree’s
build.

### Slint

- Build command: `cargo build --release` from `spike-slint/`
- Launch command: `./target/release/spike-slint --auto-load-after 10`
- PID: `2609393`
- Idle command/result: `grep '^VmRSS:' /proc/2609393/status` -> `VmRSS:\t92428 kB`
- Post-50-MiB command/result: `grep '^VmRSS:' /proc/2609393/status` -> `VmRSS:\t143632 kB`
- UI confirmation: `Rust Vec<u8>: 50.0 MiB`

### Tauri

- Build command: `pnpm tauri build` from the repository root
- Launch command: `./src-tauri/target/release/ephemera`
- Main PID: `2628051`
- Idle main command/result: `grep '^VmRSS:' /proc/2628051/status` -> `VmRSS:\t198332 kB`
- Idle tree command/result: `pids=[2628051, 2628271, 2628258] tree_vm_rss_bytes=470605824`
- Post-50-MiB main/tree commands/results: **not available**. The release Tauri
  application only loads RAM through its GUI file picker/drop path. This Wayland
  session has no `wtype`, `ydotool`, or `xdotool`, and no CLI workload hook exists,
  so I could not trigger the same 50 MiB upload without changing the application
  under test. No post-allocation Tauri number was invented.

No RSS savings claim is made: the Tauri post-allocation point is unavailable, so
the two applications were not measured at both matched workload points.
