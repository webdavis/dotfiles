use super::*;
use serde_json::{Value, json};
pub(super) struct Fixture {
    pub lane: NvimSmokeTestLane,
    pub cache: PathBuf,
    pub data: PathBuf,
}
impl Fixture {
    pub fn new(tag: &str) -> Self {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("uu-smoke-{tag}-{}", std::process::id()));
        let config = root.join("config");
        let cache = root.join("k");
        let data = root.join("d");
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("init.lua"), "owned init").unwrap();
        fs::write(config.join("lazy-lock.json"), "starting lock").unwrap();
        fs::create_dir_all(data.join("nvim/mason/bin")).unwrap();
        fs::create_dir_all(data.join("nvim/mason/package")).unwrap();
        fs::write(data.join("nvim/mason/package/tool"), "owned tool").unwrap();
        symlink("../package/tool", data.join("nvim/mason/bin/owned")).unwrap();
        Self {
            lane: NvimSmokeTestLane {
                host: NvimHost {
                    nvim: "/fixture/nvim".into(),
                    config: config.to_str().unwrap().into(),
                },
                cache: cache.to_str().unwrap().into(),
            },
            cache,
            data,
        }
    }
    pub fn run(&self, child: &Child) -> LaneReport {
        run_smoke(&self.lane, &self.data, "my-editor", child)
    }
}
pub(super) struct Child<'a> {
    f: &'a Fixture,
    mode: &'a str,
    pub calls: RefCell<Vec<Vec<String>>>,
}
impl<'a> Child<'a> {
    pub fn new(f: &'a Fixture, mode: &'a str) -> Self {
        Self {
            f,
            mode,
            calls: RefCell::new(Vec::new()),
        }
    }
}
impl CommandRunner for Child<'_> {
    fn run(&self, _: &str, _: &[&str]) -> Result<String, String> {
        panic!("smoke children require verdicts")
    }
    fn run_with_deadline(&self, _: &str, _: &[&str], _: Duration) -> Result<String, String> {
        panic!("whole lane owns deadline")
    }
    fn run_with_input(&self, program: &str, args: &[&str], input: &str) -> Result<Ran, String> {
        assert_eq!(input, "");
        self.calls.borrow_mut().push(
            std::iter::once(program)
                .chain(args.iter().copied())
                .map(String::from)
                .collect(),
        );
        let prepare = args.last() == Some(&"prepare");
        if prepare {
            fs::write(self.f.cache.join("c/nvim/lazy-lock.json"), "candidate lock").unwrap();
        } else if self.mode != "missing" {
            let request: Value =
                serde_json::from_slice(&fs::read(self.f.cache.join("run.json")).unwrap()).unwrap();
            let run = if self.mode == "stale" {
                json!("previous run")
            } else {
                request["run"].clone()
            };
            let lock = if self.mode == "lock-mismatch" {
                "wrong lock"
            } else {
                "candidate lock"
            };
            let errors = if self.mode == "unreadable" {
                vec!["Snacks history unreadable"]
            } else {
                vec![]
            };
            fs::write(self.f.cache.join("completion.json"),json!({"run":run,"lock":lock,"vim_enter":true,"errors":errors,"health_errors":1,"health_warnings":2}).to_string()).unwrap();
        }
        let verdict = match (prepare, self.mode) {
            (true, "prepare-fail") => Verdict::Failed("prepare failed".into()),
            (false, "verify-fail") => Verdict::Failed("verify failed".into()),
            _ => Verdict::Clean,
        };
        Ok(Ran {
            stdout: "owned output\n".into(),
            stderr: if !prepare && self.mode == "stderr" {
                "startup stderr".into()
            } else {
                String::new()
            },
            verdict,
        })
    }
}
