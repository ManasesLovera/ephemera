use slint::{ComponentHandle, Timer, TimerMode};
use std::time::Duration;

slint::include_modules!();

const BUFFER_SIZE: usize = 50 * 1024 * 1024;
const RAM_CAP: usize = 10 * 1024 * 1024;

fn rss_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let value = line.strip_prefix("VmRSS:")?.split_whitespace().next()?;
        value.parse::<u64>().ok().map(|kb| kb * 1024)
    })
}

fn format_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn main() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    let buffer = std::rc::Rc::new(std::cell::RefCell::new(Vec::<u8>::new()));
    let mut ticks_until_auto_load = std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .windows(2)
        .find(|args| args[0] == "--auto-load-after")
        .and_then(|args| args[1].parse::<u32>().ok())
        .map(|seconds| seconds.saturating_mul(4));

    {
        let buffer = buffer.clone();
        window.on_load_buffer(move || {
            let mut bytes = buffer.borrow_mut();
            bytes.resize(BUFFER_SIZE, 0);
            // Touch each page so the 50 MiB allocation is resident, not virtual.
            for byte in bytes.iter_mut().step_by(4096) {
                *byte = 1;
            }
        });
    }
    {
        let buffer = buffer.clone();
        window.on_clear_buffer(move || buffer.borrow_mut().clear());
    }

    let timer_window = window.as_weak();
    let timer_buffer = buffer.clone();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
        let Some(window) = timer_window.upgrade() else { return };
        if let Some(ticks) = ticks_until_auto_load.as_mut() {
            if *ticks == 0 {
                let mut bytes = timer_buffer.borrow_mut();
                bytes.resize(BUFFER_SIZE, 0);
                for byte in bytes.iter_mut().step_by(4096) {
                    *byte = 1;
                }
                ticks_until_auto_load = None;
            } else {
                *ticks -= 1;
            }
        }
        let size = timer_buffer.borrow().len();
        window.set_meter_progress((size.min(RAM_CAP) as f32) / RAM_CAP as f32);
        window.set_used_text(format_bytes(size).into());
        window.set_buffer_text(
            if size == 0 {
                "No buffer allocated".into()
            } else {
                format!("Rust Vec<u8>: {}", format_bytes(size)).into()
            },
        );
        window.set_rss_text(
            rss_bytes()
                .map(|bytes| format!("RSS: {}", format_bytes(bytes as usize)))
                .unwrap_or_else(|| "RSS: unavailable".into())
                .into(),
        );
    });

    window.run()
}
