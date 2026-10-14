//! Tests for the OSC 7501 Program Status Protocol
use super::*;
use k9::assert_equal as assert_eq;

const TERRAFORM: &str = "\x1b]7501;state=blocked:kind=permission:app=terraform:msg=QXBwbHkgMyB0byBhZGQsIDEgdG8gY2hhbmdlLCAwIHRvIGRlc3Ryb3k/\x1b\\";
const PROMPT: &str = "\x1b]133;A\x1b\\";

fn status(state: ProgramState) -> ProgramStatus {
    ProgramStatus {
        id: None,
        state,
        kind: None,
        progress: None,
        app: None,
        title: None,
        msg: None,
    }
}

fn child(id: &str, state: ProgramState) -> ProgramStatus {
    ProgramStatus {
        id: Some(id.to_string()),
        ..status(state)
    }
}

fn report(term: &mut TestTerm, body: &str) {
    term.print(format!("]7501;{body}\\"));
}

fn root(term: &TestTerm) -> Option<ProgramStatus> {
    term.get_program_statuses().get(None)
}

#[test]
fn test_report() {
    let mut term = TestTerm::new(3, 20, 0);
    assert!(term.get_program_statuses().is_empty());

    term.print(TERRAFORM);
    assert_eq!(
        root(&term),
        Some(ProgramStatus {
            kind: Some(ProgramStatusKind::Permission),
            app: Some("terraform".into()),
            msg: Some("Apply 3 to add, 1 to change, 0 to destroy?".into()),
            ..status(ProgramState::Blocked)
        })
    );

    // Each report replaces the record completely; terminated by BEL here
    term.print("]7501;state=working:progress=40");
    let working = Some(ProgramStatus {
        progress: Some(40),
        ..status(ProgramState::Working)
    });
    assert_eq!(root(&term), working);

    // A discarded report changes nothing
    report(&mut term, "state=done:msg=YQpi");
    report(&mut term, "state=sleeping");
    assert_eq!(root(&term), working);

    report(&mut term, "state=clear");
    assert!(term.get_program_statuses().is_empty());
}

#[test]
fn test_child_records() {
    let mut term = TestTerm::new(3, 20, 0);
    report(&mut term, "state=working:app=deploy");
    report(&mut term, "state=working:id=us-east:progress=40");
    report(&mut term, "state=blocked:id=eu-west");
    report(&mut term, "state=done:id=eu-west/db:app=psql");

    let set = term.get_program_statuses();
    // Children without an app take the nearest ancestor's
    assert_eq!(
        set.get(Some("us-east")),
        Some(ProgramStatus {
            app: Some("deploy".into()),
            progress: Some(40),
            ..child("us-east", ProgramState::Working)
        })
    );
    assert_eq!(
        set.records()
            .into_iter()
            .map(|r| (r.id, r.app))
            .collect::<Vec<_>>(),
        vec![
            (None, Some("deploy".into())),
            (Some("eu-west".into()), Some("deploy".into())),
            (Some("eu-west/db".into()), Some("psql".into())),
            (Some("us-east".into()), Some("deploy".into())),
        ]
    );
    assert_eq!(set.most_urgent().unwrap().id, Some("eu-west".into()));

    // Clearing a record clears the records beneath it, and only those
    report(&mut term, "state=done:id=eu-west2");
    report(&mut term, "state=clear:id=eu-west");
    let ids: Vec<_> = term
        .get_program_statuses()
        .records()
        .into_iter()
        .map(|r| r.id)
        .collect();
    assert_eq!(
        ids,
        vec![None, Some("eu-west2".into()), Some("us-east".into())]
    );

    // Clearing without an id clears every record
    report(&mut term, "state=clear");
    assert!(term.get_program_statuses().is_empty());
}

#[test]
fn test_most_urgent() {
    let mut term = TestTerm::new(3, 20, 0);
    let urgent = |term: &TestTerm| term.get_program_statuses().most_urgent().map(|r| r.id);

    report(&mut term, "state=idle");
    report(&mut term, "state=done:id=a");
    assert_eq!(urgent(&term), Some(Some("a".into())));
    report(&mut term, "state=working:id=b");
    assert_eq!(urgent(&term), Some(Some("b".into())));
    report(&mut term, "state=error:id=c");
    assert_eq!(urgent(&term), Some(Some("c".into())));
    report(&mut term, "state=blocked");
    assert_eq!(urgent(&term), Some(None));

    // Of equally urgent records, the most recently updated wins
    report(&mut term, "state=blocked:id=d");
    assert_eq!(urgent(&term), Some(Some("d".into())));
    report(&mut term, "state=blocked:msg=QQ");
    assert_eq!(urgent(&term), Some(None));
}

#[test]
fn test_remove() {
    let mut term = TestTerm::new(3, 20, 0);
    report(&mut term, "state=done");
    report(&mut term, "state=done:id=a");
    report(&mut term, "state=done:id=a/b");

    // Removes just the addressed record, not those beneath it
    term.remove_program_status(Some("a"));
    term.remove_program_status(None);
    assert_eq!(
        term.get_program_statuses().records(),
        vec![child("a/b", ProgramState::Done)]
    );

    term.clear_program_statuses();
    assert!(term.get_program_statuses().is_empty());
}

#[test]
fn test_record_cap() {
    let mut term = TestTerm::new(3, 20, 0);
    for i in 0..ProgramStatusSet::MAX_RECORDS {
        report(&mut term, &format!("state=working:id=r{i}"));
    }
    // Updating r0 makes r1 the least recently updated record
    report(&mut term, "state=done:id=r0");
    report(&mut term, "state=done:id=new");

    let set = term.get_program_statuses();
    assert_eq!(set.records().len(), ProgramStatusSet::MAX_RECORDS);
    assert_eq!(set.get(Some("r1")), None);
    assert!(set.get(Some("r0")).is_some());
    assert!(set.get(Some("new")).is_some());
}

#[test]
fn test_lifetime() {
    let mut term = TestTerm::new(3, 20, 0);

    // A new shell prompt drops working and blocked records...
    for state in ["working", "blocked"] {
        report(&mut term, &format!("state={state}"));
        report(&mut term, &format!("state={state}:id=child"));
        assert!(!term.get_program_statuses().is_empty());
        term.print(PROMPT);
        assert!(term.get_program_statuses().is_empty(), "{}", state);
    }

    // ...but not idle, done or error
    for (name, state) in [
        ("idle", ProgramState::Idle),
        ("done", ProgramState::Done),
        ("error", ProgramState::Error),
    ] {
        report(&mut term, &format!("state={name}"));
        term.print(PROMPT);
        assert_eq!(root(&term), Some(status(state)), "{name}");
    }

    // Soft reset keeps the records, full reset removes them all
    report(&mut term, "state=done:id=child");
    term.print("[!p");
    assert_eq!(root(&term), Some(status(ProgramState::Error)));
    term.print("c");
    assert!(term.get_program_statuses().is_empty());
}

#[derive(Clone, Default)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn test_feature_detection() {
    let writer = SharedWriter::default();
    let mut term = Terminal::new(
        TerminalSize::default(),
        Arc::new(TestTermConfig { scrollback: 0 }),
        "WezTerm",
        "O_o",
        Box::new(writer.clone()),
    );
    term.advance_bytes("\x1b]7501;?\x1b\\");
    assert_eq!(
        String::from_utf8(writer.0.lock().unwrap().clone()).unwrap(),
        "\x1b]7501;?\x1b\\"
    );
    assert!(term.get_program_statuses().is_empty());
}
