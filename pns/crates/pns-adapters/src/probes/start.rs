use super::*;

impl<R: CommandRunner + Send + Sync + 'static> pns_application::ProbeStart for SystemProbes<R> {
    /// Begin the desk pair and the phone chain in the background, one thread
    /// each: see the module doc on `SystemProbes` and `join_desk`/`join_phone`
    /// below. NEITHER OVERRIDE IS CONSULTED HERE; the caller already answered
    /// that in `wants`, which is the one spelling of the override rule this
    /// and the read guards in `environment_reading::read_surface` share.
    ///
    /// EVERY THREAD STARTED HERE IS JOINED BY A READ ON THE SAME PATH before
    /// anything calls `std::env::set_var`: the guards in `environment_reading::read_surface`
    /// read exactly what they asked to start, so no probe thread outlives
    /// that function, and the one `set_var` in this crate (main's blocked
    /// path) runs after it returns. `set_var` is `unsafe` because libc
    /// readers such as `localtime_r` do not take std's environment lock;
    /// `Command::spawn` does, so the rule is the general contract of a
    /// multi-threaded process, not a spawn race. Keep it when adding a
    /// thread or a `set_var`.
    fn start(&self, wants: pns_application::Wants) {
        if wants.desk && self.idle.get().is_none() && self.desk_handle_absent() {
            let runner = Arc::clone(&self.runner);
            // CAPTURED BEFORE THE SPAWN, not read from inside the thread: a
            // caller that already read the lock inline (nothing in
            // production does, but nothing forbade it either) filled
            // `screen_locked` before `start` ever ran, and the thread must
            // not run `ioreg -n Root -d1` a second time for an answer
            // `join_desk`'s `OnceCell::set` would only discard.
            let lock_already_known = self.screen_locked.get().is_some();
            let handle = std::thread::Builder::new()
                .spawn(move || {
                    // THE LOCK RIDES THIS THREAD RATHER THAN ITS OWN: the
                    // engine's rule is "the lock is read only where idle
                    // answered" (see `join_desk`), so running it here, gated
                    // on the SAME idle result, is what keeps a failed idle
                    // read from spawning a second `ioreg` for an answer
                    // nothing can use.
                    let idle = idle_reading(&*runner);
                    let lock = (!lock_already_known && idle.is_some())
                        .then(|| lock_reading(&*runner))
                        .flatten();
                    (idle, lock)
                })
                // A THREAD THE OS REFUSES FALLS BACK TO THE INLINE READ: `ok()`
                // drops a spawn failure into "nothing started", which is
                // exactly the state `join_desk` and the trait impls already
                // treat as "compute it when asked".
                .ok();
            self.desk_handle.set(handle);
        }
        if wants.phone && self.phone_atime.get().is_none() && self.phone_handle_absent() {
            let runner = Arc::clone(&self.runner);
            let tty_dir = self.tty_dir.clone();
            let handle = std::thread::Builder::new()
                .spawn(move || phone_reading(&*runner, &tty_dir))
                .ok();
            self.phone_handle.set(handle);
        }
    }
}

impl<R: CommandRunner + Send + Sync + 'static> SystemProbes<R> {
    /// Whether a desk thread is currently in flight, without consuming the
    /// handle: `Cell` has no borrow for a value that is not `Copy`, so
    /// peeking means taking it out and setting it straight back, which is
    /// safe because only the one owning thread ever touches this cell (see
    /// the crate's `OnceCell is !Sync` note: the struct itself stays
    /// single-threaded, only the probe bodies run elsewhere).
    fn desk_handle_absent(&self) -> bool {
        let handle = self.desk_handle.take();
        let absent = handle.is_none();
        self.desk_handle.set(handle);
        absent
    }

    /// The phone twin of `desk_handle_absent`.
    fn phone_handle_absent(&self) -> bool {
        let handle = self.phone_handle.take();
        let absent = handle.is_none();
        self.phone_handle.set(handle);
        absent
    }

    /// Join the desk thread if `start` began one, filling BOTH cells it owns
    /// from that one join. A no-op wherever nothing was started: the trait
    /// impls' own `get_or_init` then computes inline exactly as before this
    /// existed, which is what makes a caller that never starts answer
    /// exactly as it always has.
    ///
    /// FILLING BOTH CELLS TOGETHER, even when the lock was never attempted
    /// (idle failed to parse), is what keeps a later `screen_locked()` read
    /// from spawning a second `ioreg` for an answer the thread already
    /// decided nothing could give: the cell holds `None` either way, and
    /// `None` already means "no reading" everywhere this crate reads it.
    pub(super) fn join_desk(&self) {
        if let Some(handle) = self.desk_handle.take() {
            let (idle, lock) = handle
                .join()
                .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
            let _ = self.idle.set(idle);
            let _ = self.screen_locked.set(lock);
        }
    }

    /// The phone twin of `join_desk`.
    pub(super) fn join_phone(&self) {
        if let Some(handle) = self.phone_handle.take() {
            let atime = handle
                .join()
                .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
            let _ = self.phone_atime.set(atime);
        }
    }
}
