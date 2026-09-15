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

/// One float, always with a fractional part.
///
/// `1.0` MUST NOT BE WRITTEN `1`. TOML reads a bare `1` as an INTEGER, and the
/// only floats this schema serves are colour coordinates, whose parser takes
/// numbers only: the shorter spelling would write a file the parser refuses.
///
/// A NON-FINITE FLOAT IS REFUSED rather than written. TOML spells `nan` and
/// `inf`; neither is a coordinate, and both would round trip as the same
/// non-number into a lamp nobody can arm.
fn decimal(number: f64) -> Result<String, String> {
    if !number.is_finite() {
        return Err(format!("`{number}` is not a finite number"));
    }
    let written = number.to_string();
    Ok(if written.contains(['.', 'e', 'E']) {
        written
    } else {
        format!("{written}.0")
    })
}

/// One answer as a TOML array of floats.
fn decimal_list(values: &[f64]) -> Result<String, String> {
    let mut written = Vec::with_capacity(values.len());
    for value in values {
        written.push(decimal(*value)?);
    }
    Ok(format!("[{}]", written.join(", ")))
}

/// One array, written as whatever its own elements are: strings escaped, or
/// numbers as numbers. THE FIRST ELEMENT DECIDES WHICH, and a mixed array is
/// refused by the type of the element that disagrees with it.
fn render_array(items: &[toml::Value]) -> Result<String, String> {
    if items.first().is_some_and(toml::Value::is_float) {
        let mut numbers = Vec::with_capacity(items.len());
        for item in items {
            let Some(number) = item.as_float() else {
                return Err(format!(
                    "an array element has type `{}`, not a number",
                    item.type_str()
                ));
            };
            numbers.push(number);
        }
        return decimal_list(&numbers);
    }
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

/// One value, written the way its own TOML type is spelled: a bool, an integer
/// and a float are literals, a string and a string array are escaped, an array
/// of numbers is written as numbers, and a table shaped
/// `{ keepassxc = "<entry>", field = "Password" | "UserName" }` is a chezmoi
/// action rather than a TOML value at all. Nothing else renders.
pub(super) fn render_value(value: &toml::Value) -> Result<String, String> {
    match value {
        toml::Value::Boolean(flag) => Ok(flag.to_string()),
        toml::Value::Integer(count) => Ok(count.to_string()),
        toml::Value::Float(number) => decimal(*number),
        toml::Value::String(text) => Ok(quoted(text)),
        toml::Value::Array(items) => render_array(items),
        toml::Value::Table(table) => secret_action(table),
        other => Err(format!("type `{}` does not render", other.type_str())),
    }
}
