//! What ONE pulse runs at.

/// A pulse's own state, which is not the same thing as the word that routes
/// it.
///
/// `Held` MAKES THE SAME DISTINCTION FOR THE BREATHING STATES, and for the
/// same reason: `checks` is one routable word carrying TWO colours, exactly as
/// `unseen` is, so a config cannot route a GitHub pass without its failure and
/// there is no spelling for trying. The flavour is the EVENT's, never the
/// config's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flash {
    /// Any behaviour whose colour is fixed, which is every word but `checks`.
    /// `Behaviour::Checks` spelled here renders nothing, the same fail-dark
    /// answer a held word gets: the two GitHub colours are the arms below.
    Word(crate::lamps::config::Behaviour),
    GithubPass,
    GithubFail,
}

impl Flash {
    /// The ROUTABLE word this pulse is carried by, which is what a `behaviours`
    /// list is matched against.
    pub fn behaviour(self) -> crate::lamps::config::Behaviour {
        match self {
            Flash::Word(word) => word,
            Flash::GithubPass | Flash::GithubFail => crate::lamps::config::Behaviour::Checks,
        }
    }
}
