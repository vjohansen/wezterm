use super::*;
use crate::terminalstate::performer::Performer;
use std::sync::Arc;
use wezterm_escape_parser::parser::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
pub enum ClipboardSelection {
    Clipboard,
    PrimarySelection,
}

pub trait Clipboard: Send + Sync {
    fn set_contents(
        &self,
        selection: ClipboardSelection,
        data: Option<String>,
    ) -> anyhow::Result<()>;
}

impl Clipboard for Box<dyn Clipboard> {
    fn set_contents(
        &self,
        selection: ClipboardSelection,
        data: Option<String>,
    ) -> anyhow::Result<()> {
        self.as_ref().set_contents(selection, data)
    }
}

pub trait DeviceControlHandler: Send + Sync {
    fn handle_device_control(&mut self, _control: wezterm_escape_parser::DeviceControlMode);
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
pub enum Progress {
    #[default]
    None,
    Percentage(u8),
    Error(u8),
    Indeterminate,
}

/// The state of an OSC 7501 Program Status record
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "use_serde", serde(rename_all = "lowercase"))]
pub enum ProgramState {
    Idle,
    Working,
    Done,
    Blocked,
    Error,
}

/// What a `blocked` program is waiting on
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "use_serde", serde(rename_all = "lowercase"))]
pub enum ProgramStatusKind {
    Permission,
    Question,
    Auth,
}

/// An OSC 7501 Program Status record.
/// `title` and `msg` hold decoded text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
pub struct ProgramStatus {
    /// The record's id, such as `build/test`; `None` for the root record
    pub id: Option<String>,
    pub state: ProgramState,
    pub kind: Option<ProgramStatusKind>,
    pub progress: Option<u8>,
    pub app: Option<String>,
    pub title: Option<String>,
    pub msg: Option<String>,
}

impl ProgramStatus {
    /// `working` and `blocked` records do not survive process exit
    /// or the start of a new shell prompt
    pub fn is_transient(&self) -> bool {
        matches!(self.state, ProgramState::Working | ProgramState::Blocked)
    }

    /// How urgently the record needs the user's attention:
    /// blocked > error > working > done > idle
    pub fn urgency(&self) -> u8 {
        match self.state {
            ProgramState::Idle => 1,
            ProgramState::Done => 2,
            ProgramState::Working => 3,
            ProgramState::Error => 4,
            ProgramState::Blocked => 5,
        }
    }

    /// Whether this record is `id` or one of its descendants
    fn is_within(&self, id: &str) -> bool {
        match &self.id {
            Some(own) => {
                own == id || (own.starts_with(id) && own.as_bytes().get(id.len()) == Some(&b'/'))
            }
            None => false,
        }
    }
}

/// The set of OSC 7501 Program Status records held by a terminal:
/// the root record plus any child records addressed by `id`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
pub struct ProgramStatusSet {
    /// Ordered from least to most recently updated
    records: Vec<ProgramStatus>,
}

impl ProgramStatusSet {
    /// When a report would create a record past this cap, the record
    /// that was updated least recently is removed to make room
    pub const MAX_RECORDS: usize = 256;

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Replaces the record with the same id as `status`, or adds it.
    /// Returns true if the set changed.
    pub fn update(&mut self, status: ProgramStatus) -> bool {
        if self.records.last() == Some(&status) {
            return false;
        }
        self.records.retain(|r| r.id != status.id);
        self.records.push(status);
        if self.records.len() > Self::MAX_RECORDS {
            self.records.remove(0);
        }
        true
    }

    /// Applies a `state=clear` report: removes the record `id` and
    /// every record beneath it, or every record when `id` is `None`.
    /// Returns true if the set changed.
    pub fn clear(&mut self, id: Option<&str>) -> bool {
        let before = self.records.len();
        match id {
            Some(id) => self.records.retain(|r| !r.is_within(id)),
            None => self.records.clear(),
        }
        self.records.len() != before
    }

    /// Removes just the record `id`, or the root record when `id` is
    /// `None`; records beneath it are kept.
    /// Returns true if the set changed.
    pub fn remove(&mut self, id: Option<&str>) -> bool {
        let before = self.records.len();
        self.records.retain(|r| r.id.as_deref() != id);
        self.records.len() != before
    }

    /// Drops `working` and `blocked` records.
    /// Returns true if the set changed.
    pub fn drop_transient(&mut self) -> bool {
        let before = self.records.len();
        self.records.retain(|r| !r.is_transient());
        self.records.len() != before
    }

    /// Returns the record `id`, or the root record when `id` is `None`
    pub fn get(&self, id: Option<&str>) -> Option<ProgramStatus> {
        self.records
            .iter()
            .find(|r| r.id.as_deref() == id)
            .map(|r| self.resolve(r))
    }

    /// Returns the record that most urgently needs the user's
    /// attention; of equally urgent records, the most recently
    /// updated one wins
    pub fn most_urgent(&self) -> Option<ProgramStatus> {
        self.records
            .iter()
            .max_by_key(|r| r.urgency())
            .map(|r| self.resolve(r))
    }

    /// Returns every record, the root record first and the
    /// rest ordered by id
    pub fn records(&self) -> Vec<ProgramStatus> {
        let mut records: Vec<_> = self.records.iter().map(|r| self.resolve(r)).collect();
        records.sort_by(|a, b| a.id.cmp(&b.id));
        records
    }

    /// A record without an `app` takes the `app` of its nearest
    /// ancestor that has one
    fn resolve(&self, record: &ProgramStatus) -> ProgramStatus {
        let mut record = record.clone();
        let mut ancestor = record.id.clone();
        while record.app.is_none() {
            let Some(id) = ancestor else {
                break;
            };
            ancestor = id.rfind('/').map(|idx| id[..idx].to_string());
            record.app = self
                .records
                .iter()
                .find(|r| r.id == ancestor)
                .and_then(|r| r.app.clone());
        }
        record
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
pub enum Alert {
    Bell,
    ToastNotification {
        /// The title text for the notification.
        title: Option<String>,
        /// The message body
        body: String,
        /// Whether clicking on the notification should focus the
        /// window/tab/pane that generated it
        focus: bool,
    },
    CurrentWorkingDirectoryChanged,
    IconTitleChanged(Option<String>),
    WindowTitleChanged(String),
    TabTitleChanged(Option<String>),
    /// When the color palette has been updated
    PaletteChanged,
    /// A UserVar has changed value
    SetUserVar {
        name: String,
        value: String,
    },
    /// When something bumps the seqno in the terminal model and
    /// the terminal is not focused
    OutputSinceFocusLost,
    /// A change to the progress bar state
    Progress(Progress),
    /// A change to the OSC 7501 program status records
    ProgramStatus(ProgramStatusSet),
}

pub trait AlertHandler: Send + Sync {
    fn alert(&mut self, alert: Alert);
}

pub trait DownloadHandler: Send + Sync {
    fn save_to_downloads(&self, name: Option<String>, data: Vec<u8>);
}

/// Represents an instance of a terminal emulator.
pub struct Terminal {
    /// The terminal model/state
    state: TerminalState,
    /// Baseline terminal escape sequence parser
    parser: Parser,
}

impl Deref for Terminal {
    type Target = TerminalState;

    fn deref(&self) -> &TerminalState {
        &self.state
    }
}

impl DerefMut for Terminal {
    fn deref_mut(&mut self) -> &mut TerminalState {
        &mut self.state
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, FromDynamic, ToDynamic)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
pub struct TerminalSize {
    pub rows: usize,
    pub cols: usize,
    pub pixel_width: usize,
    pub pixel_height: usize,
    pub dpi: u32,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
            dpi: 0,
        }
    }
}

impl Terminal {
    /// Construct a new Terminal.
    /// `physical_rows` and `physical_cols` describe the dimensions
    /// of the visible portion of the terminal display in terms of
    /// the number of text cells.
    ///
    /// `pixel_width` and `pixel_height` describe the dimensions of
    /// that same visible area but in pixels.
    ///
    /// `term_program` and `term_version` are required to identify
    /// the host terminal program; they are used to respond to the
    /// terminal identification sequence `\033[>q`.
    ///
    /// `writer` is anything that implements `std::io::Write`; it
    /// is used to send input to the connected program; both keyboard
    /// and mouse input is encoded and written to that stream, as
    /// are answerback responses to a number of escape sequences.
    pub fn new(
        size: TerminalSize,
        config: Arc<dyn TerminalConfiguration + Send + Sync>,
        term_program: &str,
        term_version: &str,
        // writing to the writer sends data to input of the pty
        writer: Box<dyn std::io::Write + Send>,
    ) -> Terminal {
        Terminal {
            state: TerminalState::new(size, config, term_program, term_version, writer),
            parser: Parser::new(),
        }
    }

    /// Feed the terminal parser a slice of bytes from the output
    /// of the associated program.
    /// The slice is not required to be a complete sequence of escape
    /// characters; it is valid to feed in chunks of data as they arrive.
    /// The output is parsed and applied to the terminal model.
    pub fn advance_bytes<B: AsRef<[u8]>>(&mut self, bytes: B) {
        self.state.increment_seqno();
        {
            let bytes = bytes.as_ref();

            let mut performer = Performer::new(&mut self.state);

            self.parser.parse(bytes, |action| performer.perform(action));
        }
        self.trigger_unseen_output_notif();
    }

    pub fn perform_actions(&mut self, actions: Vec<wezterm_escape_parser::Action>) {
        self.state.increment_seqno();
        {
            let mut performer = Performer::new(&mut self.state);
            for action in actions {
                performer.perform(action);
            }
        }
        self.trigger_unseen_output_notif();
    }
}
