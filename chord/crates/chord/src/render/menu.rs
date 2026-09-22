//! The menu target: the same table as one tab-separated record per row, for
//! the shell's binding picker to read.
//!
//! Every row is emitted whatever modes it binds in, which is what lets the
//! picker show the whole surface rather than one keymap's worth of it.

use super::{RenderFault, Renderer};
use crate::table::{Action, Binding, Group, Table};

/// What the file says about itself when the table names no header of its
/// own. Skipped by the readers, which ignore a line beginning with `#`.
const NEUTRAL_HEADER: &str = "\
# GENERATED FILE: `chord render menu` over a chord binding table.
# Edit the table and render again; an edit here is lost on the next render.
";

/// The record shape is chord's own contract, so every rendering documents
/// it whatever header the table supplies.
const FIELD_ORDER: &str = "# key<TAB>group<TAB>kind<TAB>action<TAB>description\n";

pub struct Menu;

impl Renderer for Menu {
    fn render(&self, table: &Table) -> Result<String, RenderFault> {
        let mut out = super::header(table.render.menu.header.as_deref(), NEUTRAL_HEADER);
        out.push_str(FIELD_ORDER);
        for group in &table.group {
            for binding in &group.binding {
                out.push_str(&record(group, binding)?);
                out.push('\n');
            }
        }
        Ok(out)
    }
}

fn record(group: &Group, binding: &Binding) -> Result<String, RenderFault> {
    let action = binding
        .action()
        .map_err(|fault_kind| RenderFault(format!("binding {:?}: {fault_kind}", binding.key)))?;
    let (kind, body) = describe(&action);
    Ok([
        one_line(&binding.key),
        one_line(&group.name),
        kind.to_string(),
        one_line(body),
        one_line(binding.description.as_deref().unwrap_or_default()),
    ]
    .join("\t"))
}

/// The kind is what tells the picker whether the action is a command line it
/// can run, a shell function it can call, or a readline command it cannot.
fn describe<'a>(action: &Action<'a>) -> (&'static str, &'a str) {
    match action {
        Action::Insert(text) => ("insert", text),
        Action::Run(text) => ("run", text),
        Action::Function(name) => ("function", name),
        Action::Command(command) => ("command", command),
        Action::Macro(body) => ("macro", body),
    }
}

/// A record is one line of tab-separated fields, so a field carries neither.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(table_text: &str) -> Result<String, RenderFault> {
        let table: Table = toml::from_str(table_text).expect("the table must parse");
        Menu.render(&table)
    }

    fn records(rendered: &str) -> Vec<&str> {
        rendered
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect()
    }

    #[test]
    fn a_row_becomes_its_five_fields() {
        let rendered = render(
            "[[group]]\nname = \"git\"\n[[group.binding]]\nkey = \"ctrl-g a a\"\n\
             description = \"Stage a path.\"\ninsert = \"git add \"\n",
        )
        .expect("the table must render");
        assert_eq!(
            records(&rendered),
            ["ctrl-g a a\tgit\tinsert\tgit add\tStage a path."]
        );
    }

    #[test]
    fn every_mode_reaches_the_menu_including_the_emacs_only_and_mode_less_rows() {
        let rendered = render(
            "[[group]]\nname = \"g\"\n\
             [[group.binding]]\nkey = \"ctrl-a\"\nmodes = [\"emacs\"]\ncommand = \"beginning-of-line\"\n\
             [[group.binding]]\nkey = \"tab\"\nmodes = []\nfunction = \"menu-complete\"\n\
             [[group.binding]]\nkey = \"alt-l\"\nrun = \"eza\"\n",
        )
        .expect("the table must render");
        assert_eq!(
            records(&rendered),
            [
                "ctrl-a\tg\tcommand\tbeginning-of-line\t",
                "tab\tg\tfunction\tmenu-complete\t",
                "alt-l\tg\trun\teza\t",
            ]
        );
    }

    #[test]
    fn each_action_kind_is_named() {
        let rendered = render(
            "[[group]]\nname = \"g\"\n\
             [[group.binding]]\nkey = \"a\"\ninsert = \"i\"\n\
             [[group.binding]]\nkey = \"b\"\nrun = \"r\"\n\
             [[group.binding]]\nkey = \"c\"\nfunction = \"f\"\n\
             [[group.binding]]\nkey = \"d\"\ncommand = \"c\"\n\
             [[group.binding]]\nkey = \"e\"\nmacro = \"m\"\n",
        )
        .expect("the table must render");
        let kinds: Vec<&str> = records(&rendered)
            .iter()
            .map(|record| record.split('\t').nth(2).expect("a kind field"))
            .collect();
        assert_eq!(kinds, ["insert", "run", "function", "command", "macro"]);
    }

    #[test]
    fn a_multi_line_description_collapses_to_one_record() {
        let rendered = render(
            "[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"a\"\n\
             description = \"First line.\\n\\nSecond line.\"\nrun = \"ls\"\n",
        )
        .expect("the table must render");
        assert_eq!(
            records(&rendered),
            ["a\tg\trun\tls\tFirst line. Second line."]
        );
    }

    #[test]
    fn a_tab_inside_a_field_cannot_split_the_record() {
        let rendered =
            render("[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"a\"\nrun = \"ls\\t-l\"\n")
                .expect("the table must render");
        assert_eq!(records(&rendered)[0].split('\t').count(), 5);
    }

    #[test]
    fn a_row_naming_no_action_is_refused_by_key() {
        let fault = render("[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"a\"\n")
            .expect_err("a row without an action must be refused");
        assert_eq!(
            fault,
            RenderFault("binding \"a\": names no action".to_string())
        );
    }

    #[test]
    fn a_table_naming_no_header_names_the_generator_and_the_field_order() {
        let rendered = render("[[group]]\nname = \"g\"\n").expect("an empty group must render");
        assert!(rendered.starts_with("# GENERATED FILE:"), "{rendered}");
        assert!(rendered.contains("key<TAB>group<TAB>kind<TAB>action<TAB>description"));
        assert!(records(&rendered).is_empty());
    }

    #[test]
    fn the_tables_own_header_replaces_the_banner_and_keeps_the_field_order() {
        let rendered = render("[render.menu]\nheader = \"# picker records\\n#\\n\"\n")
            .expect("a configured header must render");
        assert_eq!(
            rendered,
            "# picker records\n#\n# key<TAB>group<TAB>kind<TAB>action<TAB>description\n"
        );
    }
}
