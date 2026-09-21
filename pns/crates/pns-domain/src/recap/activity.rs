//! What the activity store holds, and how the `agents` section groups it.
//!
//! THE ROW TYPE IS POLICY rather than a persistence detail, which is why it
//! lives here and the SQLite table re-exports it. Grouping events into
//! sessions and sessions into projects is arithmetic over those fields, and a
//! domain that could not name the fields would have to be handed a shape the
//! store invented.

/// One hook event as the recap reads it back. Empty fields are a harness with
/// nothing to say: Codex sends no session title, a pane outside herdr has no
/// id, and only a model switch states a model.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Event {
    pub at: u64,
    pub agent: String,
    pub state: String,
    pub project: String,
    pub branch: String,
    pub session: String,
    pub session_title: String,
    pub pane: String,
    pub workspace: String,
    pub model: String,
    pub title: String,
    pub detail: String,
    /// Where this session's own transcript lives, which `--with-transcripts`
    /// reads and nothing else does. Empty for a harness that sends none.
    pub transcript_path: String,
}

/// One session's events inside one window, in the order they arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub session: String,
    pub title: String,
    pub harness: String,
    pub branch: String,
    pub pane: String,
    pub workspace: String,
    pub model: String,
    /// How long the session ran INSIDE the window: the first event to the
    /// last. A session with one event ran no measurable time, which is a
    /// truthful zero rather than a guess at how long the turn took.
    pub duration_secs: u64,
    pub last_state: String,
    /// This session's own transcript, which `--with-transcripts` reads.
    pub transcript_path: String,
    pub events: Vec<Event>,
}

/// One project, and the sessions that touched it in the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub project: String,
    pub sessions: Vec<Session>,
}

/// The window's events grouped by project and then by session, both in the
/// order they were first seen.
///
/// FIRST-SEEN ORDER, NEVER SORTED BY NAME. The events arrive oldest first, so
/// this reads down the window the way it happened, which is the order every
/// other part of the recap prints in.
///
/// A ROW WITH NO SESSION ID IS STILL A SESSION, keyed by the empty string.
/// Dropping it would lose a harness that sends no session id from the page
/// entirely, and the page's job is to say what happened.
///
/// THE LAST FIELD WINS for a session's title, branch, pane, workspace and
/// model. A rename mid-session, a branch switch and a model switch are all
/// facts stated by a LATER event, and the recap reports where the session
/// ended up. An empty later value never overwrites a known one, because a
/// harness that stopped sending a field has not changed it.
pub fn by_project(events: &[Event]) -> Vec<Project> {
    let mut projects: Vec<Project> = Vec::new();
    for event in events {
        let project = match projects
            .iter_mut()
            .position(|held| held.project == event.project)
        {
            Some(at) => &mut projects[at],
            None => {
                projects.push(Project {
                    project: event.project.clone(),
                    sessions: Vec::new(),
                });
                projects.last_mut().expect("just pushed")
            }
        };
        match project
            .sessions
            .iter_mut()
            .find(|held| held.session == event.session)
        {
            Some(session) => session.absorb(event),
            None => project.sessions.push(Session::opened(event)),
        }
    }
    projects
}

impl Session {
    fn opened(event: &Event) -> Session {
        Session {
            session: event.session.clone(),
            title: event.session_title.clone(),
            harness: event.agent.clone(),
            branch: event.branch.clone(),
            pane: event.pane.clone(),
            workspace: event.workspace.clone(),
            model: event.model.clone(),
            duration_secs: 0,
            last_state: event.state.clone(),
            transcript_path: event.transcript_path.clone(),
            events: vec![event.clone()],
        }
    }
    fn absorb(&mut self, event: &Event) {
        keep_latest(&mut self.title, &event.session_title);
        keep_latest(&mut self.branch, &event.branch);
        keep_latest(&mut self.pane, &event.pane);
        keep_latest(&mut self.workspace, &event.workspace);
        keep_latest(&mut self.model, &event.model);
        keep_latest(&mut self.harness, &event.agent);
        keep_latest(&mut self.transcript_path, &event.transcript_path);
        if !event.state.is_empty() {
            self.last_state = event.state.clone();
        }
        // SATURATING, because the rows arrive in `at` order and a store that
        // handed them back out of order would otherwise underflow into a
        // duration of several centuries.
        self.duration_secs = event
            .at
            .saturating_sub(self.events.first().map_or(event.at, |first| first.at));
        self.events.push(event.clone());
    }
}

/// A later value replaces an earlier one only when the later one says
/// something. See `by_project`.
fn keep_latest(held: &mut String, arrived: &str) {
    if !arrived.is_empty() {
        held.clear();
        held.push_str(arrived);
    }
}

#[cfg(test)]
#[path = "activity/tests.rs"]
mod tests;
