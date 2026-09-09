//! Which way the operator asked to go, and what that means where they are
//! standing.
//!
//! THE WHOLE OF THE POLICY IS HERE, and it is three lines of it: a Neovim in
//! the pane gets the chord and moves a split, anything else moves a pane, and
//! no pane at all moves whichever pane herdr thinks is current. Everything
//! around this crate is the plumbing that finds those three facts out.

/// One of the four ways a `ctrl+<hjkl>` press can go.
///
/// AN ENUM, NOT THE WORD THE OPERATOR TYPED. The word and the chord it maps to
/// used to travel side by side as two strings, which is a pair that can
/// disagree: a call passing `"left"` with `ctrl+l` compiled and sent Neovim the
/// wrong way. One value carries both, so there is nothing to keep in step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Down,
    Up,
    Right,
}

impl Direction {
    /// The direction a keybinding's argv names, or `None` for a word this
    /// plugin does not serve.
    ///
    /// The four words are herdr's own spelling for `pane focus --direction`,
    /// which is what makes [`Direction::as_str`] a lookup rather than a
    /// translation.
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "left" => Some(Direction::Left),
            "down" => Some(Direction::Down),
            "up" => Some(Direction::Up),
            "right" => Some(Direction::Right),
            _ => None,
        }
    }

    /// The key Neovim is sent to move its own split this way.
    ///
    /// This is the mapping the operator already presses, which is the point of
    /// the plugin: one press moves a split or a pane, and they never have to
    /// know which of the two they are inside.
    pub fn chord(self) -> &'static str {
        match self {
            Direction::Left => "ctrl+h",
            Direction::Down => "ctrl+j",
            Direction::Up => "ctrl+k",
            Direction::Right => "ctrl+l",
        }
    }

    /// The word herdr's own `--direction` takes.
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Left => "left",
            Direction::Down => "down",
            Direction::Up => "up",
            Direction::Right => "right",
        }
    }
}

/// What the press does, decided once and carried out by the adapter.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Hand the chord to the Neovim running in this pane and let it move its
    /// own split.
    SendKeys { pane: String, direction: Direction },
    /// Move herdr's focus off a NAMED pane, which is the pane the press came
    /// from rather than whichever one herdr believes is current.
    Focus { pane: String, direction: Direction },
    /// Move herdr's focus off the current pane, for a press that arrived with
    /// no pane id to anchor on.
    FocusCurrent { direction: Direction },
}

/// The one decision: who moves, the editor or the multiplexer.
pub fn decide(pane: Option<&str>, is_nvim: bool, direction: Direction) -> Action {
    match pane {
        Some(pane) if is_nvim => Action::SendKeys {
            pane: pane.to_string(),
            direction,
        },
        Some(pane) => Action::Focus {
            pane: pane.to_string(),
            direction,
        },
        None => Action::FocusCurrent { direction },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four words a keybinding can carry, each mapped to the chord the
    /// operator already presses and back to herdr's own spelling.
    #[test]
    fn every_direction_carries_its_own_chord_and_its_own_word() {
        for (word, direction, chord) in [
            ("left", Direction::Left, "ctrl+h"),
            ("down", Direction::Down, "ctrl+j"),
            ("up", Direction::Up, "ctrl+k"),
            ("right", Direction::Right, "ctrl+l"),
        ] {
            assert_eq!(Direction::parse(word), Some(direction));
            assert_eq!(direction.chord(), chord);
            assert_eq!(direction.as_str(), word, "herdr's own spelling");
        }
    }

    /// A word this does not serve is refused rather than guessed at: the CLI
    /// turns that into usage and a non-zero exit, and nothing moves.
    #[test]
    fn a_word_this_does_not_serve_is_refused() {
        assert_eq!(Direction::parse("sideways"), None);
        assert_eq!(Direction::parse(""), None);
        assert_eq!(Direction::parse("Left"), None);
    }

    /// A Neovim in the pane moves its own split, which is the whole reason the
    /// press is intercepted rather than bound in herdr alone.
    #[test]
    fn a_pane_running_neovim_is_sent_the_chord() {
        assert_eq!(
            decide(Some("wW:p8"), true, Direction::Left),
            Action::SendKeys {
                pane: "wW:p8".into(),
                direction: Direction::Left,
            }
        );
    }

    /// Anything else moves the pane, off the pane the press came from rather
    /// than off whichever one herdr believes is current.
    #[test]
    fn a_pane_running_anything_else_moves_the_pane() {
        assert_eq!(
            decide(Some("wW:p8"), false, Direction::Left),
            Action::Focus {
                pane: "wW:p8".into(),
                direction: Direction::Left,
            }
        );
    }

    /// A press with no pane id still moves something: herdr's own idea of the
    /// current pane is the last anchor there is.
    #[test]
    fn a_press_with_no_pane_moves_the_current_one() {
        assert_eq!(
            decide(None, false, Direction::Left),
            Action::FocusCurrent {
                direction: Direction::Left,
            }
        );
    }
}
