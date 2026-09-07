use super::*;
/// Delete the state the lamps kept under their OLD names, and never read it.
///
/// THE DEPLOY TRANSITION, and it is a deletion rather than a migration. Every
/// one of these files is derived from the machine on the next tick anyway (a
/// wait re-arrives with its session's next event, a streak restarts the moment
/// work is seen), so carrying the contents forward would buy nothing and would
/// mean two readers of one fact for as long as the code lived.
///
/// THE DARK DIRECTION, which is what makes the held record safe to drop: the
/// old record named lamps a steady write was holding, and the binary that wrote
/// them is gone. Deleting it leaves at most one lamp lit until the operator's
/// next event, and keeping it would have the NEW tick clear lamps it never
/// wrote by names it never chose.
///
/// ONCE, WITHOUT A MARKER TO SAY SO. A removal of a name that is not there is
/// one failed syscall, so the deletion happens exactly once and every tick after
/// it pays three of those rather than a fourth state file.
pub fn sweep_legacy_state(state: &Path) {
    for legacy in ["lights-glow", "lights-working-since"] {
        let _ = std::fs::remove_file(state.join(legacy));
    }
    let _ = std::fs::remove_dir_all(state.join("lights-needs"));
}
