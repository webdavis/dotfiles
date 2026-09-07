use crate::record::{gap_line, record_detail};
use uu_application::{MarkerSnapshot, Notice, RunHeader, RunPresentation};
use uu_domain::LaneReport;

use crate::delivery::target_name;
use crate::system::{host, iso};

pub struct ConsoleRunPresentation;

impl RunPresentation for ConsoleRunPresentation {
    fn header(&self, epoch: i64, marker: &MarkerSnapshot) -> RunHeader {
        RunHeader {
            started_iso: iso(epoch),
            gap: gap_line(&marker.value, &marker.location, epoch),
            host: host(),
        }
    }

    fn write_record(&self, header: &RunHeader, reports: &[LaneReport]) -> String {
        let detail = record_detail(&header.host, &header.started_iso, &header.gap, reports);
        print!("{detail}");
        detail
    }

    fn notice(&self, notice: Notice<'_>) {
        match notice {
            Notice::NoAlerts { target, summary } => {
                let lane = target_name(target);
                println!("uu: no [alerts] block; `{lane}: {summary}` was logged and nothing else");
            }
            Notice::AlertFailed { target, cause } => {
                let lane = target_name(target);
                println!(
                    "uu: the alert for `{lane}` was NOT delivered ({cause}); it is logged here instead"
                );
            }
            Notice::NoRecords => {
                println!("uu: no [records] block; this run was logged here and nowhere else");
            }
            Notice::SigningFailed => {
                println!("uu: the [records] key is empty, so nothing could be signed or posted");
            }
            Notice::RecordPosted(description) => println!("uu: {description}"),
            Notice::MarkerClockFailed(location) => eprintln!(
                "uu: this clock could not be read at the end of the run, so the successful-run \
                 timestamp at {location} was left as it was; the next entry measures its gap from the \
                 run before this one"
            ),
            Notice::MarkerWriteFailed(failure) => eprintln!(
                "uu: could not record the successful-run timestamp at {}: {}; the next entry \
                 will report a stale or absent gap",
                failure.location, failure.cause
            ),
            Notice::StreakWriteFailed { lane, failure } => eprintln!(
                "uu: could not record lane `{lane}`'s non-success streak at {}: {}",
                failure.location, failure.cause
            ),
        }
    }
}
