use pns_domain::failure::{ClickView, parse_view};

/// The click view out of the `[plugins.macos-banner]` settings.
///
/// TWO FLAT KEYS RATHER THAN A NESTED TABLE. The design writes them as
/// `[banner.click] type` and `command`; the banner already has a table in this
/// file, and a second top-level heading for one plugin's two settings would be
/// the only place in the config where a plugin's options live outside its own
/// table.
///
/// A REFUSAL IS RETURNED, NOT SWALLOWED. A typo that quietly opened something
/// else is a click the operator believes is configured, and the one moment they
/// find out otherwise is the moment they most need the record.
pub fn banner_click(settings: &toml::Table, herdr_present: bool) -> Result<ClickView, String> {
    let read = |key: &str| {
        settings
            .get(key)
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
    };
    parse_view(read("click_type"), read("click_command"), herdr_present)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(toml: &str) -> toml::Table {
        toml.parse().unwrap()
    }

    #[test]
    fn an_empty_table_falls_to_the_inference() {
        assert_eq!(banner_click(&table(""), true).unwrap(), ClickView::Herdr);
        assert_eq!(banner_click(&table(""), false).unwrap(), ClickView::Window);
    }

    #[test]
    fn a_configured_type_and_command_are_read_from_their_two_keys() {
        let settings = table("click_type = \"command\"\nclick_command = \"/bin/echo {id}\"\n");
        assert_eq!(
            banner_click(&settings, true).unwrap(),
            ClickView::Command("/bin/echo {id}".into())
        );
    }

    /// A key of the wrong TYPE reads as absent rather than as a value, which is
    /// what keeps `click_type = 3` from being quoted back as a type name the
    /// operator never typed.
    #[test]
    fn a_key_that_is_not_a_string_reads_as_unwritten() {
        assert_eq!(
            banner_click(&table("click_type = 3\n"), false).unwrap(),
            ClickView::Window
        );
    }

    #[test]
    fn an_unknown_type_is_refused_rather_than_falling_back() {
        assert!(banner_click(&table("click_type = \"herd\"\n"), true).is_err());
    }
}
