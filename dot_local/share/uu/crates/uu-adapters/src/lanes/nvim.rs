use crate::config::NvimHost;
use crate::lanes::text::stdout_lines;
use crate::lanes::{CommandRunner, Verdict};
use uu_domain::LaneReport;
mod mason;
mod parsers;
mod plugins;

fn invoke(
    host: &NvimHost,
    module: &str,
    extra: &[&str],
    name: &str,
    runner: &dyn CommandRunner,
) -> LaneReport {
    let init = format!("{}/init.lua", host.config.trim_end_matches('/'));
    let script = format!("{}/lua/uu/{module}.lua", host.config.trim_end_matches('/'));
    let mut args = vec!["--headless", "-u", &init, "-l", &script];
    args.extend_from_slice(extra);
    let mut report = LaneReport::new(name);
    match runner.run_with_input(&host.nvim, &args, "") {
        Ok(ran) => {
            for line in stdout_lines(&ran.stdout) {
                report.noted(line);
            }
            match ran.verdict {
                Verdict::Clean => {}
                Verdict::Pending(reason) => {
                    report.pending(format!("{}: pending ({reason})", host.nvim))
                }
                Verdict::Failed(reason) | Verdict::Deferred(reason) => {
                    report.failed(format!("{}: {reason}", host.nvim))
                }
            }
        }
        Err(why) => report.failed(why),
    }
    report
}

#[cfg(test)]
mod tests;
