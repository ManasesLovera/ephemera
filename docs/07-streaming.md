# 07 — Streaming upload and the comparison report

## The idea

Every upload so far goes through the RAM store: the whole file lands in memory, and only
then can it be persisted elsewhere. That is **buffered I/O**, and it has a cost the app
has not yet shown directly — the file's *entire* size sits in memory for the duration.

**Streaming** is the alternative: read the source file in small, fixed-size chunks and
write each chunk straight to its destination, never holding more than one chunk at a
time. This is how real programs move files larger than available memory, and it is a
second core lesson sitting right next to the RAM/disk split.

Ephemera adds a second upload path — **"Stream to disk"** — next to the existing
"Upload to RAM" action. It bypasses the RAM store entirely and writes directly into the
vault, chunk by chunk. When it finishes, it shows a report comparing what actually
happened against what the buffered path would have cost.

## Why this belongs in the app

It closes the gap between "RAM has a size limit" (tier 1) and "why does that limit
matter for real work" (tier 2). A file too large for the 10 MB RAM cap can still be
copied to disk — streaming is *how*. It also motivates the DB and GCS tiers added in
[`08-database-tier.md`](08-database-tier.md) and [`09-gcs-tier.md`](09-gcs-tier.md):
uploading a 500 MB file to a database or a cloud bucket is only feasible because it is
streamed, never buffered whole.

## Mechanics

```rust
const STREAM_CHUNK: usize = 256 * 1024; // reused buffer, allocated once

async fn stream_upload_to_disk(
    source_path: PathBuf,
    dest_path: PathBuf,        // inside the vault, quota-checked before starting
    on_progress: Channel<TransferProgress>,
) -> Result<StreamReport, AppError> {
    let mut src = File::open(&source_path)?;
    let mut dst = File::create(&dest_path)?;
    let mut buf = vec![0u8; STREAM_CHUNK];   // the ONLY allocation for file data
    let total = src.metadata()?.len();
    let mut done = 0u64;

    let rss_baseline = sample_process_rss();
    let started = Instant::now();
    let mut rss_samples = Vec::new();        // populated by the 4 Hz metrics tick

    loop {
        let n = src.read(&mut buf)?;
        if n == 0 { break; }
        dst.write_all(&buf[..n])?;
        done += n as u64;
        on_progress.send(TransferProgress { bytes_done: done, bytes_total: total, .. })?;
    }
    dst.sync_all()?; // fsync — same honesty rule as every other disk write

    Ok(build_report(total, STREAM_CHUNK, started.elapsed(), rss_baseline, rss_samples))
}
```

**Key property:** peak memory attributable to this operation is `STREAM_CHUNK` (256 KB),
*regardless of file size*. A 2 MB file and a 200 MB file cost the same to stream. That
invariant is the entire lesson and should be stated explicitly in the report.

Quota check before starting: streaming still writes into the vault, so it is checked
against `MAX_DISK_BYTES` using the *known* file size (from filesystem metadata) before
the first chunk is read — same `assert_fits` used everywhere else. It is **not** checked
against the RAM cap, and — importantly — a file **larger than `MAX_SINGLE_FILE` (10 MB)
can be streamed** even though it could never be uploaded to RAM. Surface this in the UI:
attempting to drop an oversized file onto the RAM zone shows the rejection *and* a
suggestion to use "Stream to disk" instead.

## The report

Shown in a modal/panel immediately after a stream completes. Every field is either a
real measurement or a clearly-labelled derived/theoretical value — never presented as
interchangeable.

```rust
#[derive(Serialize)]
pub struct StreamReport {
    pub file_name: String,
    pub bytes_total: u64,
    pub chunk_size: u64,
    pub elapsed_ms: u64,
    pub throughput_mb_s: f64,

    // measured, from the metrics sampler running during this operation
    pub rss_baseline_bytes: u64,
    pub rss_peak_bytes: u64,
    pub rss_avg_bytes: u64,
    pub rss_peak_delta_bytes: i64,     // peak − baseline; expected to be small

    // derived — labelled "would have required", never "measured"
    pub buffered_equivalent_peak_bytes: u64,   // == bytes_total

    // derived — the concurrency payoff
    pub max_concurrent_streaming: u64, // floor(ram_cap / chunk_size)
    pub max_concurrent_buffered: u64,  // floor(ram_cap / bytes_total), min 0
}
```

`max_concurrent_*` answers the question in the brief directly: *how many files this size
could be processed at once, streaming vs. buffering through RAM?* Both are computed
against the same fixed budget — `MAX_RAM_BYTES` (10 MB) — so they are directly
comparable. For a 3 MB file: buffered allows 3 concurrent (`10 / 3`); streaming allows
40 (`10 / 0.25`). For an 8 MB file: buffered allows 1; streaming still allows 40 — the
gap widens as files get larger, which is the point.

> [!note]
> `max_concurrent_buffered` is a **theoretical** number derived from the cap and the file
> size, not a live measurement of concurrent uploads (the app does not need to actually
> run N simultaneous RAM uploads to know how many would fit). Label it as calculated.

### Report UI

- A stat pair, large: **"streaming peak: ~256 KB" vs. "buffered would need: 8.0 MB"** —
  the headline comparison, independent of file size.
- A small bar chart (sequential hue, magnitude job — see `03-ui-and-visualization.md`)
  with two bars: streaming RSS delta vs. buffered-equivalent bytes. Log-scale is
  acceptable *here specifically*, unlike the throughput chart — the point of this one
  chart is exactly to show the ratio compresses to something readable, not to make the
  reader feel the gap viscerally the way tier-1 does.
- The concurrency pair as two stat tiles: "N files at once, streaming" / "N files at
  once, buffered", same shared cap noted underneath both.
- `elapsed_ms` and `throughput_mb_s` reported plainly, same style as the existing
  per-transfer stats for RAM → disk persists, so the two paths are visually comparable.

## Where this sits in the tier graph

Streaming is a **path**, not a tier — it is a second way of getting a file from "not in
the app" to the disk tier, competing with upload-to-RAM-then-persist. It does not change
the tier graph described in [`01-requirements.md`](01-requirements.md#moving-files-and-the-tier-graph).
Streamed files land in the Disk tier exactly like a persisted RAM file does, and from
there can move to Database or GCS like any other disk file.

## Open question

Should streaming also target the Database and GCS tiers directly (stream source file →
DB row / GCS object, bypassing both RAM and local disk)? Architecturally straightforward
— same chunked-read loop, different sink — and it would make the concurrency comparison
even more dramatic for the 100 MB DB cap. **Recommendation:** build disk streaming first
(stage matches the existing build order), add DB/GCS streaming as a follow-on once the
pattern is proven. Tracked in [`06-open-questions.md`](06-open-questions.md).
