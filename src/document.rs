use crate::buffer::TextBuffer;
use gpui::Context;
use std::{borrow::Cow, ops::Range, time::Instant};

pub struct Document {
  pub buffer: TextBuffer,
}

impl Document {
  #[cfg(test)]
  pub fn new(_cx: &mut Context<Self>) -> Self {
    Self {
      buffer: TextBuffer::new(),
    }
  }

  pub fn with_text(text: &str, _cx: &mut Context<Self>) -> Self {
    Self {
      buffer: TextBuffer::from_text(text),
    }
  }

  pub fn chars(&self) -> impl Iterator<Item = char> + '_ {
    self.buffer.chars()
  }

  pub fn len(&self) -> usize {
    self.buffer.len()
  }

  pub fn len_lines(&self) -> usize {
    self.buffer.len_lines()
  }

  pub fn is_empty(&self) -> bool {
    self.buffer.is_empty()
  }

  pub fn line_content(&self, line_idx: usize) -> Option<Cow<'_, str>> {
    self.buffer.line_content(line_idx)
  }

  pub fn line_range(&self, line_idx: usize) -> Option<Range<usize>> {
    self.buffer.line_range(line_idx)
  }

  pub fn slice_to_string(&self, range: Range<usize>) -> String {
    self.buffer.slice_to_string(range)
  }

  pub fn char_to_line(&self, char_idx: usize) -> usize {
    self.buffer.char_to_line(char_idx)
  }

  pub fn line_to_char(&self, line_idx: usize) -> usize {
    self.buffer.line_to_char(line_idx)
  }

  #[cfg(test)]
  pub fn insert_char(&mut self, offset: usize, ch: char, cx: &mut Context<Self>) {
    self.buffer.transaction(Instant::now(), |buffer, tx| {
      buffer.insert(tx, offset, &ch.to_string());
    });
    cx.notify();
  }

  pub fn replace(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
    self.buffer.transaction(Instant::now(), |buffer, tx| {
      buffer.replace(tx, range, text);
    });
    cx.notify();
  }

  pub fn undo(&mut self, cx: &mut Context<Self>) -> Option<crate::buffer::TransactionId> {
    let result = self.buffer.undo();
    if result.is_some() {
      cx.notify();
    }
    result
  }

  pub fn redo(&mut self, cx: &mut Context<Self>) -> Option<crate::buffer::TransactionId> {
    let result = self.buffer.redo();
    if result.is_some() {
      cx.notify();
    }
    result
  }

  #[cfg(test)]
  pub fn can_undo(&self) -> bool {
    self.buffer.can_undo()
  }

  #[cfg(test)]
  pub fn can_redo(&self) -> bool {
    self.buffer.can_redo()
  }

  #[cfg(test)]
  pub fn set_group_interval(&mut self, interval: std::time::Duration) {
    self.buffer.set_group_interval(interval);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use gpui::{AppContext, TestAppContext};

  #[gpui::test]
  fn test_new_document(cx: &mut TestAppContext) {
    let doc = cx.new(Document::new);
    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.len(), 0);
      assert!(doc.is_empty());
      assert_eq!(doc.len_lines(), 1);
    });
  }

  #[gpui::test]
  fn test_with_text(cx: &mut TestAppContext) {
    let doc = cx.new(|cx| Document::with_text("hello world", cx));
    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.len(), 11);
      assert!(!doc.is_empty());
      assert_eq!(doc.slice_to_string(0..5), "hello");
      assert_eq!(doc.slice_to_string(6..11), "world");
    });
  }

  #[gpui::test]
  fn test_insert_char(cx: &mut TestAppContext) {
    let doc = cx.new(|cx| Document::with_text("hello", cx));
    doc.update(cx, |doc, cx| {
      doc.insert_char(5, '!', cx);
      assert_eq!(doc.len(), 6);
      assert_eq!(doc.slice_to_string(0..6), "hello!");
    });
  }

  #[gpui::test]
  fn test_replace(cx: &mut TestAppContext) {
    let doc = cx.new(|cx| Document::with_text("hello world", cx));
    doc.update(cx, |doc, cx| {
      doc.replace(6..11, "Rust", cx);
      assert_eq!(doc.slice_to_string(0..10), "hello Rust");
    });
  }

  #[gpui::test]
  fn test_multiline_document(cx: &mut TestAppContext) {
    let doc = cx.new(|cx| Document::with_text("line1\nline2\nline3", cx));
    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.len_lines(), 3);
      assert_eq!(doc.line_content(0).as_deref(), Some("line1"));
      assert_eq!(doc.line_content(1).as_deref(), Some("line2"));
      assert_eq!(doc.line_content(2).as_deref(), Some("line3"));
    });
  }

  #[gpui::test]
  fn test_line_range(cx: &mut TestAppContext) {
    let doc = cx.new(|cx| Document::with_text("abc\ndef\nghi", cx));
    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.line_range(0), Some(0..4));
      assert_eq!(doc.line_range(1), Some(4..8));
      assert_eq!(doc.line_range(2), Some(8..11));
    });
  }

  #[gpui::test]
  fn test_char_line_conversion(cx: &mut TestAppContext) {
    let doc = cx.new(|cx| Document::with_text("abc\ndef\nghi", cx));
    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.char_to_line(0), 0);
      assert_eq!(doc.char_to_line(4), 1);
      assert_eq!(doc.char_to_line(8), 2);

      assert_eq!(doc.line_to_char(0), 0);
      assert_eq!(doc.line_to_char(1), 4);
      assert_eq!(doc.line_to_char(2), 8);
    });
  }

  #[gpui::test]
  fn test_chars_iterator(cx: &mut TestAppContext) {
    let doc = cx.new(|cx| Document::with_text("abc", cx));
    doc.read_with(cx, |doc, _| {
      let chars: Vec<char> = doc.chars().collect();
      assert_eq!(chars, vec!['a', 'b', 'c']);
    });
  }

  #[gpui::test]
  fn test_unicode_handling(cx: &mut TestAppContext) {
    let doc = cx.new(|cx| Document::with_text("héllo 世界", cx));
    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.len(), 8);
      assert_eq!(doc.slice_to_string(0..5), "héllo");
      assert_eq!(doc.slice_to_string(6..8), "世界");
    });
  }

  #[gpui::test]
  fn test_empty_lines(cx: &mut TestAppContext) {
    let doc = cx.new(|cx| Document::with_text("\n\n\n", cx));
    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.len_lines(), 4);
      assert_eq!(doc.line_content(0).as_deref(), Some(""));
      assert_eq!(doc.line_content(1).as_deref(), Some(""));
      assert_eq!(doc.line_content(2).as_deref(), Some(""));
    });
  }

  #[gpui::test]
  fn test_replace_multiline(cx: &mut TestAppContext) {
    let doc = cx.new(|cx| Document::with_text("line1\nline2\nline3", cx));
    doc.update(cx, |doc, cx| {
      doc.replace(6..11, "new1\nnew2", cx);
      assert_eq!(doc.len_lines(), 4);
      assert_eq!(doc.line_content(0).as_deref(), Some("line1"));
      assert_eq!(doc.line_content(1).as_deref(), Some("new1"));
      assert_eq!(doc.line_content(2).as_deref(), Some("new2"));
      assert_eq!(doc.line_content(3).as_deref(), Some("line3"));
    });
  }

  #[gpui::test]
  fn test_line_content_removes_newlines(cx: &mut TestAppContext) {
    let doc = cx.new(|cx| Document::with_text("line1\n", cx));
    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.line_content(0).as_deref(), Some("line1"));
    });
  }

  #[gpui::test]
  fn test_line_content_removes_crlf(cx: &mut TestAppContext) {
    let doc = cx.new(|cx| Document::with_text("line1\r\n", cx));
    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.line_content(0).as_deref(), Some("line1"));
    });
  }

  #[gpui::test]
  fn test_document_undo(cx: &mut TestAppContext) {
    let doc = cx.new(Document::new);

    doc.update(cx, |doc, _cx| {
      doc.set_group_interval(std::time::Duration::from_millis(0));
    });

    doc.update(cx, |doc, cx| {
      doc.insert_char(0, 'a', cx);
      doc.insert_char(1, 'b', cx);
    });

    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.slice_to_string(0..2), "ab");
    });

    doc.update(cx, |doc, cx| {
      assert!(doc.undo(cx).is_some());
    });

    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.len(), 1);
      assert_eq!(doc.slice_to_string(0..1), "a");
    });
  }

  #[gpui::test]
  fn test_document_redo(cx: &mut TestAppContext) {
    let doc = cx.new(Document::new);

    doc.update(cx, |doc, _cx| {
      doc.set_group_interval(std::time::Duration::from_millis(0));
    });

    doc.update(cx, |doc, cx| {
      doc.insert_char(0, 'a', cx);
      doc.undo(cx);
    });

    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.len(), 0);
    });

    doc.update(cx, |doc, cx| {
      assert!(doc.redo(cx).is_some());
    });

    doc.read_with(cx, |doc, _| {
      assert_eq!(doc.slice_to_string(0..1), "a");
    });
  }

  #[gpui::test]
  fn test_document_can_undo_redo(cx: &mut TestAppContext) {
    let doc = cx.new(Document::new);

    doc.update(cx, |doc, _cx| {
      doc.set_group_interval(std::time::Duration::from_millis(0));
    });

    doc.read_with(cx, |doc, _| {
      assert!(!doc.can_undo());
      assert!(!doc.can_redo());
    });

    doc.update(cx, |doc, cx| {
      doc.insert_char(0, 'a', cx);
    });

    doc.read_with(cx, |doc, _| {
      assert!(doc.can_undo());
      assert!(!doc.can_redo());
    });

    doc.update(cx, |doc, cx| {
      doc.undo(cx);
    });

    doc.read_with(cx, |doc, _| {
      assert!(!doc.can_undo());
      assert!(doc.can_redo());
    });
  }
}
