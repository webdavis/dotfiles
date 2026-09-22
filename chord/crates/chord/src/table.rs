//! The binding table: shell-agnostic data, one TOML file, parsed here.

use serde::Deserialize;

/// The whole table. Groups carry the documentation and order the output;
/// `render` carries what each target says about the file it writes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    #[serde(default)]
    pub group: Vec<Group>,
    #[serde(default)]
    pub render: Render,
}

/// What the table says about the files it renders. Every field is optional:
/// a table that declares none of this still renders.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Render {
    /// The command that regenerates this table's outputs, named by a failed
    /// `chord check` so the reader knows how to fix the file.
    pub regenerate: Option<String>,
    #[serde(default)]
    pub bash: BashRender,
    #[serde(default)]
    pub menu: MenuRender,
}

/// The bash target's own settings.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BashRender {
    /// The comment block the rendered file opens with.
    pub header: Option<String>,
    /// The readline macro that clears the line before another macro types
    /// over it, which a typed row needs and the other row kinds do not.
    pub clear_line: Option<String>,
}

/// The menu target's own settings.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuRender {
    /// The comment block the rendered records open with.
    pub header: Option<String>,
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

impl std::fmt::Display for RowFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            RowFault::NoAction => "names no action",
            RowFault::SeveralActions => "names more than one action",
        };
        formatter.write_str(message)
    }
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

    #[test]
    fn a_table_naming_no_render_section_declares_nothing() {
        let table = parse("[[group]]\nname = \"g\"\n");
        assert_eq!(table.render.regenerate, None);
        assert_eq!(table.render.bash.header, None);
        assert_eq!(table.render.bash.clear_line, None);
        assert_eq!(table.render.menu.header, None);
    }

    #[test]
    fn the_render_section_carries_each_targets_own_output_settings() {
        let table = parse(
            "[render]\nregenerate = \"make bindings\"\n\
             [render.bash]\nheader = \"# mine\\n\"\nclear_line = \"\\\\C-x0\"\n\
             [render.menu]\nheader = \"# picker\\n\"\n",
        );
        assert_eq!(table.render.regenerate.as_deref(), Some("make bindings"));
        assert_eq!(table.render.bash.header.as_deref(), Some("# mine\n"));
        assert_eq!(table.render.bash.clear_line.as_deref(), Some("\\C-x0"));
        assert_eq!(table.render.menu.header.as_deref(), Some("# picker\n"));
    }

    #[test]
    fn a_misspelled_render_key_is_refused_rather_than_ignored() {
        let fault = toml::from_str::<Table>("[render.bash]\nheadr = \"# mine\"\n")
            .expect_err("an unknown key must be refused");
        assert!(fault.to_string().contains("headr"), "{fault}");
    }
}
