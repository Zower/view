use std::{fmt::Display, path::PathBuf};

use crop::RopeSlice;

use miette::IntoDiagnostic;
use strum::EnumString;
use tree_sitter::Tree;

pub mod buffer;

use crate::{
    lsp::{LspRequest, LspRequestData, LspResponseTransmitter},
    ts::{
        self,
        highlight::{self, LineHighlights},
    },
};

pub use self::buffer::SimpleBuffer;

// #[derive(Debug)]
// pub struct Editor {
//     workspaces: SlotMap<WorkspaceId, Workspace>,
//     active: BufferId,
//     active_workspace: WorkspaceId,
//     // config: Config,
//     // pub mode: Mode,
//     // pub mods: crate::input::Modifiers,
// }
#[derive(Debug)]
pub struct Buffer {
    lsp: Option<lsp::Lsp>,
    tree: Option<Tree>,
    pub buffer: SimpleBuffer,
}

impl Buffer {
    fn new(buffer: SimpleBuffer, lsp: Option<lsp::Lsp>) -> Self {
        let tree = ts::tree(&buffer.rope, None);

        Self {
            lsp,
            tree: Some(tree),
            buffer,
        }
    }

    pub fn create(
        buffer: SimpleBuffer,
        workspace: PathBuf,
        receiver: impl LspResponseTransmitter,
    ) -> crate::Result<Self> {
        let workspace = workspace.canonicalize().into_diagnostic()?;

        let lsp = if buffer
            .path()
            .to_str()
            .map(|it| it.contains(".rs"))
            .unwrap_or(false)
        {
            Some(lsp::Lsp::new(
                workspace,
                buffer.path().to_owned(),
                receiver,
            )?)
        } else {
            None
        };

        Ok(Self::new(buffer, lsp))
    }

    pub fn text(&self) -> String {
        self.buffer.text()
    }

    pub fn line_len(&self) -> usize {
        self.buffer.line_len()
    }

    pub fn line(&self, idx: usize) -> RopeSlice {
        self.buffer.line(idx)
    }

    pub fn cursor(&self) -> Cursor {
        self.buffer.cursor()
    }

    pub(super) fn back(&mut self) -> Option<Edit> {
        let edit = self.buffer.back()?;

        self.tree_refresh(edit);
        self.lsp_for_edit(edit, String::new());

        Some(edit)
    }

    fn lsp_for_edit(&mut self, edit: Edit, text: String) {
        match edit {
            Edit::Insert { start, .. } => {
                let range = lsp_types::Range {
                    start: lsp_types::Position {
                        line: start.line as u32,
                        character: start.character as u32,
                    },
                    end: lsp_types::Position {
                        line: start.line as u32,
                        character: start.character as u32,
                    },
                };

                self.lsp_event(LspRequestData::DidChange {
                    edit: crate::lsp::LspEdit { range, text },
                });
            }
            Edit::Delete { from, to, .. } => {
                let range = lsp_types::Range {
                    start: lsp_types::Position {
                        line: from.line as u32,
                        character: from.character as u32,
                    },
                    end: lsp_types::Position {
                        line: to.line as u32,
                        character: to.character as u32,
                    },
                };

                self.lsp_event(LspRequestData::DidChange {
                    edit: crate::lsp::LspEdit {
                        range,
                        text: String::new(),
                    },
                });
            }
        }
    }

    pub(super) fn cursor_up(&mut self) {
        self.buffer.cursor_up()
    }

    pub(super) fn cursor_right(&mut self) {
        self.buffer.cursor_right()
    }

    pub(super) fn cursor_down(&mut self) {
        self.buffer.cursor_down()
    }

    pub(super) fn cursor_left(&mut self) {
        self.buffer.cursor_left()
    }

    pub(super) fn insert(&mut self, str: impl AsRef<str>) -> Edit {
        let str = str.as_ref();
        let text = str.to_string();
        let edit = self.buffer.insert(str);

        self.tree_refresh(edit);

        self.lsp_for_edit(edit, text);

        edit
    }

    pub(super) fn line_current_char_idx(&self) -> usize {
        self.buffer.line_current_char_idx()
    }

    fn tree_refresh(&mut self, edit: Edit) {
        let Some(tree) = &mut self.tree else {
            return;
        };

        tree.edit(&edit.to_ts());
        *tree = ts::tree(&self.buffer.rope, Some(tree));
    }

    fn lsp_event(&self, event: LspRequestData) {
        let Some(lsp) = &self.lsp else { return };
        lsp.send(LspRequest {
            file: self.buffer.path.clone(),
            data: event,
        });
    }

    pub fn highlight<'query, 'sel, 'tree>(
        &'sel self,
        cursor: &'query mut tree_sitter::QueryCursor,
        query: &'query tree_sitter::Query,
        range: std::ops::Range<usize>,
    ) -> LineHighlights<'query, 'tree, 'sel>
    where
        'tree: 'query,
        'sel: 'tree,
    {
        highlight::syntax_highlight(
            self.tree.as_ref().unwrap(),
            cursor,
            query,
            &self.buffer.rope,
            range,
        )
    }
}

pub fn action(buffer: &mut Buffer, action: Action) {
    match action {
        Action::Up => buffer.cursor_up(),
        Action::Down => buffer.cursor_down(),
        Action::Left => buffer.cursor_left(),
        Action::Right => buffer.cursor_right(),
        // Action::InsertMode => self.mode = Mode::Insert,
        // Action::NormalMode => self.mode = Mode::Normal,
        Action::Hover => {
            let event = LspRequestData::Hover {
                line: buffer.cursor().line as u32,
                character: buffer.line_current_char_idx() as u32,
            };

            buffer.lsp_event(event)
        }
        Action::Complete => {
            let event = LspRequestData::Completion {
                line: buffer.cursor().line as u32,
                character: buffer.line_current_char_idx() as u32,
            };

            buffer.lsp_event(event)
        }
        Action::Back => {
            buffer.back();
        }
        Action::NewLine => {
            buffer.insert("\n");
        }
        _ => todo!(),
    }
}

#[derive(Debug, Clone, Copy, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Back,
    InsertMode,
    NormalMode,
    NewLine,
    Hover,
    Complete,
}

#[derive(Debug, Copy, Clone)]
pub struct Cursor {
    pub byte: usize,
    pub line: usize,
}

#[derive(Debug, Copy, Clone)]
pub struct CursorWithCharacter {
    pub byte: usize,
    pub character: usize,
    pub line: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self { byte: 0, line: 0 }
    }

    pub fn from_line_byte(line: usize, byte: usize) -> Self {
        Self { byte, line }
    }

    pub fn with_character(self, character: usize) -> CursorWithCharacter {
        CursorWithCharacter {
            byte: self.byte,
            character,
            line: self.line,
        }
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Mode {
    Normal,
    Insert,
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Mode::Normal => "Normal",
            Mode::Insert => "Debug",
        };

        write!(f, "{text}")
    }
}

#[derive(Clone, Copy, Debug)]

/// Represents an edit.
pub enum Edit {
    Insert {
        start: CursorWithCharacter,
        start_byte: usize,
        new_end: CursorWithCharacter,
        new_end_byte: usize,
    },
    Delete {
        from: CursorWithCharacter,
        from_byte: usize,
        to: CursorWithCharacter,
        to_byte: usize,
    },
}

impl Edit {
    fn to_ts(self) -> tree_sitter::InputEdit {
        match self {
            Edit::Insert {
                start,
                new_end,
                start_byte,
                new_end_byte,
            } => tree_sitter::InputEdit {
                start_byte,
                old_end_byte: start_byte,
                new_end_byte,
                start_position: start.into(),
                old_end_position: start.into(),
                new_end_position: new_end.into(),
            },
            Edit::Delete {
                from,
                to,
                from_byte,
                to_byte,
            } => tree_sitter::InputEdit {
                start_byte: from_byte,
                old_end_byte: to_byte,
                new_end_byte: from_byte,
                start_position: from.into(),
                old_end_position: to.into(),
                new_end_position: from.into(),
            },
        }
    }
}

mod workspace {
    use std::path::PathBuf;

    use slotmap::new_key_type;

    use crate::lsp::LspResponseTransmitter;

    new_key_type! {
        pub struct WorkspaceId;
    }

    #[derive(Debug)]
    pub(super) struct Workspace {
        pub(super) id: WorkspaceId,
        pub(super) path: PathBuf,
        pub(super) lsp: Option<super::lsp::Lsp>,
        // pub(super) buffers: Vec<BufferId>,
    }

    impl Workspace {
        pub fn new(
            id: WorkspaceId,
            path: PathBuf,
            initial_file: PathBuf,
            sync: impl LspResponseTransmitter,
        ) -> Self {
            let lsp = { super::lsp::Lsp::new(path.clone(), initial_file, sync).ok() };

            Self {
                id,
                path,
                lsp,
                // buffers: Vec::new(),
            }
        }
    }
}

mod lsp {
    use crate::lsp::{LspRequest, LspResponseTransmitter};
    use std::{
        path::PathBuf,
        sync::mpsc::{channel, Sender},
    };

    #[derive(Debug, Clone)]
    pub(super) struct Lsp {
        sender: Sender<LspRequest>,
    }

    impl Lsp {
        pub(super) fn new<T: LspResponseTransmitter>(
            workspace: PathBuf,
            file: PathBuf,
            sync: T,
        ) -> crate::Result<Self> {
            let (tx, rx) = channel();

            crate::lsp::Lsp::run(rx, sync, workspace, file);

            Ok(Self { sender: tx })
        }

        pub fn send(&self, event: LspRequest) {
            self.sender.send(event).expect("Channel to be open");
        }
    }
}

impl From<Cursor> for tree_sitter::Point {
    fn from(value: Cursor) -> Self {
        Self {
            row: value.byte,
            column: value.line,
        }
    }
}

impl From<CursorWithCharacter> for tree_sitter::Point {
    fn from(value: CursorWithCharacter) -> Self {
        Self {
            row: value.byte,
            column: value.line,
        }
    }
}

impl From<CursorWithCharacter> for Cursor {
    fn from(value: CursorWithCharacter) -> Self {
        Self {
            byte: value.byte,
            line: value.line,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {}
}
