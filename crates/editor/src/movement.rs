use gpui::Context;
use unicode_segmentation::UnicodeSegmentation;

use crate::editor::Editor;

/// Move to the previous character boundary
pub fn previous_boundary(_editor: &Editor, offset: usize, _cx: &Context<Editor>) -> usize {
  if offset == 0 {
    return 0;
  }

  // Simply move back one char - Ropey handles char boundaries correctly
  offset.saturating_sub(1)
}

/// Move to the next character boundary
pub fn next_boundary(editor: &Editor, offset: usize, cx: &Context<Editor>) -> usize {
  let doc = editor.document.read(cx);
  let doc_len = doc.len();

  if offset >= doc_len {
    return doc_len;
  }

  // Simply move forward one char - Ropey handles char boundaries correctly
  (offset + 1).min(doc_len)
}

/// Move to the previous word boundary
pub fn previous_word_boundary(editor: &Editor, offset: usize, cx: &Context<Editor>) -> usize {
  if offset == 0 {
    return 0;
  }

  let doc = editor.document.read(cx);

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

/// Move to the next word boundary
pub fn next_word_boundary(editor: &Editor, offset: usize, cx: &Context<Editor>) -> usize {
  let doc = editor.document.read(cx);
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::editor::tests::EditorTestContext;
  use gpui::TestAppContext;

  // ============================================================================
  // Word Boundary Tests
  // ============================================================================

  #[gpui::test]
  fn test_previous_word_boundary_simple(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world foo");

    // From end of "world" (offset 11), go to start of "world" (offset 6)
    let boundary = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      previous_word_boundary(editor, 11, cx)
    });
    assert_eq!(boundary, 6);
  }

  #[gpui::test]
  fn test_previous_word_boundary_at_start(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    let boundary = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      previous_word_boundary(editor, 0, cx)
    });
    assert_eq!(boundary, 0);
  }

  #[gpui::test]
  fn test_previous_word_boundary_multiple_spaces(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello   world");

    // From "world" back to start of "world"
    let boundary = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      previous_word_boundary(editor, 12, cx)
    });
    assert_eq!(boundary, 8);
  }

  #[gpui::test]
  fn test_previous_word_boundary_underscore(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "foo_bar_baz");

    // Underscores are part of words in unicode word segmentation
    let boundary = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      previous_word_boundary(editor, 11, cx)
    });
    assert_eq!(boundary, 0); // Should go to start of entire word
  }

  #[gpui::test]
  fn test_previous_word_boundary_punctuation(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello, world!");

    // From after "world" back to start of "world"
    let boundary = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      previous_word_boundary(editor, 12, cx)
    });
    assert_eq!(boundary, 7);
  }

  #[gpui::test]
  fn test_next_word_boundary_simple(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world foo");

    // From start of "hello" (offset 0), go to start of "world" (offset 6)
    let boundary = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| next_word_boundary(editor, 0, cx));
    assert_eq!(boundary, 6);
  }

  #[gpui::test]
  fn test_next_word_boundary_at_end(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello world");

    let doc_len = ctx.text().len();
    let boundary = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      next_word_boundary(editor, doc_len, cx)
    });
    assert_eq!(boundary, doc_len);
  }

  #[gpui::test]
  fn test_next_word_boundary_multiple_words(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "one two three");

    // From "one" to "two"
    let boundary1 = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| next_word_boundary(editor, 0, cx));
    assert_eq!(boundary1, 4);

    // From "two" to "three"
    let boundary2 = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| next_word_boundary(editor, 4, cx));
    assert_eq!(boundary2, 8);
  }

  #[gpui::test]
  fn test_next_word_boundary_punctuation(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello, world!");

    // From "hello" to "world"
    let boundary = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| next_word_boundary(editor, 0, cx));
    assert_eq!(boundary, 7);
  }

  #[gpui::test]
  fn test_word_boundary_unicode(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello 世界 world");

    // Previous word boundary from end
    let prev = ctx.editor.update(&mut ctx.cx, |editor, cx| {
      previous_word_boundary(editor, 13, cx)
    });
    assert_eq!(prev, 9); // Start of "world"

    // Next word boundary from start
    let next = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| next_word_boundary(editor, 0, cx));
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
      .update(&mut ctx.cx, |editor, cx| previous_boundary(editor, 3, cx));
    assert_eq!(boundary, 2);
  }

  #[gpui::test]
  fn test_previous_boundary_at_start(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello");

    let boundary = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| previous_boundary(editor, 0, cx));
    assert_eq!(boundary, 0);
  }

  #[gpui::test]
  fn test_next_boundary(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello");

    let boundary = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| next_boundary(editor, 2, cx));
    assert_eq!(boundary, 3);
  }

  #[gpui::test]
  fn test_next_boundary_at_end(cx: &mut TestAppContext) {
    let mut ctx = EditorTestContext::with_text(cx.clone(), "hello");

    let boundary = ctx
      .editor
      .update(&mut ctx.cx, |editor, cx| next_boundary(editor, 5, cx));
    assert_eq!(boundary, 5);
  }
}
