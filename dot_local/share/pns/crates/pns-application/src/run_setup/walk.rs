use crate::Terminal;
use pns_domain::{
    Answers, setup_affirmed as means_yes, setup_list as list,
    setup_router_backend as router_backend,
};

/// The walk itself: one question at a time, in the order the file is written.
///
/// NONE OF THIS DECIDES ANYTHING. Every answer is carried to the composer as
/// it was typed, and a blank one is what declines a feature there. An `Err`
/// is the walk ending mid-conversation, named by its own reason, which
/// publishes nothing at all rather than composing a file out of half of one.
///
/// THE CREDENTIALS ARE ASKED INSIDE THE WALK, right after the feature they
/// arm, because a feature switched on now and credentialed later is exactly
/// the empty-value config this wizard exists to avoid.
pub(super) fn walk(terminal: &impl Terminal) -> Result<Answers, String> {
    terminal.say(SETUP_PREAMBLE);
    terminal.say(
        "A chezmoi-managed config will be replaced on the next apply; update its source instead.
Config diffs can expose the secrets entered here.",
    );
    let mut answers = Answers {
        mobile_token: terminal.ask_hidden(
            "The phone card is on. Paste moshi's webhook secret to complete it, \
             or press enter to pair later",
        )?,
        ..Default::default()
    };

    if ask_yes(
        terminal,
        "Post every event to hermes, for the durable log and the recap?",
    )? {
        answers.hermes_key =
            armed_secret(terminal, "hermes", "the signing key that route verifies")?;
    }
    if ask_yes(
        terminal,
        "Flash hue lights green when work finishes and red when it dies?",
    )? {
        // EACH ANSWER GATES THE NEXT QUESTION: once one comes back empty the
        // feature is already declined, and the rest would be questions whose
        // answers are thrown away.
        answers.hue_bridge = armed(
            terminal,
            "the light pulse",
            "the hue bridge's address on the network",
        )?;
        if !answers.hue_bridge.is_empty() {
            answers.hue_key =
                armed_secret(terminal, "the light pulse", "an API key the bridge issued")?;
        }
        if !answers.hue_key.is_empty() {
            answers.hue_rooms = list(armed(
                terminal,
                "the light pulse",
                "the rooms to flash, comma separated, spelled as the bridge spells them",
            )?);
        }
    }
    if ask_yes(
        terminal,
        "Read whether your phone is on the home wifi, off the router's client list?",
    )? {
        // THE BACKEND HAS A WORKING DEFAULT and every other field here does
        // not, so this is the one question enter answers rather than declines.
        // A NAME NOTHING ANSWERS DECLINES THE PROBE, said here and not only in
        // the file: the composer writes that answer's table commented out, and
        // an operator who typed their router's brand deserves to hear why.
        match router_backend(&terminal.ask(&format!(
            "Which router backend? [{}]",
            pns_domain::home::UNIFI_TYPE
        ))?) {
            None => terminal.say(
                "  nothing here reads that router, so the home probe stays off; \
                 the file says how to arm it",
            ),
            Some(backend) => {
                answers.router_type = backend.to_string();
                answers.router_url = armed(terminal, "the home probe", "the router's URL")?;
                if !answers.router_url.is_empty() {
                    answers.router_api_key =
                        armed_secret(terminal, "the home probe", "an API key the router issued")?;
                }
                if !answers.router_api_key.is_empty() {
                    answers.router_device_hostname = armed(
                        terminal,
                        "the home probe",
                        "the phone's hostname on that router",
                    )?;
                }
            }
        }
    }
    if ask_yes(
        terminal,
        "Hold notifications back while a macOS Focus is on?",
    )? {
        answers.focus_modes = list(armed(
            terminal,
            "focus silencing",
            "which Focus modes mean it, comma separated",
        )?);
    }
    answers.nag = ask_yes(
        terminal,
        "Card you a second time about an approval left unanswered?",
    )?;
    Ok(answers)
}

/// One credentialed answer, and the line that says what a blank one costs.
///
/// SAID WHEN IT HAPPENS rather than only in the file: an operator who meant to
/// arm a feature and pressed enter has one chance to notice, and the composed
/// file's own commented block is read later if at all.
fn armed(terminal: &impl Terminal, feature: &str, wanted: &str) -> Result<String, String> {
    Ok(nothing_given(terminal, feature, terminal.ask(wanted)?))
}

/// The same shape as `armed`, for a secret: read with the terminal's echo
/// held off, because this is where the token, the hermes key, the hue key
/// and the router key are all asked.
fn armed_secret(terminal: &impl Terminal, feature: &str, wanted: &str) -> Result<String, String> {
    Ok(nothing_given(
        terminal,
        feature,
        terminal.ask_hidden(wanted)?,
    ))
}

/// What `armed` and `armed_secret` share: the line a blank answer costs.
fn nothing_given(terminal: &impl Terminal, feature: &str, answer: String) -> String {
    if answer.is_empty() {
        terminal.say(&format!(
            "  nothing given, so {feature} stays off; the file says how to arm it"
        ));
    }
    answer
}

/// One yes-or-no question. ENTER MEANS NO, and so does anything that is not a
/// yes: this walk arms features that deliver to a phone and to lamps, and the
/// answer nobody typed on purpose must be the one that changes nothing.
fn ask_yes(terminal: &impl Terminal, question: &str) -> Result<bool, String> {
    Ok(means_yes(&terminal.ask(&format!("{question} [y/N]"))?))
}

/// What the walk says before it starts asking.
const SETUP_PREAMBLE: &str = "\
pns setup: a few questions, and a config at the end of them.
The macOS banner and the phone card are on and are not asked about. Everything
else is off unless you arm it here, and enter is no. Nothing is written until
the last answer.";
