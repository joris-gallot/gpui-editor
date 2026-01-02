use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::time::Instant;
use tree_sitter::{InputEdit, Parser, Point, Query, QueryCursor, StreamingIterator, Tree};

use crate::theme::TokenType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
  Rust,
  TypeScript,
  JavaScript,
  PlainText,
}

impl Language {
  /// Detect language from file extension
  pub fn from_extension(extension: &str) -> Self {
    match extension {
      "rs" => Language::Rust,
      "ts" | "tsx" => Language::TypeScript,
      "js" | "jsx" => Language::JavaScript,
      _ => Language::PlainText,
    }
  }

  /// Get Tree-sitter grammar for this language
  fn tree_sitter_language(&self) -> Option<tree_sitter::Language> {
    match self {
      Language::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
      Language::TypeScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
      Language::JavaScript => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
      Language::PlainText => None,
    }
  }

  /// Get highlight queries for this language
  fn highlight_query(&self) -> Option<&'static str> {
    match self {
      Language::Rust => Some(include_str!("tree-sitter-queries/rust-highlights.scm")),
      Language::TypeScript | Language::JavaScript => Some(include_str!(
        "tree-sitter-queries/typescript-highlights.scm"
      )),
      Language::PlainText => None,
    }
  }
}

/// A highlight represents a colored token
#[derive(Debug, Clone)]
pub struct Highlight {
  pub range: Range<usize>, // Range en char offsets
  pub token_type: TokenType,
}

/// Parser and highlighter with incremental parsing support
pub struct SyntaxHighlighter {
  parser: Parser,
  tree: Option<Tree>,
  query: Query,
  source_text: String,
  /// Range of lines that have been parsed (None = nothing parsed yet)
  parsed_range: Option<Range<usize>>,
  /// Total number of lines in the document
  total_lines: usize,
  /// Cache of highlights per line
  highlight_cache: HashMap<usize, Vec<Highlight>>,
  /// Lines that need cache invalidation
  dirty_lines: HashSet<usize>,
  /// Cache statistics
  cache_hits: usize,
  cache_misses: usize,
}

impl SyntaxHighlighter {
  /// Create a new highlighter for a given language
  pub fn new(language: Language) -> Option<Self> {
    let ts_language = language.tree_sitter_language()?;
    let query_source = language.highlight_query()?;

    let mut parser = Parser::new();
    parser
      .set_language(&ts_language)
      .expect("Failed to set tree-sitter language");

    let query = Query::new(&ts_language, query_source).expect("Failed to create query");

    Some(Self {
      parser,
      tree: None,
      query,
      source_text: String::new(),
      parsed_range: None,
      total_lines: 0,
      highlight_cache: HashMap::new(),
      dirty_lines: HashSet::new(),
      cache_hits: 0,
      cache_misses: 0,
    })
  }

  /// Initialize with text but don't parse yet (true lazy loading)
  pub fn parse(&mut self, text: &str) {
    self.source_text = text.to_string();
    self.total_lines = text.lines().count();
    // Don't parse anything yet - will be done on-demand in parse_range
    self.tree = None;
    self.parsed_range = None;
    // Clear cache on full re-parse
    self.highlight_cache.clear();
    self.dirty_lines.clear();
    self.cache_hits = 0;
    self.cache_misses = 0;
  }

  /// Parse a specific range of lines (lazy parsing)
  pub fn parse_range(&mut self, line_range: Range<usize>) {
    if self.source_text.is_empty() {
      return;
    }

    // Check if we need to expand the parsed range
    let needs_parse = match &self.parsed_range {
      None => true,
      Some(parsed) => line_range.start < parsed.start || line_range.end > parsed.end,
    };

    if !needs_parse {
      return; // Already parsed
    }

    // Expand the range to include buffer (200 lines above and below)
    const PARSE_BUFFER: usize = 200;
    let expanded_start = line_range.start.saturating_sub(PARSE_BUFFER);
    let expanded_end = (line_range.end + PARSE_BUFFER).min(self.total_lines);

    // Merge with existing parsed range if any
    let new_range = match &self.parsed_range {
      None => expanded_start..expanded_end,
      Some(parsed) => {
        let start = expanded_start.min(parsed.start);
        let end = expanded_end.max(parsed.end);
        start..end
      }
    };

    // Parse the entire text on first request
    // Tree-sitter needs full context for accurate parsing and incremental updates
    if self.tree.is_none() {
      let start = Instant::now();
      self.tree = self.parser.parse(&self.source_text, None);
      let elapsed = start.elapsed();
      eprintln!(
        "🚀 Lazy parse: {} lines in {:.2}ms (requested lines {}-{})",
        self.total_lines,
        elapsed.as_secs_f64() * 1000.0,
        line_range.start,
        line_range.end
      );
    }

    self.parsed_range = Some(new_range);
  }

  /// Update parsing incrementally after an edit
  pub fn update(&mut self, edit: TextEdit, new_text: &str) {
    if let Some(tree) = &mut self.tree {
      // Convert edit to InputEdit for Tree-sitter
      let input_edit = InputEdit {
        start_byte: edit.start_byte,
        old_end_byte: edit.old_end_byte,
        new_end_byte: edit.new_end_byte,
        start_position: byte_to_point(&self.source_text, edit.start_byte),
        old_end_position: byte_to_point(&self.source_text, edit.old_end_byte),
        new_end_position: byte_to_point(new_text, edit.new_end_byte),
      };

      tree.edit(&input_edit);
    }

    self.source_text = new_text.to_string();
    self.total_lines = new_text.lines().count();

    // Mark affected lines as dirty (conservative: mark more than necessary)
    // Calculate which lines were affected by the edit
    let old_text_before_edit = &self.source_text[..edit.start_byte.min(self.source_text.len())];
    let start_line = old_text_before_edit.lines().count().saturating_sub(1);
    // Invalidate from edit line to end (conservative approach)
    for line in start_line..self.total_lines {
      self.dirty_lines.insert(line);
      self.highlight_cache.remove(&line);
    }

    // Re-parse with old tree for incremental parsing
    let start = Instant::now();
    self.tree = self.parser.parse(&self.source_text, self.tree.as_ref());
    let elapsed = start.elapsed();
    eprintln!(
      "🔄 Incremental parse: {:.2}ms (invalidated {} lines)",
      elapsed.as_secs_f64() * 1000.0,
      self.total_lines - start_line
    );
  }

  /// Get all highlights for the entire text
  #[cfg(test)]
  pub fn highlights(&mut self) -> Vec<Highlight> {
    // Parse everything if not already done
    if self.tree.is_none() && !self.source_text.is_empty() {
      let total_lines = self.total_lines;
      self.parse_range(0..total_lines);
    }

    let tree = match &self.tree {
      Some(t) => t,
      None => return Vec::new(),
    };

    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&self.query, tree.root_node(), self.source_text.as_bytes());

    let mut highlights = Vec::new();

    while let Some((match_, _)) = captures.next() {
      for capture in match_.captures {
        let node = capture.node;
        let capture_name = &self.query.capture_names()[capture.index as usize];

        if let Some(token_type) = capture_name_to_token_type(capture_name) {
          let start_byte = node.start_byte();
          let end_byte = node.end_byte();

          // Convert byte offsets to char offsets
          let start_char = byte_offset_to_char_offset(&self.source_text, start_byte);
          let end_char = byte_offset_to_char_offset(&self.source_text, end_byte);

          highlights.push(Highlight {
            range: start_char..end_char,
            token_type,
          });
        }
      }
    }

    highlights
  }

  /// Get highlights for a specific range (viewport optimization with lazy parsing and caching)
  pub fn highlights_for_range(&mut self, char_range: Range<usize>) -> Vec<Highlight> {
    // Calculate line range from char range
    let start_line = self.source_text[..char_range.start.min(self.source_text.len())]
      .lines()
      .count()
      .saturating_sub(1);
    let end_line = self.source_text[..char_range.end.min(self.source_text.len())]
      .lines()
      .count();

    // Ensure this range is parsed
    self.parse_range(start_line..end_line);

    let tree = match &self.tree {
      Some(t) => t,
      None => return Vec::new(),
    };

    // Check if we need to populate cache for any lines in this range
    let mut needs_query = false;
    for line_idx in start_line..=end_line {
      if !self.highlight_cache.contains_key(&line_idx) {
        needs_query = true;
        break;
      }
    }

    // If cache is complete for this range, return cached results
    if !needs_query {
      self.cache_hits += 1;
      let mut highlights = Vec::new();
      for line_idx in start_line..=end_line {
        if let Some(line_highlights) = self.highlight_cache.get(&line_idx) {
          highlights.extend(line_highlights.iter().cloned());
        }
      }
      return highlights;
    }

    // Cache miss - need to query Tree-sitter
    self.cache_misses += 1;
    if self.cache_misses % 100 == 0 {
      let total = self.cache_hits + self.cache_misses;
      let hit_rate = if total > 0 {
        (self.cache_hits as f64 / total as f64) * 100.0
      } else {
        0.0
      };
      eprintln!(
        "📊 Highlight cache: {} hits, {} misses ({:.1}% hit rate)",
        self.cache_hits, self.cache_misses, hit_rate
      );
    }

    // Query Tree-sitter and populate cache
    let start_byte = char_offset_to_byte_offset(&self.source_text, char_range.start);
    let end_byte = char_offset_to_byte_offset(&self.source_text, char_range.end);

    let mut cursor = QueryCursor::new();
    cursor.set_byte_range(start_byte..end_byte);

    let mut captures = cursor.captures(&self.query, tree.root_node(), self.source_text.as_bytes());

    let mut highlights = Vec::new();

    while let Some((match_, _)) = captures.next() {
      for capture in match_.captures {
        let node = capture.node;
        let capture_name = &self.query.capture_names()[capture.index as usize];

        if let Some(token_type) = capture_name_to_token_type(capture_name) {
          let start_byte = node.start_byte();
          let end_byte = node.end_byte();

          let start_char = byte_offset_to_char_offset(&self.source_text, start_byte);
          let end_char = byte_offset_to_char_offset(&self.source_text, end_byte);

          // Keep only highlights in the requested range
          if start_char < char_range.end && end_char > char_range.start {
            let highlight = Highlight {
              range: start_char..end_char,
              token_type,
            };

            // Add to cache for the line(s) this highlight belongs to
            let h_start_line = self.source_text[..start_char.min(self.source_text.len())]
              .lines()
              .count()
              .saturating_sub(1);
            let h_end_line = self.source_text[..end_char.min(self.source_text.len())]
              .lines()
              .count();

            for line in h_start_line..=h_end_line {
              self
                .highlight_cache
                .entry(line)
                .or_default()
                .push(highlight.clone());
            }

            highlights.push(highlight);
          }
        }
      }
    }

    highlights
  }
}

/// Represents a text edit for incremental parsing
#[derive(Debug, Clone)]
pub struct TextEdit {
  pub start_byte: usize,
  pub old_end_byte: usize,
  pub new_end_byte: usize,
}

/// Convert Tree-sitter capture name to TokenType
fn capture_name_to_token_type(name: &str) -> Option<TokenType> {
  // Parse names with dots (e.g. "function.method")
  let parts: Vec<&str> = name.split('.').collect();
  let base = parts[0];
  let modifier = parts.get(1).copied();

  match (base, modifier) {
    ("keyword", Some("control")) => Some(TokenType::KeywordControl),
    ("keyword", _) => Some(TokenType::Keyword),
    ("function", Some("method")) => Some(TokenType::FunctionMethod),
    ("function", Some("special")) => Some(TokenType::FunctionSpecial),
    ("function", _) => Some(TokenType::Function),
    ("type", Some("builtin")) => Some(TokenType::TypeBuiltin),
    ("type", Some("interface")) => Some(TokenType::TypeInterface),
    ("type", Some("class")) => Some(TokenType::TypeClass),
    ("type", _) => Some(TokenType::Type),
    ("string", Some("escape")) => Some(TokenType::StringEscape),
    ("string", Some("regex")) => Some(TokenType::StringRegex),
    ("string", _) => Some(TokenType::String),
    ("number", _) => Some(TokenType::Number),
    ("boolean", _) => Some(TokenType::Boolean),
    ("comment", Some("doc")) => Some(TokenType::CommentDoc),
    ("comment", _) => Some(TokenType::Comment),
    ("operator", _) => Some(TokenType::Operator),
    ("variable", Some("special")) => Some(TokenType::VariableSpecial),
    ("variable", Some("parameter")) => Some(TokenType::VariableParameter),
    ("variable", _) => Some(TokenType::Variable),
    ("property", _) => Some(TokenType::Property),
    ("constant", Some("builtin")) => Some(TokenType::ConstantBuiltin),
    ("constant", _) => Some(TokenType::Constant),
    ("punctuation", Some("bracket")) => Some(TokenType::PunctuationBracket),
    ("punctuation", Some("delimiter")) => Some(TokenType::PunctuationDelimiter),
    ("punctuation", Some("special")) => Some(TokenType::PunctuationSpecial),
    ("punctuation", _) => Some(TokenType::Punctuation),
    ("attribute", _) => Some(TokenType::Attribute),
    ("lifetime", _) => Some(TokenType::Lifetime),
    ("embedded", _) => Some(TokenType::Embedded),
    _ => None,
  }
}

/// Convert byte offset to Point (line, column) for Tree-sitter
fn byte_to_point(text: &str, byte_offset: usize) -> Point {
  let mut line = 0;
  let mut column = 0;

  for (i, ch) in text.char_indices() {
    if i >= byte_offset {
      break;
    }
    if ch == '\n' {
      line += 1;
      column = 0;
    } else {
      column += ch.len_utf8();
    }
  }

  Point::new(line, column)
}

/// Convert byte offset to char offset
fn byte_offset_to_char_offset(text: &str, byte_offset: usize) -> usize {
  text
    .char_indices()
    .take_while(|(i, _)| *i < byte_offset)
    .count()
}

/// Convert char offset to byte offset
fn char_offset_to_byte_offset(text: &str, char_offset: usize) -> usize {
  text
    .char_indices()
    .nth(char_offset)
    .map(|(i, _)| i)
    .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_language_from_extension() {
    assert_eq!(Language::from_extension("rs"), Language::Rust);
    assert_eq!(Language::from_extension("ts"), Language::TypeScript);
    assert_eq!(Language::from_extension("tsx"), Language::TypeScript);
    assert_eq!(Language::from_extension("js"), Language::JavaScript);
    assert_eq!(Language::from_extension("txt"), Language::PlainText);
  }

  #[test]
  fn test_create_rust_highlighter() {
    let highlighter = SyntaxHighlighter::new(Language::Rust);
    assert!(highlighter.is_some());
  }

  #[test]
  fn test_create_typescript_highlighter() {
    let highlighter = SyntaxHighlighter::new(Language::TypeScript);
    assert!(highlighter.is_some());
  }

  #[test]
  fn test_parse_rust_simple() {
    let mut highlighter = SyntaxHighlighter::new(Language::Rust).unwrap();
    let code = "fn main() {\n    let x = 42;\n}";
    highlighter.parse(code);

    let highlights = highlighter.highlights();
    assert!(!highlights.is_empty());

    // Check that we have keywords
    let has_keyword = highlights
      .iter()
      .any(|h| h.token_type == TokenType::Keyword);
    assert!(has_keyword, "Should have keyword highlights");
  }

  #[test]
  fn test_parse_typescript_simple() {
    let mut highlighter = SyntaxHighlighter::new(Language::TypeScript).unwrap();
    let code = "const x: number = 42;";
    highlighter.parse(code);

    let highlights = highlighter.highlights();
    assert!(!highlights.is_empty());

    // Check that we have keywords
    let has_keyword = highlights
      .iter()
      .any(|h| h.token_type == TokenType::Keyword);
    assert!(has_keyword, "Should have keyword highlights");
  }

  #[test]
  fn test_highlights_for_range() {
    let mut highlighter = SyntaxHighlighter::new(Language::Rust).unwrap();
    let code = "fn main() {\n    let x = 42;\n}";
    highlighter.parse(code);

    // Get only highlights from the first line
    let first_line_end = code.find('\n').unwrap_or(code.len());
    let highlights = highlighter.highlights_for_range(0..first_line_end);

    assert!(!highlights.is_empty());
    // All highlights must be within the range
    for h in &highlights {
      assert!(h.range.start < first_line_end);
    }
  }

  #[test]
  fn test_byte_to_char_offset() {
    let text = "hello world";
    assert_eq!(byte_offset_to_char_offset(text, 0), 0);
    assert_eq!(byte_offset_to_char_offset(text, 5), 5);
    assert_eq!(byte_offset_to_char_offset(text, 11), 11);
  }

  #[test]
  fn test_byte_to_char_offset_unicode() {
    let text = "héllo"; // é = 2 bytes
    assert_eq!(byte_offset_to_char_offset(text, 0), 0);
    assert_eq!(byte_offset_to_char_offset(text, 1), 1);
    assert_eq!(byte_offset_to_char_offset(text, 3), 2); // after é
    assert_eq!(byte_offset_to_char_offset(text, 6), 5);
  }

  #[test]
  fn test_char_to_byte_offset() {
    let text = "hello world";
    assert_eq!(char_offset_to_byte_offset(text, 0), 0);
    assert_eq!(char_offset_to_byte_offset(text, 5), 5);
    assert_eq!(char_offset_to_byte_offset(text, 11), 11);
  }

  #[test]
  fn test_char_to_byte_offset_unicode() {
    let text = "héllo"; // é = 2 bytes
    assert_eq!(char_offset_to_byte_offset(text, 0), 0);
    assert_eq!(char_offset_to_byte_offset(text, 1), 1);
    assert_eq!(char_offset_to_byte_offset(text, 2), 3); // after é
    assert_eq!(char_offset_to_byte_offset(text, 5), 6);
  }

  #[test]
  fn test_capture_name_to_token_type() {
    assert_eq!(
      capture_name_to_token_type("keyword"),
      Some(TokenType::Keyword)
    );
    assert_eq!(
      capture_name_to_token_type("keyword.control"),
      Some(TokenType::KeywordControl)
    );
    assert_eq!(
      capture_name_to_token_type("function"),
      Some(TokenType::Function)
    );
    assert_eq!(
      capture_name_to_token_type("function.method"),
      Some(TokenType::FunctionMethod)
    );
    assert_eq!(
      capture_name_to_token_type("type.builtin"),
      Some(TokenType::TypeBuiltin)
    );
    assert_eq!(
      capture_name_to_token_type("string.escape"),
      Some(TokenType::StringEscape)
    );
    assert_eq!(capture_name_to_token_type("unknown"), None);
  }

  #[test]
  fn test_incremental_parsing() {
    let mut highlighter = SyntaxHighlighter::new(Language::Rust).unwrap();
    let initial_code = "fn main() {\n    let x = 42;\n}";
    highlighter.parse(initial_code);

    let initial_count = highlighter.highlights().len();
    assert!(initial_count > 0);

    // Simulate an edit: add a new line
    let new_code = "fn main() {\n    let x = 42;\n    let y = 100;\n}";
    let edit = TextEdit {
      start_byte: 27, // After "let x = 42;\n"
      old_end_byte: 27,
      new_end_byte: 44, // After adding "    let y = 100;\n"
    };

    highlighter.update(edit, new_code);
    let new_count = highlighter.highlights().len();

    // We should have more highlights after the addition
    assert!(new_count > initial_count);
  }

  #[test]
  fn test_rust_keywords() {
    let mut highlighter = SyntaxHighlighter::new(Language::Rust).unwrap();
    highlighter.parse("fn main() { let mut x = 0; if x > 0 { } else { } }");

    let highlights = highlighter.highlights();

    let keywords: Vec<_> = highlights
      .iter()
      .filter(|h| h.token_type == TokenType::Keyword || h.token_type == TokenType::KeywordControl)
      .collect();

    assert!(
      keywords.len() >= 4,
      "Should have at least 4 keywords (including control keywords), found {}",
      keywords.len()
    );
  }

  #[test]
  fn test_rust_function_names() {
    let mut highlighter = SyntaxHighlighter::new(Language::Rust).unwrap();
    highlighter.parse("fn main() { foo(); }");

    let highlights = highlighter.highlights();
    let functions: Vec<_> = highlights
      .iter()
      .filter(|h| h.token_type == TokenType::Function)
      .collect();

    assert!(!functions.is_empty(), "Should have function highlights");
  }

  #[test]
  fn test_rust_strings() {
    let mut highlighter = SyntaxHighlighter::new(Language::Rust).unwrap();
    highlighter.parse(r#"let s = "hello";"#);

    let highlights = highlighter.highlights();
    let strings: Vec<_> = highlights
      .iter()
      .filter(|h| h.token_type == TokenType::String)
      .collect();

    assert!(!strings.is_empty(), "Should have string highlights");
  }

  #[test]
  fn test_rust_numbers() {
    let mut highlighter = SyntaxHighlighter::new(Language::Rust).unwrap();
    highlighter.parse("let x = 42;");

    let highlights = highlighter.highlights();
    let numbers: Vec<_> = highlights
      .iter()
      .filter(|h| h.token_type == TokenType::Number)
      .collect();

    assert!(!numbers.is_empty(), "Should have number highlights");
  }

  #[test]
  fn test_rust_comments() {
    let mut highlighter = SyntaxHighlighter::new(Language::Rust).unwrap();
    highlighter.parse("// This is a comment\nfn main() {}");

    let highlights = highlighter.highlights();
    let comments: Vec<_> = highlights
      .iter()
      .filter(|h| h.token_type == TokenType::Comment)
      .collect();

    assert!(!comments.is_empty(), "Should have comment highlights");
  }
}
