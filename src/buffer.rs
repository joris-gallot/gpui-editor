use std::{borrow::Cow, ops::Range};

use ropey::Rope;

#[derive(Clone, Debug)]
pub struct TextBuffer {
  text: Rope,
}

impl Default for TextBuffer {
  fn default() -> Self {
    Self::new()
  }
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

  pub fn slice_to_string(&self, range: Range<usize>) -> String {
    self.text.slice(range).to_string()
  }

  pub fn len_lines(&self) -> usize {
    self.text.len_lines()
  }

  /// Get line content without trailing newlines
  /// Returns Cow<str> to avoid allocation when no newlines need to be removed
  pub fn line_content(&self, line_idx: usize) -> Option<Cow<'_, str>> {
    if line_idx < self.len_lines() {
      let line_slice = self.text.line(line_idx);

      // Try fast path: borrow if line is contiguous in memory and has no newlines
      if let Some(line_str) = line_slice.as_str()
        && !line_str.ends_with('\n')
        && !line_str.ends_with('\r')
      {
        return Some(Cow::Borrowed(line_str));
      }

      // Slow path: line crosses chunk boundaries or has newlines, must allocate
      let mut owned = line_slice.to_string();
      if owned.ends_with('\n') {
        owned.pop();
        if owned.ends_with('\r') {
          owned.pop();
        }
      }
      Some(Cow::Owned(owned))
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

  pub fn insert(&mut self, char_index: usize, text: &str) {
    self.text.insert(char_index, text);
  }

  pub fn replace(&mut self, range: std::ops::Range<usize>, text: &str) {
    self.text.remove(range.clone());
    self.text.insert(range.start, text);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_new_buffer() {
    let buffer = TextBuffer::new();
    assert_eq!(buffer.len(), 0);
    assert!(buffer.is_empty());
    assert_eq!(buffer.len_lines(), 1); // Rope always has at least 1 line
  }

  #[test]
  fn test_insert_char() {
    let mut buffer = TextBuffer::new();
    buffer.insert_char(0, 'a');
    assert_eq!(buffer.len(), 1);
    assert!(!buffer.is_empty());

    buffer.insert_char(1, 'b');
    buffer.insert_char(2, 'c');
    assert_eq!(buffer.len(), 3);
    assert_eq!(buffer.slice_to_string(0..3), "abc");
  }

  #[test]
  fn test_insert_string() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "hello");
    assert_eq!(buffer.len(), 5);
    assert_eq!(buffer.slice_to_string(0..5), "hello");

    buffer.insert(5, " world");
    assert_eq!(buffer.len(), 11);
    assert_eq!(buffer.slice_to_string(0..11), "hello world");
  }

  #[test]
  fn test_insert_multiline() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "line1\nline2\nline3");
    assert_eq!(buffer.len_lines(), 3);
    assert_eq!(buffer.len(), 17); // "line1\nline2\nline3" = 17 chars
  }

  #[test]
  fn test_line_content() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "first line\nsecond line\nthird line");

    assert_eq!(buffer.line_content(0).as_deref(), Some("first line"));
    assert_eq!(buffer.line_content(1).as_deref(), Some("second line"));
    assert_eq!(buffer.line_content(2).as_deref(), Some("third line"));
    assert_eq!(buffer.line_content(3), None);
  }

  #[test]
  fn test_line_content_removes_newlines() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "line1\n");

    // Should not include the newline character
    assert_eq!(buffer.line_content(0).as_deref(), Some("line1"));
  }

  #[test]
  fn test_line_content_removes_crlf() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "line1\r\n");

    // Should remove both \r and \n
    assert_eq!(buffer.line_content(0).as_deref(), Some("line1"));
  }

  #[test]
  fn test_line_range() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "abc\ndef\nghi");

    assert_eq!(buffer.line_range(0), Some(0..4)); // "abc\n"
    assert_eq!(buffer.line_range(1), Some(4..8)); // "def\n"
    assert_eq!(buffer.line_range(2), Some(8..11)); // "ghi"
    assert_eq!(buffer.line_range(3), None);
  }

  #[test]
  fn test_line_range_last_line() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "first\nlast");

    // Last line should go to end of buffer
    let last_range = buffer.line_range(1).unwrap();
    assert_eq!(last_range.end, buffer.len());
  }

  #[test]
  fn test_char_to_line() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "abc\ndef\nghi");
    // Lines: 0:"abc\n" (0-3), 1:"def\n" (4-7), 2:"ghi" (8-10)

    assert_eq!(buffer.char_to_line(0), 0);
    assert_eq!(buffer.char_to_line(3), 0);
    assert_eq!(buffer.char_to_line(4), 1);
    assert_eq!(buffer.char_to_line(7), 1);
    assert_eq!(buffer.char_to_line(8), 2);
  }

  #[test]
  fn test_line_to_char() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "abc\ndef\nghi");

    assert_eq!(buffer.line_to_char(0), 0);
    assert_eq!(buffer.line_to_char(1), 4);
    assert_eq!(buffer.line_to_char(2), 8);
  }

  #[test]
  fn test_replace_single_char() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "hello");

    buffer.replace(1..2, "a"); // Replace 'e' with 'a'
    assert_eq!(buffer.slice_to_string(0..5), "hallo");
  }

  #[test]
  fn test_replace_multiple_chars() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "hello world");

    buffer.replace(6..11, "Rust"); // Replace "world" with "Rust"
    assert_eq!(buffer.slice_to_string(0..10), "hello Rust");
  }

  #[test]
  fn test_replace_with_empty() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "hello");

    buffer.replace(1..4, ""); // Delete "ell"
    assert_eq!(buffer.slice_to_string(0..2), "ho");
  }

  #[test]
  fn test_replace_with_longer_text() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "hi");

    buffer.replace(0..2, "hello world"); // Replace "hi" with "hello world"
    assert_eq!(buffer.slice_to_string(0..11), "hello world");
  }

  #[test]
  fn test_slice_to_string() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "hello world");

    assert_eq!(buffer.slice_to_string(0..5), "hello");
    assert_eq!(buffer.slice_to_string(6..11), "world");
    assert_eq!(buffer.slice_to_string(0..11), "hello world");
  }

  #[test]
  fn test_chars_iterator() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "abc");

    let chars: Vec<char> = buffer.chars().collect();
    assert_eq!(chars, vec!['a', 'b', 'c']);
  }

  #[test]
  fn test_unicode_chars() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "héllo 世界");

    // Should handle Unicode correctly
    assert_eq!(buffer.len(), 8); // 8 chars (not bytes)
    assert_eq!(buffer.slice_to_string(0..5), "héllo");
    assert_eq!(buffer.slice_to_string(6..8), "世界");
  }

  #[test]
  fn test_emoji() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "hello 👋");

    // Emoji is 1 char in Rust's char counting
    assert_eq!(buffer.len(), 7);
    let chars: Vec<char> = buffer.chars().collect();
    assert_eq!(chars[6], '👋');
  }

  #[test]
  fn test_empty_lines() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "\n\n\n");

    assert_eq!(buffer.len_lines(), 4); // 3 newlines create 4 lines
    assert_eq!(buffer.line_content(0).as_deref(), Some(""));
    assert_eq!(buffer.line_content(1).as_deref(), Some(""));
    assert_eq!(buffer.line_content(2).as_deref(), Some(""));
  }

  #[test]
  fn test_complex_multiline_replace() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "line1\nline2\nline3");

    // Replace entire second line with multiple lines
    buffer.replace(6..11, "new1\nnew2");

    assert_eq!(buffer.len_lines(), 4);
    assert_eq!(buffer.line_content(0).as_deref(), Some("line1"));
    assert_eq!(buffer.line_content(1).as_deref(), Some("new1"));
    assert_eq!(buffer.line_content(2).as_deref(), Some("new2"));
    assert_eq!(buffer.line_content(3).as_deref(), Some("line3"));
  }

  #[test]
  fn test_insert_at_middle() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "hello");
    buffer.insert(2, "123"); // Insert in middle: "he123llo"

    assert_eq!(buffer.slice_to_string(0..8), "he123llo");
  }

  #[test]
  fn test_consecutive_operations() {
    let mut buffer = TextBuffer::new();
    buffer.insert(0, "a");
    buffer.insert_char(1, 'b');
    buffer.insert(2, "c");
    buffer.replace(1..2, "BBB");

    assert_eq!(buffer.slice_to_string(0..5), "aBBBc");
  }
}
