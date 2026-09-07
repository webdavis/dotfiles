use std::collections::BTreeMap;

/// The idle threshold the bash defaults to when `PNS_DESK_IDLE_SECS` says
/// nothing: past this the operator counts as away from the desk.
pub const DEFAULT_DESK_IDLE_SECS: u64 = 120;

/// Everything the environment may override, parsed once at the edge.
/// Garbage numeric values read as absent, never as zero.
#[derive(Debug, Default, PartialEq)]
pub struct Overrides {
    pub idle_secs: Option<u64>,
    pub desk_idle_secs: Option<u64>,
    pub skip_phone: bool,
    pub force_phone: bool,
    /// A stated age for the phone's input clock, in seconds. The discovery
    /// chain behind that reading walks live processes, so a caller who
    /// already knows the answer states it and the walk never runs.
    pub phone_input_age: Option<u64>,
    /// Set when the variable was PRESENT and non-empty but not a count. The
    /// bash validators reject such a value outright rather than falling back,
    /// and the fallback is what would turn an unknown into a confident
    /// number: a probe reading where the caller overrode it, or a default
    /// threshold where the caller's was garbled.
    pub idle_invalid: bool,
    pub desk_invalid: bool,
    pub phone_invalid: bool,
    /// The operator's own typed mute, and ONE OF THE TWO FIELDS HERE THAT
    /// NEVER COME FROM THE ENVIRONMENT: it is read off a state file by the
    /// composition root and stated there. `from_env` must keep leaving it
    /// false, because a variable able to set it would let any producer mute
    /// the operator, and one able to clear it would silently end a mute they
    /// are still inside.
    pub muted: bool,
    /// A macOS Focus THE CONFIG NAMED is asserted right now, which is the
    /// operating system's own mute rather than a reading about where the
    /// operator is. It is not "a Focus is on": `[focus] silence` lists the
    /// modes that mean it, and this is already the answer to "is one of those
    /// the mode that is on".
    ///
    /// THE SECOND FIELD NEVER SET FROM THE ENVIRONMENT, for the reason above
    /// it: the composition root reads the Do Not Disturb store and states the
    /// verdict, and a variable able to force it either way would let any
    /// producer silence the operator or punch through a Focus they set.
    pub focus_active: bool,
}

impl Overrides {
    /// The operator told everything to be quiet: their own typed mute, or a
    /// macOS Focus they named in `[focus] silence`.
    ///
    /// ONE CONDITION, ONE SPELLING. The arbitration below is its first reader
    /// and the lights' own gate at the composition root is its second, and two
    /// copies of "is this event silenced" are how the two come to disagree
    /// about a lamp the operator muted.
    pub fn silenced(&self) -> bool {
        self.muted || self.focus_active
    }

    /// Whether the idle guard in `read_surface` would consult the idle
    /// probe (and, only if that answers, the lock probe qualifying it): a
    /// stated or garbled idle clock answers the question outright, and
    /// nothing underneath an outright answer is worth spawning.
    ///
    /// ONE SPELLING, read by `start` and by the guard alike, so a probe can
    /// never be started for a reading the caller already gave.
    pub fn reads_desk(&self) -> bool {
        !self.idle_invalid && self.idle_secs.is_none()
    }

    /// The phone twin: whether the phone-input guard would run the
    /// discovery chain instead of trusting a stated or garbled age.
    pub fn reads_phone(&self) -> bool {
        !self.phone_invalid && self.phone_input_age.is_none()
    }

    /// Parse the PNS_* and PNS_* variables out of an environment map.
    pub fn from_env(vars: &BTreeMap<String, String>) -> Self {
        // A present-but-garbled value is reported alongside the None, so the
        // caller can refuse it rather than fall back to a default.
        let read = |key: &str| match vars.get(key).filter(|raw| !raw.is_empty()) {
            None => (None, false),
            Some(raw) => {
                let parsed = crate::count::parse_count(raw);
                (parsed, parsed.is_none())
            }
        };
        let set = |key: &str| vars.get(key).is_some_and(|raw| !raw.is_empty());
        let (idle_secs, idle_invalid) = read("PNS_IDLE_SECS");
        let (desk_idle_secs, desk_invalid) = read("PNS_DESK_IDLE_SECS");
        let (phone_input_age, phone_invalid) = read("PNS_PHONE_INPUT_AGE");
        Self {
            idle_secs,
            desk_idle_secs,
            skip_phone: set("PNS_SKIP_PHONE"),
            force_phone: set("PNS_FORCE_PHONE"),
            phone_input_age,
            idle_invalid,
            desk_invalid,
            phone_invalid,
            // NO VARIABLE READS INTO EITHER OF THESE, deliberately: see the
            // fields.
            muted: false,
            focus_active: false,
        }
    }
}
