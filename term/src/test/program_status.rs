//! Tests for the OSC 7501 Program Status Protocol
use super::*;
use k9::assert_equal as assert_eq;

const TERRAFORM: &str = "\x1b]7501;state=blocked:kind=permission:app=terraform:msg=QXBwbHkgMyB0byBhZGQsIDEgdG8gY2hhbmdlLCAwIHRvIGRlc3Ryb3k/\x1b\\";
const PROMPT: &str = "\x1b]133;A\x1b\\";

fn status(state: ProgramState) -> ProgramStatus {
    ProgramStatus {
        state,
        kind: None,
        progress: None,
        app: None,
        title: None,
        msg: None,
    }
}

#[test]
fn test_report() {
    let mut term = TestTerm::new(3, 20, 0);
    assert_eq!(term.get_program_status(), None);

    term.print(TERRAFORM);
    assert_eq!(
        term.get_program_status(),
        Some(ProgramStatus {
            kind: Some(ProgramStatusKind::Permission),
            app: Some("terraform".into()),
            msg: Some("Apply 3 to add, 1 to change, 0 to destroy?".into()),
            ..status(ProgramState::Blocked)
        })
    );

    // Each report replaces the record completely; terminated by BEL here
    term.print("\x1b]7501;state=working:progress=40\x07");
    assert_eq!(
        term.get_program_status(),
        Some(ProgramStatus {
            progress: Some(40),
            ..status(ProgramState::Working)
        })
    );

    // A discarded report changes nothing
    term.print("\x1b]7501;state=done:msg=YQpi\x1b\\");
    term.print("\x1b]7501;state=sleeping\x1b\\");
    assert_eq!(
        term.get_program_status(),
        Some(ProgramStatus {
            progress: Some(40),
            ..status(ProgramState::Working)
        })
    );

    term.print("\x1b]7501;state=clear\x1b\\");
    assert_eq!(term.get_program_status(), None);
}

#[test]
fn test_child_records_ignored() {
    let mut term = TestTerm::new(3, 20, 0);
    term.print("\x1b]7501;state=done\x1b\\");
    term.print("\x1b]7501;state=blocked:id=build\x1b\\");
    term.print("\x1b]7501;state=clear:id=build\x1b\\");
    assert_eq!(term.get_program_status(), Some(status(ProgramState::Done)));
}

#[test]
fn test_lifetime() {
    let mut term = TestTerm::new(3, 20, 0);

    // A new shell prompt drops working and blocked records...
    for state in ["working", "blocked"] {
        term.print(format!("\x1b]7501;state={state}\x1b\\"));
        assert!(term.get_program_status().is_some());
        term.print(PROMPT);
        assert_eq!(term.get_program_status(), None, "{state}");
    }

    // ...but not idle, done or error
    for (name, state) in [
        ("idle", ProgramState::Idle),
        ("done", ProgramState::Done),
        ("error", ProgramState::Error),
    ] {
        term.print(format!("\x1b]7501;state={name}\x1b\\"));
        term.print(PROMPT);
        assert_eq!(term.get_program_status(), Some(status(state)), "{name}");
    }

    // Soft reset keeps the record, full reset removes it
    term.print("\x1b[!p");
    assert_eq!(term.get_program_status(), Some(status(ProgramState::Error)));
    term.print("\x1bc");
    assert_eq!(term.get_program_status(), None);

    term.print("\x1b]7501;state=done\x1b\\");
    term.clear_program_status();
    assert_eq!(term.get_program_status(), None);
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
    assert_eq!(term.get_program_status(), None);
}
