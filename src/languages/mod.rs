pub mod rust;
pub mod typescript;

use crate::syntax::LanguageConfig;

pub fn detect_language_config(extension: &str) -> Option<&'static LanguageConfig> {
  match extension {
    "rs" => Some(&*rust::RUST_CONFIG),
    "ts" | "tsx" | "js" | "jsx" => Some(&*typescript::TYPESCRIPT_CONFIG),
    _ => None,
  }
}
