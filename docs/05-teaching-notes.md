# 05 — Teaching notes

The concepts the app should convey, each paired with the UI element that carries it.
This doubles as the source text for the in-app "why?" popovers.

## Tier 1 — the core lesson

### Volatility

**Idea:** RAM holds data only while powered. Cut the power and the contents are gone —
not deleted, just no longer held.

**Carried by:** files vanishing on app restart; the "pull the plug" button.

**Say it as:** *RAM remembers by continuously being told to. Stop telling it — close the
program, cut the power — and it forgets instantly. Disk remembers by being physically
changed, so it keeps remembering with the power off.*

### Persistence is an act, not a property

**Idea:** nothing becomes durable by accident. Some piece of code has to decide to
write it down.

**Carried by:** the RAM → disk drag being the only route to durability.

**Say it as:** *Every "Save" button you have ever pressed is this moment. Everything you
have ever lost by not pressing one is this moment too.*

### Capacity and cost

**Idea:** RAM is fast and expensive, so there is little of it. Disk is slow and cheap,
so there is a lot.

**Carried by:** the 10 MB vs 20 MB caps and the physically longer disk meter track.

**Say it as:** *The ratio on a real machine is far more extreme than 1:2 — a laptop with
16 GB of RAM often has 1 TB of disk, roughly 1:64. We shrank it so you can fill both in
a minute.*

### Latency

**Idea:** reaching RAM takes nanoseconds; reaching durable storage takes microseconds
to milliseconds — orders of magnitude, not percentages.

**Carried by:** the throughput chart; the `fsync` on every persist.

**Say it as:** *If a RAM access took one second, an SSD access would take about two
minutes, and a spinning hard disk about three months.* (Scaled from ~100 ns / ~100 µs /
~10 ms.)

## Tier 2 — the second look

### The app itself lives in RAM

**Idea:** the program's own code, its UI, its runtime, and its buffers are all in RAM
too. The stored files are a small part of the total.

**Carried by:** the app-memory stat tile and the second time-series chart.

**Say it as:** *You stored 4 MB of files and the app is using 187 MB. The other 183 MB
is the program itself — a browser engine, a UI, a runtime. This is why an "empty" app is
never free.*

This is the answer to a question most learners already have and cannot usually get
answered: *why does my computer use so much memory when I have nothing open?*

### A reference is not a copy

**Idea:** passing data around does not necessarily duplicate it. `Arc<[u8]>` hands over
a pointer, not 9 MB.

**Carried by:** persisting a large file without the RAM meter spiking.

**Say it as:** *Two names for the same bytes cost the same as one. Copying is a choice,
and it is usually the expensive one.*

### The filesystem is shared, mutable, and public

**Idea:** the RAM store is private to this process; the vault folder is visible to every
other program on the machine.

**Carried by:** "open folder" showing the real files; rescan-on-focus picking up
externally added or deleted files.

**Say it as:** *Nothing else on your computer can reach into this app's memory. Anything
on your computer can reach into that folder.*

### Quotas and eviction

**Idea:** when a fixed-size fast tier fills, something must leave before something else
can enter. That decision is a *policy*.

**Carried by:** the quota-exceeded state and having to delete a file to make room.

**Say it as:** *You just performed cache eviction by hand. Real systems automate this
policy — least-recently-used, least-frequently-used — and choosing badly is why things
feel slow.*

### Buffered I/O

**Idea:** data usually lands in fast memory first and is written out in batches.

**Carried by:** disk cap (20 MB) exceeding RAM cap (10 MB), forcing an
upload → persist → clear → upload again loop.

**Say it as:** *You cannot hold the whole 20 MB at once, so you did it in two passes.
That is exactly how a program writes a file larger than its memory.*

## Tier 3 — the honest complications

Save these for the end. Introducing them early undermines the clean model before it has
landed, but omitting them entirely would leave the learner with a model that breaks the
first time they meet a real system.

### Swap — RAM can secretly be on disk

The OS may move idle memory pages to a swap file. Some of what the app calls "RAM" may
physically be sitting on the disk, and touching it will be dramatically slower.

Possible demo: show swap usage in the metrics panel via `sysinfo`, or simply state it.
Deliberately triggering swap with 10 MB caps is not realistic, so this is likely a text
note rather than an interaction.

### The page cache — "disk" reads that never touch the disk

The OS keeps recently-read file contents in RAM. Reading a file a second time can be
100× faster than the first, without the file having moved.

**This is demonstrable and worth building.** Read a file from the vault twice and chart
both timings. The second read being far faster, with nothing else having changed, is a
genuinely surprising result that teaches caching better than any diagram.

Getting a cold read again requires dropping the cache (`echo 3 | sudo tee
/proc/sys/vm/drop_caches`), which needs root — so frame it as a one-way demo: the first
read after a fresh boot is the slow one.

`fsync` on writes is the write-side counterpart, and it is why our persist timings are
honest — see [`02-architecture.md`](02-architecture.md).

### Copies across a boundary

Moving 10 MB from the webview into Rust can transiently cost 20–30 MB depending on the
IPC path. The architecture chooses the path-based route partly to avoid this — but the
fact that a naive implementation would triple the memory cost is itself worth telling.

### This is not a RAM disk

A `tmpfs` mount also stores files in RAM, but presents them as a filesystem with paths.
Ephemera's RAM store is plain heap memory inside one process, addressed by pointers.
Worth a sentence so the learner does not conflate them.

## Suggested guided lesson

A scripted path for classroom use — could ship as an optional walkthrough:

1. Upload a small file. Notice: RAM meter moves, disk untouched.
2. Watch the live chart while uploading a 4 MB file. Notice: it climbs in real time.
3. Reload the window (F5). Notice: files are still there — the UI is not the store.
4. Quit the app. Reopen. Notice: **they are gone.**
5. Upload again. Drag it to disk. Quit. Reopen. Notice: it survived.
6. Fill RAM past 10 MB. Read the quota error. Delete something. Try again.
7. Fill disk to 20 MB — discover that it takes two passes, and understand why.
8. Open the Instruments drawer. Compare RAM and disk throughput.
9. Look at the app-memory tile. Ask why 4 MB of files costs 187 MB.

Step 4 is the one that teaches. Everything before it exists to make the loss feel like a
loss, and everything after exists to explain it.
