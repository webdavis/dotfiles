/// What `moshi-hook status` said about this host, in the only two shapes pns
/// is willing to state: a graded local fact, and moshi's own sentence about
/// the server relayed word for word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingReport {
    pub pairing: Pairing,
    /// moshi's `server:` sentence AS MOSHI WROTE IT, RELAYED AND NEVER
    /// GRADED. Held raw: the printable filter and the relay cap belong to
    /// `pairing_lines`, which is the point the sentence becomes something
    /// printed, and putting them here as well would be two places to disagree
    /// about what is safe to print.
    /// `None` when moshi printed no such line, which is what an unpaired host
    /// prints today and what a moshi that renamed the line would print: both
    /// degrade to no relay and nothing else moves.
    pub server: Option<String>,
}

/// What the LOCAL pairing material says, which is all `status --json` knows.
///
/// `Paired` PROVES LESS THAN IT SOUNDS LIKE, and the line built from it must
/// never read as "approvals work". It says this host has pairing material on
/// disk and that moshi answered about it. It does NOT prove the running daemon
/// is serving that pairing (a re-pair mints a new host id while the live
/// daemon keeps the old one, and no daemon-side evidence is readable from
/// here), and it does not prove an approval will round trip, which needs a
/// human tapping a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pairing {
    /// moshi answered `paired: true`, and named this host these two ways.
    Paired {
        host_id: String,
        display_name: String,
    },
    /// moshi answered `paired: false`.
    Unpaired,
    /// moshi answered something with no `paired` in it.
    Unreadable,
    /// moshi did not answer at all.
    NoAnswer,
}

/// The pairing check's own lines, in the order they are printed: what pns
/// graded, then what moshi said, when moshi said anything.
pub fn pairing_lines(report: &PairingReport) -> Vec<String> {
    let mut lines = vec![format!(
        "{PREFIX}moshi pairing: {}",
        said_of(&report.pairing)
    )];
    if let Some(said) = report
        .server
        .as_deref()
        .map(printable)
        .filter(|said| !said.is_empty())
    {
        // ATTRIBUTED, because pns is not making this claim and could not
        // check it: the sentence is moshi's and the label says so.
        lines.push(format!("{PREFIX}moshi says: {said}"));
    }
    lines
}

/// Somebody else's text, made safe to put on a terminal, and capped. EVERY
/// string moshi chose goes through this: the relayed server sentence and the
/// two identity fields alike.
///
/// FILTERED AT THE POINT IT BECOMES A LINE, which is the only place that can
/// promise it: the report holds what moshi said, and this is what decides what
/// may be printed.
///
/// THE NEWLINE IS THE LOAD-BEARING ONE. An unfiltered newline would print a
/// second `pns doctor:` line that the operator would read as pns's own
/// verdict, and a report that can be made to lie about itself is worse than no
/// relay at all. The carriage return is the one that survives being split into
/// lines and returns a terminal's cursor to column zero for whatever follows
/// to overwrite the prefix with. Escapes, bells and every other control byte
/// go the same way, and so does anything outside ASCII, which is also what
/// makes the cap safe: a character is dropped whole, so the count can never
/// land inside a multi-byte sequence.
///
/// This does NOT reuse the decision log's identity filter, and the difference
/// is the point: that rule judges a short identity token that becomes a key's
/// value and replaces the whole thing when it fails, while this judges a
/// relayed English sentence full of spaces, parentheses, quotes and colons.
/// One predicate for both would have to be the wider of the two, which is the
/// narrower one weakened.
pub(super) fn printable(said: &str) -> String {
    said.chars()
        .filter(|character| *character == ' ' || character.is_ascii_graphic())
        .take(RELAY_MAX)
        .collect()
}

/// How much of somebody else's sentence this report is willing to carry. An
/// unbounded relay is an unbounded line in a report pns is responsible for.
const RELAY_MAX: usize = 200;

/// The one sentence each state has earned. EVERY ONE OF THEM IS BOUNDED BY
/// WHAT THIS CHECK CAN SEE: the paired line says who this host is paired as
/// and stops, and the three that could not answer say so rather than reading
/// as a verdict either way.
fn said_of(pairing: &Pairing) -> String {
    match pairing {
        // FILTERED THE SAME WAY THE RELAYED SENTENCE IS, because they are the
        // same kind of thing: strings another program chose, printed on the
        // operator's terminal inside a line pns signs its own name to. An
        // unfiltered newline in a `displayName` forges a `pns doctor:` line
        // exactly as one in the server sentence does.
        Pairing::Paired {
            host_id,
            display_name,
        } => format!(
            "paired as {} ({}).",
            printable(display_name),
            printable(host_id)
        ),
        // THE REMEDY IS IN THE LINE, because this is the state the whole check
        // exists for and it is invisible everywhere else: the census reports
        // the moshi channel green over its webhook the whole time, while every
        // approval card is going nowhere.
        Pairing::Unpaired => "this host is NOT paired, so every approval card is dead \
             until `moshi-hook pair` runs."
            .to_string(),
        Pairing::Unreadable => "moshi-hook answered something this cannot read.".to_string(),
        // BOTH EXPLANATIONS AND NEITHER CLAIM. The bounded spawn cannot tell
        // an absent binary from one that hung or one that exited non-zero, and
        // a machine that simply does not use moshi must not fail its doctor
        // forever, so this costs nothing on the exit code either.
        Pairing::NoAnswer => "moshi-hook did not answer (not installed, or it did not \
             answer in time), so the approval path could not be checked."
            .to_string(),
    }
}

/// How every line the doctor prints for itself is addressed.
pub(super) const PREFIX: &str = "pns doctor: ";

/// How a pairing verdict reads at a glance.
///
/// UNPAIRED IS BAD and the two unreadable states are not. A host that is not
/// paired has dead approval cards, which is a fault the operator can fix; a
/// moshi-hook that is absent or answering nonsense may simply not be installed
/// on this machine, and the exit code already treats those apart.
pub fn pairing_mark(report: &PairingReport) -> super::Mark {
    match report.pairing {
        Pairing::Paired { .. } => super::Mark::Good,
        Pairing::Unpaired => super::Mark::Bad,
        Pairing::Unreadable | Pairing::NoAnswer => super::Mark::Warn,
    }
}
