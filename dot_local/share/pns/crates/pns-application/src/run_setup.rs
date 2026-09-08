use crate::{ConfigPublisher, ConfigRenderer, Terminal};
mod walk;

pub struct RunSetup<'a, T, R, P> {
    pub terminal: &'a T,
    pub renderer: &'a R,
    pub publisher: &'a P,
}
impl<T: Terminal, R: ConfigRenderer, P: ConfigPublisher> RunSetup<'_, T, R, P> {
    pub fn run(&self, force: bool, mut report_error: impl FnMut(&str)) -> i32 {
        // THE CONFIG IS CHECKED BEFORE THE TERMINAL IS, because it is the more
        // specific answer: an operator who already has one is told that, whether
        // or not they are sitting in front of the questions.
        // `symlink_metadata`, NOT `exists`: `exists` follows a symlink and asks
        // what it resolves to, so a dangling one at the config name reads as
        // nothing at all here and the whole walk runs before the publish refuses
        // it with a claim that it "appeared while the questions were being
        // answered", which would not be true.
        if let Err(refusal) = self.publisher.check(force) {
            report_error(&refusal);
            return 2;
        }
        if !self.terminal.is_terminal() {
            report_error(
                "pns setup: this is a walk through questions and stdin is not a terminal; \
             nothing was written",
            );
            return 2;
        }
        let answers = match walk::walk(self.terminal) {
            Ok(answers) => answers,
            Err(reason) => {
                report_error(&format!("pns setup: {reason}; nothing was written"));
                return 2;
            }
        };
        let composed = self.renderer.compose(&answers);
        // THROUGH THE ENGINE'S OWN PARSER BEFORE IT IS PUBLISHED. A wizard that
        // writes a config pns then refuses is worse than no wizard: it leaves a
        // machine falling back to the core with a complaint nobody is standing in
        // front of, and it does it while the operator is being told it worked.
        if let Err(error) = self.renderer.validate(&composed) {
            report_error(&format!(
                "pns setup: what it composed does not load ({}); nothing was written",
                error
            ));
            return 2;
        }
        match self.publisher.publish(&composed, force) {
            Ok(backup) => {
                if let Some(backup) = backup {
                    self.terminal
                        .say(&format!("pns setup: kept the old config at {}", backup));
                }
                self.terminal
                    .say(&format!("pns setup: wrote {}", self.publisher.path()));
                0
            }
            Err(refusal) => {
                report_error(&format!("pns setup: {refusal}"));
                1
            }
        }
    }
}
/// What a setup typed wrong is told.
pub const SETUP_USAGE: &str =
    "pns: usage: pns setup [--force]; --force replaces an existing config, keeping it beside";

#[cfg(test)]
mod tests;
