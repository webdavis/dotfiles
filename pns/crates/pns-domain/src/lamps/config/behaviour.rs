/// What a lamp can say. A CLOSED SET, which is the whole reason `[lights]` is
/// judged here instead of passed through as a plugin's free-form settings: a
/// `behaviours` list holding a word nothing matches is a lamp that stays dark
/// while the operator is sure they routed it, with no message anywhere.
///
/// `Unseen` IS ONE WORD AND CARRIES TWO COLOURS. Its success and failure
/// flavours always ride the same lamp, so a config cannot route one without the
/// other and there is no spelling for trying. `Checks` is the second such
/// word: its pass and its failure are `Flash`'s two arms, and its pair is the
/// one pair `[lights.checks]` states in the config rather than locking here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Behaviour {
    Done,
    Failed,
    Blocked,
    Unseen,
    Looping,
    Checks,
}

/// The six words, in the spelling a config uses, and the order the refusal
/// lists them in.
pub const BEHAVIOUR_WORDS: [(&str, Behaviour); 6] = [
    ("done", Behaviour::Done),
    ("failed", Behaviour::Failed),
    ("blocked", Behaviour::Blocked),
    ("unseen", Behaviour::Unseen),
    ("loop", Behaviour::Looping),
    ("checks", Behaviour::Checks),
];
