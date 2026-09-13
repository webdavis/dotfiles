use std::sync::atomic::{AtomicI32, Ordering};

static REQUESTED: AtomicI32 = AtomicI32::new(0);

extern "C" fn request(signal: libc::c_int) {
    // Rust atomics are lock-free. No allocation, locks or I/O in a signal handler.
    let _ = REQUESTED.compare_exchange(0, signal, Ordering::Relaxed, Ordering::Relaxed);
}

/// Install once at command startup, before spawning any threads or children.
pub fn install_interruption() -> std::io::Result<()> {
    // SAFETY: sigaction is initialized before use; the handler has static lifetime
    // and only accesses a lock-free atomic. An empty mask adds no blocked signals.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = request as *const () as libc::sighandler_t;
        libc::sigemptyset(&mut action.sa_mask);
        for signal in [libc::SIGINT, libc::SIGTERM] {
            if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
    }
    Ok(())
}

pub fn interruption() -> Option<i32> {
    match REQUESTED.load(Ordering::Relaxed) {
        0 => None,
        signal => Some(signal),
    }
}

pub(crate) fn refusal() -> Option<String> {
    interruption().map(|signal| format!("interrupted by signal {signal}; no command started"))
}

#[cfg(test)]
mod tests;
