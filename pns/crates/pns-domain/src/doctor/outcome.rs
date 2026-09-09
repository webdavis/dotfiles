use super::{Pairing, PairingReport};

/// One registered plugin and what checking it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Check {
    /// The plugin's config-table name, which is also how its line is labelled.
    pub plugin: &'static str,
    pub kind: CheckKind,
}

/// What a check does, decided from the registration and the selection alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    /// One test event through this channel's own delivery path.
    Send,
    /// A signal to the lights, which no event dispatches: counted in rooms,
    /// because the bridge acknowledges no write.
    Pulse,
    /// A reading rather than a send: what the room sensor currently says.
    Presence,
    /// Nothing to check, and why.
    Skipped(&'static str),
}

/// What loading the config found, as far as the census is concerned.
///
/// IT DECIDES ONE SENTENCE: what a registered plugin the selection left out is
/// reported with. The three states are three different edits, and one wording
/// covering them sends two thirds of the operators to the wrong one. "Not
/// enabled in the config" used to be the only one, which was harmless while a
/// machine with no config ran the whole roster and nothing was ever skipped on
/// it; the core fallback made that sentence the ORDINARY report on a fresh
/// machine, pointing the operator at a file that does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigState {
    /// A config was read, so a plugin outside the selection is one it did not
    /// switch on.
    Read,
    /// There is no config file, so the core is all that runs.
    Absent,
    /// A config file exists and could not be read, so the core is all that
    /// runs. It is told apart from `Absent` because one is fixed by writing a
    /// file and the other by repairing one.
    Unreadable,
}

/// What one check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It arrived, and the channel said this about it.
    Sent(String),
    /// It arrived, and the channel had nothing to say. An executable channel
    /// is silent by design, so claiming success for it would be claiming what
    /// the code does not provide.
    SentUnreported,
    /// It did not arrive, and the channel said this about that.
    Failed(String),
    /// The lights, and how many rooms were signalled.
    Signalled(usize),
    /// What the room sensor reads right now, in every state it can be in, and
    /// the decision the narrowing ring last recorded (`None` for a ring with
    /// nothing in it).
    Presence(
        crate::presence::PresenceStatus,
        Option<crate::PresenceDecision>,
    ),
    /// Nothing was checked, and why.
    Skipped(&'static str),
}

/// The one line this check earned.
pub fn line(check: &Check, outcome: &Outcome) -> String {
    let plugin = check.plugin;
    match outcome {
        Outcome::Sent(said) => format!("{plugin}: sent, {said}"),
        Outcome::SentUnreported => format!("{plugin}: sent, this channel reports no outcome"),
        Outcome::Failed(said) => format!("{plugin}: FAILED, {said}"),
        // NEITHER CLAIM IS MADE. Zero rooms is a bridge that answered no
        // listing OR a configured name nothing matched, and the line names
        // both rather than picking one; a count above zero says the rooms were
        // addressed and stops there, because the bridge acknowledges no write.
        Outcome::Signalled(0) => format!(
            "{plugin}: FAILED, signalled no rooms \
             (no room listing from the bridge, or no configured room name matched)"
        ),
        Outcome::Signalled(1) => format!("{plugin}: signalled 1 room ({WATCH_FOR_IT})"),
        Outcome::Signalled(rooms) => format!("{plugin}: signalled {rooms} rooms ({WATCH_FOR_IT})"),
        Outcome::Skipped(reason) => format!("{plugin}: skipped, {reason}"),
        Outcome::Presence(status, last_narrowing) => {
            super::presence::presence_said(plugin, status, last_narrowing.as_ref())
        }
    }
}

/// What the operator has to do to confirm a pulse, since nothing else can.
const WATCH_FOR_IT: &str = "watch for the flash; the bridge acknowledges no write";

/// The last line: how the whole run went.
pub fn summary(outcomes: &[Outcome]) -> String {
    let count = |wanted: Verdict| outcomes.iter().filter(|o| verdict(o) == wanted).count();
    format!(
        "pns doctor: {} sent, {} failed, {} skipped",
        count(Verdict::Sent),
        count(Verdict::Failed),
        count(Verdict::Skipped)
    )
}

/// What the shell learns.
///
/// NOT THE ALWAYS-EXIT-0 CONTRACT'S TERRITORY: that covers the hook and
/// notification paths, where a non-zero exit fails the turn being reported on.
/// This is hand typed and is never a hook.
///
/// THE PAIRING IS AN ARGUMENT RATHER THAN A SECOND CODE THE CALLER COMBINES,
/// which is the same rule the summary and this function already share: two
/// contributors decided at one point cannot disagree, and two decided at two
/// call sites eventually will.
pub fn exit_code(outcomes: &[Outcome], pairing: &PairingReport) -> i32 {
    if outcomes
        .iter()
        .any(|outcome| verdict(outcome) == Verdict::Failed)
    {
        return 1;
    }
    // AN UNPAIRED HOST IS A DEAD APPROVAL PATH, and it is the one pairing
    // state that moves this: the check only reaches it on a machine where
    // moshi-hook is installed and answering, and there an unregistered host
    // means every card is going nowhere while the census reports the mobile
    // channel green over its webhook. The other three states could not check
    // and are inert, so a machine that does not use moshi still exits 0.
    if pairing.pairing == Pairing::Unpaired {
        return 1;
    }
    // A CHECK WITH NOTHING TO CHECK MUST NEVER REPORT GREEN, which is the same
    // ruling the mute took: reporting success for something that is not in
    // effect is the worst outcome available.
    i32::from(
        !outcomes
            .iter()
            .any(|outcome| verdict(outcome) == Verdict::Sent),
    )
}

/// The three buckets every outcome falls into, decided ONCE so the summary's
/// counts and the exit code cannot read the same run differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Sent,
    Failed,
    Skipped,
}

fn verdict(outcome: &Outcome) -> Verdict {
    match outcome {
        Outcome::Sent(_) | Outcome::SentUnreported => Verdict::Sent,
        Outcome::Failed(_) => Verdict::Failed,
        // A PULSE THAT REACHED NO ROOM REACHED NOTHING. It is the shape every
        // hue misconfiguration takes, and an enabled channel that could not be
        // attempted is exactly what the exit contract calls a failure.
        Outcome::Signalled(0) => Verdict::Failed,
        Outcome::Signalled(_) => Verdict::Sent,
        Outcome::Skipped(_) => Verdict::Skipped,
        // A READING IS NEVER A SEND, in any state. Nothing was delivered
        // through the sensor and nothing failed to be, so it counts with the
        // checks that had nothing to send rather than moving the exit code
        // in either direction: a bridge that stopped answering costs the
        // lights their narrowing, and no notification at all.
        Outcome::Presence(..) => Verdict::Skipped,
    }
}

/// How a check's verdict reads at a glance.
///
/// ONLY A FAILED SEND IS BAD, and a skip never is. A plugin the config left off
/// is the operator's own choice, and grading a choice as a fault teaches them to
/// scroll past the marks.
pub fn outcome_mark(outcome: &Outcome) -> super::Mark {
    match outcome {
        Outcome::Sent(_) | Outcome::SentUnreported | Outcome::Signalled(_) => super::Mark::Good,
        Outcome::Failed(_) => super::Mark::Bad,
        Outcome::Skipped(_) | Outcome::Presence(_, _) => super::Mark::Note,
    }
}
