//! # CSS parser
//!
//! Simple CSS parser and "renderer"

mod lexer;
mod rules;
mod selectors;
mod values;

use derive_more::From;
pub use lexer::{lex_source, CSSToken};
pub use rules::Rule;
pub use selectors::Selector;
use source_map::{Counter, SourceId, Span, StringWithSourceMap, ToString};
use std::{mem, path::Path};
use tokenizer_lib::{BufferedTokenQueue, Token, TokenReader};
pub use values::CSSValue;

#[derive(Debug)]
pub struct ParseError {
    pub reason: String,
    pub position: Span,
}

impl From<Option<(CSSToken, Token<CSSToken, Span>)>> for ParseError {
    fn from(opt: Option<(CSSToken, Token<CSSToken, Span>)>) -> Self {
        if let Some((expected_type, invalid_token)) = opt {
            Self {
                reason: format!(
                    "Expected '{:?}' found '{:?}'",
                    expected_type, invalid_token.0
                ),
                position: invalid_token.1,
            }
        } else {
            unreachable!()
        }
    }
}

/// Settings for rendering ASTNodes to CSS
#[derive(Clone)]
pub struct ToStringSettings {
    pub minify: bool,
    pub indent_with: String,
}

impl Default for ToStringSettings {
    fn default() -> Self {
        ToStringSettings {
            minify: false,
            indent_with: "    ".to_owned(),
        }
    }
}

impl ToStringSettings {
    /// Minified settings, ASTNode::to_string will not return whitespace
    pub fn minified() -> Self {
        ToStringSettings {
            minify: true,
            indent_with: "".to_owned(),
        }
    }
}

pub(crate) fn token_as_ident(token: Token<CSSToken, Span>) -> Result<(String, Span), ParseError> {
    if let CSSToken::Ident(val) = token.0 {
        Ok((val, token.1))
    } else {
        Err(ParseError {
            reason: format!("Expected ident found '{:?}'", token.0),
            position: token.1,
        })
    }
}

pub trait ASTNode: Sized + Send + Sync + 'static {
    /// Parses structure from string
    #[cfg(not(target_arch = "wasm32"))]
    fn from_string(
        string: String,
        source_id: SourceId,
        offset: Option<usize>,
    ) -> Result<Self, ParseError> {
        use std::thread;
        use tokenizer_lib::ParallelTokenQueue;

        if string.len() > 2048 {
            let (mut sender, mut reader) = ParallelTokenQueue::new();
            let parsing_thread = thread::spawn(move || {
                let res = Self::from_reader(&mut reader);
                if res.is_ok() {
                    reader.expect_next(CSSToken::EOS)?;
                }
                res
            });
            lexer::lex_source(&string, &mut sender, source_id, None)?;
            parsing_thread.join().expect("Parsing thread panicked")
        } else {
            let mut reader = BufferedTokenQueue::new();
            lexer::lex_source(&string, &mut reader, SourceId::null(), offset)?;
            let this = Self::from_reader(&mut reader);
            reader.expect_next(CSSToken::EOS)?;
            this
        }
    }

    /// Parses structure from string
    #[cfg(target_arch = "wasm32")]
    fn from_string(string: String) -> Result<Self, ParseError> {
        let mut reader = StaticTokenChannel::new();
        lexer::lex_source(&string, &mut reader)?;
        let this = Self::from_reader(&mut reader);
        reader.expect_next(CSSToken::EOS)?;
        this
    }

    /// Returns position of node as span **as it was parsed**. May be invalid or none after mutation
    fn get_position(&self) -> Option<&Span>;

    fn from_reader(reader: &mut impl TokenReader<CSSToken, Span>) -> Result<Self, ParseError>;

    /// Depth indicates the indentation of current block
    fn to_string_from_buffer(
        &self,
        buf: &mut impl ToString,
        settings: &ToStringSettings,
        depth: u8,
    );

    /// Returns structure as valid string. If `SourceMap` passed will add mappings to SourceMap
    fn to_string(&self, settings: &ToStringSettings) -> String {
        let mut buffer = String::new();
        self.to_string_from_buffer(&mut buffer, settings, 0);
        buffer
    }
}

/// A StyleSheet with a collection of rules
#[derive(Debug)]
pub struct StyleSheet {
    pub entries: Vec<Entry>,
}

#[derive(Debug, From)]
pub enum Entry {
    Rule(Rule),
    Comment(String),
}

impl StyleSheet {
    fn from_reader(reader: &mut impl TokenReader<CSSToken, Span>) -> Result<Self, ParseError> {
        let mut entries: Vec<Entry> = Vec::new();
        while let Some(peek) = reader.peek() {
            match peek {
                Token(CSSToken::EOS, _) => break,
                Token(CSSToken::Comment(_), _) => {
                    if let Token(CSSToken::Comment(comment), _) = reader.next().unwrap() {
                        entries.push(Entry::Comment(comment));
                    } else {
                        unreachable!()
                    }
                }
                _ => {
                    entries.push(Rule::from_reader(reader)?.into());
                }
            }
        }
        Ok(Self { entries })
    }

    fn to_string_from_buffer(&self, buf: &mut impl ToString, settings: &ToStringSettings) {
        for (idx, entry) in self.entries.iter().enumerate() {
            match entry {
                Entry::Rule(rule) => {
                    rule.to_string_from_buffer(buf, settings, 0);
                }
                Entry::Comment(comment) => {
                    if !settings.minify {
                        buf.push_str("/*");
                        buf.push_str_contains_new_line(comment);
                        buf.push_str("*/");
                    }
                }
            }
            if !settings.minify && idx + 1 < self.entries.len() {
                buf.push_new_line();
                buf.push_new_line();
            }
        }
    }

    pub fn to_string(&self, settings: Option<ToStringSettings>) -> String {
        let mut buf = String::new();
        self.to_string_from_buffer(&mut buf, &settings.unwrap_or_default());
        buf
    }

    /// TODO better return type
    pub fn to_string_with_source_map(
        &self,
        settings: Option<ToStringSettings>,
    ) -> (String, String) {
        let mut buf = StringWithSourceMap::new();
        self.to_string_from_buffer(&mut buf, &settings.unwrap_or_default());
        buf.build()
    }

    pub fn length(&self, settings: Option<ToStringSettings>) -> usize {
        let mut buf = Counter::new();
        self.to_string_from_buffer(&mut buf, &settings.unwrap_or_default());
        buf.get_count()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, ParseError> {
        use std::fs;

        let path_buf = path.as_ref().to_path_buf();
        let source = fs::read_to_string(path).unwrap();
        let source_id = SourceId::new(path_buf, source.clone());
        Self::from_string(source, source_id)
    }

    pub fn from_string(source: String, source_id: SourceId) -> Result<Self, ParseError> {
        use std::thread;
        use tokenizer_lib::ParallelTokenQueue;

        let (mut sender, mut reader) = ParallelTokenQueue::new();
        let parsing_thread = thread::spawn(move || {
            let res = Self::from_reader(&mut reader);
            if res.is_ok() {
                reader.expect_next(CSSToken::EOS)?;
            }
            res
        });

        lexer::lex_source(&source, &mut sender, source_id, None)?;
        parsing_thread.join().unwrap()
    }
}

/// Will "raise" or "unnest" rules in the stylesheet. Mutates StyleSheet
pub fn raise_nested_rules(stylesheet: &mut StyleSheet) {
    let mut raised_rules: Vec<Rule> = Vec::new();
    for entry in stylesheet.entries.iter_mut() {
        if let Entry::Rule(rule) = entry {
            raise_subrules(rule, &mut raised_rules);
        }
    }
    stylesheet
        .entries
        .extend(raised_rules.into_iter().map(Into::into));
}

/// Will remove nested rules leaving declarations in place
fn raise_subrules(rule: &mut Rule, raised_rules: &mut Vec<Rule>) {
    if let Some(nested_rules) = &mut rule.nested_rules {
        // Changing nested rule here
        for mut nested_rule in nested_rules.drain(..) {
            let old_selectors = mem::replace(&mut nested_rule.selectors, vec![]);
            for selector in rule.selectors.iter() {
                for nested_selector in old_selectors.iter().cloned() {
                    nested_rule
                        .selectors
                        .push(selector.nest_selector(nested_selector));
                }
            }
            raise_subrules(&mut nested_rule, raised_rules);
            raised_rules.push(nested_rule);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_rules() {
        let style_sheet = StyleSheet::from_string(
            include_str!("../examples/example1.css").to_owned(),
            SourceId::null(),
        )
        .unwrap();
        assert_eq!(style_sheet.entries.len(), 2);
    }

    #[test]
    fn style_sheet_to_string() {
        let source = include_str!("../examples/example1.css").to_owned();
        let style_sheet = StyleSheet::from_string(source.clone(), SourceId::null()).unwrap();
        assert_eq!(style_sheet.to_string(None), source.replace('\r', ""));
    }
}
