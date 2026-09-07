/// The exit code the two weekly jobs this ported from already use to mean
/// "nothing was attempted, try later" (a serialize-lock EX_TEMPFAIL). Matching
/// it is the whole point of this verdict: a lane exiting anything else stays a
/// failure.
///
/// THE COLLISION IS REAL BUT NARROW. The system header defines 75 only as a
/// generic temporary failure, and hermes itself already uses 75 for a
/// completed graceful gateway response, so a lane whose PROGRAM IS hermes, or
/// which propagates hermes's own exit code unchanged, could exit 75 for a
/// reason that has nothing to do with deferral. Verified against both
/// existing weekly jobs (2026-09-02): neither propagates an inner hermes exit
/// code outward, but only one of them calls hermes at all. The Homebrew job
/// never touches hermes. The agent-skills job guards its own call with
/// `command -v hermes` first, then captures the result as an `if` condition
/// (`if update_output="$(hermes ...)"; then ... else ... fi`), never reading
/// `$?` afterward; every exit either job actually returns comes from its own
/// explicit `exit N` statements alone. A future `command` lane whose `run` is
/// hermes itself, or that forwards hermes's own status unchanged, would
/// collide; the shipped config template says so.
pub const DEFERRED_EXIT_CODE: i32 = 75;
