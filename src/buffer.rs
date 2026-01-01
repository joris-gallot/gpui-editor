use std::ops::Range;

use ropey::Rope;

#[derive(Clone, Debug)]
pub struct TextBuffer {
  text: Rope,
}

impl TextBuffer {
  pub fn new() -> Self {
    Self { text: Rope::new() }
  }

  pub fn insert_char(&mut self, index: usize, ch: char) {
    self.text.insert_char(index, ch);
  }

  pub fn len(&self) -> usize {
    self.text.len_chars()
  }

  pub fn is_empty(&self) -> bool {
    self.text.len_chars() == 0
  }

  pub fn chars(&self) -> impl Iterator<Item = char> {
    self.text.chars()
  }

  pub fn char_to_line(&self, char_idx: usize) -> usize {
    self.text.char_to_line(char_idx)
  }

  pub fn line_to_char(&self, line_idx: usize) -> usize {
    self.text.line_to_char(line_idx)
  }

  pub fn as_bytes(&self) -> Vec<u8> {
    self.text.bytes().collect()
  }

  pub fn slice_to_string(&self, range: Range<usize>) -> String {
    self.text.slice(range).to_string()
  }

  pub fn len_lines(&self) -> usize {
    self.text.len_lines()
  }

  pub fn line_content(&self, line_idx: usize) -> Option<String> {
    if line_idx < self.len_lines() {
      let mut line = self.text.line(line_idx).to_string();

      // Remove newline characters as GPUI's shape_line doesn't accept them
      if line.ends_with('\n') {
        line.pop();

        if line.ends_with('\r') {
          line.pop();
        }
      }

      Some(line)
    } else {
      None
    }
  }

  pub fn line_range(&self, line_idx: usize) -> Option<Range<usize>> {
    if line_idx < self.len_lines() {
      let start = self.line_to_char(line_idx);
      let end = if line_idx + 1 < self.len_lines() {
        self.line_to_char(line_idx + 1)
      } else {
        self.len()
      };
      Some(start..end)
    } else {
      None
    }
  }

  pub fn graphemes(&self) -> String {
    self.text.slice(..).to_string()
  }

  pub fn insert(&mut self, char_index: usize, text: &str) {
    self.text.insert(char_index, text);
  }

  pub fn replace(&mut self, range: std::ops::Range<usize>, text: &str) {
    self.text.remove(range.clone());
    self.text.insert(range.start, text);
  }
}
