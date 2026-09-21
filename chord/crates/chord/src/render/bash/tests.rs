use super::*;

const FIVE_ROWS: &str = r#"
[[group]]
name = "sample"
description = "Every row kind, once."

[[group.binding]]
key = "ctrl-g a a"
description = "Stage a path."
insert = "git add "

[[group.binding]]
key = "alt-l"
run = "eza -ahlrs date"

[[group.binding]]
key = "tab"
modes = []
function = "menu-complete"

[[group.binding]]
key = "ctrl-a"
modes = ["vi-insert"]
command = "beginning-of-line"

[[group.binding]]
key = "ctrl-g o f"
modes = ["vi-insert"]
macro = "\\C-x0\\C-_I\\C-x1\\C-x2\\b"
"#;

const GOLDEN: &str = r#"
# --- sample ---
# Every row kind, once.

# Stage a path.
builtin bind -m vi-insert '"\C-gaa": "\C-x0git add "'
builtin bind -m vi-command '"\C-gaa": "i\C-x0git add "'

builtin bind -m vi-insert '"\M-l": "\C-x0eza -ahlrs date\r"'
builtin bind -m vi-command '"\M-l": "i\C-x0eza -ahlrs date\r"'

builtin bind -x '"\t": "menu-complete"'

builtin bind -m vi-insert '"\C-a": beginning-of-line'

builtin bind -m vi-insert '"\C-gof": "\C-x0\C-_I\C-x1\C-x2\b"'
"#;

fn render(table_text: &str) -> Result<String, RenderFault> {
    let table: Table = toml::from_str(table_text).expect("the table must parse");
    Bash.render(&table)
}

#[test]
fn five_rows_render_to_their_golden_text() {
    let rendered = render(FIVE_ROWS).expect("the sample table must render");
    let body = rendered
        .strip_prefix(HEADER)
        .unwrap_or_else(|| panic!("the rendering must open with the header:\n{rendered}"));
    assert_eq!(body, GOLDEN, "\n{body}");
}

#[test]
fn the_rendering_ends_in_a_newline() {
    let rendered = render(FIVE_ROWS).expect("the sample table must render");
    assert!(rendered.ends_with('\n'));
}

#[test]
fn a_quote_in_typed_text_is_spliced_back_into_the_argument() {
    let rendered = render(
        "[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"ctrl-g b a\"\nmodes = [\"vi-insert\"]\n\
         run = \"echo 'Local:'\"\n",
    )
    .expect("a quoted argument must render");
    assert!(
        rendered.contains(r#"builtin bind -m vi-insert '"\C-gba": "\C-x0echo '\''Local:'\''\r"'"#),
        "{rendered}"
    );
}

#[test]
fn a_double_quote_and_a_backslash_are_escaped_for_readline() {
    let rendered = render(
        "[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"a\"\nmodes = [\"vi-insert\"]\n\
         insert = \"echo \\\"x\\\" \\\\y\"\n",
    )
    .expect("escaped text must render");
    assert!(rendered.contains(r#""\C-x0echo \"x\" \\y""#), "{rendered}");
}

#[test]
fn a_carriage_return_inside_typed_text_is_refused() {
    let fault =
        render("[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"a\"\nrun = \"ls\\r\"\n")
            .expect_err("a carriage return must be refused");
    assert!(fault.0.contains("carriage return"), "{}", fault.0);
}

#[test]
fn an_unknown_key_token_names_the_row_it_came_from() {
    let fault =
        render("[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"hyper-a\"\nrun = \"ls\"\n")
            .expect_err("an unknown token must be refused");
    assert!(fault.0.contains("hyper-a"), "{}", fault.0);
}

#[test]
fn a_macro_body_reaches_the_output_verbatim() {
    let rendered = render(
        "[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"ctrl-g o f\"\n\
         modes = [\"vi-command\"]\nmacro = \"i\\\\C-x0\\\\C-_I\\\\b\"\n",
    )
    .expect("a raw macro must render");
    assert!(
        rendered.contains(r#"builtin bind -m vi-command '"\C-gof": "i\C-x0\C-_I\b"'"#),
        "{rendered}"
    );
}
