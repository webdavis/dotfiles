use pns_application::{AgentWork, CommandRunner};

pub struct HerdrWork<R>(pub R);
impl<R: CommandRunner> AgentWork for HerdrWork<R> {
    fn statuses(&self) -> Vec<String> {
        self.0
            .run("herdr", &["workspace", "list"])
            .map(|answer| super::workspace_agent_statuses(&answer))
            .unwrap_or_default()
    }
}
