use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub(crate) struct RecordedCli {
    directory: PathBuf,
    pub(crate) binary: String,
}

fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

impl RecordedCli {
    pub(crate) fn answering(list: &str, action: &str) -> Self {
        let number = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory =
            std::env::temp_dir().join(format!("hwj-cli-{}-{number}", std::process::id()));
        std::fs::create_dir(&directory).expect("create owned CLI directory");
        let binary = directory.join("herdr");
        let script = format!(
            "#!/bin/bash\nset -euo pipefail\ncd {}\ncount=0\nif [[ -f count ]]; then read -r count < count; fi\ncount=$((count + 1))\nprintf '%s\\n' \"$count\" > count\nprintf '%s\\0' \"$@\" > \"call-$count\"\ncase \"${{1-}} ${{2-}}\" in\n  'workspace list') printf '%s' {} ;;\n  'workspace focus'|'workspace create') printf '%s' {} ;;\n  *) exit 2 ;;\nesac\n",
            quoted(&directory.to_string_lossy()),
            quoted(list),
            quoted(action)
        );
        std::fs::write(&binary, script).expect("write argument-recording executable");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700))
            .expect("make fake CLI executable");
        Self {
            binary: binary.to_string_lossy().into_owned(),
            directory,
        }
    }

    pub(crate) fn calls(&self) -> Vec<Vec<String>> {
        let count: usize = std::fs::read_to_string(self.directory.join("count"))
            .expect("CLI ran")
            .trim()
            .parse()
            .expect("call count");
        (1..=count)
            .map(|number| {
                let bytes = std::fs::read(self.directory.join(format!("call-{number}")))
                    .expect("argv record");
                bytes[..bytes.len() - 1]
                    .split(|byte| *byte == 0)
                    .map(|arg| String::from_utf8(arg.to_vec()).expect("UTF-8 test argument"))
                    .collect()
            })
            .collect()
    }
}

impl Drop for RecordedCli {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).expect("remove owned CLI fixture");
    }
}
