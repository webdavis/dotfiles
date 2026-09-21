//! The binding table: shell-agnostic data, one TOML file, parsed here.

use serde::Deserialize;

/// The whole table. Groups carry the documentation and order the output.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    #[serde(default)]
    pub group: Vec<Group>,
}

/// One documented section of the table.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub binding: Vec<Binding>,
}

/// One row. Exactly one action field is set.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    /// The chord in key notation, for example `ctrl-g a a`.
    pub key: String,
    pub description: Option<String>,
    /// Editing modes the row binds in. Absent means both vi modes; an empty
    /// list means the row binds in whatever keymap is current, with no mode
    /// flag at all.
    pub modes: Option<Vec<String>>,
    pub insert: Option<String>,
    pub run: Option<String>,
    pub function: Option<String>,
    pub command: Option<String>,
    #[serde(rename = "macro")]
    pub macro_body: Option<String>,
}

/// What a row does when its chord is pressed.
#[derive(Debug, PartialEq, Eq)]
pub enum Action<'a> {
    /// Type the text and leave it on the line.
    Insert(&'a str),
    /// Type the text and run it.
    Run(&'a str),
    /// Call a shell function (readline's `bind -x`).
    Function(&'a str),
    /// Run a readline command such as `beginning-of-line`.
    Command(&'a str),
    /// A readline macro body, written in readline's own escapes, for the
    /// chords the other kinds cannot express.
    Macro(&'a str),
}

/// Why a row cannot be rendered.
#[derive(Debug, PartialEq, Eq)]
pub enum RowFault {
    NoAction,
    SeveralActions,
}

impl Binding {
    pub fn action(&self) -> Result<Action<'_>, RowFault> {
        let candidates = [
            self.insert.as_deref().map(Action::Insert),
            self.run.as_deref().map(Action::Run),
            self.function.as_deref().map(Action::Function),
            self.command.as_deref().map(Action::Command),
            self.macro_body.as_deref().map(Action::Macro),
        ];
        let mut found = candidates.into_iter().flatten();
        match (found.next(), found.next()) {
            (Some(action), None) => Ok(action),
            (None, _) => Err(RowFault::NoAction),
            (Some(_), Some(_)) => Err(RowFault::SeveralActions),
        }
    }

    /// The modes this row binds in. `None` in the file means both vi modes.
    pub fn modes(&self) -> &[String] {
        match &self.modes {
            Some(modes) => modes,
            None => DEFAULT_MODES.as_slice(),
        }
    }
}

/// Nearly every chord is a pair: one binding in each vi keymap.
static DEFAULT_MODES: std::sync::LazyLock<[String; 2]> =
    std::sync::LazyLock::new(|| ["vi-insert".to_string(), "vi-command".to_string()]);

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Table {
        toml::from_str(text).expect("the table must parse")
    }

    #[test]
    fn a_row_without_modes_binds_in_both_vi_keymaps() {
        let table =
            parse("[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"alt-l\"\nrun = \"ls\"\n");
        assert_eq!(
            table.group[0].binding[0].modes(),
            ["vi-insert", "vi-command"]
        );
    }

    #[test]
    fn an_empty_mode_list_stays_empty() {
        let table = parse(
            "[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"tab\"\nfunction = \"f\"\nmodes = []\n",
        );
        assert!(table.group[0].binding[0].modes().is_empty());
    }

    #[test]
    fn a_row_names_exactly_one_action() {
        let table = parse(
            "[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"a\"\nrun = \"ls\"\ninsert = \"ls\"\n\
             [[group.binding]]\nkey = \"b\"\n",
        );
        assert_eq!(
            table.group[0].binding[0].action(),
            Err(RowFault::SeveralActions)
        );
        assert_eq!(table.group[0].binding[1].action(), Err(RowFault::NoAction));
    }
}
