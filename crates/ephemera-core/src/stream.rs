use crate::error::AppError;
use crate::metrics::sample_rss_now;
use crate::types::{Direction, StreamReport, TransferProgress, MAX_RAM_BYTES, STREAM_CHUNK_BYTES};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

/// Reads `source` in fixed-size chunks and writes each directly to `dest`, never
/// holding more than one chunk in memory. Peak memory attributable to this operation
/// is `STREAM_CHUNK_BYTES` regardless of file size — that invariant is the whole point.
pub fn stream_copy(
    source: &Path,
    dest: &Path,
    file_id: String,
    mut on_progress: impl FnMut(TransferProgress),
) -> Result<StreamReport, AppError> {
    let mut src = std::fs::File::open(source)?;
    let total = src.metadata()?.len();
    let mut dst = std::fs::File::create(dest)?;
    let mut buf = vec![0u8; STREAM_CHUNK_BYTES];

    let rss_baseline = sample_rss_now();
    let mut rss_peak = rss_baseline;
    let mut rss_sum: u128 = 0;
    let mut rss_samples: u64 = 0;

    let started = Instant::now();
    let mut done: u64 = 0;
    loop {
        let n = src.read(&mut buf)?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n])?;
        done += n as u64;

        let rss_now = sample_rss_now();
        rss_peak = rss_peak.max(rss_now);
        rss_sum += rss_now as u128;
        rss_samples += 1;

        on_progress(TransferProgress {
            id: file_id.clone(),
            direction: Direction::StreamToDisk,
            bytes_done: done,
            bytes_total: total,
        });
    }
    dst.flush()?;
    dst.sync_all()?;

    let elapsed = started.elapsed();
    let elapsed_ms = elapsed.as_millis() as u64;
    let throughput_mb_s = if elapsed_ms > 0 {
        (total as f64 / 1_048_576.0) / (elapsed_ms as f64 / 1000.0)
    } else {
        0.0
    };
    let rss_avg = if rss_samples > 0 {
        (rss_sum / rss_samples as u128) as u64
    } else {
        rss_baseline
    };

    let chunk = STREAM_CHUNK_BYTES as u64;
    let max_concurrent_streaming = (MAX_RAM_BYTES / chunk).max(1);
    let max_concurrent_buffered = MAX_RAM_BYTES.checked_div(total).unwrap_or(0);

    Ok(StreamReport {
        file_name: source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        bytes_total: total,
        chunk_size: chunk,
        elapsed_ms,
        throughput_mb_s,
        rss_baseline_bytes: rss_baseline,
        rss_peak_bytes: rss_peak,
        rss_avg_bytes: rss_avg,
        rss_peak_delta_bytes: rss_peak as i64 - rss_baseline as i64,
        buffered_equivalent_peak_bytes: total,
        max_concurrent_streaming,
        max_concurrent_buffered,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_bytes_exactly_and_reports_correct_total() {
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("source.bin");
        // Larger than one chunk so the loop actually iterates more than once.
        let payload = vec![0x42u8; STREAM_CHUNK_BYTES * 3 + 12345];
        std::fs::write(&src_path, &payload).unwrap();
        let dst_path = dir.path().join("dest.bin");

        let mut progress_calls = 0u32;
        let report =
            stream_copy(&src_path, &dst_path, "id-1".into(), |_| progress_calls += 1).unwrap();

        let written = std::fs::read(&dst_path).unwrap();
        assert_eq!(written, payload);
        assert_eq!(report.bytes_total, payload.len() as u64);
        assert_eq!(report.buffered_equivalent_peak_bytes, payload.len() as u64);
        assert_eq!(report.chunk_size, STREAM_CHUNK_BYTES as u64);
        assert!(
            progress_calls >= 4,
            "expected at least 4 chunks, got {progress_calls} progress calls"
        );
    }

    #[test]
    fn concurrency_math_matches_the_documented_formula() {
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("source.bin");
        std::fs::write(&src_path, vec![0u8; 8 * 1024 * 1024]).unwrap(); // 8 MB
        let dst_path = dir.path().join("dest.bin");

        let report = stream_copy(&src_path, &dst_path, "id-2".into(), |_| {}).unwrap();

        // floor(10MB / 256KB) = 40
        assert_eq!(report.max_concurrent_streaming, 40);
        // floor(10MB / 8MB) = 1
        assert_eq!(report.max_concurrent_buffered, 1);
    }

    #[test]
    fn empty_file_streams_without_error() {
        let dir = tempfile::tempdir().unwrap();
        let src_path = dir.path().join("empty.bin");
        std::fs::write(&src_path, []).unwrap();
        let dst_path = dir.path().join("dest.bin");

        let report = stream_copy(&src_path, &dst_path, "id-3".into(), |_| {}).unwrap();
        assert_eq!(report.bytes_total, 0);
        assert_eq!(std::fs::read(&dst_path).unwrap().len(), 0);
    }
}
