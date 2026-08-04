# 00 — Vision

## The problem

"RAM is temporary, disk is permanent" is a sentence students memorise and do not
understand. Diagrams do not fix it, because a diagram of a memory hierarchy is just
another thing to memorise. The gap is that nothing in normal computer use *forces* you
to notice which of the two you are using — the OS hides the boundary on purpose.

## The approach

Ephemera removes the hiding. It is a familiar file-manager UI (Google Drive is the
mental model) that has been split down the middle, with the two storage classes made
into two visible, differently-behaved places:

- **Uploading a file puts it in RAM and nowhere else.** No temp file, no cache, no
  autosave. The bytes are a `Vec<u8>` on the Rust process heap.
- **Quitting the app destroys them.** Not a simulation — the process exits and the
  allocation is reclaimed by the OS. Reopening the app shows an empty RAM pane.
- **Disk requires an explicit act.** A file only becomes durable because the user chose
  to make it durable, which is the single most important idea in the whole project.

The lesson lands because the user loses a file once. Everything else in the app exists
to explain *why* that happened and what it cost.

## Design principles

1. **The metaphor must be real, not decorated.** The RAM pane is backed by actual
   process memory; the disk pane is backed by actual files in a real folder. Nothing is
   faked. When we do fake something (see the latency throttle in
   [`06-open-questions.md`](06-open-questions.md)) it must be labelled in the UI as a
   teaching aid.
2. **Small numbers so the limits are reachable.** 10 MB of RAM and 20 MB of disk can be
   filled with a few photos in under a minute. A limit you never hit teaches nothing.
3. **Show the cost, always.** Every operation reports what it consumed: bytes, time,
   throughput. The dashboard is not decoration — it is the lesson.
4. **Honest about the leaky parts.** The OS pages RAM to swap; the page cache keeps
   "disk" reads in RAM. These complications are not hidden, they are the advanced
   material (see [`05-teaching-notes.md`](05-teaching-notes.md)).

## Who it is for

A learner or a teacher demonstrating to a class. It should be understandable with no
prompting after about thirty seconds of clicking, and should reward a second, closer
look with the dashboard.

## Success criteria

Ephemera works if a user can, unprompted, answer these after using it:

- Why did my file disappear?
- Why is the RAM pane smaller than the disk pane?
- Why did saving to disk take longer than uploading?
- Why does the app use 180 MB of memory when I only stored 4 MB of files?

## Explicit non-goals

- Not a real file manager. No folders, no sharing, no sync, no search.
- Not a benchmark tool. Timings are illustrative, not rigorous.
- Not multi-user, not networked, no accounts.
- Not a RAM disk. Ephemera does not create a `tmpfs` mount; the RAM store is ordinary
  heap memory inside the app process, which is a different and simpler thing.
