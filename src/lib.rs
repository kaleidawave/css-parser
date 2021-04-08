//! # CSS parser
//!
//! Simple CSS parser and "renderer"

mod lexer;
pub use lexer::{lex_source, CSSToken};
mod source_map;
pub use source_map::SourceMap;
mod selectors;
pub use selectors::Selector;
mod values;
pub use values::CSSValue;
mod rules;
use lazy_static::lazy_static;
pub use rules::Rule;
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        atomic::{AtomicU8, Ordering},
        RwLock,
    },
    thread,
};
use tokenizer_lib::{StaticTokenChannel, StreamedTokenChannel, Token, TokenReader};

lazy_static! {
    pub static ref SOURCE_IDS: RwLock<HashMap<SourceId, (String, Option<String>)>> =
        RwLock::new(HashMap::new());
}

static SOURCE_ID_COUNTER: AtomicU8 = AtomicU8::new(1);

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct SourceId(pub u8);

impl SourceId {
    fn new() -> Self {
        Self(SOURCE_ID_COUNTER.fetch_add(1, Ordering::SeqCst))
    }

    fn null() -> Self {
        Self(0)
    }
}

/// Position of token, line_start, column_start, line_end, column_end, source_id
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Span(pub usize, pub usize, pub usize, pub usize, pub SourceId);

impl Span {
    /// Returns whether the end of `self` is the start of `other`
    pub fn is_adjacent(&self, other: &Self) -> bool {
        self.4 == other.4 && self.2 == other.0 && self.3 == other.1
    }

    /// Returns a new `Span` which starts at the start of `self` a ends at the end of `other`
    pub fn union(&self, other: &Self) -> Span {
        Span(self.0, self.1, other.2, other.3, self.4.clone())
    }
}

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
pub struct ToStringSettings {
    pub minify: bool,
    pub indent_with: String,
    pub generate_source_map: bool,
}

impl std::default::Default for ToStringSettings {
    fn default() -> Self {
        ToStringSettings {
            minify: false,
            indent_with: "    ".to_owned(),
            generate_source_map: false,
        }
    }
}

impl ToStringSettings {
    /// Minified settings, ASTNode::to_string will not return whitespace
    pub fn minified() -> Self {
        ToStringSettings {
            minify: true,
            indent_with: "".to_owned(),
            generate_source_map: false,
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

pub struct ToStringer<'a>(pub &'a mut String, pub &'a mut Option<SourceMap>);

impl ToStringer<'_> {
    pub fn push(&mut self, chr: char) {
        if let Some(ref mut source_map) = self.1 {
            source_map.add_to_column(chr.len_utf16());
        }
        self.0.push(chr);
    }

    pub fn push_new_line(&mut self) {
        if let Some(ref mut source_map) = self.1 {
            source_map.add_new_line();
        }
        self.0.push('\n');
    }

    pub fn push_str(&mut self, slice: &str) {
        if let Some(ref mut source_map) = self.1 {
            source_map.add_to_column(slice.chars().count());
        }
        self.0.push_str(slice);
    }

    /// Used to push slices that may contain new lines
    pub fn push_str_contains_new_line(&mut self, slice: &str) {
        if let Some(source_map) = self.1 {
            for chr in slice.chars() {
                if chr == '\n' {
                    source_map.add_new_line()
                }
            }
        }
        self.0.push_str(slice);
    }

    pub fn add_mapping(
        &mut self,
        original_line: usize,
        original_column: usize,
        source_id: SourceId,
    ) {
        if let Some(ref mut source_map) = self.1 {
            source_map.add_mapping(original_line, original_column, source_id);
        }
    }
}

pub trait ASTNode: Sized {
    /// Parses structure from string
    #[cfg(not(target_arch = "wasm32"))]
    fn from_string(string: String) -> Result<Self, ParseError> {
        if string.len() > 2048 {
            let (mut sender, mut reader) = StreamedTokenChannel::new();
            let parse_source_thread =
                thread::spawn(move || lexer::lex_source(&string, SourceId::null(), &mut sender));

            let this = Self::from_reader(&mut reader).and_then(|val| {
                // Checks script parsing did not throw
                parse_source_thread.join().unwrap().and(Ok(val))
            });
            reader.expect_next(CSSToken::EOS)?;
            this
        } else {
            let mut reader = StaticTokenChannel::new();
            lexer::lex_source(&string, SourceId::null(), &mut reader)?;
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
        buf: &mut ToStringer<'_>,
        settings: &ToStringSettings,
        depth: u8,
    );

    /// Returns structure as valid string. If `SourceMap` passed will add mappings to SourceMap
    fn to_string(&self, settings: &ToStringSettings) -> (String, Option<SourceMap>) {
        let mut buffer = String::new();
        let mut source_map = if settings.generate_source_map {
            Some(SourceMap::new())
        } else {
            None
        };
        let mut to_stringer = ToStringer(&mut buffer, &mut source_map);
        self.to_string_from_buffer(&mut to_stringer, settings, 0);
        (buffer, source_map)
    }
}

/// A StyleSheet with a collection of rules
#[derive(Debug)]
pub struct StyleSheet {
    pub rules: Vec<Rule>,
}

impl ASTNode for StyleSheet {
    fn from_reader(reader: &mut impl TokenReader<CSSToken, Span>) -> Result<Self, ParseError> {
        let mut rules: Vec<Rule> = Vec::new();
        while reader.peek().is_some() && reader.peek().unwrap().0 != CSSToken::EOS {
            rules.push(Rule::from_reader(reader)?);
        }
        Ok(Self { rules })
    }

    fn to_string_from_buffer(
        &self,
        buf: &mut ToStringer<'_>,
        settings: &ToStringSettings,
        _depth: u8,
    ) {
        for (idx, rule) in self.rules.iter().enumerate() {
            rule.to_string_from_buffer(buf, settings, 0);
            if !settings.minify && idx + 1 < self.rules.len() {
                buf.push_new_line();
                buf.push_new_line();
            }
        }
    }

    fn get_position(&self) -> Option<&Span> {
        unimplemented!()
    }
}

impl StyleSheet {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_path(path: &Path, display_name: &String) -> Result<Self, ParseError> {
        use std::fs;
        let source = fs::read_to_string(path.clone()).unwrap();
        let (mut sender, mut reader) = StreamedTokenChannel::new();
        let source_id = SourceId::new();
        SOURCE_IDS.write().unwrap().insert(source_id, (display_name.clone(), Some(source.clone())));
        let parse_source_thread =
            thread::spawn(move || lexer::lex_source(&source, source_id, &mut sender));

        let this = Self::from_reader(&mut reader).and_then(|val| {
            // Checks script parsing did not throw
            parse_source_thread.join().unwrap().and(Ok(val))
        });
        reader.expect_next(CSSToken::EOS)?;
        this
    }
}

/// Will "raise" or "unnest" rules in the stylesheet. Mutates StyleSheet
pub fn raise_rules(style_sheet: &mut StyleSheet) {
    let mut raised_rules: Vec<Rule> = Vec::new();
    for rule in style_sheet.rules.iter_mut() {
        raise_subrules(rule, &mut raised_rules);
    }
    style_sheet.rules.append(&mut raised_rules);
}

/// Will remove nested rules leaving declarations in place
fn raise_subrules(rule: &mut Rule, raised_rules: &mut Vec<Rule>) {
    if let Some(nested_rules) = &mut rule.nested_rules {
        for mut nested_rule in nested_rules.drain(..) {
            nested_rule.selector = rule.selector.nest_selector(nested_rule.selector);
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
        let style_sheet =
            StyleSheet::from_string(include_str!("../examples/example1.css").to_owned()).unwrap();
        assert_eq!(style_sheet.rules.len(), 2);
    }

    #[test]
    fn style_sheet_to_string() {
        let source = include_str!("../examples/example1.css").to_owned();
        let style_sheet = StyleSheet::from_string(source.clone()).unwrap();
        assert_eq!(
            style_sheet.to_string(&ToStringSettings::default()).0,
            source.replace('\r', "")
        );
    }
}
