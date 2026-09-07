use crate::*;

/// Hue's settings, only when the operator enabled it explicitly.
pub(crate) fn enabled_hue_table(config: &pns::config::Config) -> Option<toml::Table> {
    config
        .plugins
        .get("hue")
        .filter(|hue| hue.enabled)
        .map(|hue| hue.settings.clone())
}
/// Whether a card fires while the operator is watching the pane on mobile.
///
/// DEFAULT OFF (operator ruling 2026-08-12): a card about the pane already on
/// screen is noise, and the pulse alone marks the long command finishing.
///
/// A value of the WRONG TYPE is refused out loud, the way the config layer
/// refuses a non-boolean `enabled` by name. Reading `"true"` as false is the
/// same defect one level down: the operator asked for something, did not get
/// it, and was told nothing.
///
/// IT IS HANDED THE ARMED TABLE rather than the config, because every read of
/// `[plugins.mobile]` goes through one accessor: a toggle honoured under a
/// table whose backend was refused would be one setting of a refused table
/// still in force.
fn watch_card(settings: &toml::Table) -> bool {
    let Some(stated) = settings.get("mobile_watch_card") else {
        return false;
    };
    stated.as_bool().unwrap_or_else(|| {
        eprintln!(
            "pns: config error ([plugins.mobile] mobile_watch_card is {}, not a boolean); the mobile watching card stays off",
            stated.type_str()
        );
        false
    })
}
/// What reading `[plugins.mobile]` decided, carried whole rather than
/// collapsed into an absent token.
///
/// THE COMPLAINT TRAVELS WITH THE OUTCOME. A backend nobody answers and a
/// token nobody wrote are two different edits, and folding both into `None`
/// made the doctor name `token` for a fault that was `type`, on a machine
/// whose token was already correct.
#[derive(Default)]
pub(crate) struct Mobile {
    /// The push token, when the table is armed and states one. `None` is the
    /// not-set-up case, which the deliver seam names its own config key for.
    pub(crate) token: Option<String>,
    /// Why no card can be pushed: the table is switched on and names a backend
    /// nothing compiled in answers. The mobile leg fails with these words
    /// wherever it is dispatched.
    pub(crate) refusal: Option<String>,
    /// Whether a card fires while the operator is watching the pane.
    pub(crate) watch_card: bool,
}
/// The one read of `[plugins.mobile]`, and the one place its refusal reaches
/// stderr.
///
/// THE COMPLAINT IS PRINTED HERE because this is the composition root, which is
/// where every other returned warning becomes a line. ONCE, whatever the table
/// is going to be read for, because the table is read once: the token, the
/// toggle and the refusal come out of a single verdict instead of three
/// readers that each had to remember to ask the same question.
pub(crate) fn read_mobile(config: &pns::config::Config) -> Mobile {
    let settings = match pns::config::armed_mobile(config) {
        Ok(settings) => settings,
        Err(reason) => {
            eprintln!("pns: config error ({reason}); no card is pushed");
            return Mobile {
                refusal: Some(reason),
                ..Mobile::default()
            };
        }
    };
    let Some(settings) = settings else {
        return Mobile::default();
    };
    Mobile {
        token: moshi_secret(settings),
        refusal: None,
        watch_card: watch_card(settings),
    }
}
/// One line about a table the event path deliberately never refuses.
///
/// A DISABLED TABLE IS INERT (operator ruling 2026-08-31). Nothing at load and
/// nothing on the event path enforces the `type` under a switched-off table,
/// because a line about a channel the operator turned off, printed on every
/// event, is noise. It is still a misconfiguration waiting for the moment the
/// switch flips, so the DIAGNOSTIC says it, once, where diagnostics live and
/// where the operator is standing there reading.
///
/// ON STDERR, with the config complaints and not with the census: the doctor's
/// stdout is one line per registered plugin plus its summary, and this is
/// about a table rather than about a check. It moves no exit code, which is
/// the same rule the Focus and daemon lines keep: a switch nobody flipped is
/// not a broken notifier.
fn disabled_backend_warning(table: &str, only_type: &str) -> String {
    format!(
        "pns: [plugins.{table}] is switched off and names no backend this binary answers \
         (the only type is {only_type:?}); nothing refuses it until it is enabled"
    )
}
/// Every switched-off table whose `type` names no compiled-in backend, in the
/// order the roster registers them.
pub(crate) fn disabled_backend_warnings(config: &pns::config::Config) -> Vec<String> {
    let switched_off = |name: &str| {
        config
            .plugins
            .get(name)
            .filter(|entry| !entry.enabled)
            .map(|entry| &entry.settings)
    };
    let mut warnings = Vec::new();
    // THE TYPE ALONE on both tables. `router_settings` settles the type before
    // it reads anything else, which is why only its two type refusals count
    // here: a switched-off table naming a backend that DOES answer, with a
    // missing `router_url` under it, is a different edit and not this
    // warning's business.
    if switched_off("router").is_some_and(|settings| {
        matches!(
            pns::home::router_settings(settings),
            Err(pns::home::SetupFailure::NoType | pns::home::SetupFailure::UnknownType(_))
        )
    }) {
        warnings.push(disabled_backend_warning("router", pns::home::UNIFI_TYPE));
    }
    if switched_off("mobile").is_some_and(|settings| mobile_backend(settings).is_err()) {
        warnings.push(disabled_backend_warning("mobile", MOSHI_TYPE));
    }
    warnings
}
/// One plugin's settings table, when the config carries the plugin at all.
pub(crate) fn plugin_settings<'config>(
    config: &'config pns::config::Config,
    name: &str,
) -> Option<&'config toml::Table> {
    config.plugins.get(name).map(|plugin| &plugin.settings)
}
