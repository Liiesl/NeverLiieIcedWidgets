//! Synchronized editing support for multi-item rename.
//!
//! In "op mode" a [`GhostTrailTextInput`](super::GhostTrailTextInput) does not
//! edit its value itself. Instead it publishes [`EditOp`]s describing each
//! keystroke, and the application replays the same operation against every
//! item's own [`SyncState`] (its own text, caret and selection anchor). This
//! makes a single keystroke edit N inputs simultaneously, each at its own
//! cursor position — e.g. typing `a` in one input inserts `a` into every
//! input at its own caret.
//!
//! All indices are grapheme indices, matching the [`Value`] model of the
//! widget, and every operation is executed through the same
//! [`Editor`]/[`Cursor`] machinery the normal (single-input) mode uses, so
//! word jumps, selection semantics and clamping behave identically.

use super::cursor::State as CursorState;
use super::{Cursor, Editor, Value};

/// A keystroke-level edit, published by an op-mode input.
#[derive(Debug, Clone)]
pub enum EditOp {
    /// Insert text at the caret (typing or an IME commit).
    Insert {
        text: String,
    },
    /// Paste clipboard contents at the caret.
    Paste {
        text: String,
    },
    /// Delete backwards (Backspace). Deletes the selection when one exists;
    /// `word` expands the deletion to a word when no selection exists.
    DeleteBackward {
        word: bool,
    },
    /// Delete forwards (Delete). Deletes the selection when one exists;
    /// `word` expands the deletion to a word when no selection exists.
    DeleteForward {
        word: bool,
    },
    /// Move the caret by a grapheme delta (`-1`/`+1`), a word when `word`,
    /// extending the selection when `select`.
    MoveCaret {
        delta: isize,
        word: bool,
        select: bool,
    },
    /// Jump the caret to the start of the text (Home), extending the
    /// selection when `select`.
    JumpToStart {
        select: bool,
    },
    /// Jump the caret to the end of the text (End), extending the selection
    /// when `select`.
    JumpToEnd {
        select: bool,
    },
    /// Select the entire text (Ctrl+A).
    SelectAll,
    /// Place the caret at a grapheme position (mouse click). Applied only to
    /// the input that published it; `select` extends an existing selection
    /// (Shift+click).
    SetCaret {
        position: usize,
        select: bool,
    },
    /// Select the word at a grapheme position (double click). Applied only
    /// to the input that published it.
    SelectWordAt {
        position: usize,
    },
}

/// The editable state of one item in a synchronized multi-rename session.
#[derive(Debug, Clone)]
pub struct SyncState {
    /// The current text of the input.
    pub text: String,
    /// The caret position, in graphemes.
    pub caret: usize,
    /// The selection anchor, in graphemes. `None` when there is no selection
    /// (a plain caret).
    pub anchor: Option<usize>,
}

/// The caret position that marks the end of a file stem (before the
/// extension), given the stem's length in *bytes*. For directories or
/// extension-less names `stem_byte_len` equals the whole name length.
pub fn initial_caret(name: &str, stem_byte_len: usize) -> usize {
    let mut bytes = stem_byte_len.min(name.len());
    while bytes > 0 && !name.is_char_boundary(bytes) {
        bytes -= 1;
    }
    Value::new(&name[..bytes]).len()
}

/// Applies an [`EditOp`] to one [`SyncState`], reusing the same value/cursor
/// machinery as the single-input editor so behavior stays identical.
pub fn apply_op(state: &mut SyncState, op: &EditOp) {
    let mut value = Value::new(&state.text);
    let mut cursor = Cursor::default();
    match state.anchor {
        Some(anchor) => cursor.select_range(anchor, state.caret),
        None => cursor.move_to(state.caret),
    }

    match op {
        EditOp::Insert { text } | EditOp::Paste { text } => {
            let mut editor = Editor::new(&mut value, &mut cursor);
            editor.paste(Value::new(text));
        }
        EditOp::DeleteBackward { word } => {
            if *word && cursor.selection(&value).is_none() {
                cursor.select_left_by_words(&value);
            }
            let mut editor = Editor::new(&mut value, &mut cursor);
            editor.backspace();
        }
        EditOp::DeleteForward { word } => {
            if *word && cursor.selection(&value).is_none() {
                cursor.select_right_by_words(&value);
            }
            let mut editor = Editor::new(&mut value, &mut cursor);
            editor.delete();
        }
        EditOp::MoveCaret {
            delta,
            word,
            select,
        } => {
            let right = *delta > 0;
            if *select {
                match (right, *word) {
                    (true, true) => cursor.select_right_by_words(&value),
                    (true, false) => cursor.select_right(&value),
                    (false, true) => cursor.select_left_by_words(&value),
                    (false, false) => cursor.select_left(&value),
                }
            } else {
                match (right, *word) {
                    (true, true) => cursor.move_right_by_words(&value),
                    (true, false) => cursor.move_right(&value),
                    (false, true) => cursor.move_left_by_words(&value),
                    (false, false) => cursor.move_left(&value),
                }
            }
        }
        EditOp::JumpToStart { select } => {
            if *select {
                cursor.select_range(cursor.start(&value), 0);
            } else {
                cursor.move_to(0);
            }
        }
        EditOp::JumpToEnd { select } => {
            if *select {
                cursor.select_range(cursor.start(&value), value.len());
            } else {
                cursor.move_to(value.len());
            }
        }
        EditOp::SelectAll => cursor.select_all(&value),
        EditOp::SetCaret { position, select } => {
            let position = (*position).min(value.len());
            if *select {
                cursor.select_range(cursor.start(&value), position);
            } else {
                cursor.move_to(position);
            }
        }
        EditOp::SelectWordAt { position } => {
            let position = (*position).min(value.len());
            cursor.select_range(
                value.previous_start_of_word(position),
                value.next_end_of_word(position),
            );
        }
    }

    state.text = value.to_string();
    match cursor.state(&value) {
        CursorState::Index(index) => {
            state.caret = index;
            state.anchor = None;
        }
        CursorState::Selection { start, end } => {
            state.caret = end;
            state.anchor = Some(start);
        }
    }
}
