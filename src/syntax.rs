use crate::theme::TokenType;
use std::ops::Range;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

/// Highlight span with token type
#[derive(Clone, Debug)]
pub struct HighlightSpan {
  pub byte_range: Range<usize>,
  pub token_type: TokenType,
}

/// Language configuration
pub struct LanguageConfig {
  pub name: &'static str,
  pub highlight_config: HighlightConfiguration,
  pub extensions: &'static [&'static str],
}

/// Syntax highlighting manager
pub struct SyntaxHighlighter {
  highlighter: Highlighter,
  pub config: &'static LanguageConfig,
}

impl SyntaxHighlighter {
  pub fn new(config: &'static LanguageConfig) -> Self {
    Self {
      highlighter: Highlighter::new(),
      config,
    }
  }

  /// Highlight complete text
  /// Returns Ok(highlights) or Err if parsing fails
  pub fn highlight_text(&mut self, text: &str) -> Result<Vec<HighlightSpan>, String> {
    let events = self
      .highlighter
      .highlight(&self.config.highlight_config, text.as_bytes(), None, |_| {
        None
      })
      .map_err(|e| format!("Highlight failed: {}", e))?;

    let mut highlights = Vec::new();
    let mut highlight_stack = Vec::new();

    for event in events {
      match event.map_err(|e| format!("Event error: {}", e))? {
        HighlightEvent::Source { start, end } => {
          if let Some(&highlight_idx) = highlight_stack.last() {
            highlights.push(HighlightSpan {
              byte_range: start..end,
              token_type: map_highlight_index_to_token_type(highlight_idx),
            });
          }
        }
        HighlightEvent::HighlightStart(idx) => {
          highlight_stack.push(idx.0);
        }
        HighlightEvent::HighlightEnd => {
          highlight_stack.pop();
        }
      }
    }

    Ok(highlights)
  }
}

/// Map highlight index to TokenType
fn map_highlight_index_to_token_type(idx: usize) -> TokenType {
  // Indices correspond to the order in highlight_names of HighlightConfiguration
  // See languages/rust.rs for the list of names
  match idx {
    0 => TokenType::Keyword,             // keyword
    1 => TokenType::KeywordControl,      // keyword.control
    2 => TokenType::Function,            // function
    3 => TokenType::FunctionMethod,      // function.method
    4 => TokenType::FunctionSpecial,     // function.macro
    5 => TokenType::Type,                // type
    6 => TokenType::TypeBuiltin,         // type.builtin
    7 => TokenType::String,              // string
    8 => TokenType::StringEscape,        // string.escape
    9 => TokenType::Number,              // number
    10 => TokenType::Comment,            // comment
    11 => TokenType::Variable,           // variable
    12 => TokenType::Property,           // property
    13 => TokenType::Constant,           // constant
    14 => TokenType::Operator,           // operator
    15 => TokenType::PunctuationBracket, // punctuation.bracket
    16 => TokenType::Attribute,          // attribute
    17 => TokenType::Lifetime,           // lifetime
    _ => TokenType::Variable,            // fallback
  }
}
