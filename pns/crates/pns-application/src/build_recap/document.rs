//! The recap as one machine-readable document, schema 1.
//!
//! IT ALWAYS CARRIES EVERY CONFIGURED SECTION, empty or not, because omission
//! is a display choice and a consumer wants the whole shape every time. Only
//! `--section` narrows it, and even then `schema` and `window` stay.
//!
//! VERBOSE DETAIL IS ALWAYS PRESENT. `-v` decides what a person is shown; a
//! consumer parsing this has no such preference, and a field that came and
//! went with a flag is a field nobody can rely on.

use super::{AGENTS, Assembled, OPEN, wanted};
use pns_domain::recap::{
    activity::{Project, Session},
    document::{Node, SCHEMA},
    external::Sourcing,
    sections::Open,
};

/// The whole document for one assembled recap.
pub fn document(assembled: &Assembled, wall_clock: impl Fn(u64) -> String) -> Node {
    let mut sections: Vec<(String, Node)> = Vec::new();
    if wanted(&assembled.sections, AGENTS) {
        sections.push((
            AGENTS.to_string(),
            Node::List(
                assembled
                    .projects
                    .iter()
                    .map(|project| project_node(project, &wall_clock))
                    .collect(),
            ),
        ));
    }
    for gathered in &assembled.sources {
        sections.push((gathered.name.to_string(), rows_node(&gathered.sourcing)));
    }
    if wanted(&assembled.sections, OPEN) {
        sections.push((OPEN.to_string(), open_node(&assembled.open)));
    }
    Node::map([
        ("schema", Node::Number(SCHEMA)),
        (
            "window",
            Node::map([
                (
                    "name",
                    match &assembled.window {
                        Some(name) => Node::text(name),
                        None => Node::Absent,
                    },
                ),
                ("since", Node::text(&wall_clock(assembled.since))),
                ("until", Node::text(&wall_clock(assembled.until))),
                ("previous", Node::Flag(assembled.previous)),
            ]),
        ),
        ("sections", Node::Map(sections)),
    ])
}

fn project_node(project: &Project, wall_clock: &impl Fn(u64) -> String) -> Node {
    Node::map([
        ("project", Node::text(&project.project)),
        (
            "sessions",
            Node::List(
                project
                    .sessions
                    .iter()
                    .map(|session| session_node(session, wall_clock))
                    .collect(),
            ),
        ),
    ])
}

fn session_node(session: &Session, wall_clock: &impl Fn(u64) -> String) -> Node {
    Node::map([
        ("title", Node::text(&session.title)),
        ("harness", Node::text(&session.harness)),
        ("branch", Node::text(&session.branch)),
        ("session", Node::text(&session.session)),
        ("pane", Node::text(&session.pane)),
        ("workspace", Node::text(&session.workspace)),
        ("model", Node::text(&session.model)),
        ("duration_secs", Node::Number(session.duration_secs)),
        ("last_state", Node::text(&session.last_state)),
        (
            "events",
            Node::List(
                session
                    .events
                    .iter()
                    .map(|event| {
                        Node::map([
                            ("at", Node::text(&wall_clock(event.at))),
                            ("state", Node::text(&event.state)),
                            ("title", Node::text(&event.title)),
                            ("detail", Node::text(&event.detail)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

/// One list section: the rows it holds, and whether a cap upstream stopped
/// the fetch before every row was read.
///
/// `more` IS ALWAYS ZERO: the document applies no cap of its own and prints
/// every row `sourcing` holds, so it has no omitted count to report. A
/// fetch's own ceiling is a different fact, and `at_least` is that one, true
/// when the rows read cannot be called a total. The three states that carry
/// no rows say so in `state` rather than by being absent, because a consumer
/// that could not tell a broken command from a quiet window would report the
/// wrong news.
fn rows_node(sourcing: &Sourcing) -> Node {
    let rows = sourcing.rows();
    Node::map([
        (
            "state",
            Node::text(match sourcing {
                Sourcing::Unconfigured => "unconfigured",
                Sourcing::Unavailable => "unavailable",
                Sourcing::Failed(_) => "failed",
                Sourcing::Read(..) => "read",
            }),
        ),
        (
            "exit_code",
            match sourcing {
                Sourcing::Failed(code) => Node::Number(u64::try_from(*code).unwrap_or_default()),
                _ => Node::Absent,
            },
        ),
        ("more", Node::Number(0)),
        (
            "at_least",
            Node::Flag(matches!(sourcing, Sourcing::Read(_, true))),
        ),
        ("rows", Node::rows(&rows)),
    ])
}

fn open_node(open: &Open) -> Node {
    Node::map([
        ("sessions", Node::rows(&open.sessions)),
        ("pull_requests", Node::rows(&open.pull_requests)),
        ("applies", Node::rows(&open.applies)),
        // ALWAYS PRESENT, zero included: the document carries the whole shape
        // every time, so a consumer never has to tell an absent field from a
        // count of none.
        ("dead_lettered", Node::Number(open.dead_lettered as u64)),
    ])
}
