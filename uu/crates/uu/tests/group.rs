mod support;

use support::*;

#[test]
fn a_group_runs_its_lanes_in_the_order_it_lists_them() {
    let home = Home::new("group-order");
    let ran = home.dir.join("ran");
    let first = home.write_stub("first", &format!("cat >/dev/null\necho first >>{ran:?}\n"));
    let second = home.write_stub(
        "second",
        &format!("cat >/dev/null\necho second >>{ran:?}\n"),
    );
    let home = home.with_config(&format!(
        "[lane.first]\ncommand = [{first:?}]\n\n\
         [lane.second]\ncommand = [{second:?}]\n\n\
         [group.both]\nlanes = [\"second\", \"first\"]\n"
    ));
    let output = home.uu(&["run", "both"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        std::fs::read_to_string(&ran).expect("both lanes ran"),
        "second\nfirst\n"
    );
}
