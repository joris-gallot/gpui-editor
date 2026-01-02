use gpui::Hsla;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
  Keyword,
  KeywordControl,
  Function,
  FunctionMethod,
  FunctionSpecial,
  Type,
  TypeBuiltin,
  TypeInterface,
  TypeClass,
  String,
  StringEscape,
  StringRegex,
  Number,
  Boolean,
  Comment,
  CommentDoc,
  Operator,
  Variable,
  VariableSpecial,
  VariableParameter,
  Property,
  Constant,
  ConstantBuiltin,
  Punctuation,
  PunctuationBracket,
  PunctuationDelimiter,
  PunctuationSpecial,
  Attribute,
  Lifetime,
  Embedded,
}

#[derive(Debug, Clone)]
pub struct SyntaxTheme {
  pub keyword: Hsla,
  pub keyword_control: Hsla,
  pub function: Hsla,
  pub function_method: Hsla,
  pub function_special: Hsla,
  pub type_name: Hsla,
  pub type_builtin: Hsla,
  pub type_interface: Hsla,
  pub type_class: Hsla,
  pub string: Hsla,
  pub string_escape: Hsla,
  pub string_regex: Hsla,
  pub number: Hsla,
  pub boolean: Hsla,
  pub comment: Hsla,
  pub comment_doc: Hsla,
  pub operator: Hsla,
  pub variable: Hsla,
  pub variable_special: Hsla,
  pub variable_parameter: Hsla,
  pub property: Hsla,
  pub constant: Hsla,
  pub constant_builtin: Hsla,
  pub punctuation: Hsla,
  pub punctuation_bracket: Hsla,
  pub punctuation_delimiter: Hsla,
  pub punctuation_special: Hsla,
  pub attribute: Hsla,
  pub lifetime: Hsla,
  pub embedded: Hsla,
}

impl SyntaxTheme {
  pub fn color_for_token(&self, token_type: TokenType) -> Hsla {
    match token_type {
      TokenType::Keyword => self.keyword,
      TokenType::KeywordControl => self.keyword_control,
      TokenType::Function => self.function,
      TokenType::FunctionMethod => self.function_method,
      TokenType::FunctionSpecial => self.function_special,
      TokenType::Type => self.type_name,
      TokenType::TypeBuiltin => self.type_builtin,
      TokenType::TypeInterface => self.type_interface,
      TokenType::TypeClass => self.type_class,
      TokenType::String => self.string,
      TokenType::StringEscape => self.string_escape,
      TokenType::StringRegex => self.string_regex,
      TokenType::Number => self.number,
      TokenType::Boolean => self.boolean,
      TokenType::Comment => self.comment,
      TokenType::CommentDoc => self.comment_doc,
      TokenType::Operator => self.operator,
      TokenType::Variable => self.variable,
      TokenType::VariableSpecial => self.variable_special,
      TokenType::VariableParameter => self.variable_parameter,
      TokenType::Property => self.property,
      TokenType::Constant => self.constant,
      TokenType::ConstantBuiltin => self.constant_builtin,
      TokenType::Punctuation => self.punctuation,
      TokenType::PunctuationBracket => self.punctuation_bracket,
      TokenType::PunctuationDelimiter => self.punctuation_delimiter,
      TokenType::PunctuationSpecial => self.punctuation_special,
      TokenType::Attribute => self.attribute,
      TokenType::Lifetime => self.lifetime,
      TokenType::Embedded => self.embedded,
    }
  }
}

impl Default for SyntaxTheme {
  fn default() -> Self {
    Self {
      // Keywords - blue
      keyword: Hsla {
        h: 210.0 / 360.0,
        s: 0.59,
        l: 0.63,
        a: 1.0,
      }, // #569cd6
      keyword_control: Hsla {
        h: 291.0 / 360.0,
        s: 0.47,
        l: 0.63,
        a: 1.0,
      }, // #c586c0

      // Functions - yellow
      function: Hsla {
        h: 50.0 / 360.0,
        s: 0.61,
        l: 0.71,
        a: 1.0,
      }, // #dcdcaa
      function_method: Hsla {
        h: 50.0 / 360.0,
        s: 0.61,
        l: 0.71,
        a: 1.0,
      }, // #dcdcaa
      function_special: Hsla {
        h: 210.0 / 360.0,
        s: 0.59,
        l: 0.63,
        a: 1.0,
      }, // #569cd6 (macros in blue)

      // Types - cyan/turquoise
      type_name: Hsla {
        h: 167.0 / 360.0,
        s: 0.49,
        l: 0.59,
        a: 1.0,
      }, // #4ec9b0
      type_builtin: Hsla {
        h: 167.0 / 360.0,
        s: 0.49,
        l: 0.59,
        a: 1.0,
      }, // #4ec9b0
      type_interface: Hsla {
        h: 167.0 / 360.0,
        s: 0.49,
        l: 0.59,
        a: 1.0,
      }, // #4ec9b0
      type_class: Hsla {
        h: 167.0 / 360.0,
        s: 0.49,
        l: 0.59,
        a: 1.0,
      }, // #4ec9b0

      // Strings - orange
      string: Hsla {
        h: 25.0 / 360.0,
        s: 0.51,
        l: 0.63,
        a: 1.0,
      }, // #ce9178
      string_escape: Hsla {
        h: 50.0 / 360.0,
        s: 0.61,
        l: 0.71,
        a: 1.0,
      }, // #d7ba7d
      string_regex: Hsla {
        h: 341.0 / 360.0,
        s: 0.79,
        l: 0.68,
        a: 1.0,
      }, // #d16969

      // Numbers - light green
      number: Hsla {
        h: 99.0 / 360.0,
        s: 0.28,
        l: 0.71,
        a: 1.0,
      }, // #b5cea8
      boolean: Hsla {
        h: 210.0 / 360.0,
        s: 0.59,
        l: 0.63,
        a: 1.0,
      }, // #569cd6

      // Comments - green/gray
      comment: Hsla {
        h: 113.0 / 360.0,
        s: 0.23,
        l: 0.49,
        a: 1.0,
      }, // #6a9955
      comment_doc: Hsla {
        h: 113.0 / 360.0,
        s: 0.23,
        l: 0.55,
        a: 1.0,
      }, // #6a9955 (slightly brighter)

      // Operators - white
      operator: Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.86,
        a: 1.0,
      }, // #d4d4d4

      // Variables
      variable: Hsla {
        h: 215.0 / 360.0,
        s: 0.76,
        l: 0.78,
        a: 1.0,
      }, // #9cdcfe
      variable_special: Hsla {
        h: 210.0 / 360.0,
        s: 0.59,
        l: 0.63,
        a: 1.0,
      }, // #569cd6 (this, self)
      variable_parameter: Hsla {
        h: 215.0 / 360.0,
        s: 0.76,
        l: 0.78,
        a: 1.0,
      }, // #9cdcfe

      // Properties
      property: Hsla {
        h: 215.0 / 360.0,
        s: 0.76,
        l: 0.78,
        a: 1.0,
      }, // #9cdcfe

      // Constants
      constant: Hsla {
        h: 215.0 / 360.0,
        s: 0.76,
        l: 0.78,
        a: 1.0,
      }, // #9cdcfe
      constant_builtin: Hsla {
        h: 210.0 / 360.0,
        s: 0.59,
        l: 0.63,
        a: 1.0,
      }, // #569cd6 (true, false, null, etc.)

      // Punctuation - light gray
      punctuation: Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.83,
        a: 1.0,
      }, // #d4d4d4
      punctuation_bracket: Hsla {
        h: 50.0 / 360.0,
        s: 0.61,
        l: 0.71,
        a: 1.0,
      }, // #ffd700 (brackets in gold)
      punctuation_delimiter: Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.83,
        a: 1.0,
      }, // #d4d4d4
      punctuation_special: Hsla {
        h: 210.0 / 360.0,
        s: 0.59,
        l: 0.63,
        a: 1.0,
      }, // #569cd6

      // Attributes/Decorators - yellow
      attribute: Hsla {
        h: 50.0 / 360.0,
        s: 0.61,
        l: 0.71,
        a: 1.0,
      }, // #dcdcaa

      // Lifetime (Rust) - cyan
      lifetime: Hsla {
        h: 167.0 / 360.0,
        s: 0.49,
        l: 0.59,
        a: 1.0,
      }, // #4ec9b0

      // Embedded (template strings)
      embedded: Hsla {
        h: 215.0 / 360.0,
        s: 0.76,
        l: 0.78,
        a: 1.0,
      }, // #9cdcfe
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_default_theme_exists() {
    let theme = SyntaxTheme::default();
    assert_eq!(theme.keyword.a, 1.0);
  }

  #[test]
  fn test_color_for_token() {
    let theme = SyntaxTheme::default();
    let keyword_color = theme.color_for_token(TokenType::Keyword);
    assert_eq!(keyword_color, theme.keyword);

    let function_color = theme.color_for_token(TokenType::Function);
    assert_eq!(function_color, theme.function);

    let string_color = theme.color_for_token(TokenType::String);
    assert_eq!(string_color, theme.string);
  }

  #[test]
  fn test_all_token_types_have_colors() {
    let theme = SyntaxTheme::default();

    let token_types = [
      TokenType::Keyword,
      TokenType::Function,
      TokenType::Type,
      TokenType::String,
      TokenType::Number,
      TokenType::Comment,
      TokenType::Operator,
      TokenType::Variable,
      TokenType::Property,
      TokenType::Constant,
      TokenType::Punctuation,
    ];

    for token_type in token_types {
      let color = theme.color_for_token(token_type);
      assert!(color.a > 0.0);
    }
  }
}
