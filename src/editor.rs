use std::{
  collections::{HashMap, VecDeque},
  ops::Range,
  sync::Arc,
  time::Instant,
};

use crate::{
  buffer::TransactionId,
  document::Document,
  editor_element::{EditorElement, PositionMap},
};
use gpui::{
  App, Bounds, ClipboardItem, Context, CursorStyle, Entity, EntityInputHandler, FocusHandle,
  Focusable, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ShapedLine,
  UTF16Selection, Window, actions, black, div, prelude::*, px, white,
};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug)]
struct Transaction {
  id: TransactionId,
  selection_before: Range<usize>,
  selection_after: Range<usize>,
}

// Default viewport height before first render
const DEFAULT_VIEWPORT_HEIGHT: f32 = 800.0;
// Maximum number of cached shaped lines
const MAX_CACHE_SIZE: usize = 200;
// Number of lines of padding when auto-scrolling to cursor
const SCROLL_PADDING: usize = 3;

actions!(
  editor,
  [
    Enter,
    Backspace,
    BackspaceWord,
    BackspaceAll,
    Delete,
    Up,
    Down,
    Left,
    AltLeft,
    CmdLeft,
    Right,
    CmdRight,
    AltRight,
    CmdUp,
    CmdDown,
    SelectUp,
    SelectDown,
    SelectLeft,
    SelectRight,
    SelectCmdLeft,
    SelectCmdRight,
    SelectCmdUp,
    SelectCmdDown,
    SelectWordLeft,
    SelectWordRight,
    SelectAll,
    Home,
    End,
    ShowCharacterPalette,
    Paste,
    Cut,
    Copy,
    Undo,
    Redo,
    Quit,
  ]
);

pub struct Editor {
  pub document: Entity<Document>,
  pub focus_handle: FocusHandle,
  pub selected_range: Range<usize>,
  pub selection_reversed: bool,
  pub marked_range: Option<Range<usize>>,
  pub is_selecting: bool,

  // Performance: cache and viewport
  pub line_layouts: HashMap<usize, Arc<ShapedLine>>,
  pub scroll_offset: f32, // In lines (0.0 = top)
  pub viewport_height: Pixels,

  // Cache size limit to prevent memory issues with large files
  max_cache_size: usize,

  // Target column for vertical navigation
  target_column: Option<usize>,

  undo_stack: VecDeque<Transaction>,
  redo_stack: VecDeque<Transaction>,

  is_dark_mode: bool,
}

impl Editor {
  pub fn new(cx: &mut Context<Self>) -> Self {
    // Create a test document with 100k lines for performance testing
    let mut content = String::new();
    for i in 0..100_000 {
      content.push_str(&format!(
        "Line {} - This is some test content to see how the editor performs with many lines\n",
        i + 1
      ));
    }

    let document = cx.new(|cx| Document::with_text(&content, cx));

    Self {
      document,
      focus_handle: cx.focus_handle(),
      selected_range: 0..0,
      selection_reversed: false,
      marked_range: None,
      is_selecting: false,
      line_layouts: HashMap::new(),
      scroll_offset: 0.0,
      viewport_height: px(DEFAULT_VIEWPORT_HEIGHT), // Will be updated from actual bounds
      max_cache_size: MAX_CACHE_SIZE,
      target_column: None,
      undo_stack: VecDeque::new(),
      redo_stack: VecDeque::new(),
      is_dark_mode: true,
    }
  }

  pub fn document(&self) -> &Entity<Document> {
    &self.document
  }

  /// Invalidate a single line in the cache
  fn invalidate_line(&mut self, line: usize) {
    self.line_layouts.remove(&line);
  }

  /// Invalidate all lines from start_line onwards (for multi-line edits)
  fn invalidate_lines_from(&mut self, start_line: usize) {
    self
      .line_layouts
      .retain(|&line_idx, _| line_idx < start_line);
  }

  pub fn ensure_cache_size(&mut self, viewport: Range<usize>) {
    // If cache is too large, keep only lines near the viewport
    if self.line_layouts.len() > self.max_cache_size {
      let viewport_start = viewport.start.saturating_sub(50);
      let viewport_end = viewport.end + 50;

      self
        .line_layouts
        .retain(|&line_idx, _| line_idx >= viewport_start && line_idx < viewport_end);
    }
  }

  fn ensure_cursor_visible(&mut self, window: &Window, cx: &mut Context<Self>) {
    let document = self.document.read(cx);
    let cursor_offset = self.cursor_offset();
    let cursor_line = document.char_to_line(cursor_offset);
    let total_lines = document.len_lines();

    // Calculate how many lines are visible in the viewport
    let line_height = window.line_height();
    let visible_lines = (self.viewport_height / line_height).floor() as usize;

    // Offset for context padding when scrolling
    let scroll_padding = SCROLL_PADDING;

    // Calculate the visible range with padding
    let scroll_start = self.scroll_offset as usize;
    let scroll_end = scroll_start + visible_lines;

    // Ensure cursor is within the visible range with padding
    if cursor_line < scroll_start + scroll_padding {
      // Cursor is too close to top, scroll up
      self.scroll_offset = (cursor_line.saturating_sub(scroll_padding)) as f32;
    } else if cursor_line >= scroll_end.saturating_sub(scroll_padding) {
      // Cursor is too close to bottom, scroll down
      let target_line = cursor_line + scroll_padding;
      self.scroll_offset = (target_line as f32 - visible_lines as f32 + 1.0).max(0.0);
    }

    // Clamp scroll_offset to valid range
    let max_scroll = (total_lines as f32 - visible_lines as f32).max(0.0);
    self.scroll_offset = self.scroll_offset.max(0.0).min(max_scroll);
  }

  fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    let cursor = self.cursor_offset();
    let current_line = self.document.read(cx).char_to_line(cursor);
    let selection_before = self.selected_range.clone();

    let transaction_id = self.document.update(cx, |doc, cx| {
      let id = doc.buffer.transaction(Instant::now(), |buffer, tx| {
        buffer.insert(tx, cursor, "\n");
      });
      cx.notify();
      id
    });

    self.move_to(cursor + 1, cx);
    let selection_after = self.selected_range.clone();

    self.record_transaction(transaction_id, selection_before, selection_after);

    self.invalidate_lines_from(current_line);

    self.ensure_cursor_visible(window, cx);
  }

  fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
    let new_cursor = {
      let document = self.document.read(cx);
      let cursor_offset = self.cursor_offset();
      let current_line = document.char_to_line(cursor_offset);

      if current_line > 0 {
        if self.target_column.is_none() {
          let line_start = document.line_to_char(current_line);
          self.target_column = Some(cursor_offset - line_start);
        }

        let target_column = self.target_column.unwrap();

        // Calculate new position in target line
        let target_line = current_line - 1;
        let target_start = document.line_to_char(target_line);
        let target_len = document.line_content(target_line).unwrap_or_default().len();

        Some(target_start + target_column.min(target_len))
      } else {
        // On first line, go to beginning of buffer
        self.target_column = None;
        Some(0)
      }
    };

    if let Some(cursor) = new_cursor {
      self.move_to(cursor, cx);
      self.ensure_cursor_visible(window, cx);
    }
  }

  fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
    let new_cursor = {
      let document = self.document.read(cx);
      let cursor_offset = self.cursor_offset();
      let current_line = document.char_to_line(cursor_offset);

      if current_line < document.len_lines().saturating_sub(1) {
        if self.target_column.is_none() {
          let line_start = document.line_to_char(current_line);
          self.target_column = Some(cursor_offset - line_start);
        }

        let target_column = self.target_column.unwrap();

        let target_line = current_line + 1;
        let target_start = document.line_to_char(target_line);
        let target_len = document.line_content(target_line).unwrap_or_default().len();

        Some(target_start + target_column.min(target_len))
      } else {
        self.target_column = None;
        Some(document.len())
      }
    };

    if let Some(cursor) = new_cursor {
      self.move_to(cursor, cx);
      self.ensure_cursor_visible(window, cx);
    }
  }

  fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    if self.selected_range.is_empty() {
      self.move_to(self.previous_boundary(self.cursor_offset(), cx), cx);
    } else {
      self.move_to(self.selected_range.start, cx)
    }
  }

  fn alt_left(&mut self, _: &AltLeft, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    if self.selected_range.is_empty() {
      self.move_to(self.previous_word_boundary(self.cursor_offset(), cx), cx);
    } else {
      self.move_to(self.selected_range.start, cx)
    }
  }

  fn cmd_left(&mut self, _: &CmdLeft, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    let document = self.document.read(cx);
    let cursor = self.cursor_offset();
    let line = document.char_to_line(cursor);
    let line_start = document.line_to_char(line);
    self.move_to(line_start, cx);
  }

  fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    if self.selected_range.is_empty() {
      self.move_to(self.next_boundary(self.selected_range.end, cx), cx);
    } else {
      self.move_to(self.selected_range.end, cx)
    }
  }

  fn alt_right(&mut self, _: &AltRight, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    if self.selected_range.is_empty() {
      self.move_to(self.next_word_boundary(self.selected_range.end, cx), cx);
    } else {
      self.move_to(self.selected_range.end, cx)
    }
  }

  fn cmd_right(&mut self, _: &CmdRight, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    let document = self.document.read(cx);
    let cursor = self.cursor_offset();
    let line = document.char_to_line(cursor);
    let line_range = document.line_range(line).unwrap_or(0..0);
    // Go to end of line content (before the newline)
    let line_content = document.line_content(line).unwrap_or_default();
    let line_end = line_range.start + line_content.len();
    self.move_to(line_end, cx);
  }

  fn cmd_up(&mut self, _: &CmdUp, window: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None; // Reset target column on jump
    self.move_to(0, cx);
    self.ensure_cursor_visible(window, cx);
  }

  fn cmd_down(&mut self, _: &CmdDown, window: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None; // Reset target column on jump
    let document = self.document.read(cx);
    self.move_to(document.len(), cx);
    self.ensure_cursor_visible(window, cx);
  }

  fn select_cmd_left(&mut self, _: &SelectCmdLeft, _: &mut Window, cx: &mut Context<Self>) {
    let document = self.document.read(cx);
    let cursor = self.cursor_offset();
    let line = document.char_to_line(cursor);
    let line_start = document.line_to_char(line);
    self.select_to(line_start, cx);
  }

  fn select_cmd_right(&mut self, _: &SelectCmdRight, _: &mut Window, cx: &mut Context<Self>) {
    let document = self.document.read(cx);
    let cursor = self.cursor_offset();
    let line = document.char_to_line(cursor);
    let line_range = document.line_range(line).unwrap_or(0..0);
    let line_content = document.line_content(line).unwrap_or_default();
    let line_end = line_range.start + line_content.len();
    self.select_to(line_end, cx);
  }

  fn select_cmd_up(&mut self, _: &SelectCmdUp, window: &mut Window, cx: &mut Context<Self>) {
    self.select_to(0, cx);
    self.ensure_cursor_visible(window, cx);
  }

  fn select_cmd_down(&mut self, _: &SelectCmdDown, window: &mut Window, cx: &mut Context<Self>) {
    let document = self.document.read(cx);
    self.select_to(document.len(), cx);
    self.ensure_cursor_visible(window, cx);
  }

  fn select_up(&mut self, _: &SelectUp, window: &mut Window, cx: &mut Context<Self>) {
    // Keep the anchor point of the selection
    let anchor = if self.selection_reversed {
      self.selected_range.end
    } else {
      self.selected_range.start
    };

    // Calculate new cursor position (same logic as up())
    let new_cursor = {
      let document = self.document.read(cx);
      let cursor_offset = self.cursor_offset();
      let current_line = document.char_to_line(cursor_offset);

      if current_line > 0 {
        if self.target_column.is_none() {
          let line_start = document.line_to_char(current_line);
          self.target_column = Some(cursor_offset - line_start);
        }

        let target_column = self.target_column.unwrap();

        let target_line = current_line - 1;
        let target_start = document.line_to_char(target_line);
        let target_len = document.line_content(target_line).unwrap_or_default().len();

        Some(target_start + target_column.min(target_len))
      } else {
        self.target_column = None;
        Some(0)
      }
    };

    // Move cursor and extend selection
    let cursor = new_cursor.unwrap();
    if anchor <= cursor {
      self.selected_range = anchor..cursor;
      self.selection_reversed = false;
    } else {
      self.selected_range = cursor..anchor;
      self.selection_reversed = true;
    }
    self.ensure_cursor_visible(window, cx);
    cx.notify();
  }

  fn select_down(&mut self, _: &SelectDown, window: &mut Window, cx: &mut Context<Self>) {
    // Keep the anchor point of the selection
    let anchor = if self.selection_reversed {
      self.selected_range.end
    } else {
      self.selected_range.start
    };

    // Calculate new cursor position (same logic as down())
    let new_cursor = {
      let document = self.document.read(cx);
      let cursor_offset = self.cursor_offset();
      let current_line = document.char_to_line(cursor_offset);
      let total_lines = document.len_lines();

      if current_line + 1 < total_lines {
        if self.target_column.is_none() {
          let line_start = document.line_to_char(current_line);
          self.target_column = Some(cursor_offset - line_start);
        }

        let target_column = self.target_column.unwrap();

        let target_line = current_line + 1;
        let target_start = document.line_to_char(target_line);
        let target_len = document.line_content(target_line).unwrap_or_default().len();

        Some(target_start + target_column.min(target_len))
      } else {
        // On last line, go to end of buffer
        self.target_column = None;
        Some(document.len())
      }
    };

    // Move cursor and extend selection
    let cursor = new_cursor.unwrap();
    if anchor <= cursor {
      self.selected_range = anchor..cursor;
      self.selection_reversed = false;
    } else {
      self.selected_range = cursor..anchor;
      self.selection_reversed = true;
    }
    self.ensure_cursor_visible(window, cx);
    cx.notify();
  }

  fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    self.select_to(self.previous_boundary(self.cursor_offset(), cx), cx);
  }

  fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    self.select_to(self.previous_word_boundary(self.cursor_offset(), cx), cx);
  }

  fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    self.select_to(self.next_boundary(self.cursor_offset(), cx), cx);
  }

  fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    self.select_to(self.next_word_boundary(self.cursor_offset(), cx), cx);
  }

  fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    let doc_len = self.document.read(cx).len();

    self.move_to(0, cx);
    self.select_to(doc_len, cx);
  }

  fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    self.move_to(0, cx);
  }

  fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    let doc_len = self.document.read(cx).len();
    self.move_to(doc_len, cx);
  }

  fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    if self.selected_range.is_empty() {
      self.select_to(self.previous_boundary(self.cursor_offset(), cx), cx)
    }
    self.replace_text_in_range(None, "", window, cx)
  }

  fn backspace_word(&mut self, _: &BackspaceWord, window: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    if self.selected_range.is_empty() {
      let document = self.document.read(cx);
      let cursor = self.cursor_offset();
      let line = document.char_to_line(cursor);
      let line_start = document.line_to_char(line);

      // If we're at the beginning of an empty line, behave like simple backspace
      if cursor == line_start && document.line_content(line).unwrap_or_default().is_empty() {
        self.select_to(self.previous_boundary(cursor, cx), cx);
      } else {
        self.select_to(self.previous_word_boundary(cursor, cx), cx);
      }
    }
    self.replace_text_in_range(None, "", window, cx)
  }

  fn backspace_all(&mut self, _: &BackspaceAll, window: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    if self.selected_range.is_empty() {
      let document = self.document.read(cx);
      let cursor = self.cursor_offset();
      let line = document.char_to_line(cursor);
      let line_start = document.line_to_char(line);

      // If we're at the beginning of an empty line, behave like simple backspace
      if cursor == line_start && document.line_content(line).unwrap_or_default().is_empty() {
        self.select_to(self.previous_boundary(cursor, cx), cx);
      } else {
        // Delete from start of current line to cursor
        self.select_to(line_start, cx);
      }
    }
    self.replace_text_in_range(None, "", window, cx)
  }

  fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    if self.selected_range.is_empty() {
      self.select_to(self.next_boundary(self.cursor_offset(), cx), cx)
    }
    self.replace_text_in_range(None, "", window, cx)
  }

  pub fn mouse_left_down(
    &mut self,
    event: &MouseDownEvent,
    position_map: &PositionMap,
    _window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.target_column = None;
    self.is_selecting = true;

    let document = self.document.read(cx);
    let Some(offset) = position_map.point_for_position(event.position, document) else {
      return;
    };

    if event.modifiers.shift {
      self.select_to(offset, cx);
    } else {
      self.move_to(offset, cx);
    }
  }

  pub fn mouse_left_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
    self.is_selecting = false;
  }

  pub fn mouse_dragged(
    &mut self,
    event: &MouseMoveEvent,
    position_map: &PositionMap,
    _: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if !self.is_selecting {
      return;
    }

    let document = self.document.read(cx);
    let Some(offset) = position_map.point_for_position(event.position, document) else {
      return;
    };

    self.select_to(offset, cx);
  }

  fn show_character_palette(
    &mut self,
    _: &ShowCharacterPalette,
    window: &mut Window,
    _: &mut Context<Self>,
  ) {
    window.show_character_palette();
  }

  fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
      let cursor = self.cursor_offset();
      let current_line = self.document.read(cx).char_to_line(cursor);
      self.replace_text_in_range(None, &text, window, cx);
      // Invalidate cache from current line onwards since paste may add multiple lines
      self.invalidate_lines_from(current_line);
    }
  }

  fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
    if !self.selected_range.is_empty() {
      cx.write_to_clipboard(ClipboardItem::new_string(
        self
          .document
          .read(cx)
          .slice_to_string(self.selected_range.clone()),
      ));
    }
  }

  fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
    self.target_column = None;
    if !self.selected_range.is_empty() {
      let cursor = self.cursor_offset();
      let current_line = self.document.read(cx).char_to_line(cursor);
      cx.write_to_clipboard(ClipboardItem::new_string(
        self
          .document
          .read(cx)
          .slice_to_string(self.selected_range.clone()),
      ));
      self.replace_text_in_range(None, "", window, cx);
      // Invalidate cache from current line onwards since cut may affect multiple lines
      self.invalidate_lines_from(current_line);
    }
  }

  fn undo(&mut self, _: &Undo, _window: &mut Window, cx: &mut Context<Self>) {
    if let Some(transaction) = self.undo_stack.pop_back() {
      let buffer_tx_id = self.document.update(cx, |doc, cx| doc.undo(cx));

      // Only restore selection if buffer undo succeeded
      if buffer_tx_id.is_some() {
        // Restore cursor position from before the transaction
        self.selected_range = transaction.selection_before.clone();
        self.selection_reversed = false;

        // Invalidate cache (content may have changed significantly)
        self.line_layouts.clear();

        // Move transaction to redo stack
        self.redo_stack.push_back(transaction);

        cx.notify();
      } else {
        // Buffer undo failed, push transaction back
        self.undo_stack.push_back(transaction);
      }
    }
  }

  fn redo(&mut self, _: &Redo, _window: &mut Window, cx: &mut Context<Self>) {
    if let Some(transaction) = self.redo_stack.pop_back() {
      let buffer_tx_id = self.document.update(cx, |doc, cx| doc.redo(cx));

      // Only restore selection if buffer redo succeeded
      if buffer_tx_id.is_some() {
        // Restore cursor position from after the transaction
        self.selected_range = transaction.selection_after.clone();
        self.selection_reversed = false;

        // Invalidate cache
        self.line_layouts.clear();

        // Move transaction to undo stack
        self.undo_stack.push_back(transaction);

        cx.notify();
      } else {
        // Buffer redo failed, push transaction back
        self.redo_stack.push_back(transaction);
      }
    }
  }

  fn record_transaction(
    &mut self,
    id: TransactionId,
    selection_before: Range<usize>,
    selection_after: Range<usize>,
  ) {
    // Check if we should update an existing transaction with the same ID (grouping)
    if let Some(transaction) = self.undo_stack.iter_mut().find(|t| t.id == id) {
      transaction.selection_after = selection_after;
    } else {
      // Create new transaction
      self.undo_stack.push_back(Transaction {
        id,
        selection_before,
        selection_after,
      });
      self.redo_stack.clear();
    }
  }

  fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
    self.selected_range = offset..offset;

    cx.notify();
  }

  pub fn cursor_offset(&self) -> usize {
    if self.selection_reversed {
      self.selected_range.start
    } else {
      self.selected_range.end
    }
  }

  fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
    if self.selection_reversed {
      self.selected_range.start = offset
    } else {
      self.selected_range.end = offset
    };
    if self.selected_range.end < self.selected_range.start {
      self.selection_reversed = !self.selection_reversed;
      self.selected_range = self.selected_range.end..self.selected_range.start;
    }
    cx.notify()
  }

  fn offset_from_utf16(&self, offset: usize, cx: &App) -> usize {
    let document = self.document.read(cx);
    let mut utf16_count = 0;

    for (char_offset, ch) in document.chars().enumerate() {
      if utf16_count >= offset {
        return char_offset;
      }
      utf16_count += ch.len_utf16();
    }

    document.len()
  }

  fn offset_to_utf16(&self, offset: usize, cx: &App) -> usize {
    let document = self.document.read(cx);
    let mut utf16_offset = 0;

    for (char_count, ch) in document.chars().enumerate() {
      if char_count >= offset {
        break;
      }
      utf16_offset += ch.len_utf16();
    }

    utf16_offset
  }

  fn range_to_utf16(&self, range: &Range<usize>, cx: &App) -> Range<usize> {
    self.offset_to_utf16(range.start, cx)..self.offset_to_utf16(range.end, cx)
  }

  fn range_from_utf16(&self, range_utf16: &Range<usize>, cx: &App) -> Range<usize> {
    self.offset_from_utf16(range_utf16.start, cx)..self.offset_from_utf16(range_utf16.end, cx)
  }

  fn previous_boundary(&self, offset: usize, _cx: &mut Context<Self>) -> usize {
    if offset == 0 {
      return 0;
    }

    // Simply move back one char - Ropey handles char boundaries correctly
    offset.saturating_sub(1)
  }

  fn previous_word_boundary(&self, offset: usize, cx: &mut Context<Self>) -> usize {
    if offset == 0 {
      return 0;
    }

    let doc = self.document.read(cx);

    // Work on a slice around the cursor instead of entire buffer
    // Get up to 1000 chars before cursor (enough for any reasonable word navigation)
    let start = offset.saturating_sub(1000);
    let slice = doc.slice_to_string(start..offset);
    let relative_offset = offset - start;

    // Find word boundaries in the slice
    let mut last_boundary = 0;
    for (idx, _) in slice.unicode_word_indices() {
      if idx < relative_offset {
        last_boundary = idx;
      } else {
        break;
      }
    }

    start + last_boundary
  }

  fn next_boundary(&self, offset: usize, cx: &mut Context<Self>) -> usize {
    let doc = self.document.read(cx);
    let doc_len = doc.len();

    if offset >= doc_len {
      return doc_len;
    }

    // Simply move forward one char - Ropey handles char boundaries correctly
    (offset + 1).min(doc_len)
  }

  fn next_word_boundary(&self, offset: usize, cx: &mut Context<Self>) -> usize {
    let doc = self.document.read(cx);
    let doc_len = doc.len();

    if offset >= doc_len {
      return doc_len;
    }

    // Work on a slice around the cursor instead of entire buffer
    // Get up to 1000 chars after cursor
    let end = (offset + 1000).min(doc_len);
    let slice = doc.slice_to_string(offset..end);

    // Find the next word boundary in the slice
    let mut word_indices = slice.unicode_word_indices();

    // Find the first word boundary after the start
    if let Some((idx, _)) = word_indices.next() {
      if idx > 0 {
        return offset + idx;
      }
      // If we're at the start of a word, find the next one
      if let Some((idx, _)) = word_indices.next() {
        return offset + idx;
      }
    }

    end
  }
}

impl EntityInputHandler for Editor {
  fn text_for_range(
    &mut self,
    range_utf16: Range<usize>,
    actual_range: &mut Option<Range<usize>>,
    _window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Option<String> {
    let doc = self.document.read(cx);
    let range = self.range_from_utf16(&range_utf16, cx);
    actual_range.replace(self.range_to_utf16(&range, cx));
    Some(doc.slice_to_string(range))
  }

  fn selected_text_range(
    &mut self,
    _ignore_disabled_input: bool,
    _window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Option<UTF16Selection> {
    Some(UTF16Selection {
      range: self.range_to_utf16(&self.selected_range, cx),
      reversed: self.selection_reversed,
    })
  }

  fn marked_text_range(
    &self,
    _window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Option<Range<usize>> {
    self
      .marked_range
      .as_ref()
      .map(|range| self.range_to_utf16(range, cx))
  }

  fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
    self.marked_range = None;
  }

  fn replace_text_in_range(
    &mut self,
    range_utf16: Option<Range<usize>>,
    new_text: &str,
    _: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let range = range_utf16
      .as_ref()
      .map(|range_utf16| self.range_from_utf16(range_utf16, cx))
      .or(self.marked_range.clone())
      .unwrap_or(self.selected_range.clone());

    let selection_before = self.selected_range.clone();
    let start_line = self.document.read(cx).char_to_line(range.start);
    let end_line = self.document.read(cx).char_to_line(range.end);

    let transaction_id = self.document.update(cx, |doc, cx| {
      let id = doc.buffer.transaction(Instant::now(), |buffer, tx| {
        buffer.replace(tx, range.clone(), new_text);
      });
      cx.notify();
      id
    });

    let has_newline = new_text.contains('\n');

    if has_newline || start_line != end_line {
      // Multi-line edit: invalidate from start line onwards
      self.invalidate_lines_from(start_line);
    } else {
      // Single-line edit: only invalidate the affected line
      self.invalidate_line(start_line);
    }

    self.selected_range = range.start + new_text.len()..range.start + new_text.len();
    self.marked_range.take();

    let selection_after = self.selected_range.clone();

    self.record_transaction(transaction_id, selection_before, selection_after);

    cx.notify();
  }

  fn replace_and_mark_text_in_range(
    &mut self,
    range_utf16: Option<Range<usize>>,
    new_text: &str,
    new_selected_range_utf16: Option<Range<usize>>,
    _window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let range = range_utf16
      .as_ref()
      .map(|range_utf16| self.range_from_utf16(range_utf16, cx))
      .or(self.marked_range.clone())
      .unwrap_or(self.selected_range.clone());

    let start_line = self.document.read(cx).char_to_line(range.start);

    self.document.update(cx, |doc, cx| {
      doc.replace(range.clone(), new_text, cx);
    });

    // Invalidate cache for all lines from the start of the edit
    self.invalidate_lines_from(start_line);

    if !new_text.is_empty() {
      self.marked_range = Some(range.start..range.start + new_text.len());
    } else {
      self.marked_range = None;
    }
    self.selected_range = new_selected_range_utf16
      .as_ref()
      .map(|range_utf16| self.range_from_utf16(range_utf16, cx))
      .map(|new_range| new_range.start + range.start..new_range.end + range.end)
      .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

    cx.notify();
  }

  fn bounds_for_range(
    &mut self,
    _range_utf16: Range<usize>,
    _bounds: Bounds<Pixels>,
    _window: &mut Window,
    _cx: &mut Context<Self>,
  ) -> Option<Bounds<Pixels>> {
    None
  }

  fn character_index_for_point(
    &mut self,
    _point: Point<Pixels>,
    _window: &mut Window,
    _cx: &mut Context<Self>,
  ) -> Option<usize> {
    None
  }
}

impl Render for Editor {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .key_context("Editor")
      .track_focus(&self.focus_handle(cx))
      .cursor(CursorStyle::IBeam)
      .size_full()
      .on_action(cx.listener(Self::enter))
      .on_action(cx.listener(Self::backspace))
      .on_action(cx.listener(Self::backspace_word))
      .on_action(cx.listener(Self::backspace_all))
      .on_action(cx.listener(Self::delete))
      .on_action(cx.listener(Self::up))
      .on_action(cx.listener(Self::down))
      .on_action(cx.listener(Self::left))
      .on_action(cx.listener(Self::alt_left))
      .on_action(cx.listener(Self::cmd_left))
      .on_action(cx.listener(Self::right))
      .on_action(cx.listener(Self::alt_right))
      .on_action(cx.listener(Self::cmd_right))
      .on_action(cx.listener(Self::cmd_up))
      .on_action(cx.listener(Self::cmd_down))
      .on_action(cx.listener(Self::select_cmd_left))
      .on_action(cx.listener(Self::select_cmd_right))
      .on_action(cx.listener(Self::select_cmd_up))
      .on_action(cx.listener(Self::select_cmd_down))
      .on_action(cx.listener(Self::select_up))
      .on_action(cx.listener(Self::select_down))
      .on_action(cx.listener(Self::select_left))
      .on_action(cx.listener(Self::select_word_left))
      .on_action(cx.listener(Self::select_right))
      .on_action(cx.listener(Self::select_word_right))
      .on_action(cx.listener(Self::select_all))
      .on_action(cx.listener(Self::home))
      .on_action(cx.listener(Self::end))
      .on_action(cx.listener(Self::show_character_palette))
      .on_action(cx.listener(Self::paste))
      .on_action(cx.listener(Self::cut))
      .on_action(cx.listener(Self::copy))
      .on_action(cx.listener(Self::undo))
      .on_action(cx.listener(Self::redo))
      .when_else(self.is_dark_mode, |el| el.bg(black()), |el| el.bg(white()))
      .when_else(
        self.is_dark_mode,
        |el| el.text_color(white()),
        |el| el.text_color(black()),
      )
      .child(EditorElement::new(cx.entity().clone()))
  }
}

impl Focusable for Editor {
  fn focus_handle(&self, _: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use gpui::TestAppContext;

  /// Helper context for testing Editor
  struct EditorTestContext {
    pub cx: TestAppContext,
    pub editor: Entity<Editor>,
  }

  impl EditorTestContext {
    /// Create a new test context with empty document
    #[allow(dead_code)]
    fn new(mut cx: TestAppContext) -> Self {
      let editor = cx.new(|cx| {
        let doc = cx.new(Document::new);
        Editor {
          document: doc,
          focus_handle: cx.focus_handle(),
          selected_range: 0..0,
          selection_reversed: false,
          marked_range: None,
          is_selecting: false,
          line_layouts: HashMap::new(),
          scroll_offset: 0.0,
          viewport_height: px(DEFAULT_VIEWPORT_HEIGHT),
          max_cache_size: MAX_CACHE_SIZE,
          target_column: None,
          undo_stack: VecDeque::new(),
          redo_stack: VecDeque::new(),
          is_dark_mode: false,
        }
      });

      Self { cx, editor }
    }

    /// Create a test context with specific text content
    fn with_text(mut cx: TestAppContext, text: &str) -> Self {
      let editor = cx.new(|cx| {
        let doc = cx.new(|cx| Document::with_text(text, cx));
        Editor {
          document: doc,
          focus_handle: cx.focus_handle(),
          selected_range: 0..0,
          selection_reversed: false,
          marked_range: None,
          is_selecting: false,
          line_layouts: HashMap::new(),
          scroll_offset: 0.0,
          viewport_height: px(DEFAULT_VIEWPORT_HEIGHT),
          max_cache_size: MAX_CACHE_SIZE,
          target_column: None,
          undo_stack: VecDeque::new(),
          redo_stack: VecDeque::new(),
          is_dark_mode: false,
        }
      });

      Self { cx, editor }
    }

    /// Create a test context with multiple lines for testing
    fn with_lines(cx: TestAppContext, count: usize) -> Self {
      let mut text = String::new();
      for i in 0..count {
        if i > 0 {
          text.push('\n');
        }
        text.push_str(&format!("Line {}", i + 1));
      }
      Self::with_text(cx, &text)
    }

    /// Get the current text content
    fn text(&self) -> String {
      self.editor.read_with(&self.cx, |editor, cx| {
        let doc = editor.document().read(cx);
        doc.slice_to_string(0..doc.len())
      })
    }

    /// Get the current cursor offset
    fn cursor_offset(&self) -> usize {
      self
        .editor
        .read_with(&self.cx, |editor, _| editor.cursor_offset())
    }

    /// Get the current selection range
    fn selection(&self) -> Range<usize> {
      self
        .editor
        .read_with(&self.cx, |editor, _| editor.selected_range.clone())
    }

    /// Get whether selection is reversed
    #[allow(dead_code)]
    fn selection_reversed(&self) -> bool {
      self
        .editor
        .read_with(&self.cx, |editor, _| editor.selection_reversed)
    }

    /// Set cursor position (collapses selection)
    fn set_cursor(&mut self, offset: usize) {
      self.editor.update(&mut self.cx, |editor, cx| {
        editor.move_to(offset, cx);
      });
    }

    /// Set selection range
    fn set_selection(&mut self, range: Range<usize>, reversed: bool) {
      self.editor.update(&mut self.cx, |editor, _| {
        editor.selected_range = range;
        editor.selection_reversed = reversed;
      });
    }

    /// Get the number of cached lines
    fn cache_size(&self) -> usize {
      self
        .editor
        .read_with(&self.cx, |editor, _| editor.line_layouts.len())
    }

    /// Check if a specific line is cached
    fn is_line_cached(&self, line_idx: usize) -> bool {
      self.editor.read_with(&self.cx, |editor, _| {
        editor.line_layouts.contains_key(&line_idx)
      })
    }
  }

  // ============================================================================
  // Cache Management Tests

  #[gpui::test]
  fn test_invalidate_line_single(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_lines(cx.clone(), 10);

    // Simulate cached lines
    ctx.editor.update(&mut ctx.cx, |editor, _| {
      for i in 0..5 {
        editor
          .line_layouts
          .insert(i, Arc::new(ShapedLine::default()));
      }
    });

    // Verify all are cached
    for i in 0..5 {
      assert!(ctx.is_line_cached(i));
    }

    // Invalidate line 2
    ctx.editor.update(&mut ctx.cx, |editor, _| {
      editor.invalidate_line(2);
    });

    // Line 2 should be removed, others stay
    assert!(ctx.is_line_cached(0));
    assert!(ctx.is_line_cached(1));
    assert!(!ctx.is_line_cached(2));
    assert!(ctx.is_line_cached(3));
    assert!(ctx.is_line_cached(4));
  }

  #[gpui::test]
  fn test_invalidate_lines_from(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_lines(cx.clone(), 10);

    // Simulate cached lines 0-9
    ctx.editor.update(&mut ctx.cx, |editor, _| {
      for i in 0..10 {
        editor
          .line_layouts
          .insert(i, Arc::new(ShapedLine::default()));
      }
    });

    assert_eq!(ctx.cache_size(), 10);

    // Invalidate from line 5
    ctx.editor.update(&mut ctx.cx, |editor, _| {
      editor.invalidate_lines_from(5);
    });

    // Lines 0-4 should remain, 5-9 should be removed
    assert!(ctx.is_line_cached(0));
    assert!(ctx.is_line_cached(4));
    assert!(!ctx.is_line_cached(5));
    assert!(!ctx.is_line_cached(9));
    assert_eq!(ctx.cache_size(), 5);
  }

  #[gpui::test]
  fn test_ensure_cache_size_limit(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_lines(cx.clone(), 300);

    // Fill cache beyond MAX_CACHE_SIZE
    ctx.editor.update(&mut ctx.cx, |editor, _| {
      for i in 0..250 {
        editor
          .line_layouts
          .insert(i, Arc::new(ShapedLine::default()));
      }
    });

    assert_eq!(ctx.cache_size(), 250);

    // Call ensure_cache_size with viewport at lines 100-120
    ctx.editor.update(&mut ctx.cx, |editor, _| {
      editor.ensure_cache_size(100..120);
    });

    // Cache should be reduced
    assert!(ctx.cache_size() < 250);

    // Lines near viewport should be kept (50..170 range)
    ctx.editor.read_with(&ctx.cx, |editor, _| {
      assert!(editor.line_layouts.contains_key(&100));
      assert!(editor.line_layouts.contains_key(&110));
    });
  }

  #[gpui::test]
  fn test_cache_retention_after_viewport_change(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_lines(cx.clone(), 100);

    // Cache lines 10-20
    ctx.editor.update(&mut ctx.cx, |editor, _| {
      for i in 10..=20 {
        editor
          .line_layouts
          .insert(i, Arc::new(ShapedLine::default()));
      }
    });

    // Ensure cache size with different viewport
    ctx.editor.update(&mut ctx.cx, |editor, _| {
      editor.ensure_cache_size(30..40);
    });

    // Old cache should still exist (under limit)
    assert!(ctx.is_line_cached(15));
  }

  #[gpui::test]
  fn test_invalidate_on_insert(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "line1\nline2\nline3");

    // Cache all lines
    ctx.editor.update(&mut ctx.cx, |editor, _| {
      for i in 0..3 {
        editor
          .line_layouts
          .insert(i, Arc::new(ShapedLine::default()));
      }
    });

    // Insert char on line 1 (offset 6 = start of "line2")
    ctx.set_cursor(6);
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.document.update(cx, |doc, cx| {
        doc.insert_char(6, 'X', cx);
      });
      editor.invalidate_line(1);
    });

    // Only line 1 should be invalidated
    assert!(ctx.is_line_cached(0));
    assert!(!ctx.is_line_cached(1));
    assert!(ctx.is_line_cached(2));
  }

  #[gpui::test]
  fn test_invalidate_on_newline(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "line1\nline2\nline3");

    // Cache all lines
    ctx.editor.update(&mut ctx.cx, |editor, _| {
      for i in 0..3 {
        editor
          .line_layouts
          .insert(i, Arc::new(ShapedLine::default()));
      }
    });

    // Insert newline on line 1
    ctx.set_cursor(6);
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      let current_line = editor.document.read(cx).char_to_line(6);
      editor.document.update(cx, |doc, cx| {
        doc.insert_char(6, '\n', cx);
      });
      editor.invalidate_lines_from(current_line);
    });

    // Lines from 1 onwards should be invalidated
    assert!(ctx.is_line_cached(0));
    assert!(!ctx.is_line_cached(1));
    assert!(!ctx.is_line_cached(2));
  }

  // ============================================================================
  // Navigation Tests
  // ============================================================================

  #[gpui::test]
  fn test_cursor_offset_initial(cx: &mut TestAppContext) {
    let ctx = EditorTestContext::with_text(cx.clone(), "hello world");
    assert_eq!(ctx.cursor_offset(), 0);
    assert_eq!(ctx.selection(), 0..0);
  }

  #[gpui::test]
  fn test_move_to(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    ctx.set_cursor(5);
    assert_eq!(ctx.cursor_offset(), 5);
    assert_eq!(ctx.selection(), 5..5);

    ctx.set_cursor(11);
    assert_eq!(ctx.cursor_offset(), 11);
  }

  #[gpui::test]
  fn test_left_navigation(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello");

    ctx.set_cursor(3);

    // Test the internal logic by checking cursor moved left
    let prev_offset = ctx.cursor_offset();
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      let new_offset = if editor.selected_range.is_empty() {
        editor.cursor_offset().saturating_sub(1)
      } else {
        editor.selected_range.start.min(editor.selected_range.end)
      };
      editor.move_to(new_offset, cx);
    });
    assert_eq!(ctx.cursor_offset(), prev_offset - 1);
  }

  #[gpui::test]
  fn test_left_at_start(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello");

    ctx.set_cursor(0);
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      let new_offset = editor.cursor_offset().saturating_sub(1);
      editor.move_to(new_offset, cx);
    });
    assert_eq!(ctx.cursor_offset(), 0); // Should stay at 0
  }

  #[gpui::test]
  fn test_right_navigation(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello");

    ctx.set_cursor(2);
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      let doc_len = editor.document().read(cx).len();
      let new_offset = if editor.selected_range.is_empty() {
        (editor.cursor_offset() + 1).min(doc_len)
      } else {
        editor.selected_range.start.max(editor.selected_range.end)
      };
      editor.move_to(new_offset, cx);
    });
    assert_eq!(ctx.cursor_offset(), 3);
  }

  #[gpui::test]
  fn test_right_at_end(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello");

    ctx.set_cursor(5);
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      let doc_len = editor.document().read(cx).len();
      let new_offset = (editor.cursor_offset() + 1).min(doc_len);
      editor.move_to(new_offset, cx);
    });
    assert_eq!(ctx.cursor_offset(), 5); // Should stay at end
  }

  // Note: Navigation tests that require Window are skipped for now
  // These will be tested with integration tests or VisualTestContext

  #[gpui::test]
  fn test_move_to_updates_cursor(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    ctx.set_cursor(7);
    assert_eq!(ctx.cursor_offset(), 7);
    assert_eq!(ctx.selection(), 7..7);
  }

  #[gpui::test]
  fn test_cursor_at_line_boundary(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "line1\nline2\nline3");

    // Test cursor at line starts
    ctx.set_cursor(0);
    assert_eq!(ctx.cursor_offset(), 0);

    ctx.set_cursor(6); // Start of line2
    assert_eq!(ctx.cursor_offset(), 6);

    ctx.set_cursor(12); // Start of line3
    assert_eq!(ctx.cursor_offset(), 12);
  }

  #[gpui::test]
  fn test_cursor_positioning(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    // Test various cursor positions
    for pos in [0, 5, 11] {
      ctx.set_cursor(pos);
      assert_eq!(ctx.cursor_offset(), pos);
      assert_eq!(ctx.selection(), pos..pos);
    }
  }

  // ============================================================================
  // Text Editing Tests
  // ============================================================================

  // Note: Text editing tests that require Window are skipped for now
  // The core logic is well-tested in buffer.rs and document.rs

  #[gpui::test]
  fn test_selection_with_replace(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    // Test replacing selection
    ctx.set_selection(2..7, false);
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      let range = editor.selected_range.clone();
      editor.document.update(cx, |doc, cx| {
        doc.replace(range, "X", cx);
      });
      editor.move_to(2, cx);
    });

    assert_eq!(ctx.text(), "heXorld");
    assert_eq!(ctx.cursor_offset(), 2);
  }

  #[gpui::test]
  fn test_insert_at_cursor(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello");

    ctx.set_cursor(5);
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      let cursor = editor.cursor_offset();
      editor.document.update(cx, |doc, cx| {
        doc.insert_char(cursor, '!', cx);
      });
      editor.move_to(cursor + 1, cx);
    });

    assert_eq!(ctx.text(), "hello!");
    assert_eq!(ctx.cursor_offset(), 6);
  }

  #[gpui::test]
  fn test_unicode_editing(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello 👋 world");

    // Verify emoji is present
    let text = ctx.text();
    assert!(text.contains("👋"));

    // Test cursor positioning around emoji
    ctx.set_cursor(6); // Before emoji
    assert_eq!(ctx.cursor_offset(), 6);

    ctx.set_cursor(7); // After emoji
    assert_eq!(ctx.cursor_offset(), 7);
  }

  #[gpui::test]
  fn test_cache_invalidation_on_edit(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "line1\nline2\nline3");

    // Cache all lines
    ctx.editor.update(&mut ctx.cx, |editor, _| {
      for i in 0..3 {
        editor
          .line_layouts
          .insert(i, Arc::new(ShapedLine::default()));
      }
    });

    // Edit line 1
    ctx.set_cursor(6);
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.document.update(cx, |doc, cx| {
        doc.insert_char(6, 'X', cx);
      });
      let line = editor.document.read(cx).char_to_line(6);
      editor.invalidate_line(line);
    });

    // Only line 1 should be invalidated
    assert!(ctx.is_line_cached(0));
    assert!(!ctx.is_line_cached(1));
    assert!(ctx.is_line_cached(2));
  }

  #[gpui::test]
  fn test_multiline_cache_invalidation(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "line1\nline2\nline3\nline4");

    // Cache all lines
    ctx.editor.update(&mut ctx.cx, |editor, _| {
      for i in 0..4 {
        editor
          .line_layouts
          .insert(i, Arc::new(ShapedLine::default()));
      }
    });

    // Insert newline on line 1
    ctx.set_cursor(6);
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      let line = editor.document.read(cx).char_to_line(6);
      editor.document.update(cx, |doc, cx| {
        doc.insert_char(6, '\n', cx);
      });
      editor.invalidate_lines_from(line);
    });

    // Lines from 1 onwards should be invalidated
    assert!(ctx.is_line_cached(0));
    assert!(!ctx.is_line_cached(1));
    assert!(!ctx.is_line_cached(2));
    assert!(!ctx.is_line_cached(3));
  }

  // ============================================================================
  // UTF-16 Conversion Tests
  // ============================================================================

  #[gpui::test]
  fn test_offset_to_utf16_ascii(cx: &mut TestAppContext) {
    let ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    let utf16_offset = ctx
      .editor
      .read_with(&ctx.cx, |editor, cx| editor.offset_to_utf16(5, cx));

    // ASCII: UTF-8 and UTF-16 offsets are the same
    assert_eq!(utf16_offset, 5);
  }

  #[gpui::test]
  fn test_offset_from_utf16_ascii(cx: &mut TestAppContext) {
    let ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    let utf8_offset = ctx
      .editor
      .read_with(&ctx.cx, |editor, cx| editor.offset_from_utf16(5, cx));

    assert_eq!(utf8_offset, 5);
  }

  #[gpui::test]
  fn test_offset_to_utf16_emoji(cx: &mut TestAppContext) {
    // "hello 👋 world" - emoji is 4 bytes in UTF-8, 2 code units in UTF-16
    let ctx = EditorTestContext::with_text(cx.clone(), "hello 👋 world");

    // Offset 6 is before emoji (after "hello ")
    let utf16_before = ctx
      .editor
      .read_with(&ctx.cx, |editor, cx| editor.offset_to_utf16(6, cx));
    assert_eq!(utf16_before, 6);

    // Offset 7 is after emoji (4-byte char)
    let utf16_after = ctx
      .editor
      .read_with(&ctx.cx, |editor, cx| editor.offset_to_utf16(7, cx));
    // In UTF-16: "hello " (6) + "👋" (2) = 8
    assert_eq!(utf16_after, 8);
  }

  #[gpui::test]
  fn test_offset_from_utf16_emoji(cx: &mut TestAppContext) {
    let ctx = EditorTestContext::with_text(cx.clone(), "hello 👋 world");

    // UTF-16 offset 6 = before emoji
    let utf8_before = ctx
      .editor
      .read_with(&ctx.cx, |editor, cx| editor.offset_from_utf16(6, cx));
    assert_eq!(utf8_before, 6);

    // UTF-16 offset 8 = after emoji (👋 is 2 UTF-16 code units)
    let utf8_after = ctx
      .editor
      .read_with(&ctx.cx, |editor, cx| editor.offset_from_utf16(8, cx));
    assert_eq!(utf8_after, 7); // 4-byte emoji = 1 char in UTF-8 offset
  }

  #[gpui::test]
  fn test_offset_to_utf16_multibyte(cx: &mut TestAppContext) {
    // "café" - é is 2 bytes in UTF-8, 1 code unit in UTF-16
    let ctx = EditorTestContext::with_text(cx.clone(), "café");

    let utf16_end = ctx.editor.read_with(&ctx.cx, |editor, cx| {
      editor.offset_to_utf16(5, cx) // 5 bytes: c(1) + a(1) + f(1) + é(2)
    });
    assert_eq!(utf16_end, 4); // 4 UTF-16 code units
  }

  #[gpui::test]
  fn test_range_to_utf16(cx: &mut TestAppContext) {
    let ctx = EditorTestContext::with_text(cx.clone(), "hello 👋 world");

    let utf16_range = ctx
      .editor
      .read_with(&ctx.cx, |editor, cx| editor.range_to_utf16(&(0..7), cx));

    // Range 0..7 in UTF-8 = "hello 👋"
    // In UTF-16: 0..8 (emoji is 2 code units)
    assert_eq!(utf16_range, 0..8);
  }

  #[gpui::test]
  fn test_range_from_utf16(cx: &mut TestAppContext) {
    let ctx = EditorTestContext::with_text(cx.clone(), "hello 👋 world");

    let utf8_range = ctx
      .editor
      .read_with(&ctx.cx, |editor, cx| editor.range_from_utf16(&(0..8), cx));

    // Range 0..8 in UTF-16 = "hello 👋"
    // In UTF-8: 0..7 (emoji is 4 bytes but counts as 1 char offset)
    assert_eq!(utf8_range, 0..7);
  }

  #[gpui::test]
  fn test_utf16_roundtrip(cx: &mut TestAppContext) {
    let ctx = EditorTestContext::with_text(cx.clone(), "hello 👋 世界");

    // Test roundtrip: UTF-8 -> UTF-16 -> UTF-8
    for offset in [0, 5, 6, 7, 8, 9] {
      let utf16 = ctx
        .editor
        .read_with(&ctx.cx, |editor, cx| editor.offset_to_utf16(offset, cx));
      let back_to_utf8 = ctx
        .editor
        .read_with(&ctx.cx, |editor, cx| editor.offset_from_utf16(utf16, cx));
      assert_eq!(
        back_to_utf8, offset,
        "Roundtrip failed for offset {}",
        offset
      );
    }
  }

  // ============================================================================
  // Word Boundary Tests
  // ============================================================================

  #[gpui::test]
  fn test_previous_word_boundary_simple(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world foo");

    // From end of "world" (offset 11), go to start of "world" (offset 6)
    let boundary = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.previous_word_boundary(11, cx)
    });
    assert_eq!(boundary, 6);
  }

  #[gpui::test]
  fn test_previous_word_boundary_at_start(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    let boundary = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.previous_word_boundary(0, cx)
    });
    assert_eq!(boundary, 0);
  }

  #[gpui::test]
  fn test_previous_word_boundary_multiple_spaces(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello   world");

    // From "world" back to start of "world"
    let boundary = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.previous_word_boundary(12, cx)
    });
    assert_eq!(boundary, 8);
  }

  #[gpui::test]
  fn test_previous_word_boundary_underscore(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "foo_bar_baz");

    // Underscores are part of words in unicode word segmentation
    let boundary = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.previous_word_boundary(11, cx)
    });
    assert_eq!(boundary, 0); // Should go to start of entire word
  }

  #[gpui::test]
  fn test_previous_word_boundary_punctuation(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello, world!");

    // From after "world" back to start of "world"
    let boundary = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.previous_word_boundary(12, cx)
    });
    assert_eq!(boundary, 7);
  }

  #[gpui::test]
  fn test_next_word_boundary_simple(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world foo");

    // From start of "hello" (offset 0), go to start of "world" (offset 6)
    let boundary = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| editor.next_word_boundary(0, cx));
    assert_eq!(boundary, 6);
  }

  #[gpui::test]
  fn test_next_word_boundary_at_end(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    let doc_len = ctx.text().len();
    let boundary = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.next_word_boundary(doc_len, cx)
    });
    assert_eq!(boundary, doc_len);
  }

  #[gpui::test]
  fn test_next_word_boundary_multiple_words(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "one two three");

    // From "one" to "two"
    let boundary1 = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| editor.next_word_boundary(0, cx));
    assert_eq!(boundary1, 4);

    // From "two" to "three"
    let boundary2 = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| editor.next_word_boundary(4, cx));
    assert_eq!(boundary2, 8);
  }

  #[gpui::test]
  fn test_next_word_boundary_punctuation(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello, world!");

    // From "hello" to "world"
    let boundary = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| editor.next_word_boundary(0, cx));
    assert_eq!(boundary, 7);
  }

  #[gpui::test]
  fn test_word_boundary_unicode(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello 世界 world");

    // Previous word boundary from end
    let prev = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.previous_word_boundary(13, cx)
    });
    assert_eq!(prev, 9); // Start of "world"

    // Next word boundary from start
    let next = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| editor.next_word_boundary(0, cx));
    assert_eq!(next, 6); // Start of "世界"
  }

  // ============================================================================
  // Character Boundary Tests
  // ============================================================================

  #[gpui::test]
  fn test_previous_boundary(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello");

    let boundary = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| editor.previous_boundary(3, cx));
    assert_eq!(boundary, 2);
  }

  #[gpui::test]
  fn test_previous_boundary_at_start(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello");

    let boundary = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| editor.previous_boundary(0, cx));
    assert_eq!(boundary, 0);
  }

  #[gpui::test]
  fn test_next_boundary(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello");

    let boundary = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| editor.next_boundary(2, cx));
    assert_eq!(boundary, 3);
  }

  #[gpui::test]
  fn test_next_boundary_at_end(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello");

    let boundary = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| editor.next_boundary(5, cx));
    assert_eq!(boundary, 5);
  }

  // ============================================================================
  // Selection Logic Tests
  // ============================================================================

  #[gpui::test]
  fn test_select_to_forward(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    ctx.set_cursor(0);
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.select_to(5, cx);
    });

    assert_eq!(ctx.selection(), 0..5);
    assert!(!ctx.selection_reversed());
  }

  #[gpui::test]
  fn test_select_to_backward(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    ctx.set_cursor(5);
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.select_to(0, cx);
    });

    assert_eq!(ctx.selection(), 0..5);
    assert!(ctx.selection_reversed());
  }

  #[gpui::test]
  fn test_select_to_extends_selection(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    // Start with selection 2..5
    ctx.set_selection(2..5, false);

    // Extend to 8
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.select_to(8, cx);
    });

    assert_eq!(ctx.selection(), 2..8);
    assert!(!ctx.selection_reversed());
  }

  #[gpui::test]
  fn test_select_to_reverses_direction(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    // Start with forward selection 2..5
    ctx.set_selection(2..5, false);

    // Select backwards past anchor
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.select_to(0, cx);
    });

    assert_eq!(ctx.selection(), 0..2);
    assert!(ctx.selection_reversed());
  }

  #[gpui::test]
  fn test_selection_anchor_preserved(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    // Set selection with anchor at 3
    ctx.set_selection(3..7, false);

    // Select to different position, anchor should stay at 3
    ctx.editor.update(&mut ctx.cx, |editor, cx| {
      editor.select_to(10, cx);
    });

    assert_eq!(ctx.selection(), 3..10);
  }
}
