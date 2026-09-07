use super::*;

/// One answer as a TOML basic string.
///
/// A PASTED SECRET IS UNTRUSTED TEXT and is escaped rather than refused: raw
/// interpolation composes a file that will not load at best, and at worst one
/// whose value stops where the operator's own quote did.
///
/// `{` AND `}` ARE ESCAPED TOO, even though TOML itself has no complaint about
/// either one bare: this text is what an eventual `.tmpl` file regenerates
/// from, and chezmoi's own template engine reads a live `{{ ... }}` action
/// anywhere in that file, quotes or no quotes. Splitting the pair into two
/// `\uXXXX` escapes keeps a pasted value from ever handing chezmoi one.
pub(super) fn quoted(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '{' | '}' => quoted.push_str(&format!("\\u{:04X}", character as u32)),
            // TOML admits no bare control character inside a basic string.
            control if control < ' ' || control == '\u{7f}' => {
                quoted.push_str(&format!("\\u{:04X}", control as u32));
            }
            plain => quoted.push(plain),
        }
    }
    quoted.push('"');
    quoted
}

/// One answer as a TOML array of basic strings.
fn quoted_list(values: &[String]) -> String {
    let quoted: Vec<String> = values.iter().map(|value| quoted(value)).collect();
    format!("[{}]", quoted.join(", "))
}

/// One value, written the way its own TOML type is spelled: a bool and an
/// integer are literals, a string and a string array are escaped, and a table
/// shaped `{ keepassxc = "<entry>", field = "Password" | "UserName" }` is a
/// chezmoi action rather than a TOML value at all. Nothing else renders.
pub(super) fn render_value(value: &toml::Value) -> Result<String, String> {
    match value {
        toml::Value::Boolean(flag) => Ok(flag.to_string()),
        toml::Value::Integer(count) => Ok(count.to_string()),
        toml::Value::String(text) => Ok(quoted(text)),
        toml::Value::Array(items) => {
            let mut strings = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    toml::Value::String(text) => strings.push(text.clone()),
                    other => {
                        return Err(format!(
                            "an array element has type `{}`, not a string",
                            other.type_str()
                        ));
                    }
                }
            }
            Ok(quoted_list(&strings))
        }
        toml::Value::Table(table) => secret_action(table),
        other => Err(format!("type `{}` does not render", other.type_str())),
    }
}
