use pns_domain::Delivery;

// A PANIC IS ONE LEG'S FAILURE, never the run's. Without this an
// unwinding channel takes the remaining legs and, in a hand-run
// check, the rest of the census with it, and a census that ended
// early is read as a report that finished. The default hook still
// prints its own trace to stderr, which is left alone: silencing
// it process-wide would hide every other panic in the binary.
pub fn deliver_guarded(name: &str, deliver: impl FnOnce() -> Delivery) -> Delivery {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(deliver)).unwrap_or_else(|_| {
        // NO PAYLOAD TEXT: a panic message is written for a developer
        // and may quote anything the channel was holding.
        Delivery::Failed(format!("the {} channel PANICKED; nothing was sent", name))
    })
}
