/// What one surface admits: every event, a priority page alone, or nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admits {
    All,
    Priority,
    None,
}

impl Admits {
    /// The word the config spells it with, which is also what the report
    /// prints: one spelling, so a refusal and a report cannot disagree.
    pub fn word(self) -> &'static str {
        match self {
            Admits::All => "all",
            Admits::Priority => "priority",
            Admits::None => "none",
        }
    }

    /// The word as the config may write it, or None for anything else.
    pub fn parse(word: &str) -> Option<Admits> {
        match word {
            "all" => Some(Admits::All),
            "priority" => Some(Admits::Priority),
            "none" => Some(Admits::None),
            _ => None,
        }
    }

    /// Whether this surface takes an event, given whether it is a page.
    pub fn admits(self, priority: bool) -> bool {
        match self {
            Admits::All => true,
            Admits::Priority => priority,
            Admits::None => false,
        }
    }
}

/// One profile's five settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub quiet: bool,
    pub banner: Admits,
    pub discord: Admits,
    pub phone: Admits,
    pub lights: Admits,
}

impl Default for Profile {
    /// TODAY'S BEHAVIOUR, which is what a machine with no `[profiles]` table
    /// has: every surface loud and no hush of its own.
    fn default() -> Self {
        Profile {
            quiet: false,
            banner: Admits::All,
            discord: Admits::All,
            phone: Admits::All,
            lights: Admits::All,
        }
    }
}

impl Profile {
    /// Whether a priority page reaches anything at all under this profile,
    /// WHEREVER THE OPERATOR IS.
    ///
    /// THE FLOOR IS `discord` ALONE, and it is checked at LOAD rather than at
    /// delivery: a config that cannot express a silenced page is one no rule
    /// and no override can select into silence. `banner` and `phone` are not
    /// part of this check because they are already conditioned on presence
    /// before a profile ever sees them (`crate::surface::plan`): banner fires
    /// only at the desk, phone never fires at the desk. A profile checked
    /// against "any one of the four" could pass this test while still
    /// silencing a page in practice, if the operator happened to be wherever
    /// the admitting surface does not reach. `discord` is the one leg
    /// `channel_plan` never masks by presence.
    pub fn admits_a_page(&self) -> bool {
        self.discord != Admits::None
    }

    /// Whether this profile's own hush applies to an event.
    ///
    /// NEVER TO A PAGE. A page crosses a profile's quiet the way a
    /// `bypass_mute = true` class crosses the operator's timed mute.
    pub fn hushes(&self, priority: bool) -> bool {
        self.quiet && !priority
    }
}

#[cfg(test)]
mod tests;
