//! Rendering a table into one output target.
//!
//! One module per target behind one trait, so another target is one new
//! module and one more arm of `for_target`.

pub mod bash;
pub mod menu;

use crate::table::Table;

/// Why a table cannot be rendered for a target. The message names the row.
#[derive(Debug, PartialEq, Eq)]
pub struct RenderFault(pub String);

impl std::fmt::Display for RenderFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub trait Renderer {
    fn render(&self, table: &Table) -> Result<String, RenderFault>;
}

/// The targets this tool renders: a shell's own binding syntax, and the
/// tab-separated records the shell's binding picker reads.
pub fn for_target(target: &str) -> Option<Box<dyn Renderer>> {
    match target {
        "bash" => Some(Box::new(bash::Bash)),
        "menu" => Some(Box::new(menu::Menu)),
        _ => None,
    }
}
