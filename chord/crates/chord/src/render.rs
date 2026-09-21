//! Rendering a table into one shell's native binding syntax.
//!
//! One module per shell behind one trait, so a second shell is one new
//! module and one more arm of `for_shell`.

pub mod bash;

use crate::table::Table;

/// Why a table cannot be rendered for a shell. The message names the row.
#[derive(Debug, PartialEq, Eq)]
pub struct RenderFault(pub String);

impl std::fmt::Display for RenderFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub trait ShellRenderer {
    fn render(&self, table: &Table) -> Result<String, RenderFault>;
}

/// The shells this tool speaks.
pub fn for_shell(shell: &str) -> Option<Box<dyn ShellRenderer>> {
    match shell {
        "bash" => Some(Box::new(bash::Bash)),
        _ => None,
    }
}
