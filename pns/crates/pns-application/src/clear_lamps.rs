use crate::{HeldLamps, LampBridge, LampWrite};

/// Put out whatever a steady glow write is still holding, and forget it.
///
/// THE FILE IS THE FENCE. An ordinary event reads whether it exists and stops
/// there, so every event that is not a return from an absence costs one failed
/// open and no network at all.
///
/// IT FORGETS EVEN THOUGH THE WRITE MIGHT HAVE FAILED, and the cost is stated
/// rather than coded around: `put` is fire and forget, so a refused clear is
/// invisible and the lamp stays lit with nothing recorded to put it out. That
/// is the same exposure the steady write already carries by not expiring, and
/// the alternative is worse: a record kept until somebody proved the write
/// landed would have every later event re-clearing a lamp that is already
/// dark, forever, on a machine whose daemon is down.
pub fn clear_held_lamps<B: LampBridge>(held: &impl HeldLamps, bridge: impl FnOnce() -> Option<B>) {
    // A RECORD THIS CANNOT READ NAMES NO LAMP TO PUT OUT, and it is KEPT: the
    // clear works off names alone, so there is nothing to write, and forgetting
    // the file would take the tick's only chance of repairing it with it.
    let Some(paths) = held.read() else {
        return;
    };
    if paths.is_empty() {
        return;
    }
    let Some(bridge) = bridge() else {
        return;
    };
    for entry in &paths {
        bridge.write(&entry.path, &LampWrite::Clear);
    }
    // The failure is DROPPED here, in this function's own stated style: the
    // PUTs are already out, so the worst a failed forget costs is one more
    // clear of a lamp that is already dark.
    let _ = held.remember(&[]);
}

#[cfg(test)]
mod tests;
