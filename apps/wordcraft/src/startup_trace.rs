//! Opt-in monotonic startup diagnostics; no document paths or content are recorded.
use std::io::Write;
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;
static START: OnceLock<Instant> = OnceLock::new();
static ENABLED: OnceLock<bool> = OnceLock::new();
static FIRST_UI: AtomicBool = AtomicBool::new(false);

pub fn mark(phase: &str) {
    let start = START.get_or_init(Instant::now);
    if *ENABLED.get_or_init(|| std::env::var_os("OMAWORD_STARTUP_TRACE").as_deref() == Some(std::ffi::OsStr::new("1"))) {
        let _ = writeln!(std::io::stderr().lock(), "oma-startup elapsed_ms={} phase={phase}", start.elapsed().as_millis());
    }
}

pub fn first_ui() {
    if !FIRST_UI.swap(true, Ordering::Relaxed) {
        mark("first-ui-entry");
    }
}
