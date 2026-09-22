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

/// The comment block a rendered file opens with: the table's own header, or
/// the target's neutral banner when the table names none. A header that does
/// not end its last line gains the newline the body needs after it.
pub fn header(configured: Option<&str>, neutral: &str) -> String {
    let text = configured.unwrap_or(neutral);
    if text.is_empty() || text.ends_with('\n') {
        return text.to_string();
    }
    format!("{text}\n")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_own_header_replaces_the_neutral_one() {
        assert_eq!(header(Some("# mine\n"), "# neutral\n"), "# mine\n");
    }

    #[test]
    fn a_table_naming_no_header_gets_the_neutral_one() {
        assert_eq!(header(None, "# neutral\n"), "# neutral\n");
    }

    #[test]
    fn a_header_missing_its_final_newline_gains_one() {
        assert_eq!(header(Some("# mine"), "# neutral\n"), "# mine\n");
    }

    #[test]
    fn an_empty_header_opens_the_file_with_nothing() {
        assert_eq!(header(Some(""), "# neutral\n"), "");
    }
}
