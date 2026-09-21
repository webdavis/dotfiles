use pns_adapters::{CalendarSource, GoogleConsent, LoadOutcome, config_path, load_config};

/// What `pns calendar` takes, which is the one-time walk and nothing else.
pub(crate) const CALENDAR_USAGE: &str = "\
pns: usage:
  pns calendar consent             mint the refresh token `[quiet.calendar]` reads
  pns calendar consent --client-id <id> --client-secret-stdin
                                   the same walk for a client the config does
                                   not name yet, its secret piped on stdin so it
                                   is never typed where the terminal echoes it,
                                   e.g. keepassxc-cli show -a Password <entry> |
                                   pns calendar consent --client-id <id> \\
                                     --client-secret-stdin
";

/// The flag that names the OAuth client when the config does not.
const CLIENT_ID_FLAG: &str = "--client-id";
/// The flag that says the client secret arrives on standard input.
const CLIENT_SECRET_STDIN_FLAG: &str = "--client-secret-stdin";
/// The one verb.
const CONSENT: &str = "consent";

/// What the operator is told to do with the one thing this prints.
const KEEP_IT_IN_THE_VAULT: &str = "Store that refresh token in KeePassXC now: it is printed once, \
     pns writes it to no file, and `[quiet.calendar] refresh_token` reads it from the vault.";

/// `pns calendar <verb>`: the one-time walk that mints the refresh token the
/// google quiet source reads.
///
/// EXIT 2 FOR A MISTYPED INVOCATION, `pns resume`'s code: this is only ever
/// hand-run, and a flag swallowed silently is a consent the operator believes
/// they granted.
pub(crate) fn calendar_mode(verb: &str) -> i32 {
    if verb != CONSENT {
        eprintln!("{CALENDAR_USAGE}");
        return 2;
    }
    consent_mode(&crate::arguments_after_verb())
}

fn consent_mode(arguments: &[String]) -> i32 {
    let client = match stated_client(arguments).and_then(|stated| match stated {
        Some(id) => read_secret().map(|secret| (id, secret)),
        None => configured_client(),
    }) {
        Ok(client) => client,
        Err(complaint) => {
            eprintln!("pns calendar consent: {complaint}");
            eprint!("{CALENDAR_USAGE}");
            return 2;
        }
    };
    match GoogleConsent::default().mint(&client.0, &client.1, &mut |url| {
        print_lines(&asked(url));
    }) {
        Ok(token) => print_lines(&minted(&token)),
        Err(complaint) => {
            eprintln!("pns calendar consent: {complaint}");
            1
        }
    }
}

/// What the operator does with the URL this walk is waiting on.
fn asked(url: &str) -> Vec<String> {
    vec![
        "pns calendar consent: open this URL, grant the consent, and come back:".to_string(),
        String::new(),
        format!("  {url}"),
        String::new(),
    ]
}

/// The one thing this command produces, and the sentence that says what to do
/// with it.
fn minted(token: &str) -> Vec<String> {
    vec![
        token.to_string(),
        String::new(),
        format!("pns calendar consent: {KEEP_IT_IN_THE_VAULT}"),
    ]
}

fn print_lines(lines: &[String]) -> i32 {
    use std::io::Write;
    if writeln!(std::io::stdout().lock(), "{}", lines.join("\n")).is_err() {
        return 1;
    }
    0
}

/// The OAuth client the flags name, or `None` for the one the config names.
///
/// `--client-secret` and `--client-secret=<value>` ARE BOTH REFUSED BY NAME
/// rather than read: a secret in argv is readable by every process on the
/// machine, and a flag that merely fails to parse would read to the operator
/// as a typo rather than as the reason.
fn stated_client(arguments: &[String]) -> Result<Option<String>, String> {
    let mut id: Option<String> = None;
    let mut stdin_secret = false;
    let mut words = arguments.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            CLIENT_ID_FLAG => {
                id = Some(words.next().cloned().ok_or_else(|| {
                    format!("`{CLIENT_ID_FLAG}` takes the OAuth client id and was given none")
                })?);
            }
            CLIENT_SECRET_STDIN_FLAG => stdin_secret = true,
            word if word == "--client-secret" || word.starts_with("--client-secret=") => {
                return Err(format!(
                    "`--client-secret` would put the secret in argv, where every process on this \
                     machine can read it; pass it on standard input with \
                     `{CLIENT_SECRET_STDIN_FLAG}`"
                ));
            }
            other => return Err(format!("`{other}` is not a flag this walk takes")),
        }
    }
    match (id, stdin_secret) {
        (Some(id), true) => Ok(Some(id)),
        (None, false) => Ok(None),
        _ => Err(format!(
            "`{CLIENT_ID_FLAG}` and `{CLIENT_SECRET_STDIN_FLAG}` are given together, or neither is \
             and the client is read from `[quiet.calendar]`"
        )),
    }
}

/// The client secret, read whole off standard input.
fn read_secret() -> Result<String, String> {
    use std::io::Read;
    let mut held = String::new();
    std::io::stdin()
        .read_to_string(&mut held)
        .map_err(|_| "the client secret could not be read from standard input".to_string())?;
    let secret = held.trim().to_string();
    if secret.is_empty() {
        return Err("standard input carried no client secret".to_string());
    }
    Ok(secret)
}

/// The client the config already names, which is the walk a re-mint runs.
fn configured_client() -> Result<(String, String), String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let source = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => config.quiet_calendar.source,
        Ok(LoadOutcome::Missing) => CalendarSource::default(),
        Err(error) => {
            return Err(format!("the config could not be read ({})", error.detail()));
        }
    };
    match source {
        CalendarSource::Google(google) => Ok((google.client_id, google.client_secret)),
        CalendarSource::Command(_) => Err(format!(
            "`[quiet.calendar]` does not state `type = \"google\"`, so name the client with \
             `{CLIENT_ID_FLAG}` and `{CLIENT_SECRET_STDIN_FLAG}`"
        )),
    }
}

#[cfg(test)]
#[path = "command_calendar/tests.rs"]
mod tests;
