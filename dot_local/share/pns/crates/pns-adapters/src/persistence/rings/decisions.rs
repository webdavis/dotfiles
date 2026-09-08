use pns_domain::{ABSENT, Record, count, printable, tri, verdicts, yes_no};
mod revise;
pub(crate) use revise::revise_leg;
/// One decision as one line: `<epoch> <key=value ...>`.
///
/// NO FREE TEXT REACHES IT. The detail, the branch, the project and the pane
/// id are the operator's own content, and this file is printed to a terminal
/// by `pns doctor`, so recording them would put that content into a state file
/// and then onto a screen. The pane appears as the two booleans the decision
/// actually used it for. Every other value here is a number, a boolean, an
/// enum name or a plugin name out of the compiled roster.
///
/// NOT JSON, though the crate already carries a JSON writer. The only reader
/// is the section below, whose whole parse is one `split_once(' ')` over the
/// epoch; a JSON round trip would add a schema and an error taxonomy to render
/// a sentence that reads as it stands.
///
/// NO actionId IS RECORDED, because pns never has one: the notification seam
/// answers a bool and drops the body, and on the approval path moshi mints the
/// id inside itself and answers with an exit code.
pub fn line(record: &Record) -> String {
    let inputs = record.decision.inputs;
    let overrides = record.overrides;
    format!(
        "{epoch} {agent}/{state} \
         mode={mode} agent={payload_agent} tool={tool} \
         surface={surface:?} visibility={visibility:?} \
         session_visibility={session_visibility:?} \
         desk_age={desk_age} phone_age={phone_age} tap_age={tap_age} \
         locked={locked} fresh_window={fresh_window} long_running={long_running} \
         nag={nag} \
         local_only={local_only} remote_only={remote_only} \
         pane={pane} pane_dropped={pane_dropped} watch_card={watch_card} \
         muted={muted} focus={focus} skip_phone={skip_phone} force_phone={force_phone} \
         idle_invalid={idle_invalid} desk_invalid={desk_invalid} \
         phone_invalid={phone_invalid} \
         plan=banner:{banner},card:{card},pulse:{pulse} legs={legs}",
        epoch = inputs
            .now_secs
            .map_or_else(|| NO_CLOCK.to_string(), |now| now.to_string()),
        agent = printable(&record.event.agent),
        state = printable(&record.event.state),
        // THE PAYLOAD'S OWN IDENTITY, PRINTABLE-FILTERED LIKE `agent`/`state`
        // ABOVE: `tool_name` is remote text a connected MCP server named, so
        // it gets the same allowlist rather than a free pass into a file
        // `pns doctor` prints straight to a terminal.
        mode = printable(record.permission_mode),
        payload_agent = printable(record.agent_id),
        tool = printable(record.tool_name),
        // A FIELDLESS DERIVED `Debug` IS THE VARIANT NAME, which is exactly
        // what an enum reads as here.
        surface = inputs.surface,
        visibility = inputs.visibility,
        session_visibility = inputs.session_visibility,
        desk_age = count(inputs.desk_input_age),
        phone_age = count(inputs.phone_input_age),
        tap_age = count(inputs.marker_age),
        locked = tri(inputs.screen_locked),
        fresh_window = count(inputs.desk_fresh_secs),
        long_running = yes_no(inputs.long_running),
        nag = yes_no(record.nag),
        local_only = yes_no(inputs.local_only),
        remote_only = yes_no(inputs.remote_only),
        // THE PANE AS THE DECISION USED IT and no further: its value is a
        // multiplexer id this crate does not own, and these two booleans are
        // everything the decision read out of it.
        pane = if inputs.pane_present {
            "present"
        } else {
            ABSENT
        },
        pane_dropped = yes_no(record.decision.pane_dropped),
        watch_card = yes_no(inputs.mobile_watch_card),
        muted = yes_no(overrides.muted),
        // TWO FIELDS RATHER THAN ONE. The log exists to answer "why did no
        // card fire", and "you have a `pns quiet` running" sends the operator
        // somewhere completely different from "your Mac is in a Focus you told
        // pns to respect".
        focus = yes_no(overrides.focus_active),
        skip_phone = yes_no(overrides.skip_phone),
        force_phone = yes_no(overrides.force_phone),
        idle_invalid = yes_no(overrides.idle_invalid),
        desk_invalid = yes_no(overrides.desk_invalid),
        phone_invalid = yes_no(overrides.phone_invalid),
        banner = yes_no(record.decision.plan.banner),
        card = yes_no(record.decision.plan.phone_card),
        pulse = yes_no(record.decision.plan.pulse),
        legs = verdicts(record.legs),
    )
}

/// A clock nobody could read, in the field an epoch second would hold. It is
/// a RECOGNIZED value rather than epoch zero, which would parse cleanly and
/// render as 56 years ago, and rather than an empty field, which the reader
/// could not tell from a line it failed to parse.
const NO_CLOCK: &str = "-";

#[cfg(test)]
mod tests;
