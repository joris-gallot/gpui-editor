use crate::buffer::TextBuffer;
use gpui::Context;
use std::ops::Range;

pub struct Document {
  buffer: TextBuffer,
}

impl Document {
  pub fn new(_cx: &mut Context<Self>) -> Self {
    Self {
      buffer: TextBuffer::new(),
    }
  }

  pub fn with_text(text: &str, cx: &mut Context<Self>) -> Self {
    let mut doc = Self::new(cx);
    doc.buffer.insert(0, text);
    doc
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

  pub fn line_content(&self, line_idx: usize) -> Option<String> {
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

  pub fn as_bytes(&self) -> Vec<u8> {
    self.buffer.as_bytes()
  }

  pub fn insert_char(&mut self, offset: usize, ch: char, cx: &mut Context<Self>) {
    self.buffer.insert_char(offset, ch);
    cx.notify();
  }

  pub fn replace(&mut self, range: Range<usize>, text: &str, cx: &mut Context<Self>) {
    self.buffer.replace(range, text);
    cx.notify();
  }
}
