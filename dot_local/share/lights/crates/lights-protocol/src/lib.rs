mod command;
mod output;
pub use command::{BrightnessRequest, Command, Request, parse};
pub use output::Output;

pub const HELP: &str = "Usage: lights [--room <alias|name>] [--notify] <command>\n\
  toggle                  toggle power (default command)\n\
  on | off                request power on or off\n\
  brightness up | down    request one configured step\n\
  brightness <n>          request an absolute level, clamped to 1-100\n\
  scene <name>            activate a scene in the selected room\n\
  scene next | previous  cycle the configured rotation\n\
  status                  report power, brightness and scene\n\
  --notify                notify after an accepted write\n\
  --help                  print this help\n";
