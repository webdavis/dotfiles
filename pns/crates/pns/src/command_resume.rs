use crate::style::{self, HeaderLine, Paint, Tone};
use pns_adapters::WorkspaceRow;
use pns_application::CommandRunner;
use pns_protocol::ResumePage;

/// What `pns resume` takes, which is one of two ways to say the same page.
pub(crate) const RESUME_USAGE: &str = "pns: usage: pns resume [--json | --notify]";

/// `pns resume`: where the operator was, read from the state pns already
/// keeps plus one herdr listing.
///
/// A MODE, in `recap_mode`'s sense: it takes no decision from any event, and
/// the one delivery it can make is the operator's own request for it. The
/// unlock automation is a caller of `--notify` and nothing about it lives
/// here; the subcommand is the API.
///
/// EXIT 2 FOR A MISTYPED INVOCATION, `mute_mode`'s code: this is only ever
/// hand-run or run by an automation the operator wrote, and a flag swallowed
/// silently is a page they believe was sent.
pub(crate) fn resume_mode() -> i32 {
    let Some(form) = Form::of(&crate::arguments_after_subcommand()) else {
        eprintln!("{RESUME_USAGE}");
        return 2;
    };
    let page = answer();
    match form {
        Form::Page => print_page(&render(Paint::for_stdout(), &page)),
        Form::Json => match page.encode() {
            Ok(text) => print_page(&[text]),
            Err(_) => 1,
        },
        Form::Notify => notify(&page),
    }
}

/// Which of the three ways to say the page this call asked for.
#[derive(Debug, PartialEq, Eq)]
enum Form {
    /// Printed, framed, to the terminal.
    Page,
    /// The same fields, as pns's own JSON object.
    Json,
    /// The same page through the engine, as a banner and the phone when away.
    Notify,
}

impl Form {
    /// ONE FLAG AT A TIME, and nothing else at all. `--json --notify` would
    /// have to mean a printed page and a delivered page in one run, and a
    /// caller who typed both has not decided which they wanted.
    fn of(arguments: &[String]) -> Option<Self> {
        match arguments {
            [] => Some(Self::Page),
            [flag] if flag == JSON_FLAG => Some(Self::Json),
            [flag] if flag == NOTIFY_FLAG => Some(Self::Notify),
            _ => None,
        }
    }
}

/// The flag that asks for the fields rather than the page. THE JSON FIELDS
/// CARRY THE PAGE'S OWN NAMES, which is the standing rule for every pns
/// answer that has both forms.
const JSON_FLAG: &str = "--json";
/// The flag that sends the page instead of printing it.
const NOTIFY_FLAG: &str = "--notify";

/// Every answer the page is made of, gathered once.
///
/// NOTHING HERE FAILS THE COMMAND. A herdr that did not answer, a store that
/// cannot be read and a machine with nothing waiting each leave their own
/// field empty, because a report that refused to print because one of its
/// five facts was missing would be useless in exactly the moment it is asked
/// for.
fn answer() -> ResumePage {
    let workspaces = pns_adapters::parse_workspaces(
        &pns_adapters::SystemCommandRunner
            .run(HERDR, &["workspace", "list"])
            .unwrap_or_default(),
    );
    let store = pns_adapters::SqliteStore::new(crate::state_dir());
    let mut page = ResumePage {
        workspace: workspaces
            .iter()
            .find(|workspace| workspace.focused)
            .map(|workspace| workspace.label.clone())
            .unwrap_or_default(),
        command: store.newest_shell_command().unwrap_or_default(),
        ..ResumePage::default()
    };
    if let Some(waiting) = store.newest_wait() {
        page.worktree = worktree_of(&workspaces, &waiting.branch, &waiting.project);
        page.waiting = true;
        page.title = waiting.title;
        page.branch = waiting.branch;
    }
    page
}

/// The CLI this binary is allowed to run for a workspace listing.
const HERDR: &str = "herdr";

/// The checkout the waiting branch is in.
///
/// ONLY THE WORKSPACE HERDR OPENED ON THAT BRANCH'S WORKTREE, found by the
/// directory name, because a lane's worktree is `<repo>/<branch slug>`. NO
/// FALLBACK: the focused workspace and the session's own `project` are both
/// answers to a different question ("where is this terminal", "what is this
/// session called"), not "where is that branch checked out", and printing
/// either as `Worktree:` states something false with no "not known" marker.
/// A slug can repeat across repositories, so a match whose `repo_name`
/// agrees with the session's project wins over the first hit.
fn worktree_of(workspaces: &[WorkspaceRow], branch: &str, project: &str) -> String {
    let slug = branch.replace('/', "-");
    if slug.is_empty() {
        return String::new();
    }
    let matches: Vec<&WorkspaceRow> = workspaces
        .iter()
        .filter(|workspace| directory_name(&workspace.checkout_path) == slug)
        .collect();
    matches
        .iter()
        .find(|workspace| workspace.repo_name == project)
        .or_else(|| matches.first())
        .map(|workspace| workspace.checkout_path.clone())
        .unwrap_or_default()
}

fn directory_name(path: &str) -> &str {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or("")
}

/// The page, in the house style: a header, then one section per question the
/// operator is asking when they come back.
fn render(paint: Paint, page: &ResumePage) -> Vec<String> {
    let row = |tone, text: &str| style::row(paint, tone, "·", 2, text);
    let mut lines = style::header(
        paint,
        "pns resume",
        &[HeaderLine {
            label: "About",
            text: "where you were, and what is waiting on you",
        }],
    );
    lines.push(String::new());
    lines.push(style::heading(
        paint,
        "Workspace",
        "the herdr workspace you were last in",
    ));
    lines.push(row(
        Tone::Quiet,
        &named(&page.workspace, "herdr named no focused workspace"),
    ));
    lines.push(String::new());
    lines.push(style::heading(
        paint,
        "Waiting on you",
        "the agent session blocked on an answer",
    ));
    if page.waiting {
        lines.push(row(
            Tone::Warn,
            &format!("Session: {}", labelled(&page.title)),
        ));
        lines.push(row(
            Tone::Quiet,
            &format!("Branch: {}", labelled(&page.branch)),
        ));
        lines.push(row(
            Tone::Quiet,
            &format!("Worktree: {}", labelled(&page.worktree)),
        ));
    } else {
        lines.push(row(Tone::Good, "Nothing is waiting on you."));
    }
    lines.push(String::new());
    lines.push(style::heading(
        paint,
        "Last command",
        "the newest command the shell notifier timed",
    ));
    lines.push(row(
        Tone::Quiet,
        &named(&page.command, "no command has been timed yet"),
    ));
    lines
}

/// A value, or the sentence that says pns does not know it. AN UNKNOWN IS
/// SAID OUT LOUD rather than printed as a blank row, which reads as a value
/// of nothing.
fn named(value: &str, unknown: &'static str) -> String {
    if value.is_empty() {
        unknown.to_string()
    } else {
        value.to_string()
    }
}

/// A labelled field's value, or the words for the one pns does not know.
fn labelled(value: &str) -> &str {
    if value.is_empty() { "not known" } else { value }
}

fn print_page(lines: &[String]) -> i32 {
    use std::io::Write;
    if writeln!(std::io::stdout().lock(), "{}", lines.join("\n")).is_err() {
        return 1;
    }
    0
}

/// The same page, through the engine.
///
/// THE ORDINARY PRODUCER PATH AND NOT A PATH OF ITS OWN, which is what the
/// GitHub poll's own submission settled: reading an envelope this binary
/// built runs the identical ledger, policy and dispatch, so the presence gate
/// that decides banner-only or banner-and-phone is the engine's existing one
/// and this command adds no delivery rule.
///
/// AN OBSERVATION, because nothing is waiting on this page: it reports where
/// the operator was, so it changes no workflow state and arms no reminder.
///
/// THE PAGE IS THE DETAIL, rendered plain: the ledger's message is composed
/// from the detail, so what was recorded is the page the terminal would have
/// printed, with no escape sequence in it.
fn notify(page: &ResumePage) -> i32 {
    let body = render(Paint::Plain, page).join("\n");
    let Some(request) = request_for(&body) else {
        return 1;
    };
    match request.encode() {
        Ok(encoded) => crate::event_flow::submit_encoded(encoded.as_bytes()),
        Err(_) => 1,
    }
}

/// The envelope one page submits as.
///
/// THE ID IS THE SECOND IT WAS ASKED FOR, so two runs are two events: the
/// ledger is keyed on the producer's id, and a fixed id would answer the
/// second unlock of the day with the first one's record.
///
/// NO BRANCH, PROJECT OR PANE. The page already names all three in its own
/// body, and a branch on the envelope prefixes the recorded message with it.
fn request_for(body: &str) -> Option<pns_protocol::RequestEnvelope> {
    let mut request = pns_protocol::RequestEnvelope::new(
        pns_protocol::RequestId::new(format!("resume-{}", crate::now_secs().unwrap_or_default()))
            .ok()?,
        pns_protocol::Name::new(PRODUCER).ok()?,
        pns_protocol::State::Observation,
    );
    request.detail = body.to_string();
    Some(request)
}

/// Who this page is from. pns itself: the state it reads is pns's own.
const PRODUCER: &str = "pns";

#[cfg(test)]
#[path = "command_resume/tests.rs"]
mod tests;
