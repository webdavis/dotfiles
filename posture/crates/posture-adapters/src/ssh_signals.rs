use posture_application::SshInstallSignals;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

static OWNED: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicI32 = AtomicI32::new(0);
static CANCELLABLE: AtomicBool = AtomicBool::new(false);

pub struct SshSignals {
    previous: Vec<(i32, libc::sigaction)>,
    armed: bool,
}
impl SshSignals {
    pub fn arm() -> io::Result<Self> {
        OWNED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| io::Error::other("an SSH install already owns signal handling"))?;
        PENDING.store(0, Ordering::SeqCst);
        CANCELLABLE.store(true, Ordering::SeqCst);
        let mut guard = Self {
            previous: Vec::new(),
            armed: true,
        };
        // All fields are initialized before installation; the handler only touches lock-free atomics.
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = receive as *const () as usize;
        unsafe {
            libc::sigemptyset(&mut action.sa_mask);
        }
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            unsafe {
                libc::sigaddset(&mut action.sa_mask, signal);
            }
        }
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            let mut old = unsafe { std::mem::zeroed() };
            if unsafe { libc::sigaction(signal, &action, &mut old) } == -1 {
                return Err(io::Error::last_os_error());
            }
            guard.previous.push((signal, old));
        }
        Ok(guard)
    }
    pub fn reraise(&mut self) {
        let signal = self.pending();
        self.disarm();
        if let Some(signal) = signal {
            // Rollback has finished. Restore a terminal disposition and unblock only this signal
            // on this thread so the caller observes a signal death, not an ordinary exit 128+n.
            unsafe {
                libc::signal(signal, libc::SIG_DFL);
                let mut set = std::mem::zeroed();
                libc::sigemptyset(&mut set);
                libc::sigaddset(&mut set, signal);
                libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut());
                libc::raise(signal);
            }
        }
    }
}
impl SshInstallSignals for SshSignals {
    fn pending(&self) -> Option<i32> {
        let signal = PENDING.load(Ordering::SeqCst);
        (signal != 0).then_some(signal)
    }
    fn defer(&mut self) {
        CANCELLABLE.store(false, Ordering::SeqCst);
    }
    fn disarm(&mut self) {
        if !self.armed {
            return;
        }
        self.defer();
        for (signal, previous) in self.previous.drain(..) {
            // These exact dispositions were read when this guard took ownership.
            unsafe {
                libc::sigaction(signal, &previous, std::ptr::null_mut());
            }
        }
        self.armed = false;
        OWNED.store(false, Ordering::SeqCst);
    }
}
impl Drop for SshSignals {
    fn drop(&mut self) {
        self.disarm();
    }
}

extern "C" fn receive(signal: i32) {
    let _ = PENDING.compare_exchange(0, signal, Ordering::SeqCst, Ordering::SeqCst);
}
pub fn ssh_install_cancelled() -> bool {
    CANCELLABLE.load(Ordering::SeqCst) && PENDING.load(Ordering::SeqCst) != 0
}

#[cfg(test)]
mod tests;
