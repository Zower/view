use std::path::{Path, PathBuf};

use crop::{Rope, RopeSlice};
use miette::IntoDiagnostic;

use super::{Cursor, CursorWithCharacter, Edit};

#[derive(Clone, Debug)]
pub struct SimpleBuffer {
    pub path: PathBuf,
    pub(super) rope: Rope,
    pub(super) cursor: Cursor,
}

impl SimpleBuffer {
    pub fn open(path: PathBuf) -> crate::Result<Self> {
        let str = std::fs::read_to_string(&path).into_diagnostic()?;
        let rope = Rope::from(str);

        Ok(Self {
            rope,
            cursor: Cursor::new(),
            path,
        })
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn insert(&mut self, text: impl AsRef<str>) -> Edit {
        let start = self.cursor.with_character(self.line_current_char_idx());
        let start_byte = self.global_cursor_to_byte();

        let text = text.as_ref();

        let len = text.len();

        if !text.is_empty() {
            self.rope.insert(start_byte, text);

            let new_lines = text.split('\n').count() - 1;

            if new_lines > 0 {
                self.cursor.line += new_lines;
                self.cursor.byte = text.lines().last().map(|line| line.len()).unwrap_or(0);
            } else {
                self.cursor.byte += len;
            }
        }

        Edit::Insert {
            start,
            start_byte,
            new_end: self.cursor.with_character(self.line_current_char_idx()),
            new_end_byte: self.global_cursor_to_byte(),
        }
    }

    pub(super) fn back(&mut self) -> Option<Edit> {
        if self.cursor.line == 0 && self.cursor.byte == 0 {
            return None;
        }

        if self.cursor.byte == 0 {
            self.cursor_line_up();

            let from = self.cursor_with_character();
            let from_byte = self.global_cursor_to_byte();

            let to_byte = self.global_cursor_to_byte() + 1;

            let to = CursorWithCharacter {
                byte: 0,
                character: 0,
                line: self.cursor.line + 1,
            };

            self.rope.delete(from_byte..to_byte);

            return Some(Edit::Delete {
                from,
                from_byte,
                to,
                to_byte,
            });
        }

        let start = self
            .global_prev_char_index()
            .expect("A previous char index");

        let end = self.global_cursor_to_byte();

        let to = self.cursor_with_character();

        let range = start..end;

        self.rope.delete(range.clone());

        if self.cursor.byte == 0 {
            self.cursor_line_up()
        } else {
            self.cursor.byte = self.cursor.byte.saturating_sub(range.len());
        }

        let from = self.cursor.with_character(self.line_current_char_idx());

        Some(Edit::Delete {
            from,
            to,
            from_byte: start,
            to_byte: end,
        })
    }

    fn cursor_with_character(&self) -> super::CursorWithCharacter {
        self.cursor.with_character(self.line_current_char_idx())
    }

    pub(super) fn cursor_line_up(&mut self) {
        assert!(self.cursor.line > 0);
        assert!(self.cursor.byte == 0);

        self.cursor.line = self.cursor.line.saturating_sub(1);
        self.cursor.byte = self.current_line().byte_len();
    }

    pub(super) fn line_prev_char_index(&self) -> Option<usize> {
        if self.cursor.byte == 0 {
            return None;
        }

        let line = self.current_line();
        let mut row = self.cursor.byte;

        loop {
            debug_assert!(row >= 1);
            row -= 1;

            if line.is_char_boundary(row) {
                return Some(row);
            }
        }
    }

    pub(super) fn global_prev_char_index(&self) -> Option<usize> {
        self.line_prev_char_index()
            .map(|local| self.line_byte_to_global(self.cursor.line, local))
    }

    pub(super) fn line_next_char_index(&self) -> Option<usize> {
        let line = self.current_line();

        if self.cursor.byte == line.byte_len() {
            return None;
        }

        let mut row = self.cursor.byte;

        loop {
            debug_assert!(row <= line.byte_len());
            row += 1;

            if line.is_char_boundary(row) {
                return Some(row);
            }
        }
    }

    pub(super) fn global_next_char_index(&self) -> Option<usize> {
        self.line_next_char_index()
            .map(|local| self.line_byte_to_global(self.cursor.line, local))
    }

    pub(super) fn global_cursor_to_byte(&self) -> usize {
        self.line_byte_to_global(self.cursor.line, self.cursor.byte)
    }

    pub(super) fn line_byte_to_global(&self, line: usize, row: usize) -> usize {
        self.rope.byte_of_line(line) + row
    }

    pub(super) fn current_line_start_byte(&self) -> usize {
        self.rope.byte_of_line(self.cursor.line)
    }

    pub(super) fn clamp_cursor_max(&mut self, max: usize) {
        self.cursor.byte = self.cursor.byte.clamp(0, max);
    }

    pub(super) fn cursor_left(&mut self) {
        if self.cursor.byte == 0 {
            return;
        }

        self.cursor.byte = self
            .global_prev_char_index()
            .unwrap()
            .saturating_sub(self.current_line_start_byte());
    }

    pub(super) fn cursor_down(&mut self) {
        self.cursor.line = self
            .cursor
            .line
            .saturating_add(1)
            .min(self.rope.line_len().saturating_sub(1));

        self.clamp_cursor_max(self.current_line().byte_len());

        if !self.current_line().is_char_boundary(self.cursor.byte) {
            self.cursor.byte = self.line_prev_char_index().unwrap_or(0);
        }
    }

    pub(super) fn cursor_up(&mut self) {
        self.cursor.line = self.cursor.line.saturating_sub(1);

        self.clamp_cursor_max(self.current_line().byte_len());

        if !self.current_line().is_char_boundary(self.cursor.byte) {
            self.cursor.byte = self.line_prev_char_index().unwrap_or(0);
        }
    }

    pub(super) fn cursor_right(&mut self) {
        if let Some(next) = self.global_next_char_index() {
            self.cursor.byte = next - self.current_line_start_byte();
        }
    }

    pub(super) fn line_char_idx(&self, cursor: Cursor) -> usize {
        line_char_idx(&self.rope, cursor)
    }

    pub(super) fn line_current_char_idx(&self) -> usize {
        self.line_char_idx(self.cursor)
    }

    pub(super) fn current_line(&self) -> RopeSlice {
        self.rope.line(self.cursor.line)
    }

    pub fn current_char(&self) -> Option<char> {
        let line = self.current_line();

        line.byte_slice(self.cursor.byte..).chars().next()
    }

    pub(crate) fn line_len(&self) -> usize {
        self.rope.line_len()
    }

    pub(crate) fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub(crate) fn line(&self, idx: usize) -> RopeSlice {
        self.rope.line(idx)
    }
}

pub(super) fn line_char_idx(rope: &Rope, cursor: Cursor) -> usize {
    let line = rope.line(cursor.line);

    if cursor.byte == 0 {
        return 0;
    }

    let mut idx = 0;
    let mut length = 0;

    for char in line.chars() {
        idx += 1;

        length += char.len_utf8();

        if length == cursor.byte {
            return idx;
        }
    }

    panic!(
        "cursor is not on char boundary l: {} byte: {}, text: {}",
        cursor.line,
        cursor.byte,
        rope.line(cursor.line)
    );
}
