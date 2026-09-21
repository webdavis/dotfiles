use super::*;
use crate::test_sandbox::Sandbox;
use posture_adapters::DesiredStaging;
use posture_application::{ConvergeStaging, StagingRefusal};
use posture_domain::ConvergeFile;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};

struct Fixture {
    /// Canonical, because a refusal names the path it resolved and a
    /// symlinked temporary directory would not match it.
    root: PathBuf,
    /// Removes the tree when the test drops the fixture.
    _sandbox: Sandbox,
}

impl Fixture {
    fn new() -> Self {
        let sandbox = Sandbox::new("converge-default");
        let root = sandbox.path().canonicalize().unwrap();
        fs::create_dir(root.join("scratch")).unwrap();
        Self {
            root,
            _sandbox: sandbox,
        }
    }

    fn desired(&self) -> PathBuf {
        self.root.join(".local/libexec/posture/converge/desired")
    }

    fn write_tree(&self, path: &std::path::Path) {
        fs::create_dir_all(path.join("packs")).unwrap();
        for file in ConvergeFile::ALL {
            fs::write(path.join(file.relative_path()), b"desired bytes\n").unwrap();
        }
    }

    fn staging(&self) -> DesiredStaging {
        let config =
            Configuration::read(|name| (name == "HOME").then(|| self.root.as_os_str().to_owned()))
                .unwrap();
        DesiredStaging::new(config.desired, self.root.join("scratch"))
    }

    fn assert_no_copy(&self) {
        assert_eq!(fs::read_dir(self.root.join("scratch")).unwrap().count(), 0);
    }
}

#[test]
fn the_default_desired_tree_is_copied_privately_before_the_source_can_change() {
    let fixture = Fixture::new();
    fixture.write_tree(&fixture.desired());
    let staged = fixture.staging().prepare().unwrap();
    let source = staged.source(ConvergeFile::Configuration);
    let private = source.parent().unwrap();
    assert_ne!(private, fixture.desired());
    assert_eq!(
        fs::metadata(private).unwrap().permissions().mode() & 0o7777,
        0o700
    );
    fs::write(fixture.desired().join("osquery.conf"), b"substituted").unwrap();
    assert_eq!(fs::read(source).unwrap(), b"desired bytes\n");
    drop(staged);
    fixture.assert_no_copy();
}

#[test]
fn a_missing_default_desired_tree_does_not_read_legacy_files() {
    let fixture = Fixture::new();
    let legacy = fixture
        .root
        .join(".local/libexec/osquery/osquery-converge/desired");
    fixture.write_tree(&legacy);
    assert_eq!(
        fixture.staging().prepare().unwrap_err(),
        [StagingRefusal::MissingDirectory(fixture.desired())]
    );
    fixture.assert_no_copy();
    assert_eq!(
        fs::read(legacy.join("osquery.conf")).unwrap(),
        b"desired bytes\n"
    );
}

#[test]
fn a_symlink_at_the_new_converge_component_is_refused_before_copying() {
    let fixture = Fixture::new();
    fixture.write_tree(&fixture.desired());
    let component = fixture.desired().parent().unwrap().to_path_buf();
    let substitute = fixture.root.join("substitute");
    fs::rename(&component, &substitute).unwrap();
    symlink(&substitute, &component).unwrap();
    assert_eq!(
        fixture.staging().prepare().unwrap_err(),
        [StagingRefusal::SymlinkComponent(component)]
    );
    fixture.assert_no_copy();
}
