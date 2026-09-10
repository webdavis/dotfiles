use pns_domain::Answers;

pub trait Terminal {
    fn is_terminal(&self) -> bool;
    fn say(&self, line: &str);
    /// The walk's opening: the invocation, then labelled lines, then a rule.
    ///
    /// A SHAPE RATHER THAN A STRING, because this crate decides nothing about
    /// how a terminal looks and must not learn. The adapter owns the style; the
    /// walk owns what the words are.
    fn open(&self, invocation: &str, lines: &[(&str, &str)]);
    /// A titled section of the walk, with a faint blurb beside it.
    fn section(&self, title: &str, blurb: &str);
    fn ask(&self, question: &str) -> Result<String, String>;
    fn ask_hidden(&self, question: &str) -> Result<String, String>;
}

pub trait ConfigRenderer {
    fn compose(&self, answers: &Answers) -> String;
    fn validate(&self, composed: &str) -> Result<(), String>;
}

pub trait ConfigPublisher {
    fn path(&self) -> String;
    fn check(&self, force: bool) -> Result<(), String>;
    fn publish(&self, composed: &str, force: bool) -> Result<Option<String>, String>;
}
